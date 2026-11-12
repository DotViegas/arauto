//! The WhatsApp tools, called by a Hermes in session, answered by the hub through the adapter
//! of the WAHA against the fake engine: what each one answers, the list of tools that follows
//! the matrix, taking an access away at the next call, the budget of a reading, forwarding
//! with its two verbs, editing and deleting with the complete identifier, and the states of a
//! number.

use std::sync::Arc;
use std::time::Duration;

use arauto_hub::admin::whatsapp::PairOutcome;
use arauto_hub::core::access::Grant;
use arauto_protocol::clock::SystemClock;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::CallOutcome;
use serde_json::{Value, json};

use crate::agent::{ALL_VERBS, Agent, NONE, READ, READ_SEND, all, list, names, ok, refused, verbs};
use crate::world::{World, message, phone};

const NOW_S: i64 = 1_800_000_000;

fn contact(n: u32) -> String {
    format!("{}@c.us", phone(n))
}

const READ_TOOLS: [&str; 7] = [
    "whatsapp_status",
    "whatsapp_list_chats",
    "whatsapp_read_chat",
    "whatsapp_get_contact",
    "whatsapp_download_media",
    "whatsapp_list_groups",
    "whatsapp_get_group",
];

const SEND_TOOLS: [&str; 7] = [
    "whatsapp_check_number",
    "whatsapp_send_text",
    "whatsapp_send_file",
    "whatsapp_mark_read",
    "whatsapp_set_typing",
    "whatsapp_react",
    "whatsapp_forward",
];

fn expected(families: &[&[&str]]) -> Vec<String> {
    let mut names = vec!["arauto_status".to_owned()];
    for family in families {
        names.extend(family.iter().map(|name| (*name).to_owned()));
    }
    names
}

/// A world with one connected number, `balcao`, a chat with Ana and two messages in it.
async fn balcao() -> (World, String, String) {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let ana = contact(50);
    world.fake.add_chat(&session, &ana, "Ana");
    world.fake.register(&phone(50), &ana);
    world.fake.add_message(
        &session,
        &ana,
        message(&ana, "3EB0A0000001", NOW_S, false, "hi there"),
    );
    world.fake.add_message(
        &session,
        &ana,
        message(&ana, "3EB0A0000002", NOW_S + 5, true, "hello Ana"),
    );
    (world, session, ana)
}

