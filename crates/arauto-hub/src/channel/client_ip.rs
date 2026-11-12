//! The address of a connector, read the way `ARAUTO_CLIENT_IP_SOURCE` says
//! (`docs/15-mvp.md`, sections 4 and 6).
//!
//! - `socket`: the address of the socket. Any forwarding header is ignored.
//! - `forwarded`: the `X-Forwarded-For` header, believed only when the socket is one of the
//!   trusted proxies. The list is read from the right: each proxy appends the address it saw,
//!   so the rightmost address that is not a trusted proxy is the client. What is to its left
//!   was written by the client itself and is never believed.
//!
//! A trusted proxy that does not say who the client is gets 400: taking its own address would
//! put every connector behind it under one origin, and one of them could fill the ceiling of
//! all the others.

use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

use crate::config::{ClientIpSource, Network};

/// The header a proxy writes.
pub(crate) const FORWARDED_FOR: &str = "x-forwarded-for";

/// Most addresses the hub reads from the header; a longer chain is refused.
const CHAIN_MAX: usize = 32;

/// Why the address of a connector could not be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Untold;

/// The address of the connector behind a request.
pub(crate) fn client_ip(
    source: &ClientIpSource,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> Result<IpAddr, Untold> {
    let peer = canonical(peer.ip());
    let trusted_proxies = match source {
        ClientIpSource::Socket => return Ok(peer),
        ClientIpSource::Forwarded { trusted_proxies } => trusted_proxies,
    };
    if !trusts(trusted_proxies, peer) {
        // A peer that is not on the list and brings the header is how a wrong list looks:
        // the header is ignored, and every connector seems to come from this one address.
        if headers.contains_key(FORWARDED_FOR) {
            warn_once(peer);
        }
        return Ok(peer);
    }
    let mut chain = Vec::new();
    for value in headers.get_all(FORWARDED_FOR) {
        let text = value.to_str().map_err(|_| Untold)?;
        for item in text.split(',') {
            if chain.len() >= CHAIN_MAX {
                return Err(Untold);
            }
            let address: IpAddr = item.trim().parse().map_err(|_| Untold)?;
            chain.push(canonical(address));
        }
    }
    chain
        .into_iter()
        .rev()
        .find(|address| !trusts(trusted_proxies, *address))
        .ok_or(Untold)
}

/// Says, once for each address and for a few addresses only, that a peer outside the list
/// of trusted proxies sent the header: the list is probably wrong.
fn warn_once(peer: IpAddr) {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock, PoisonError};
    const TOLD_MAX: usize = 8;
    static TOLD: OnceLock<Mutex<HashSet<IpAddr>>> = OnceLock::new();
    // A set of addresses: a panic elsewhere leaves it usable.
    let mut told = TOLD
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if told.len() < TOLD_MAX && told.insert(peer) {
        tracing::warn!(
            %peer,
            "a connection that is not from a trusted proxy brought X-Forwarded-For, which was \
             ignored: if this is your proxy, put this address in ARAUTO_TRUSTED_PROXIES"
        );
    }
}

fn trusts(proxies: &[Network], address: IpAddr) -> bool {
    proxies.iter().any(|network| network.contains(address))
}

/// An IPv4 address written as IPv6 is the IPv4 address.
fn canonical(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 => v4,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use axum::http::HeaderValue;

    fn forwarded(proxies: &[&str]) -> ClientIpSource {
        ClientIpSource::Forwarded {
            trusted_proxies: proxies.iter().map(|p| Network::parse(p).unwrap()).collect(),
        }
    }

    fn headers(values: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(FORWARDED_FOR, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    fn peer(text: &str) -> SocketAddr {
        SocketAddr::new(text.parse().unwrap(), 40_000)
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn with_socket_the_header_is_never_read() {
        let got = client_ip(
            &ClientIpSource::Socket,
            peer("198.51.100.7"),
            &headers(&["203.0.113.9"]),
        );
        assert_eq!(got, Ok(ip("198.51.100.7")));
    }

    #[test]
    fn only_a_trusted_proxy_is_believed() {
        let source = forwarded(&["192.0.2.10"]);
        // The socket is not the proxy: whatever it writes is ignored.
        let got = client_ip(&source, peer("198.51.100.7"), &headers(&["203.0.113.9"]));
        assert_eq!(got, Ok(ip("198.51.100.7")));
        let got = client_ip(&source, peer("192.0.2.10"), &headers(&["203.0.113.9"]));
        assert_eq!(got, Ok(ip("203.0.113.9")));
    }

    #[test]
    fn the_rightmost_untrusted_address_is_the_client() {
        let source = forwarded(&["192.0.2.0/24"]);
        // The client wrote 203.0.113.66 itself; the proxies appended the rest.
        let got = client_ip(
            &source,
            peer("192.0.2.10"),
            &headers(&["203.0.113.66, 198.51.100.7", "192.0.2.11"]),
        );
        assert_eq!(got, Ok(ip("198.51.100.7")));
    }

    #[test]
    fn a_trusted_proxy_that_does_not_say_who_the_client_is_is_refused() {
        let source = forwarded(&["192.0.2.0/24"]);
        for values in [
            &[][..],
            &["not an address"][..],
            &["192.0.2.11"][..],
            &["203.0.113.9:443"][..],
            &["203.0.113.9,,192.0.2.11"][..],
        ] {
            let got = client_ip(&source, peer("192.0.2.10"), &headers(values));
            assert_eq!(got, Err(Untold), "{values:?}");
        }
        let long = vec!["203.0.113.9"; CHAIN_MAX + 1].join(",");
        let got = client_ip(&source, peer("192.0.2.10"), &headers(&[&long]));
        assert_eq!(got, Err(Untold));
    }

    #[test]
    fn an_ipv4_written_as_ipv6_is_the_ipv4() {
        let source = forwarded(&["192.0.2.10"]);
        let got = client_ip(
            &source,
            peer("::ffff:192.0.2.10"),
            &headers(&["::ffff:203.0.113.9"]),
        );
        assert_eq!(got, Ok(ip("203.0.113.9")));
    }
}
