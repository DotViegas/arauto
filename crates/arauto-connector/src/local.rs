//! The messages of the local socket between the copies of the connector
//! (`docs/14-protocolo-do-canal.md`, section 11; `docs/11-conector.md`, section 3).
//!
//! Every message is one JSON object in one line, with `"v": 1`, both ways. A request may carry
//! an `"id"`, a positive integer, which the answer repeats; a `call` must carry one, and its
//! answers may come in any order.
//!
//! | Request | Answer |
//! |---|---|
//! | `{"v":1,"op":"call","id":1,"tool":"arauto_status","args":{}}` | `{"v":1,"id":1,"ok":true,"content":{}}` or `{"v":1,"id":1,"ok":false,"code":"AR-203"}` |
//! | `{"v":1,"op":"tools"}` | `{"v":1,"ok":true,"tools":["arauto_status"]}` |
//! | `{"v":1,"op":"watch"}` | `{"v":1,"ok":true,"tools":[...]}`, then one line `{"v":1,"tools":[...]}` each time the list changes |
//! | `{"v":1,"op":"status"}` | `{"v":1,"ok":true,"status":{...}}`, without any secret |
//! | `{"v":1,"op":"await","id":1,"thread":"<uuid>","minutes":20}` | one final line, later: `{"v":1,"id":1,"ok":true,"arrived":true,"thread":"<uuid>","peer":"<name>"}`, `{"v":1,"id":1,"ok":true,"arrived":false}` (the deadline), `{"v":1,"id":1,"ok":false,"code":"AR-708"}` or `{"v":1,"id":1,"ok":false,"error":"busy"}` |
//! | `{"v":1,"op":"new_request"}` | `{"v":1,"ok":true}`: the leader reads the folders again |
//! | `{"v":1,"op":"leave"}` | `{"v":1,"ok":true,"done":true,"left":[{"device":"...","outcome":"closed"}]}` |
//! | `{"v":1,"op":"restart"}` | `{"v":1,"ok":true}`, and the leader ends |
//! | anything else | `{"v":1,"ok":false,"error":"unsupported"}` or `{"v":1,"ok":false,"error":"invalid"}` |
//!
//! When the leader ends for a restart, every connection still open gets one last line,
//! `{"v":1,"restart":true}`, before it closes: a copy that forwards ends too, so that Hermes
//! starts it again on the new program.
//!
//! The answer to `await` carries only a thread and a Hermes name that the leader validated from
//! the hub's listing of threads: never the text of a note. A connection holds one `await` at a
//! time, and a leader holds at most 8 for its Hermes.
//!
//! What comes in is hostile: a line is at most a little more than the largest message of the
//! channel, it is read with the rules of a message (no repeated key, a ceiling of values and of
//! nesting), and nothing of it is repeated in an answer but the number of the request. No
//! answer carries the identity, the credential, the invite or the code.

use arauto_protocol::errors::{ErrorCode, RetryAfter};
use arauto_protocol::limits::{CONNECTOR_MESSAGE_MAX, HUB_MESSAGE_MAX};
use arauto_protocol::message::{CallOutcome, Payload, read_strict};
use arauto_protocol::names::HermesName;
use arauto_protocol::tools::ToolName;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use uuid::Uuid;

/// The version of the messages.
pub(crate) const VERSION: u64 = 1;

/// The largest line a leader reads: a call of the largest size the hub takes, and its frame.
pub(crate) const REQUEST_LINE_MAX: usize = CONNECTOR_MESSAGE_MAX + 64 * 1024;

/// The largest line a copy reads from the leader: an answer of the largest size the hub sends.
pub(crate) const ANSWER_LINE_MAX: usize = HUB_MESSAGE_MAX + 64 * 1024;

