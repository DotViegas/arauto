//! The handle in the address of an instruction page (`docs/decisoes/0042`).
//!
//! 16 bytes from the generator of the system, written as 26 lowercase letters and digits. It is
//! **not** the invite: it is born with it, the hub keeps only its SHA-256 (with a domain of its
//! own, so that the digest of a handle is never the digest of another secret), and the digest is
//! erased with the invite.
//!
//! `PageHandle` has no `Display`, `Clone` or serde, and its `Debug` says `[redacted]`.

use arauto_protocol::secret::{RandomError, Secret};
use data_encoding::BASE32_NOPAD;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

/// The bytes of a handle.
const LEN: usize = 16;

/// The letters and digits of a handle, as they go in the address.
pub const TEXT_LEN: usize = 26;

/// What is hashed before the handle.
const DOMAIN: &[u8] = b"arauto join page\0";

/// The SHA-256 of a handle: what the database keeps and what a page is found by.
#[derive(Clone, Copy)]
pub struct PageDigest([u8; 32]);

impl PageDigest {
    /// The bytes, to bind in a query.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PartialEq for PageDigest {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().ct_eq(other.0.as_slice()).into()
    }
}

impl Eq for PageDigest {}

impl std::fmt::Debug for PageDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PageDigest(..)")
    }
}

/// The handle of the instruction page of one invite.
pub struct PageHandle(Secret<LEN>);

impl PageHandle {
    /// Draws a handle from the generator of the system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn generate() -> Result<Self, RandomError> {
        Secret::random().map(Self)
    }

    /// Reads the text of an address: exactly 26 characters of `a-z` and `2-7`, the last one
    /// canonical. Anything else is not a handle.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != TEXT_LEN || !text.bytes().all(|b| matches!(b, b'a'..=b'z' | b'2'..=b'7')) {
            return None;
        }
        let upper = Zeroizing::new(text.to_ascii_uppercase());
        // The decoder refuses a last character with bits set past the 128: one text per handle.
        let bytes = Zeroizing::new(BASE32_NOPAD.decode(upper.as_bytes()).ok()?);
        Secret::from_slice(&bytes).map(Self)
    }

    /// The handle as it goes in the address: lowercase base32 without padding.
    #[must_use]
    pub fn reveal(&self) -> Zeroizing<String> {
        let mut text = Zeroizing::new(BASE32_NOPAD.encode(self.0.expose()));
        text.make_ascii_lowercase();
        text
    }

    /// What the hub keeps of it.
    #[must_use]
    pub fn digest(&self) -> PageDigest {
        let mut hasher = Sha256::new();
        hasher.update(DOMAIN);
        hasher.update(self.0.expose());
        PageDigest(hasher.finalize().into())
    }
}

impl std::fmt::Debug for PageHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PageHandle([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_has_one_text_and_does_not_print_itself() {
        let handle = PageHandle::generate().unwrap();
        let text = handle.reveal();
        assert_eq!(text.len(), TEXT_LEN);
        assert!(
            text.bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        );
        let again = PageHandle::parse(&text).expect("the text of a handle is read back");
        assert!(again.digest() == handle.digest());
        assert_eq!(again.reveal().as_str(), text.as_str());
        // The strict form.
        assert!(PageHandle::parse(&text[..25]).is_none(), "25 characters");
        assert!(
            PageHandle::parse(&format!("{}a", text.as_str())).is_none(),
            "27 characters"
        );
        assert!(PageHandle::parse(&text.to_ascii_uppercase()).is_none());
        assert!(PageHandle::parse(&format!("0{}", &text[1..])).is_none());
        assert!(PageHandle::parse(&format!("{}-", &text[..25])).is_none());
        assert!(PageHandle::parse("").is_none());
        // The last character carries 3 bits of the key and 2 of padding: one with the padding
        // set is another text for the same bytes, and is refused.
        let canonical = "aaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert!(PageHandle::parse(canonical).is_some());
        assert!(PageHandle::parse("aaaaaaaaaaaaaaaaaaaaaaaaab").is_none());
        assert!(PageHandle::parse("aaaaaaaaaaaaaaaaaaaaaaaaaq").is_some());
        // Nothing prints it.
        assert_eq!(format!("{handle:?}"), "PageHandle([redacted])");
        assert_eq!(format!("{:?}", handle.digest()), "PageDigest(..)");
        // The digest has its own domain, pinned: SHA-256 of "arauto join page\0" and 16 zeros.
        let zero = PageHandle::parse(canonical).unwrap();
        let mut expected = Sha256::new();
        expected.update(b"arauto join page\0");
        expected.update([0_u8; 16]);
        assert_eq!(
            zero.digest().as_bytes().as_slice(),
            expected.finalize().as_slice()
        );
        assert_ne!(
            zero.digest().as_bytes().as_slice(),
            Sha256::digest([0_u8; 16]).as_slice(),
            "a digest of its own domain"
        );
    }
}
