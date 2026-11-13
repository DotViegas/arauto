//! The two secrets of the panel: the entry code and the identifier of a session.
//!
//! Both come from the generator of the system and are kept by the hub only as a SHA-256
//! (`docs/04-seguranca-conexao.md`, section 4: what is only checked is stored as a digest).
//! The entry code is of short duration: it appears once, on the terminal of whoever ran the
//! admin command, and opens one session. The identifier of the session lives in a cookie the
//! page cannot read.
//!
//! Neither type has `Display`, `Clone` or serde, and their `Debug` says `[redacted]`.

use arauto_protocol::secret::{RandomError, Secret};
use data_encoding::{BASE32_NOPAD, BASE64URL_NOPAD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

/// The bytes of an entry code: 160 bits, 32 letters and digits when written.
const ENTRY_LEN: usize = 20;

/// The bytes of the identifier of a session.
const SESSION_LEN: usize = 32;

/// The letters and digits of an entry code, without the hyphens.
const ENTRY_TEXT_LEN: usize = 32;

/// The length of the identifier of a session, as it goes in the cookie.
const SESSION_TEXT_LEN: usize = 43;

/// What is hashed before each secret, so that the digest of one is never the digest of the
/// other.
const ENTRY_DOMAIN: &[u8] = b"arauto panel entry code\0";
const SESSION_DOMAIN: &[u8] = b"arauto panel session\0";

/// The SHA-256 of a secret of the panel: what the database keeps and what a row is found by.
#[derive(Clone, Copy)]
pub struct PanelDigest([u8; 32]);

impl PanelDigest {
    fn of(domain: &[u8], secret: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(secret);
        Self(hasher.finalize().into())
    }

    /// The bytes, to bind in a query.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PartialEq for PanelDigest {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().ct_eq(other.0.as_slice()).into()
    }
}

impl Eq for PanelDigest {}

impl std::fmt::Debug for PanelDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PanelDigest(..)")
    }
}

/// The code that opens one session of the panel.
pub struct EntryCode(Secret<ENTRY_LEN>);

impl EntryCode {
    /// Draws a code from the generator of the system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn generate() -> Result<Self, RandomError> {
        Secret::random().map(Self)
    }

    /// Reads what a person typed or pasted: the 32 letters and digits, in either case, with
    /// or without the hyphens and spaces between the groups. Anything else is not a code.
    #[must_use]
    pub fn parse(typed: &str) -> Option<Self> {
        let mut text = Zeroizing::new(String::with_capacity(ENTRY_TEXT_LEN));
        for character in typed.chars() {
            match character {
                '-' | ' ' | '\t' | '\r' | '\n' => {}
                other if other.is_ascii_alphanumeric() => {
                    if text.len() == ENTRY_TEXT_LEN {
                        return None;
                    }
                    text.push(other.to_ascii_uppercase());
                }
                _ => return None,
            }
        }
        if text.len() != ENTRY_TEXT_LEN {
            return None;
        }
        let bytes = Zeroizing::new(BASE32_NOPAD.decode(text.as_bytes()).ok()?);
        Secret::from_slice(&bytes).map(Self)
    }

    /// The code as it is shown, once: eight groups of four, in lowercase.
    #[must_use]
    pub fn reveal(&self) -> Zeroizing<String> {
        let plain = Zeroizing::new(BASE32_NOPAD.encode(self.0.expose()).to_ascii_lowercase());
        let mut grouped = Zeroizing::new(String::with_capacity(ENTRY_TEXT_LEN + 7));
        for (index, character) in plain.chars().enumerate() {
            if index > 0 && index % 4 == 0 {
                grouped.push('-');
            }
            grouped.push(character);
        }
        grouped
    }

    /// What the hub keeps of it.
    #[must_use]
    pub fn digest(&self) -> PanelDigest {
        PanelDigest::of(ENTRY_DOMAIN, self.0.expose())
    }
}

impl std::fmt::Debug for EntryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EntryCode([redacted])")
    }
}

/// The identifier of a session of the panel: what the cookie carries.
pub struct SessionSecret(Secret<SESSION_LEN>);

impl SessionSecret {
    /// Draws an identifier from the generator of the system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn generate() -> Result<Self, RandomError> {
        Secret::random().map(Self)
    }

    /// Reads the value of the cookie. Anything that is not exactly one identifier is none.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if value.len() != SESSION_TEXT_LEN {
            return None;
        }
        let bytes = Zeroizing::new(BASE64URL_NOPAD.decode(value.as_bytes()).ok()?);
        Secret::from_slice(&bytes).map(Self)
    }

    /// The identifier as it goes in the cookie.
    #[must_use]
    pub fn reveal(&self) -> Zeroizing<String> {
        Zeroizing::new(BASE64URL_NOPAD.encode(self.0.expose()))
    }

    /// What the hub keeps of it.
    #[must_use]
    pub fn digest(&self) -> PanelDigest {
        PanelDigest::of(SESSION_DOMAIN, self.0.expose())
    }
}

impl std::fmt::Debug for SessionSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionSecret([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_code_is_read_back_however_a_person_types_it() {
        let code = EntryCode::generate().unwrap();
        let shown = code.reveal();
        assert_eq!(shown.len(), 39, "eight groups of four and seven hyphens");
        assert!(
            shown
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        );
        let digest = code.digest();
        for typed in [
            shown.to_string(),
            shown.to_ascii_uppercase(),
            shown.replace('-', ""),
            shown.replace('-', " "),
            format!("  {}\r\n", shown.as_str()),
        ] {
            assert!(
                EntryCode::parse(&typed).unwrap().digest() == digest,
                "{typed}"
            );
        }
    }

    #[test]
    fn what_is_not_a_code_is_refused_without_a_guess() {
        let shown = EntryCode::generate().unwrap().reveal().to_string();
        let plain = shown.replace('-', "");
        for typed in [
            String::new(),
            plain[..31].to_owned(),
            format!("{plain}a"),
            plain.replace(plain.chars().next().unwrap(), "!"),
            // 0, 1, 8 and 9 are not letters of this alphabet.
            "0189".repeat(8),
            "ação".repeat(8),
        ] {
            assert!(EntryCode::parse(&typed).is_none(), "{typed}");
        }
    }

    #[test]
    fn the_identifier_of_a_session_is_read_back_only_whole() {
        let session = SessionSecret::generate().unwrap();
        let value = session.reveal();
        assert_eq!(value.len(), SESSION_TEXT_LEN);
        assert!(SessionSecret::parse(&value).unwrap().digest() == session.digest());
        assert!(SessionSecret::parse(&value[..42]).is_none());
        assert!(SessionSecret::parse(&format!("{}=", value.as_str())).is_none());
        assert!(SessionSecret::parse("").is_none());
    }

    #[test]
    fn two_secrets_never_share_a_digest_and_nothing_prints_them() {
        let one = EntryCode::generate().unwrap();
        let two = EntryCode::generate().unwrap();
        assert!(one.digest() != two.digest());
        // The same bytes as a code and as an identifier do not give the same digest.
        assert!(
            PanelDigest::of(ENTRY_DOMAIN, &[7; 20]) != PanelDigest::of(SESSION_DOMAIN, &[7; 20])
        );
        let session = SessionSecret::generate().unwrap();
        for printed in [
            format!("{one:?}"),
            format!("{session:?}"),
            format!("{:?}", one.digest()),
        ] {
            assert!(!printed.contains(one.reveal().as_str()));
            assert!(!printed.contains(session.reveal().as_str()));
            assert!(printed.contains("redacted") || printed.contains(".."));
        }
    }
}