/// A request to the leader.
#[derive(Debug)]
pub(crate) enum Request {
    /// A tool call.
    Call {
        /// The name of the tool.
        tool: ToolName,
        /// The arguments.
        args: Payload,
    },
    /// The list of released tools.
    Tools,
    /// The list now, and each time it changes.
    Watch,
    /// The situation.
    Status,
    /// A request was created, or the folders changed otherwise: they are read again.
    FolderChanged,
    /// Leave, for every identity of the folder.
    Leave,
    /// End, so that Hermes starts the new version.
    Restart,
    /// Wait for an answer in a thread (decision 0048).
    Await {
        /// The thread.
        thread: Uuid,
        /// How long, in minutes: 1 to 120.
        minutes: u32,
    },
}

/// A request and its number.
#[derive(Debug)]
pub(crate) struct Envelope {
    pub(crate) id: Option<u64>,
    pub(crate) request: Request,
}

/// Why a line is not a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Another version, or an operation this one does not know.
    Unsupported,
    /// A field missing or out of its format.
    Invalid,
    /// This connection or this leader already holds the most waits it may.
    Busy,
}

impl Refusal {
    fn text(self) -> &'static str {
        match self {
            Refusal::Unsupported => "unsupported",
            Refusal::Invalid => "invalid",
            Refusal::Busy => "busy",
        }
    }
}

/// Reads one line.
pub(crate) fn parse_request(line: &[u8]) -> Result<Envelope, (Option<u64>, Refusal)> {
    let Ok(Value::Object(mut object)) = read_strict(line) else {
        return Err((None, Refusal::Invalid));
    };
    let id = match object.get("id") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_u64().filter(|id| *id > 0) {
            Some(id) => Some(id),
            None => return Err((None, Refusal::Invalid)),
        },
    };
    if object.get("v").and_then(Value::as_u64) != Some(VERSION) {
        return Err((id, Refusal::Unsupported));
    }
    let Some(Value::String(op)) = object.remove("op") else {
        return Err((id, Refusal::Unsupported));
    };
    let request = match op.as_str() {
        "call" => {
            let tool = object
                .get("tool")
                .and_then(Value::as_str)
                .and_then(|tool| ToolName::parse(tool).ok());
            let args = match object.remove("args") {
                Some(Value::Object(args)) => Some(args),
                None => Some(Map::new()),
                Some(_) => None,
            };
            match (id, tool, args) {
                (Some(_), Some(tool), Some(args)) => Request::Call {
                    tool,
                    args: Payload::new(args),
                },
                _ => return Err((id, Refusal::Invalid)),
            }
        }
        "tools" => Request::Tools,
        "watch" => Request::Watch,
        "status" => Request::Status,
        "new_request" => Request::FolderChanged,
        "leave" => Request::Leave,
        "restart" => Request::Restart,
        "await" => {
            let thread = object
                .get("thread")
                .and_then(Value::as_str)
                .and_then(crate::reply::parse_thread);
            let minutes = object
                .get("minutes")
                .and_then(Value::as_u64)
                .filter(|minutes| (1..=u64::from(crate::reply::MINUTES_MAX)).contains(minutes))
                .and_then(|minutes| u32::try_from(minutes).ok());
            match (id, thread, minutes) {
                (Some(_), Some(thread), Some(minutes)) => Request::Await { thread, minutes },
                _ => return Err((id, Refusal::Invalid)),
            }
        }
        _ => return Err((id, Refusal::Unsupported)),
    };
    Ok(Envelope { id, request })
}

/// One line of JSON, with its end.
pub(crate) fn line(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    bytes.push(b'\n');
    bytes
}

fn with_id(mut object: Map<String, Value>, id: Option<u64>) -> Value {
    object.insert("v".to_owned(), Value::from(VERSION));
    if let Some(id) = id {
        object.insert("id".to_owned(), Value::from(id));
    }
    Value::Object(object)
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(object) => object,
        _ => Map::new(),
    }
}

/// The answer to a line that is not a request.
pub(crate) fn refusal(id: Option<u64>, refusal: Refusal) -> Value {
    with_id(object(json!({"ok": false, "error": refusal.text()})), id)
}

