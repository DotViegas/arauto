//! The verification code.
//!
//! Hub and connector compute, each on its own, six digits from two things: the handshake hash
//! and a nonce the hub draws and only sends, inside the encrypted channel, after the last
//! handshake message. The connector shows the code, the Hermes says it, the admin types it.
//! The hub never sends or shows the code (`docs/04-seguranca-conexao.md`, C4).
//!
//! The function is the one of `docs/14-protocolo-do-canal.md`, section 8:
//!
//! ```text
//! material = HKDF-SHA-256( salt = hub nonce (32 bytes),
//!                          key  = handshake hash (32 bytes),
//!                          info = "arauto/code/1" ),  64 bytes of output
//! code     = six digits taken from the material, without bias
//! ```
//!
//! "Without bias": the material is read as 16 words of 4 bytes, most significant byte first.
//! The first word below 4,000,000,000 is reduced modulo 1,000,000 and the others are discarded.
//! When no word serves, the hub draws another nonce.

use core::fmt;

use hkdf::Hkdf;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::limits::ENROLL_NONCE_LEN;
use crate::noise::HandshakeHash;
use crate::secret::{RandomError, fill_random};

/// The `info` of the derivation.
pub const CODE_INFO: &[u8] = b"arauto/code/1";

/// Bytes of key material the code is taken from.
const MATERIAL_LEN: usize = 64;

/// Words at or above this value are discarded. It is the largest multiple of the modulus that
/// fits in 32 bits, so every code is as likely as any other.
const WORD_LIMIT: u32 = 4_000_000_000;

/// The code has six digits.
const CODE_MODULUS: u32 = 1_000_000;

/// The nonce the hub draws for one enrollment and sends in `enrolled`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EnrollNonce([u8; ENROLL_NONCE_LEN]);

impl EnrollNonce {
    /// Draws a nonce from the generator of the operating system.
    ///
    /// # Errors
    /// [`RandomError`] when the generator fails.
    pub fn random() -> Result<Self, RandomError> {
        let mut bytes = [0u8; ENROLL_NONCE_LEN];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// A nonce from its 32 bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; ENROLL_NONCE_LEN]) -> Self {
        Self(bytes)
    }

    /// The 32 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ENROLL_NONCE_LEN] {
        &self.0
    }
}

impl fmt::Debug for EnrollNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EnrollNonce({})",
            data_encoding::HEXLOWER.encode(&self.0)
        )
    }
}

impl_base64_serde!(EnrollNonce, ENROLL_NONCE_LEN, "enrollment nonce");

/// The code could not be computed from this nonce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CodeError {
    /// None of the 16 words is below 4,000,000,000. The hub draws another nonce before it
    /// sends `enrolled`. The chance is about one in 4 x 10^18.
    #[error("no word of the material gives a code without bias")]
    NoUsableWord,
    /// The derivation library refused to produce the material. It only refuses outputs above
    /// 8,160 bytes and this one has 64, so this never happens; the variant exists so that no
    /// path of this function can panic.
    #[error("the key derivation refused the output length")]
    Derivation,
}

/// The six digits the connector shows and the admin types. Not a secret.
#[derive(Clone, Copy)]
pub struct VerificationCode(u32);

impl VerificationCode {
    /// Computes the code of one enrollment.
    ///
    /// # Errors
    /// [`CodeError::NoUsableWord`] when the hub has to draw another nonce.
    pub fn derive(nonce: &EnrollNonce, handshake_hash: &HandshakeHash) -> Result<Self, CodeError> {
        let hkdf = Hkdf::<Sha256>::new(Some(nonce.as_bytes()), handshake_hash.as_bytes());
        let mut material = [0u8; MATERIAL_LEN];
        hkdf.expand(CODE_INFO, &mut material)
            .map_err(|_| CodeError::Derivation)?;
        Self::from_material(&material)
    }

    /// Reads a code typed by a person: six digits, with or without the hyphen in the middle.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = match text.as_bytes() {
            [a, b, c, b'-', d, e, f] | [a, b, c, d, e, f] => [*a, *b, *c, *d, *e, *f],
            _ => return None,
        };
        let mut value = 0u32;
        for digit in digits {
            if !digit.is_ascii_digit() {
                return None;
            }
            value = value * 10 + u32::from(digit - b'0');
        }
        Some(Self(value))
    }

    /// The code as a number, from 0 to 999,999.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Takes the code from the 64 bytes of material.
    fn from_material(material: &[u8; MATERIAL_LEN]) -> Result<Self, CodeError> {
        let (words, _) = material.as_chunks::<4>();
        words
            .iter()
            .map(|word| u32::from_be_bytes(*word))
            .find(|word| *word < WORD_LIMIT)
            .map(|word| Self(word % CODE_MODULUS))
            .ok_or(CodeError::NoUsableWord)
    }
}

