//! The canary of the secrets and the personal data of the WhatsApp (`docs/15-mvp.md`,
//! section 5): the key of the API of the engine, the secret of its events, the text of a
//! message and a telephone go through a whole flow (the engine configured, a number added and
//! paired, events received, repeated, forged, malformed and late, readings and sendings with
//! the engine failing in each way, the supervision correcting a session and raising alerts, a
//! new secret of the events, refused commands), and none of them may show up in the log of the
//! process, in the errors, in the audit, in the alerts or in clear in the new tables.
//!
//! A binary of its own, like the canary of the connection: the log of the process is captured
//! from its first line, at every level, and no other test writes into it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[allow(dead_code, unused_imports)]
#[path = "channel/support.rs"]
mod support;

use arauto_testkit::hub::fixture;

#[allow(dead_code)]
#[path = "whatsapp/world.rs"]
mod world;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::engine::{self, EngineChatId, MessageBody, MessageQuery, Page, Phone, SendText};
use arauto_protocol::clock::{Clock, SystemClock};
use arauto_testkit::engine::{Fault, post, sign};
use data_encoding::{BASE64, BASE64_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXUPPER};
use serde_json::json;
use uuid::Uuid;

use crate::world::{World, message, phone};

/// Every line of the log of the process.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// The ways a secret of bytes can be written down.
fn spellings(bytes: &[u8]) -> Vec<String> {
    vec![
        HEXLOWER.encode(bytes),
        HEXUPPER.encode(bytes),
        BASE64.encode(bytes),
        BASE64_NOPAD.encode(bytes),
        BASE64URL_NOPAD.encode(bytes),
        format!("{bytes:?}"),
    ]
}

/// The ways a telephone can be written down: with and without the ninth digit, with `+`, and
/// as the identifier of its chat.
fn telephone_spellings(digits: &str) -> Vec<String> {
    let parsed = Phone::parse(digits).unwrap();
    let mut written = Vec::new();
    for form in parsed.forms() {
        let digits = form.digits().to_owned();
        written.push(format!("{digits}@c.us"));
        written.push(format!("+{digits}"));
        written.push(digits);
    }
    written
}

/// An error as the screen and the log would write it.
fn written(error: &(impl std::fmt::Display + std::fmt::Debug)) -> String {
    format!("{error}\n{error:?}\n")
}

/// Where a secret shows up in `text`, if anywhere.
fn leaks(text: &str, secrets: &[(String, String)]) -> Vec<String> {
    secrets
        .iter()
        .filter(|(_, written)| text.contains(written.as_str()))
        .map(|(what, _)| what.clone())
        .collect()
}