/// The answer to a call.
pub(crate) fn call_answer(id: u64, outcome: &CallOutcome) -> Value {
    let body = match outcome {
        CallOutcome::Ok(content) => json!({"ok": true, "content": content}),
        CallOutcome::Refused { code, retry_after } => match retry_after {
            Some(wait) => json!({"ok": false, "code": code.as_str(), "retry_after_s": wait.get()}),
            None => json!({"ok": false, "code": code.as_str()}),
        },
    };
    with_id(object(body), Some(id))
}

/// The list of tools, as an answer or as a line of `watch`.
pub(crate) fn tools_answer(id: Option<u64>, tools: &[ToolName], ok: bool) -> Value {
    let names: Vec<Value> = tools
        .iter()
        .map(|tool| Value::from(tool.as_str()))
        .collect();
    let mut body = Map::new();
    if ok {
        body.insert("ok".to_owned(), Value::Bool(true));
    }
    body.insert("tools".to_owned(), Value::Array(names));
    with_id(body, id)
}

/// An answer that only says yes.
pub(crate) fn done(id: Option<u64>) -> Value {
    with_id(object(json!({"ok": true})), id)
}

/// The last line of every connection when the leader ends for a restart.
pub(crate) fn restarting() -> Value {
    with_id(object(json!({"restart": true})), None)
}

/// Whether a line of the leader says it ends for a restart.
pub(crate) fn is_restart(object: &Map<String, Value>) -> bool {
    object.get("restart") == Some(&Value::Bool(true))
}

/// An answer with a body under `key`.
pub(crate) fn answer(id: Option<u64>, key: &str, body: Value) -> Value {
    let mut object = Map::new();
    object.insert("ok".to_owned(), Value::Bool(true));
    object.insert(key.to_owned(), body);
    with_id(object, id)
}

/// Reads a call answer that came from the leader. Anything out of its form is AR-207: the
/// call left, and its result is not known.
pub(crate) fn read_call_answer(object: &Map<String, Value>) -> CallOutcome {
    match object.get("ok") {
        Some(Value::Bool(true)) => match object.get("content") {
            Some(Value::Object(content)) => CallOutcome::Ok(Payload::new(content.clone())),
            _ => crate::session::refused(ErrorCode::Ar207),
        },
        Some(Value::Bool(false)) => {
            let code = object
                .get("code")
                .and_then(Value::as_str)
                .and_then(ErrorCode::parse)
                .unwrap_or(ErrorCode::Ar207);
            let retry_after = object
                .get("retry_after_s")
                .and_then(Value::as_u64)
                .and_then(|wait| u32::try_from(wait).ok())
                .and_then(RetryAfter::new);
            CallOutcome::Refused { code, retry_after }
        }
        _ => crate::session::refused(ErrorCode::Ar207),
    }
}

/// The final line of `await` when an answer arrived: a thread and a name, both checked.
pub(crate) fn await_arrived(id: u64, thread: Uuid, peer: &HermesName) -> Value {
    with_id(
        object(json!({
            "ok": true,
            "arrived": true,
            "thread": thread.as_hyphenated().to_string(),
            "peer": peer.as_str(),
        })),
        Some(id),
    )
}

/// The final line of `await` at its deadline.
pub(crate) fn await_deadline(id: u64) -> Value {
    with_id(object(json!({"ok": true, "arrived": false})), Some(id))
}

/// The final line of `await` when a tool was refused.
pub(crate) fn await_refused(id: u64, code: ErrorCode) -> Value {
    with_id(
        object(json!({"ok": false, "code": code.as_str()})),
        Some(id),
    )
}

/// What the leader said to an `await`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AwaitAnswer {
    /// An answer arrived.
    Arrived { thread: Uuid, peer: HermesName },
    /// The deadline came.
    Deadline,
    /// A tool was refused, with this code.
    Code(ErrorCode),
    /// Too many waits.
    Busy,
    /// An older leader, or a request it did not accept.
    Unsupported,
}

