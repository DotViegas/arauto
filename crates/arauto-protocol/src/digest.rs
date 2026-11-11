//! The digest of a sealed credential.
//!
//! The connector answers `credential` with `saved`, which carries the SHA-256 of the block it
//! received and wrote to disk. The hub keeps, for each device, only the generation and this
//! digest of the current and of the pending credential, never the credential itself
//! (`docs/14-protocolo-do-canal.md`, sections 5.3, 6 and 9).

use core::fmt;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::limits::DIGEST_LEN;
use crate::secret::SealedCredential;

/// SHA-256 of `bytes`.
pub(crate) fn sha256(bytes: &[u8]) -> [u8; DIGEST_LEN] {
    Sha256::digest(bytes).into()
}

/// SHA-256 of a whole sealed credential, exactly as the hub issued it.
#[derive(Clone, Copy)]
pub struct CredentialDigest([u8; DIGEST_LEN]);

impl CredentialDigest {
    /// The digest of `credential`.
    #[must_use]
    pub fn of(credential: &SealedCredential) -> Self {
        Self(sha256(credential.expose()))
    }

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

impl PartialEq for CredentialDigest {
    /// Compares in constant time: the hub decides a promotion on this comparison.
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().ct_eq(other.0.as_slice()).into()
    }
}

impl Eq for CredentialDigest {}

impl fmt::Debug for CredentialDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CredentialDigest({})",
            data_encoding::HEXLOWER.encode(&self.0)
        )
    }
}

impl_base64_serde!(CredentialDigest, DIGEST_LEN, "credential digest");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::hex;

    #[test]
    fn sha256_matches_the_published_vectors() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_digest_is_the_sha256_of_the_whole_block() {
        let zeros = SealedCredential::from_slice(&[0u8; 32]).unwrap();
        assert_eq!(
            hex(CredentialDigest::of(&zeros).as_bytes()),
            "66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925"
        );
        let counting: Vec<u8> = (0u8..32).collect();
        let counting = SealedCredential::new(counting).unwrap();
        assert_eq!(
            hex(CredentialDigest::of(&counting).as_bytes()),
            "630dcd2966c4336691125448bbb25b4ff412a49c732db2c8abc1b8581bd710dd"
        );
    }

    #[test]
    fn any_change_in_the_block_changes_the_digest() {
        let block = vec![0x41u8; 200];
        let digest = CredentialDigest::of(&SealedCredential::from_slice(&block).unwrap());
        assert_eq!(
            digest,
            CredentialDigest::of(&SealedCredential::from_slice(&block).unwrap())
        );
        for position in [0usize, 1, 99, 199] {
            let mut changed = block.clone();
            changed[position] ^= 0x01;
            let other = CredentialDigest::of(&SealedCredential::from_slice(&changed).unwrap());
            assert_ne!(digest, other, "byte {position}");
        }
        // One byte more or less is another block.
        let shorter = CredentialDigest::of(&SealedCredential::from_slice(&block[..199]).unwrap());
        assert_ne!(digest, shorter);
        let mut longer = block.clone();
        longer.push(0x41);
        assert_ne!(
            digest,
            CredentialDigest::of(&SealedCredential::new(longer).unwrap())
        );
    }

    #[test]
    fn comparison_sees_every_byte() {
        let base = CredentialDigest::from_bytes([7u8; 32]);
        for position in 0..32 {
            let mut bytes = [7u8; 32];
            bytes[position] ^= 0x80;
            assert_ne!(base, CredentialDigest::from_bytes(bytes), "byte {position}");
        }
        assert_eq!(base, CredentialDigest::from_bytes([7u8; 32]));
    }

    #[test]
    fn the_digest_travels_as_strict_base64() {
        let digest = CredentialDigest::from_bytes([0xabu8; 32]);
        let json = serde_json::to_string(&digest).unwrap();
        assert_eq!(json, "\"q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6s=\"");
        assert_eq!(
            serde_json::from_str::<CredentialDigest>(&json).unwrap(),
            digest
        );

        let quoted = |text: String| format!("\"{text}\"");
        let standard = crate::base64::encode(&[0xfbu8; 32]);
        assert!(serde_json::from_str::<CredentialDigest>(&quoted(standard.clone())).is_ok());
        let bad = [
            // One byte less and one byte more.
            quoted(crate::base64::encode(&[0xabu8; 31])),
            quoted(crate::base64::encode(&[0xabu8; 33])),
            // No padding, URL alphabet, hexadecimal, nothing.
            quoted(standard.trim_end_matches('=').to_owned()),
            quoted(data_encoding::BASE64URL.encode(&[0xfbu8; 32])),
            quoted(hex(&[0xabu8; 32])),
            quoted(String::new()),
            "null".to_owned(),
            "7".to_owned(),
        ];
        for json in bad {
            assert!(
                serde_json::from_str::<CredentialDigest>(&json).is_err(),
                "{json}"
            );
        }
    }

    #[test]
    fn debug_shows_the_digest_in_hexadecimal() {
        let digest = CredentialDigest::from_bytes([0x0fu8; 32]);
        assert_eq!(
            format!("{digest:?}"),
            format!("CredentialDigest({})", "0f".repeat(32))
        );
    }
}
