//! The rules of the download by address (decision 0055), each with a test that fails when the
//! rule is taken away: the ranges that are not public, `http://`, a name that resolves to a
//! private IP (refused before any connection), a redirect to a private IP, the 10 MB cut, the
//! time of the whole download, and one download that goes through, with the name in TLS and in
//! `Host`, by the allowance that only a test build has.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use super::*;

const CA: &[u8] = include_bytes!("testdata/test-ca.der");
const SERVER: &[u8] = include_bytes!("testdata/test-server.der");
const SERVER_KEY: &[u8] = include_bytes!("testdata/test-server-key.der");

/// The name the certificate of the test server is for.
const NAME: &str = "files.example.org";

// ---------------------------------------------------------------------------- a resolver

/// A resolver of the test: each name to the IPs it is given, and a count of the questions.
#[derive(Debug, Default)]
struct Names {
    known: Mutex<HashMap<String, Vec<IpAddr>>>,
    asked: AtomicUsize,
}

impl Names {
    fn with(entries: Vec<(&str, Vec<IpAddr>)>) -> Arc<Self> {
        let names = Self::default();
        for (name, ips) in entries {
            names.known.lock().unwrap().insert(name.to_owned(), ips);
        }
        Arc::new(names)
    }

    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl Resolve for Names {
    fn resolve<'a>(&'a self, host: &'a str, _port: u16) -> BoxFuture<'a, Result<Vec<IpAddr>, ()>> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        let found = self.known.lock().unwrap().get(host).cloned();
        Box::pin(async move { found.ok_or(()) })
    }
}

fn v4(text: &str) -> IpAddr {
    IpAddr::V4(text.parse::<Ipv4Addr>().unwrap())
}

fn v6(text: &str) -> IpAddr {
    IpAddr::V6(text.parse::<Ipv6Addr>().unwrap())
}

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

// ---------------------------------------------------------------------------- a server

/// An HTTPS server of the test on this machine, with the certificate of [`NAME`], and a count of
/// the connections it accepted.
struct Server {
    port: u16,
    accepted: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn start() -> Self {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![rustls::pki_types::CertificateDer::from(SERVER.to_vec())],
                rustls::pki_types::PrivateKeyDer::Pkcs8(
                    rustls::pki_types::PrivatePkcs8KeyDer::from(SERVER_KEY.to_vec()),
                ),
            )
            .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&accepted);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                counted.fetch_add(1, Ordering::SeqCst);
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    if let Ok(stream) = acceptor.accept(stream).await {
                        let _ = answer(stream, port).await;
                    }
                });
            }
        });
        Self {
            port,
            accepted,
            task,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("https://{NAME}:{}{path}", self.port)
    }

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

/// The body every download that goes through gets.
const FILE: &[u8] = b"%PDF-1.4 a file of the test";

async fn answer<S: AsyncReadExt + AsyncWriteExt + Unpin>(
    mut stream: S,
    port: u16,
) -> std::io::Result<()> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await? == 0 || head.len() > 16 * 1024 {
            return Ok(());
        }
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let path = head.split(' ').nth(1).unwrap_or("/").to_owned();
    let host = head
        .lines()
        .find_map(|line| line.strip_prefix("host: ").or(line.strip_prefix("Host: ")))
        .unwrap_or_default()
        .to_owned();
    let redirect = |location: String| {
        format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n")
    };
    // The file goes only to a request whose `Host` is the name and the port of the address: a
    // request that carries the IP it connected to, no port or no `Host` at all gets 421.
    let wanted = format!("{NAME}:{port}");
    match path.as_str() {
        "/file.pdf" if host != wanted => {
            stream
                .write_all(b"HTTP/1.1 421 Misdirected Request\r\nContent-Length: 0\r\n\r\n")
                .await?;
        }
        "/file.pdf" => {
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
                        FILE.len()
                    )
                    .as_bytes(),
                )
                .await?;
            stream.write_all(FILE).await?;
        }
        "/untyped" => {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc")
                .await?;
        }
        "/declared-too-large" => {
            let above = WHATSAPP_FILE_BY_ADDRESS_MAX_BYTES + 1;
            stream
                .write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {above}\r\n\r\n").as_bytes())
                .await?;
        }
        "/endless" => {
            // No size declared: chunks until the client stops reading, up to 64 MB.
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await?;
            let chunk = vec![b'x'; 64 * 1024];
            let mut chunked = format!("{:x}\r\n", chunk.len()).into_bytes();
            chunked.extend_from_slice(&chunk);
            chunked.extend_from_slice(b"\r\n");
            for _ in 0..1_024 {
                stream.write_all(&chunked).await?;
            }
            stream.write_all(b"0\r\n\r\n").await?;
        }
        "/slow" => {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n")
                .await?;
            tokio::time::sleep(Duration::from_secs(10)).await;
            stream.write_all(b"abc").await?;
        }
        "/missing" => {
            stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .await?;
        }
        "/to/file" => {
            stream
                .write_all(redirect("/file.pdf".to_owned()).as_bytes())
                .await?;
        }
        "/to/inside" => {
            stream
                .write_all(
                    redirect(format!("https://inside.example.org:{port}/file.pdf")).as_bytes(),
                )
                .await?;
        }
        "/to/private-ip" => {
            stream
                .write_all(redirect(format!("https://10.0.0.7:{port}/file.pdf")).as_bytes())
                .await?;
        }
        "/to/http" => {
            stream
                .write_all(redirect(format!("http://{NAME}:{port}/file.pdf")).as_bytes())
                .await?;
        }
        "/to/self" => {
            stream
                .write_all(redirect("/to/self".to_owned()).as_bytes())
                .await?;
        }
        _ => {
            stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
                .await?;
        }
    }
    stream.flush().await?;
    stream.shutdown().await
}

