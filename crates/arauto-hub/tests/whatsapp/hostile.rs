//! Hostile answers of the engine, and hostile values given to the hub.
//!
//! A slow, failing, malformed, enormous or mistyped answer is an error of that call, within
//! its deadline, and holds no call of another session. An identifier outside its format is
//! refused by its type, before any request is put together: the fake proves by its list of
//! requests that none reached it.

use std::time::{Duration, Instant};

use arauto_hub::engine::types::*;
use arauto_hub::engine::{Cause, EngineError, EventsSecret, SessionConfig, WhatsappEngine};
use arauto_protocol::names::NumberAlias;
use arauto_testkit::engine::{FakeEngine, Fault};

use crate::world::{KEY, session, waha};

async fn two_sessions() -> (
    FakeEngine,
    arauto_hub::engine::waha::Waha,
    SessionName,
    SessionName,
) {
    let fake = FakeEngine::start(KEY).await.unwrap();
    let adapter = waha(&fake);
    let secret = EventsSecret::draw().unwrap();
    let config = SessionConfig {
        events_url: "http://127.0.0.1:9/v1/engine/00000000-0000-0000-0000-000000000001/events"
            .to_owned(),
        events_secret: &secret,
    };
    let (first, second) = (session(), session());
    for name in [&first, &second] {
        adapter.ensure_session(name, &config, false).await.unwrap();
        fake.connect(name.as_str(), "5511900000000");
    }
    (fake, adapter, first, second)
}

fn page() -> Page {
    Page::new(10, 0).unwrap()
}

#[tokio::test]
async fn a_slow_engine_is_cut_at_the_deadline_and_holds_no_other_number() {
    let (fake, adapter, slow, quick) = two_sessions().await;
    fake.fail(
        &format!("/api/{}/", slow.as_str()),
        Fault::Slow(Duration::from_secs(30)),
    );
    let started = Instant::now();
    let (slow_answer, quick_answer) = tokio::join!(adapter.chats(&slow, page()), async {
        let started = Instant::now();
        let answer = adapter.chats(&quick, page()).await;
        (answer, started.elapsed())
    });
    assert_eq!(slow_answer, Err(EngineError::Unavailable(Cause::Timeout)));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the deadline of the call held"
    );
    let (quick_answer, took) = quick_answer;
    assert!(quick_answer.is_ok());
    assert!(
        took < Duration::from_secs(1),
        "the other number answered at once: {took:?}"
    );
}

#[tokio::test]
async fn an_error_a_malformed_an_enormous_or_a_mistyped_answer_is_an_error_of_that_call() {
    let (fake, adapter, name, _) = two_sessions().await;
    for (fault, expected) in [
        (Fault::ServerError, Cause::Status(500)),
        (Fault::Malformed, Cause::Malformed),
        (Fault::Huge(6 * 1024 * 1024), Cause::TooLarge),
        (Fault::WrongType, Cause::ContentType),
    ] {
        fake.heal();
        fake.fail("/chats/overview", fault.clone());
        assert_eq!(
            adapter.chats(&name, page()).await,
            Err(EngineError::Unavailable(expected)),
            "{fault:?}"
        );
        // The session itself still answers: only that call failed.
        assert!(adapter.session(&name).await.unwrap().is_some());
    }
    fake.heal();
    fake.fail("/api/server/version", Fault::Malformed);
    assert_eq!(
        adapter.version().await,
        Err(EngineError::Unavailable(Cause::Malformed))
    );
    fake.heal();
    // A session status the engine does not document is not guessed.
    fake.set_status(name.as_str(), "SOMETHING_NEW");
    assert_eq!(
        adapter.session(&name).await,
        Err(EngineError::Unavailable(Cause::Malformed))
    );
}

#[tokio::test]
async fn an_engine_that_is_not_there_is_unavailable() {
    let fake = FakeEngine::start(KEY).await.unwrap();
    let adapter = waha(&fake);
    drop(fake);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(matches!(
        adapter.health().await,
        Err(EngineError::Unavailable(Cause::Connect | Cause::Timeout))
    ));
}

/// What a value written by a Hermes or by the other side of a chat may try.
const MALICIOUS: &[&str] = &[
    "../../api/sessions",
    "..",
    "%2e%2e",
    "%2E%2E%2Fapi",
    "5511900000000@c.us/../../sessions",
    "5511900000000@c.us%2F..%2F",
    "5511900000000@c.us\0",
    "5511900000000@c.us\r\nX-Api-Key: x",
    "5511900000000@c.us?session=other",
    "5511900000000@c.us#x",
    "5511900000000@c.us/",
    "/5511900000000@c.us",
    "5511900000000@c.us\\..\\",
    "true_5511900000000@c.us_../x",
    "false_../../x_3EB0",
    "true_5511900000000@c.us_3EB0%2F",
    "aaaaaaaaaaaaaaaaaaaa/../b",
    "aaaaaaaaaaaaaaaaaaaa.",
    "\u{202e}5511900000000@c.us",
];

#[tokio::test]
async fn a_malicious_identifier_is_refused_before_any_request_is_made() {
    let (fake, adapter, name, _) = two_sessions().await;
    fake.forget_received();
    let long = "1".repeat(10_000);
    let mut values: Vec<&str> = MALICIOUS.to_vec();
    values.push(&long);
    for value in values {
        // What the tools of the next stage do with an argument: parse it with its type, and
        // only then call the engine. A value outside the format never gets that far.
        if let Ok(to) = EngineChatId::parse(value) {
            let _ = adapter.chats(&name, page()).await;
            panic!("accepted as a chat: {value:?} -> {to:?}");
        }
        assert!(EngineMessageId::parse(value).is_err(), "message {value:?}");
        assert!(
            FullMessageId::parse(value).is_err(),
            "complete message {value:?}"
        );
        assert!(SessionName::parse(value).is_err(), "session {value:?}");
        assert!(NumberAlias::parse(value).is_err(), "alias {value:?}");
        assert!(Phone::parse(value).is_err(), "telephone {value:?}");
    }
    // A plausible name of a session someone else configured is not one the hub draws.
    assert!(SessionName::parse("default").is_err());
    assert!(fake.received().is_empty(), "nothing reached the engine");

    // And every path the adapter ever writes is of the narrow alphabet.
    let chat = EngineChatId::parse("5511900000001@c.us").unwrap();
    let message = FullMessageId::parse("true_5511900000001@c.us_3EB0BEEF").unwrap();
    let _ = adapter
        .messages(
            &name,
            &chat,
            MessageQuery {
                page: page(),
                before_s: None,
                after_s: None,
            },
        )
        .await;
    let _ = adapter.contact(&name, &chat).await;
    let _ = adapter.delete_message(&name, &message).await;
    let _ = adapter
        .edit_message(&name, &message, &MessageBody::to_send("x").unwrap())
        .await;
    for request in fake.received() {
        assert!(
            request
                .path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.%".contains(&b)),
            "{}",
            request.path
        );
        assert!(!request.path.contains(".."), "{}", request.path);
        assert!(!request.path.contains("%2"), "{}", request.path);
        assert!(!request.path.contains("//"), "{}", request.path);
    }
}
