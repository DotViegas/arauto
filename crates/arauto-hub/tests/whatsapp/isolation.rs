//! A Hermes does not reach the number of another (`docs/05-modelo-de-ameacas.md`, threat 14;
//! `docs/15-mvp.md`, section 7, M3): not by the alias, not by the identifier of a chat or of
//! a message of another number, not from another organization; the answer does not say
//! whether the number exists; and `to` and `message_id` that are malicious are refused and
//! never reach the fake engine.

use arauto_protocol::errors::ErrorCode;
use serde_json::{Value, json};

use crate::agent::{ALL_VERBS, Agent, all, ok, refused};
use crate::world::{World, message, phone};

const NOW_S: i64 = 1_800_000_000;

/// A number with one chat and one message, and a Hermes with every verb on it. Returns the
/// identifier of the chat and of the message, as that Hermes read them.
async fn number_of(world: &World, alias: &str, n: u32, agent: &mut Agent) -> (String, Value) {
    let number = world.connected(alias, &phone(n)).await;
    let session = number.session.as_str().to_owned();
    let chat = format!("{}@c.us", phone(n + 100));
    world.fake.add_chat(&session, &chat, "Someone");
    world.fake.add_message(
        &session,
        &chat,
        message(&chat, "3EB0F0000001", NOW_S, true, "mine"),
    );
    world.grant(&agent.name, alias, all(ALL_VERBS)).await;
    let listed = ok(agent
        .call("whatsapp_list_chats", json!({"number": alias}))
        .await);
    let chat = listed["chats"][0]["chat"].as_str().unwrap().to_owned();
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"number": alias, "chat": chat}))
        .await);
    (chat, read["messages"][0]["message_id"].clone())
}