/// A client of the test: [`NAME`] resolves to this machine, the root of the test is trusted,
/// and the loopback IPs are let through.
fn allowed_client(limits: Limits) -> (Fetcher, Arc<Names>) {
    let names = Names::with(vec![
        (NAME, vec![LOOPBACK]),
        ("inside.example.org", vec![v4("10.1.2.3")]),
    ]);
    let fetcher = Fetcher::for_tests(names.clone(), CA, true, limits);
    (fetcher, names)
}

// ---------------------------------------------------------------------------- the ranges

#[test]
fn every_range_that_is_not_public_is_refused() {
    let not_public = [
        // IPv4.
        "0.0.0.0",
        "0.1.2.3",
        "10.0.0.1",
        "10.255.255.255",
        "100.64.0.1",
        "100.127.255.254",
        "127.0.0.1",
        "127.255.255.254",
        "169.254.169.254",
        "172.16.0.1",
        "172.31.255.254",
        "192.0.0.1",
        "192.0.2.10",
        "192.168.0.1",
        "192.168.255.254",
        "198.18.0.1",
        "198.19.255.254",
        "198.51.100.10",
        "203.0.113.10",
        "224.0.0.1",
        "239.255.255.250",
        "240.0.0.1",
        "255.255.255.255",
        // IPv6.
        "::",
        "::1",
        "fe80::1",
        "febf::1",
        "fc00::1",
        "fd12:3456:789a::1",
        "ff02::1",
        "ff0e::1",
        "2001:db8::1",
        "2001:db8:ffff::1",
        "fec0::1",
        "100::1",
        "2001::1",
        "64:ff9b:1::1",
        // An IPv4 of those inside an IPv6: mapped, compatible, NAT64 and 6to4.
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "::ffff:169.254.169.254",
        "::ffff:192.168.1.1",
        "::ffff:0.0.0.0",
        "::ffff:255.255.255.255",
        "::127.0.0.1",
        "::10.0.0.1",
        "::192.168.1.1",
        "64:ff9b::10.0.0.1",
        "64:ff9b::127.0.0.1",
        "2002:7f00:1::1",
        "2002:a00:1::1",
        "2002:c0a8:101::1",
    ];
    for text in not_public {
        let ip = if text.contains(':') {
            v6(text)
        } else {
            v4(text)
        };
        assert!(!is_public(ip), "{text} is not public");
    }
    let public = [
        "1.1.1.1",
        "8.8.8.8",
        "100.63.255.255",
        "100.128.0.1",
        "172.15.255.255",
        "172.32.0.1",
        "192.0.3.1",
        "192.169.0.1",
        "198.17.255.255",
        "198.20.0.1",
        "223.255.255.254",
        "2606:4700:4700::1111",
        "2a00:1450:4001::1",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
        "2002:808:808::1",
    ];
    for text in public {
        let ip = if text.contains(':') {
            v6(text)
        } else {
            v4(text)
        };
        assert!(is_public(ip), "{text} is public");
    }
}

