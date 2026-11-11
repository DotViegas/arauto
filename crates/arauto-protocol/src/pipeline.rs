//! The whole path of a message through every layer of the crate: written as JSON, cut in
//! frames, sealed, opened, put together and read. Each module has its own tests; these show
//! that the pieces fit the way the hub and the connector will use them.

use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::code::{EnrollNonce, VerificationCode};
use crate::digest::CredentialDigest;
use crate::errors::{ErrorCode, RetryAfter};
use crate::frame::{FrameError, Reassembler, Side, SplitError, Splitter, Unlimited};
use crate::invite::Invite;
use crate::limits::{FRAME_CHUNK_MAX, PRE_SESSION_MESSAGE_MAX, STREAM_IDLE_TIMEOUT_MS};
use crate::message::{
    Call, CallId, CallOutcome, CallResult, ChannelState, Check, CheckNonce, CheckOk, Closing,
    ConfirmWithin, Credential, Declared, DeclaredText, Enroll, Enrolled, EnrollmentNumber,
    Generation, Hello, HelloOk, Left, LeftStatus, Message, Negotiation, NoticeAck, Payload, Ping,
    Pong, Present, ProtoRange, Purpose, Rotated, Saved, Session, ToolList, decode, encode,
};
use crate::names::HermesName;
use crate::noise::{
    ConnectorHandshake, HandshakeHash, HubHandshake, NoiseError, Opener, PublicKey, Sealer,
    StaticKeypair,
};
use crate::notice::{AckOutcome, Notice, NoticeCount, NoticeId, NoticeMsg};
use crate::secret::SealedCredential;
use crate::tools::{Grants, HERMES_SEND_MESSAGE, ToolArgError, ToolCall, ToolName, released};

/// One end of a connection: what a program keeps after the handshake.
struct End {
    side: Side,
    splitter: Splitter,
    sealer: Sealer,
    opener: Opener,
    reassembler: Reassembler<Unlimited>,
    handshake_hash: HandshakeHash,
    remote_static: PublicKey,
    now_ms: i64,
}

impl End {
    fn enter_session(&mut self) {
        self.splitter.enter_session();
        self.reassembler.enter_session();
    }

    /// The frames of one message, not yet sealed.
    fn frames(&mut self, message: &Message) -> Vec<Vec<u8>> {
        let kind = message.kind().expect("only known messages are sent");
        assert!(
            kind.sent_by(self.side),
            "{kind} is not sent by {:?}",
            self.side
        );
        let bytes = encode(message).unwrap();
        self.splitter
            .split(&bytes, message.is_control())
            .unwrap()
            .map(|frame| frame.to_bytes())
            .collect()
    }

    /// Seals one frame at the moment it goes to the websocket.
    fn seal(&mut self, frame: &[u8]) -> Vec<u8> {
        self.sealer.seal(frame).unwrap()
    }

    /// Receives one websocket message. Returns a message when it completes one.
    fn receive(&mut self, websocket_message: &[u8]) -> Option<Message> {
        self.now_ms += 5;
        let frame = self.opener.open(websocket_message).unwrap();
        let delivered = self.reassembler.push(&frame, self.now_ms).unwrap()?;
        Some(decode(&delivered.payload, delivered.control).unwrap())
    }
}

/// A connection after the handshake: the connector and the hub.
fn connect(identity: &StaticKeypair, hub_keys: &StaticKeypair) -> (End, End) {
    let (pending_connector, first) =
        ConnectorHandshake::start(identity, hub_keys.public()).unwrap();
    let (pending_hub, second) = HubHandshake::respond(hub_keys, &first).unwrap();
    let (third, connector) = pending_connector.finish(&second).unwrap();
    let hub = pending_hub.finish(&third).unwrap();
    let end = |side: Side, channel: crate::noise::Channel| End {
        side,
        splitter: Splitter::new(side),
        sealer: channel.sealer,
        opener: channel.opener,
        reassembler: Reassembler::new(side.peer(), STREAM_IDLE_TIMEOUT_MS, Unlimited),
        handshake_hash: channel.handshake_hash,
        remote_static: channel.remote_static,
        now_ms: 1_000_000,
    };
    (end(Side::Connector, connector), end(Side::Hub, hub))
}

