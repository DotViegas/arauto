use std::collections::HashSet;

use proptest::prelude::*;
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::limits::{
    CONNECTOR_MESSAGE_MAX, MESSAGE_DEPTH_MAX, MESSAGE_VALUES_MAX, PRE_SESSION_MESSAGE_MAX,
};
use crate::names::{ChatId, NumberAlias};
use crate::notice::{Notice, NoticeCount, Rejected};
use crate::secret::Secret;
use crate::test_util::assert_hides;

const INVITE: &str = "arc1aaaqeayeaudaocajbifqydiob4ibceqtcqkrmfyydenbwha5dypq";
const NOTICE_ID: &str = "3f2b7c1e-58a4-4d6b-9e0f-a1b2c3d4e5f6";
const THREAD: &str = "0b9c5f5e-6f0a-4c3e-9d54-1c2f3a4b5c6d";

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// Base64 of 16 and of 32 zero bytes.
fn zeros16() -> String {
    format!("{}==", "A".repeat(22))
}

fn zeros32() -> String {
    format!("{}=", "A".repeat(43))
}

fn declared_text(value: &str) -> DeclaredText {
    DeclaredText::parse(value).unwrap()
}

fn tool_list(names: &[&str]) -> ToolList {
    ToolList::new(
        names
            .iter()
            .map(|name| ToolName::parse(name).unwrap())
            .collect(),
    )
    .unwrap()
}

fn sealed(bytes: &[u8]) -> SealedCredential {
    SealedCredential::from_slice(bytes).unwrap()
}

fn generation(value: u64) -> Generation {
    Generation::new(value).unwrap()
}

fn call_id(value: u64) -> CallId {
    CallId::new(value).unwrap()
}

fn notice_id() -> NoticeId {
    NoticeId::new(Uuid::try_parse(NOTICE_ID).unwrap())
}

fn payload(value: Value) -> Payload {
    match value {
        Value::Object(object) => Payload::new(object),
        other => panic!("not an object: {other}"),
    }
}

fn counting_invite() -> Invite {
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::try_from(index).unwrap();
    }
    Invite::from_secret(Secret::new(bytes))
}

fn hello(purpose: Purpose) -> Message {
    Message::Hello(Hello {
        proto: ProtoRange::CURRENT,
        connector: declared_text("0.1.0"),
        purpose,
    })
}

/// Whether the text is of a type that travels with the CONTROL flag.
fn control_of(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value.get("t").and_then(Value::as_str).map(str::to_owned))
        .is_some_and(|tag| matches!(tag.as_str(), "check" | "check_ok" | "ping" | "pong"))
}

/// Reads a message with the CONTROL flag its type asks for.
fn read(text: &str) -> Result<Message, DecodeError> {
    decode(text.as_bytes(), control_of(text))
}

fn read_value(value: &Value) -> Result<Message, DecodeError> {
    read(&value.to_string())
}

fn write(message: &Message) -> String {
    String::from_utf8(encode(message).unwrap()).unwrap()
}

