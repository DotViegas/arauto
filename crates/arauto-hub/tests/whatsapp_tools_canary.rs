//! The canary of the WhatsApp tools, end to end (`docs/15-mvp.md`, section 5): a Hermes in
//! session reads, sends, forwards, edits and deletes through the hub and the fake engine,
//! with a matrix of accesses given and changed by the admin, refused calls of every kind and
//! the engine failing in every way; the key of the engine, the secret of its events, the text
//! of a message and the telephones must show up in none of: the log of the process, the
//! refusals the Hermes gets, the audit, the alerts and the tables of the matrix.
//!
//! What a reading returns is the content the tool exists to return: the text and the
//! telephones go to the Hermes there, and only there.
//!
//! A binary of its own: the log of the process is captured from its first line.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[allow(dead_code, unused_imports)]
#[path = "channel/support.rs"]
mod support;

use arauto_testkit::hub::fixture;

#[allow(dead_code)]
#[path = "whatsapp/world.rs"]
mod world;

#[allow(dead_code)]
#[path = "whatsapp/agent.rs"]
mod agent;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_hub::engine::Phone;
use arauto_protocol::message::CallOutcome;
use arauto_testkit::engine::Fault;
use data_encoding::{BASE64, HEXLOWER};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::agent::{ALL_VERBS, NONE, READ, all, ok};
use crate::world::{World, message, phone};

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

fn leaks(text: &str, secrets: &[(String, String)]) -> Vec<String> {
    secrets
        .iter()
        .filter(|(_, written)| text.contains(written.as_str()))
        .map(|(what, _)| what.clone())
        .collect()
}

