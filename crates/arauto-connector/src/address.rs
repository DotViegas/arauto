//! The address of the hub, as the block gives it: the websocket of the channel
//! (`docs/14-protocolo-do-canal.md`, section 1).
//!
//! ```text
//! wss://hub.example.com/v1/channel       any host, with TLS (the `tls` feature)
//! ws://127.0.0.1:8443/v1/channel          only this machine: 127.0.0.1, [::1], localhost
//! ```
//!
//! The text is read strictly: a scheme, a host and an optional port, and the one path of the
//! channel. No user, no query, no fragment. An address without TLS to another machine is
//! refused here, before anything else happens.

use core::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};

/// The path of the websocket of the channel.
pub const CHANNEL_PATH: &str = "/v1/channel";

/// The longest address accepted. A host name has at most 253 characters.
const ADDRESS_MAX_LEN: usize = 300;

/// Why a text is not the address of a hub. No variant repeats the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    /// Not `ws://` or `wss://`, with a host, an optional port and the path of the channel.
    #[error("the address of the hub is a websocket address with the path of the channel")]
    Form,
    /// `ws://` to an address that is not this machine.
    #[error("an address without TLS is only accepted for this machine")]
    PlainNotLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Host {
    Name(String),
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

impl Host {
    fn parse(text: &str) -> Option<Self> {
        if let Some(inside) = text.strip_prefix('[') {
            let inside = inside.strip_suffix(']')?;
            return inside.parse().ok().map(Host::V6);
        }
        if let Ok(v4) = text.parse::<Ipv4Addr>() {
            return Some(Host::V4(v4));
        }
        is_host_name(text).then(|| Host::Name(text.to_ascii_lowercase()))
    }

    /// Whether this host is this machine.
    fn is_local(&self) -> bool {
        match self {
            Host::Name(name) => name == "localhost",
            Host::V4(v4) => *v4 == Ipv4Addr::LOCALHOST,
            Host::V6(v6) => *v6 == Ipv6Addr::LOCALHOST,
        }
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::Name(name) => f.write_str(name),
            Host::V4(v4) => write!(f, "{v4}"),
            Host::V6(v6) => write!(f, "[{v6}]"),
        }
    }
}

/// A host name: labels of letters, digits and hyphens, separated by dots, none of them empty
/// nor starting or ending with a hyphen.
fn is_host_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 253
        && text.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

/// The address of a hub: where the websocket of its channel is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HubAddress {
    secure: bool,
    host: Host,
    port: Option<u16>,
}

impl HubAddress {
    /// Reads the address of the block.
    ///
    /// # Errors
    /// [`AddressError::Form`] for a text that is not such an address;
    /// [`AddressError::PlainNotLocal`] for `ws://` to another machine.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        if text.len() > ADDRESS_MAX_LEN || !text.is_ascii() {
            return Err(AddressError::Form);
        }
        let (secure, rest) = if let Some(rest) = text.strip_prefix("wss://") {
            (true, rest)
        } else if let Some(rest) = text.strip_prefix("ws://") {
            (false, rest)
        } else {
            return Err(AddressError::Form);
        };
        let authority = rest.strip_suffix(CHANNEL_PATH).ok_or(AddressError::Form)?;
        if authority.is_empty() || authority.contains(['/', '?', '#', '@', '\\', ' ']) {
            return Err(AddressError::Form);
        }
        let (host, port) = split_port(authority)?;
        let host = Host::parse(host).ok_or(AddressError::Form)?;
        if !secure && !host.is_local() {
            return Err(AddressError::PlainNotLocal);
        }
        Ok(Self { secure, host, port })
    }

    /// Whether the channel goes over TLS.
    #[must_use]
    pub const fn is_secure(&self) -> bool {
        self.secure
    }

    /// The host, as a name or an address without brackets: what is resolved and what the
    /// certificate is checked against.
    #[must_use]
    pub fn host(&self) -> String {
        match &self.host {
            Host::Name(name) => name.clone(),
            Host::V4(v4) => v4.to_string(),
            Host::V6(v6) => v6.to_string(),
        }
    }

    /// The port: the one written, or 443 with TLS and 80 without.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(if self.secure { 443 } else { 80 })
    }

    /// The address of the websocket, for the request.
    #[must_use]
    pub fn url(&self) -> String {
        self.to_string()
    }
}

