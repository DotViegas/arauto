//! JSON-RPC 2.0 over lines, as the server of tools speaks it: one message per line, and no line
//! inside a message (the stdio transport of MCP).
//!
//! | What comes | What it is |
//! |---|---|
//! | An object with `"id"` (a text or an integer) and `"method"` | a request: it always gets an answer |
//! | An object with `"method"` and without `"id"` | a notification: it **never** gets an answer, even when it is out of its form |
//! | An object with `"id"` and `"result"` or `"error"` | an answer of the client: this server asks nothing, so it is dropped |
//! | Anything else | `-32600`, with the identifier when it could be read |
//! | What is not JSON by the rules of a message of the channel | `-32700`, with `null` as the identifier |
//!
//! What comes in is hostile. It is read with the rules of a message of the channel (no repeated
//! key, a ceiling of values and of nesting); a line longer than the ceiling is read to its end
//! and dropped, never kept. No error repeats what came: of the input, only the identifier goes
//! back. What goes out has no line break inside a message, not even the ones of Unicode.

use std::io;

use arauto_protocol::message::{DecodeError, read_strict};
use serde::Serialize;
use serde_json::{Map, Value};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

/// The version of JSON-RPC.
pub(crate) const JSONRPC: &str = "2.0";

/// The longest identifier in text that is accepted. An identifier is kept while its request is
/// in progress, so it is bounded.
pub(crate) const ID_TEXT_MAX: usize = 256;

/// The identifier of a request: a text or an integer, never `null`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Id {
    /// A text.
    Text(String),
    /// A negative integer.
    Signed(i64),
    /// A positive integer, or zero.
    Unsigned(u64),
}

impl Id {
    /// Reads an identifier: a text of at most [`ID_TEXT_MAX`] bytes, or an integer.
    pub(crate) fn read(value: &Value) -> Option<Self> {
        match value {
            Value::String(text) if text.len() <= ID_TEXT_MAX => Some(Id::Text(text.clone())),
            Value::Number(number) => number
                .as_u64()
                .map(Id::Unsigned)
                .or_else(|| number.as_i64().map(Id::Signed)),
            _ => None,
        }
    }

    /// The identifier as it goes back.
    pub(crate) fn to_value(&self) -> Value {
        match self {
            Id::Text(text) => Value::String(text.clone()),
            Id::Signed(number) => Value::from(*number),
            Id::Unsigned(number) => Value::from(*number),
        }
    }
}

/// The `params` of a message.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Params {
    /// None were given.
    Absent,
    /// An object, as every method of MCP takes.
    Object(Map<String, Value>),
    /// Anything else: an array or a value that is not structured.
    Other,
}

impl Params {
    /// The object of the parameters, empty when none were given.
    pub(crate) fn object(self) -> Option<Map<String, Value>> {
        match self {
            Params::Absent => Some(Map::new()),
            Params::Object(object) => Some(object),
            Params::Other => None,
        }
    }
}

/// An error of JSON-RPC: the code and a fixed message, which repeats nothing of what came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RpcError {
    /// The code.
    pub(crate) code: i64,
    /// The message.
    pub(crate) message: &'static str,
}

