//! Standard base64 with padding, read strictly.
//!
//! Binary fields of the messages travel in this form. Only the canonical text of a value is
//! accepted: one text for each value, so a message is never read in two ways.

use data_encoding::BASE64;
use zeroize::Zeroize;

/// The text is not the canonical base64 of a value within the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Base64Error;

/// The name under which the tests count the texts handed to the decoder.
#[cfg(test)]
const DECODE_WORK: &str = "base64 text decoded";

/// Writes `bytes` as base64.
pub(crate) fn encode(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}

/// Reads at most `max_len` bytes from `text`. Intermediate copies are wiped: the value may be
/// a secret.
pub(crate) fn decode(text: &str, max_len: usize) -> Result<Vec<u8>, Base64Error> {
    // The limit is checked on the text, before anything is decoded or allocated.
    if text.len() > BASE64.encode_len(max_len) {
        return Err(Base64Error);
    }
    #[cfg(test)]
    crate::test_util::work_probe::record(DECODE_WORK);
    let mut bytes = BASE64.decode(text.as_bytes()).map_err(|_| Base64Error)?;
    // The library also accepts padded blocks glued together ("QQ==QQ=="). Writing the value
    // again and comparing leaves exactly one accepted text for each value.
    let mut canonical = BASE64.encode(&bytes);
    let is_canonical = canonical == text;
    canonical.zeroize();
    if !is_canonical || bytes.len() > max_len {
        bytes.zeroize();
        return Err(Base64Error);
    }
    Ok(bytes)
}

/// Reads exactly `N` bytes from `text`.
pub(crate) fn decode_array<const N: usize>(text: &str) -> Result<[u8; N], Base64Error> {
    if text.len() != BASE64.encode_len(N) {
        return Err(Base64Error);
    }
    let mut bytes = decode(text, N)?;
    let array = <[u8; N]>::try_from(bytes.as_slice()).map_err(|_| Base64Error);
    bytes.zeroize();
    array
}

