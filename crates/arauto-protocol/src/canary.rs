//! The canary: a known secret is planted in every public type that can hold one, and must not
//! show up in anything the type prints.
//!
//! A secret has two ways out, both explicit and both checked here to really carry it:
//! `expose` (or `reveal`) on the secret itself, and [`crate::message::encode`] for the three
//! messages that take a secret to the other side. Everything else stays blind to it: `Debug`
//! prints `[redacted]`, and the types have neither `Display` nor a serde serializer.

use core::fmt::{Debug, Display};

use serde::Serialize;
use serde_json::json;

use crate::digest::CredentialDigest;
use crate::frame::{Reassembler, Side, Splitter, Unlimited};
use crate::invite::Invite;
use crate::message::{
    Call, CallId, CallOutcome, CallResult, Credential, Declared, Enroll, Generation, Message,
    Payload, Present, decode, encode,
};
use crate::names::MessageText;
use crate::noise::{Channel, ConnectorHandshake, HubHandshake, Opener, Sealer, StaticKeypair};
use crate::secret::{SealedCredential, Secret};
use crate::test_util::{assert_hides, hex, shows};
use crate::tools::{HERMES_SEND_MESSAGE, ToolCall, ToolName};

/// The secret planted as identity, invite and credential.
const CANARY: [u8; 32] = *b"ARAUTO-CANARY-0123456789abcdef!!";

/// A second one, for the key of the hub.
const HUB_CANARY: [u8; 32] = *b"HUB-CANARY-fedcba9876543210-hub!";

/// Content of a conversation, which is not a secret of the protocol but must stay out of logs.
const CONTENT: &str = "CONTENT-CANARY what a person wrote to another";

fn shown<T: Debug>(value: &T) -> String {
    format!("{value:?}\n{value:#?}")
}

fn secret() -> Secret<32> {
    Secret::new(CANARY)
}

fn sealed() -> SealedCredential {
    SealedCredential::from_slice(&CANARY).unwrap()
}

fn invite() -> Invite {
    Invite::from_secret(secret())
}

fn identity() -> StaticKeypair {
    StaticKeypair::from_private(secret())
}

fn hub() -> StaticKeypair {
    StaticKeypair::from_private(Secret::new(HUB_CANARY))
}

fn object(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => object,
        other => panic!("not an object: {other}"),
    }
}

fn content() -> Payload {
    Payload::new(object(
        json!({"text": CONTENT, "nested": {"again": [CONTENT]}}),
    ))
}

/// If the detector were blind, every other test here would pass for nothing.
#[test]
fn the_detector_sees_a_planted_secret() {
    let invite_text = invite().reveal();
    let leaks = [
        hex(&CANARY),
        hex(&CANARY).to_ascii_uppercase(),
        crate::base64::encode(&CANARY),
        data_encoding::BASE64URL_NOPAD.encode(&CANARY),
        String::from_utf8(CANARY.to_vec()).unwrap(),
        format!("{CANARY:?}"),
        invite_text.to_string(),
        format!("prefix {} suffix", hex(&CANARY)),
    ];
    for leak in leaks {
        assert!(
            shows(&leak, &CANARY).is_some(),
            "the detector missed {leak}"
        );
    }
    assert_eq!(shows("[redacted]", &CANARY), None);
    assert_eq!(shows(&hex(&HUB_CANARY), &CANARY), None);
}