#[tokio::test]
async fn an_ip_written_in_the_address_is_checked_like_a_resolved_one() {
    let names = Names::with(vec![]);
    let fetcher = Fetcher::new(names.clone());
    for url in [
        "https://127.0.0.1/x",
        "https://10.0.0.1:8443/x",
        "https://169.254.169.254/latest/meta-data",
        "https://192.168.0.1/",
        "https://[::1]/x",
        "https://[::ffff:127.0.0.1]/x",
        "https://[::ffff:10.0.0.1]:443/x",
        "https://[fe80::1]/x",
        "https://[fd00::1]/x",
        "https://[2001:db8::1]/x",
    ] {
        assert_eq!(
            fetcher.fetch(url).await.unwrap_err(),
            Refused::NotPublic,
            "{url}"
        );
    }
    assert_eq!(names.asked(), 0, "an IP is never resolved");
}

#[tokio::test]
async fn only_an_https_address_in_the_format_is_taken() {
    let names = Names::with(vec![(NAME, vec![v4("8.8.8.8")])]);
    let fetcher = Fetcher::new(names.clone());
    for url in [
        "http://files.example.org/file.pdf",
        "HTTPS://files.example.org/file.pdf",
        "ftp://files.example.org/file.pdf",
        "file:///etc/passwd",
        "files.example.org/file.pdf",
        "//files.example.org/file.pdf",
    ] {
        assert_eq!(
            fetcher.fetch(url).await.unwrap_err(),
            Refused::NotHttps,
            "{url}"
        );
    }
    for url in [
        "https://",
        "https:///file.pdf",
        "https://user@files.example.org/file.pdf",
        "https://user:pass@files.example.org/file.pdf",
        "https://files.example.org:0/x",
        "https://files.example.org:65536/x",
        "https://files.example.org:/x",
        "https://files.example.org:44a/x",
        "https://files.example.org\\@10.0.0.1/x",
        "https://files_example.org/x",
        "https://-files.example.org/x",
        "https://files..example.org/x",
        "https://[::1/x",
        "https://[::1]x/x",
        "https://files.example.org/a b",
        "https://files.example.org/\u{e9}",
    ] {
        assert_eq!(
            fetcher.fetch(url).await.unwrap_err(),
            Refused::Malformed,
            "{url}"
        );
    }
    assert_eq!(names.asked(), 0, "nothing out of the format is resolved");
}

/// A name that resolves to a private IP is refused before any connection: the server of the
/// test, reachable at the loopback IP the name also resolves to, sees none.
#[tokio::test]
async fn a_name_that_resolves_to_a_private_ip_is_refused_before_any_connection() {
    let server = Server::start().await;
    // Production rules (no allowance): a loopback answer alone is refused.
    let names = Names::with(vec![(NAME, vec![LOOPBACK])]);
    let fetcher = Fetcher::new(names.clone());
    assert_eq!(
        fetcher.fetch(&server.url("/file.pdf")).await.unwrap_err(),
        Refused::NotPublic
    );
    assert_eq!(names.asked(), 1);
    // Even where loopback is let through, one private IP among the answers refuses the name.
    let names = Names::with(vec![(NAME, vec![LOOPBACK, v4("192.168.10.20")])]);
    let fetcher = Fetcher::for_tests(names.clone(), CA, true, Limits::DECIDED);
    assert_eq!(
        fetcher.fetch(&server.url("/file.pdf")).await.unwrap_err(),
        Refused::NotPublic
    );
    // A public IP with a private one, and a private IPv6.
    for ips in [
        vec![v4("8.8.8.8"), v4("10.0.0.1")],
        vec![v6("fd00::5")],
        vec![v6("::ffff:172.16.0.9")],
    ] {
        let names = Names::with(vec![(NAME, ips.clone())]);
        let fetcher = Fetcher::new(names);
        assert_eq!(
            fetcher.fetch(&server.url("/file.pdf")).await.unwrap_err(),
            Refused::NotPublic,
            "{ips:?}"
        );
    }
    // A name that does not resolve.
    let fetcher = Fetcher::new(Names::with(vec![]));
    assert_eq!(
        fetcher.fetch(&server.url("/file.pdf")).await.unwrap_err(),
        Refused::NotResolved
    );
    assert_eq!(server.accepted(), 0, "no connection was made");
}

// ---------------------------------------------------------------------------- a download