/// Sends `message` from one end to the other and returns what arrived.
fn deliver(from: &mut End, to: &mut End, message: &Message) -> Message {
    let frames = from.frames(message);
    let last = frames.len() - 1;
    let mut arrived = None;
    for (index, frame) in frames.iter().enumerate() {
        let sealed = from.seal(frame);
        let received = to.receive(&sealed);
        assert_eq!(
            received.is_some(),
            index == last,
            "delivered at frame {index} of {last}"
        );
        arrived = received;
    }
    arrived.expect("the last frame delivers the message")
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(object) => object,
        other => panic!("not an object: {other}"),
    }
}

fn hello(purpose: Purpose) -> Message {
    Message::Hello(Hello {
        proto: ProtoRange::CURRENT,
        connector: DeclaredText::lossy(env!("CARGO_PKG_VERSION")),
        purpose,
    })
}

/// What a program keeps for a connection can live in a task and move between threads.
#[test]
fn the_pieces_of_a_connection_can_move_between_tasks() {
    fn assert_send<T: Send>() {}
    assert_send::<Splitter>();
    assert_send::<Reassembler<Unlimited>>();
    assert_send::<Reassembler<std::sync::Arc<Unlimited>>>();
    assert_send::<Sealer>();
    assert_send::<Opener>();
    assert_send::<Message>();
    assert_send::<StaticKeypair>();
    assert_send::<crate::limits::Deadlines>();
}

#[test]
fn enrollment_ends_with_the_same_code_on_both_sides() {
    let identity = StaticKeypair::generate().unwrap();
    let hub_keys = StaticKeypair::generate().unwrap();

    // The hub issued an invite and kept only its digest; the text went in the block.
    let issued = Invite::generate().unwrap();
    let stored_digest = issued.digest();
    let block_text = issued.reveal();

    let (mut connector, mut hub) = connect(&identity, &hub_keys);
    assert_eq!(&hub.remote_static, identity.public());
    assert_eq!(&connector.remote_static, hub_keys.public());

    // hello and hello_ok
    let sent = hello(Purpose::Enroll);
    let Message::Hello(received) = deliver(&mut connector, &mut hub, &sent) else {
        panic!("the hub expected hello");
    };
    assert_eq!(received.purpose, Purpose::Enroll);
    let Negotiation::Agreed(version) = ProtoRange::CURRENT.negotiate(received.proto) else {
        panic!("the same build always agrees with itself");
    };
    let answer = Message::HelloOk(HelloOk::Agreed(version));
    assert_eq!(deliver(&mut hub, &mut connector, &answer), answer);

    // enroll: the connector read the invite from its arguments, strictly.
    let typed_invite = Invite::parse(&block_text).unwrap();
    let enroll = Message::Enroll(Enroll {
        invite: typed_invite,
        declared: Declared {
            hostname: DeclaredText::lossy("build-host\u{1b}[0m"),
            os: DeclaredText::lossy("linux"),
            hermes_version: DeclaredText::lossy("0.23.0"),
        },
    });
    let Message::Enroll(received) = deliver(&mut connector, &mut hub, &enroll) else {
        panic!("the hub expected enroll");
    };
    assert_eq!(
        received.invite.digest(),
        stored_digest,
        "the hub finds the invite by its digest"
    );
    assert_eq!(received.declared.hostname.as_str(), "build-host[0m");

    // enrolled: the hub draws a nonce, and another one in the rare case no code comes out.
    let (nonce, code_at_hub) = loop {
        let nonce = EnrollNonce::random().unwrap();
        if let Ok(code) = VerificationCode::derive(&nonce, &hub.handshake_hash) {
            break (nonce, code);
        }
    };
    let enrolled = Message::Enrolled(Enrolled {
        nonce,
        confirm_within_s: ConfirmWithin::new(600).unwrap(),
        enrollment: EnrollmentNumber::new(1).unwrap(),
    });
    let Message::Enrolled(received) = deliver(&mut hub, &mut connector, &enrolled) else {
        panic!("the connector expected enrolled");
    };

    // Each side computes the code on its own; the hub never sent it.
    let code_at_connector =
        VerificationCode::derive(&received.nonce, &connector.handshake_hash).unwrap();
    assert_eq!(code_at_connector, code_at_hub);
    // The connector shows it, the Hermes says it, the admin types it.
    let typed = VerificationCode::parse(&code_at_connector.to_string()).unwrap();
    assert_eq!(typed, code_at_hub);

    // Another handshake with the same nonce gives another code: the code is tied to the
    // connection that asked.
    let (other_connector, _) = connect(&identity, &hub_keys);
    if let Ok(other) = VerificationCode::derive(&nonce, &other_connector.handshake_hash) {
        assert_ne!(other, code_at_hub);
    }
}

