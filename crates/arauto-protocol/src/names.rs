//! Names and texts that are checked before they go anywhere.
//!
//! The name of a Hermes and the alias of a number are the only texts given by the admin that
//! reach an LLM, inside the fixed phrase of a notice. That is why their format is narrow, and
//! why the check is compiled into both programs: even a hub that was broken into can only put
//! in the phrase a name in this format (`docs/13-conversa-entre-hermes.md`, section 2, and
//! `docs/14-protocolo-do-canal.md`, section 5.3).

use core::fmt;
use core::str::FromStr;

use crate::limits::{CHAT_ID_MAX_LEN, NAME_MAX_LEN, PEER_TEXT_MAX_BYTES};

/// Names nobody can take: an LLM could read them as a role or as the voice of the system.
pub const RESERVED_NAMES: [&str; 10] = [
    "system",
    "operator",
    "admin",
    "arauto",
    "user",
    "assistant",
    "hub",
    "hermes",
    "developer",
    "tool",
];

/// Why a text is not a valid name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// Empty, or longer than 32 characters.
    #[error("a name has from 1 to 32 characters")]
    Length,
    /// The first character is not a lowercase letter or a digit.
    #[error("a name starts with a lowercase letter or a digit")]
    FirstCharacter,
    /// A character other than lowercase letters, digits, hyphen and underscore.
    #[error("a name has only lowercase letters, digits, hyphen and underscore")]
    Character,
    /// One of [`RESERVED_NAMES`].
    #[error("this name is reserved")]
    Reserved,
}

/// Checks `^[a-z0-9][a-z0-9_-]{0,31}$` and the reserved names.
fn check_name(text: &str) -> Result<(), NameError> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.len() > NAME_MAX_LEN {
        return Err(NameError::Length);
    }
    let starts_well = bytes
        .first()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    if !starts_well {
        return Err(NameError::FirstCharacter);
    }
    let allowed = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-';
    if !bytes.iter().all(allowed) {
        return Err(NameError::Character);
    }
    if RESERVED_NAMES.contains(&text) {
        return Err(NameError::Reserved);
    }
    Ok(())
}

macro_rules! name_type {
    ($(#[$meta:meta])* $name:ident, $expecting:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Checks the format and the reserved names.
            ///
            /// # Errors
            /// [`NameError`] says which rule the text breaks.
            pub fn parse(text: &str) -> Result<Self, NameError> {
                check_name(text)?;
                Ok(Self(text.to_owned()))
            }

            /// The name.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = NameError;

            fn from_str(text: &str) -> Result<Self, NameError> {
                Self::parse(text)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                crate::serde_util::text(deserializer, $expecting, |text| Self::parse(text).ok())
            }
        }
    };
}

name_type!(
    /// The name of a Hermes: `^[a-z0-9][a-z0-9_-]{0,31}$`, and not a reserved name.
    HermesName,
    "Hermes name"
);

name_type!(
    /// The alias the admin gives to a number. Same format and same reserved names as
    /// [`HermesName`].
    NumberAlias,
    "number alias"
);

/// The text is not a chat identifier: from 1 to 64 characters of `A-Z a-z 0-9 _ -`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a chat identifier has from 1 to 64 characters of A-Z a-z 0-9 _ -")]
pub struct ChatIdError;

/// The opaque identifier of a chat, as it goes in a notice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChatId(String);

impl ChatId {
    /// Checks the length and the characters.
    ///
    /// # Errors
    /// [`ChatIdError`] when the text is empty, too long or has another character.
    pub fn parse(text: &str) -> Result<Self, ChatIdError> {
        let bytes = text.as_bytes();
        let allowed = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-';
        if bytes.is_empty() || bytes.len() > CHAT_ID_MAX_LEN || !bytes.iter().all(allowed) {
            return Err(ChatIdError);
        }
        Ok(Self(text.to_owned()))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ChatId {
    type Err = ChatIdError;

    fn from_str(text: &str) -> Result<Self, ChatIdError> {
        Self::parse(text)
    }
}

impl fmt::Display for ChatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ChatId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for ChatId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for ChatId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::serde_util::text(deserializer, "chat identifier", |text| {
            Self::parse(text).ok()
        })
    }
}

