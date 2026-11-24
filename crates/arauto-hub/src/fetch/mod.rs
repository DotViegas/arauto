//! The download of a file `whatsapp_send_file` names by address (decision 0055, section 2;
//! decision 0024): the one HTTPS client of the hub.
//!
//! The address comes from a Hermes, and a Hermes may have been led by a text it read: it is
//! hostile (rule 10). Before any connection, and again after each redirect:
//!
//! - **`https://` only**, with a host and an optional port; no user, no `\`, nothing but visible
//!   ASCII. Anything else is refused.
//! - **The name is resolved with the resolver of the system** ([`SystemResolver`]), and the
//!   address is refused when **any** of the IPs it gives is not public ([`is_public`]):
//!   loopback, private, link-local, shared, unique local, multicast, unspecified, `0.0.0.0/8`,
//!   broadcast, reserved, the documentation ranges, and an IPv4 of those written inside an
//!   IPv6 (mapped, compatible, NAT64, 6to4, Teredo).
//! - **The hub connects to the IP it checked**: the name is never resolved again. The name goes
//!   in TLS (SNI) and in `Host`, and the certificate is checked against the public roots the
//!   connector also uses (`webpki-roots`), with the provider `ring` of `rustls`.
//! - **At most 3 redirects**, each one `https://` and checked again as above.
//! - **10 MB**, read in a stream and cut there; a declared size above it is refused before
//!   reading. **20 seconds** for the whole download, redirects included.
//!
//! Every refusal is one value of [`Refused`], a fixed word for the log; the address, the name,
//! the IP and what the server answered never go into an error or a log. The hub answers every
//! refusal with AR-715.
//!
//! The resolver is injected ([`Resolve`]), as the engine is: a resolver that gives a private
//! IP is refused like the system's. The only way around the checks is the allowance of the
//! tests of this module, which exists only in a test build (`#[cfg(test)]`), and a test reads
//! this file to prove that nothing of it is left outside such a build.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::limits::{
    WHATSAPP_FETCH_DEADLINE_MS, WHATSAPP_FETCH_REDIRECTS_MAX, WHATSAPP_FILE_BY_ADDRESS_MAX_BYTES,
    WHATSAPP_FILE_URL_MAX_LEN,
};
use bytes::Bytes;
use futures_util::future::BoxFuture;
use http_body_util::{BodyExt, Empty, Limited};
use hyper::header::{
    ACCEPT, ACCEPT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HOST, LOCATION, USER_AGENT,
};
use hyper::{Request, StatusCode};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

#[cfg(test)]
mod allowance;
#[cfg(test)]
mod tests;

/// The longest media type kept from an answer.
const MIMETYPE_MAX_LEN: usize = 127;

/// The longest host name.
const HOST_MAX_LEN: usize = 253;

/// Why a file could not be fetched. A fixed word each: nothing of the address or the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Not an `https://` address.
    NotHttps,
    /// An address out of the format: no host, a user, a bad port, a character out of place.
    Malformed,
    /// The name resolved to an IP that is not public, or is one.
    NotPublic,
    /// The name did not resolve.
    NotResolved,
    /// No connection to any of the IPs.
    Connect,
    /// TLS failed: the certificate, the name, the handshake.
    Tls,
    /// The server answered something other than the file, or the answer broke off.
    Status,
    /// Above 10 MB.
    TooLarge,
    /// The whole download took longer than its time.
    Timeout,
    /// More redirects than allowed.
    TooManyRedirects,
}

impl Refused {
    /// A fixed word for the log.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Refused::NotHttps => "not an https address",
            Refused::Malformed => "an address out of the format",
            Refused::NotPublic => "an address that is not public",
            Refused::NotResolved => "a name that did not resolve",
            Refused::Connect => "no connection",
            Refused::Tls => "the TLS failed",
            Refused::Status => "an answer that is not the file",
            Refused::TooLarge => "a file above the limit",
            Refused::Timeout => "no answer in time",
            Refused::TooManyRedirects => "too many redirects",
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What was downloaded. Its `Debug` prints neither the bytes nor the media type.
pub struct Fetched {
    /// The media type the server answered, when it gave one in the format.
    pub mimetype: Option<String>,
    /// The bytes, at most 10 MB.
    pub data: Vec<u8>,
}

impl fmt::Debug for Fetched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fetched({} bytes)", self.data.len())
    }
}

