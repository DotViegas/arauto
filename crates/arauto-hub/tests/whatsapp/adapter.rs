//! Each operation of the adapter of the WAHA against the fake engine, with no hub.

use arauto_hub::engine::types::*;
use arauto_hub::engine::waha::Waha;
use arauto_hub::engine::{
    ApiKey, Cause, EngineAddress, EngineError, EventsSecret, SessionConfig, WhatsappEngine,
};
use arauto_testkit::engine::FakeEngine;
use serde_json::{Value, json};

use crate::world::{DEADLINES, KEY, message, session, waha};

const CHAT: &str = "5511900000001@c.us";
const GROUP: &str = "120363012345678901@g.us";

fn config(secret: &EventsSecret) -> SessionConfig<'_> {
    SessionConfig {
        events_url: "http://127.0.0.1:9/v1/engine/00000000-0000-0000-0000-000000000001/events"
            .to_owned(),
        events_secret: secret,
    }
}

/// A fake with one session, connected as `5511900000000`.
async fn connected() -> (FakeEngine, Waha, SessionName) {
    let fake = FakeEngine::start(KEY).await.unwrap();
    let adapter = waha(&fake);
    let name = session();
    let secret = EventsSecret::draw().unwrap();
    adapter
        .ensure_session(&name, &config(&secret), false)
        .await
        .unwrap();
    fake.connect(name.as_str(), "5511900000000");
    (fake, adapter, name)
}

fn chat(text: &str) -> EngineChatId {
    EngineChatId::parse(text).unwrap()
}

#[tokio::test]
async fn health_and_version() {
    let fake = FakeEngine::start(KEY).await.unwrap();
    let adapter = waha(&fake);
    adapter.health().await.unwrap();
    let version = adapter.version().await.unwrap();
    assert_eq!(
        (version.version.as_str(), version.engine.as_str()),
        ("2026.10.1", "GOWS")
    );
    assert!(version.is_tested());
    fake.set_version("2027.1.0", "GOWS");
    assert!(!adapter.version().await.unwrap().is_tested());
    // Every request carries the key, in its header.
    assert!(
        fake.received()
            .iter()
            .all(|r| r.api_key.as_deref() == Some(KEY))
    );
}

#[tokio::test]
async fn a_wrong_key_makes_the_engine_unavailable() {
    let fake = FakeEngine::start(KEY).await.unwrap();
    let adapter = Waha::new(
        EngineAddress::parse(&fake.address()).unwrap(),
        Some(ApiKey::parse("another-key-123").unwrap()),
        DEADLINES,
    );
    assert_eq!(
        adapter.version().await,
        Err(EngineError::Unavailable(Cause::KeyRefused))
    );
}