/// The positive path, by the allowance of a test build: the name in `Host` and in TLS, the file
/// and its media type; a redirect to a path of the same host is followed. The server of the test
/// gives the file only when `Host` is the name and the port of the address, so a request that
/// carries the IP it connected to fails here.
#[tokio::test]
async fn a_public_file_is_downloaded_with_its_media_type() {
    let server = Server::start().await;
    let (fetcher, names) = allowed_client(Limits::DECIDED);
    let file = fetcher.fetch(&server.url("/file.pdf")).await.unwrap();
    assert_eq!(file.data, FILE);
    assert_eq!(file.mimetype.as_deref(), Some("application/pdf"));
    assert_eq!(
        format!("{file:?}"),
        format!("Fetched({} bytes)", FILE.len())
    );
    let untyped = fetcher.fetch(&server.url("/untyped")).await.unwrap();
    assert_eq!(untyped.data, b"abc");
    assert_eq!(untyped.mimetype, None);
    let redirected = fetcher.fetch(&server.url("/to/file")).await.unwrap();
    assert_eq!(redirected.data, FILE);
    // A server that is not the file.
    assert_eq!(
        fetcher.fetch(&server.url("/missing")).await.unwrap_err(),
        Refused::Status
    );
    assert!(names.asked() >= 4, "the name is resolved at each request");
    // The certificate is checked: without the root of the test, the same server is refused.
    let names = Names::with(vec![(NAME, vec![LOOPBACK])]);
    let mut untrusted = Fetcher::new(names);
    untrusted.allowance = allowance::Allowance { loopback: true };
    assert_eq!(
        untrusted.fetch(&server.url("/file.pdf")).await.unwrap_err(),
        Refused::Tls
    );
    // And the name must be the one of the certificate.
    let names = Names::with(vec![("other.example.org", vec![LOOPBACK])]);
    let fetcher = Fetcher::for_tests(names, CA, true, Limits::DECIDED);
    let other = format!("https://other.example.org:{}/file.pdf", server.port);
    assert_eq!(fetcher.fetch(&other).await.unwrap_err(), Refused::Tls);
}

#[tokio::test]
async fn a_redirect_is_checked_again_and_counted() {
    let server = Server::start().await;
    let (fetcher, _) = allowed_client(Limits::DECIDED);
    // To a name that resolves to a private IP: refused before connecting to it.
    let before = server.accepted();
    assert_eq!(
        fetcher.fetch(&server.url("/to/inside")).await.unwrap_err(),
        Refused::NotPublic
    );
    assert_eq!(
        server.accepted(),
        before + 1,
        "only the first request connected"
    );
    // To a private IP written in the address.
    assert_eq!(
        fetcher
            .fetch(&server.url("/to/private-ip"))
            .await
            .unwrap_err(),
        Refused::NotPublic
    );
    // To `http://`.
    assert_eq!(
        fetcher.fetch(&server.url("/to/http")).await.unwrap_err(),
        Refused::NotHttps
    );
    // At most 3: the fourth is refused, after 4 requests.
    let before = server.accepted();
    assert_eq!(
        fetcher.fetch(&server.url("/to/self")).await.unwrap_err(),
        Refused::TooManyRedirects
    );
    assert_eq!(server.accepted(), before + 4);
    assert_eq!(Limits::DECIDED.redirects, 3);
}

#[test]
fn the_forms_of_a_redirect() {
    let here = Target::parse("https://files.example.org:8443/a/b?c=d#e").unwrap();
    assert_eq!(here.path, "/a/b?c=d");
    assert_eq!(here.authority(), "files.example.org:8443");
    assert_eq!(
        here.join("/x").unwrap(),
        Target::parse("https://files.example.org:8443/x").unwrap()
    );
    assert_eq!(
        here.join("//cdn.example.org/y").unwrap(),
        Target::parse("https://cdn.example.org/y").unwrap()
    );
    assert_eq!(
        here.join("https://cdn.example.org/z").unwrap().authority(),
        "cdn.example.org"
    );
    assert_eq!(
        here.join("http://cdn.example.org/z"),
        Err(Refused::NotHttps)
    );
    assert_eq!(
        here.join("HTTP://cdn.example.org/z"),
        Err(Refused::NotHttps)
    );
    assert_eq!(here.join("ftp://cdn.example.org/z"), Err(Refused::NotHttps));
    assert_eq!(here.join("relative/path"), Err(Refused::Malformed));
    assert_eq!(here.join(""), Err(Refused::Malformed));
    assert_eq!(
        Target::parse("https://Files.Example.org?q").unwrap().path,
        "/?q"
    );
    assert_eq!(
        Target::parse("https://[2606:4700::1]:8443/")
            .unwrap()
            .authority(),
        "[2606:4700::1]:8443"
    );
}