#[test]
fn the_detector_sees_a_planted_secret() {
    let mut secrets = Vec::new();
    for written in telephone_spellings("5511987600099") {
        secrets.push(("planted".to_owned(), written.clone()));
        assert_eq!(leaks(&format!("x {written} y"), &secrets), ["planted"]);
        secrets.clear();
    }
    // The form without the ninth digit is looked for too.
    assert!(telephone_spellings("5511987600099").contains(&"551187600099".to_owned()));
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn no_secret_of_the_whatsapp_reaches_the_log_the_errors_the_audit_or_the_alerts() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    let world = World::new().await;
    let mut secrets: Vec<(String, String)> = Vec::new();
    let mut errors = String::new();

    // The key of the engine, our own.
    let key = format!("canary-key-{}", Uuid::new_v4().simple());
    world.fake.set_api_key(&key);
    world
        .fixture
        .admin
        .engine_set_key(&world.vault, &key, world.now())
        .await
        .unwrap();
    secrets.push(("engine key".to_owned(), key.clone()));
    // A key typed with a control character is refused without being repeated.
    let typed = format!("typed-key-{}", Uuid::new_v4().simple());
    let refused = world
        .fixture
        .admin
        .engine_set_key(&world.vault, &format!("{typed}\u{7}"), world.now())
        .await
        .unwrap_err();
    errors.push_str(&written(&refused));
    secrets.push(("typed key".to_owned(), typed));

    // A number, added and paired with its telephone.
    let own = phone(42);
    let number = world.connected("canary", &own).await;
    let session = number.session.as_str().to_owned();
    for written in telephone_spellings(&own) {
        secrets.push(("telephone of the number".to_owned(), written));
    }
    let contact = phone(43);
    for written in telephone_spellings(&contact) {
        secrets.push(("telephone of a contact".to_owned(), written));
    }
    let chat = format!("{contact}@c.us");
    let (url, events_key) = world.fake.webhook(&session).unwrap();
    secrets.push(("secret of the events".to_owned(), events_key.clone()));
    secrets.push((
        "secret of the events".to_owned(),
        events_key.to_ascii_uppercase(),
    ));
    for written in spellings(&HEXLOWER.decode(events_key.as_bytes()).unwrap()) {
        secrets.push(("secret of the events".to_owned(), written));
    }
    let text = format!("canary text {}", Uuid::new_v4().simple());
    secrets.push(("text of a message".to_owned(), text.clone()));

    // Events: received, repeated, from the number itself, forged, malformed and late.
    let now_s = SystemClock.now_ms() / 1_000;
    let incoming = message(&chat, "3EB0CA0001", now_s, false, &text);
    let body = world
        .fake
        .envelope(&session, "message.any", incoming.clone(), None)
        .unwrap();
    assert_eq!(world.fake.deliver(&session, &body).await.unwrap(), 200);
    assert_eq!(world.fake.deliver(&session, &body).await.unwrap(), 200);
    let mine = message(&chat, "3EB0CA0002", now_s, true, &text);
    assert_eq!(
        world
            .fake
            .emit(&session, "message.any", mine)
            .await
            .unwrap(),
        200
    );
    let forged = sign(b"not-the-secret", &body);
    let headers = [
        ("Content-Type", "application/json"),
        ("X-Webhook-Hmac", forged.as_str()),
    ];
    assert_eq!(post(&url, &headers, &body).await.unwrap(), 401);
    let late = world
        .fake
        .envelope_at(&session, "message.any", incoming, None, 1_000)
        .unwrap();
    assert_eq!(world.fake.deliver(&session, &late).await.unwrap(), 400);
    let malformed = json!({"id": format!("evt_{text}"), "timestamp": SystemClock.now_ms(),
                           "event": "message.any", "session": session,
                           "payload": {"id": format!("../{text}"), "body": text}});
    let malformed = malformed.to_string().into_bytes();
    assert_eq!(world.fake.deliver(&session, &malformed).await.unwrap(), 400);
    let reaction = json!({"id": format!("false_{chat}_3EB0CA0003"), "from": chat,
                          "reaction": {"text": text, "messageId": format!("false_{chat}_3EB0CA0001")}});
    assert_eq!(
        world
            .fake
            .emit(&session, "message.reaction", reaction)
            .await
            .unwrap(),
        200
    );
    let status = json!({"status": "WORKING"});
    assert_eq!(
        world
            .fake
            .emit(&session, "session.status", status)
            .await
            .unwrap(),
        200
    );

    // Readings and sendings, with the engine answering, then failing in every way.
    world.fake.add_chat(&session, &chat, &text);
    world.fake.add_message(
        &session,
        &chat,
        message(&chat, "3EB0CA0004", now_s, false, &text),
    );
    let loaded = world.engine().await;
    let client = &loaded.client;
    let name = number.session.clone();
    let chat_id = EngineChatId::parse(&chat).unwrap();
    let query = MessageQuery {
        page: Page::new(30, 0).unwrap(),
        before_s: None,
        after_s: None,
    };
    let read = client.messages(&name, &chat_id, query).await.unwrap();
    assert!(!read.is_empty());
    let to_send = SendText {
        chat: chat_id.clone(),
        text: MessageBody::to_send(&text).unwrap(),
        reply_to: None,
        mentions: Vec::new(),
    };
    client.send_text(&name, &to_send).await.unwrap();
    let contact_phone = Phone::parse(&contact).unwrap();
    for fault in [
        Fault::ServerError,
        Fault::Malformed,
        Fault::Huge(5 * 1024 * 1024),
        Fault::WrongType,
        Fault::Slow(Duration::from_secs(3)),
    ] {
        world.fake.fail("/api", fault);
        let failed = client.messages(&name, &chat_id, query).await.unwrap_err();
        errors.push_str(&written(&failed));
        // A sending the engine accepted with a body the hub cannot read is a sending done,
        // without the identifier of the message: only the other faults are errors.
        match client.send_text(&name, &to_send).await {
            Err(failed) => errors.push_str(&written(&failed)),
            Ok(sent) => assert!(sent.id.is_none(), "{sent:?}"),
        }
        let failed = client
            .check_number(&name, &contact_phone)
            .await
            .unwrap_err();
        errors.push_str(&written(&failed));
        world.fake.heal();
    }

    // The supervision: a session changed from outside is corrected, a version that was not
    // tested raises an alert, and the catch-up records what the inbox missed.
    world.fake.add_message(
        &session,
        &chat,
        message(
            &chat,
            "3EB0CA0005",
            SystemClock.now_ms() / 1_000,
            false,
            &text,
        ),
    );
    let mut tampered = world.fake.config(&session).unwrap();
    tampered["webhooks"][0]["url"] = json!(format!("http://198.51.100.9/{text}"));
    world.fake.tamper_config(&session, tampered);
    world.fake.set_version("2099.1.1", "GOWS");
    engine::supervise(world.hub()).await;

    // A new secret of the events reaches the session, and is not shown either.
    world
        .fixture
        .admin
        .engine_new_events_secret(&world.vault, world.now())
        .await
        .unwrap();
    engine::supervise(world.hub()).await;
    let (_, renewed) = world.fake.webhook(&session).unwrap();
    assert_ne!(renewed, events_key, "the session got the new secret");
    secrets.push(("new secret of the events".to_owned(), renewed.clone()));
    secrets.push((
        "new secret of the events".to_owned(),
        renewed.to_ascii_uppercase(),
    ));

    // Commands that are refused: the same telephone again, a telephone out of the format, an
    // engine whose key was changed under it.
    let taken = world
        .fixture
        .admin
        .number_add(
            &world.vault,
            "again",
            &format!("+{own}"),
            false,
            world.now(),
        )
        .await
        .unwrap_err();
    errors.push_str(&written(&taken));
    let wrong = world
        .fixture
        .admin
        .number_add(
            &world.vault,
            "wrong",
            &format!("{text} {own}"),
            false,
            world.now(),
        )
        .await
        .unwrap_err();
    errors.push_str(&written(&wrong));
    world.fake.set_api_key("another-key-of-the-engine");
    let unhealthy = world
        .fixture
        .admin
        .engine_health(&world.vault)
        .await
        .unwrap_err();
    errors.push_str(&written(&unhealthy));
    // The rendering of the commands is what the admin reads on the screen.
    let shown = world.fixture.admin.engine_show().await.unwrap();
    errors.push_str(&arauto_hub::admin::render::whatsapp::engine_show(shown.as_ref()).text);
    let listed = world.fixture.admin.number_list(true).await.unwrap();
    errors.push_str(&format!("{listed:?}"));

    tokio::time::sleep(Duration::from_millis(300)).await;
    let log = captured.text();
    assert!(
        log.len() > 1_000,
        "the log is captured (it has {} bytes)",
        log.len()
    );
    assert!(errors.len() > 100, "the errors are collected");
    let mut stored = String::new();
    for table in [
        "audit",
        "alert",
        "engine",
        "number",
        "chat",
        "chat_engine_id",
        "engine_event",
    ] {
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT row_to_json(t)::text FROM {table} t"
        )))
        .fetch_all(world.hub().pool())
        .await
        .unwrap();
        stored.push_str(&rows.join("\n"));
        stored.push('\n');
    }
    // The flow wrote what it should, so that the search means something.
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM engine_event")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    assert!(events >= 4, "the events are recorded ({events})");
    let alerts: i64 = sqlx::query_scalar("SELECT count(*) FROM alert")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    assert!(alerts >= 2, "the alerts are raised ({alerts})");
    for (place, text) in [
        ("log", &log),
        ("errors and screens", &errors),
        ("audit, alerts and the tables of the WhatsApp", &stored),
    ] {
        let found = leaks(text, &secrets);
        assert!(found.is_empty(), "{place} holds: {found:?}");
    }
}
