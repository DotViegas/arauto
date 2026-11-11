//! Types that hold a secret.
//!
//! A secret has its own type: it is wiped when dropped, prints `[redacted]`, compares in
//! constant time and has neither `Display` nor a serde implementation. Reading the bytes takes an
//! explicit call to `expose`, which is easy to find in a review.
//!
//! [`Secret`] has a fixed size and holds keys and the bytes of an invite. [`SealedCredential`]
//! holds the sealed block the hub issues, which the connector keeps and never opens.
//!
//! # What is not wiped
//!
//! A secret type wipes its own copy, and the copies this crate makes on the way are wiped
//! where it owns them: the base64 text of a credential, the text of an invite, the private
//! keys inside a handshake. Two kinds of copy stay in memory, in the clear, until the
//! allocator reuses it:
//!
//! - the bytes of a message on its way. What [`crate::message::encode`] returns for `enroll`,
//!   `present` and `credential`, the frames cut from it, the frame an
//!   [`crate::noise::Opener`] opens and the message a [`crate::frame::Reassembler`] delivers
//!   are plain buffers, as is the JSON value a message is read from. They belong to whoever
//!   called, who can wipe them when done;
//! - what the Noise library keeps of a handshake besides the private keys: the chaining key
//!   and the two transport keys of the connection.

use core::fmt;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::limits::CREDENTIAL_MAX_LEN;

/// What every secret prints instead of its content.
const REDACTED: &str = "[redacted]";

/// The random generator of the operating system failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the random generator of the operating system failed")]
pub struct RandomError;

/// Fills `dest` from the generator of the operating system, the only source of randomness for
/// anything that must not be guessed.
pub(crate) fn fill_random(dest: &mut [u8]) -> Result<(), RandomError> {
    getrandom::fill(dest).map_err(|_| RandomError)
}

/// A secret of `N` bytes.
pub struct Secret<const N: usize>([u8; N]);

impl<const N: usize> Secret<N> {
    /// Wraps `bytes`. The caller should not keep another copy.
    #[must_use]
    pub const fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    /// Draws a new secret from the generator of the operating system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn random() -> Result<Self, RandomError> {
        let mut secret = Self([0; N]);
        fill_random(&mut secret.0)?;
        Ok(secret)
    }

    /// Copies a secret from `bytes`. Returns `None` when the length is not `N`.
    #[must_use]
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        <[u8; N]>::try_from(bytes).ok().map(Self)
    }

    /// The bytes of the secret. Whoever calls this answers for where they go.
    #[must_use]
    pub const fn expose(&self) -> &[u8; N] {
        &self.0
    }
}

impl<const N: usize> fmt::Debug for Secret<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<const N: usize> ConstantTimeEq for Secret<N> {
    fn ct_eq(&self, other: &Self) -> subtle::Choice {
        self.0.as_slice().ct_eq(other.0.as_slice())
    }
}

impl<const N: usize> PartialEq for Secret<N> {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl<const N: usize> Eq for Secret<N> {}

impl<const N: usize> Zeroize for Secret<N> {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl<const N: usize> Drop for Secret<N> {
    fn drop(&mut self) {
        self.0.zeroize();
        #[cfg(test)]
        wipe_probe::record(self.0.iter().all(|byte| *byte == 0));
    }
}

impl<const N: usize> ZeroizeOnDrop for Secret<N> {}

/// A sealed credential does not have from 1 to 1,024 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a sealed credential has from 1 to 1024 bytes")]
pub struct CredentialLengthError;

/// The sealed credential: a block of bytes only the hub opens.
///
/// The connector stores it as received and presents it to come back. It is a lasting secret:
/// it never goes to a screen, a log, an audit record or a tool answer.
pub struct SealedCredential(Vec<u8>);

impl SealedCredential {
    /// Wraps the bytes of a credential. On error the bytes are wiped.
    ///
    /// # Errors
    /// [`CredentialLengthError`] when `bytes` is empty or longer than 1,024 bytes.
    pub fn new(mut bytes: Vec<u8>) -> Result<Self, CredentialLengthError> {
        if bytes.is_empty() || bytes.len() > CREDENTIAL_MAX_LEN {
            wipe(&mut bytes);
            return Err(CredentialLengthError);
        }
        Ok(Self(bytes))
    }

    /// Copies a credential from `bytes`.
    ///
    /// # Errors
    /// [`CredentialLengthError`] when `bytes` is empty or longer than 1,024 bytes.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CredentialLengthError> {
        if bytes.is_empty() || bytes.len() > CREDENTIAL_MAX_LEN {
            return Err(CredentialLengthError);
        }
        Ok(Self(bytes.to_vec()))
    }

    /// The bytes of the credential. Whoever calls this answers for where they go.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for SealedCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl ConstantTimeEq for SealedCredential {
    fn ct_eq(&self, other: &Self) -> subtle::Choice {
        self.0.as_slice().ct_eq(other.0.as_slice())
    }
}

impl PartialEq for SealedCredential {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl Eq for SealedCredential {}

impl Zeroize for SealedCredential {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for SealedCredential {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Wipes the content and the spare capacity of `bytes`, and leaves it empty.
fn wipe(bytes: &mut Vec<u8>) {
    bytes.as_mut_slice().zeroize();
    #[cfg(test)]
    wipe_probe::record(bytes.iter().all(|byte| *byte == 0));
    bytes.zeroize();
}

impl ZeroizeOnDrop for SealedCredential {}

/// Lets a test see that dropping a secret wiped it: the bytes cannot be read after the drop
/// without `unsafe`, so the drop itself reports what it left behind.
#[cfg(test)]
pub(crate) mod wipe_probe {
    use std::cell::Cell;