#[test]
fn the_detector_sees_a_planted_secret() {
    let secrets = vec![("planted".to_owned(), "+5511987600077".to_owned())];
    assert_eq!(leaks("a +5511987600077 b", &secrets), ["planted"]);
    assert!(leaks("a b", &secrets).is_empty());
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn no_secret_of_the_whatsapp_tools_reaches_the_log_the_refusals_the_audit_or_the_alerts() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .init();

    let world = World::new().await;
    let mut secrets: Vec<(String, String)> = Vec::new();
    let key = format!("canary-key-{}", Uuid::new_v4().simple());
    world.fake.set_api_key(&key);
    world
        .fixture
        .admin
        .engine_set_key(&world.vault, &key, world.now())
        .await
        .unwrap();
    secrets.push(("engine key".to_owned(), key));
    let own = phone(77);
    let number = world.connected("canario", &own).await;
    let session = number.session.as_str().to_owned();
    for written in telephone_spellings(&own) {
        secrets.push(("telephone of the number".to_owned(), written));
    }
    let contact = phone(78);
    for written in telephone_spellings(&contact) {
        secrets.push(("telephone of a contact".to_owned(), written));
    }
    let stranger = phone(79);
    for written in telephone_spellings(&stranger) {
        secrets.push(("telephone of a stranger".to_owned(), written));
    }
    let nobody = phone(80);
    for written in telephone_spellings(&nobody) {
        secrets.push(("telephone without WhatsApp".to_owned(), written));
    }
    let (_, events_key) = world.fake.webhook(&session).unwrap();
    secrets.push(("secret of the events".to_owned(), events_key.clone()));
    secrets.push((
        "secret of the events".to_owned(),
        BASE64.encode(&HEXLOWER.decode(events_key.as_bytes()).unwrap()),
    ));
    let text = format!("canary text {}", Uuid::new_v4().simple());
    secrets.push(("text of a message".to_owned(), text.clone()));

    let chat = format!("{contact}@c.us");
    world.fake.add_chat(&session, &chat, &text);
    world.fake.register(&contact, &chat);
    world.fake.register(&stranger, &format!("{stranger}@c.us"));
    world.fake.add_message(
        &session,
        &chat,
        message(&chat, "3EB0CA0001", 1_800_000_000, false, &text),
    );
    world.fake.add_message(
        &session,
        &chat,
        message(&chat, "3EB0CA0002", 1_800_000_001, true, &text),
    );

    let mut agent = world.agent().await;
    let mut refusals = String::new();
    let refused = |outcome: CallOutcome, refusals: &mut String| {
        if let CallOutcome::Refused { .. } = &outcome {
            refusals.push_str(&format!("{outcome:?}\n"));
        }
        outcome
    };
    // Before any access, every tool is refused.
    for (tool, args) in [
        (
            "whatsapp_send_text",
            json!({"to": format!("+{contact}"), "text": text}),
        ),
        (
            "whatsapp_check_number",
            json!({"phone": format!("+{stranger}")}),
        ),
    ] {
        let outcome = refused(agent.call(tool, args).await, &mut refusals);
        assert!(matches!(outcome, CallOutcome::Refused { .. }));
    }
    world.grant(&agent.name, "canario", all(ALL_VERBS)).await;
    world
        .line(&agent.name, "canario", &format!("+{contact}"), ALL_VERBS)
        .await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    let hub_chat = listed["chats"][0]["chat"].as_str().unwrap().to_owned();
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": hub_chat}))
        .await);
    let mine = read["messages"][0]["message_id"].clone();
    let theirs = read["messages"][1]["message_id"].clone();
    // Sendings and changes, with the text and the telephones in them.
    ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": format!("+{contact}"), "text": text, "reply_to": theirs}),
        )
        .await);
    ok(agent
        .call(
            "whatsapp_send_file",
            json!({"to": hub_chat, "kind": "document", "mimetype": "application/pdf",
                   "filename": format!("{text}.pdf"), "data": BASE64.encode(text.as_bytes()),
                   "caption": text}),
        )
        .await);
    ok(agent
        .call(
            "whatsapp_check_number",
            json!({"phone": format!("+{stranger}")}),
        )
        .await);
    ok(agent
        .call(
            "whatsapp_edit_message",
            json!({"message_id": mine, "text": text}),
        )
        .await);
    ok(agent
        .call(
            "whatsapp_forward",
            json!({"message_id": theirs, "to": hub_chat}),
        )
        .await);
    // Refusals of every kind, with the secrets in the arguments.
    for (tool, args) in [
        (
            "whatsapp_send_text",
            json!({"to": format!("+{stranger}"), "text": text}),
        ),
        (
            "whatsapp_send_text",
            json!({"to": format!("{contact}@c.us"), "text": text}),
        ),
        (
            "whatsapp_send_text",
            json!({"to": hub_chat, "text": format!("{text}\u{0}")}),
        ),
        (
            "whatsapp_delete_message",
            json!({"message_id": format!("{}A", mine.as_str().unwrap())}),
        ),
        (
            "whatsapp_read_chat",
            json!({"chat": hub_chat, "number": "nao-existe"}),
        ),
        ("whatsapp_status", json!({"number": text})),
        (
            "whatsapp_send_text",
            json!({"to": format!("+{nobody}"), "text": text}),
        ),
    ] {
        let outcome = refused(agent.call(tool, args).await, &mut refusals);
        assert!(matches!(outcome, CallOutcome::Refused { .. }), "{tool}");
    }
    // The engine failing in every way, under the calls of the Hermes.
    for fault in [
        Fault::ServerError,
        Fault::Malformed,
        Fault::Huge(5 * 1024 * 1024),
        Fault::WrongType,
        Fault::Slow(Duration::from_secs(3)),
    ] {
        world.fake.fail("/api", fault);
        for (tool, args) in [
            ("whatsapp_read_chat", json!({"chat": hub_chat})),
            (
                "whatsapp_send_text",
                json!({"to": format!("+{contact}"), "text": text}),
            ),
            (
                "whatsapp_check_number",
                json!({"phone": format!("+{stranger}")}),
            ),
        ] {
            refused(agent.call(tool, args).await, &mut refusals);
        }
        world.fake.heal();
    }
    // The admin changes the matrix: blocked, and the access taken away.
    world.line(&agent.name, "canario", &hub_chat, NONE).await;
    refused(
        agent
            .call("whatsapp_read_chat", json!({"chat": hub_chat}))
            .await,
        &mut refusals,
    );
    world.grant(&agent.name, "canario", all(READ)).await;
    world.take_away(&agent.name, "canario").await;
    refused(
        agent
            .call("whatsapp_send_text", json!({"to": hub_chat, "text": text}))
            .await,
        &mut refusals,
    );
    let status = ok(agent.call("arauto_status", json!({})).await);

    tokio::time::sleep(Duration::from_millis(300)).await;
    let log = String::from_utf8_lossy(&captured.0.lock().unwrap()).into_owned();
    assert!(
        log.len() > 1_000,
        "the log is captured ({} bytes)",
        log.len()
    );
    assert!(refusals.lines().count() >= 15, "the refusals are collected");
    let mut stored = String::new();
    for table in [
        "audit",
        "alert",
        "access",
        "access_chat",
        "chat",
        "chat_engine_id",
        "number",
        "engine",
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
    let changes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit WHERE action LIKE 'access%' AND security")
            .fetch_one(world.hub().pool())
            .await
            .unwrap();
    assert!(
        changes >= 4,
        "the changes of the matrix are audited ({changes})"
    );
    for (place, text) in [
        ("log", &log),
        ("refusals", &refusals),
        ("status", &Value::Object(status).to_string()),
        ("audit, alerts and the tables of the matrix", &stored),
    ] {
        let found = leaks(text, &secrets);
        assert!(found.is_empty(), "{place} holds: {found:?}");
    }
}