/// The identifier the hub gave the only chat a listing shows.
async fn first_chat(agent: &mut Agent) -> String {
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    listed["chats"][0]["chat"].as_str().unwrap().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_list_of_tools_follows_the_matrix_in_the_session_and_in_tools() {
    let (world, _, _) = balcao().await;
    let mut agent = world.agent().await;
    // Connecting is not permitting: only the status.
    assert_eq!(agent.started_with(), ["arauto_status"]);
    let status = ok(agent.call("arauto_status", json!({})).await);
    assert_eq!(status["numbers"], json!([]));

    world.grant(&agent.name, "balcao", all(READ)).await;
    assert_eq!(agent.next_tools().await, expected(&[&READ_TOOLS]));
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    assert_eq!(
        agent.next_tools().await,
        expected(&[&READ_TOOLS, &SEND_TOOLS])
    );
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    assert_eq!(
        agent.next_tools().await,
        expected(&[
            &READ_TOOLS,
            &SEND_TOOLS,
            &["whatsapp_edit_message", "whatsapp_delete_message"]
        ])
    );
    // The format of a number in `arauto_status`: no telephone, no session of the engine.
    let status = ok(agent.call("arauto_status", json!({})).await);
    assert_eq!(
        status["numbers"],
        json!([{"number": "balcao", "state": "connected", "scope": "all",
                "verbs": ["read", "send", "modify", "wake"], "chat_verbs": [],
                "can_start_chats": false}])
    );
    let text = Value::Object(status).to_string();
    assert!(!text.contains(&phone(1)), "{text}");

    // A new session starts with the list of now.
    let mut second = world.agent().await;
    assert_eq!(
        second.started_with(),
        ["arauto_status"],
        "another Hermes has nothing"
    );
    world.grant(&second.name, "balcao", list(NONE)).await;
    // A scope "list" without lines releases nothing either.
    assert!(second.quiet_for(Duration::from_millis(800)).await);
    let phone_of_ana = format!("+{}", phone(50));
    world
        .line(&second.name, "balcao", &phone_of_ana, READ)
        .await;
    assert_eq!(second.next_tools().await, expected(&[&READ_TOOLS]));

    // Taken away: back to the status alone.
    world.take_away(&agent.name, "balcao").await;
    assert_eq!(agent.next_tools().await, ["arauto_status"]);
}

/// "Taking an access away holds at the next call": with the session open, the old list of
/// tools in hand and no sweep or notice to tell it, the next call is refused.
#[tokio::test(flavor = "multi_thread")]
async fn taking_an_access_away_holds_at_the_next_call_with_the_old_list() {
    let world = World::with(
        |settings| settings.sweep_interval_ms = 3_600_000,
        Arc::new(SystemClock),
    )
    .await;
    let number = world.connected("balcao", &phone(2)).await;
    let session = number.session.as_str().to_owned();
    let ana = contact(51);
    world.fake.add_chat(&session, &ana, "Ana");
    world.fake.add_message(
        &session,
        &ana,
        message(&ana, "3EB0B0000001", NOW_S, false, "hi"),
    );
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    let chat = first_chat(&mut agent).await;
    assert!(
        ok(agent
            .call("whatsapp_read_chat", json!({"chat": chat}))
            .await)["messages"]
            .as_array()
            .is_some_and(|messages| messages.len() == 1)
    );

    // A line that blocks the chat holds at the next call.
    world.line(&agent.name, "balcao", &chat, NONE).await;
    let blocked = agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await;
    assert_eq!(refused(&blocked), Some(ErrorCode::Ar705));
    let blocked = agent
        .call("whatsapp_send_text", json!({"to": chat, "text": "x"}))
        .await;
    assert_eq!(refused(&blocked), Some(ErrorCode::Ar705));
    assert!(world.fake.sent().is_empty());

    // The access taken away holds at the next call too.
    world.take_away(&agent.name, "balcao").await;
    for (tool, args) in [
        ("whatsapp_status", json!({})),
        ("whatsapp_list_chats", json!({})),
        ("whatsapp_check_number", json!({"phone": phone(52)})),
    ] {
        let outcome = agent.call(tool, args).await;
        assert_eq!(refused(&outcome), Some(ErrorCode::Ar701), "{tool}");
    }
    // Nothing told the session: it still has the old list, and no `tools` came.
    assert!(agent.tools.is_empty());
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn the_reading_tools_answer_through_the_engine() {
    let (world, session, ana) = balcao().await;
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ)).await;

    let status = ok(agent.call("whatsapp_status", json!({})).await);
    assert_eq!(status["number"], "balcao");
    assert_eq!(status["state"], "connected");
    assert_eq!(status["connected"], true);

    let listed = ok(agent
        .call("whatsapp_list_chats", json!({"number": "balcao"}))
        .await);
    let chats = listed["chats"].as_array().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0]["kind"], "contact");
    assert_eq!(chats[0]["name"], "Ana");
    assert_eq!(chats[0]["phone"], format!("+{}", phone(50)));
    assert_eq!(chats[0]["last_message"]["text"], "hello Ana");
    assert_eq!(listed["more"], false);
    let chat = chats[0]["chat"].as_str().unwrap().to_owned();
    // The same chat keeps its identifier.
    assert_eq!(first_chat(&mut agent).await, chat);

    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let messages = read["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["text"], "hello Ana");
    assert_eq!(messages[0]["from_me"], true);
    assert_eq!(messages[0]["time"], NOW_S + 5);
    assert_eq!(messages[1]["author"], format!("+{}", phone(50)));
    assert_eq!(read["more"], false);
    // The identifier of a message is the same at every reading.
    let again = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    assert_eq!(
        again["messages"][1]["message_id"],
        messages[1]["message_id"]
    );
    // `before` and `after` move through the time.
    let older = ok(agent
        .call(
            "whatsapp_read_chat",
            json!({"chat": chat, "before": NOW_S + 5}),
        )
        .await);
    assert_eq!(older["messages"].as_array().unwrap().len(), 1);
    assert_eq!(older["messages"][0]["text"], "hi there");
    let newer = ok(agent
        .call(
            "whatsapp_read_chat",
            json!({"chat": chat, "after": NOW_S + 1}),
        )
        .await);
    assert_eq!(newer["messages"].as_array().unwrap().len(), 1);

    world.fake.add_contact(&ana, "Ana Agenda", "Ana", "Busy");
    let person = ok(agent
        .call("whatsapp_get_contact", json!({"chat": chat}))
        .await);
    assert_eq!(person["name"], "Ana Agenda");
    assert_eq!(person["push_name"], "Ana");
    assert_eq!(person["about"], "Busy");

    // A file.
    let mut with_file = message(&ana, "3EB0A0000003", NOW_S + 9, false, "");
    with_file["hasMedia"] = json!(true);
    let stored = format!("false_{ana}_3EB0A0000003.pdf");
    with_file["media"] =
        json!({"file": stored, "mimetype": "application/pdf", "filename": "a.pdf"});
    world.fake.add_message(&session, &ana, with_file);
    world
        .fake
        .add_file(&stored, "application/pdf", b"%PDF-1.4 fake".to_vec());
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let newest = &read["messages"][0];
    assert_eq!(newest["file"]["mimetype"], "application/pdf");
    let file = ok(agent
        .call(
            "whatsapp_download_media",
            json!({"message_id": newest["message_id"]}),
        )
        .await);
    assert_eq!(file["mimetype"], "application/pdf");
    assert_eq!(file["filename"], "a.pdf");
    assert_eq!(
        data_encoding::BASE64
            .decode(file["data"].as_str().unwrap().as_bytes())
            .unwrap(),
        b"%PDF-1.4 fake"
    );

    // Groups.
    let group = "120363000000000001@g.us";
    let alice = contact(53);
    let bob = contact(54);
    world.fake.add_group(
        &session,
        group,
        "The team",
        &[
            (alice.as_str(), "superadmin"),
            (bob.as_str(), "participant"),
        ],
    );
    let groups = ok(agent.call("whatsapp_list_groups", json!({})).await);
    let groups = groups["groups"].as_array().unwrap().clone();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["subject"], "The team");
    assert_eq!(groups[0]["participants"], 2);
    let group_chat = groups[0]["chat"].as_str().unwrap();
    let detail = ok(agent
        .call(
            "whatsapp_get_group",
            json!({"chat": group_chat, "limit": 1}),
        )
        .await);
    assert_eq!(detail["total"], 2);
    assert_eq!(detail["participants"].as_array().unwrap().len(), 1);
    assert_eq!(detail["next_offset"], 1);
    // A group is not a contact, and a contact is not a group.
    let wrong = agent
        .call("whatsapp_get_contact", json!({"chat": group_chat}))
        .await;
    assert_eq!(refused(&wrong), Some(ErrorCode::Ar711));
    let wrong = agent
        .call("whatsapp_get_group", json!({"chat": chat}))
        .await;
    assert_eq!(refused(&wrong), Some(ErrorCode::Ar711));

    // Reading is not sending.
    let send = agent
        .call("whatsapp_send_text", json!({"to": chat, "text": "x"}))
        .await;
    assert_eq!(refused(&send), Some(ErrorCode::Ar701));
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread")]
async fn the_sending_tools_answer_through_the_engine() {
    let (world, _, ana) = balcao().await;
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    let chat = first_chat(&mut agent).await;

    // Asking about a telephone.
    world.fake.register(&phone(50), &ana);
    let checked = ok(agent
        .call(
            "whatsapp_check_number",
            json!({"phone": format!("+{}", phone(50))}),
        )
        .await);
    assert_eq!(checked["has_whatsapp"], true);
    assert_eq!(checked["exact"], format!("+{}", phone(50)));
    let unknown = ok(agent
        .call("whatsapp_check_number", json!({"phone": phone(60)}))
        .await);
    assert_eq!(unknown["has_whatsapp"], false);

    // A text to the chat, and to its telephone: the same chat.
    let sent = ok(agent
        .call("whatsapp_send_text", json!({"to": chat, "text": "one"}))
        .await);
    assert_eq!(sent["chat"], chat);
    assert!(sent["message_id"].is_string());
    let by_phone = ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": format!("+{}", phone(50)), "text": "two"}),
        )
        .await);
    assert_eq!(by_phone["chat"], chat);
    // Answering a message of the chat.
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let theirs = read["messages"][1]["message_id"].clone();
    ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "three", "reply_to": theirs}),
        )
        .await);
    let sent = world.fake.sent();
    let texts: Vec<&Value> = sent
        .iter()
        .filter(|(action, _)| action == "sendText")
        .map(|(_, body)| body)
        .collect();
    assert_eq!(texts.len(), 3);
    assert_eq!(texts[0]["chatId"], ana);
    assert_eq!(texts[0]["text"], "one");
    assert_eq!(texts[2]["reply_to"], format!("false_{ana}_3EB0A0000001"));

    // Someone who never talked to the number: only with "may start chats".
    let stranger = contact(61);
    world.fake.register(&phone(61), &stranger);
    let new = agent
        .call(
            "whatsapp_send_text",
            json!({"to": format!("+{}", phone(61)), "text": "hello"}),
        )
        .await;
    assert_eq!(refused(&new), Some(ErrorCode::Ar701));
    // A telephone without WhatsApp is a chat that does not exist.
    let none = agent
        .call(
            "whatsapp_send_text",
            json!({"to": phone(62), "text": "hello"}),
        )
        .await;
    assert_eq!(refused(&none), Some(ErrorCode::Ar705));
    world
        .grant(
            &agent.name,
            "balcao",
            Grant {
                can_start_chats: true,
                ..all(READ_SEND)
            },
        )
        .await;
    let started = ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": format!("+{}", phone(61)), "text": "hello"}),
        )
        .await);
    assert_ne!(started["chat"], chat);

    // A file, a reaction, read and typing.
    let file = ok(agent
        .call(
            "whatsapp_send_file",
            json!({"to": chat, "kind": "document", "mimetype": "application/pdf",
                   "filename": "report.pdf", "data": data_encoding::BASE64.encode(b"%PDF"),
                   "caption": "the report"}),
        )
        .await);
    assert_eq!(file["chat"], chat);
    ok(agent
        .call(
            "whatsapp_react",
            json!({"message_id": theirs, "reaction": "\u{1F44D}"}),
        )
        .await);
    ok(agent
        .call("whatsapp_mark_read", json!({"chat": chat}))
        .await);
    ok(agent
        .call("whatsapp_set_typing", json!({"chat": chat, "on": true}))
        .await);
    let actions: Vec<String> = world
        .fake
        .sent()
        .into_iter()
        .map(|(action, _)| action)
        .collect();
    for action in ["sendFile", "reaction", "sendSeen", "startTyping"] {
        assert!(
            actions.contains(&action.to_owned()),
            "{action}: {actions:?}"
        );
    }
    // A reply to a message of another chat is refused.
    let wrong = agent
        .call(
            "whatsapp_send_text",
            json!({"to": started["chat"], "text": "x", "reply_to": theirs}),
        )
        .await;
    assert_eq!(refused(&wrong), Some(ErrorCode::Ar711));
}