impl RpcError {
    /// Not JSON, or not one value.
    pub(crate) const PARSE: Self = Self {
        code: -32_700,
        message: "Parse error",
    };
    /// A line longer than the ceiling.
    pub(crate) const TOO_LARGE: Self = Self {
        code: -32_700,
        message: "Parse error: the message is too large",
    };
    /// Nested deeper than a message may be.
    pub(crate) const TOO_DEEP: Self = Self {
        code: -32_700,
        message: "Parse error: the message is nested too deep",
    };
    /// More values than a message may have.
    pub(crate) const TOO_MANY_VALUES: Self = Self {
        code: -32_700,
        message: "Parse error: the message has too many values",
    };
    /// The same key twice in one object.
    pub(crate) const REPEATED_KEY: Self = Self {
        code: -32_700,
        message: "Parse error: a key is repeated",
    };
    /// Not a request of JSON-RPC.
    pub(crate) const INVALID_REQUEST: Self = Self {
        code: -32_600,
        message: "Invalid Request",
    };
    /// More than one message in an array: the versions of MCP this server speaks have no batch.
    pub(crate) const BATCH: Self = Self {
        code: -32_600,
        message: "Invalid Request: batches are not supported",
    };
    /// A request before `initialize`.
    pub(crate) const NOT_INITIALIZED: Self = Self {
        code: -32_600,
        message: "Invalid Request: the server is not initialized",
    };
    /// A second `initialize`.
    pub(crate) const ALREADY_INITIALIZED: Self = Self {
        code: -32_600,
        message: "Invalid Request: the server is already initialized",
    };
    /// The identifier of a request still in progress.
    pub(crate) const ID_IN_USE: Self = Self {
        code: -32_600,
        message: "Invalid Request: the identifier is in use by a request in progress",
    };
    /// A method this server does not have.
    pub(crate) const METHOD_NOT_FOUND: Self = Self {
        code: -32_601,
        message: "Method not found",
    };
    /// The parameters are not the ones of the method.
    pub(crate) const INVALID_PARAMS: Self = Self {
        code: -32_602,
        message: "Invalid params",
    };
    /// A tool that is not in the catalog of the connector.
    pub(crate) const UNKNOWN_TOOL: Self = Self {
        code: -32_602,
        message: "Invalid params: unknown tool",
    };
    /// A cursor of a list: this server never gives one.
    pub(crate) const INVALID_CURSOR: Self = Self {
        code: -32_602,
        message: "Invalid params: invalid cursor",
    };
}

/// What a line holds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Incoming {
    /// A request.
    Request {
        /// Its identifier.
        id: Id,
        /// The method.
        method: String,
        /// The parameters.
        params: Params,
    },
    /// A notification.
    Notification {
        /// The method.
        method: String,
        /// The parameters.
        params: Params,
    },
    /// Nothing to do: an answer of the client, or a notification out of its form.
    Dropped,
    /// Answered with `error`, with the identifier when it could be read.
    Invalid {
        /// The identifier, when there was one in its form.
        id: Option<Id>,
        /// The error.
        error: RpcError,
    },
}

const fn parse_error(error: DecodeError) -> RpcError {
    match error {
        DecodeError::RepeatedKey => RpcError::REPEATED_KEY,
        DecodeError::TooManyValues => RpcError::TOO_MANY_VALUES,
        DecodeError::TooDeep => RpcError::TOO_DEEP,
        _ => RpcError::PARSE,
    }
}

/// Reads one line.
pub(crate) fn parse(line: &[u8]) -> Incoming {
    let value = match read_strict(line) {
        Ok(value) => value,
        Err(error) => {
            return Incoming::Invalid {
                id: None,
                error: parse_error(error),
            };
        }
    };
    let mut object = match value {
        Value::Object(object) => object,
        Value::Array(_) => {
            return Incoming::Invalid {
                id: None,
                error: RpcError::BATCH,
            };
        }
        _ => {
            return Incoming::Invalid {
                id: None,
                error: RpcError::INVALID_REQUEST,
            };
        }
    };
    let version = object.get("jsonrpc").and_then(Value::as_str) == Some(JSONRPC);
    let params = match object.remove("params") {
        None => Params::Absent,
        Some(Value::Object(params)) => Params::Object(params),
        Some(_) => Params::Other,
    };
    let method = object.remove("method");
    let Some(id) = object.get("id") else {
        return match method {
            // A notification is never answered: one out of its form is dropped.
            Some(Value::String(method)) if version => Incoming::Notification { method, params },
            Some(Value::String(_)) => Incoming::Dropped,
            _ => Incoming::Invalid {
                id: None,
                error: RpcError::INVALID_REQUEST,
            },
        };
    };
    let Some(id) = Id::read(id) else {
        return Incoming::Invalid {
            id: None,
            error: RpcError::INVALID_REQUEST,
        };
    };
    match method {
        Some(Value::String(method)) if version => Incoming::Request { id, method, params },
        None if object.contains_key("result") || object.contains_key("error") => Incoming::Dropped,
        _ => Incoming::Invalid {
            id: Some(id),
            error: RpcError::INVALID_REQUEST,
        },
    }
}