// ---------------------------------------------------------------------------- the limits

/// 10 MB, read in a stream and cut there: a declared size above it is refused before reading,
/// and an answer without a size is cut while it streams.
#[tokio::test]
async fn a_file_above_10_mb_is_cut() {
    assert_eq!(Limits::DECIDED.max_bytes, 10 * 1024 * 1024);
    let server = Server::start().await;
    let (fetcher, _) = allowed_client(Limits::DECIDED);
    assert_eq!(
        fetcher
            .fetch(&server.url("/declared-too-large"))
            .await
            .unwrap_err(),
        Refused::TooLarge
    );
    assert_eq!(
        fetcher.fetch(&server.url("/endless")).await.unwrap_err(),
        Refused::TooLarge
    );
    // A smaller limit cuts what passes the decided one: the cut follows the limit.
    let (small, _) = allowed_client(Limits {
        max_bytes: 10,
        ..Limits::DECIDED
    });
    assert_eq!(
        small.fetch(&server.url("/file.pdf")).await.unwrap_err(),
        Refused::TooLarge
    );
}

/// The whole download has a time: a server that does not finish is given up at it.
#[tokio::test]
async fn the_whole_download_has_a_time() {
    assert_eq!(Limits::DECIDED.deadline, Duration::from_secs(20));
    let server = Server::start().await;
    let (fetcher, _) = allowed_client(Limits {
        deadline: Duration::from_millis(500),
        ..Limits::DECIDED
    });
    let started = tokio::time::Instant::now();
    assert_eq!(
        fetcher.fetch(&server.url("/slow")).await.unwrap_err(),
        Refused::Timeout
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

// ---------------------------------------------------------------------------- the allowance

/// The source of the client, with every item marked `#[cfg(test)]` taken out: what a build that
/// is shipped compiles.
fn shipped_source() -> String {
    let source = include_str!("mod.rs");
    let mut kept = Vec::new();
    let mut lines = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"));
    while let Some(line) = lines.next() {
        if line.trim() != "#[cfg(test)]" {
            kept.push(line);
            continue;
        }
        // The item that follows: one line ending in `;` or `,`, or up to its closing brace.
        let mut depth = 0i32;
        for next in lines.by_ref() {
            depth += i32::try_from(next.matches('{').count()).unwrap();
            depth -= i32::try_from(next.matches('}').count()).unwrap();
            let trimmed = next.trim_end();
            if depth <= 0
                && (trimmed.ends_with(';') || trimmed.ends_with(',') || trimmed.ends_with('}'))
            {
                break;
            }
        }
    }
    kept.join("\n")
}

#[test]
fn the_allowance_exists_only_in_a_test_build() {
    let shipped = shipped_source();
    let lower = shipped.to_ascii_lowercase();
    // Every mention of the allowance, of the roots of the tests and of their constructor is in
    // an item a shipped build does not compile.
    for word in ["allowance", "for_tests", "loopback: true", "testdata"] {
        assert!(!lower.contains(word), "{word} outside #[cfg(test)]");
    }
    // The client of the hub is built only with the public roots and the decided limits.
    assert!(shipped.contains("tls: client_config(public_roots()),"));
    assert!(shipped.contains("limits: Limits::DECIDED,"));
    // The scan sees what it looks for: the source does mention them under `#[cfg(test)]`.
    let whole = include_str!("mod.rs");
    assert!(whole.contains("allowance: allowance::Allowance"));
    assert!(whole.contains("mod allowance;"));
    // And the client of the hub lets nothing through that is not public.
    let fetcher = Fetcher::system();
    assert_eq!(fetcher.allowance, allowance::Allowance::default());
    assert!(!fetcher.allowed(LOOPBACK));
    assert!(!fetcher.allowed(v4("10.0.0.1")));
    assert!(fetcher.allowed(v4("8.8.8.8")));
    assert_eq!(fetcher.limits(), Limits::DECIDED);
}

#[test]
fn a_refusal_says_nothing_of_the_address() {
    for refused in [
        Refused::NotHttps,
        Refused::Malformed,
        Refused::NotPublic,
        Refused::NotResolved,
        Refused::Connect,
        Refused::Tls,
        Refused::Status,
        Refused::TooLarge,
        Refused::Timeout,
        Refused::TooManyRedirects,
    ] {
        let text = refused.to_string();
        assert_eq!(text, refused.as_str());
        assert!(
            text.bytes().all(|b| b.is_ascii_alphabetic() || b == b' '),
            "{text}"
        );
    }
}