/// Reads the final line of an `await` that came from the leader. `None` for a restart line
/// and for anything out of its form.
pub(crate) fn read_await_answer(object: &Map<String, Value>, id: u64) -> Option<AwaitAnswer> {
    if is_restart(object) || object.get("id").and_then(Value::as_u64) != Some(id) {
        return None;
    }
    match object.get("ok")? {
        Value::Bool(true) => match object.get("arrived")? {
            Value::Bool(true) => {
                let thread = crate::reply::parse_thread(object.get("thread")?.as_str()?)?;
                let peer = HermesName::parse(object.get("peer")?.as_str()?).ok()?;
                Some(AwaitAnswer::Arrived { thread, peer })
            }
            Value::Bool(false) => Some(AwaitAnswer::Deadline),
            _ => None,
        },
        Value::Bool(false) => {
            if let Some(code) = object.get("code").and_then(Value::as_str) {
                return ErrorCode::parse(code).map(AwaitAnswer::Code);
            }
            match object.get("error").and_then(Value::as_str)? {
                "busy" => Some(AwaitAnswer::Busy),
                "unsupported" | "invalid" => Some(AwaitAnswer::Unsupported),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Reads the names of a list of tools that came from the leader. A name out of its format is
/// left out.
pub(crate) fn read_tools(object: &Map<String, Value>) -> Option<Vec<ToolName>> {
    let names = object.get("tools")?.as_array()?;
    Some(
        names
            .iter()
            .filter_map(|name| name.as_str().and_then(|name| ToolName::parse(name).ok()))
            .take(arauto_protocol::limits::TOOL_LIST_MAX)
            .collect(),
    )
}

/// Why a line could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineError {
    /// Longer than the ceiling: the connection is dropped.
    TooLong,
    /// The connection failed.
    Io,
}

/// Reads lines of at most `cap` bytes, never keeping more than that in memory.
pub(crate) struct Lines<R> {
    reader: R,
    cap: usize,
}

impl<R: AsyncBufRead + Unpin> Lines<R> {
    pub(crate) fn new(reader: R, cap: usize) -> Self {
        Self { reader, cap }
    }

    /// The next line, without its end; `None` at the end of the stream. A last line without
    /// its end is not a line.
    pub(crate) async fn next(&mut self) -> Result<Option<Vec<u8>>, LineError> {
        let mut line = Vec::new();
        loop {
            let available = self.reader.fill_buf().await.map_err(|_| LineError::Io)?;
            if available.is_empty() {
                return Ok(None);
            }
            let (taken, found) = match available.iter().position(|byte| *byte == b'\n') {
                Some(end) => (end.saturating_add(1), true),
                None => (available.len(), false),
            };
            let piece = available.get(..taken).unwrap_or_default();
            let piece = if found {
                piece
                    .get(..piece.len().saturating_sub(1))
                    .unwrap_or_default()
            } else {
                piece
            };
            if line.len().saturating_add(piece.len()) > self.cap {
                return Err(LineError::TooLong);
            }
            line.extend_from_slice(piece);
            self.reader.consume(taken);
            if found {
                return Ok(Some(line));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Envelope, (Option<u64>, Refusal)> {
        parse_request(text.as_bytes())
    }

    #[test]
    fn the_requests_are_read() {
        let call = parse(r#"{"v":1,"op":"call","id":7,"tool":"arauto_status","args":{}}"#).unwrap();
        assert_eq!(call.id, Some(7));
        assert!(
            matches!(call.request, Request::Call { ref tool, .. } if tool.as_str() == "arauto_status")
        );
        // Arguments may be left out.
        assert!(parse(r#"{"v":1,"op":"call","id":1,"tool":"arauto_status"}"#).is_ok());
        for (text, expected) in [
            (r#"{"v":1,"op":"tools"}"#, "tools"),
            (r#"{"v":1,"op":"watch"}"#, "watch"),
            (r#"{"v":1,"op":"status","id":3}"#, "status"),
            (r#"{"v":1,"op":"new_request"}"#, "new"),
            (r#"{"v":1,"op":"leave"}"#, "leave"),
            (r#"{"v":1,"op":"restart"}"#, "restart"),
            (
                r#"{"v":1,"op":"await","id":2,"thread":"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d","minutes":20}"#,
                "await",
            ),
        ] {
            let request = parse(text).unwrap().request;
            let name = match request {
                Request::Tools => "tools",
                Request::Watch => "watch",
                Request::Status => "status",
                Request::FolderChanged => "new",
                Request::Leave => "leave",
                Request::Restart => "restart",
                Request::Await { .. } => "await",
                Request::Call { .. } => "call",
            };
            assert_eq!(name, expected, "{text}");
        }
    }

    /// Hostile lines are refused, and the refusal repeats nothing but the number.
    #[test]
    fn hostile_lines_are_refused() {
        for (text, refusal) in [
            ("", Refusal::Invalid),
            ("[]", Refusal::Invalid),
            ("not json", Refusal::Invalid),
            (r#"{"v":1,"op":"tools","op":"leave"}"#, Refusal::Invalid),
            (r#"{"v":1,"op":"tools","id":-1}"#, Refusal::Invalid),
            (r#"{"v":1,"op":"tools","id":0}"#, Refusal::Invalid),
            (r#"{"v":1,"op":"tools","id":"1"}"#, Refusal::Invalid),
            (r#"{"v":2,"op":"tools"}"#, Refusal::Unsupported),
            (r#"{"op":"tools"}"#, Refusal::Unsupported),
            (r#"{"v":1,"op":"format_the_disk"}"#, Refusal::Unsupported),
            (r#"{"v":1}"#, Refusal::Unsupported),
            (
                r#"{"v":1,"op":"call","tool":"arauto_status"}"#,
                Refusal::Invalid,
            ),
            (
                r#"{"v":1,"op":"call","id":1,"tool":"Not A Tool"}"#,
                Refusal::Invalid,
            ),
            (
                r#"{"v":1,"op":"call","id":1,"tool":"arauto_status","args":[]}"#,
                Refusal::Invalid,
            ),
            (
                r#"{"v":1,"op":"await","thread":"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d","minutes":5}"#,
                Refusal::Invalid,
            ),
            (
                r#"{"v":1,"op":"await","id":1,"minutes":5}"#,
                Refusal::Invalid,
            ),
        ] {
            let (_, found) = parse(text).unwrap_err();
            assert_eq!(found, refusal, "{text}");
        }
        let good = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
        for minutes in ["0", "121", "\"20\"", "1.5", "-1", "null"] {
            let text =
                format!(r#"{{"v":1,"op":"await","id":1,"thread":"{good}","minutes":{minutes}}}"#);
            assert_eq!(parse(&text).unwrap_err().1, Refusal::Invalid, "{text}");
        }
        for thread in [
            "\"0a1b2c3d4e5f4a6b8c7d9e0f1a2b3c4d\"",
            "\"{0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d}\"",
            "\"$(id)\"",
            "\"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4dd\"",
            "null",
            "5",
        ] {
            let text = format!(r#"{{"v":1,"op":"await","id":1,"thread":{thread},"minutes":5}}"#);
            assert_eq!(parse(&text).unwrap_err().1, Refusal::Invalid, "{text}");
        }
        let deep = format!(
            r#"{{"v":1,"op":"tools","x":{}{}}}"#,
            "[".repeat(100),
            "]".repeat(100)
        );
        assert!(parse(&deep).is_err());
        let answer = refusal(Some(4), Refusal::Unsupported);
        assert_eq!(
            answer,
            json!({"v":1,"id":4,"ok":false,"error":"unsupported"})
        );
    }

    #[test]
    fn answers_carry_the_version_and_the_number() {
        let ok = call_answer(2, &CallOutcome::Ok(Payload::default()));
        assert_eq!(ok, json!({"v":1,"id":2,"ok":true,"content":{}}));
        let refused = call_answer(3, &crate::session::refused(ErrorCode::Ar203));
        assert_eq!(refused, json!({"v":1,"id":3,"ok":false,"code":"AR-203"}));
        assert_eq!(
            read_call_answer(refused.as_object().unwrap()),
            crate::session::refused(ErrorCode::Ar203)
        );
        assert_eq!(
            read_call_answer(json!({"ok":false,"code":"AR-999"}).as_object().unwrap()),
            crate::session::refused(ErrorCode::Ar207)
        );
        let tools = tools_answer(None, &[ToolName::parse("arauto_status").unwrap()], true);
        assert_eq!(tools, json!({"v":1,"ok":true,"tools":["arauto_status"]}));
        assert_eq!(
            read_tools(tools.as_object().unwrap()).unwrap(),
            vec![ToolName::parse("arauto_status").unwrap()]
        );
        assert!(line(&done(None)).ends_with(b"\n"));
    }

    #[tokio::test]
    async fn a_line_above_the_ceiling_is_refused_without_keeping_it() {
        let text = b"short\nsecond line\nthis one is far too long for the ceiling\n".to_vec();
        let mut lines = Lines::new(tokio::io::BufReader::with_capacity(4, &text[..]), 16);
        assert_eq!(lines.next().await.unwrap(), Some(b"short".to_vec()));
        assert_eq!(lines.next().await.unwrap(), Some(b"second line".to_vec()));
        assert_eq!(lines.next().await, Err(LineError::TooLong));
        let mut lines = Lines::new(&b"no end"[..], 16);
        assert_eq!(lines.next().await.unwrap(), None);
    }

    #[test]
    fn the_await_answers_are_the_wire_lines() {
        let thread = crate::reply::parse_thread("0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d").unwrap();
        let peer = HermesName::parse("ana").unwrap();
        assert_eq!(
            await_arrived(3, thread, &peer),
            json!({"v":1,"id":3,"ok":true,"arrived":true,
                   "thread":"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d","peer":"ana"})
        );
        assert_eq!(
            await_deadline(3),
            json!({"v":1,"id":3,"ok":true,"arrived":false})
        );
        assert_eq!(
            await_refused(3, ErrorCode::Ar708),
            json!({"v":1,"id":3,"ok":false,"code":"AR-708"})
        );
        assert_eq!(
            refusal(Some(3), Refusal::Busy),
            json!({"v":1,"id":3,"ok":false,"error":"busy"})
        );
        for (line, expected) in [
            (
                await_arrived(1, thread, &peer),
                AwaitAnswer::Arrived {
                    thread,
                    peer: peer.clone(),
                },
            ),
            (await_deadline(1), AwaitAnswer::Deadline),
            (
                await_refused(1, ErrorCode::Ar203),
                AwaitAnswer::Code(ErrorCode::Ar203),
            ),
            (refusal(Some(1), Refusal::Busy), AwaitAnswer::Busy),
            (
                refusal(Some(1), Refusal::Unsupported),
                AwaitAnswer::Unsupported,
            ),
            (refusal(Some(1), Refusal::Invalid), AwaitAnswer::Unsupported),
        ] {
            assert_eq!(
                read_await_answer(line.as_object().unwrap(), 1),
                Some(expected)
            );
        }
    }

    #[test]
    fn an_await_answer_is_read_only_in_its_form() {
        for line in [
            json!({"v":1,"id":2,"ok":true,"arrived":false}),
            json!({"v":1,"id":1,"ok":true,"arrived":true,"thread":"0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d","peer":"Ana"}),
            json!({"v":1,"id":1,"ok":true,"arrived":true,"thread":"$(id)","peer":"ana"}),
            json!({"v":1,"id":1,"ok":true,"arrived":true,"peer":"ana"}),
            json!({"v":1,"id":1,"ok":true}),
            json!({"v":1,"id":1,"ok":false,"code":"AR-999"}),
            json!({"v":1,"id":1,"ok":false,"error":"other"}),
            json!({"v":1,"id":1,"ok":false}),
            json!({"v":1,"restart":true}),
        ] {
            assert_eq!(
                read_await_answer(line.as_object().unwrap(), 1),
                None,
                "{line}"
            );
        }
    }
}