/// Resolves a host name into IPs. The hub uses the resolver of the system; a test gives its
/// own. Whatever it answers is checked by [`is_public`] before any connection.
pub trait Resolve: Send + Sync + fmt::Debug {
    /// The IPs of `host`. An error is a name that did not resolve.
    fn resolve<'a>(&'a self, host: &'a str, port: u16) -> BoxFuture<'a, Result<Vec<IpAddr>, ()>>;
}

/// The resolver of the system (`getaddrinfo`, through Tokio).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemResolver;

impl Resolve for SystemResolver {
    fn resolve<'a>(&'a self, host: &'a str, port: u16) -> BoxFuture<'a, Result<Vec<IpAddr>, ()>> {
        Box::pin(async move {
            let found = tokio::net::lookup_host((host, port))
                .await
                .map_err(|_| ())?;
            Ok(found.map(|address| address.ip()).collect())
        })
    }
}

/// The limits of one download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The largest file.
    pub max_bytes: usize,
    /// The whole download, redirects included.
    pub deadline: Duration,
    /// The most redirects followed.
    pub redirects: usize,
}

impl Limits {
    /// The limits of decision 0055: 10 MB, 20 seconds, 3 redirects.
    pub const DECIDED: Limits = Limits {
        max_bytes: WHATSAPP_FILE_BY_ADDRESS_MAX_BYTES,
        deadline: Duration::from_millis(WHATSAPP_FETCH_DEADLINE_MS.unsigned_abs()),
        redirects: WHATSAPP_FETCH_REDIRECTS_MAX,
    };
}

/// The HTTPS client of the hub, for the files of `whatsapp_send_file` with `url`.
pub struct Fetcher {
    resolver: Arc<dyn Resolve>,
    /// `None` only when the TLS provider refused its own defaults: every download is refused.
    tls: Option<Arc<rustls::ClientConfig>>,
    limits: Limits,
    #[cfg(test)]
    allowance: allowance::Allowance,
}

impl fmt::Debug for Fetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fetcher")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// The TLS of the client: the public roots, the provider `ring`, its safe defaults.
fn client_config(roots: rustls::RootCertStore) -> Option<Arc<rustls::ClientConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .ok()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Some(Arc::new(config))
}

fn public_roots() -> rustls::RootCertStore {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    roots
}

impl Fetcher {
    /// The client of the hub: the resolver of the system, the public roots, the limits of
    /// decision 0055.
    #[must_use]
    pub fn system() -> Self {
        Self::new(Arc::new(SystemResolver))
    }

    /// The client with another resolver, and everything else as [`Fetcher::system`]. What the
    /// resolver answers is checked all the same.
    #[must_use]
    pub fn new(resolver: Arc<dyn Resolve>) -> Self {
        Self {
            resolver,
            tls: client_config(public_roots()),
            limits: Limits::DECIDED,
            #[cfg(test)]
            allowance: allowance::Allowance::default(),
        }
    }

