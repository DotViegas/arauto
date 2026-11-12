//! Where the engine is, how the hub proves itself to it, and how the engine proves itself to
//! the hub (`docs/06-modelo-de-dados.md`, section 4).
//!
//! The two secrets never leave the hub but towards the engine: the key of its API goes in a
//! header of each request; the secret of the events goes, once per session of the engine, in
//! its configuration, and signs every event (HMAC-SHA-512 over the body, checked in constant
//! time). In the database both are sealed by the vault with the master key.

use std::fmt;

use arauto_protocol::limits::KEY_LEN;
use arauto_protocol::secret::Secret;
use data_encoding::HEXLOWER;
use hkdf::hmac::{Hmac, KeyInit, Mac};
use sha2::Sha512;
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

/// The path of the inbox of events of one engine, on the internal port.
pub const EVENTS_PATH_PREFIX: &str = "/v1/engine/";

/// The end of the path of the inbox of events.
pub const EVENTS_PATH_SUFFIX: &str = "/events";

/// Why an address is refused. The texts are fixed: what was typed is never repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    /// Not `http://host[:port]`.
    #[error(
        "the address is http://host or http://host:port, with no path, query or user; the \
         engine sits on the private network"
    )]
    Form,
    /// The name of the machine has an underscore, which the engine refuses in the address of
    /// its events.
    #[error("the name of the machine cannot have an underscore: the engine refuses it")]
    Underscore,
}

/// Where something is reached over HTTP: the host and the port.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpAddress {
    secure: bool,
    host: String,
    port: u16,
}

impl fmt::Debug for HttpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The address of the private network is not a secret, but it is not logged either.
        f.write_str("HttpAddress([private])")
    }
}

impl HttpAddress {
    fn parse(text: &str, allow_secure: bool) -> Result<Self, AddressError> {
        if text.len() > 255 {
            return Err(AddressError::Form);
        }
        let (secure, rest) = if let Some(rest) = text.strip_prefix("http://") {
            (false, rest)
        } else if let Some(rest) = text.strip_prefix("https://").filter(|_| allow_secure) {
            (true, rest)
        } else {
            return Err(AddressError::Form);
        };
        let authority = rest.strip_suffix('/').unwrap_or(rest);
        if authority.is_empty() || authority.contains(['/', '?', '#', '@', '\\', '%']) {
            return Err(AddressError::Form);
        }
        let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
            let (address, port) = match inner.split_once("]:") {
                Some((address, port)) => (address, Some(port)),
                None => (inner.strip_suffix(']').ok_or(AddressError::Form)?, None),
            };
            address
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| AddressError::Form)?;
            (format!("[{address}]"), port)
        } else {
            match authority.rsplit_once(':') {
                Some((host, port)) => (host.to_owned(), Some(port)),
                None => (authority.to_owned(), None),
            }
        };
        let port = match port {
            Some(port) => match port.parse::<u16>() {
                Ok(number) if number > 0 && number.to_string() == port => number,
                _ => return Err(AddressError::Form),
            },
            None if secure => 443,
            None => 80,
        };
        if !host.starts_with('[') {
            if host.contains('_') {
                return Err(AddressError::Underscore);
            }
            let label_ok = |label: &str| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            };
            if host.is_empty() || !host.split('.').all(label_ok) {
                return Err(AddressError::Form);
            }
        }
        Ok(Self {
            secure,
            host: host.to_ascii_lowercase(),
            port,
        })
    }

    /// The host, as it goes in a connection (an IPv6 address without its brackets).
    #[must_use]
    pub fn host(&self) -> &str {
        self.host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(&self.host)
    }

    /// The port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// `host:port`, as it goes in the `Host` header.
    #[must_use]
    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// The address as it is stored and shown to the admin.
    #[must_use]
    pub fn to_text(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        format!("{scheme}://{}", self.authority())
    }
}

/// Where the hub reaches the engine: `http://host[:port]`, on the private network. There is no
/// TLS towards the engine in the MVP: it is never published (`docs/15-mvp.md`, section 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineAddress(HttpAddress);