/// The answer to the request `id`.
pub(crate) fn result(id: &Id, result: Value) -> Vec<u8> {
    let mut message = Map::new();
    message.insert("jsonrpc".to_owned(), Value::from(JSONRPC));
    message.insert("id".to_owned(), id.to_value());
    message.insert("result".to_owned(), result);
    line(&Value::Object(message))
}

/// The error of a request, of `id`, or of `null` when the identifier could not be read.
pub(crate) fn error(id: Option<&Id>, error: RpcError) -> Vec<u8> {
    let mut body = Map::new();
    body.insert("code".to_owned(), Value::from(error.code));
    body.insert("message".to_owned(), Value::from(error.message));
    let mut message = Map::new();
    message.insert("jsonrpc".to_owned(), Value::from(JSONRPC));
    message.insert("id".to_owned(), id.map_or(Value::Null, Id::to_value));
    message.insert("error".to_owned(), Value::Object(body));
    line(&Value::Object(message))
}

/// A notification of this server.
pub(crate) fn notification(method: &str) -> Vec<u8> {
    let mut message = Map::new();
    message.insert("jsonrpc".to_owned(), Value::from(JSONRPC));
    message.insert("method".to_owned(), Value::from(method));
    line(&Value::Object(message))
}

/// One message in one line, with its end. Every line break inside it is escaped, the ones of
/// Unicode too: a reader that splits lines on them still reads one message per line.
pub(crate) fn line(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, OneLine);
    if value.serialize(&mut serializer).is_err() {
        // A value of serde_json always serializes; this keeps the line a message.
        bytes = b"{}".to_vec();
    }
    bytes.push(b'\n');
    bytes
}

/// The formatter of [`line`]: compact, with U+0085, U+2028 and U+2029 escaped in every text.
struct OneLine;

impl serde_json::ser::Formatter for OneLine {
    fn write_string_fragment<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut start = 0usize;
        for (at, character) in fragment.char_indices() {
            if matches!(character, '\u{85}' | '\u{2028}' | '\u{2029}') {
                writer.write_all(fragment.get(start..at).unwrap_or_default().as_bytes())?;
                write!(writer, "\\u{:04x}", u32::from(character))?;
                start = at.saturating_add(character.len_utf8());
            }
        }
        writer.write_all(fragment.get(start..).unwrap_or_default().as_bytes())
    }
}

/// What the next line of the input is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Read {
    /// A line, without its end.
    Line(Vec<u8>),
    /// A line longer than the ceiling: it was read to its end and dropped.
    TooLong,
    /// The input ended. A last line without its end is not a message.
    End,
}

/// Reads lines of at most `cap` bytes. A longer line is read to its end and dropped: memory
/// never holds more than the ceiling.
pub(crate) struct Lines<R> {
    reader: R,
    cap: usize,
}

impl<R: AsyncBufRead + Unpin> Lines<R> {
    pub(crate) fn new(reader: R, cap: usize) -> Self {
        Self { reader, cap }
    }