#[test]
fn a_channel_from_hello_to_the_session_and_its_use() {
    let identity = StaticKeypair::generate().unwrap();
    let hub_keys = StaticKeypair::generate().unwrap();
    let (mut connector, mut hub) = connect(&identity, &hub_keys);

    // hello, hello_ok, state
    let sent = hello(Purpose::Channel);
    assert_eq!(deliver(&mut connector, &mut hub, &sent), sent);
    let answer = Message::HelloOk(HelloOk::Agreed(1));
    assert_eq!(deliver(&mut hub, &mut connector, &answer), answer);
    let state = Message::State(ChannelState::Approved);
    assert_eq!(deliver(&mut hub, &mut connector, &state), state);

    // present: an approved device at generation zero has no credential yet.
    let present = Message::Present(Present { credential: None });
    assert_eq!(deliver(&mut connector, &mut hub, &present), present);

    // check and check_ok, as control frames.
    let nonce = CheckNonce::random().unwrap();
    let check = Message::Check(Check { nonce });
    let Message::Check(received) = deliver(&mut hub, &mut connector, &check) else {
        panic!("the connector expected check");
    };
    let check_ok = Message::CheckOk(CheckOk {
        nonce: received.nonce,
    });
    let Message::CheckOk(received) = deliver(&mut connector, &mut hub, &check_ok) else {
        panic!("the hub expected check_ok");
    };
    assert_eq!(received.nonce, nonce);

    // credential and saved: the connector answers with the digest of what it wrote to disk.
    let issued = SealedCredential::new(vec![0x5a; 180]).unwrap();
    let pending_digest = CredentialDigest::of(&issued);
    let credential = Message::Credential(Credential {
        generation: Generation::new(1).unwrap(),
        sealed: issued,
    });
    let Message::Credential(received) = deliver(&mut hub, &mut connector, &credential) else {
        panic!("the connector expected credential");
    };
    let saved = Message::Saved(Saved {
        generation: received.generation,
        digest: CredentialDigest::of(&received.sealed),
    });
    let Message::Saved(received) = deliver(&mut connector, &mut hub, &saved) else {
        panic!("the hub expected saved");
    };
    assert_eq!(
        received.digest, pending_digest,
        "the saved credential is the pending one"
    );

    // session: the tools come from what the Hermes has.
    let grants = Grants {
        peer_any: true,
        peer_outbound: true,
        ..Grants::default()
    };
    let tools = ToolList::new(released(&grants).map(|tool| tool.tool_name()).collect()).unwrap();
    let session = Message::Session(Session {
        hermes: HermesName::parse("ana").unwrap(),
        tools: tools.clone(),
        wake: true,
    });
    // `session` is the last message of the phase before: the hub cuts it with those limits,
    // and each side switches right after writing or reading it.
    let frames = hub.frames(&session);
    hub.enter_session();
    assert_eq!(frames.len(), 1);
    let sealed = hub.seal(&frames[0]);
    let Some(Message::Session(received)) = connector.receive(&sealed) else {
        panic!("the connector expected session");
    };
    connector.enter_session();
    assert!(received.tools.contains(HERMES_SEND_MESSAGE));
    assert_eq!(received.tools, tools);

    // A call, checked again by the hub, and its result.
    let call = Message::Call(Call {
        id: CallId::new(1).unwrap(),
        tool: ToolName::parse(HERMES_SEND_MESSAGE).unwrap(),
        args: Payload::new(object(
            json!({"to": "bento", "text": "the report is ready"}),
        )),
    });
    let Message::Call(received) = deliver(&mut connector, &mut hub, &call) else {
        panic!("the hub expected call");
    };
    let Ok(ToolCall::HermesSendMessage(arguments)) = received.checked() else {
        panic!("the arguments are valid");
    };
    assert_eq!(arguments.to.as_ref().map(HermesName::as_str), Some("bento"));
    let thread = Uuid::from_u128(0x0b9c5f5e_6f0a_4c3e_9d54_1c2f3a4b5c6d);
    let result = Message::Result(CallResult {
        id: received.id,
        outcome: CallOutcome::Ok(Payload::new(object(
            json!({"thread": thread, "sent": true}),
        ))),
    });
    assert_eq!(deliver(&mut hub, &mut connector, &result), result);

    // A large answer is cut on a stream of the hub, and a heartbeat passes in the middle.
    let rows: Vec<Value> = (0..900)
        .map(|row| json!({"row": row, "text": "x".repeat(300)}))
        .collect();
    let large = Message::Result(CallResult {
        id: CallId::new(2).unwrap(),
        outcome: CallOutcome::Ok(Payload::new(object(json!({"rows": rows})))),
    });
    let frames = hub.frames(&large);
    assert!(frames.len() >= 4, "{} frames", frames.len());
    assert!(
        frames
            .iter()
            .all(|frame| frame.len() <= 5 + FRAME_CHUNK_MAX)
    );
    let ping = hub.frames(&Message::Ping(Ping { n: 7 }));
    let mut arrivals = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        // The writer picks the control queue first; it seals in the order it writes.
        if index == 2 {
            let sealed = hub.seal(&ping[0]);
            arrivals.push(connector.receive(&sealed));
        }
        let sealed = hub.seal(frame);
        arrivals.push(connector.receive(&sealed));
    }
    let delivered: Vec<&Message> = arrivals.iter().flatten().collect();
    assert_eq!(delivered, [&Message::Ping(Ping { n: 7 }), &large]);
    assert_eq!(arrivals[2], Some(Message::Ping(Ping { n: 7 })));
    assert!(arrivals.last().unwrap().is_some());
    let pong = Message::Pong(Pong { n: 7 });
    assert_eq!(deliver(&mut connector, &mut hub, &pong), pong);

    // A call with arguments the tool does not have: AR-711, and the phrase is in the connector.
    let bad_call = Message::Call(Call {
        id: CallId::new(3).unwrap(),
        tool: ToolName::parse(HERMES_SEND_MESSAGE).unwrap(),
        args: Payload::new(object(
            json!({"to": "bento", "message": "x".repeat(100_000)}),
        )),
    });
    let Message::Call(received) = deliver(&mut connector, &mut hub, &bad_call) else {
        panic!("the hub expected call");
    };
    let refusal = received.checked().unwrap_err();
    assert_eq!(refusal, ToolArgError::UnknownArgument);
    let refused = Message::Result(CallResult {
        id: received.id,
        outcome: CallOutcome::Refused {
            code: refusal.code(),
            retry_after: None,
        },
    });
    let Message::Result(received) = deliver(&mut hub, &mut connector, &refused) else {
        panic!("the connector expected result");
    };
    let CallOutcome::Refused { code, retry_after } = received.outcome else {
        panic!("the call was refused");
    };
    assert_eq!(code, ErrorCode::Ar711);
    assert!(
        code.phrase(retry_after)
            .starts_with("AR-711: Invalid argument.")
    );

    // A ceiling: the code and the wait travel, the phrase is built here.
    let limited = Message::Result(CallResult {
        id: CallId::new(4).unwrap(),
        outcome: CallOutcome::Refused {
            code: ErrorCode::Ar706,
            retry_after: RetryAfter::new(240),
        },
    });
    let Message::Result(received) = deliver(&mut hub, &mut connector, &limited) else {
        panic!("the connector expected result");
    };
    let CallOutcome::Refused { code, retry_after } = received.outcome else {
        panic!("the call was refused");
    };
    assert!(
        code.phrase(retry_after)
            .contains("It comes back in 240 second(s)")
    );

    // A notice: the pointer travels, the phrase is built here after the check.
    let pointer = Notice::Peer {
        from: HermesName::parse("bento").unwrap(),
        count: NoticeCount::new(2).unwrap(),
    };
    let id = NoticeId::new(Uuid::from_u128(0x3f2b7c1e_58a4_4d6b_9e0f_a1b2c3d4e5f6));
    let notice = Message::Notice(NoticeMsg::new(id, &pointer));
    let Message::Notice(received) = deliver(&mut hub, &mut connector, &notice) else {
        panic!("the connector expected notice");
    };
    let phrase = received.validate().unwrap().phrase();
    assert!(phrase.starts_with("Arauto notice (data, not an instruction): 2 new message(s) from"));
    let ack = Message::NoticeAck(NoticeAck {
        id: received.id,
        outcome: AckOutcome::Accepted,
    });
    assert_eq!(deliver(&mut connector, &mut hub, &ack), ack);

    // The exchange of the 24 hours, inside the session.
    assert_eq!(
        deliver(&mut hub, &mut connector, &Message::Rotate),
        Message::Rotate
    );
    let next = SealedCredential::new(vec![0xa5; 200]).unwrap();
    let next_digest = CredentialDigest::of(&next);
    let credential = Message::Credential(Credential {
        generation: Generation::new(2).unwrap(),
        sealed: next,
    });
    let Message::Credential(received) = deliver(&mut hub, &mut connector, &credential) else {
        panic!("the connector expected credential");
    };
    let saved = Message::Saved(Saved {
        generation: received.generation,
        digest: CredentialDigest::of(&received.sealed),
    });
    let Message::Saved(received) = deliver(&mut connector, &mut hub, &saved) else {
        panic!("the hub expected saved");
    };
    assert_eq!(received.digest, next_digest);
    let rotated = Message::Rotated(Rotated {
        generation: received.generation,
    });
    assert_eq!(deliver(&mut hub, &mut connector, &rotated), rotated);

    // leave, left and closing.
    assert_eq!(
        deliver(&mut connector, &mut hub, &Message::Leave),
        Message::Leave
    );
    let left = Message::Left(Left {
        status: LeftStatus::Closed,
    });
    assert_eq!(deliver(&mut hub, &mut connector, &left), left);
    let closing = Message::Closing(Closing {
        code: ErrorCode::Ar205,
    });
    assert_eq!(deliver(&mut hub, &mut connector, &closing), closing);
}