/// Whether `c` reorders or hides the text around it: the direction embeddings, overrides and
/// isolates (U+202A to U+202E and U+2066 to U+2069) and the tag characters (U+E0000 to
/// U+E007F).
///
/// The first kind makes a text show in another order than it was written; the second is
/// invisible and can carry a whole instruction for an LLM inside a text that looks innocent.
/// Neither is needed to write in any language: text that runs from right to left takes its
/// direction from its own letters.
#[must_use]
pub const fn is_deceptive(c: char) -> bool {
    matches!(
        c,
        '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{e0000}'..='\u{e007f}'
    )
}

/// Whether `c` shows nothing: the code points Unicode lists as `Default_Ignorable_Code_Point`
/// (soft hyphen, zero-width space and joiners, direction marks, fillers, variation selectors,
/// the byte-order mark, tags).
const fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{34f}'
            | '\u{61c}'
            | '\u{115f}'..='\u{1160}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180b}'..='\u{180f}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{3164}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{feff}'
            | '\u{ffa0}'
            | '\u{fff0}'..='\u{fff8}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0000}'..='\u{e0fff}'
    )
}

/// Why a text cannot be a message between Hermes agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    /// No bytes at all.
    #[error("the text is empty")]
    Empty,
    /// More than 16,384 bytes in UTF-8.
    #[error("the text has more than 16384 bytes")]
    TooLong,
    /// A control character other than a line break and a tab. A carriage return only counts
    /// as a line break right before a line feed.
    #[error("the text has a control character")]
    ControlCharacter,
    /// A character that reorders or hides text ([`is_deceptive`]).
    #[error("the text has a direction or tag character")]
    DeceptiveCharacter,
    /// Nothing but white space and characters that show nothing.
    #[error("the text is only white space")]
    OnlyWhitespace,
}

/// The text of a message from one Hermes to another.
///
/// From 1 to 16,384 bytes of UTF-8, and:
///
/// - no control character other than a line break and a tab. A line break is a line feed, or
///   a carriage return right before a line feed: a carriage return on its own makes a terminal
///   write over the line it is on;
/// - no character that reorders or hides text ([`is_deceptive`]);
/// - not made only of white space and of characters that show nothing.
///
/// It is content written by an agent: it has no `Display` and its `Debug` shows only the size,
/// so it does not end up in a log.
#[derive(Clone, PartialEq, Eq)]
pub struct MessageText(String);