    /// The next line.
    ///
    /// # Errors
    /// What the input says.
    pub(crate) async fn next(&mut self) -> io::Result<Read> {
        let mut line = Vec::new();
        let mut over = false;
        loop {
            let available = self.reader.fill_buf().await?;
            if available.is_empty() {
                return Ok(Read::End);
            }
            let (taken, found) = match available.iter().position(|byte| *byte == b'\n') {
                Some(end) => (end.saturating_add(1), true),
                None => (available.len(), false),
            };
            if !over {
                let piece = available.get(..taken).unwrap_or_default();
                let piece = if found {
                    piece
                        .get(..piece.len().saturating_sub(1))
                        .unwrap_or_default()
                } else {
                    piece
                };
                if line.len().saturating_add(piece.len()) > self.cap {
                    over = true;
                    line = Vec::new();
                } else {
                    line.extend_from_slice(piece);
                }
            }
            self.reader.consume(taken);
            if found {
                return Ok(if over {
                    Read::TooLong
                } else {
                    Read::Line(line)
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse_text(text: &str) -> Incoming {
        parse(text.as_bytes())
    }

    fn invalid(id: Option<Id>, error: RpcError) -> Incoming {
        Incoming::Invalid { id, error }
    }

    #[test]
    fn requests_and_notifications_are_read() {
        assert_eq!(
            parse_text(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#),
            Incoming::Request {
                id: Id::Unsigned(1),
                method: "ping".to_owned(),
                params: Params::Absent
            }
        );
        assert_eq!(
            parse_text(r#"{"jsonrpc":"2.0","id":"a-1","method":"tools/list","params":{}}"#),
            Incoming::Request {
                id: Id::Text("a-1".to_owned()),
                method: "tools/list".to_owned(),
                params: Params::Object(Map::new())
            }
        );
        assert_eq!(
            parse_text(r#"{"jsonrpc":"2.0","id":-7,"method":"x","params":[1]}"#),
            Incoming::Request {
                id: Id::Signed(-7),
                method: "x".to_owned(),
                params: Params::Other
            }
        );
        assert_eq!(
            parse_text(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            Incoming::Notification {
                method: "notifications/initialized".to_owned(),
                params: Params::Absent
            }
        );
    }

    /// A notification is never answered, not even one out of its form; an answer of the client
    /// is dropped.
    #[test]
    fn what_has_no_answer_is_dropped() {
        for text in [
            r#"{"jsonrpc":"1.0","method":"notifications/initialized"}"#,
            r#"{"method":"anything"}"#,
            r#"{"jsonrpc":"2.0","id":3,"result":{}}"#,
            r#"{"jsonrpc":"2.0","id":3,"error":{"code":1,"message":"x"}}"#,
        ] {
            assert_eq!(parse_text(text), Incoming::Dropped, "{text}");
        }
        assert!(matches!(
            parse_text(r#"{"jsonrpc":"2.0","method":"x","params":5}"#),
            Incoming::Notification {
                params: Params::Other,
                ..
            }
        ));
    }

    #[test]
    fn what_is_not_json_is_a_parse_error() {
        let deep = format!("{}{}", "[".repeat(100), "]".repeat(100));
        let many = format!("[{}0]", "0,".repeat(100_000));
        for (text, error) in [
            ("", RpcError::PARSE),
            ("not json", RpcError::PARSE),
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"ping""#,
                RpcError::PARSE,
            ),
            (r#"{"jsonrpc":"2.0"} {"jsonrpc":"2.0"}"#, RpcError::PARSE),
            (
                r#"{"jsonrpc":"2.0","id":1,"method":"ping","method":"x"}"#,
                RpcError::REPEATED_KEY,
            ),
            (deep.as_str(), RpcError::TOO_DEEP),
            (many.as_str(), RpcError::TOO_MANY_VALUES),
        ] {
            assert_eq!(
                parse_text(text),
                invalid(None, error),
                "{}",
                &text[..text.len().min(60)]
            );
        }
        assert_eq!(parse(&[0xff, 0xfe]), invalid(None, RpcError::PARSE));
    }

    #[test]
    fn what_is_not_a_request_is_invalid() {
        let long = format!(
            r#"{{"jsonrpc":"2.0","id":"{}","method":"ping"}}"#,
            "x".repeat(ID_TEXT_MAX + 1)
        );
        for (text, id, error) in [
            ("[]", None, RpcError::BATCH),
            (
                r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#,
                None,
                RpcError::BATCH,
            ),
            ("5", None, RpcError::INVALID_REQUEST),
            (r#""ping""#, None, RpcError::INVALID_REQUEST),
            ("null", None, RpcError::INVALID_REQUEST),
            ("{}", None, RpcError::INVALID_REQUEST),
            (
                r#"{"jsonrpc":"2.0","method":7}"#,
                None,
                RpcError::INVALID_REQUEST,
            ),
            (
                r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
                None,
                RpcError::INVALID_REQUEST,
            ),
            (
                r#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#,
                None,
                RpcError::INVALID_REQUEST,
            ),
            (
                r#"{"jsonrpc":"2.0","id":{"a":1},"method":"ping"}"#,
                None,
                RpcError::INVALID_REQUEST,
            ),
            (
                r#"{"jsonrpc":"2.0","id":true,"method":"ping"}"#,
                None,
                RpcError::INVALID_REQUEST,
            ),
            (long.as_str(), None, RpcError::INVALID_REQUEST),
            (
                r#"{"jsonrpc":"1.0","id":4,"method":"ping"}"#,
                Some(Id::Unsigned(4)),
                RpcError::INVALID_REQUEST,
            ),
            (
                r#"{"id":5,"method":"ping"}"#,
                Some(Id::Unsigned(5)),
                RpcError::INVALID_REQUEST,
            ),
            (
                r#"{"jsonrpc":"2.0","id":6,"method":["ping"]}"#,
                Some(Id::Unsigned(6)),
                RpcError::INVALID_REQUEST,
            ),
            (
                r#"{"jsonrpc":"2.0","id":"x"}"#,
                Some(Id::Text("x".to_owned())),
                RpcError::INVALID_REQUEST,
            ),
        ] {
            assert_eq!(
                parse_text(text),
                invalid(id, error),
                "{}",
                &text[..text.len().min(60)]
            );
        }
    }

    #[test]
    fn identifiers_go_back_as_they_came() {
        for value in [
            json!(0),
            json!(7),
            json!(-3),
            json!(u64::MAX),
            json!(i64::MIN),
            json!("abc"),
        ] {
            assert_eq!(Id::read(&value).unwrap().to_value(), value);
        }
        let answer: Value =
            serde_json::from_slice(&result(&Id::Text("q".to_owned()), json!({}))).unwrap();
        assert_eq!(answer, json!({"jsonrpc":"2.0","id":"q","result":{}}));
        let answer: Value =
            serde_json::from_slice(&error(None, RpcError::METHOD_NOT_FOUND)).unwrap();
        assert_eq!(
            answer,
            json!({"jsonrpc":"2.0","id":null,"error":{"code":-32601,"message":"Method not found"}})
        );
        let told: Value =
            serde_json::from_slice(&notification("notifications/tools/list_changed")).unwrap();
        assert_eq!(
            told,
            json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"})
        );
    }

    /// No line break inside a message, not even the ones of Unicode, and the text reads back
    /// the same.
    #[test]
    fn a_message_is_one_line() {
        let text = "a\nb\r\nc\u{85}d\u{2028}e\u{2029}f";
        let written = line(&json!({"text": text, text: 1}));
        assert_eq!(written.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert!(written.ends_with(b"\n"));
        let written = String::from_utf8(written).unwrap();
        for separator in ['\u{85}', '\u{2028}', '\u{2029}', '\r'] {
            assert!(!written.contains(separator), "{written:?}");
        }
        assert!(written.contains("\\u2028") && written.contains("\\u0085"));
        let read: Value = serde_json::from_str(&written).unwrap();
        assert_eq!(read["text"], text);
        assert_eq!(read[text], 1);
    }

    #[tokio::test]
    async fn a_line_above_the_ceiling_is_dropped_and_the_next_one_is_read() {
        let mut text = b"short\n".to_vec();
        text.extend(vec![b'x'; 100]);
        text.extend(b"\nafter\nno end");
        let mut lines = Lines::new(tokio::io::BufReader::with_capacity(8, &text[..]), 16);
        assert_eq!(lines.next().await.unwrap(), Read::Line(b"short".to_vec()));
        assert_eq!(lines.next().await.unwrap(), Read::TooLong);
        assert_eq!(lines.next().await.unwrap(), Read::Line(b"after".to_vec()));
        assert_eq!(lines.next().await.unwrap(), Read::End);
        // Exactly the ceiling is a line.
        let exact = format!("{}\n", "y".repeat(16));
        let mut lines = Lines::new(exact.as_bytes(), 16);
        assert_eq!(lines.next().await.unwrap(), Read::Line(vec![b'y'; 16]));
    }
}