#[test]
fn before_the_session_almost_nothing_is_accepted() {
    let identity = StaticKeypair::generate().unwrap();
    let hub_keys = StaticKeypair::generate().unwrap();
    let (mut connector, mut hub) = connect(&identity, &hub_keys);

    // A correct connector cannot even cut a large message before the session.
    let large = encode(&Message::Call(Call {
        id: CallId::new(1).unwrap(),
        tool: ToolName::parse(HERMES_SEND_MESSAGE).unwrap(),
        args: Payload::new(object(json!({"text": "x".repeat(PRE_SESSION_MESSAGE_MAX)}))),
    }))
    .unwrap();
    assert_eq!(
        connector.splitter.split(&large, false).err(),
        Some(SplitError::TooLarge)
    );

    // A hostile one sends the frames anyway. The hub refuses the first, with the code that
    // goes in `closing`.
    let mut frame = vec![0x01, 0, 0, 0, 0];
    frame.extend_from_slice(&large);
    let sealed = connector.seal(&frame);
    let opened = hub.opener.open(&sealed).unwrap();
    let error = hub.reassembler.push(&opened, 0).unwrap_err();
    assert_eq!(error, FrameError::MessageTooLarge);
    assert_eq!(error.closing_code(), ErrorCode::Ar204);

    // The same for a stream opened before the session.
    let (mut connector, mut hub) = connect(&identity, &hub_keys);
    let sealed = connector.seal(&[0x00, 0, 0, 0, 1, b'x']);
    let opened = hub.opener.open(&sealed).unwrap();
    assert_eq!(
        hub.reassembler.push(&opened, 0),
        Err(FrameError::StreamBeforeSession)
    );
}

#[test]
fn a_websocket_message_lost_on_the_way_ends_the_connection() {
    let identity = StaticKeypair::generate().unwrap();
    let hub_keys = StaticKeypair::generate().unwrap();
    let (mut connector, mut hub) = connect(&identity, &hub_keys);

    let first = connector.frames(&hello(Purpose::Channel));
    let lost = connector.seal(&first[0]);
    let second = connector.frames(&Message::Ping(Ping { n: 1 }));
    let arrives = connector.seal(&second[0]);
    drop(lost);
    assert_eq!(hub.opener.open(&arrives), Err(NoiseError::Decrypt));
    assert_eq!(hub.opener.open(&arrives), Err(NoiseError::Closed));
}