/// Every way of naming the number, chat or message of someone else, as a list of calls.
fn reaching(number: Option<&str>, chat: &str, message: &Value) -> Vec<(&'static str, Value)> {
    let with = |mut args: Value| {
        if let Some(number) = number {
            args["number"] = json!(number);
        }
        args
    };
    vec![
        ("whatsapp_read_chat", with(json!({"chat": chat}))),
        ("whatsapp_send_text", with(json!({"to": chat, "text": "x"}))),
        ("whatsapp_mark_read", with(json!({"chat": chat}))),
        ("whatsapp_get_contact", with(json!({"chat": chat}))),
        (
            "whatsapp_download_media",
            with(json!({"message_id": message})),
        ),
        (
            "whatsapp_react",
            with(json!({"message_id": message, "reaction": "x"})),
        ),
        (
            "whatsapp_delete_message",
            with(json!({"message_id": message})),
        ),
        (
            "whatsapp_edit_message",
            with(json!({"message_id": message, "text": "x"})),
        ),
        (
            "whatsapp_forward",
            with(json!({"message_id": message, "to": chat})),
        ),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hermes_does_not_reach_the_number_of_another() {
    let world = World::new().await;
    let mut owner = world.agent().await;
    let mut other = world.agent().await;
    let (theirs_chat, theirs_message) = number_of(&world, "deles", 10, &mut owner).await;
    let (ours_chat, ours_message) = number_of(&world, "nosso", 11, &mut other).await;
    world.fake.forget_received();

    // By the alias: the number of another and a number that does not exist are one answer.
    for alias in ["deles", "nao-existe"] {
        for tool in [
            "whatsapp_status",
            "whatsapp_list_chats",
            "whatsapp_list_groups",
        ] {
            let outcome = other.call(tool, json!({"number": alias})).await;
            assert_eq!(refused(&outcome), Some(ErrorCode::Ar701), "{tool} {alias}");
        }
        let outcome = other
            .call(
                "whatsapp_check_number",
                json!({"number": alias, "phone": phone(90)}),
            )
            .await;
        assert_eq!(refused(&outcome), Some(ErrorCode::Ar701), "{alias}");
    }
    // By the identifier of a chat or a message of the other number, through its own number:
    // the chat or the message that does not exist.
    for (tool, args) in reaching(Some("nosso"), &theirs_chat, &theirs_message) {
        let code = refused(&other.call(tool, args.clone()).await);
        assert!(
            matches!(code, Some(ErrorCode::Ar705 | ErrorCode::Ar712)),
            "{tool}: {code:?}"
        );
    }
    // Leaving the number out: the only number of this Hermes is its own.
    for (tool, args) in reaching(None, &theirs_chat, &theirs_message) {
        let code = refused(&other.call(tool, args).await);
        assert!(
            matches!(code, Some(ErrorCode::Ar705 | ErrorCode::Ar712)),
            "{tool}: {code:?}"
        );
    }
    // A message of its own number forwarded to a chat of the other: refused too.
    let crossed = other
        .call(
            "whatsapp_forward",
            json!({"message_id": ours_message, "to": theirs_chat}),
        )
        .await;
    assert_eq!(refused(&crossed), Some(ErrorCode::Ar705));
    // Nothing of it reached the engine, for any session.
    assert_eq!(world.fake.received(), [], "the engine got a request");
    assert!(world.fake.sent().is_empty());

    // With two numbers, leaving it out is not a choice the hub makes.
    world.grant(&owner.name, "nosso", all(ALL_VERBS)).await;
    let ambiguous = owner.call("whatsapp_status", json!({})).await;
    assert_eq!(refused(&ambiguous), Some(ErrorCode::Ar711));
    // And what it may use, it uses.
    ok(owner
        .call(
            "whatsapp_read_chat",
            json!({"number": "nosso", "chat": ours_chat}),
        )
        .await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hermes_does_not_reach_a_number_of_another_organization() {
    let here = World::new().await;
    let there = World::new().await;
    let mut ours = here.agent().await;
    let mut theirs = there.agent().await;
    // The same alias in both.
    let (_, _) = number_of(&here, "balcao", 20, &mut ours).await;
    let (their_chat, their_message) = number_of(&there, "balcao", 21, &mut theirs).await;
    here.fake.forget_received();
    there.fake.forget_received();
    for (tool, args) in reaching(Some("balcao"), &their_chat, &their_message) {
        let code = refused(&ours.call(tool, args.clone()).await);
        assert!(
            matches!(code, Some(ErrorCode::Ar705 | ErrorCode::Ar712)),
            "{tool}: {code:?}"
        );
    }
    assert_eq!(here.fake.received(), []);
    assert_eq!(there.fake.received(), []);
}

/// `to`, `chat` and `message_id` that are malicious: refused with AR-711 (the form) or AR-712
/// (a form the hub did not sign), and the fake engine gets no request.
#[tokio::test(flavor = "multi_thread")]
async fn malicious_destinations_and_identifiers_never_reach_the_engine() {
    let world = World::new().await;
    let mut agent = world.agent().await;
    let (chat, signed) = number_of(&world, "balcao", 30, &mut agent).await;
    // A hostile engine lists, in this chat, a message that names another chat.
    let number_session = world.number("balcao").await.session.as_str().to_owned();
    let mut foreign = message(
        &format!("{}@c.us", phone(131)),
        "3EB0F0000099",
        NOW_S,
        false,
        "x",
    );
    foreign["timestamp"] = json!(NOW_S + 50);
    world
        .fake
        .add_message(&number_session, &format!("{}@c.us", phone(130)), foreign);
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let foreign_id = read["messages"][0]["message_id"].clone();
    world.fake.forget_received();
    let outcome = agent
        .call(
            "whatsapp_react",
            json!({"message_id": foreign_id, "reaction": "x"}),
        )
        .await;
    assert_eq!(refused(&outcome), Some(ErrorCode::Ar712));
    let session = world.number("balcao").await.session.as_str().to_owned();
    for to in [
        "../../api/sessions",
        "5511987600130@c.us",
        "120363000000000001@g.us",
        &format!("../{session}"),
        "+55 11 9%2e%2e",
        "5511\u{0}987600130",
        "abc",
    ] {
        let outcome = agent
            .call("whatsapp_send_text", json!({"to": to, "text": "x"}))
            .await;
        assert_eq!(refused(&outcome), Some(ErrorCode::Ar711), "{to:?}");
    }
    let message_text = signed.as_str().unwrap().to_owned();
    // The signed identifier changed by one character, or made up.
    let mut changed: Vec<char> = message_text.chars().collect();
    let last = changed.len() - 1;
    changed[last] = if changed[last] == 'A' { 'B' } else { 'A' };
    let changed: String = changed.into_iter().collect();
    let made_up = data_encoding::BASE64URL_NOPAD.encode(
        format!(
            "0123456789abcdef{}true_5511987600130@c.us_3EB0F0000001",
            "s".repeat(32)
        )
        .as_bytes(),
    );
    // A real chat of this number, with an identifier of a message the hub never signed.
    let real = data_encoding::BASE64URL_NOPAD
        .decode(message_text.as_bytes())
        .unwrap();
    let mut unsigned = real[..16].to_vec();
    unsigned.extend_from_slice(&[0u8; 32]);
    unsigned.extend_from_slice(format!("true_{}@c.us_3EB0FORGED01", phone(130)).as_bytes());
    let unsigned = data_encoding::BASE64URL_NOPAD.encode(&unsigned);
    for given in [changed.as_str(), made_up.as_str(), unsigned.as_str()] {
        for tool in ["whatsapp_delete_message", "whatsapp_download_media"] {
            let outcome = agent.call(tool, json!({"message_id": given})).await;
            assert_eq!(refused(&outcome), Some(ErrorCode::Ar712), "{tool}");
        }
    }
    for given in ["../../x", "true_5511987600130@c.us_3EB0F0000001", ""] {
        let outcome = agent
            .call("whatsapp_delete_message", json!({"message_id": given}))
            .await;
        assert_eq!(refused(&outcome), Some(ErrorCode::Ar711), "{given:?}");
    }
    for chat in ["../x", "5511987600130@c.us", &session] {
        let outcome = agent
            .call("whatsapp_read_chat", json!({"chat": chat}))
            .await;
        assert_eq!(refused(&outcome), Some(ErrorCode::Ar711), "{chat:?}");
    }
    // A chat identifier in the right form that the hub never gave.
    let unknown = agent
        .call(
            "whatsapp_read_chat",
            json!({"chat": "0b9c5f5e-6f0a-4c3e-9d54-1c2f3a4b5c6d"}),
        )
        .await;
    assert_eq!(refused(&unknown), Some(ErrorCode::Ar705));
    assert_eq!(world.fake.received(), [], "the engine got a request");
    // The real ones still work.
    ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    ok(agent
        .call("whatsapp_delete_message", json!({"message_id": signed}))
        .await);
}
