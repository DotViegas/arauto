//! Small serde helpers for the field types.
//!
//! The error texts built here never repeat the value that was refused: it comes from the other
//! side and may be hostile or secret.

use core::fmt;
use core::marker::PhantomData;

use serde::de::{self, Deserializer, Visitor};

struct TextVisitor<T, F> {
    expecting: &'static str,
    parse: F,
    value: PhantomData<T>,
}

impl<T, F: FnOnce(&str) -> Option<T>> Visitor<'_> for TextVisitor<T, F> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.expecting)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<T, E> {
        let expecting = self.expecting;
        (self.parse)(text).ok_or_else(|| E::custom(format_args!("invalid {expecting}")))
    }
}

/// Reads a string and hands it to `parse`. `None` becomes an error that names only `expecting`.
pub(crate) fn text<'de, D, T, F>(
    deserializer: D,
    expecting: &'static str,
    parse: F,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    F: FnOnce(&str) -> Option<T>,
{
    deserializer.deserialize_str(TextVisitor {
        expecting,
        parse,
        value: PhantomData,
    })
}

struct IntegerVisitor {
    expecting: &'static str,
    min: u64,
    max: u64,
}

impl Visitor<'_> for IntegerVisitor {
    type Value = u64;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.expecting)
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<u64, E> {
        if (self.min..=self.max).contains(&value) {
            Ok(value)
        } else {
            Err(E::custom(format_args!("{} out of range", self.expecting)))
        }
    }
}

/// Reads an integer in `min..=max`. A fraction, a negative number or a number written as text
/// is refused.
pub(crate) fn integer<'de, D>(
    deserializer: D,
    expecting: &'static str,
    min: u64,
    max: u64,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_u64(IntegerVisitor {
        expecting,
        min,
        max,
    })
}

/// Reads a UUID written in the usual form: 36 characters, with hyphens. The other forms the
/// library accepts (without hyphens, in braces, as a URN) are refused, so each identifier has
/// one text apart from the case of its letters.
pub(crate) fn uuid_text(text: &str) -> Option<uuid::Uuid> {
    if text.len() != 36 {
        return None;
    }
    uuid::Uuid::try_parse(text).ok()
}

/// Defines an enumeration whose values travel as one fixed text each, and are read only from
/// that text.
///
/// The reader `serde` derives for an enumeration also takes the object form
/// (`{"channel": null}` for `"channel"`): a second way to write the same message, which would
/// become part of the frozen format the day it shipped.
macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident, $expecting:literal {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $text:literal,
            )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        impl $name {
            /// The text that travels.
            #[must_use]
            $vis const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text,)+
                }
            }

            /// The value of a text. `None` for any other text.
            #[must_use]
            $vis fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $crate::serde_util::text(deserializer, $expecting, Self::parse)
            }
        }
    };
}

/// Defines an integer type that only exists inside a range, with serde.
macro_rules! bounded_integer {
    (
        $(#[$meta:meta])*
        $name:ident($inner:ty), $min:expr, $max:expr, $expecting:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($inner);

        impl $name {
            /// The smallest value.
            pub const MIN: $name = $name($min);
            /// The largest value.
            pub const MAX: $name = $name($max);

            /// Returns `None` when `value` is outside the range.
            #[must_use]
            pub fn new(value: $inner) -> Option<Self> {
                (Self::MIN.0..=Self::MAX.0).contains(&value).then_some(Self(value))
            }

            /// The number.
            #[must_use]
            pub const fn get(self) -> $inner {
                self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Display::fmt(&self.0, f)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serde::Serialize::serialize(&self.0, serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = $crate::serde_util::integer(
                    deserializer,
                    $expecting,
                    u64::from(Self::MIN.0),
                    u64::from(Self::MAX.0),
                )?;
                <$inner>::try_from(value)
                    .ok()
                    .and_then(Self::new)
                    .ok_or_else(|| serde::de::Error::custom(concat!($expecting, " out of range")))
            }
        }
    };
}