    /// The limits of a download.
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }

    /// Downloads the file at `url`, following up to the redirects of the limits, within the
    /// time of the limits.
    ///
    /// # Errors
    /// [`Refused`]: the address, the network, the server or the limits.
    pub async fn fetch(&self, url: &str) -> Result<Fetched, Refused> {
        tokio::time::timeout(self.limits.deadline, self.follow(url))
            .await
            .map_err(|_| Refused::Timeout)?
    }

    async fn follow(&self, url: &str) -> Result<Fetched, Refused> {
        let mut target = Target::parse(url)?;
        let mut redirects = 0usize;
        loop {
            match self.once(&target).await? {
                Step::File(file) => return Ok(file),
                Step::Redirect(location) => {
                    redirects = redirects.saturating_add(1);
                    if redirects > self.limits.redirects {
                        return Err(Refused::TooManyRedirects);
                    }
                    target = target.join(&location)?;
                }
            }
        }
    }

    /// Whether the hub may connect to `ip`: only a public IP.
    fn allowed(&self, ip: IpAddr) -> bool {
        #[cfg(test)]
        if self.allowance.lets(ip) {
            return true;
        }
        is_public(ip)
    }

    /// The checked IPs of the host of `target`: every one public, or a refusal.
    async fn checked(&self, target: &Target) -> Result<Vec<IpAddr>, Refused> {
        let ips = match &target.host {
            Host::Ip(ip) => vec![*ip],
            Host::Name(name) => self
                .resolver
                .resolve(name, target.port)
                .await
                .map_err(|()| Refused::NotResolved)?,
        };
        if ips.is_empty() {
            return Err(Refused::NotResolved);
        }
        if !ips.iter().all(|ip| self.allowed(*ip)) {
            return Err(Refused::NotPublic);
        }
        Ok(ips)
    }

    /// One request: the checks, the connection to a checked IP, TLS, the answer.
    async fn once(&self, target: &Target) -> Result<Step, Refused> {
        let ips = self.checked(target).await?;
        let tls = self.tls.clone().ok_or(Refused::Tls)?;
        let mut stream = None;
        for ip in ips {
            if let Ok(connected) = TcpStream::connect(SocketAddr::new(ip, target.port)).await {
                stream = Some(connected);
                break;
            }
        }
        let stream = stream.ok_or(Refused::Connect)?;
        let _ = stream.set_nodelay(true);
        let name = target.server_name()?;
        let stream = TlsConnector::from(tls)
            .connect(name, stream)
            .await
            .map_err(|_| Refused::Tls)?;
        let (mut sender, connection) =
            hyper::client::conn::http1::handshake::<_, Empty<Bytes>>(TokioIo::new(stream))
                .await
                .map_err(|_| Refused::Connect)?;
        // The connection is driven beside the request and ends with it.
        let driver = AbortOnDrop(tokio::spawn(async move {
            let _ = connection.await;
        }));
        let request = Request::builder()
            .method(hyper::Method::GET)
            .uri(target.path.as_str())
            .header(HOST, target.authority())
            .header(USER_AGENT, "arauto-hub")
            .header(ACCEPT, "*/*")
            .header(ACCEPT_ENCODING, "identity")
            .body(Empty::<Bytes>::new())
            .map_err(|_| Refused::Malformed)?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| Refused::Status)?;
        let status = response.status();
        if matches!(
            status,
            StatusCode::MOVED_PERMANENTLY
                | StatusCode::FOUND
                | StatusCode::SEE_OTHER
                | StatusCode::TEMPORARY_REDIRECT
                | StatusCode::PERMANENT_REDIRECT
        ) {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(Refused::Status)?
                .to_owned();
            drop(driver);
            return Ok(Step::Redirect(location));
        }
        if status != StatusCode::OK {
            return Err(Refused::Status);
        }
        let max = self.limits.max_bytes;
        // Before reading: a declared size above the limit is refused at once.
        let declared = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok());
        if declared.is_some_and(|size| size > max as u64) {
            return Err(Refused::TooLarge);
        }
        let mimetype = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|text| !text.is_empty() && text.len() <= MIMETYPE_MAX_LEN)
            .map(str::to_owned);
        // Read in a stream and cut at the limit.
        let data = Limited::new(response.into_body(), max)
            .collect()
            .await
            .map_err(|error| {
                if error.is::<http_body_util::LengthLimitError>() {
                    Refused::TooLarge
                } else {
                    Refused::Status
                }
            })?
            .to_bytes();
        drop(driver);
        if data.is_empty() {
            return Err(Refused::Status);
        }
        Ok(Step::File(Fetched {
            mimetype,
            data: data.to_vec(),
        }))
    }
}

/// What one request gave.
enum Step {
    File(Fetched),
    Redirect(String),
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

// ---------------------------------------------------------------------------- the address

/// The host of an address.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Host {
    /// A name, in lowercase.
    Name(String),
    /// An IP written in the address.
    Ip(IpAddr),
}

/// An `https://` address, read strictly.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    host: Host,
    port: u16,
    /// The path and the query, starting with `/`, without the fragment.
    path: String,
}