impl EngineAddress {
    /// Checks the address.
    ///
    /// # Errors
    /// [`AddressError`] says which rule the text breaks.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        HttpAddress::parse(text, false).map(Self)
    }

    /// The address.
    #[must_use]
    pub fn http(&self) -> &HttpAddress {
        &self.0
    }
}

/// Where the engine reaches the internal port of the hub: `http://host[:port]` (or
/// `https://`), with no underscore in the name of the machine (`referencia/waha-fatos.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventsUrl(HttpAddress);

impl EventsUrl {
    /// Checks the address.
    ///
    /// # Errors
    /// [`AddressError`] says which rule the text breaks.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        HttpAddress::parse(text, true).map(Self)
    }

    /// The address, as it is stored.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.0.to_text()
    }

    /// The address of the inbox of the engine `engine`, as it goes in the configuration of
    /// each session of the engine.
    #[must_use]
    pub fn inbox(&self, engine: Uuid) -> String {
        format!(
            "{}{EVENTS_PATH_PREFIX}{}{EVENTS_PATH_SUFFIX}",
            self.0.to_text(),
            engine.as_hyphenated()
        )
    }
}

/// The key of the API of the engine: from 8 to 256 visible ASCII characters. It prints
/// `[redacted]` and is wiped when dropped.
#[derive(Clone)]
pub struct ApiKey(Zeroizing<String>);

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey([redacted])")
    }
}

/// The key of the engine is not 8 to 256 visible ASCII characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the key of the engine has from 8 to 256 visible ASCII characters, with no space")]
pub struct ApiKeyError;

impl ApiKey {
    /// Checks the key. A line end around it is dropped.
    ///
    /// # Errors
    /// [`ApiKeyError`] for anything else; the text is not repeated.
    pub fn parse(text: &str) -> Result<Self, ApiKeyError> {
        let text = text.trim_end_matches(['\r', '\n']);
        if !(8..=256).contains(&text.len()) || !text.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(ApiKeyError);
        }
        Ok(Self(Zeroizing::new(text.to_owned())))
    }

    /// The key. Whoever calls this answers for where it goes: only the header to the engine.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// The secret that signs the events of the engine: 32 bytes from the generator of the system,
/// given to the engine as 64 hexadecimal characters.
pub struct EventsSecret(Secret<KEY_LEN>);

impl fmt::Debug for EventsSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EventsSecret([redacted])")
    }
}

/// The length of the signature of an event: SHA-512, in hexadecimal.
const SIGNATURE_HEX_LEN: usize = 128;

impl EventsSecret {
    /// Draws a new secret.
    ///
    /// # Errors
    /// When the generator of the system fails.
    pub fn draw() -> Result<Self, arauto_protocol::secret::RandomError> {
        Secret::random().map(Self)
    }

    /// The secret as it was stored, back.
    #[must_use]
    pub fn from_stored(bytes: &[u8]) -> Option<Self> {
        Secret::from_slice(bytes).map(Self)
    }

    /// The bytes, to be sealed.
    #[must_use]
    pub fn expose(&self) -> &[u8; KEY_LEN] {
        self.0.expose()
    }

    /// The key as the engine takes it: 64 hexadecimal characters.
    #[must_use]
    pub fn hmac_key(&self) -> Zeroizing<String> {
        Zeroizing::new(HEXLOWER.encode(self.0.expose()))
    }

    /// Whether `key`, as the engine shows it, is this secret. Compared in constant time.
    #[must_use]
    pub fn is_hmac_key(&self, key: &str) -> bool {
        bool::from(self.hmac_key().as_bytes().ct_eq(key.as_bytes()))
    }

    fn mac(&self) -> Hmac<Sha512> {
        // The key of the engine is the hexadecimal text (`docs/...waha-fatos`: the key is what
        // was configured), and HMAC takes a key of any length: this never fails.
        #[allow(clippy::expect_used)]
        <Hmac<Sha512> as KeyInit>::new_from_slice(self.hmac_key().as_bytes())
            .expect("HMAC takes a key of any length")
    }

    /// The signature of `body`, in hexadecimal: what the engine sends in `X-Webhook-Hmac`.
    #[must_use]
    pub fn sign(&self, body: &[u8]) -> String {
        let mut mac = self.mac();
        mac.update(body);
        HEXLOWER.encode(&mac.finalize().into_bytes())
    }