#[tokio::test]
async fn a_session_is_created_configured_and_reconfigured_only_when_it_differs() {
    let fake = FakeEngine::start(KEY).await.unwrap();
    let adapter = waha(&fake);
    let name = session();
    let secret = EventsSecret::draw().unwrap();
    let wanted = config(&secret);
    assert_eq!(adapter.session(&name).await.unwrap(), None);
    assert_eq!(
        adapter.ensure_session(&name, &wanted, false).await.unwrap(),
        Ensured::Created
    );
    let stored = fake.config(name.as_str()).unwrap();
    let hook = &stored["webhooks"][0];
    assert_eq!(hook["url"], json!(wanted.events_url));
    assert_eq!(hook["hmac"]["key"], json!(secret.hmac_key().as_str()));
    assert_eq!(
        hook["events"],
        json!([
            "session.status",
            "message.any",
            "message.reaction",
            "message.edited",
            "message.revoked"
        ])
    );
    // Created already configured, groups kept, and not started.
    assert_eq!(stored["ignore"]["groups"], json!(false));
    assert_eq!(fake.status(name.as_str()).as_deref(), Some("STOPPED"));
    // The same configuration is not sent again: sending restarts the session.
    for _ in 0..3 {
        assert_eq!(
            adapter.ensure_session(&name, &wanted, true).await.unwrap(),
            Ensured::Unchanged
        );
    }
    assert_eq!(fake.configured(name.as_str()), 1);
    // A destination added from outside is corrected, whole.
    let mut tampered = stored.clone();
    tampered["webhooks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"url": "http://198.51.100.9/steal", "events": ["message.any"]}));
    fake.tamper_config(name.as_str(), tampered);
    let Ensured::Reconfigured(differences) =
        adapter.ensure_session(&name, &wanted, true).await.unwrap()
    else {
        panic!("the destination from outside was kept");
    };
    assert!(!differences.is_empty());
    assert_eq!(fake.config(name.as_str()).unwrap(), stored);
    assert_eq!(fake.configured(name.as_str()), 2);
    // Another secret is another configuration.
    let other = EventsSecret::draw().unwrap();
    assert!(matches!(
        adapter
            .ensure_session(&name, &config(&other), true)
            .await
            .unwrap(),
        Ensured::Reconfigured(_)
    ));
}

#[tokio::test]
async fn the_life_of_a_session() {
    let fake = FakeEngine::start(KEY).await.unwrap();
    let adapter = waha(&fake);
    let name = session();
    let secret = EventsSecret::draw().unwrap();
    adapter
        .ensure_session(&name, &config(&secret), false)
        .await
        .unwrap();
    adapter.start(&name).await.unwrap();
    let info = adapter.session(&name).await.unwrap().unwrap();
    assert_eq!(info.status, EngineStatus::ScanQrCode);
    assert!(info.me.is_none());
    let qr = adapter.pairing_qr(&name).await.unwrap();
    assert!(qr.expose().starts_with("2@"));
    let phone = Phone::parse("+55 11 90000-0000").unwrap();
    let code = adapter.pairing_code(&name, &phone).await.unwrap();
    assert_eq!(code.expose(), "ABCD-EFGH");
    let asked = fake.received().last().unwrap().json().unwrap();
    assert_eq!(asked["phoneNumber"], json!("5511900000000"));
    fake.connect(name.as_str(), "5511900000000");
    let info = adapter.session(&name).await.unwrap().unwrap();
    assert_eq!(info.status, EngineStatus::Working);
    assert_eq!(info.me.unwrap().phone, Some(phone));
    adapter.stop(&name).await.unwrap();
    assert_eq!(fake.status(name.as_str()).as_deref(), Some("STOPPED"));
    adapter.restart(&name).await.unwrap();
    assert_eq!(fake.status(name.as_str()).as_deref(), Some("WORKING"));
    adapter.log_out_and_delete(&name).await.unwrap();
    assert_eq!(adapter.session(&name).await.unwrap(), None);
    // Leaving a session that is not there is not an error.
    adapter.log_out_and_delete(&name).await.unwrap();
}

#[tokio::test]
async fn chats_and_messages_the_newest_first() {
    let (fake, adapter, name) = connected().await;
    fake.add_chat(name.as_str(), CHAT, "Someone");
    fake.add_message(
        name.as_str(),
        CHAT,
        message(CHAT, "3EB0A1", 100, false, "first"),
    );
    fake.add_message(
        name.as_str(),
        CHAT,
        message(CHAT, "3EB0A2", 200, true, "second"),
    );
    fake.add_message(
        name.as_str(),
        CHAT,
        message(CHAT, "3EB0A3", 300, false, "third"),
    );
    let chats = adapter
        .chats(&name, Page::new(10, 0).unwrap())
        .await
        .unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].id, chat(CHAT));
    assert_eq!(chats[0].name.as_ref().unwrap().expose(), "Someone");
    assert_eq!(chats[0].last.as_ref().unwrap().body.expose(), "third");

    let all = MessageQuery {
        page: Page::new(10, 0).unwrap(),
        before_s: None,
        after_s: None,
    };
    let messages = adapter.messages(&name, &chat(CHAT), all).await.unwrap();
    let bodies: Vec<&str> = messages.iter().map(|m| m.body.expose()).collect();
    assert_eq!(bodies, ["third", "second", "first"]);
    assert_eq!(messages[1].author, Author::Me);
    assert_eq!(messages[0].author, Author::Other(chat(CHAT)));
    assert_eq!(messages[0].at_ms, 300_000);
    let window = MessageQuery {
        page: Page::new(10, 0).unwrap(),
        before_s: Some(300),
        after_s: Some(150),
    };
    let messages = adapter.messages(&name, &chat(CHAT), window).await.unwrap();
    assert_eq!(
        messages.iter().map(|m| m.body.expose()).collect::<Vec<_>>(),
        ["second"]
    );
    // The chat went in the path, with `@` written `%40`.
    assert!(
        fake.received().iter().any(
            |r| r.path == format!("/api/{}/chats/5511900000001%40c.us/messages", name.as_str())
        )
    );
}