/// Implements serde for a tuple struct around `[u8; N]`, as strict base64 text.
macro_rules! impl_base64_serde {
    ($type:ident, $len:expr, $expecting:literal) => {
        impl serde::Serialize for $type {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&$crate::base64::encode(&self.0))
            }
        }

        impl<'de> serde::Deserialize<'de> for $type {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $crate::serde_util::text(deserializer, $expecting, |text| {
                    $crate::base64::decode_array::<{ $len }>(text)
                        .ok()
                        .map($type)
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_text_goes_both_ways() {
        for len in 0..70usize {
            let bytes: Vec<u8> = (0..len)
                .map(|i| u8::try_from(i * 7 % 256).unwrap())
                .collect();
            let text = encode(&bytes);
            assert_eq!(decode(&text, len).unwrap(), bytes);
            assert_eq!(decode(&text, 1_024).unwrap(), bytes);
        }
        assert_eq!(encode(b"A"), "QQ==");
        assert_eq!(encode(b"AB"), "QUI=");
        assert_eq!(encode(b"ABC"), "QUJD");
        assert_eq!(encode(&[0xfb, 0xff]), "+/8=");
    }

    #[test]
    fn padding_is_required() {
        assert_eq!(decode("QQ==", 8), Ok(b"A".to_vec()));
        assert_eq!(decode("QQ", 8), Err(Base64Error));
        assert_eq!(decode("QQ=", 8), Err(Base64Error));
        assert_eq!(decode("QUI", 8), Err(Base64Error));
        assert_eq!(decode("QQ===", 8), Err(Base64Error));
        assert_eq!(decode("QQ======", 8), Err(Base64Error));
        assert_eq!(decode("=", 8), Err(Base64Error));
        assert_eq!(decode("====", 8), Err(Base64Error));
    }

    #[test]
    fn only_the_standard_alphabet_is_accepted() {
        assert_eq!(decode("+/8=", 8), Ok(vec![0xfb, 0xff]));
        assert_eq!(decode("-_8=", 8), Err(Base64Error), "URL alphabet");
        assert_eq!(decode("QQ==\n", 8), Err(Base64Error));
        assert_eq!(decode(" QQ==", 8), Err(Base64Error));
        assert_eq!(decode("QU JD", 8), Err(Base64Error));
        assert_eq!(decode("QUJD\r\n", 8), Err(Base64Error));
        assert_eq!(decode("QUJ\u{e9}", 8), Err(Base64Error));
    }

    #[test]
    fn non_canonical_texts_are_refused() {
        // Glued blocks: the library alone would read this as "AA".
        assert_eq!(
            data_encoding::BASE64.decode(b"QQ==QQ=="),
            Ok(b"AA".to_vec())
        );
        assert_eq!(decode("QQ==QQ==", 8), Err(Base64Error));
        assert_eq!(decode("QUI=QUJD", 8), Err(Base64Error));
        // Bits left over after the last byte must be zero.
        assert_eq!(decode("QR==", 8), Err(Base64Error));
        assert_eq!(decode("QUJ=", 8), Err(Base64Error));
    }

    #[test]
    fn the_limit_is_checked_on_the_text_and_on_the_value() {
        let text = encode(&[7u8; 10]);
        assert!(decode(&text, 10).is_ok());
        assert_eq!(decode(&text, 9), Err(Base64Error));
        // Nine bytes and eight bytes give texts of the same length: only the decoded length
        // can tell them apart.
        assert_eq!(encode(&[7u8; 9]).len(), encode(&[7u8; 8]).len());
        assert_eq!(decode(&encode(&[7u8; 9]), 8), Err(Base64Error));
        assert_eq!(decode(&encode(&[7u8; 8]), 7), Err(Base64Error));
        assert_eq!(decode("", 0), Ok(Vec::new()));
    }

    /// The other side picks the size of the text. One over the limit is refused by its length
    /// alone: decoding it first would give the same answer, after work and memory in the
    /// proportion the other side chose.
    #[test]
    fn a_text_over_the_limit_is_refused_before_anything_is_decoded() {
        use crate::test_util::work_probe;

        let _ = work_probe::take(DECODE_WORK);
        // The control: a text within the limit does reach the decoder.
        assert_eq!(decode("QUJD", 3), Ok(b"ABC".to_vec()));
        assert_eq!(work_probe::take(DECODE_WORK), 1);

        // A megabyte of valid base64 against the limit of a credential.
        let huge = encode(&vec![0x5au8; 1 << 20]);
        assert_eq!(decode(&huge, 1_024), Err(Base64Error));
        assert_eq!(
            work_probe::take(DECODE_WORK),
            0,
            "a text over the limit was decoded"
        );
        // The shortest text that is over: 1,027 bytes take four characters more than 1,024.
        let over = encode(&[0x5au8; 1_027]);
        assert_eq!(over.len(), encode(&[0x5au8; 1_024]).len() + 4);
        assert_eq!(decode(&over, 1_024), Err(Base64Error));
        assert_eq!(work_probe::take(DECODE_WORK), 0);
        assert_eq!(decode_array::<16>(&huge), Err(Base64Error));
        assert_eq!(work_probe::take(DECODE_WORK), 0);

        // A text of the length of the limit has to be decoded to be judged: 1,025 bytes are
        // written in as many characters as 1,024.
        let same_length = encode(&[0x5au8; 1_025]);
        assert_eq!(same_length.len(), encode(&[0x5au8; 1_024]).len());
        assert_eq!(decode(&same_length, 1_024), Err(Base64Error));
        assert_eq!(work_probe::take(DECODE_WORK), 1);
    }

    #[test]
    fn arrays_want_the_exact_size() {
        let sixteen = encode(&[9u8; 16]);
        assert_eq!(decode_array::<16>(&sixteen), Ok([9u8; 16]));
        assert_eq!(decode_array::<15>(&sixteen), Err(Base64Error));
        assert_eq!(decode_array::<17>(&sixteen), Err(Base64Error));
        assert_eq!(decode_array::<32>(&sixteen), Err(Base64Error));
        assert_eq!(decode_array::<16>(&encode(&[9u8; 17])), Err(Base64Error));
        assert_eq!(decode_array::<16>(""), Err(Base64Error));
        assert_eq!(decode_array::<0>(""), Ok([]));
    }
}