impl Target {
    /// Reads an absolute address.
    fn parse(url: &str) -> Result<Self, Refused> {
        if url.is_empty()
            || url.len() > WHATSAPP_FILE_URL_MAX_LEN
            || !url.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Refused::Malformed);
        }
        let Some(rest) = url.strip_prefix("https://") else {
            return Err(Refused::NotHttps);
        };
        if url.contains('\\') {
            return Err(Refused::Malformed);
        }
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (authority, rest) = rest.split_at(end);
        let (host, port) = authority_parts(authority)?;
        let rest = rest.split('#').next().unwrap_or_default();
        let path = if rest.is_empty() {
            "/".to_owned()
        } else if rest.starts_with('?') {
            format!("/{rest}")
        } else {
            rest.to_owned()
        };
        Ok(Self { host, port, path })
    }

    /// The address a redirect leads to: absolute (`https://` only), without the scheme
    /// (`//host/...`), or a path of this host (`/...`). Any other form is refused.
    fn join(&self, location: &str) -> Result<Self, Refused> {
        if location.starts_with("//") {
            return Self::parse(&format!("https:{location}"));
        }
        if location.starts_with('/') {
            return Self::parse(&format!("https://{}{location}", self.authority()));
        }
        if location
            .get(..7)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
        {
            return Err(Refused::NotHttps);
        }
        if location.contains("://") {
            return Self::parse(location);
        }
        Err(Refused::Malformed)
    }

    /// `host[:port]`, as it goes in `Host`: the port only when it is not 443.
    fn authority(&self) -> String {
        let host = match &self.host {
            Host::Name(name) => name.clone(),
            Host::Ip(IpAddr::V4(ip)) => ip.to_string(),
            Host::Ip(IpAddr::V6(ip)) => format!("[{ip}]"),
        };
        if self.port == 443 {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }

    /// The name in TLS.
    fn server_name(&self) -> Result<ServerName<'static>, Refused> {
        match &self.host {
            Host::Name(name) => ServerName::try_from(name.clone()).map_err(|_| Refused::Malformed),
            Host::Ip(ip) => Ok(ServerName::IpAddress((*ip).into())),
        }
    }
}

/// The host and the port of `authority`.
fn authority_parts(authority: &str) -> Result<(Host, u16), Refused> {
    if authority.is_empty() || authority.contains('@') {
        return Err(Refused::Malformed);
    }
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (inside, after) = rest.split_once(']').ok_or(Refused::Malformed)?;
        let ip: Ipv6Addr = inside.parse().map_err(|_| Refused::Malformed)?;
        let port = match after {
            "" => None,
            after => Some(after.strip_prefix(':').ok_or(Refused::Malformed)?),
        };
        (Host::Ip(IpAddr::V6(ip)), port)
    } else {
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        };
        (name_or_ip(host)?, port)
    };
    let port = match port {
        None => 443,
        Some(port) => {
            if port.is_empty() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Refused::Malformed);
            }
            match port.parse::<u16>() {
                Ok(port) if port > 0 => port,
                _ => return Err(Refused::Malformed),
            }
        }
    };
    Ok((host, port))
}

/// A host written in an address: an IPv4 in dotted form, or a name of letters, digits,
/// hyphens and dots.
fn name_or_ip(host: &str) -> Result<Host, Refused> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Ok(Host::Ip(IpAddr::V4(ip)));
    }
    let host = host.to_ascii_lowercase();
    let label_ok = |label: &str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    if host.is_empty() || host.len() > HOST_MAX_LEN || !host.split('.').all(label_ok) {
        return Err(Refused::Malformed);
    }
    Ok(Host::Name(host))
}

// ---------------------------------------------------------------------------- the IPs

/// Whether `ip` is a public address of the internet: none of the ranges of decision 0055, and
/// none of the other ranges reserved for a special use.
#[must_use]
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => is_public_v6(ip),
    }
}