#[tokio::test]
async fn a_contact() {
    let (fake, adapter, name) = connected().await;
    fake.add_contact(CHAT, "Agenda name", "Own name", "Busy");
    let contact = adapter.contact(&name, &chat(CHAT)).await.unwrap();
    assert_eq!(contact.name.unwrap().expose(), "Agenda name");
    assert_eq!(contact.push_name.unwrap().expose(), "Own name");
    assert_eq!(contact.about.unwrap().expose(), "Busy");
    assert_eq!(
        contact.picture_url.as_deref(),
        Some("https://pps.example.com/picture.jpg")
    );
    assert_eq!(
        adapter.contact(&name, &chat("5511900000099@c.us")).await,
        Err(EngineError::NotFound)
    );
}

#[tokio::test]
async fn a_file_is_downloaded_from_the_engine_only_and_up_to_7_mb() {
    let (fake, adapter, name) = connected().await;
    fake.add_chat(name.as_str(), CHAT, "Someone");
    let mut with_file = message(CHAT, "3EB0F1", 100, false, "");
    with_file["hasMedia"] = json!(true);
    // The engine names the file after the message, `@` and all.
    let stored = format!("false_{CHAT}_3EB0F1.jpeg");
    with_file["media"] =
        json!({"file": stored, "mimetype": "image/jpeg", "filename": "photo.jpeg"});
    fake.add_message(name.as_str(), CHAT, with_file);
    fake.add_file(&stored, "image/jpeg", vec![7u8; 1_000]);
    let id = EngineMessageId::parse(&format!("false_{CHAT}_3EB0F1")).unwrap();
    let file = adapter
        .download_media(&name, &chat(CHAT), &id)
        .await
        .unwrap();
    assert_eq!(file.data.len(), 1_000);
    assert_eq!(file.mimetype.as_str(), "image/jpeg");
    assert_eq!(file.filename.unwrap().as_str(), "photo.jpeg");

    let mut too_big = message(CHAT, "3EB0F2", 100, false, "");
    too_big["hasMedia"] = json!(true);
    too_big["media"] = json!({"file": "s/big.bin", "mimetype": "application/octet-stream"});
    fake.add_message(name.as_str(), CHAT, too_big);
    fake.add_file(
        "s/big.bin",
        "application/octet-stream",
        vec![0u8; DOWNLOAD_MAX_BYTES + 1],
    );
    let id = EngineMessageId::parse(&format!("false_{CHAT}_3EB0F2")).unwrap();
    assert_eq!(
        adapter
            .download_media(&name, &chat(CHAT), &id)
            .await
            .map(|f| f.data.len()),
        Err(EngineError::Unavailable(Cause::TooLarge))
    );

    // An address of the file outside the files of the engine is never followed.
    let mut elsewhere = message(CHAT, "3EB0F3", 100, false, "");
    elsewhere["hasMedia"] = json!(true);
    elsewhere["media"] =
        json!({"url": "http://198.51.100.7/api/sessions", "mimetype": "image/jpeg"});
    fake.add_message(name.as_str(), CHAT, elsewhere);
    let id = EngineMessageId::parse(&format!("false_{CHAT}_3EB0F3")).unwrap();
    fake.forget_received();
    assert_eq!(
        adapter
            .download_media(&name, &chat(CHAT), &id)
            .await
            .map(|f| f.data.len()),
        Err(EngineError::Unavailable(Cause::Malformed))
    );
    assert_eq!(fake.received().len(), 1, "only the message was asked for");
}