/// `whatsapp_forward` needs reading in the chat of the message and sending in the other
/// (`docs/09-contrato-mcp.md`, section 3).
#[tokio::test(flavor = "multi_thread")]
async fn forwarding_needs_reading_at_the_origin_and_sending_at_the_destination() {
    let (world, session, _) = balcao().await;
    let bruno = contact(55);
    world.fake.add_chat(&session, &bruno, "Bruno");
    world.fake.add_message(
        &session,
        &bruno,
        message(&bruno, "3EB0C0000001", NOW_S, false, "yo"),
    );
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", list(NONE)).await;
    let ana_phone = format!("+{}", phone(50));
    let bruno_phone = format!("+{}", phone(55));
    world.fake.register(&phone(50), &contact(50));
    world.fake.register(&phone(55), &bruno);
    // Reading in both: the message is read in Ana's chat, and Bruno's chat cannot be written.
    world.line(&agent.name, "balcao", &ana_phone, READ).await;
    world.line(&agent.name, "balcao", &bruno_phone, READ).await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    let chat_of = |name: &str| {
        listed["chats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|chat| chat["name"] == name)
            .unwrap()["chat"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (ana, bruno_chat) = (chat_of("Ana"), chat_of("Bruno"));
    let read = ok(agent.call("whatsapp_read_chat", json!({"chat": ana})).await);
    let message_id = read["messages"][0]["message_id"].clone();
    let forward = json!({"message_id": message_id, "to": bruno_chat});
    // The family of sending is not released yet.
    assert_eq!(
        refused(&agent.call("whatsapp_forward", forward.clone()).await),
        Some(ErrorCode::Ar701)
    );
    // Sending at the origin and at the destination, and no reading at the origin.
    world
        .line(&agent.name, "balcao", &bruno_phone, READ_SEND)
        .await;
    world
        .line(
            &agent.name,
            "balcao",
            &ana_phone,
            verbs(false, true, false, false),
        )
        .await;
    assert_eq!(
        refused(&agent.call("whatsapp_forward", forward.clone()).await),
        Some(ErrorCode::Ar712)
    );
    // Reading at the origin, and no sending at the destination (the family of sending stays
    // released by the number).
    world
        .grant(
            &agent.name,
            "balcao",
            list(verbs(false, true, false, false)),
        )
        .await;
    world.line(&agent.name, "balcao", &ana_phone, READ).await;
    world.line(&agent.name, "balcao", &bruno_phone, READ).await;
    assert_eq!(
        refused(&agent.call("whatsapp_forward", forward.clone()).await),
        Some(ErrorCode::Ar705)
    );
    assert!(world.fake.sent().is_empty(), "nothing reached the engine");
    // Both: reading at the origin, sending at the destination.
    world
        .line(
            &agent.name,
            "balcao",
            &bruno_phone,
            verbs(false, true, false, false),
        )
        .await;
    let forwarded = ok(agent.call("whatsapp_forward", forward).await);
    assert_eq!(forwarded["chat"], bruno_chat);
    let sent = world.fake.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "forwardMessage");
    assert_eq!(sent[0].1["chatId"], bruno);
}

/// Editing without a text, editing or deleting by an identifier that is not complete, are
/// refused before the engine (`docs/09-contrato-mcp.md`, section 4).
#[tokio::test(flavor = "multi_thread")]
async fn editing_and_deleting_need_modify_and_the_complete_identifier() {
    let (world, session, ana) = balcao().await;
    // A message the engine gave with the identifier of WhatsApp alone.
    let mut plain = message(&ana, "x", NOW_S + 7, true, "plain");
    plain["id"] = json!("3EB0PLAIN00001");
    world.fake.add_message(&session, &ana, plain);
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    let chat = first_chat(&mut agent).await;
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let messages = read["messages"].as_array().unwrap().clone();
    let plain = messages.iter().find(|m| m["text"] == "plain").unwrap()["message_id"].clone();
    let mine = messages.iter().find(|m| m["text"] == "hello Ana").unwrap()["message_id"].clone();
    let theirs = messages.iter().find(|m| m["text"] == "hi there").unwrap()["message_id"].clone();
    // Without modify, the family is not released.
    let edit = agent
        .call(
            "whatsapp_edit_message",
            json!({"message_id": mine, "text": "fixed"}),
        )
        .await;
    assert_eq!(refused(&edit), Some(ErrorCode::Ar701));
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    let no_text = agent
        .call("whatsapp_edit_message", json!({"message_id": mine}))
        .await;
    assert_eq!(refused(&no_text), Some(ErrorCode::Ar711));
    let edit_plain = agent
        .call(
            "whatsapp_edit_message",
            json!({"message_id": plain, "text": "x"}),
        )
        .await;
    assert_eq!(refused(&edit_plain), Some(ErrorCode::Ar711));
    let delete_plain = agent
        .call("whatsapp_delete_message", json!({"message_id": plain}))
        .await;
    assert_eq!(refused(&delete_plain), Some(ErrorCode::Ar711));
    // A message of the other side cannot be edited.
    let theirs_edit = agent
        .call(
            "whatsapp_edit_message",
            json!({"message_id": theirs, "text": "x"}),
        )
        .await;
    assert_eq!(refused(&theirs_edit), Some(ErrorCode::Ar711));
    assert!(world.fake.edited().is_empty() && world.fake.deleted().is_empty());
    ok(agent
        .call(
            "whatsapp_edit_message",
            json!({"message_id": mine, "text": "fixed"}),
        )
        .await);
    ok(agent
        .call("whatsapp_delete_message", json!({"message_id": mine}))
        .await);
    assert_eq!(world.fake.edited().len(), 1);
    assert_eq!(world.fake.edited()[0].2["text"], "fixed");
    assert_eq!(world.fake.deleted().len(), 1);
}

/// `whatsapp_read_chat`: 30 by default, at most 200, within 256 KiB, cut at whole messages,
/// saying whether more remain; following `next_before` reads every message once.
#[tokio::test(flavor = "multi_thread")]
async fn a_reading_keeps_to_its_budget_and_loses_no_message() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(3)).await;
    let session = number.session.as_str().to_owned();
    let big = contact(70);
    world.fake.add_chat(&session, &big, "Big");
    let text = "x".repeat(60 * 1024);
    for n in 0..12 {
        // Three messages in each second: a page ends between them.
        let at = NOW_S + n / 3;
        world.fake.add_message(
            &session,
            &big,
            message(&big, &format!("3EB0D{n:07}"), at, false, &text),
        );
    }
    let small = contact(71);
    world.fake.add_chat(&session, &small, "Small");
    for n in 0..45 {
        world.fake.add_message(
            &session,
            &small,
            message(&small, &format!("3EB0E{n:07}"), NOW_S + n, false, "short"),
        );
    }
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ)).await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    let chat_of = |name: &str| {
        listed["chats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|chat| chat["name"] == name)
            .unwrap()["chat"]
            .as_str()
            .unwrap()
            .to_owned()
    };

    let small_chat = chat_of("Small");
    let page = ok(agent
        .call("whatsapp_read_chat", json!({"chat": small_chat}))
        .await);
    assert_eq!(page["messages"].as_array().unwrap().len(), 30);
    assert_eq!(page["more"], true);
    let all_of_it = ok(agent
        .call(
            "whatsapp_read_chat",
            json!({"chat": small_chat, "limit": 200}),
        )
        .await);
    assert_eq!(all_of_it["messages"].as_array().unwrap().len(), 45);
    assert_eq!(all_of_it["more"], false);
    assert_eq!(all_of_it["next_before"], Value::Null);

    let big_chat = chat_of("Big");
    let mut seen = Vec::new();
    let mut before: Option<i64> = None;
    for _ in 0..12 {
        let mut args = json!({"chat": big_chat, "limit": 200});
        if let Some(before) = before {
            args["before"] = json!(before);
        }
        let outcome = agent.call("whatsapp_read_chat", args).await;
        let CallOutcome::Ok(page) = outcome else {
            panic!("refused: {outcome:?}");
        };
        let size = serde_json::to_vec(page.as_map()).unwrap().len();
        assert!(size <= 256 * 1024, "{size} bytes");
        let page = page.into_map();
        let messages = page["messages"].as_array().unwrap();
        assert!(!messages.is_empty());
        for message in messages {
            seen.push(message["message_id"].as_str().unwrap().to_owned());
        }
        if page["more"] == false {
            break;
        }
        before = page["next_before"].as_i64();
        assert!(before.is_some());
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 12, "every message once: {} read", seen.len());
    assert_eq!(seen.len(), 12, "and none twice");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_number_that_is_not_connected_answers_ar702_and_a_swapped_one_stops_its_accesses() {
    let (world, _, _) = balcao().await;
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    let chat = first_chat(&mut agent).await;
    world
        .fixture
        .admin
        .number_disconnect(&world.vault, "balcao", world.now())
        .await
        .unwrap();
    let status = ok(agent.call("whatsapp_status", json!({})).await);
    assert_eq!(status["state"], "disconnected");
    assert_eq!(status["connected"], false);
    for (tool, args) in [
        ("whatsapp_read_chat", json!({"chat": chat})),
        ("whatsapp_send_text", json!({"to": chat, "text": "x"})),
        ("whatsapp_list_chats", json!({})),
    ] {
        assert_eq!(
            refused(&agent.call(tool, args).await),
            Some(ErrorCode::Ar702),
            "{tool}"
        );
    }

    // Another telephone paired the number "trocado": its accesses wait for the admin.
    world.add("trocado", &phone(4)).await;
    assert_eq!(world.pair("trocado", &phone(5)).await, PairOutcome::Swapped);
    let mut second = world.agent().await;
    world.grant(&second.name, "trocado", all(READ_SEND)).await;
    assert!(second.quiet_for(Duration::from_millis(800)).await);
    assert_eq!(second.started_with(), ["arauto_status"]);
    assert_eq!(
        refused(&second.call("whatsapp_status", json!({})).await),
        Some(ErrorCode::Ar701),
        "nothing released"
    );
    // The first Hermes names it: stopped, AR-702.
    world.grant(&agent.name, "trocado", all(READ)).await;
    let swapped = agent
        .call("whatsapp_status", json!({"number": "trocado"}))
        .await;
    assert_eq!(refused(&swapped), Some(ErrorCode::Ar702));
    let status = ok(agent.call("arauto_status", json!({})).await);
    assert_eq!(status["numbers"][1]["state"], "number_swapped");
    // The admin accepts the telephone: the accesses work again, and the list follows.
    world
        .fixture
        .admin
        .number_confirm("trocado", world.now())
        .await
        .unwrap();
    assert_eq!(
        second.next_tools().await,
        expected(&[&READ_TOOLS, &SEND_TOOLS])
    );
    assert_eq!(names(second.tools.last().unwrap()).len(), 15);
}

/// With the sweep far away, the notice of the admin command is what tells the sessions: an
/// access given, and the number removed, which takes its accesses along.
#[tokio::test(flavor = "multi_thread")]
async fn the_notice_of_the_admin_command_brings_the_new_list() {
    let world = World::with(
        |settings| {
            settings.listen_admin = true;
            settings.sweep_interval_ms = 3_600_000;
        },
        Arc::new(SystemClock),
    )
    .await;
    world.connected("balcao", &phone(14)).await;
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ)).await;
    assert_eq!(agent.next_tools().await, expected(&[&READ_TOOLS]));
    world
        .fixture
        .admin
        .number_remove(
            &world.vault,
            "balcao",
            arauto_hub::admin::whatsapp::Removal::KeepInEngine,
            world.now(),
        )
        .await
        .unwrap();
    assert_eq!(agent.next_tools().await, ["arauto_status"]);
    let gone = agent
        .call("whatsapp_status", json!({"number": "balcao"}))
        .await;
    assert_eq!(refused(&gone), Some(ErrorCode::Ar701));
}