#[test]
fn no_debug_output_shows_a_secret() {
    let identity = identity();
    let hub = hub();
    let (pending_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
    let mut outputs = vec![
        ("Secret", shown(&secret())),
        ("SealedCredential", shown(&sealed())),
        ("Invite", shown(&invite())),
        ("StaticKeypair", shown(&identity)),
        ("StaticKeypair of the hub", shown(&hub)),
        ("Option<Secret>", shown(&Some(secret()))),
        ("Vec<SealedCredential>", shown(&vec![sealed(), sealed()])),
        ("ConnectorHandshake", shown(&pending_connector)),
        ("HubHandshake", shown(&pending_hub)),
    ];
    let (third, connector) = pending_connector.finish(&second).unwrap();
    let at_hub = pending_hub.finish(&third).unwrap();
    outputs.push(("Channel of the connector", shown(&connector)));
    outputs.push(("Channel of the hub", shown(&at_hub)));
    outputs.push(("Sealer", shown(&connector.sealer)));
    outputs.push(("Opener", shown(&at_hub.opener)));

    let enroll = Enroll {
        invite: invite(),
        declared: Declared::default(),
    };
    let present = Present {
        credential: Some(sealed()),
    };
    let credential = Credential {
        generation: Generation::MIN,
        sealed: sealed(),
    };
    outputs.push(("Enroll", shown(&enroll)));
    outputs.push(("Present", shown(&present)));
    outputs.push(("Credential", shown(&credential)));
    outputs.push(("Message::Enroll", shown(&Message::Enroll(enroll))));
    outputs.push(("Message::Present", shown(&Message::Present(present))));
    outputs.push((
        "Message::Credential",
        shown(&Message::Credential(credential)),
    ));

    for (what, output) in &outputs {
        assert_hides(output, &CANARY, what);
        assert_hides(output, &HUB_CANARY, what);
    }
    // The secrets themselves print exactly the marker.
    assert_eq!(format!("{:?}", secret()), "[redacted]");
    assert_eq!(format!("{:?}", sealed()), "[redacted]");
    assert_eq!(format!("{:?}", invite()), "Invite([redacted])");
}

#[test]
fn no_error_shows_a_secret() {
    let invite_text = invite().reveal().to_string();
    let credential_text = crate::base64::encode(&CANARY);
    let mut outputs = Vec::new();

    // An invite with a typing mistake is almost the invite.
    let mut wrong_end = invite_text.clone();
    wrong_end.pop();
    wrong_end.push('b');
    for text in [
        wrong_end,
        invite_text.to_ascii_uppercase(),
        format!("{invite_text}a"),
    ] {
        let error = Invite::parse(&text).unwrap_err();
        outputs.push(("InviteError", format!("{error} {}", shown(&error))));
    }

    // Messages that fail to read with the secret inside.
    let broken = [
        format!(r#"{{"t":"present","credential":"{credential_text}!"}}"#),
        format!(r#"{{"t":"present","credential":"{credential_text}","credential":null}}"#),
        format!(r#"{{"t":"credential","generation":0,"sealed":"{credential_text}"}}"#),
        format!(r#"{{"t":"credential","generation":1,"sealed":["{credential_text}"]}}"#),
        format!(r#"{{"t":"enroll","invite":"{invite_text}","declared":"missing"}}"#),
        format!(r#"{{"t":"enroll","invite":"{invite_text}x","declared":{{}}}}"#),
        format!(r#"{{"t":"present","credential":"{credential_text}""#),
        format!(r#"{{"t":"{credential_text}","credential":"{credential_text}"}}"#),
    ];
    for text in broken {
        match decode(text.as_bytes(), false) {
            Err(error) => outputs.push(("DecodeError", format!("{error} {}", shown(&error)))),
            Ok(message) => outputs.push(("a message of unknown type", shown(&message))),
        }
    }

    // A credential that is too long is refused without being shown.
    let error = SealedCredential::new(CANARY.repeat(40)).unwrap_err();
    outputs.push((
        "CredentialLengthError",
        format!("{error} {}", shown(&error)),
    ));

    // A frame that fails to open after it was changed on the way.
    let (mut connector, mut at_hub) = channels();
    let mut frame = connector.sealer.seal(&CANARY).unwrap();
    frame[3] ^= 1;
    let error = at_hub.opener.open(&frame).unwrap_err();
    outputs.push((
        "NoiseError",
        format!("{error} {} {}", shown(&error), shown(&at_hub.opener)),
    ));

    // A handshake that fails against the wrong hub.
    let (_, first) = ConnectorHandshake::start(&identity(), identity().public()).unwrap();
    let error = HubHandshake::respond(&hub(), &first).unwrap_err();
    outputs.push((
        "NoiseError of the handshake",
        format!("{error} {}", shown(&error)),
    ));

    for (what, output) in &outputs {
        assert_hides(output, &CANARY, what);
        assert_hides(output, &HUB_CANARY, what);
        assert!(!output.contains(&invite_text[4..]), "{what}: {output}");
    }
}

fn channels() -> (Channel, Channel) {
    let identity = identity();
    let hub = hub();
    let (pending_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
    let (third, connector) = pending_connector.finish(&second).unwrap();
    (connector, pending_hub.finish(&third).unwrap())
}

#[test]
fn secret_types_have_neither_display_nor_serde() {
    assert_not_impl!(Secret<32>: Display, Serialize);
    assert_not_impl!(Secret<16>: Display, Serialize);
    assert_not_impl!(SealedCredential: Display, Serialize);
    assert_not_impl!(Invite: Display, Serialize);
    assert_not_impl!(StaticKeypair: Display, Serialize);
    assert_not_impl!(ConnectorHandshake: Display, Serialize);
    assert_not_impl!(HubHandshake: Display, Serialize);
    assert_not_impl!(Channel: Display, Serialize);
    assert_not_impl!(Sealer: Display, Serialize);
    assert_not_impl!(Opener: Display, Serialize);
    assert_not_impl!(Enroll: Display, Serialize);
    assert_not_impl!(Present: Display, Serialize);
    assert_not_impl!(Credential: Display, Serialize);
    assert_not_impl!(Message: Display, Serialize);
    assert_not_impl!(MessageText: Display, Serialize);
    assert_not_impl!(ToolCall: Display, Serialize);
}

/// What does have a serializer and is computed from a secret gives nothing of it away.
#[test]
fn what_is_derived_from_a_secret_can_be_stored() {
    let outputs = [
        serde_json::to_string(&CredentialDigest::of(&sealed())).unwrap(),
        serde_json::to_string(&invite().digest()).unwrap(),
        shown(&CredentialDigest::of(&sealed())),
        shown(&invite().digest()),
        shown(identity().public()),
        shown(hub().public()),
    ];
    for output in &outputs {
        assert_hides(output, &CANARY, "a value derived from the secret");
        assert_hides(output, &HUB_CANARY, "a value derived from the secret");
    }
}

/// Whether `needle` appears anywhere inside `bytes`.
fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

/// What goes to the websocket never carries a private key, and after the handshake it carries
/// nothing in the clear.
#[test]
fn nothing_on_the_wire_shows_a_key_or_the_content() {
    let identity = identity();
    let hub = hub();
    let (pending_connector, first) = ConnectorHandshake::start(&identity, hub.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(&hub, &first).unwrap();
    let (third, mut connector) = pending_connector.finish(&second).unwrap();
    let mut at_hub = pending_hub.finish(&third).unwrap();

    let credential = encode(&Message::Present(Present {
        credential: Some(sealed()),
    }))
    .unwrap();
    let sealed_frame = connector.sealer.seal(&credential).unwrap();
    let answer = at_hub.sealer.seal(CONTENT.as_bytes()).unwrap();

    for (what, bytes) in [
        ("the first handshake message", &first[..]),
        ("the second handshake message", &second[..]),
        ("the third handshake message", &third[..]),
        ("a sealed frame of the connector", &sealed_frame[..]),
        ("a sealed frame of the hub", &answer[..]),
    ] {
        assert!(!contains(bytes, &CANARY), "{what} carries the identity");
        assert!(
            !contains(bytes, &HUB_CANARY),
            "{what} carries the key of the hub"
        );
        assert!(
            !contains(bytes, CONTENT.as_bytes()),
            "{what} carries content in the clear"
        );
        // The identity only travels sealed: not even its public part is readable.
        assert!(
            !contains(bytes, identity.public().as_bytes()),
            "{what} shows which device is connecting"
        );
    }
    let credential_text = crate::base64::encode(&CANARY);
    assert!(
        contains(&credential, credential_text.as_bytes()),
        "the control of this test"
    );
    assert!(!contains(&sealed_frame, credential_text.as_bytes()));
    // The other side reads it all the same.
    assert_eq!(at_hub.opener.open(&sealed_frame).unwrap(), credential);
}

/// The two explicit ways out do carry the secret: without them nothing would work, and with
/// them the canary proves the tests above are looking at live secrets.
#[test]
fn the_explicit_ways_out_carry_the_secret() {
    assert_eq!(secret().expose(), &CANARY);
    assert_eq!(sealed().expose(), &CANARY);
    assert_eq!(identity().private().expose(), &CANARY);
    assert!(shows(&invite().reveal(), &CANARY).is_some());

    let messages = [
        Message::Enroll(Enroll {
            invite: invite(),
            declared: Declared::default(),
        }),
        Message::Present(Present {
            credential: Some(sealed()),
        }),
        Message::Credential(Credential {
            generation: Generation::MIN,
            sealed: sealed(),
        }),
    ];
    for message in messages {
        let wire = String::from_utf8(encode(&message).unwrap()).unwrap();
        assert!(
            shows(&wire, &CANARY).is_some(),
            "the secret did not travel: {wire}"
        );
        // And it arrives.
        assert_eq!(decode(wire.as_bytes(), false), Ok(message));
    }
}

#[test]
fn content_of_conversations_is_not_printed() {
    let text = MessageText::parse(CONTENT).unwrap();
    let call = Call {
        id: CallId::MIN,
        tool: ToolName::parse(HERMES_SEND_MESSAGE).unwrap(),
        args: content(),
    };
    let arguments = object(json!({"to": "bento", "text": CONTENT}));
    let checked = ToolCall::parse(HERMES_SEND_MESSAGE, &arguments).unwrap();
    let result = CallResult {
        id: CallId::MIN,
        outcome: CallOutcome::Ok(content()),
    };

    let bytes = encode(&Message::Result(result.clone())).unwrap();
    let mut splitter = Splitter::new(Side::Hub);
    let frames = splitter.split(&bytes, false).unwrap();
    let frame = frames.clone().next().unwrap();
    let frame_bytes = frame.to_bytes();
    let mut reassembler = Reassembler::new(Side::Hub, 60_000, Unlimited);
    let delivered = reassembler.push(&frame_bytes, 0).unwrap().unwrap();

    let mut waiting = Reassembler::new(Side::Hub, 60_000, Unlimited);
    waiting.enter_session();
    let mut first_chunk = vec![0u8, 0, 0, 0, 2];
    first_chunk.extend_from_slice(CONTENT.as_bytes());
    assert_eq!(waiting.push(&first_chunk, 0), Ok(None));

    let outputs = [
        ("MessageText", shown(&text)),
        ("Payload", shown(&content())),
        ("Call", shown(&call)),
        ("Message::Call", shown(&Message::Call(call))),
        ("ToolCall", shown(&checked)),
        ("CallResult", shown(&result)),
        ("Message::Result", shown(&Message::Result(result))),
        ("Frames", shown(&frames)),
        ("Frame", shown(&frame)),
        ("Delivered", shown(&delivered)),
        ("Reassembler", shown(&waiting)),
    ];
    for (what, output) in &outputs {
        assert!(!output.contains("CONTENT-CANARY"), "{what}: {output}");
        assert_hides(output, CONTENT.as_bytes(), what);
    }
}

/// A notice is read loosely, so that a bad one is rejected without closing the connection:
/// until it is checked its fields are whatever the hub sent. None of that is printed.
#[test]
fn a_notice_does_not_print_what_the_hub_sent() {
    let hostile = format!(
        r#"{{"t":"notice","id":"3f2b7c1e-58a4-4d6b-9e0f-a1b2c3d4e5f6","kind":"{CONTENT}","number":"{CONTENT}","chat":{{"deep":["{CONTENT}"]}},"from":"{CONTENT}","count":"{CONTENT}"}}"#
    );
    let message = decode(hostile.as_bytes(), false).unwrap();
    let Message::Notice(notice) = &message else {
        panic!("not a notice: {message:?}");
    };
    assert!(notice.validate().is_err());
    for (what, output) in [
        ("NoticeMsg", shown(notice)),
        ("Message::Notice", shown(&message)),
    ] {
        assert!(!output.contains("CONTENT-CANARY"), "{what}: {output}");
        assert_hides(&output, CONTENT.as_bytes(), what);
    }
}