#[tokio::test]
async fn groups_are_counted_by_their_list_and_read_in_parts() {
    let (fake, adapter, name) = connected().await;
    let participants: Vec<(String, &str)> = (0..5)
        .map(|i| {
            (
                format!("551190000010{i}@c.us"),
                if i == 0 { "superadmin" } else { "participant" },
            )
        })
        .collect();
    let borrowed: Vec<(&str, &str)> = participants
        .iter()
        .map(|(id, role)| (id.as_str(), *role))
        .collect();
    fake.add_group(name.as_str(), GROUP, "The team", &borrowed);
    let groups = adapter
        .groups(&name, Page::new(10, 0).unwrap())
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].participants, 5,
        "the count of the engine reads zero; the list has five"
    );
    let first = adapter
        .group(&name, &chat(GROUP), Page::new(2, 0).unwrap())
        .await
        .unwrap();
    assert_eq!(first.total, 5);
    assert_eq!(first.participants.len(), 2);
    assert_eq!(first.participants[0].role, Role::SuperAdmin);
    assert_eq!(first.next_offset, Some(2));
    let last = adapter
        .group(&name, &chat(GROUP), Page::new(2, 4).unwrap())
        .await
        .unwrap();
    assert_eq!(last.participants.len(), 1);
    assert_eq!(last.next_offset, None);
    assert_eq!(
        adapter
            .group(&name, &chat(CHAT), Page::new(2, 0).unwrap())
            .await,
        Err(EngineError::Invalid("group identifier"))
    );
}