/// Splits `host[:port]`, where an IPv6 host is in brackets.
fn split_port(authority: &str) -> Result<(&str, Option<u16>), AddressError> {
    let (host, port) = if authority.starts_with('[') {
        match authority.find(']') {
            Some(end) => {
                let (host, rest) = authority.split_at(end.saturating_add(1));
                match rest.strip_prefix(':') {
                    Some(port) => (host, Some(port)),
                    None if rest.is_empty() => (host, None),
                    None => return Err(AddressError::Form),
                }
            }
            None => return Err(AddressError::Form),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port {
        None => None,
        Some(port) => {
            // Digits only: no sign, no space, no leading plus.
            if port.is_empty() || port.len() > 5 || !port.bytes().all(|b| b.is_ascii_digit()) {
                return Err(AddressError::Form);
            }
            match port.parse::<u16>() {
                Ok(0) | Err(_) => return Err(AddressError::Form),
                Ok(port) => Some(port),
            }
        }
    };
    Ok((host, port))
}

impl fmt::Display for HubAddress {
    /// The address in its one written form: lowercase scheme and host, and the port only when
    /// it was written.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scheme = if self.secure { "wss" } else { "ws" };
        match self.port {
            Some(port) => write!(f, "{scheme}://{}:{port}{CHANNEL_PATH}", self.host),
            None => write!(f, "{scheme}://{}{CHANNEL_PATH}", self.host),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_addresses_of_the_block_are_read() {
        let address = HubAddress::parse("wss://hub.example.com/v1/channel").unwrap();
        assert!(address.is_secure());
        assert_eq!(address.host(), "hub.example.com");
        assert_eq!(address.port(), 443);
        assert_eq!(address.url(), "wss://hub.example.com/v1/channel");

        let address = HubAddress::parse("wss://Hub.Example.COM:8443/v1/channel").unwrap();
        assert_eq!(address.port(), 8443);
        assert_eq!(address.url(), "wss://hub.example.com:8443/v1/channel");

        let address = HubAddress::parse("ws://127.0.0.1:41000/v1/channel").unwrap();
        assert!(!address.is_secure());
        assert_eq!(
            (address.host().as_str(), address.port()),
            ("127.0.0.1", 41000)
        );

        assert!(HubAddress::parse("ws://localhost/v1/channel").is_ok());
        assert!(HubAddress::parse("ws://LOCALHOST:1/v1/channel").is_ok());
        let v6 = HubAddress::parse("ws://[::1]:9/v1/channel").unwrap();
        assert_eq!((v6.host().as_str(), v6.port()), ("::1", 9));
        assert_eq!(v6.url(), "ws://[::1]:9/v1/channel");
        assert!(HubAddress::parse("wss://192.0.2.10/v1/channel").is_ok());
        assert!(HubAddress::parse("wss://[2001:db8::1]:443/v1/channel").is_ok());
    }

    /// `docs/14-protocolo-do-canal.md`, section 1: without TLS, only this machine.
    #[test]
    fn plain_websocket_is_refused_for_another_machine() {
        for text in [
            "ws://192.0.2.1/v1/channel",
            "ws://hub.example.com/v1/channel",
            "ws://127.0.0.2/v1/channel",
            "ws://[2001:db8::1]/v1/channel",
            "ws://localhost.example.com/v1/channel",
            "ws://0.0.0.0:80/v1/channel",
        ] {
            assert_eq!(
                HubAddress::parse(text),
                Err(AddressError::PlainNotLocal),
                "{text}"
            );
        }
    }

    #[test]
    fn anything_else_is_not_an_address() {
        for text in [
            "",
            "hub.example.com",
            "https://hub.example.com/v1/channel",
            "wss://hub.example.com",
            "wss://hub.example.com/",
            "wss://hub.example.com/v1/channel/",
            "wss://hub.example.com/v1/channel?x=1",
            "wss://hub.example.com/v1/channel#x",
            "wss://user@hub.example.com/v1/channel",
            "wss://hub.example.com:0/v1/channel",
            "wss://hub.example.com:65536/v1/channel",
            "wss://hub.example.com:+80/v1/channel",
            "wss://hub.example.com:/v1/channel",
            "wss://hub..example.com/v1/channel",
            "wss://-hub.example.com/v1/channel",
            "wss://hub_example.com/v1/channel",
            "wss://hub example.com/v1/channel",
            "wss://[::1/v1/channel",
            "wss://[::1]x/v1/channel",
            "wss:///v1/channel",
            "wss://h\u{e9}b.example.com/v1/channel",
            "WSS://hub.example.com/v1/channel",
        ] {
            assert_eq!(HubAddress::parse(text), Err(AddressError::Form), "{text}");
        }
        let long = format!("wss://{}.example.com/v1/channel", "a".repeat(300));
        assert_eq!(HubAddress::parse(&long), Err(AddressError::Form));
    }

    #[test]
    fn one_written_form_for_each_address() {
        let a = HubAddress::parse("wss://HUB.example.com/v1/channel").unwrap();
        let b = HubAddress::parse("wss://hub.example.com/v1/channel").unwrap();
        assert_eq!(a, b);
        assert_eq!(HubAddress::parse(&a.url()).unwrap(), a);
        // The written port is part of the address: another text, another address.
        let c = HubAddress::parse("wss://hub.example.com:443/v1/channel").unwrap();
        assert_ne!(a, c);
    }
}