impl MessageText {
    /// Checks the rules and keeps the text.
    ///
    /// # Errors
    /// [`TextError`] says which rule the text breaks.
    pub fn new(text: String) -> Result<Self, TextError> {
        if text.is_empty() {
            return Err(TextError::Empty);
        }
        if text.len() > PEER_TEXT_MAX_BYTES {
            return Err(TextError::TooLong);
        }
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let line_break_or_tab = match c {
                '\n' | '\t' => true,
                '\r' => chars.peek() == Some(&'\n'),
                _ => false,
            };
            if c.is_control() && !line_break_or_tab {
                return Err(TextError::ControlCharacter);
            }
            if is_deceptive(c) {
                return Err(TextError::DeceptiveCharacter);
            }
        }
        if text.chars().all(|c| c.is_whitespace() || is_invisible(c)) {
            return Err(TextError::OnlyWhitespace);
        }
        Ok(Self(text))
    }

    /// Checks the rules and copies the text.
    ///
    /// # Errors
    /// [`TextError`] says which rule the text breaks.
    pub fn parse(text: &str) -> Result<Self, TextError> {
        // The size is checked before the copy.
        if text.len() > PEER_TEXT_MAX_BYTES {
            return Err(TextError::TooLong);
        }
        Self::new(text.to_owned())
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The text, taken out.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for MessageText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MessageText({} bytes)", self.0.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_in_the_format_are_accepted() {
        let thirty_two = "a".repeat(32);
        for text in [
            "a",
            "0",
            "ana",
            "bento-2",
            "a_b-c",
            "9lives",
            "x-",
            "z_",
            &thirty_two,
        ] {
            assert_eq!(HermesName::parse(text).unwrap().as_str(), text);
            assert_eq!(NumberAlias::parse(text).unwrap().as_str(), text);
        }
    }

    #[test]
    fn names_out_of_the_format_are_refused() {
        let thirty_three = "a".repeat(33);
        let cases: [(&str, NameError); 21] = [
            ("", NameError::Length),
            (&thirty_three, NameError::Length),
            ("-ana", NameError::FirstCharacter),
            ("_ana", NameError::FirstCharacter),
            ("Ana", NameError::FirstCharacter),
            (" ana", NameError::FirstCharacter),
            ("\u{e1}na", NameError::FirstCharacter),
            ("aNa", NameError::Character),
            ("ANA", NameError::FirstCharacter),
            ("ana ", NameError::Character),
            ("ana\n", NameError::Character),
            ("an a", NameError::Character),
            ("ana.b", NameError::Character),
            ("ana/b", NameError::Character),
            ("ana\"", NameError::Character),
            ("ana\u{e7}", NameError::Character),
            ("ana\0", NameError::Character),
            ("ana=", NameError::Character),
            ("a\u{200b}b", NameError::Character),
            ("a\u{0430}", NameError::Character),
            ("a\tb", NameError::Character),
        ];
        for (text, expected) in cases {
            assert_eq!(HermesName::parse(text), Err(expected), "{text:?}");
            assert_eq!(NumberAlias::parse(text), Err(expected), "{text:?}");
        }
    }

    #[test]
    fn reserved_names_are_refused_for_hermes_and_for_numbers() {
        assert_eq!(RESERVED_NAMES.len(), 10);
        for reserved in RESERVED_NAMES {
            assert_eq!(
                HermesName::parse(reserved),
                Err(NameError::Reserved),
                "{reserved}"
            );
            assert_eq!(
                NumberAlias::parse(reserved),
                Err(NameError::Reserved),
                "{reserved}"
            );
            // Only the exact word is reserved.
            let longer = format!("{reserved}-2");
            assert!(HermesName::parse(&longer).is_ok(), "{longer}");
        }
        for listed in [
            "system",
            "operator",
            "admin",
            "arauto",
            "user",
            "assistant",
            "hub",
            "hermes",
            "developer",
            "tool",
        ] {
            assert!(RESERVED_NAMES.contains(&listed), "{listed} left the list");
        }
    }

    #[test]
    fn names_go_through_serde_checked() {
        let name: HermesName = serde_json::from_str("\"ana\"").unwrap();
        assert_eq!(name.as_str(), "ana");
        assert_eq!(serde_json::to_string(&name).unwrap(), "\"ana\"");
        assert_eq!(name.to_string(), "ana");
        assert_eq!("ana".parse::<HermesName>().unwrap(), name);
        for bad in [
            "\"Ana\"",
            "\"system\"",
            "\"\"",
            "\"a b\"",
            "5",
            "null",
            "[\"ana\"]",
        ] {
            assert!(serde_json::from_str::<HermesName>(bad).is_err(), "{bad}");
            assert!(serde_json::from_str::<NumberAlias>(bad).is_err(), "{bad}");
        }
        let alias: NumberAlias = serde_json::from_str("\"balcao\"").unwrap();
        assert_eq!(serde_json::to_string(&alias).unwrap(), "\"balcao\"");
    }

    #[test]
    fn serde_errors_do_not_repeat_the_refused_text() {
        let hostile = "\"EVIL\\u001b[31m-name\"";
        let error = serde_json::from_str::<HermesName>(hostile)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("EVIL"), "{error}");
        let error = serde_json::from_str::<ChatId>("\"bad chat!\"")
            .unwrap_err()
            .to_string();
        assert!(!error.contains("bad chat"), "{error}");
    }

    #[test]
    fn chat_identifiers_accepted_and_refused() {
        let sixty_four = "Z".repeat(64);
        for text in [
            "a",
            "A",
            "0",
            "15555550100-1600000000",
            "abc_DEF-123",
            &sixty_four,
        ] {
            assert_eq!(ChatId::parse(text).unwrap().as_str(), text);
        }
        let sixty_five = "Z".repeat(65);
        for text in [
            "",
            &sixty_five,
            "a b",
            "a@c.us",
            "a.b",
            "a\n",
            "a/b",
            "a\"b",
            "\u{e1}",
            "a\0",
            "a+b",
            "a=",
        ] {
            assert_eq!(ChatId::parse(text), Err(ChatIdError), "{text:?}");
        }
        let id: ChatId = serde_json::from_str("\"abc_DEF-123\"").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"abc_DEF-123\"");
        assert_eq!(id.to_string(), "abc_DEF-123");
        assert!(serde_json::from_str::<ChatId>("\"a b\"").is_err());
    }

    #[test]
    fn message_text_accepts_ordinary_text() {
        let longest = "a".repeat(PEER_TEXT_MAX_BYTES);
        for text in [
            "a",
            "ol\u{e1}, tudo bem?",
            "line one\nline two\r\n\tindented",
            " x ",
            &longest,
        ] {
            assert_eq!(MessageText::parse(text).unwrap().as_str(), text);
            assert_eq!(
                MessageText::new(text.to_owned()).unwrap().into_string(),
                text
            );
        }
    }

    #[test]
    fn message_text_limits() {
        assert_eq!(MessageText::parse(""), Err(TextError::Empty));
        let too_long = "a".repeat(PEER_TEXT_MAX_BYTES + 1);
        assert_eq!(MessageText::parse(&too_long), Err(TextError::TooLong));
        assert_eq!(MessageText::new(too_long), Err(TextError::TooLong));
        // The limit counts bytes, not characters: 8,192 two-byte characters fit, one more
        // does not.
        let two_bytes = "\u{e9}".repeat(PEER_TEXT_MAX_BYTES / 2);
        assert!(MessageText::parse(&two_bytes).is_ok());
        let over = format!("{two_bytes}\u{e9}");
        assert_eq!(MessageText::parse(&over), Err(TextError::TooLong));
    }

    #[test]
    fn message_text_refuses_control_characters() {
        for control in [
            '\0', '\u{1}', '\u{7}', '\u{8}', '\u{b}', '\u{c}', '\u{1b}', '\u{7f}', '\u{85}',
            '\u{9f}',
        ] {
            let text = format!("before{control}after");
            assert_eq!(
                MessageText::parse(&text),
                Err(TextError::ControlCharacter),
                "{control:?}"
            );
        }
        for allowed in ["\n", "\t", "\r\n", "\r\n\r\n", "\n\n"] {
            let text = format!("before{allowed}after");
            assert!(MessageText::parse(&text).is_ok(), "{allowed:?}");
        }
    }

    /// A carriage return on its own makes a terminal write over the line it is on: it is only
    /// a line break right before a line feed.
    #[test]
    fn message_text_refuses_a_carriage_return_on_its_own() {
        for text in [
            "a\rb",
            "paid\rfake",
            "a\r",
            "\ra",
            "a\r\rb",
            "a\r\r\nb",
            "a\n\rb",
            "a\r\tb",
            "a\r b",
        ] {
            assert_eq!(
                MessageText::parse(text),
                Err(TextError::ControlCharacter),
                "{text:?}"
            );
            assert_eq!(
                MessageText::new(text.to_owned()),
                Err(TextError::ControlCharacter),
                "{text:?}"
            );
        }
        assert!(MessageText::parse("a\r\nb").is_ok());
        assert!(MessageText::parse("a\r\n").is_ok());
    }

    #[test]
    fn message_text_refuses_direction_and_tag_characters() {
        let deceptive = [
            // Embeddings and overrides.
            '\u{202a}',
            '\u{202b}',
            '\u{202c}',
            '\u{202d}',
            '\u{202e}',
            // Isolates.
            '\u{2066}',
            '\u{2067}',
            '\u{2068}',
            '\u{2069}',
            // Tags: the first, the language tag, a tag letter and the last.
            '\u{e0000}',
            '\u{e0001}',
            '\u{e0041}',
            '\u{e007f}',
        ];
        for c in deceptive {
            assert!(is_deceptive(c), "{c:?}");
            for text in [
                format!("{c}abc"),
                format!("abc{c}"),
                format!("ab{c}cd"),
                format!("{c}"),
            ] {
                assert_eq!(
                    MessageText::parse(&text),
                    Err(TextError::DeceptiveCharacter),
                    "{text:?}"
                );
            }
        }
        // The neighbours of each range, and what a text in any language needs, stay.
        for c in [
            '\u{2029}',
            '\u{202f}',
            '\u{2065}',
            '\u{206a}',
            '\u{dffff}',
            '\u{e0080}',
            // The direction marks and the joiners: Arabic, Hebrew, Persian and emoji use them.
            '\u{200e}',
            '\u{200f}',
            '\u{200c}',
            '\u{200d}',
            '\u{fe0f}',
        ] {
            assert!(!is_deceptive(c), "{c:?}");
            let text = format!("a{c}b");
            assert!(MessageText::parse(&text).is_ok(), "{text:?}");
        }
        assert!(MessageText::parse("\u{5e9}\u{5dc}\u{5d5}\u{5dd} abc").is_ok());
        assert!(MessageText::parse("\u{645}\u{631}\u{62d}\u{628}\u{627}").is_ok());
    }

    #[test]
    fn message_text_refuses_only_invisible_characters() {
        for text in [
            "\u{200b}",
            "\u{feff}",
            "\u{ad}",
            "\u{34f}",
            "\u{61c}",
            "\u{115f}\u{1160}",
            "\u{17b4}\u{17b5}",
            "\u{180b}\u{180e}\u{180f}",
            "\u{200b}\u{200c}\u{200d}\u{200e}\u{200f}",
            "\u{2060}\u{2061}\u{2064}\u{2065}\u{206a}\u{206f}",
            "\u{3164}",
            "\u{fe00}\u{fe0f}",
            "\u{ffa0}",
            "\u{fff0}\u{fff8}",
            "\u{1bca0}\u{1bca3}",
            "\u{1d173}\u{1d17a}",
            "\u{e0080}\u{e0100}\u{e01ef}\u{e0fff}",
            // Mixed with white space it is still nothing to read.
            " \u{200b}\n\u{feff}\t",
            "\u{a0}\u{ad}",
        ] {
            assert_eq!(
                MessageText::parse(text),
                Err(TextError::OnlyWhitespace),
                "{text:?}"
            );
        }
        // One character that shows is enough, and the invisible ones around it stay: a joiner
        // or a variation selector is part of ordinary text.
        for text in [
            "\u{200b}a",
            "a\u{feff}",
            "\u{2764}\u{fe0f}",
            "\u{1f468}\u{200d}\u{1f469}",
        ] {
            assert!(MessageText::parse(text).is_ok(), "{text:?}");
        }
        // The neighbours of the ranges show something.
        for c in [
            '\u{ac}',
            '\u{ae}',
            '\u{1161}',
            '\u{2010}',
            '\u{2070}',
            '\u{3165}',
            '\u{fff9}',
            '\u{1bca4}',
            '\u{e1000}',
        ] {
            assert!(MessageText::parse(&c.to_string()).is_ok(), "{c:?}");
        }
    }

    #[test]
    fn message_text_refuses_only_white_space() {
        for text in [
            " ",
            "   ",
            "\n",
            "\r\n",
            "\t",
            " \n\t ",
            "\u{a0}",
            "\u{2003}\u{3000}",
        ] {
            assert_eq!(
                MessageText::parse(text),
                Err(TextError::OnlyWhitespace),
                "{text:?}"
            );
        }
        assert!(MessageText::parse(" . ").is_ok());
    }

    #[test]
    fn message_text_does_not_print_itself() {
        let text = MessageText::parse("CANARY message content").unwrap();
        assert_eq!(format!("{text:?}"), "MessageText(22 bytes)");
        assert_not_impl!(MessageText: core::fmt::Display, serde::Serialize);
    }
}
