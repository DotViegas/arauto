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
    // From the right: the last item is what the nearest proxy saw. The walk stops at the
    // first address that is not a trusted proxy; what is to its left was written by the client
    // itself, so it is never read, and an item there that is not an address changes nothing.
    let mut read = 0usize;
    for value in headers.get_all(FORWARDED_FOR).iter().rev() {
        let text = value.to_str().map_err(|_| Untold)?;
        for item in text.rsplit(',') {
            if read >= CHAIN_MAX {
                return Err(Untold);
            }
            read = read.saturating_add(1);
            let address = canonical(item.trim().parse::<IpAddr>().map_err(|_| Untold)?);
            if !trusts(trusted_proxies, address) {
                return Ok(address);
            }
        }
    }
    Err(Untold)
}

/// Whether a request came from a connection that is not a trusted proxy and still brought the
/// forwarding header: how a wrong `ARAUTO_TRUSTED_PROXIES` looks. Told from the request itself,
/// so the check window of the panel says it whatever the log already said. Only reads.
#[must_use]
pub(crate) fn untrusted_forwarder(
    source: &ClientIpSource,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> bool {
    match source {
        ClientIpSource::Socket => false,
        ClientIpSource::Forwarded { trusted_proxies } => {
            headers.contains_key(FORWARDED_FOR) && !trusts(trusted_proxies, canonical(peer.ip()))
        }
    }
}

/// Whether the hub is set to believe a proxy (`forwarded`) and the connection is not from one:
/// no trusted proxy speaks for this request, so the hub sees the peer itself. `false` with
/// `socket` (nothing is believed there, and `ARAUTO_TRUSTED_PROXIES` is refused) and `false`
/// for a trusted proxy that did not say who the client is (its address is already in the
/// list). Only reads.
#[must_use]
pub(crate) fn no_trusted_proxy(source: &ClientIpSource, peer: SocketAddr) -> bool {
    match source {
        ClientIpSource::Socket => false,
        ClientIpSource::Forwarded { trusted_proxies } => {
            !trusts(trusted_proxies, canonical(peer.ip()))
        }
    }
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
pub(crate) fn canonical(address: IpAddr) -> IpAddr {
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
        // More proxies than the hub reads, with no client among them.
        let long = vec!["192.0.2.11"; CHAIN_MAX + 1].join(",");
        let got = client_ip(
            &source,
            peer("192.0.2.10"),
            &headers(&["203.0.113.9", &long]),
        );
        assert_eq!(got, Err(Untold));
    }

    #[test]
    fn what_the_client_wrote_left_of_itself_changes_nothing() {
        let source = forwarded(&["192.0.2.0/24"]);
        // The client wrote garbage, or a chain longer than the hub reads, before the address
        // the proxy appended: the walk from the right stops at the client and never reads it.
        let long = vec!["203.0.113.66"; CHAIN_MAX * 2].join(",");
        for values in [
            &["x, 198.51.100.7"][..],
            &["not an address", "198.51.100.7, 192.0.2.11"][..],
            &[long.as_str(), "198.51.100.7"][..],
        ] {
            let got = client_ip(&source, peer("192.0.2.10"), &headers(values));
            assert_eq!(got, Ok(ip("198.51.100.7")), "{values:?}");
        }
    }

    #[test]
    fn a_forwarder_outside_the_list_is_told_from_the_request() {
        let source = forwarded(&["192.0.2.10"]);
        let with = headers(&["203.0.113.9"]);
        assert!(untrusted_forwarder(&source, peer("198.51.100.7"), &with));
        assert!(!untrusted_forwarder(&source, peer("192.0.2.10"), &with));
        assert!(!untrusted_forwarder(
            &source,
            peer("198.51.100.7"),
            &headers(&[])
        ));
        assert!(!untrusted_forwarder(
            &ClientIpSource::Socket,
            peer("198.51.100.7"),
            &with
        ));
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

    /// The window tells an admin to add a proxy to the list only when the hub believes proxies
    /// and the connection is not from one: never with `socket` (the list is refused there), and
    /// never for a proxy that is already in the list and said nothing about the client.
    #[test]
    fn no_trusted_proxy_is_told_only_for_a_peer_outside_the_list_of_a_forwarded_hub() {
        let source = forwarded(&["10.1.0.0/16"]);
        assert!(!no_trusted_proxy(&source, peer("10.1.0.5")));
        assert!(no_trusted_proxy(&source, peer("198.51.100.7")));
        assert!(no_trusted_proxy(&source, peer("10.2.0.5")));
        assert!(!no_trusted_proxy(
            &ClientIpSource::Socket,
            peer("198.51.100.7")
        ));
        assert!(!no_trusted_proxy(
            &ClientIpSource::Socket,
            peer("172.18.0.2")
        ));
    }
}