/// Every type of message, some in more than one shape, with the exact text that travels.
fn golden() -> Vec<(Message, String)> {
    let nonce32 = zeros32();
    let nonce16 = zeros16();
    vec![
        (
            hello(Purpose::Channel),
            r#"{"t":"hello","proto":[1,1],"connector":"0.1.0","purpose":"channel"}"#.to_owned(),
        ),
        (
            Message::Hello(Hello {
                proto: ProtoRange::new(1, 3).unwrap(),
                connector: declared_text("0.2.0-rc.1"),
                purpose: Purpose::Enroll,
            }),
            r#"{"t":"hello","proto":[1,3],"connector":"0.2.0-rc.1","purpose":"enroll"}"#.to_owned(),
        ),
        (
            hello(Purpose::Leave),
            r#"{"t":"hello","proto":[1,1],"connector":"0.1.0","purpose":"leave"}"#.to_owned(),
        ),
        (Message::HelloOk(HelloOk::Agreed(1)), r#"{"t":"hello_ok","proto":1}"#.to_owned()),
        (
            Message::HelloOk(HelloOk::Outdated),
            r#"{"t":"hello_ok","proto":0,"outdated":true}"#.to_owned(),
        ),
        (
            Message::Enroll(Enroll {
                invite: counting_invite(),
                declared: Declared {
                    hostname: declared_text("host-1"),
                    os: declared_text("linux"),
                    hermes_version: declared_text("0.23.0"),
                },
            }),
            format!(
                r#"{{"t":"enroll","invite":"{INVITE}","declared":{{"hostname":"host-1","os":"linux","hermes_version":"0.23.0"}}}}"#
            ),
        ),
        (
            Message::Enrolled(Enrolled {
                nonce: EnrollNonce::from_bytes([0u8; 32]),
                confirm_within_s: ConfirmWithin::new(600).unwrap(),
                enrollment: EnrollmentNumber::new(1).unwrap(),
            }),
            format!(r#"{{"t":"enrolled","nonce":"{nonce32}","confirm_within_s":600,"enrollment":1}}"#),
        ),
        (Message::State(ChannelState::Waiting), r#"{"t":"state","state":"waiting"}"#.to_owned()),
        (Message::State(ChannelState::Approved), r#"{"t":"state","state":"approved"}"#.to_owned()),
        (Message::State(ChannelState::Active), r#"{"t":"state","state":"active"}"#.to_owned()),
        (
            Message::State(ChannelState::Ended(ErrorCode::Ar502)),
            r#"{"t":"state","state":"ended","code":"AR-502"}"#.to_owned(),
        ),
        (
            Message::Present(Present { credential: None }),
            r#"{"t":"present","credential":null}"#.to_owned(),
        ),
        (
            Message::Present(Present { credential: Some(sealed(&[1, 2, 3])) }),
            r#"{"t":"present","credential":"AQID"}"#.to_owned(),
        ),
        (
            Message::Check(Check { nonce: CheckNonce::from_bytes([0u8; 16]) }),
            format!(r#"{{"t":"check","nonce":"{nonce16}"}}"#),
        ),
        (
            Message::CheckOk(CheckOk { nonce: CheckNonce::from_bytes([0u8; 16]) }),
            format!(r#"{{"t":"check_ok","nonce":"{nonce16}"}}"#),
        ),
        (
            Message::Credential(Credential { generation: generation(7), sealed: sealed(&[1, 2, 3]) }),
            r#"{"t":"credential","generation":7,"sealed":"AQID"}"#.to_owned(),
        ),
        (
            Message::Saved(Saved {
                generation: generation(7),
                digest: CredentialDigest::from_bytes([0u8; 32]),
            }),
            format!(r#"{{"t":"saved","generation":7,"digest":"{nonce32}"}}"#),
        ),
        (
            Message::Session(Session {
                hermes: HermesName::parse("ana").unwrap(),
                tools: tool_list(&["arauto_status", "hermes_list_peers"]),
                wake: true,
            }),
            r#"{"t":"session","hermes":"ana","tools":["arauto_status","hermes_list_peers"],"wake":true}"#
                .to_owned(),
        ),
        (
            Message::Session(Session {
                hermes: HermesName::parse("bento-2").unwrap(),
                tools: tool_list(&[]),
                wake: false,
            }),
            r#"{"t":"session","hermes":"bento-2","tools":[],"wake":false}"#.to_owned(),
        ),
        (
            Message::Tools(Tools { tools: tool_list(&["arauto_status"]) }),
            r#"{"t":"tools","tools":["arauto_status"]}"#.to_owned(),
        ),
        (
            Message::Call(Call {
                id: call_id(1),
                tool: ToolName::parse("hermes_send_message").unwrap(),
                args: payload(json!({"to": "bento", "text": "hi"})),
            }),
            r#"{"t":"call","id":1,"tool":"hermes_send_message","args":{"text":"hi","to":"bento"}}"#
                .to_owned(),
        ),
        (
            Message::Call(Call {
                id: call_id(u64::MAX),
                tool: ToolName::parse("arauto_status").unwrap(),
                args: Payload::default(),
            }),
            r#"{"t":"call","id":18446744073709551615,"tool":"arauto_status","args":{}}"#.to_owned(),
        ),
        (
            Message::Result(CallResult {
                id: call_id(1),
                outcome: CallOutcome::Ok(payload(json!({"thread": THREAD, "unread": [1, 2]}))),
            }),
            format!(
                r#"{{"t":"result","id":1,"ok":true,"content":{{"thread":"{THREAD}","unread":[1,2]}}}}"#
            ),
        ),
        (
            Message::Result(CallResult {
                id: call_id(2),
                outcome: CallOutcome::Refused {
                    code: ErrorCode::Ar706,
                    retry_after: RetryAfter::new(120),
                },
            }),
            r#"{"t":"result","id":2,"ok":false,"code":"AR-706","retry_after_s":120}"#.to_owned(),
        ),
        (
            Message::Result(CallResult {
                id: call_id(3),
                outcome: CallOutcome::Refused { code: ErrorCode::Ar711, retry_after: None },
            }),
            r#"{"t":"result","id":3,"ok":false,"code":"AR-711"}"#.to_owned(),
        ),
        (
            Message::Notice(NoticeMsg::new(
                notice_id(),
                &Notice::Peer {
                    from: HermesName::parse("ana").unwrap(),
                    count: NoticeCount::new(2).unwrap(),
                },
            )),
            format!(r#"{{"t":"notice","id":"{NOTICE_ID}","kind":"peer","from":"ana","count":2}}"#),
        ),
        (
            Message::Notice(NoticeMsg::new(
                notice_id(),
                &Notice::Whatsapp {
                    number: NumberAlias::parse("balcao").unwrap(),
                    chat: ChatId::parse("abc-123").unwrap(),
                    count: NoticeCount::new(9_999).unwrap(),
                },
            )),
            format!(
                r#"{{"t":"notice","id":"{NOTICE_ID}","kind":"whatsapp","number":"balcao","chat":"abc-123","count":9999}}"#
            ),
        ),
        (
            Message::NoticeAck(NoticeAck { id: notice_id(), outcome: AckOutcome::Accepted }),
            format!(r#"{{"t":"notice_ack","id":"{NOTICE_ID}","outcome":"accepted"}}"#),
        ),
        (Message::Rotate, r#"{"t":"rotate"}"#.to_owned()),
        (
            Message::Rotated(Rotated { generation: generation(8) }),
            r#"{"t":"rotated","generation":8}"#.to_owned(),
        ),
        (Message::Ping(Ping { n: 5 }), r#"{"t":"ping","n":5}"#.to_owned()),
        (Message::Pong(Pong { n: 5 }), r#"{"t":"pong","n":5}"#.to_owned()),
        (Message::Leave, r#"{"t":"leave"}"#.to_owned()),
        (
            Message::Left(Left { status: LeftStatus::Closed }),
            r#"{"t":"left","status":"closed"}"#.to_owned(),
        ),
        (
            Message::Left(Left { status: LeftStatus::AlreadyClosed }),
            r#"{"t":"left","status":"already_closed"}"#.to_owned(),
        ),
        (
            Message::Left(Left { status: LeftStatus::Unknown }),
            r#"{"t":"left","status":"unknown"}"#.to_owned(),
        ),
        (
            Message::Closing(Closing { code: ErrorCode::Ar205 }),
            r#"{"t":"closing","code":"AR-205"}"#.to_owned(),
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------------------------

#[test]
fn every_message_is_written_and_read_as_its_exact_text() {
    for (message, text) in golden() {
        assert_eq!(write(&message), text);
        assert_eq!(read(&text).as_ref(), Ok(&message), "{text}");
        // And once more through the writer: what was read writes the same bytes.
        assert_eq!(write(&read(&text).unwrap()), text);
    }
}

#[test]
fn the_golden_list_covers_every_type_of_message() {
    let covered: HashSet<MessageType> = golden()
        .iter()
        .map(|(message, _)| message.kind().unwrap())
        .collect();
    let all: HashSet<MessageType> = MessageType::ALL.iter().copied().collect();
    assert_eq!(covered, all);
    assert_eq!(MessageType::ALL.len(), 23);
    // The type of the message is the `t` of its text.
    for (message, text) in golden() {
        let tag = message.kind().unwrap().tag();
        assert!(text.starts_with(&format!(r#"{{"t":"{tag}""#)), "{text}");
        assert_eq!(MessageType::from_tag(tag), message.kind());
        assert_eq!(message.kind().unwrap().to_string(), tag);
    }
}

#[test]
fn the_hello_of_the_document() {
    let text = r#"{"t":"hello","proto":[1,1],"connector":"0.1.0","purpose":"channel"}"#;
    let message = read(text).unwrap();
    let Message::Hello(hello) = &message else {
        panic!("not a hello: {message:?}")
    };
    assert_eq!((hello.proto.min(), hello.proto.max()), (1, 1));
    assert_eq!(hello.connector.as_str(), "0.1.0");
    assert_eq!(hello.purpose, Purpose::Channel);
    assert_eq!(write(&message), text);
}

#[test]
fn unit_messages_and_unknown_do_not_have_a_type_confusion() {
    assert_eq!(Message::Unknown.kind(), None);
    assert!(!Message::Unknown.is_control());
    assert_eq!(encode(&Message::Unknown), Err(EncodeError::UnknownType));
    assert_eq!(MessageType::from_tag("unknown"), None);
    assert_eq!(MessageType::from_tag(""), None);
    assert_eq!(MessageType::from_tag("Hello"), None);
    let tags: HashSet<&str> = MessageType::ALL.iter().map(|kind| kind.tag()).collect();
    assert_eq!(
        tags.len(),
        MessageType::ALL.len(),
        "two types with the same tag"
    );
}

// ---------------------------------------------------------------------------------------------
// Unknown fields, repeated keys, unknown types
// ---------------------------------------------------------------------------------------------

#[test]
fn unknown_fields_are_ignored() {
    for (message, text) in golden() {
        let mut value: Value = serde_json::from_str(&text).unwrap();
        value["zz_future"] = json!({"nested": [1, {"deep": null}], "text": "anything"});
        value["a_first"] = json!("sorted before every other key");
        value["T"] = json!("not the type");
        assert_eq!(read_value(&value).as_ref(), Ok(&message), "{value}");
    }
    // Inside a nested object too.
    let text = format!(
        r#"{{"t":"enroll","invite":"{INVITE}","declared":{{"hostname":"h","os":"o","hermes_version":"v","arch":"x86_64"}}}}"#
    );
    assert!(matches!(read(&text), Ok(Message::Enroll(_))));
    // And in a different order, with white space around.
    let text = " {\n  \"purpose\": \"channel\",\n  \"connector\": \"0.1.0\",\n  \"proto\": [1, 1],\n  \"t\": \"hello\"\n}\n";
    assert_eq!(read(text), Ok(hello(Purpose::Channel)));
}

#[test]
fn a_repeated_key_is_refused_in_every_message() {
    for (message, text) in golden() {
        let tag = message.kind().unwrap().tag();
        // The type itself, twice.
        let repeated = format!(r#"{},"t":"{tag}"}}"#, &text[..text.len() - 1]);
        assert_eq!(read(&repeated), Err(DecodeError::RepeatedKey), "{repeated}");
        // An unknown field, twice.
        let repeated = format!(r#"{},"x":1,"x":1}}"#, &text[..text.len() - 1]);
        assert_eq!(read(&repeated), Err(DecodeError::RepeatedKey), "{repeated}");
        // The same text with the field once is fine.
        let once = format!(r#"{},"x":1}}"#, &text[..text.len() - 1]);
        assert_eq!(read(&once).as_ref(), Ok(&message), "{once}");
    }
}

#[test]
fn a_repeated_key_is_refused_wherever_it_is() {
    let cases = [
        // Two readers could each believe a different value.
        r#"{"t":"present","credential":null,"credential":"AQID"}"#.to_owned(),
        r#"{"t":"present","credential":"AQID","credential":null}"#.to_owned(),
        r#"{"t":"ping","n":1,"n":2}"#.to_owned(),
        r#"{"t":"ping","t":"pong","n":1}"#.to_owned(),
        r#"{"t":"hello","t":"enroll","proto":[1,1],"connector":"x","purpose":"channel"}"#.to_owned(),
        r#"{"t":"result","id":1,"ok":true,"ok":false,"content":{}}"#.to_owned(),
        // Nested, at any depth.
        r#"{"t":"call","id":1,"tool":"hermes_send_message","args":{"to":"ana","to":"bento","text":"x"}}"#
            .to_owned(),
        r#"{"t":"call","id":1,"tool":"arauto_status","args":{"a":{"b":{"c":1,"c":2}}}}"#.to_owned(),
        r#"{"t":"call","id":1,"tool":"arauto_status","args":{"list":[1,{"k":1,"k":1}]}}"#.to_owned(),
        r#"{"t":"result","id":1,"ok":true,"content":{"x":[[{"y":1,"y":2}]]}}"#.to_owned(),
        format!(
            r#"{{"t":"enroll","invite":"{INVITE}","declared":{{"hostname":"a","hostname":"b","os":"o","hermes_version":"v"}}}}"#
        ),
        // In a field nobody reads.
        r#"{"t":"ping","n":1,"future":{"a":1,"a":1}}"#.to_owned(),
        // In a message of unknown type, which would otherwise be ignored.
        r#"{"t":"future","a":1,"a":2}"#.to_owned(),
        r#"{"t":"future","t":"ping","n":1}"#.to_owned(),
        // Written with an escape, it is still the same key.
        r#"{"t":"ping","n":1,"n":2}"#.to_owned(),
    ];
    for text in cases {
        assert_eq!(read(&text), Err(DecodeError::RepeatedKey), "{text}");
    }
    // Keys that only look alike are different keys.
    assert_eq!(
        read(r#"{"t":"ping","n":1,"N":2," n":3}"#),
        Ok(Message::Ping(Ping { n: 1 }))
    );
}

/// The strict reader on its own, for JSON that is not a message of the channel: the same
/// refusals, and any value, with or without `"t"`.
#[test]
fn the_strict_reader_takes_any_value_with_the_rules_of_a_message() {
    assert_eq!(
        read_strict(br#"{"v":1,"op":"tools","x":[1,{"y":null}]}"#),
        Ok(json!({"v":1,"op":"tools","x":[1,{"y":null}]}))
    );
    assert_eq!(read_strict(b" [1, 2] \n"), Ok(json!([1, 2])));
    assert_eq!(read_strict(b"7"), Ok(json!(7)));
    for (text, error) in [
        (r#"{"v":1,"v":2}"#.to_owned(), DecodeError::RepeatedKey),
        (
            r#"{"a":[{"b":1,"b":1}]}"#.to_owned(),
            DecodeError::RepeatedKey,
        ),
        (r#"{"v":1} {"v":1}"#.to_owned(), DecodeError::Json),
        ("{".to_owned(), DecodeError::Json),
        (String::new(), DecodeError::Json),
        (
            format!(
                "{}0{}",
                "[".repeat(MESSAGE_DEPTH_MAX + 1),
                "]".repeat(MESSAGE_DEPTH_MAX + 1)
            ),
            DecodeError::TooDeep,
        ),
        (
            format!("[{}0]", "0,".repeat(MESSAGE_VALUES_MAX)),
            DecodeError::TooManyValues,
        ),
    ] {
        assert_eq!(read_strict(text.as_bytes()), Err(error), "{text:.40}");
    }
    // Exactly at the ceilings it is read.
    let deepest = format!(
        "{}0{}",
        "[".repeat(MESSAGE_DEPTH_MAX),
        "]".repeat(MESSAGE_DEPTH_MAX)
    );
    assert!(read_strict(deepest.as_bytes()).is_ok());
    let widest = format!("[{}0]", "0,".repeat(MESSAGE_VALUES_MAX - 2));
    assert!(read_strict(widest.as_bytes()).is_ok());
}

#[test]
fn a_message_of_unknown_type_is_its_own_variant() {
    for text in [
        r#"{"t":"future_message"}"#,
        r#"{"t":"future_message","x":[1,2,3],"y":{"z":null}}"#,
        r#"{"t":""}"#,
        r#"{"t":"HELLO","proto":[1,1],"connector":"x","purpose":"channel"}"#,
        r#"{"t":"hello ","proto":[1,1],"connector":"x","purpose":"channel"}"#,
        r#"{"t":"ping\u0000","n":1}"#,
    ] {
        assert_eq!(
            decode(text.as_bytes(), false),
            Ok(Message::Unknown),
            "{text}"
        );
        assert_eq!(
            decode(text.as_bytes(), true),
            Ok(Message::Unknown),
            "{text}"
        );
    }
}

#[test]
fn what_is_not_a_message_is_refused() {
    let no_type = [
        r#"{}"#,
        r#"{"x":1}"#,
        r#"{"t":1}"#,
        r#"{"t":null}"#,
        r#"{"t":["hello"]}"#,
        r#"{"t":{"t":"hello"}}"#,
        r#"{"T":"hello"}"#,
        r#"{"type":"hello"}"#,
    ];
    for text in no_type {
        assert_eq!(read(text), Err(DecodeError::NoType), "{text}");
    }
    let not_an_object = [
        r#"[]"#,
        r#"["hello",[1,1],"0.1.0","channel"]"#,
        r#"[{"t":"ping","n":1}]"#,
        r#""hello""#,
        "1",
        "null",
        "true",
    ];
    for text in not_an_object {
        assert_eq!(read(text), Err(DecodeError::NotAnObject), "{text}");
    }
    let not_json = [
        "",
        " ",
        "{",
        r#"{"t":"ping","n":1"#,
        r#"{"t":"ping","n":1}{"t":"ping","n":2}"#,
        r#"{"t":"ping","n":1} x"#,
        r#"{"t":"ping","n":1},"#,
        r#"{'t':'ping','n':1}"#,
        r#"{"t":"ping","n":1,}"#,
        r#"{t:"ping",n:1}"#,
        "// comment\n{\"t\":\"ping\",\"n\":1}",
        "\u{feff}{\"t\":\"ping\",\"n\":1}",
        r#"{"t":"ping","n":NaN}"#,
        r#"{"t":"ping","n":Infinity}"#,
        r#"{"t":"ping","n":1e999}"#,
        r#"{"t":"ping","n":01}"#,
        r#"{"t":"ping","n":0x1}"#,
        "{\"t\":\"pi\nng\",\"n\":1}",
        r#"{"t":"ping","n":1,"x":"\ud800"}"#,
    ];
    for text in not_json {
        assert_eq!(
            decode(text.as_bytes(), true),
            Err(DecodeError::Json),
            "{text:?}"
        );
    }
    // Bytes that are not UTF-8, inside and outside a string.
    assert_eq!(
        decode(b"{\"t\":\"ping\",\"n\":1,\"x\":\"\xff\"}", true),
        Err(DecodeError::Json)
    );
    assert_eq!(decode(b"\xff\xfe{}", false), Err(DecodeError::Json));
    assert_eq!(decode(&[0u8; 16], false), Err(DecodeError::Json));
    // White space after the object is not a second value.
    assert_eq!(
        read("{\"t\":\"ping\",\"n\":1}\n \t"),
        Ok(Message::Ping(Ping { n: 1 }))
    );
}

#[test]
fn deep_nesting_is_an_error_and_not_a_crash() {
    for depth in [200usize, 10_000, 100_000] {
        let arrays = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        let text = format!(r#"{{"t":"ping","n":1,"x":{arrays}}}"#);
        assert_eq!(read(&text), Err(DecodeError::TooDeep), "{depth} arrays");
        let objects = format!("{}1{}", r#"{"a":"#.repeat(depth), "}".repeat(depth));
        let text = format!(r#"{{"t":"call","id":1,"tool":"arauto_status","args":{objects}}}"#);
        assert_eq!(read(&text), Err(DecodeError::TooDeep), "{depth} objects");
        // Not even in a message nobody would read.
        let text = format!(r#"{{"t":"future","x":{arrays}}}"#);
        assert_eq!(
            read(&text),
            Err(DecodeError::TooDeep),
            "{depth} arrays, unknown type"
        );
    }
}

/// A text with `arrays` arrays one inside the other, around a zero.
fn nested_arrays(arrays: usize) -> String {
    format!("{}0{}", "[".repeat(arrays), "]".repeat(arrays))
}

#[test]
fn a_message_has_a_ceiling_of_nesting() {
    assert_eq!(MESSAGE_DEPTH_MAX, 64);
    // The message is the first level; 63 arrays inside it reach the ceiling.
    let deepest = format!(
        r#"{{"t":"ping","n":1,"x":{}}}"#,
        nested_arrays(MESSAGE_DEPTH_MAX - 1)
    );
    assert_eq!(read(&deepest), Ok(Message::Ping(Ping { n: 1 })));
    let too_deep = format!(
        r#"{{"t":"ping","n":1,"x":{}}}"#,
        nested_arrays(MESSAGE_DEPTH_MAX)
    );
    assert_eq!(read(&too_deep), Err(DecodeError::TooDeep));

    // The same inside the arguments of a call, which sit one level further in.
    let call = |arrays: usize| {
        format!(
            r#"{{"t":"call","id":1,"tool":"arauto_status","args":{{"x":{}}}}}"#,
            nested_arrays(arrays)
        )
    };
    assert!(read(&call(MESSAGE_DEPTH_MAX - 2)).is_ok());
    assert_eq!(
        read(&call(MESSAGE_DEPTH_MAX - 1)),
        Err(DecodeError::TooDeep)
    );
    // Objects count like arrays.
    let objects = |depth: usize| {
        format!(
            r#"{{"t":"future","x":{}0{}}}"#,
            r#"{"a":"#.repeat(depth),
            "}".repeat(depth)
        )
    };
    assert_eq!(read(&objects(MESSAGE_DEPTH_MAX - 1)), Ok(Message::Unknown));
    assert_eq!(read(&objects(MESSAGE_DEPTH_MAX)), Err(DecodeError::TooDeep));
}

/// A message of unknown type with `values` JSON values in all: the object, its `t`, one array
/// and what the array holds.
fn message_with_values(values: usize) -> String {
    let zeros = vec!["0"; values - 3].join(",");
    format!(r#"{{"t":"future","x":[{zeros}]}}"#)
}

#[test]
fn a_message_has_a_ceiling_of_values() {
    assert_eq!(MESSAGE_VALUES_MAX, 100_000);
    assert_eq!(
        read(&message_with_values(MESSAGE_VALUES_MAX)),
        Ok(Message::Unknown)
    );
    assert_eq!(
        read(&message_with_values(MESSAGE_VALUES_MAX + 1)),
        Err(DecodeError::TooManyValues)
    );
    // Every kind of value counts, wherever it is.
    for value in ["0", "null", "true", r#""""#, "[]", "{}", "-1", "1.5"] {
        let items = vec![value; MESSAGE_VALUES_MAX].join(",");
        let text = format!(r#"{{"t":"ping","n":1,"x":[{items}]}}"#);
        assert_eq!(read(&text), Err(DecodeError::TooManyValues), "{value}");
    }
    let entries: Vec<String> = (0..MESSAGE_VALUES_MAX)
        .map(|i| format!(r#""k{i}":0"#))
        .collect();
    let text = format!(r#"{{"t":"ping","n":1,"x":{{{}}}}}"#, entries.join(","));
    assert_eq!(read(&text), Err(DecodeError::TooManyValues));
}

/// The largest message the connector may send, made of nothing but tiny values, would need
/// dozens of times its size to be read. It is refused long before that.
#[test]
fn a_message_of_tiny_values_cannot_ask_for_a_lot_of_memory() {
    let prefix = r#"{"t":"call","id":1,"tool":"arauto_status","args":{"x":["#;
    let suffix = "0]}}";
    let zeros = (CONNECTOR_MESSAGE_MAX - prefix.len() - suffix.len()) / 2;
    let text = format!("{prefix}{}{suffix}", "0,".repeat(zeros));
    assert!(text.len() <= CONNECTOR_MESSAGE_MAX);
    assert!(text.len() > CONNECTOR_MESSAGE_MAX - 4);
    assert_eq!(read(&text), Err(DecodeError::TooManyValues));
    // The same bytes as one long text are a single value, and go through.
    let long_text = "0".repeat(CONNECTOR_MESSAGE_MAX - 100);
    let text =
        format!(r#"{{"t":"call","id":1,"tool":"arauto_status","args":{{"x":"{long_text}"}}}}"#);
    assert!(matches!(read(&text), Ok(Message::Call(_))));
}

#[test]
fn the_sender_refuses_what_the_receiver_would_refuse() {
    let call = |args: Value| {
        Message::Call(Call {
            id: call_id(1),
            tool: ToolName::parse("arauto_status").unwrap(),
            args: payload(args),
        })
    };
    let result = |content: Value| {
        Message::Result(CallResult {
            id: call_id(1),
            outcome: CallOutcome::Ok(payload(content)),
        })
    };

    // Values. Besides the array, a call has six: the message, `t`, `id`, `tool`, `args` and
    // the array itself. A result has the same six, with `ok` and `content`.
    let most = MESSAGE_VALUES_MAX - 6;
    for build in [&call as &dyn Fn(Value) -> Message, &result] {
        let fits = build(json!({"x": vec![0u8; most]}));
        let bytes = encode(&fits).unwrap();
        assert_eq!(
            decode(&bytes, false).as_ref(),
            Ok(&fits),
            "the largest that fits"
        );
        let over = build(json!({"x": vec![0u8; most + 1]}));
        assert_eq!(encode(&over), Err(EncodeError::TooManyValues));
        // Had it been sent anyway, the other side would have closed the connection.
        let zeros = vec!["0"; most + 1].join(",");
        let kind = if matches!(fits, Message::Call(_)) {
            format!(r#"{{"t":"call","id":1,"tool":"arauto_status","args":{{"x":[{zeros}]}}}}"#)
        } else {
            format!(r#"{{"t":"result","id":1,"ok":true,"content":{{"x":[{zeros}]}}}}"#)
        };
        assert_eq!(read(&kind), Err(DecodeError::TooManyValues));
    }

    // Nesting. The arguments are the second level of the message.
    let nested = |arrays: usize| -> Value { serde_json::from_str(&nested_arrays(arrays)).unwrap() };
    for build in [&call as &dyn Fn(Value) -> Message, &result] {
        let fits = build(json!({"x": nested(MESSAGE_DEPTH_MAX - 2)}));
        let bytes = encode(&fits).unwrap();
        assert_eq!(
            decode(&bytes, false).as_ref(),
            Ok(&fits),
            "the deepest that fits"
        );
        let over = build(json!({"x": nested(MESSAGE_DEPTH_MAX - 1)}));
        assert_eq!(encode(&over), Err(EncodeError::TooDeep));
    }
    assert_eq!(
        read(&format!(
            r#"{{"t":"result","id":1,"ok":true,"content":{{"x":{}}}}}"#,
            nested_arrays(MESSAGE_DEPTH_MAX - 1)
        )),
        Err(DecodeError::TooDeep)
    );

    // A refusal carries no content, and nothing to measure.
    let refused = Message::Result(CallResult {
        id: call_id(1),
        outcome: CallOutcome::Refused {
            code: ErrorCode::Ar709,
            retry_after: None,
        },
    });
    assert!(encode(&refused).is_ok());
}

/// Measuring what is about to be sent must not itself be a way to bring the sender down: a
/// value built in memory, deeper than any stack, is refused without recursion.
#[test]
fn a_value_deeper_than_the_stack_is_refused_without_overflow() {
    let mut value = json!(0);
    for _ in 0..200_000 {
        value = Value::Array(vec![value]);
    }
    // The object is built by hand: the macro of the JSON library would recurse on the way in.
    let mut args = Map::new();
    args.insert("x".to_owned(), value);
    let message = Message::Call(Call {
        id: call_id(1),
        tool: ToolName::parse("arauto_status").unwrap(),
        args: Payload::new(args),
    });
    assert_eq!(encode(&message), Err(EncodeError::TooDeep));
    // Dropping such a value recurses inside the JSON library: take it apart by hand.
    let Message::Call(call) = message else {
        panic!("built as a call above")
    };
    let mut rest = call.args.into_map().remove("x").unwrap();
    while let Value::Array(mut items) = rest {
        rest = items.pop().unwrap();
    }
}

// ---------------------------------------------------------------------------------------------
// CONTROL
// ---------------------------------------------------------------------------------------------

#[test]
fn only_heartbeat_and_check_are_control() {
    let control: Vec<&str> = MessageType::ALL
        .iter()
        .filter(|kind| kind.is_control())
        .map(|kind| kind.tag())
        .collect();
    assert_eq!(control, ["check", "check_ok", "ping", "pong"]);
    for (message, _) in golden() {
        assert_eq!(message.is_control(), message.kind().unwrap().is_control());
    }
}

#[test]
fn the_control_flag_must_match_the_type() {
    for (message, text) in golden() {
        let kind = message.kind().unwrap();
        assert_eq!(
            decode(text.as_bytes(), kind.is_control()).as_ref(),
            Ok(&message)
        );
        assert_eq!(
            decode(text.as_bytes(), !kind.is_control()),
            Err(DecodeError::ControlFlag(kind)),
            "{text}"
        );
    }
}

#[test]
fn a_waiting_device_only_sends_four_types() {
    let allowed: Vec<&str> = MessageType::ALL
        .iter()
        .filter(|kind| kind.allowed_while_waiting())
        .map(|kind| kind.tag())
        .collect();
    assert_eq!(allowed, ["check_ok", "ping", "pong", "leave"]);
    // Everything a waiting device may send is something a connector sends.
    for kind in MessageType::ALL {
        if kind.allowed_while_waiting() {
            assert!(kind.sent_by(Side::Connector), "{kind}");
        }
    }
}

#[test]
fn each_type_is_sent_by_the_side_the_document_says() {
    let by = |side: Side| -> Vec<&'static str> {
        MessageType::ALL
            .iter()
            .filter(|kind| kind.sent_by(side))
            .map(|kind| kind.tag())
            .collect()
    };
    assert_eq!(
        by(Side::Connector),
        [
            "hello",
            "enroll",
            "present",
            "check_ok",
            "saved",
            "call",
            "notice_ack",
            "ping",
            "pong",
            "leave"
        ]
    );
    assert_eq!(
        by(Side::Hub),
        [
            "hello_ok",
            "enrolled",
            "state",
            "check",
            "credential",
            "session",
            "tools",
            "result",
            "notice",
            "rotate",
            "rotated",
            "ping",
            "pong",
            "left",
            "closing"
        ]
    );
    for kind in MessageType::ALL {
        assert!(
            kind.sent_by(Side::Connector) || kind.sent_by(Side::Hub),
            "{kind}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The limits of each field
// ---------------------------------------------------------------------------------------------

/// Asserts that `value` is refused as a message of type `kind` with an invalid field.
fn assert_invalid(kind: MessageType, value: &Value) {
    assert_eq!(
        read_value(value),
        Err(DecodeError::InvalidField(kind)),
        "{value}"
    );
}

/// A valid message of each type, as JSON, to be broken one field at a time.
fn base(kind: MessageType) -> Value {
    let text = golden()
        .into_iter()
        .find(|(message, _)| message.kind() == Some(kind))
        .map(|(_, text)| text)
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn with(kind: MessageType, key: &str, value: Value) -> Value {
    let mut message = base(kind);
    message[key] = value;
    message
}

fn without(kind: MessageType, key: &str) -> Value {
    let mut message = base(kind);
    message.as_object_mut().unwrap().remove(key);
    message
}

#[test]
fn every_field_of_every_message_is_required() {
    for kind in MessageType::ALL {
        let message = base(*kind);
        for key in message
            .as_object()
            .unwrap()
            .keys()
            .filter(|key| *key != "t")
        {
            // `outdated`, `code` of a state that is not ended and `retry_after_s` are optional;
            // the base messages do not have them.
            if *kind == MessageType::Notice && key != "id" {
                // The pointer of a notice is for the connector to validate: without one of its
                // fields the message is still read, and then rejected without closing.
                let Ok(Message::Notice(notice)) = read_value(&without(*kind, key)) else {
                    panic!("a notice without {key} was not read");
                };
                assert_eq!(notice.validate(), Err(Rejected), "{key}");
                continue;
            }
            assert_invalid(*kind, &without(*kind, key));
        }
    }
}

#[test]
fn hello_limits() {
    let kind = MessageType::Hello;
    assert!(read_value(&with(kind, "proto", json!([1, 7]))).is_ok());
    assert!(read_value(&with(kind, "proto", json!([4294967295u32, 4294967295u32]))).is_ok());
    for proto in [
        json!([0, 1]),
        json!([2, 1]),
        json!([0, 0]),
        json!([1]),
        json!([]),
        json!([1, 1, 1]),
        json!([1, 4294967296u64]),
        json!([-1, 1]),
        json!([1.0, 1.0]),
        json!(["1", "1"]),
        json!(1),
        json!({"min": 1, "max": 1}),
        json!(null),
    ] {
        assert_invalid(kind, &with(kind, "proto", proto));
    }
    // The program version is declared text: up to 128 characters, no control character.
    assert!(read_value(&with(kind, "connector", json!("v".repeat(128)))).is_ok());
    assert!(read_value(&with(kind, "connector", json!("\u{e9}".repeat(128)))).is_ok());
    assert!(read_value(&with(kind, "connector", json!(""))).is_ok());
    for connector in [
        json!("v".repeat(129)),
        json!("\u{e9}".repeat(129)),
        json!("1.0\u{1b}[31m"),
        json!("1.0\n"),
        json!("1.0\u{0}"),
        json!("1.0\u{7f}"),
        json!("1.0\u{85}"),
        // Shown in one line, in the order it was written, with nothing hidden.
        json!("a\u{202e}b"),
        json!("a\u{202a}b"),
        json!("a\u{2066}b"),
        json!("a\u{2069}b"),
        json!("a\u{e0001}b"),
        json!("a\u{e007f}b"),
        json!("a\u{2028}b"),
        json!("a\u{2029}b"),
        json!(1),
        json!(null),
        json!(["0.1.0"]),
    ] {
        assert_invalid(kind, &with(kind, "connector", connector));
    }
    for purpose in [
        json!("admin"),
        json!("Channel"),
        json!(""),
        json!(1),
        json!(null),
        json!({"channel": null}),
        json!(["channel"]),
    ] {
        assert_invalid(kind, &with(kind, "purpose", purpose));
    }
}

/// The fields that take one of a few words are read from the word and from nothing else. The
/// reader the JSON library derives for an enumeration also takes `{"word": null}`, which would
/// be a second way to write a message, three of them frozen forever.
#[test]
fn a_word_field_is_read_only_from_its_text() {
    let cases: [(MessageType, &str, &[&str]); 4] = [
        (
            MessageType::Hello,
            "purpose",
            &["enroll", "channel", "leave"],
        ),
        (
            MessageType::State,
            "state",
            &["waiting", "approved", "active", "ended"],
        ),
        (
            MessageType::Left,
            "status",
            &["closed", "already_closed", "unknown"],
        ),
        (
            MessageType::NoticeAck,
            "outcome",
            &["accepted", "duplicate", "failed", "throttled", "rejected"],
        ),
    ];
    for (kind, key, words) in cases {
        for word in words {
            // The control of this test: the word itself is read. (`ended` needs its code.)
            let mut message = with(kind, key, json!(word));
            message["code"] = json!("AR-502");
            assert!(read_value(&message).is_ok(), "{message}");
            for other_form in [
                json!({ *word: null }),
                json!({ *word: [] }),
                json!({ *word: {} }),
                json!({ *word: *word }),
                json!([*word]),
                json!([[*word]]),
                json!({ "value": *word }),
            ] {
                let mut message = with(kind, key, other_form);
                message["code"] = json!("AR-502");
                assert_invalid(kind, &message);
            }
        }
    }
    // The exact messages of the review.
    for (kind, text) in [
        (
            MessageType::Hello,
            r#"{"t":"hello","proto":[1,1],"connector":"x","purpose":{"channel":null}}"#.to_owned(),
        ),
        (
            MessageType::State,
            r#"{"t":"state","state":{"active":null}}"#.to_owned(),
        ),
        (
            MessageType::State,
            r#"{"t":"state","state":{"ended":null},"code":"AR-502"}"#.to_owned(),
        ),
        (
            MessageType::Left,
            r#"{"t":"left","status":{"closed":null}}"#.to_owned(),
        ),
        (
            MessageType::NoticeAck,
            format!(r#"{{"t":"notice_ack","id":"{NOTICE_ID}","outcome":{{"accepted":null}}}}"#),
        ),
        (
            MessageType::Closing,
            r#"{"t":"closing","code":{"AR-502":null}}"#.to_owned(),
        ),
    ] {
        assert_eq!(read(&text), Err(DecodeError::InvalidField(kind)), "{text}");
    }
    // The words themselves, both ways.
    for purpose in [Purpose::Enroll, Purpose::Channel, Purpose::Leave] {
        assert_eq!(Purpose::parse(purpose.as_str()), Some(purpose));
    }
    for status in [
        LeftStatus::Closed,
        LeftStatus::AlreadyClosed,
        LeftStatus::Unknown,
    ] {
        assert_eq!(LeftStatus::parse(status.as_str()), Some(status));
    }
    assert_eq!(Purpose::parse("Channel"), None);
    assert_eq!(LeftStatus::parse(""), None);
}

#[test]
fn hello_ok_limits() {
    let kind = MessageType::HelloOk;
    assert_eq!(
        read(r#"{"t":"hello_ok","proto":2,"outdated":false}"#),
        Ok(Message::HelloOk(HelloOk::Agreed(2)))
    );
    assert_eq!(
        read(r#"{"t":"hello_ok","proto":4294967295}"#),
        Ok(Message::HelloOk(HelloOk::Agreed(u32::MAX)))
    );
    for text in [
        // Version zero only exists together with `outdated`.
        r#"{"t":"hello_ok","proto":0}"#,
        r#"{"t":"hello_ok","proto":0,"outdated":false}"#,
        r#"{"t":"hello_ok","proto":1,"outdated":true}"#,
        r#"{"t":"hello_ok","proto":-1}"#,
        r#"{"t":"hello_ok","proto":"1"}"#,
        r#"{"t":"hello_ok","proto":1.0}"#,
        r#"{"t":"hello_ok","proto":4294967296}"#,
        r#"{"t":"hello_ok","proto":0,"outdated":"true"}"#,
        r#"{"t":"hello_ok","proto":0,"outdated":1}"#,
        r#"{"t":"hello_ok","proto":0,"outdated":null}"#,
        r#"{"t":"hello_ok","outdated":true}"#,
    ] {
        assert_eq!(read(text), Err(DecodeError::InvalidField(kind)), "{text}");
    }
    // What the reader refuses the writer does not send.
    assert_eq!(
        encode(&Message::HelloOk(HelloOk::Agreed(0))),
        Err(EncodeError::VersionZero)
    );
    assert!(encode(&Message::HelloOk(HelloOk::Agreed(1))).is_ok());
    assert!(encode(&Message::HelloOk(HelloOk::Outdated)).is_ok());
}

/// The hub answers with the highest version in common: a version the connector did not offer
/// comes from a hub that is broken or hostile, and the connector has how to tell.
#[test]
fn the_connector_checks_the_agreed_version_against_what_it_offered() {
    let range = |min: u32, max: u32| ProtoRange::new(min, max).unwrap();
    let offered = range(2, 4);
    for version in [2u32, 3, 4] {
        assert!(offered.contains(version), "{version}");
        assert!(HelloOk::Agreed(version).fits(offered), "{version}");
    }
    for version in [0u32, 1, 5, 6, 4_000_000_000, u32::MAX] {
        assert!(!offered.contains(version), "{version}");
        assert!(!HelloOk::Agreed(version).fits(offered), "{version}");
    }
    assert!(HelloOk::Outdated.fits(offered));
    assert!(HelloOk::Outdated.fits(ProtoRange::CURRENT));
    assert!(ProtoRange::CURRENT.contains(PROTOCOL_VERSION));
    assert!(!ProtoRange::CURRENT.contains(0));
    assert!(!ProtoRange::CURRENT.contains(2));
    assert!(range(u32::MAX, u32::MAX).contains(u32::MAX));
    assert!(range(1, u32::MAX).contains(1));

    // The message of the review is read, as any version from 1 is, and then does not fit.
    let Ok(Message::HelloOk(answer)) = read(r#"{"t":"hello_ok","proto":4000000000}"#) else {
        panic!("not read");
    };
    assert_eq!(answer, HelloOk::Agreed(4_000_000_000));
    assert!(!answer.fits(ProtoRange::CURRENT));
    // Whatever the hub negotiates fits what the connector offered.
    let hub = range(3, 5);
    for (min, max) in [(3, 5), (1, 3), (1, 4), (4, 4), (5, 9), (1, 9)] {
        let Negotiation::Agreed(version) = hub.negotiate(range(min, max)) else {
            panic!("no version in common for {min}..{max}");
        };
        assert!(HelloOk::Agreed(version).fits(range(min, max)));
        assert!(hub.contains(version));
    }
}

#[test]
fn enroll_limits() {
    let kind = MessageType::Enroll;
    for invite in [
        json!(INVITE.to_ascii_uppercase()),
        json!(&INVITE[..55]),
        json!(format!("{INVITE}a")),
        json!(format!("arc2{}", &INVITE[4..])),
        json!(format!("{}=", &INVITE[..55])),
        json!(format!("{}b", &INVITE[..55])),
        json!(""),
        json!(5),
        json!(null),
        json!([INVITE]),
    ] {
        assert_invalid(kind, &with(kind, "invite", invite));
    }
    let declared = |hostname: Value, os: Value, hermes_version: Value| {
        with(
            kind,
            "declared",
            json!({"hostname": hostname, "os": os, "hermes_version": hermes_version}),
        )
    };
    let longest = "x".repeat(128);
    assert!(read_value(&declared(json!(longest), json!(longest), json!(longest))).is_ok());
    assert!(read_value(&declared(json!(""), json!(""), json!(""))).is_ok());
    let too_long = "x".repeat(129);
    assert_invalid(kind, &declared(json!(too_long), json!("o"), json!("v")));
    assert_invalid(kind, &declared(json!("h"), json!(too_long), json!("v")));
    assert_invalid(kind, &declared(json!("h"), json!("o"), json!(too_long)));
    assert_invalid(
        kind,
        &declared(json!("h\u{1b}]0;title\u{7}"), json!("o"), json!("v")),
    );
    assert_invalid(kind, &declared(json!(1), json!("o"), json!("v")));
    assert_invalid(kind, &declared(json!("h"), json!(null), json!("v")));
    for declared in [
        json!(["h", "o", "v"]),
        json!("h o v"),
        json!(null),
        json!({"hostname": "h", "os": "o"}),
        json!({"os": "o", "hermes_version": "v"}),
        json!({}),
    ] {
        assert_invalid(kind, &with(kind, "declared", declared));
    }
}

#[test]
fn enrolled_limits() {
    let kind = MessageType::Enrolled;
    for len in [0usize, 16, 31, 33, 64] {
        assert_invalid(
            kind,
            &with(kind, "nonce", json!(crate::base64::encode(&vec![7u8; len]))),
        );
    }
    assert!(
        read_value(&with(
            kind,
            "nonce",
            json!(crate::base64::encode(&[7u8; 32]))
        ))
        .is_ok()
    );
    for nonce in [
        json!(zeros32().trim_end_matches('=')),
        json!(1),
        json!(null),
        json!([0]),
    ] {
        assert_invalid(kind, &with(kind, "nonce", nonce));
    }
    for seconds in [1u32, 600, 86_400] {
        assert!(read_value(&with(kind, "confirm_within_s", json!(seconds))).is_ok());
    }
    for seconds in [
        json!(0),
        json!(86_401),
        json!(-5),
        json!(1.5),
        json!("600"),
        json!(null),
    ] {
        assert_invalid(kind, &with(kind, "confirm_within_s", seconds));
    }
    for enrollment in 1..=5 {
        assert!(read_value(&with(kind, "enrollment", json!(enrollment))).is_ok());
    }
    for enrollment in [
        json!(0),
        json!(6),
        json!(256),
        json!(-1),
        json!(1.0),
        json!("1"),
        json!(null),
    ] {
        assert_invalid(kind, &with(kind, "enrollment", enrollment));
    }
}

#[test]
fn state_limits() {
    let kind = MessageType::State;
    for state in [
        json!("quarantine"),
        json!("Waiting"),
        json!(""),
        json!(1),
        json!(null),
        json!({"active": null}),
        json!(["active"]),
    ] {
        assert_invalid(kind, &with(kind, "state", state));
    }
    // An ended state always says why.
    assert_eq!(
        read(r#"{"t":"state","state":"ended"}"#),
        Err(DecodeError::InvalidField(kind))
    );
    for code in [json!("AR-999"), json!("ar-502"), json!(502), json!(null)] {
        let message = json!({"t": "state", "state": "ended", "code": code});
        assert_invalid(kind, &message);
    }
    for code in ErrorCode::ALL {
        let message = json!({"t": "state", "state": "ended", "code": code.as_str()});
        assert_eq!(
            read_value(&message),
            Ok(Message::State(ChannelState::Ended(*code)))
        );
    }
    // A code on a state that is not ended decides nothing.
    assert_eq!(
        read(r#"{"t":"state","state":"waiting","code":"AR-502"}"#),
        Ok(Message::State(ChannelState::Waiting))
    );
}

#[test]
fn credential_size_limits() {
    let largest = vec![0xa5u8; CREDENTIAL_MAX_LEN];
    let too_large = vec![0xa5u8; CREDENTIAL_MAX_LEN + 1];
    let text = |bytes: &[u8]| json!(crate::base64::encode(bytes));

    // present
    let kind = MessageType::Present;
    assert_eq!(
        read_value(&with(kind, "credential", text(&largest))),
        Ok(Message::Present(Present {
            credential: Some(sealed(&largest))
        }))
    );
    assert_eq!(
        read_value(&with(kind, "credential", text(&[9]))),
        Ok(Message::Present(Present {
            credential: Some(sealed(&[9]))
        }))
    );
    assert_invalid(kind, &with(kind, "credential", text(&too_large)));
    assert_invalid(kind, &with(kind, "credential", text(&vec![1u8; 4_000])));
    assert_invalid(kind, &with(kind, "credential", json!("")));
    assert_invalid(kind, &with(kind, "credential", json!(5)));
    assert_invalid(kind, &with(kind, "credential", json!(false)));
    assert_invalid(kind, &with(kind, "credential", json!(["AQID"])));
    assert_invalid(kind, &with(kind, "credential", json!({"sealed": "AQID"})));
    // No credential is said with null, never by leaving the field out.
    assert_eq!(
        read(r#"{"t":"present"}"#),
        Err(DecodeError::InvalidField(kind))
    );

    // credential
    let kind = MessageType::Credential;
    assert_eq!(
        read_value(&with(kind, "sealed", text(&largest))),
        Ok(Message::Credential(Credential {
            generation: generation(7),
            sealed: sealed(&largest)
        }))
    );
    assert_invalid(kind, &with(kind, "sealed", text(&too_large)));
    assert_invalid(kind, &with(kind, "sealed", json!("")));
    assert_invalid(kind, &with(kind, "sealed", json!(null)));
    assert_invalid(kind, &with(kind, "sealed", json!(7)));
}

#[test]
fn binary_fields_are_strict_base64() {
    // "AQID" is the bytes 1, 2, 3; "AQI=" is 1, 2; "AQ==" is 1.
    let present = |text: &str| read_value(&json!({"t": "present", "credential": text}));
    assert!(present("AQID").is_ok());
    assert!(present("AQI=").is_ok());
    assert!(present("AQ==").is_ok());
    for text in [
        "AQI",      // missing padding
        "AQ",       // missing padding
        "AQ=",      // short padding
        "AQID=",    // padding that is not needed
        "AQID====", // a whole block of padding
        "AQ==AQ==", // two padded blocks glued together
        "AR==",     // bits left over that are not zero
        "AQJ=",     // the same, with one padding character
        "AQ-_",     // URL alphabet
        "AQ ID",    // white space
        "AQID\n",   // line break
        " AQID", "AQID\r\n", "A", "=", "AQ*D", "0x0102",
    ] {
        assert_eq!(
            present(text),
            Err(DecodeError::InvalidField(MessageType::Present)),
            "{text:?}"
        );
        let credential = json!({"t": "credential", "generation": 1, "sealed": text});
        assert_invalid(MessageType::Credential, &credential);
    }
    // Fixed-size fields are as strict.
    let good = zeros16();
    assert!(read_value(&json!({"t": "check", "nonce": good})).is_ok());
    let url_safe = data_encoding::BASE64URL.encode(&[0xfbu8; 16]);
    for text in [
        good.trim_end_matches('=').to_owned(),
        format!("{}\n", good),
        format!("{}==", "A".repeat(20)),
        url_safe,
        "00".repeat(16),
    ] {
        assert_invalid(MessageType::Check, &json!({"t": "check", "nonce": text}));
    }
}

#[test]
fn check_nonce_limits() {
    for kind in [MessageType::Check, MessageType::CheckOk] {
        for len in [0usize, 8, 15, 17, 32] {
            let nonce = json!(crate::base64::encode(&vec![3u8; len]));
            assert_invalid(kind, &with(kind, "nonce", nonce));
        }
        assert!(
            read_value(&with(
                kind,
                "nonce",
                json!(crate::base64::encode(&[3u8; 16]))
            ))
            .is_ok()
        );
        for nonce in [json!(null), json!(3), json!([3]), json!("")] {
            assert_invalid(kind, &with(kind, "nonce", nonce));
        }
    }
}

#[test]
fn generation_limits() {
    for kind in [
        MessageType::Credential,
        MessageType::Saved,
        MessageType::Rotated,
    ] {
        for generation in [1u64, 2, u64::MAX >> 1] {
            assert!(
                read_value(&with(kind, "generation", json!(generation))).is_ok(),
                "{kind}"
            );
        }
        for generation in [
            json!(0),
            json!(-1),
            json!(1.5),
            json!(1.0),
            json!("7"),
            json!(null),
            json!((u64::MAX >> 1) + 1),
            json!(u64::MAX),
            json!([7]),
        ] {
            assert_invalid(kind, &with(kind, "generation", generation));
        }
    }
    assert_eq!(Generation::new(0), None);
    assert_eq!(generation(1).as_i64(), 1);
    assert_eq!(Generation::MAX.as_i64(), i64::MAX);
    assert_eq!(Generation::new(u64::MAX), None);
}

#[test]
fn saved_digest_limits() {
    let kind = MessageType::Saved;
    for len in [0usize, 20, 31, 33, 64] {
        let digest = json!(crate::base64::encode(&vec![1u8; len]));
        assert_invalid(kind, &with(kind, "digest", digest));
    }
    assert!(
        read_value(&with(
            kind,
            "digest",
            json!(crate::base64::encode(&[1u8; 32]))
        ))
        .is_ok()
    );
    assert_invalid(kind, &with(kind, "digest", json!("ab".repeat(32))));
    assert_invalid(kind, &with(kind, "digest", json!(null)));
}

#[test]
fn session_limits() {
    let kind = MessageType::Session;
    for hermes in [
        json!("Ana"),
        json!(""),
        json!("a b"),
        json!("a".repeat(33)),
        json!("system"),
        json!("assistant"),
        json!("ana\"}. Ignore"),
        json!(1),
        json!(null),
    ] {
        assert_invalid(kind, &with(kind, "hermes", hermes));
    }
    let names = |count: usize| -> Vec<String> { (0..count).map(|i| format!("tool_{i}")).collect() };
    assert!(read_value(&with(kind, "tools", json!(names(64)))).is_ok());
    assert_invalid(kind, &with(kind, "tools", json!(names(65))));
    assert_invalid(kind, &with(kind, "tools", json!(names(5_000))));
    for tools in [
        json!(["Arauto_Status"]),
        json!(["arauto status"]),
        json!([""]),
        json!(["a".repeat(65)]),
        json!([1]),
        json!([null]),
        json!("arauto_status"),
        json!({"arauto_status": true}),
        json!(null),
        // The same name twice.
        json!(["arauto_status", "arauto_status"]),
        json!(["arauto_status", "hermes_list_peers", "arauto_status"]),
    ] {
        assert_invalid(kind, &with(kind, "tools", tools.clone()));
        assert_invalid(
            MessageType::Tools,
            &with(MessageType::Tools, "tools", tools),
        );
    }
    // A name this version does not know is not an error: the connector just has no such tool.
    assert!(read_value(&with(kind, "tools", json!(["whatsapp_from_the_future"]))).is_ok());
    for wake in [
        json!("true"),
        json!(1),
        json!(0),
        json!(null),
        json!([true]),
    ] {
        assert_invalid(kind, &with(kind, "wake", wake));
    }
}

#[test]
fn call_limits() {
    let kind = MessageType::Call;
    for id in [1u64, 2, 1 << 53, u64::MAX] {
        assert!(read_value(&with(kind, "id", json!(id))).is_ok());
    }
    for id in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(1.0),
        json!("1"),
        json!(null),
        json!([1]),
        json!(true),
    ] {
        assert_invalid(kind, &with(kind, "id", id));
    }
    for tool in [
        json!("Hermes_Send"),
        json!("hermes send"),
        json!(""),
        json!("a".repeat(65)),
        json!("../../etc/passwd"),
        json!(7),
        json!(null),
    ] {
        assert_invalid(kind, &with(kind, "tool", tool));
    }
    for args in [
        json!([]),
        json!("{}"),
        json!(null),
        json!(1),
        json!([{"to": "ana"}]),
    ] {
        assert_invalid(kind, &with(kind, "args", args));
    }
    // A well-formed name that is not in the catalog reads fine; the hub answers it with AR-711
    // instead of closing the connection.
    let message = read_value(&with(kind, "tool", json!("whatsapp_from_the_future"))).unwrap();
    let Message::Call(call) = message else {
        panic!("not a call")
    };
    assert_eq!(call.checked(), Err(ToolArgError::UnknownTool));
    assert_eq!(call.checked().unwrap_err().code(), ErrorCode::Ar711);
    // And a call of the catalog is checked argument by argument.
    let Message::Call(call) = read_value(&base(kind)).unwrap() else {
        panic!("not a call")
    };
    assert!(matches!(call.checked(), Ok(ToolCall::HermesSendMessage(_))));
}

#[test]
fn result_limits() {
    let kind = MessageType::Result;
    let ok = |text: &str| assert!(read(text).is_ok(), "{text}");
    let invalid = |text: &str| {
        assert_eq!(read(text), Err(DecodeError::InvalidField(kind)), "{text}");
    };
    ok(r#"{"t":"result","id":1,"ok":true,"content":{}}"#);
    ok(r#"{"t":"result","id":1,"ok":false,"code":"AR-704"}"#);
    ok(r#"{"t":"result","id":1,"ok":false,"code":"AR-706","retry_after_s":1}"#);
    ok(r#"{"t":"result","id":1,"ok":false,"code":"AR-706","retry_after_s":86400}"#);
    // The wait is from 1 to 86,400 seconds.
    invalid(r#"{"t":"result","id":1,"ok":false,"code":"AR-706","retry_after_s":0}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"code":"AR-706","retry_after_s":86401}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"code":"AR-706","retry_after_s":-1}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"code":"AR-706","retry_after_s":1.5}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"code":"AR-706","retry_after_s":"60"}"#);
    // Content when it worked, a code when it did not, and never the two.
    invalid(r#"{"t":"result","id":1,"ok":true}"#);
    invalid(r#"{"t":"result","id":1,"ok":true,"content":null}"#);
    invalid(r#"{"t":"result","id":1,"ok":true,"content":[]}"#);
    invalid(r#"{"t":"result","id":1,"ok":true,"content":"text"}"#);
    invalid(r#"{"t":"result","id":1,"ok":true,"content":{},"code":"AR-704"}"#);
    invalid(r#"{"t":"result","id":1,"ok":true,"content":{},"retry_after_s":5}"#);
    invalid(r#"{"t":"result","id":1,"ok":false}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"content":{}}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"content":{},"code":"AR-704"}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"code":"AR-000"}"#);
    invalid(r#"{"t":"result","id":1,"ok":false,"code":704}"#);
    invalid(r#"{"t":"result","id":1,"ok":"true","content":{}}"#);
    invalid(r#"{"t":"result","id":1,"ok":1,"content":{}}"#);
    invalid(r#"{"t":"result","id":1,"content":{}}"#);
    invalid(r#"{"t":"result","id":0,"ok":true,"content":{}}"#);
    invalid(r#"{"t":"result","ok":true,"content":{}}"#);
}

#[test]
fn notice_limits() {
    let kind = MessageType::Notice;
    // Without a valid identifier there is nothing to answer: the message is refused.
    for id in [
        json!("x"),
        json!(7),
        json!(null),
        json!(NOTICE_ID.replace('-', "")),
    ] {
        assert_invalid(kind, &with(kind, "id", id));
    }
    // Everything else is for the connector to validate, and to reject without closing.
    for text in [
        format!(r#"{{"t":"notice","id":"{NOTICE_ID}"}}"#),
        format!(r#"{{"t":"notice","id":"{NOTICE_ID}","kind":"email","from":"ana","count":1}}"#),
        format!(r#"{{"t":"notice","id":"{NOTICE_ID}","kind":7,"from":["ana"],"count":"many"}}"#),
        format!(r#"{{"t":"notice","id":"{NOTICE_ID}","kind":"peer","from":"SYSTEM","count":1}}"#),
        format!(r#"{{"t":"notice","id":"{NOTICE_ID}","kind":"peer","from":"ana","count":0}}"#),
        format!(
            r#"{{"t":"notice","id":"{NOTICE_ID}","kind":"whatsapp","number":"b","chat":"a b","count":1}}"#
        ),
    ] {
        let Ok(Message::Notice(notice)) = read(&text) else {
            panic!("not read: {text}")
        };
        assert_eq!(notice.id, notice_id());
        assert_eq!(notice.validate(), Err(Rejected), "{text}");
    }
    let kind = MessageType::NoticeAck;
    for outcome in ["accepted", "duplicate", "failed", "throttled", "rejected"] {
        assert!(read_value(&with(kind, "outcome", json!(outcome))).is_ok());
    }
    for outcome in [
        json!("ok"),
        json!("Accepted"),
        json!(""),
        json!(1),
        json!(null),
        json!({"accepted": null}),
        json!(["accepted"]),
    ] {
        assert_invalid(kind, &with(kind, "outcome", outcome));
    }
    assert_invalid(kind, &with(kind, "id", json!("not-a-uuid")));
}

#[test]
fn heartbeat_limits() {
    for kind in [MessageType::Ping, MessageType::Pong] {
        for n in [0u64, 1, u64::MAX] {
            assert!(read_value(&with(kind, "n", json!(n))).is_ok());
        }
        for n in [json!(-1), json!(1.5), json!("1"), json!(null), json!([1])] {
            assert_invalid(kind, &with(kind, "n", n));
        }
    }
}

#[test]
fn left_and_closing_limits() {
    let kind = MessageType::Left;
    for status in [
        json!("gone"),
        json!("Closed"),
        json!(""),
        json!(1),
        json!(null),
        json!({"closed": null}),
        json!(["closed"]),
    ] {
        assert_invalid(kind, &with(kind, "status", status));
    }
    let kind = MessageType::Closing;
    for code in ErrorCode::ALL {
        assert_eq!(
            read_value(&with(kind, "code", json!(code.as_str()))),
            Ok(Message::Closing(Closing { code: *code }))
        );
    }
    for code in [
        json!("AR-103"),
        json!("ar-204"),
        json!("AR-204 "),
        json!(204),
        json!(null),
        json!(""),
        json!({"AR-204": null}),
        json!(["AR-204"]),
    ] {
        assert_invalid(kind, &with(kind, "code", code));
    }
}

#[test]
fn messages_without_fields_ignore_whatever_comes() {
    assert_eq!(
        read(r#"{"t":"rotate","generation":"x","n":[1]}"#),
        Ok(Message::Rotate)
    );
    assert_eq!(
        read(r#"{"t":"leave","credential":"AQID"}"#),
        Ok(Message::Leave)
    );
}

// ---------------------------------------------------------------------------------------------
// Errors and secrets
// ---------------------------------------------------------------------------------------------

#[test]
fn errors_do_not_repeat_what_was_received() {
    let hostile = [
        r#"{"t":"hello","proto":[1,1],"connector":"0.1.0","purpose":"EVIL\u001b[31m"}"#,
        r#"{"t":"closing","code":"EVIL-code"}"#,
        r#"{"t":"session","hermes":"EVIL name","tools":[],"wake":true}"#,
        r#"{"t":"session","hermes":"ana","tools":["EVIL tool"],"wake":true}"#,
        r#"{"t":"present","credential":"EVIL not base64"}"#,
        r#"{"t":"enroll","invite":"EVIL-invite","declared":{"hostname":"h","os":"o","hermes_version":"v"}}"#,
        r#"{"t":"state","state":"EVIL"}"#,
        r#"{"t":"call","id":"EVIL","tool":"arauto_status","args":{}}"#,
        r#"{"t":"ping","n":1,"EVIL":1,"EVIL":2}"#,
        r#"["EVIL"]"#,
        r#"{"t":["EVIL"]}"#,
        r#"{"t":"ping","n":EVIL}"#,
    ];
    for text in hostile {
        let error = decode(text.as_bytes(), false)
            .or_else(|_| decode(text.as_bytes(), true))
            .expect_err("every one of these is refused");
        let shown = format!("{error} {error:?}");
        assert!(!shown.contains("EVIL"), "{shown}");
        assert!(shown.is_ascii());
        assert_eq!(error.closing_code(), ErrorCode::Ar204);
    }
    assert_eq!(
        DecodeError::InvalidField(MessageType::NoticeAck).to_string(),
        "message of type notice_ack with a missing or invalid field"
    );
    assert_eq!(
        DecodeError::ControlFlag(MessageType::Ping).to_string(),
        "message of type ping with the wrong CONTROL flag"
    );
}

#[test]
fn messages_with_a_secret_have_no_generic_serde() {
    assert_not_impl!(Message: serde::Serialize, Clone, core::fmt::Display);
    assert_not_impl!(Enroll: serde::Serialize, Clone, core::fmt::Display);
    assert_not_impl!(Present: serde::Serialize, Clone, core::fmt::Display);
    assert_not_impl!(Credential: serde::Serialize, Clone, core::fmt::Display);
}

#[test]
fn messages_do_not_print_secrets_or_content() {
    let secret: Vec<u8> = (0u8..=200).collect();
    let invite_bytes: Vec<u8> = (0u8..32).collect();
    let messages = [
        Message::Present(Present {
            credential: Some(sealed(&secret)),
        }),
        Message::Credential(Credential {
            generation: generation(3),
            sealed: sealed(&secret),
        }),
        Message::Enroll(Enroll {
            invite: counting_invite(),
            declared: Declared::default(),
        }),
    ];
    for message in &messages {
        let shown = format!("{message:?} {message:#?}");
        assert!(shown.contains("[redacted]"), "{shown}");
        assert_hides(&shown, &secret, "Debug of a message");
        assert_hides(&shown, &invite_bytes, "Debug of a message");
        assert!(!shown.contains(&INVITE[4..]), "{shown}");
        // The secret does travel: that is what the message is for.
        let wire = write(message);
        assert!(
            wire.contains(&crate::base64::encode(&secret)) || wire.contains(INVITE),
            "{wire}"
        );
    }
    // The arguments of a call and the content of a result are messages of people.
    let content = json!({"text": "CANARY private conversation", "to": "CANARY-peer"});
    let call = Message::Call(Call {
        id: call_id(1),
        tool: ToolName::parse("hermes_send_message").unwrap(),
        args: payload(content.clone()),
    });
    let result = Message::Result(CallResult {
        id: call_id(1),
        outcome: CallOutcome::Ok(payload(content)),
    });
    for message in [&call, &result] {
        let shown = format!("{message:?} {message:#?}");
        assert!(!shown.contains("CANARY"), "{shown}");
        assert!(shown.contains("Payload(2 keys)"), "{shown}");
        assert!(write(message).contains("CANARY private conversation"));
    }
}

// ---------------------------------------------------------------------------------------------
// Sizes
// ---------------------------------------------------------------------------------------------

#[test]
fn messages_that_travel_before_the_session_fit_its_limit_at_their_largest() {
    // Four-byte characters, 128 of them in each declared text.
    let widest = declared_text(&"\u{1f600}".repeat(128));
    let largest = [
        Message::Hello(Hello {
            proto: ProtoRange::new(u32::MAX, u32::MAX).unwrap(),
            connector: widest.clone(),
            purpose: Purpose::Channel,
        }),
        Message::Enroll(Enroll {
            invite: counting_invite(),
            declared: Declared {
                hostname: widest.clone(),
                os: widest.clone(),
                hermes_version: widest,
            },
        }),
        Message::Present(Present {
            credential: Some(sealed(&[0xffu8; CREDENTIAL_MAX_LEN])),
        }),
        Message::Credential(Credential {
            generation: Generation::MAX,
            sealed: sealed(&[0xffu8; CREDENTIAL_MAX_LEN]),
        }),
        Message::Enrolled(Enrolled {
            nonce: EnrollNonce::from_bytes([0xff; 32]),
            confirm_within_s: ConfirmWithin::MAX,
            enrollment: EnrollmentNumber::MAX,
        }),
        Message::Saved(Saved {
            generation: Generation::MAX,
            digest: CredentialDigest::from_bytes([0xff; 32]),
        }),
        // `session` is the last message of the phase: it has to fit with the largest list of the
        // largest names, and so with any catalog.
        Message::Session(Session {
            hermes: HermesName::parse(&"a".repeat(32)).unwrap(),
            tools: ToolList::new(
                (0..crate::limits::TOOL_LIST_MAX)
                    .map(|index| {
                        // The longest names there are, each of them once.
                        let name = format!(
                            "{}{index:02}",
                            "t".repeat(crate::limits::TOOL_NAME_MAX_LEN - 2)
                        );
                        ToolName::parse(&name).unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
            wake: false,
        }),
        Message::Session(Session {
            hermes: HermesName::parse(&"a".repeat(32)).unwrap(),
            tools: ToolList::new(
                crate::tools::CATALOG
                    .iter()
                    .map(|tool| tool.tool_name())
                    .collect(),
            )
            .unwrap(),
            wake: true,
        }),
    ];
    for message in &largest {
        let bytes = encode(message).unwrap();
        assert!(
            bytes.len() <= PRE_SESSION_MESSAGE_MAX,
            "{:?} takes {} bytes",
            message.kind(),
            bytes.len()
        );
        assert_eq!(decode(&bytes, false).as_ref(), Ok(message));
    }
}

#[test]
fn a_large_result_goes_through() {
    let items: Vec<Value> = (0..2_000)
        .map(|index| json!({"index": index, "text": "x".repeat(500), "nested": {"a": [1, 2, 3]}}))
        .collect();
    let message = Message::Result(CallResult {
        id: call_id(9),
        outcome: CallOutcome::Ok(payload(json!({"items": items, "more": true}))),
    });
    let bytes = encode(&message).unwrap();
    assert!(bytes.len() > 1_000_000);
    assert_eq!(decode(&bytes, false), Ok(message));
}

// ---------------------------------------------------------------------------------------------
// Field types on their own
// ---------------------------------------------------------------------------------------------

#[test]
fn the_tool_list_has_a_ceiling() {
    let names = |count: usize| -> Vec<ToolName> {
        (0..count)
            .map(|i| ToolName::parse(&format!("tool_{i}")).unwrap())
            .collect()
    };
    assert!(ToolList::new(names(0)).is_ok());
    assert!(ToolList::new(names(64)).is_ok());
    assert_eq!(ToolList::new(names(65)), Err(ToolListError));
    // Each name once, wherever the second one is.
    for (first, second) in [(0usize, 1usize), (0, 63), (62, 63), (10, 40)] {
        let mut repeated = names(64);
        repeated[second] = repeated[first].clone();
        assert_eq!(
            ToolList::new(repeated),
            Err(ToolListError),
            "{first} and {second}"
        );
    }
    let twice = ["arauto_status", "arauto_status"]
        .map(|name| ToolName::parse(name).unwrap())
        .to_vec();
    assert_eq!(ToolList::new(twice), Err(ToolListError));
    let list = tool_list(&["arauto_status", "hermes_list_peers"]);
    assert!(list.contains("arauto_status"));
    assert!(!list.contains("hermes_send_message"));
    assert_eq!(list.as_slice().len(), 2);
    assert_eq!(ToolList::default().as_slice().len(), 0);
}

#[test]
fn declared_text_made_to_fit() {
    assert_eq!(DeclaredText::lossy("host-1").as_str(), "host-1");
    assert_eq!(
        DeclaredText::lossy("a\u{1b}[31mb\nc\0d").as_str(),
        "a[31mbcd"
    );
    assert_eq!(
        DeclaredText::lossy(&"x".repeat(500)).as_str(),
        "x".repeat(128)
    );
    assert_eq!(
        DeclaredText::lossy(&"\u{e9}".repeat(500)).as_str(),
        "\u{e9}".repeat(128)
    );
    assert_eq!(DeclaredText::lossy("").as_str(), "");
    // What reorders, hides or breaks the line is dropped as well.
    assert_eq!(
        DeclaredText::lossy("a\u{202e}b\u{2066}c\u{e0041}d\u{2028}e\u{2029}f").as_str(),
        "abcdef"
    );
    // Whatever comes out is valid.
    for text in [
        "plain",
        "tab\there",
        "\u{7f}\u{80}\u{9f}",
        "srv\u{202e}gpj.exe",
        "\u{2028}\u{2029}\u{202a}\u{2069}\u{e0000}\u{e007f}",
        &"\u{1f600}".repeat(300),
    ] {
        let fitted = DeclaredText::lossy(text);
        assert_eq!(DeclaredText::parse(fitted.as_str()), Ok(fitted));
    }
    assert_eq!(
        DeclaredText::parse(&"x".repeat(129)),
        Err(DeclaredTextError)
    );
    assert_eq!(DeclaredText::parse("a\tb"), Err(DeclaredTextError));
    // The name shown to who approves is the name that was sent: nothing reorders it, hides
    // part of it or breaks the line.
    for c in [
        '\u{202a}',
        '\u{202b}',
        '\u{202c}',
        '\u{202d}',
        '\u{202e}',
        '\u{2066}',
        '\u{2067}',
        '\u{2068}',
        '\u{2069}',
        '\u{e0000}',
        '\u{e0001}',
        '\u{e0041}',
        '\u{e007f}',
        '\u{2028}',
        '\u{2029}',
    ] {
        for text in [format!("a{c}b"), format!("{c}"), format!("ab{c}")] {
            assert_eq!(
                DeclaredText::parse(&text),
                Err(DeclaredTextError),
                "{text:?}"
            );
        }
    }
    // The neighbours of those ranges, and what names in any language use, are text.
    for c in [
        '\u{2027}',
        '\u{202f}',
        '\u{2065}',
        '\u{206a}',
        '\u{e0080}',
        '\u{200d}',
        '\u{200f}',
        '\u{5d0}',
        '\u{e9}',
    ] {
        let text = format!("a{c}b");
        assert_eq!(DeclaredText::parse(&text).unwrap().as_str(), text);
    }
    assert_eq!(
        DeclaredText::parse(&"x".repeat(100_000)),
        Err(DeclaredTextError)
    );
}

#[test]
fn protocol_versions_are_negotiated_to_the_highest_in_common() {
    let range = |min: u32, max: u32| ProtoRange::new(min, max).unwrap();
    assert_eq!(ProtoRange::new(0, 1), None);
    assert_eq!(ProtoRange::new(2, 1), None);
    assert_eq!(ProtoRange::new(0, 0), None);
    assert_eq!(ProtoRange::CURRENT, range(1, 1));
    assert_eq!(
        (
            PROTOCOL_VERSION,
            ProtoRange::CURRENT.min(),
            ProtoRange::CURRENT.max()
        ),
        (1, 1, 1)
    );

    let hub = range(3, 5);
    assert_eq!(hub.negotiate(range(3, 5)), Negotiation::Agreed(5));
    assert_eq!(hub.negotiate(range(1, 3)), Negotiation::Agreed(3));
    assert_eq!(hub.negotiate(range(1, 4)), Negotiation::Agreed(4));
    assert_eq!(hub.negotiate(range(4, 4)), Negotiation::Agreed(4));
    assert_eq!(hub.negotiate(range(5, 9)), Negotiation::Agreed(5));
    assert_eq!(hub.negotiate(range(1, 9)), Negotiation::Agreed(5));
    assert_eq!(hub.negotiate(range(1, 2)), Negotiation::ConnectorTooOld);
    assert_eq!(hub.negotiate(range(1, 1)), Negotiation::ConnectorTooOld);
    assert_eq!(hub.negotiate(range(6, 9)), Negotiation::ConnectorTooNew);
    assert_eq!(hub.negotiate(range(6, 6)), Negotiation::ConnectorTooNew);
    assert_eq!(
        ProtoRange::CURRENT.negotiate(ProtoRange::CURRENT),
        Negotiation::Agreed(1)
    );
}

#[test]
fn check_nonces_are_drawn_and_compared() {
    let a = CheckNonce::random().unwrap();
    let b = CheckNonce::random().unwrap();
    assert_ne!(a, b);
    assert_eq!(a, CheckNonce::from_bytes(*a.as_bytes()));
    for position in 0..16 {
        let mut bytes = *a.as_bytes();
        bytes[position] ^= 1;
        assert_ne!(a, CheckNonce::from_bytes(bytes));
    }
    assert_eq!(
        format!("{:?}", CheckNonce::from_bytes([0x1f; 16])),
        format!("CheckNonce({})", "1f".repeat(16))
    );
}

#[test]
fn bounded_numbers() {
    assert_eq!(EnrollmentNumber::new(0), None);
    assert_eq!(EnrollmentNumber::new(6), None);
    assert_eq!(
        (EnrollmentNumber::MIN.get(), EnrollmentNumber::MAX.get()),
        (1, 5)
    );
    assert_eq!(ConfirmWithin::new(0), None);
    assert_eq!(ConfirmWithin::new(86_401), None);
    assert_eq!(CallId::new(0), None);
    assert_eq!(CallId::new(u64::MAX).map(CallId::get), Some(u64::MAX));
    assert_eq!(call_id(42).to_string(), "42");
}

// ---------------------------------------------------------------------------------------------
// Anything at all
// ---------------------------------------------------------------------------------------------

/// JSON without fractions: this build of the JSON library does not promise that every
/// fraction reads back identical.
fn any_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        "\\PC{0,12}".prop_map(Value::from),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::from),
            prop::collection::btree_map("\\PC{0,8}", inner, 0..6)
                .prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

fn any_object() -> impl Strategy<Value = Map<String, Value>> {
    prop::collection::btree_map("\\PC{0,8}", any_json(), 0..6)
        .prop_map(|map| map.into_iter().collect())
}

proptest! {
    #![proptest_config(crate::test_util::random_rounds(256))]

    /// Hostile input never brings the reader down: any bytes give a message or an error.
    #[test]
    fn any_bytes_are_read_without_panic(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = decode(&bytes, false);
        let _ = decode(&bytes, true);
    }

    /// The same for any JSON, under every known type.
    #[test]
    fn any_json_is_read_without_panic(mut object in any_object(), index in 0usize..23) {
        let kind = MessageType::ALL[index];
        object.insert("t".to_owned(), Value::from(kind.tag()));
        let text = Value::Object(object).to_string();
        if let Ok(message) = decode(text.as_bytes(), kind.is_control()) {
            prop_assert_eq!(message.kind(), Some(kind));
            // Whatever was accepted can be written and read again as the same message.
            let again = encode(&message).unwrap();
            prop_assert_eq!(decode(&again, kind.is_control()), Ok(message));
        }
    }

    /// Arguments and content are carried exactly, whatever JSON they are.
    #[test]
    fn payloads_are_carried_exactly(args in any_object(), id in 1u64..) {
        let call = Message::Call(Call {
            id: call_id(id),
            tool: ToolName::parse("arauto_status").unwrap(),
            args: Payload::new(args.clone()),
        });
        let bytes = encode(&call).unwrap();
        prop_assert_eq!(decode(&bytes, false), Ok(call));

        let result = Message::Result(CallResult {
            id: call_id(id),
            outcome: CallOutcome::Ok(Payload::new(args)),
        });
        let bytes = encode(&result).unwrap();
        prop_assert_eq!(decode(&bytes, false), Ok(result));
    }

    /// One key repeated anywhere in arbitrary content is always caught.
    #[test]
    fn a_repeated_key_in_any_content_is_caught(args in any_object(), key in "[a-z]{1,6}") {
        let inner = Value::Object(args).to_string();
        let text = format!(
            r#"{{"t":"call","id":1,"tool":"arauto_status","args":{{"wrap":[{inner},{{"{key}":1,"{key}":2}}]}}}}"#
        );
        prop_assert_eq!(decode(text.as_bytes(), false), Err(DecodeError::RepeatedKey));
    }
}