impl PartialEq for VerificationCode {
    /// Compares in constant time. The code is not a secret, but the hub decides an approval on
    /// this comparison.
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for VerificationCode {}

impl fmt::Display for VerificationCode {
    /// Writes the code as `000-000`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:03}-{:03}", self.0 / 1_000, self.0 % 1_000)
    }
}

impl fmt::Debug for VerificationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VerificationCode({self})")
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_util::hex;

    // -----------------------------------------------------------------------------------------
    // A second implementation, written from the documents and from the published standards
    // (FIPS 180-4, RFC 2104 and RFC 5869) without any of the libraries the crate uses. Two
    // implementations that agree do not depend on the same library being right.
    // -----------------------------------------------------------------------------------------

    mod independent {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        pub(super) fn sha256(message: &[u8]) -> [u8; 32] {
            let mut state: [u32; 8] = [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ];
            let mut padded = message.to_vec();
            padded.push(0x80);
            while padded.len() % 64 != 56 {
                padded.push(0);
            }
            let bit_length = u64::try_from(message.len()).unwrap() * 8;
            padded.extend_from_slice(&bit_length.to_be_bytes());

            for block in padded.chunks(64) {
                let mut w = [0u32; 64];
                for (index, word) in block.chunks(4).enumerate() {
                    w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
                }
                for t in 16..64 {
                    let s0 =
                        w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
                    let s1 =
                        w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
                    w[t] = w[t - 16]
                        .wrapping_add(s0)
                        .wrapping_add(w[t - 7])
                        .wrapping_add(s1);
                }
                let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
                for t in 0..64 {
                    let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                    let choice = (e & f) ^ (!e & g);
                    let t1 = h
                        .wrapping_add(big_s1)
                        .wrapping_add(choice)
                        .wrapping_add(K[t])
                        .wrapping_add(w[t]);
                    let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                    let majority = (a & b) ^ (a & c) ^ (b & c);
                    let t2 = big_s0.wrapping_add(majority);
                    h = g;
                    g = f;
                    f = e;
                    e = d.wrapping_add(t1);
                    d = c;
                    c = b;
                    b = a;
                    a = t1.wrapping_add(t2);
                }
                for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
                    *slot = slot.wrapping_add(value);
                }
            }
            let mut digest = [0u8; 32];
            for (chunk, word) in digest.chunks_mut(4).zip(state) {
                chunk.copy_from_slice(&word.to_be_bytes());
            }
            digest
        }

        pub(super) fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
            let mut block = [0u8; 64];
            if key.len() > 64 {
                block[..32].copy_from_slice(&sha256(key));
            } else {
                block[..key.len()].copy_from_slice(key);
            }
            let mut inner: Vec<u8> = block.iter().map(|byte| byte ^ 0x36).collect();
            inner.extend_from_slice(message);
            let mut outer: Vec<u8> = block.iter().map(|byte| byte ^ 0x5c).collect();
            outer.extend_from_slice(&sha256(&inner));
            sha256(&outer)
        }

        pub(super) fn hkdf(salt: &[u8], key: &[u8], info: &[u8], length: usize) -> Vec<u8> {
            let pseudorandom_key = hmac(salt, key);
            let mut output = Vec::new();
            let mut previous: Vec<u8> = Vec::new();
            let mut counter = 1u8;
            while output.len() < length {
                let mut input = previous.clone();
                input.extend_from_slice(info);
                input.push(counter);
                previous = hmac(&pseudorandom_key, &input).to_vec();
                output.extend_from_slice(&previous);
                counter += 1;
            }
            output.truncate(length);
            output
        }

        /// The whole function of the document, down to the text the person reads.
        pub(super) fn code(nonce: &[u8; 32], handshake_hash: &[u8; 32]) -> Option<String> {
            let material = hkdf(nonce, handshake_hash, b"arauto/code/1", 64);
            for index in 0..16 {
                let mut word = 0u64;
                for byte in &material[index * 4..index * 4 + 4] {
                    word = word * 256 + u64::from(*byte);
                }
                if word < 4_000_000_000 {
                    let digits = format!("{:06}", word % 1_000_000);
                    return Some(format!("{}-{}", &digits[..3], &digits[3..]));
                }
            }
            None
        }
    }

    fn counting(from: u8) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (offset, byte) in bytes.iter_mut().enumerate() {
            *byte = from + u8::try_from(offset).unwrap();
        }
        bytes
    }

    fn code_of(nonce: [u8; 32], handshake_hash: [u8; 32]) -> String {
        VerificationCode::derive(
            &EnrollNonce::from_bytes(nonce),
            &HandshakeHash::from_bytes(handshake_hash),
        )
        .unwrap()
        .to_string()
    }

    #[test]
    fn the_second_implementation_matches_the_published_vectors() {
        // FIPS 180-4.
        assert_eq!(
            hex(&independent::sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&independent::sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&independent::sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // RFC 4231, test case 2.
        assert_eq!(
            hex(&independent::hmac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // RFC 5869, test case 1.
        let salt: Vec<u8> = (0x00u8..=0x0c).collect();
        let info: Vec<u8> = (0xf0u8..=0xf9).collect();
        assert_eq!(
            hex(&independent::hkdf(&salt, &[0x0b; 22], &info, 42)),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    /// The two vectors of `docs/14-protocolo-do-canal.md`, section 8. The gold values were also
    /// computed outside Rust, with the HMAC of another standard library.
    #[test]
    fn document_vectors() {
        // (a) nonce = 00 x 32, handshake hash = 00 x 32.
        assert_eq!(code_of([0u8; 32], [0u8; 32]), "028-213");
        assert_eq!(
            independent::code(&[0u8; 32], &[0u8; 32]).as_deref(),
            Some("028-213")
        );
        // (b) nonce = 00 01 .. 1f, handshake hash = 20 21 .. 3f.
        assert_eq!(code_of(counting(0x00), counting(0x20)), "935-415");
        assert_eq!(
            independent::code(&counting(0x00), &counting(0x20)).as_deref(),
            Some("935-415")
        );
    }

    #[test]
    fn the_material_is_the_documented_hkdf() {
        // The 64 bytes of vector (a), from the second implementation and from outside Rust.
        let expected = "8668fff5e28a98700473c594992fb85725eeb094d22d216fd615340e0bbc50c4\
                        6274ac06db0910a49a51d75554624a11ba8ee7180c5c2f81091a5609ef565b8a";
        assert_eq!(
            hex(&independent::hkdf(
                &[0u8; 32],
                &[0u8; 32],
                b"arauto/code/1",
                64
            )),
            expected
        );
        let mut material = [0u8; 64];
        Hkdf::<Sha256>::new(Some(&[0u8; 32]), &[0u8; 32])
            .expand(CODE_INFO, &mut material)
            .unwrap();
        assert_eq!(hex(&material), expected);
        assert_eq!(CODE_INFO, b"arauto/code/1");
    }

    #[test]
    fn nonce_and_hash_are_not_interchangeable() {
        // The nonce is the salt and the hash is the key: swapping them gives another code.
        let direct = code_of(counting(0x00), counting(0x20));
        let swapped = code_of(counting(0x20), counting(0x00));
        assert_ne!(direct, swapped);
        assert_eq!(
            Some(swapped),
            independent::code(&counting(0x20), &counting(0x00))
        );
    }

    #[test]
    fn every_bit_of_the_inputs_matters() {
        let base = code_of(counting(0x00), counting(0x20));
        let mut different = 0;
        for position in 0..32 {
            let mut nonce = counting(0x00);
            nonce[position] ^= 0x01;
            let mut hash = counting(0x20);
            hash[position] ^= 0x80;
            different += usize::from(code_of(nonce, counting(0x20)) != base);
            different += usize::from(code_of(counting(0x00), hash) != base);
        }
        // Two codes of six digits coincide once in a million: 64 changes all give another code.
        assert_eq!(different, 64);
    }

    fn material_of(words: [u32; 16]) -> [u8; 64] {
        let mut material = [0u8; 64];
        for (chunk, word) in material.chunks_mut(4).zip(words) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        material
    }

    #[test]
    fn the_first_word_below_the_limit_is_used() {
        let code = |words: [u32; 16]| VerificationCode::from_material(&material_of(words));
        assert_eq!(code([0; 16]).unwrap().value(), 0);
        assert_eq!(code([123_456_789; 16]).unwrap().value(), 456_789);
        // The largest word that serves, and the first that does not.
        let mut words = [7u32; 16];
        words[0] = 3_999_999_999;
        assert_eq!(code(words).unwrap().value(), 999_999);
        words[0] = 4_000_000_000;
        assert_eq!(
            code(words).unwrap().value(),
            7,
            "a word at the limit is discarded"
        );
        words[0] = u32::MAX;
        assert_eq!(code(words).unwrap().value(), 7);
        // Only the first one that serves counts; the others are discarded.
        let mut words = [u32::MAX; 16];
        words[5] = 1_000_042;
        words[6] = 2_000_099;
        assert_eq!(code(words).unwrap().value(), 42);
        // The last word still serves.
        let mut words = [4_000_000_000u32; 16];
        words[15] = 3_000_000_017;
        assert_eq!(code(words).unwrap().value(), 17);
    }

    #[test]
    fn the_words_are_read_most_significant_byte_first() {
        let mut material = [0xffu8; 64];
        material[..4].copy_from_slice(&[0x00, 0x00, 0x01, 0x02]);
        assert_eq!(
            VerificationCode::from_material(&material).unwrap().value(),
            258
        );
        // Read the other way round the same bytes would be 0x02010000, another code.
        assert_ne!(258, 0x0201_0000u32 % 1_000_000);
    }

    #[test]
    fn when_no_word_serves_the_hub_must_draw_again() {
        assert_eq!(
            VerificationCode::from_material(&[0xff; 64]),
            Err(CodeError::NoUsableWord)
        );
        assert_eq!(
            VerificationCode::from_material(&material_of([4_000_000_000; 16])),
            Err(CodeError::NoUsableWord)
        );
        let mut words = [u32::MAX; 16];
        words[15] = 4_000_000_001;
        assert_eq!(
            VerificationCode::from_material(&material_of(words)),
            Err(CodeError::NoUsableWord)
        );
        assert!(!CodeError::NoUsableWord.to_string().is_empty());
        assert_ne!(CodeError::Derivation, CodeError::NoUsableWord);
        assert!(!CodeError::Derivation.to_string().is_empty());
    }

    #[test]
    fn the_code_is_written_as_three_digits_hyphen_three_digits() {
        let text = |value: u32| VerificationCode(value).to_string();
        assert_eq!(text(0), "000-000");
        assert_eq!(text(7), "000-007");
        assert_eq!(text(1_002), "001-002");
        assert_eq!(text(28_213), "028-213");
        assert_eq!(text(418_203), "418-203");
        assert_eq!(text(999_999), "999-999");
        assert_eq!(
            format!("{:?}", VerificationCode(1_002)),
            "VerificationCode(001-002)"
        );
        for value in [0u32, 1, 999, 1_000, 99_999, 100_000, 999_999] {
            let shown = text(value);
            assert_eq!(shown.len(), 7);
            assert_eq!(shown.as_bytes()[3], b'-');
            assert!(
                shown
                    .bytes()
                    .enumerate()
                    .all(|(i, b)| i == 3 || b.is_ascii_digit())
            );
        }
    }

    #[test]
    fn a_typed_code_is_read_with_or_without_the_hyphen() {
        assert_eq!(VerificationCode::parse("418-203").unwrap().value(), 418_203);
        assert_eq!(VerificationCode::parse("418203").unwrap().value(), 418_203);
        assert_eq!(VerificationCode::parse("000-000").unwrap().value(), 0);
        assert_eq!(VerificationCode::parse("000007").unwrap().value(), 7);
        for value in [0u32, 7, 1_002, 418_203, 999_999] {
            let code = VerificationCode(value);
            assert_eq!(VerificationCode::parse(&code.to_string()), Some(code));
        }
        for text in [
            "",
            "418",
            "41820",
            "4182033",
            "418-2033",
            "4182-03",
            "418 203",
            "418_203",
            "418-20a",
            " 418-203",
            "418-203 ",
            "418--203",
            "-418203",
            "418203-",
            "41-8203",
            "+18-203",
            "\u{664}18-203",
            "418-203\n",
            "4.8-203",
        ] {
            assert_eq!(VerificationCode::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn codes_compare_by_value() {
        assert_eq!(VerificationCode(5), VerificationCode(5));
        assert_ne!(VerificationCode(5), VerificationCode(6));
        assert_ne!(VerificationCode(0), VerificationCode(999_999));
    }

    #[test]
    fn the_nonce_is_drawn_and_travels_as_base64() {
        let a = EnrollNonce::random().unwrap();
        let b = EnrollNonce::random().unwrap();
        assert_ne!(a, b);
        assert_ne!(a.as_bytes(), &[0u8; 32]);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json.len(), 46, "44 characters of base64 and two quotes");
        assert_eq!(serde_json::from_str::<EnrollNonce>(&json).unwrap(), a);
        assert_eq!(EnrollNonce::from_bytes(*a.as_bytes()), a);
        // 31 and 33 bytes are not a nonce.
        for len in [0usize, 16, 31, 33, 64] {
            let json = format!("\"{}\"", crate::base64::encode(&vec![1u8; len]));
            assert!(serde_json::from_str::<EnrollNonce>(&json).is_err(), "{len}");
        }
        assert_eq!(
            format!("{:?}", EnrollNonce::from_bytes([0xa1; 32])),
            format!("EnrollNonce({})", "a1".repeat(32))
        );
    }

    proptest! {
        #![proptest_config(crate::test_util::random_rounds(256))]

        /// The library path and the second implementation agree on any input.
        #[test]
        fn both_implementations_agree(nonce in any::<[u8; 32]>(), hash in any::<[u8; 32]>()) {
            let ours = VerificationCode::derive(
                &EnrollNonce::from_bytes(nonce),
                &HandshakeHash::from_bytes(hash),
            )
            .ok()
            .map(|code| code.to_string());
            prop_assert_eq!(ours, independent::code(&nonce, &hash));
        }
    }
}