#[tokio::test]
async fn a_brazilian_telephone_is_checked_with_and_without_the_ninth_digit() {
    let (fake, adapter, name) = connected().await;
    // WhatsApp knows it without the ninth digit; it is asked with it.
    fake.register("551187654321", "551187654321@c.us");
    let asked = Phone::parse("5511987654321").unwrap();
    let found = adapter.check_number(&name, &asked).await.unwrap();
    assert_eq!(found, Some(chat("551187654321@c.us")));
    let phones: Vec<String> = fake
        .received()
        .iter()
        .filter(|r| r.path == "/api/contacts/check-exists")
        .map(|r| r.query.clone())
        .collect();
    assert_eq!(phones.len(), 2);
    assert!(phones[0].contains("phone=5511987654321") && phones[1].contains("phone=551187654321"));
    assert_eq!(
        adapter
            .check_number(&name, &Phone::parse("15555550123").unwrap())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn sending_texts_files_reactions_and_the_rest() {
    let (fake, adapter, name) = connected().await;
    let reply =
        EngineMessageId::parse(&format!("false_{GROUP}_3EB0AA_5511900000002@c.us")).unwrap();
    let sent = adapter
        .send_text(
            &name,
            &SendText {
                chat: chat(GROUP),
                text: MessageBody::to_send("hello @5511900000002").unwrap(),
                reply_to: Some(reply.clone()),
                mentions: vec![chat("5511900000002@c.us")],
            },
        )
        .await
        .unwrap();
    assert!(sent.id.is_some());
    let file = SendFile {
        chat: chat(CHAT),
        kind: FileKind::Document,
        file: FileContent {
            mimetype: MimeType::parse("application/pdf").unwrap(),
            filename: Some(FileName::parse("report.pdf").unwrap()),
            data: vec![1, 2, 3],
        },
        caption: Some(MessageBody::to_send("the report").unwrap()),
    };
    adapter.send_file(&name, &file).await.unwrap();
    adapter.mark_read(&name, &chat(CHAT)).await.unwrap();
    adapter.set_typing(&name, &chat(CHAT), true).await.unwrap();
    adapter.set_typing(&name, &chat(CHAT), false).await.unwrap();
    adapter
        .react(&name, &reply, &Reaction::parse("\u{1F44D}").unwrap())
        .await
        .unwrap();
    adapter.forward(&name, &reply, &chat(CHAT)).await.unwrap();
    let sent = fake.sent();
    let actions: Vec<&str> = sent.iter().map(|(action, _)| action.as_str()).collect();
    assert_eq!(
        actions,
        [
            "sendText",
            "sendFile",
            "sendSeen",
            "startTyping",
            "stopTyping",
            "reaction",
            "forwardMessage"
        ]
    );
    let text: &Value = &sent[0].1;
    assert_eq!(text["chatId"], json!(GROUP));
    assert_eq!(text["reply_to"], json!(reply.as_str()));
    assert_eq!(text["mentions"], json!(["5511900000002@c.us"]));
    assert_eq!(sent[1].1["file"]["data"], json!("AQID"));
    assert_eq!(sent[1].1["caption"], json!("the report"));

    // Above 3 MB, or a mention outside a group, never reaches the engine.
    fake.forget_received();
    let mut big = file.clone();
    big.file.data = vec![0u8; SEND_FILE_MAX_BYTES + 1];
    assert_eq!(
        adapter.send_file(&name, &big).await,
        Err(EngineError::Invalid("file size"))
    );
    let outside = SendText {
        chat: chat(CHAT),
        text: MessageBody::to_send("hi").unwrap(),
        reply_to: None,
        mentions: vec![chat("5511900000002@c.us")],
    };
    assert_eq!(
        adapter.send_text(&name, &outside).await,
        Err(EngineError::Invalid("mentions"))
    );
    assert!(fake.received().is_empty());
}

#[tokio::test]
async fn editing_needs_a_text_and_both_need_the_complete_identifier() {
    let (fake, adapter, name) = connected().await;
    fake.forget_received();
    let mine = FullMessageId::parse(&format!("true_{CHAT}_3EB0BEEF")).unwrap();
    adapter
        .edit_message(&name, &mine, &MessageBody::to_send("fixed").unwrap())
        .await
        .unwrap();
    adapter.delete_message(&name, &mine).await.unwrap();
    assert_eq!(fake.edited().len(), 1);
    assert_eq!(fake.edited()[0].2, json!({"text": "fixed"}));
    assert_eq!(fake.deleted().len(), 1);
    let paths: Vec<String> = fake.received().iter().map(|r| r.path.clone()).collect();
    let expected = format!(
        "/api/{}/chats/5511900000001%40c.us/messages/true_5511900000001%40c.us_3EB0BEEF",
        name.as_str()
    );
    assert_eq!(paths, [expected.clone(), expected]);

    // Without a text, or of a message of someone else: refused before the engine.
    fake.forget_received();
    let empty = MessageBody::parse("").unwrap();
    assert_eq!(
        adapter.edit_message(&name, &mine, &empty).await,
        Err(EngineError::Invalid("message text"))
    );
    let theirs = FullMessageId::parse(&format!("false_{CHAT}_3EB0BEEF")).unwrap();
    assert!(
        adapter
            .edit_message(&name, &theirs, &MessageBody::to_send("x").unwrap())
            .await
            .is_err()
    );
    // The identifier of WhatsApp alone is not a complete one: there is no way to delete by it.
    assert!(FullMessageId::parse("3EB0BEEF").is_err());
    assert!(fake.received().is_empty());
}