/// With the scope "list", a chat without a line does not exist for the Hermes, however it is
/// named; with "all", the verbs of the number hold in it.
#[tokio::test(flavor = "multi_thread")]
async fn a_list_scope_hides_the_chats_without_a_line() {
    let (world, session, _) = balcao().await;
    let bruno = contact(56);
    world.fake.add_chat(&session, &bruno, "Bruno");
    world.fake.register(&phone(56), &bruno);
    world.fake.add_message(
        &session,
        &bruno,
        message(&bruno, "3EB0H0000001", NOW_S, false, "yo"),
    );
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    assert_eq!(listed["chats"].as_array().unwrap().len(), 2);
    let bruno_chat = listed["chats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|chat| chat["name"] == "Bruno")
        .unwrap()["chat"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(agent
        .call("whatsapp_read_chat", json!({"chat": bruno_chat}))
        .await);

    world.grant(&agent.name, "balcao", list(READ_SEND)).await;
    world
        .line(&agent.name, "balcao", &format!("+{}", phone(50)), READ_SEND)
        .await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    let names: Vec<&Value> = listed["chats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chat| &chat["name"])
        .collect();
    assert_eq!(names, [&json!("Ana")]);
    for (tool, args) in [
        ("whatsapp_read_chat", json!({"chat": bruno_chat})),
        ("whatsapp_send_text", json!({"to": bruno_chat, "text": "x"})),
        (
            "whatsapp_send_text",
            json!({"to": format!("+{}", phone(56)), "text": "x"}),
        ),
        ("whatsapp_mark_read", json!({"chat": bruno_chat})),
    ] {
        assert_eq!(
            refused(&agent.call(tool, args).await),
            Some(ErrorCode::Ar705),
            "{tool}"
        );
    }
}