/// `ip` is in `network/prefix`.
fn in_v4(ip: Ipv4Addr, network: [u8; 4], prefix: u32) -> bool {
    let mask = u32::MAX.checked_shl(32 - prefix).unwrap_or(0);
    u32::from(ip) & mask == u32::from(Ipv4Addr::from(network)) & mask
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    const NOT_PUBLIC: [([u8; 4], u32); 16] = [
        ([0, 0, 0, 0], 8),          // "this network", the unspecified address among them
        ([10, 0, 0, 0], 8),         // private
        ([100, 64, 0, 0], 10),      // shared address space
        ([127, 0, 0, 0], 8),        // loopback
        ([169, 254, 0, 0], 16),     // link-local
        ([172, 16, 0, 0], 12),      // private
        ([192, 0, 0, 0], 24),       // IETF protocol assignments
        ([192, 0, 2, 0], 24),       // documentation (TEST-NET-1)
        ([192, 88, 99, 0], 24),     // the old 6to4 relay
        ([192, 168, 0, 0], 16),     // private
        ([198, 18, 0, 0], 15),      // benchmarking
        ([198, 51, 100, 0], 24),    // documentation (TEST-NET-2)
        ([203, 0, 113, 0], 24),     // documentation (TEST-NET-3)
        ([224, 0, 0, 0], 4),        // multicast
        ([240, 0, 0, 0], 4),        // reserved
        ([255, 255, 255, 255], 32), // broadcast
    ];
    !NOT_PUBLIC
        .iter()
        .any(|(network, prefix)| in_v4(ip, *network, *prefix))
}

/// `ip` is in `network/prefix`.
fn in_v6(ip: Ipv6Addr, network: [u16; 8], prefix: u32) -> bool {
    let mask = u128::MAX.checked_shl(128 - prefix).unwrap_or(0);
    u128::from(ip) & mask == u128::from(Ipv6Addr::from(network)) & mask
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let bits = u128::from(ip);
    // An IPv4 written inside an IPv6 is judged as that IPv4.
    let low_v4 = Ipv4Addr::from(u32::try_from(bits & 0xffff_ffff).unwrap_or(0));
    if ip.is_unspecified() || ip.is_loopback() {
        return false;
    }
    // ::ffff:a.b.c.d (mapped), ::a.b.c.d (compatible), 64:ff9b::a.b.c.d (NAT64) and
    // 64:ff9b:1::/48 (local NAT64).
    if in_v6(ip, [0, 0, 0, 0, 0, 0xffff, 0, 0], 96)
        || in_v6(ip, [0, 0, 0, 0, 0, 0, 0, 0], 96)
        || in_v6(ip, [0x64, 0xff9b, 0, 0, 0, 0, 0, 0], 96)
    {
        return is_public_v4(low_v4);
    }
    // 2002:a.b.c.d::/48 (6to4).
    if in_v6(ip, [0x2002, 0, 0, 0, 0, 0, 0, 0], 16) {
        let embedded = Ipv4Addr::from(u32::try_from((bits >> 80) & 0xffff_ffff).unwrap_or(0));
        return is_public_v4(embedded);
    }
    const NOT_PUBLIC: [([u16; 8], u32); 10] = [
        ([0x64, 0xff9b, 1, 0, 0, 0, 0, 0], 48),  // local NAT64
        ([0x100, 0, 0, 0, 0, 0, 0, 0], 64),      // discard only
        ([0x2001, 0, 0, 0, 0, 0, 0, 0], 32),     // Teredo: an IPv4 hidden inside
        ([0x2001, 0x2, 0, 0, 0, 0, 0, 0], 48),   // benchmarking
        ([0x2001, 0xdb8, 0, 0, 0, 0, 0, 0], 32), // documentation
        ([0x3fff, 0, 0, 0, 0, 0, 0, 0], 20),     // documentation
        ([0xfc00, 0, 0, 0, 0, 0, 0, 0], 7),      // unique local
        ([0xfe80, 0, 0, 0, 0, 0, 0, 0], 10),     // link-local
        ([0xfec0, 0, 0, 0, 0, 0, 0, 0], 10),     // the old site-local
        ([0xff00, 0, 0, 0, 0, 0, 0, 0], 8),      // multicast
    ];
    !NOT_PUBLIC
        .iter()
        .any(|(network, prefix)| in_v6(ip, *network, *prefix))
}
