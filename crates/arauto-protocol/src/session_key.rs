//! The key of the Hermes session of a WhatsApp contact (decision 0054, section 1).
//!
//! The key is a fixed prefix and the 64 lowercase hexadecimal characters of the SHA-256 of a
//! canonical string: lines joined by `\n`, in UTF-8, with no final newline. Nothing is
//! invented here: the digest is SHA-256 as it is, with no key and no secret, so an outside
//! sender can build the same key without Arauto's code (`docs/14-protocolo-do-canal.md`,
//! section 5.3). The digest is not secrecy: telephones can be found by trying them.
//!
//! | Kind of chat | Key | Canonical string, one line each |
//! |---|---|---|
//! | Contact with a known telephone | `arauto-wa1-<hex>` | `arauto-hermes-session-v1`, `whatsapp-contact`, `+<business>`, `+<contact>` |
//! | Group | `arauto-wg1-<hex>` | `arauto-hermes-session-v1`, `whatsapp-group`, `+<business>`, `<group id without @ suffix>` |
//! | Contact known only by a hidden identifier | `arauto-wh1-<hex>` | `arauto-hermes-session-v1`, `whatsapp-hidden`, `+<business>`, `<hidden id without @ suffix>` |
//!
//! The telephones are E.164 digits. A Brazilian mobile is written in its 13-digit form
//! ([`canonical_phone`]), so the same person gets one key whether or not the ninth digit was
//! there.

use core::fmt;

use sha2::{Digest, Sha256};

/// The fixed first line of every canonical string.
const VERSION_LINE: &str = "arauto-hermes-session-v1";

/// The three prefixes a key can have.
const PREFIXES: [&str; 3] = ["arauto-wa1-", "arauto-wg1-", "arauto-wh1-"];

/// The length of every key.
pub const KEY_LEN: usize = 11 + 64;

/// The canonical form of a telephone: E.164 digits, without `+`.
///
/// A `55` telephone of 12 digits whose fifth digit is from 6 to 9 gets a `9` after the area
/// code. Every other telephone is returned as it is.
#[must_use]
pub fn canonical_phone(digits: &str) -> String {
    let bytes = digits.as_bytes();
    if bytes.len() == 12
        && digits.starts_with("55")
        && bytes.iter().all(u8::is_ascii_digit)
        && matches!(bytes.get(4), Some(b'6'..=b'9'))
    {
        let (area, rest) = digits.split_at(4);
        return format!("{area}9{rest}");
    }
    digits.to_owned()
}

fn plus_canonical(digits: &str) -> String {
    format!(
        "+{}",
        canonical_phone(digits.strip_prefix('+').unwrap_or(digits))
    )
}

fn derive(prefix: &str, kind: &str, business: &str, subject: &str) -> String {
    let canonical = [VERSION_LINE, kind, business, subject].join("\n");
    let digest = Sha256::digest(canonical.as_bytes());
    let mut key = String::with_capacity(KEY_LEN);
    key.push_str(prefix);
    for byte in digest {
        key.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        key.push(char::from_digit(u32::from(byte & 15), 16).unwrap_or('0'));
    }
    key
}

/// The key of a contact with a known telephone. Both arguments are E.164 digits, with or
/// without a leading `+`; both are put in canonical form.
#[must_use]
pub fn whatsapp_contact_key(business: &str, contact: &str) -> String {
    derive(
        "arauto-wa1-",
        "whatsapp-contact",
        &plus_canonical(business),
        &plus_canonical(contact),
    )
}

/// The key of a group: the identifier of the engine, without the `@` suffix.
#[must_use]
pub fn whatsapp_group_key(business: &str, group_id: &str) -> String {
    derive(
        "arauto-wg1-",
        "whatsapp-group",
        &plus_canonical(business),
        group_id,
    )
}

/// The key of a contact known only by a hidden identifier: digits, without the `@` suffix.
#[must_use]
pub fn whatsapp_hidden_key(business: &str, hidden_id: &str) -> String {
    derive(
        "arauto-wh1-",
        "whatsapp-hidden",
        &plus_canonical(business),
        hidden_id,
    )
}

/// A key that passed the format check: `^arauto-(wa1|wg1|wh1)-[0-9a-f]{64}$`. Its `Debug`
/// never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionKey(String);

impl SessionKey {
    /// Checks the exact format.
    ///
    /// # Errors
    /// [`BadSessionKey`] for anything else.
    pub fn parse(text: &str) -> Result<Self, BadSessionKey> {
        let hex = PREFIXES
            .iter()
            .find_map(|prefix| text.strip_prefix(prefix))
            .ok_or(BadSessionKey)?;
        if hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self(text.to_owned()))
        } else {
            Err(BadSessionKey)
        }
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionKey([redacted])")
    }
}

/// The text is not a key of the Hermes session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not a session key")]
pub struct BadSessionKey;

#[cfg(test)]
mod tests {
    use super::*;

    const VECTOR: &str =
        "arauto-wa1-6335221bb43b84c8931d4447bc2a89f739f1fc9e08d205886c855b0e34a1edb4";

    #[test]
    fn the_published_test_vector() {
        assert_eq!(whatsapp_contact_key("15555550100", "15555550123"), VECTOR);
        assert_eq!(whatsapp_contact_key("+15555550100", "+15555550123"), VECTOR);
        assert!(SessionKey::parse(VECTOR).is_ok());
    }

    #[test]
    fn both_brazilian_forms_give_one_key() {
        assert_eq!(canonical_phone("551199999999"), "5511999999999");
        assert_eq!(canonical_phone("5511999999999"), "5511999999999");
        // A landline (fifth digit below 6), other lengths and other countries stay as they are.
        assert_eq!(canonical_phone("551133334444"), "551133334444");
        assert_eq!(canonical_phone("15555550123"), "15555550123");
        assert_eq!(canonical_phone("55119999999"), "55119999999");
        assert_eq!(
            whatsapp_contact_key("15555550100", "551199999999"),
            whatsapp_contact_key("15555550100", "5511999999999")
        );
    }

    #[test]
    fn groups_and_hidden_keys_have_their_own_prefix_and_differ() {
        let group = whatsapp_group_key("15555550100", "120363000000000001");
        let hidden = whatsapp_hidden_key("15555550100", "120363000000000001");
        assert!(group.starts_with("arauto-wg1-") && hidden.starts_with("arauto-wh1-"));
        assert_ne!(group[11..], hidden[11..]);
        assert_eq!(group.len(), KEY_LEN);
        assert!(SessionKey::parse(&group).is_ok() && SessionKey::parse(&hidden).is_ok());
        assert_ne!(
            whatsapp_group_key("15555550100", "1"),
            whatsapp_group_key("15555550101", "1")
        );
    }

    #[test]
    fn the_format_is_exact() {
        let hex = &VECTOR[11..];
        for bad in [
            VECTOR.to_uppercase(),
            format!("arauto-wx1-{hex}"),
            format!("arauto-wa1-{}", &hex[1..]),
            format!("arauto-wa1-{hex}0"),
            format!("arauto-wa1-{hex}\n"),
            format!(" arauto-wa1-{hex}"),
            format!("arauto-wa1-{}", hex.replacen('6', "G", 1)),
            "../../etc/passwd".to_owned(),
            "+15555550123".to_owned(),
            String::new(),
        ] {
            assert!(SessionKey::parse(&bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            format!("{:?}", SessionKey::parse(VECTOR).unwrap()),
            "SessionKey([redacted])"
        );
    }
}
