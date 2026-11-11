//! The invite.
//!
//! An invite is 32 bytes drawn by the hub, written as `arc1` followed by their base32 without
//! padding, in lowercase: 56 characters, none of which a chat application formats. It lives 15
//! minutes, serves once and only to ask for a connection. The hub keeps only the SHA-256 of the
//! 32 bytes (`docs/14-protocolo-do-canal.md`, section 10).
//!
//! The text is read strictly: one text for each invite. The connector refuses anything else
//! locally, with AR-101, before talking to the hub.

use core::fmt;

use data_encoding::BASE32_NOPAD;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::digest::sha256;
use crate::limits::{DIGEST_LEN, INVITE_LEN, INVITE_TEXT_LEN};
use crate::secret::{RandomError, Secret};

/// What every invite text starts with.
pub const INVITE_PREFIX: &str = "arc1";

/// Why a text is not an invite. The text itself is never repeated: it may be a real invite
/// with a typing mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InviteError {
    /// Not exactly 56 characters.
    #[error("an invite has 56 characters")]
    Length,
    /// Does not start with `arc1`.
    #[error("an invite starts with arc1")]
    Prefix,
    /// A character outside `a-z` and `2-7` after the prefix.
    #[error("an invite has only the characters a-z and 2-7 after the prefix")]
    Character,
    /// The last character carries bits that no invite has.
    #[error("the invite is not written in its only valid form")]
    Encoding,
}

/// An invite: a short-lived secret. It appears once, inside the block, and is never stored in
/// the clear.
#[derive(Debug, PartialEq, Eq)]
pub struct Invite(Secret<INVITE_LEN>);

impl Invite {
    /// Draws a new invite from the generator of the operating system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn generate() -> Result<Self, RandomError> {
        Secret::random().map(Self)
    }

    /// An invite from its 32 bytes.
    #[must_use]
    pub const fn from_secret(bytes: Secret<INVITE_LEN>) -> Self {
        Self(bytes)
    }

    /// Reads the text of an invite, strictly.
    ///
    /// # Errors
    /// [`InviteError`] says which rule the text breaks, without repeating the text.
    pub fn parse(text: &str) -> Result<Self, InviteError> {
        if text.len() != INVITE_TEXT_LEN {
            return Err(InviteError::Length);
        }
        let body = text
            .strip_prefix(INVITE_PREFIX)
            .ok_or(InviteError::Prefix)?;
        if !body.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7')) {
            return Err(InviteError::Character);
        }
        // The library knows the uppercase alphabet. Uppercase in the input was refused above.
        let upper = Zeroizing::new(body.to_ascii_uppercase());
        let bytes = Zeroizing::new(
            BASE32_NOPAD
                .decode(upper.as_bytes())
                .map_err(|_| InviteError::Encoding)?,
        );
        Secret::from_slice(&bytes)
            .map(Self)
            .ok_or(InviteError::Encoding)
    }

    /// Writes the invite as text. This is the secret itself: it goes into the block, or into
    /// the `enroll` message, and nowhere else.
    #[must_use]
    pub fn reveal(&self) -> Zeroizing<String> {
        let mut body = Zeroizing::new(BASE32_NOPAD.encode(self.0.expose()));
        body.make_ascii_lowercase();
        let mut text = Zeroizing::new(String::with_capacity(INVITE_TEXT_LEN));
        text.push_str(INVITE_PREFIX);
        text.push_str(&body);
        text
    }

    /// The SHA-256 of the 32 bytes: what the hub stores, and how the connector recognizes the
    /// same invite presented again.
    #[must_use]
    pub fn digest(&self) -> InviteDigest {
        InviteDigest(sha256(self.0.expose()))
    }
}

/// SHA-256 of the 32 bytes of an invite. It is not a secret: whoever reads it cannot use the
/// invite.
#[derive(Clone, Copy)]
pub struct InviteDigest([u8; DIGEST_LEN]);

impl InviteDigest {
    /// A digest read from storage.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DIGEST_LEN]) -> Self {
        Self(bytes)
    }

    /// The 32 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DIGEST_LEN] {
        &self.0
    }
}