    /// Whether `signature` (hexadecimal, either case) is the one of `body`. The comparison of
    /// the codes is in constant time.
    #[must_use]
    pub fn verifies(&self, body: &[u8], signature: &str) -> bool {
        if signature.len() != SIGNATURE_HEX_LEN {
            return false;
        }
        let Ok(tag) = HEXLOWER.decode(signature.to_ascii_lowercase().as_bytes()) else {
            return false;
        };
        let mut mac = self.mac();
        mac.update(body);
        mac.verify_slice(&tag).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_is_reached_over_plain_http_on_the_private_network() {
        let address = EngineAddress::parse("http://waha:3000").unwrap();
        assert_eq!(address.http().to_text(), "http://waha:3000");
        assert_eq!(address.http().authority(), "waha:3000");
        assert_eq!(
            EngineAddress::parse("http://192.0.2.7")
                .unwrap()
                .http()
                .port(),
            80
        );
        let v6 = EngineAddress::parse("http://[2001:db8::7]:3000").unwrap();
        assert_eq!(v6.http().host(), "2001:db8::7");
        for wrong in [
            "",
            "waha:3000",
            "https://waha:3000",
            "ftp://waha",
            "http://",
            "http://waha:3000/api",
            "http://user:pass@waha:3000",
            "http://waha:0",
            "http://waha:99999",
            "http://waha:03000",
            "http://wa%68a",
            "http://wa ha",
            "http://waha?x",
            "http://-waha",
            "http://[not-v6]",
        ] {
            assert!(EngineAddress::parse(wrong).is_err(), "{wrong:?}");
        }
    }

    #[test]
    fn the_address_of_the_events_has_no_underscore() {
        assert_eq!(
            EventsUrl::parse("http://my_hub:9000"),
            Err(AddressError::Underscore)
        );
        let url = EventsUrl::parse("http://arauto-hub:9000").unwrap();
        let engine = Uuid::from_u128(1);
        assert_eq!(
            url.inbox(engine),
            "http://arauto-hub:9000/v1/engine/00000000-0000-0000-0000-000000000001/events"
        );
        assert!(EventsUrl::parse("https://hub.example.com").is_ok());
    }

    #[test]
    fn the_key_of_the_engine_is_checked_and_never_printed() {
        let key = ApiKey::parse("s3cr3t-engine-key\n").unwrap();
        assert_eq!(key.expose(), "s3cr3t-engine-key");
        assert_eq!(format!("{key:?}"), "ApiKey([redacted])");
        for wrong in [
            "",
            "short",
            "with space inside",
            "tab\there!",
            &"k".repeat(257),
        ] {
            assert!(ApiKey::parse(wrong).is_err(), "{wrong:?}");
        }
        assert!(!ApiKeyError.to_string().contains("s3cr3t"));
    }

    #[test]
    fn an_event_verifies_only_with_its_own_signature() {
        let secret = EventsSecret::draw().unwrap();
        let body = br#"{"event":"message.any"}"#;
        let signature = secret.sign(body);
        assert_eq!(signature.len(), 128);
        assert!(secret.verifies(body, &signature));
        assert!(secret.verifies(body, &signature.to_ascii_uppercase()));
        assert!(!secret.verifies(b"{\"event\":\"message\"}", &signature));
        let other = EventsSecret::draw().unwrap();
        assert!(!other.verifies(body, &signature));
        let mut changed = signature.clone().into_bytes();
        changed[0] = if changed[0] == b'0' { b'1' } else { b'0' };
        assert!(!secret.verifies(body, &String::from_utf8(changed).unwrap()));
        assert!(!secret.verifies(body, ""));
        assert!(!secret.verifies(body, &signature[..64]));
        assert!(!secret.verifies(body, &"z".repeat(128)));
        assert!(secret.is_hmac_key(&secret.hmac_key()));
        assert!(!secret.is_hmac_key(&other.hmac_key()));
        assert_eq!(format!("{secret:?}"), "EventsSecret([redacted])");
    }
}