    thread_local! {
        static WIPES: Cell<(u32, bool)> = const { Cell::new((0, true)) };
    }

    pub(crate) fn record(all_zero: bool) {
        WIPES.with(|wipes| {
            let (count, clean) = wipes.get();
            wipes.set((count.saturating_add(1), clean && all_zero));
        });
    }

    /// How many secrets this thread dropped since the last call, and whether all were zero.
    pub(crate) fn take() -> (u32, bool) {
        WIPES.with(|wipes| wipes.replace((0, true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANARY: [u8; 32] = *b"CANARY-secret-bytes-0123456789ab";

    #[test]
    fn debug_shows_redacted_and_nothing_else() {
        let secret = Secret::new(CANARY);
        assert_eq!(format!("{secret:?}"), "[redacted]");
        assert_eq!(format!("{secret:#?}"), "[redacted]");
        let credential = SealedCredential::from_slice(&CANARY).unwrap();
        assert_eq!(format!("{credential:?}"), "[redacted]");
        assert_eq!(format!("{credential:#?}"), "[redacted]");
        // Inside another structure the content stays hidden too.
        let nested = format!("{:?}", (Some(&secret), vec![&credential]));
        assert_eq!(nested, "(Some([redacted]), [[redacted]])");
    }

    #[test]
    fn secrets_have_no_display_no_serde_and_no_copy() {
        assert_not_impl!(Secret<32>: core::fmt::Display, serde::Serialize, Clone, Copy);
        assert_not_impl!(SealedCredential: core::fmt::Display, serde::Serialize, Clone, Copy);
    }

    #[test]
    fn comparison_sees_every_byte() {
        let a = Secret::new(CANARY);
        let b = Secret::new(CANARY);
        assert_eq!(a, b);
        assert!(bool::from(a.ct_eq(&b)));
        for position in 0..CANARY.len() {
            let mut changed = CANARY;
            changed[position] ^= 1;
            let other = Secret::new(changed);
            assert_ne!(a, other, "a difference at byte {position} went unnoticed");
            assert!(!bool::from(a.ct_eq(&other)));
        }
    }

    #[test]
    fn from_slice_wants_the_exact_length() {
        assert!(Secret::<32>::from_slice(&CANARY).is_some());
        assert!(Secret::<32>::from_slice(&CANARY[..31]).is_none());
        assert!(Secret::<32>::from_slice(&[0u8; 33]).is_none());
        assert!(Secret::<32>::from_slice(&[]).is_none());
        assert_eq!(Secret::<32>::from_slice(&CANARY).unwrap().expose(), &CANARY);
    }

    #[test]
    fn random_secrets_differ() {
        let a = Secret::<32>::random().unwrap();
        let b = Secret::<32>::random().unwrap();
        assert_ne!(a, b);
        assert_ne!(a.expose(), &[0u8; 32]);
    }

    #[test]
    fn zeroize_wipes_in_place() {
        let mut secret = Secret::new(CANARY);
        secret.zeroize();
        assert_eq!(secret.expose(), &[0u8; 32]);
        let mut credential = SealedCredential::from_slice(&CANARY).unwrap();
        credential.zeroize();
        assert!(credential.expose().is_empty());
    }

    #[test]
    fn dropping_a_secret_wipes_it() {
        let _ = wipe_probe::take();
        drop(Secret::new(CANARY));
        assert_eq!(wipe_probe::take(), (1, true));
        drop(SealedCredential::from_slice(&CANARY).unwrap());
        assert_eq!(wipe_probe::take(), (1, true));
        // A refused credential is wiped as well, although it never becomes a value to drop.
        assert!(SealedCredential::new(vec![7u8; CREDENTIAL_MAX_LEN + 1]).is_err());
        assert_eq!(wipe_probe::take(), (1, true));
    }

    #[test]
    fn credential_length_limits() {
        assert_eq!(
            SealedCredential::new(Vec::new()),
            Err(CredentialLengthError)
        );
        assert_eq!(
            SealedCredential::from_slice(&[]),
            Err(CredentialLengthError)
        );
        assert!(SealedCredential::new(vec![1]).is_ok());
        assert!(SealedCredential::new(vec![1; CREDENTIAL_MAX_LEN]).is_ok());
        assert!(SealedCredential::from_slice(&[1; CREDENTIAL_MAX_LEN]).is_ok());
        assert_eq!(
            SealedCredential::new(vec![1; CREDENTIAL_MAX_LEN + 1]),
            Err(CredentialLengthError)
        );
        assert_eq!(
            SealedCredential::from_slice(&[1; CREDENTIAL_MAX_LEN + 1]),
            Err(CredentialLengthError)
        );
    }

    #[test]
    fn credential_comparison_sees_content_and_length() {
        let a = SealedCredential::from_slice(&[1, 2, 3]).unwrap();
        assert_eq!(a, SealedCredential::from_slice(&[1, 2, 3]).unwrap());
        assert_ne!(a, SealedCredential::from_slice(&[1, 2, 4]).unwrap());
        assert_ne!(a, SealedCredential::from_slice(&[1, 2]).unwrap());
        assert_ne!(a, SealedCredential::from_slice(&[1, 2, 3, 0]).unwrap());
    }

    #[test]
    fn error_texts_carry_no_input() {
        assert_eq!(
            RandomError.to_string(),
            "the random generator of the operating system failed"
        );
        assert_eq!(
            CredentialLengthError.to_string(),
            "a sealed credential has from 1 to 1024 bytes"
        );
    }
}