impl PartialEq for InviteDigest {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().ct_eq(other.0.as_slice()).into()
    }
}

impl Eq for InviteDigest {}

impl fmt::Debug for InviteDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InviteDigest({})",
            data_encoding::HEXLOWER.encode(&self.0)
        )
    }
}

impl_base64_serde!(InviteDigest, DIGEST_LEN, "invite digest");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{assert_hides, hex};

    const COUNTING: &str = "arc1aaaqeayeaudaocajbifqydiob4ibceqtcqkrmfyydenbwha5dypq";

    fn counting() -> Invite {
        let mut bytes = [0u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::try_from(index).unwrap();
        }
        Invite::from_secret(Secret::new(bytes))
    }

    #[test]
    fn the_text_matches_the_reference_values() {
        // Computed outside this crate, with the base32 of another standard library.
        assert_eq!(counting().reveal().as_str(), COUNTING);
        assert_eq!(
            Invite::from_secret(Secret::new([0u8; 32]))
                .reveal()
                .as_str(),
            "arc1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            Invite::from_secret(Secret::new([0xffu8; 32]))
                .reveal()
                .as_str(),
            "arc1777777777777777777777777777777777777777777777777777q"
        );
    }

    #[test]
    fn a_generated_invite_has_the_documented_shape() {
        let invite = Invite::generate().unwrap();
        let text = invite.reveal();
        assert_eq!(text.len(), 56);
        assert!(text.starts_with("arc1"));
        assert!(
            text[4..]
                .bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        );
        assert_eq!(Invite::parse(&text).unwrap(), invite);
        assert_ne!(
            Invite::generate().unwrap(),
            invite,
            "two draws gave the same invite"
        );
    }

    #[test]
    fn reading_and_writing_agree() {
        assert_eq!(Invite::parse(COUNTING).unwrap(), counting());
        assert_eq!(Invite::parse(COUNTING).unwrap().reveal().as_str(), COUNTING);
        for _ in 0..50 {
            let invite = Invite::generate().unwrap();
            assert_eq!(Invite::parse(&invite.reveal()).unwrap(), invite);
        }
    }

    #[test]
    fn the_size_is_exact() {
        assert_eq!(Invite::parse(""), Err(InviteError::Length));
        assert_eq!(Invite::parse("arc1"), Err(InviteError::Length));
        assert_eq!(Invite::parse(&COUNTING[..55]), Err(InviteError::Length));
        assert_eq!(
            Invite::parse(&format!("{COUNTING}a")),
            Err(InviteError::Length)
        );
        assert_eq!(
            Invite::parse(&format!("{COUNTING}\n")),
            Err(InviteError::Length)
        );
        assert_eq!(
            Invite::parse(&format!(" {COUNTING}")),
            Err(InviteError::Length)
        );
    }

    #[test]
    fn the_prefix_is_exact() {
        for prefix in [
            "arc2", "ARC1", "Arc1", "arc0", "inv1", "aaaa", "rc1a", " arc",
        ] {
            let text = format!("{prefix}{}", &COUNTING[4..]);
            assert_eq!(Invite::parse(&text), Err(InviteError::Prefix), "{prefix}");
        }
    }

    #[test]
    fn uppercase_is_refused() {
        let upper = format!("arc1{}", COUNTING[4..].to_ascii_uppercase());
        assert_eq!(Invite::parse(&upper), Err(InviteError::Character));
        // One uppercase letter is enough, wherever it is.
        for position in 4..56 {
            let mut text = COUNTING.as_bytes().to_vec();
            if !text[position].is_ascii_lowercase() {
                continue;
            }
            text[position] = text[position].to_ascii_uppercase();
            let text = String::from_utf8(text).unwrap();
            assert_eq!(
                Invite::parse(&text),
                Err(InviteError::Character),
                "position {position}"
            );
        }
        assert_eq!(
            Invite::parse(&COUNTING.to_ascii_uppercase()),
            Err(InviteError::Prefix)
        );
    }

    #[test]
    fn padding_and_foreign_characters_are_refused() {
        for foreign in [
            '=', '0', '1', '8', '9', '-', '_', ' ', '\n', '+', '/', '.', '\0',
        ] {
            for position in [4usize, 30, 55] {
                let mut text = COUNTING.to_owned();
                text.replace_range(position..=position, &foreign.to_string());
                assert_eq!(text.len(), 56);
                assert_eq!(
                    Invite::parse(&text),
                    Err(InviteError::Character),
                    "{foreign:?} at {position}"
                );
            }
        }
        // A text with padding in the usual place: 52 characters would need none.
        let padded = format!("{}====", &COUNTING[..52]);
        assert_eq!(Invite::parse(&padded), Err(InviteError::Character));
        // Two bytes that are one character keep the size and are still refused.
        let accented = format!("{}\u{e9}", &COUNTING[..54]);
        assert_eq!(accented.len(), 56);
        assert_eq!(Invite::parse(&accented), Err(InviteError::Character));
    }

    #[test]
    fn only_one_text_reads_as_each_invite() {
        // 52 characters carry 260 bits, and an invite has 256: the last character can only be
        // `a` or `q`. Any other one would be a second text for the same bytes.
        let stem = &COUNTING[..55];
        let mut accepted = Vec::new();
        for last in "abcdefghijklmnopqrstuvwxyz234567".chars() {
            let text = format!("{stem}{last}");
            match Invite::parse(&text) {
                Ok(invite) => {
                    assert_eq!(invite.reveal().as_str(), text);
                    accepted.push(last);
                }
                Err(error) => assert_eq!(error, InviteError::Encoding, "{last}"),
            }
        }
        assert_eq!(accepted, ['a', 'q']);
    }

    #[test]
    fn the_digest_is_the_sha256_of_the_32_bytes() {
        assert_eq!(
            hex(Invite::from_secret(Secret::new([0u8; 32]))
                .digest()
                .as_bytes()),
            "66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925"
        );
        assert_eq!(
            hex(counting().digest().as_bytes()),
            "630dcd2966c4336691125448bbb25b4ff412a49c732db2c8abc1b8581bd710dd"
        );
        assert_eq!(
            counting().digest(),
            Invite::parse(COUNTING).unwrap().digest()
        );
        assert_ne!(counting().digest(), Invite::generate().unwrap().digest());
    }

    #[test]
    fn the_digest_goes_through_storage() {
        let digest = counting().digest();
        assert_eq!(InviteDigest::from_bytes(*digest.as_bytes()), digest);
        let json = serde_json::to_string(&digest).unwrap();
        assert_eq!(serde_json::from_str::<InviteDigest>(&json).unwrap(), digest);
        assert!(serde_json::from_str::<InviteDigest>("\"AAAA\"").is_err());
        assert_eq!(
            format!("{digest:?}"),
            "InviteDigest(630dcd2966c4336691125448bbb25b4ff412a49c732db2c8abc1b8581bd710dd)"
        );
    }

    #[test]
    fn an_invite_does_not_print_itself() {
        let invite = counting();
        let debug = format!("{invite:?} {invite:#?}");
        assert_eq!(format!("{invite:?}"), "Invite([redacted])");
        let bytes: Vec<u8> = (0u8..32).collect();
        assert_hides(&debug, &bytes, "Debug of Invite");
        assert!(!debug.contains(&COUNTING[4..]), "{debug}");
        assert_not_impl!(Invite: core::fmt::Display, serde::Serialize, Clone);
    }

    #[test]
    fn errors_do_not_repeat_the_text() {
        let almost = format!("{}A", &COUNTING[..55]);
        for (text, error) in [
            (almost.as_str(), InviteError::Character),
            (&COUNTING[..40], InviteError::Length),
        ] {
            let failure = Invite::parse(text).unwrap_err();
            assert_eq!(failure, error);
            let shown = format!("{failure} {failure:?}");
            assert!(!shown.contains(&COUNTING[4..20]), "{shown}");
        }
    }
}
