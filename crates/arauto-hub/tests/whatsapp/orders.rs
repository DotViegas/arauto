//! Who gives a Hermes orders on WhatsApp (decision 0051, a proposal): the hub refuses every
//! Hermes writing in the chat where a Hermes takes orders, when one end of it is a number of
//! the hub and the admin recorded it. Reading is never refused, and no other chat is.
//!
//! Telephones are the ones of the other tests (`phone(n)`), which belong to no one.
//!
//! The setting: Hermes `h` takes orders on the telephone `BRIDGE` (its own WhatsApp bridge),
//! and the telephone `OPERATOR` gives it orders. Both are numbers of the hub, `operador` and
//! `ponte`. `g` is another Hermes. Both may read, send, modify and start chats on both numbers.

use std::collections::HashMap;

use arauto_hub::admin::AdminError;
use arauto_hub::admin::orders::OrdersEdit;
use arauto_hub::repo::orders::Invalid;
use arauto_hub::vault::WhatsappField;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::CallOutcome;
use arauto_testkit::engine::Fault;
use serde_json::{Map, Value, json};

use crate::agent::{ALL_VERBS, Agent, READ, all, list, ok, refused};
use crate::world::{World, message, phone};

const NOW_S: i64 = 1_800_000_000;

/// The telephone that gives orders: the number `operador`.
const OPERATOR: u32 = 1;
/// The telephone the bridge of `h` runs on: the number `ponte`.
const BRIDGE: u32 = 2;
const CUSTOMER: u32 = 50;
const CUSTOMER_B: u32 = 51;
const NEW_CUSTOMER: u32 = 52;
const GROUP: &str = "120363012345678901@g.us";

fn id(n: u32) -> String {
    format!("{}@c.us", phone(n))
}

fn plus(n: u32) -> String {
    format!("+{}", phone(n))
}

/// The same telephone without the ninth digit: the other form of a Brazilian mobile.
fn without_ninth(n: u32) -> String {
    let mut digits = phone(n);
    digits.remove(4);
    digits
}

fn grant() -> arauto_hub::core::access::Grant {
    arauto_hub::core::access::Grant {
        scope: arauto_hub::core::access::Scope::All,
        verbs: ALL_VERBS,
        can_start_chats: true,
    }
}

struct Ready {
    world: World,
    g: Agent,
    h: Agent,
    /// The session of `operador` and of `ponte` in the fake engine.
    operador: String,
    ponte: String,
}

/// Two numbers with their chats, and two Hermes with every verb on both. Nothing recorded yet.
/// `bridge_chat` says whether the engine has the chat of `operador` with the bridge: a test
/// that gives a chat a second identifier by hand needs the hub not to have that chat already.
async fn ready(bridge_chat: bool) -> Ready {
    let world = World::new().await;
    let operador = world
        .connected("operador", &phone(OPERATOR))
        .await
        .session
        .as_str()
        .to_owned();
    let ponte = world
        .connected("ponte", &phone(BRIDGE))
        .await
        .session
        .as_str()
        .to_owned();
    if bridge_chat {
        world.fake.add_chat(&operador, &id(BRIDGE), "Bridge");
        world.fake.add_message(
            &operador,
            &id(BRIDGE),
            message(&id(BRIDGE), "3EB0OR000001", NOW_S, false, "an order"),
        );
        world.fake.add_message(
            &operador,
            &id(BRIDGE),
            message(&id(BRIDGE), "3EB0OR000002", NOW_S + 1, true, "my note"),
        );
    }
    world.fake.add_chat(&operador, &id(CUSTOMER), "Ana");
    world.fake.add_message(
        &operador,
        &id(CUSTOMER),
        message(&id(CUSTOMER), "3EB0OR000003", NOW_S + 2, false, "hi"),
    );
    world.fake.add_chat(&operador, GROUP, "Team");
    world.fake.add_message(
        &operador,
        GROUP,
        message(GROUP, "3EB0OR000004", NOW_S + 3, false, "team"),
    );
    world.fake.add_chat(&ponte, &id(OPERATOR), "Operator");
    world.fake.add_message(
        &ponte,
        &id(OPERATOR),
        message(&id(OPERATOR), "3EB0PO000001", NOW_S, false, "an order"),
    );
    world.fake.add_chat(&ponte, &id(CUSTOMER_B), "Bia");
    world.fake.add_message(
        &ponte,
        &id(CUSTOMER_B),
        message(&id(CUSTOMER_B), "3EB0PO000002", NOW_S + 1, false, "hi"),
    );
    world.fake.register(&phone(NEW_CUSTOMER), &id(NEW_CUSTOMER));
    let g = world.agent().await;
    let h = world.agent().await;
    for agent in [&g, &h] {
        world.grant(&agent.name, "operador", grant()).await;
        world.grant(&agent.name, "ponte", grant()).await;
    }
    Ready {
        world,
        g,
        h,
        operador,
        ponte,
    }
}

/// A call on a number.
async fn go(agent: &mut Agent, tool: &str, number: &str, args: Value) -> CallOutcome {
    let mut args: Map<String, Value> = match args {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    args.insert("number".to_owned(), json!(number));
    agent.call(tool, Value::Object(args)).await
}

/// The chats of a number by name, as the hub names them.
async fn chats(agent: &mut Agent, number: &str) -> HashMap<String, String> {
    let listed = ok(go(agent, "whatsapp_list_chats", number, json!({})).await);
    listed["chats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chat| {
            (
                chat["name"].as_str().unwrap().to_owned(),
                chat["chat"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The messages of a chat by text: the identifier to give back.
async fn messages(agent: &mut Agent, number: &str, chat: &str) -> HashMap<String, Value> {
    let read = ok(go(agent, "whatsapp_read_chat", number, json!({"chat": chat})).await);
    read["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["text"].as_str().unwrap().to_owned(),
                m["message_id"].clone(),
            )
        })
        .collect()
}

async fn send(agent: &mut Agent, number: &str, to: &str) -> Option<ErrorCode> {
    refused(
        &go(
            agent,
            "whatsapp_send_text",
            number,
            json!({"to": to, "text": "hello"}),
        )
        .await,
    )
}

async fn record(
    world: &World,
    hermes: &str,
    takes: Option<u32>,
    from: &[u32],
    self_chat: bool,
) -> Result<arauto_hub::admin::orders::OrdersOutcome, AdminError> {
    let edit = OrdersEdit {
        takes_orders_on: Some(takes.map(plus)),
        self_chat,
        orders_from: Some(from.iter().map(|n| plus(*n)).collect()),
    };
    world
        .fixture
        .admin
        .hermes_orders_set(&world.vault, hermes, &edit, world.now())
        .await
}

/// `h` takes orders on the bridge, from the operator.
async fn record_the_usual(world: &World, hermes: &str) {
    record(world, hermes, Some(BRIDGE), &[OPERATOR], false)
        .await
        .unwrap();
}

fn sent(world: &World) -> usize {
    world.fake.sent().len()
}

fn requests(world: &World, needle: &str) -> usize {
    world
        .fake
        .received()
        .iter()
        .filter(|request| request.path.contains(needle))
        .count()
}

const AR705: Option<ErrorCode> = Some(ErrorCode::Ar705);
const AR712: Option<ErrorCode> = Some(ErrorCode::Ar712);

// ------------------------------------------------------------------------------------ the gate

/// Tests 1 and 10. SECURITY RULE (decision 0051): a writing in the chat between the telephone a
/// Hermes's WhatsApp runs on and a telephone that gives it orders is refused to every Hermes,
/// the one that takes the orders included; the record is read at each call.
#[tokio::test(flavor = "multi_thread")]
async fn the_chat_where_a_hermes_takes_orders_is_refused_to_every_hermes() {
    let mut r = ready(true).await;
    let by_name = chats(&mut r.g, "operador").await;
    let bridge = by_name["Bridge"].clone();
    let ana = by_name["Ana"].clone();
    // Nothing recorded: the number is open, and it writes there.
    assert_eq!(send(&mut r.g, "operador", &bridge).await, None);
    assert_eq!(sent(&r.world), 1);

    // The admin records it between two calls: the very next call is refused, for G and for H.
    record_the_usual(&r.world, &r.h.name).await;
    assert_eq!(send(&mut r.g, "operador", &bridge).await, AR705);
    assert_eq!(send(&mut r.h, "operador", &bridge).await, AR705);
    assert_eq!(sent(&r.world), 1, "nothing reached the engine");
    // Every other chat of the number stays as the accesses say.
    assert_eq!(send(&mut r.g, "operador", &ana).await, None);
    assert_eq!(sent(&r.world), 2);

    // Cleared: allowed again, at the next call.
    record(&r.world, &r.h.name, None, &[], false).await.unwrap();
    assert_eq!(send(&mut r.h, "operador", &bridge).await, None);
    assert_eq!(sent(&r.world), 3);
}

/// Test 2: the Hermes's own WhatsApp is a number of the hub, and the operator its other end
/// (decision 0051, D1): writing to the operator would pass for the Hermes.
#[tokio::test(flavor = "multi_thread")]
async fn the_chat_of_the_bridge_with_the_operator_is_refused_too() {
    let mut r = ready(true).await;
    let by_name = chats(&mut r.g, "ponte").await;
    let operator = by_name["Operator"].clone();
    let bia = by_name["Bia"].clone();
    assert_eq!(send(&mut r.g, "ponte", &operator).await, None);
    record_the_usual(&r.world, &r.h.name).await;
    assert_eq!(send(&mut r.g, "ponte", &operator).await, AR705);
    assert_eq!(send(&mut r.h, "ponte", &operator).await, AR705);
    // The customer chats of the bridge's number stay open.
    assert_eq!(send(&mut r.g, "ponte", &bia).await, None);
}

/// Test 3: a sending by telephone to a chat the hub does not have yet is matched on the
/// telephone given, in both forms, before the send limit and before the engine hears of it.
#[tokio::test(flavor = "multi_thread")]
async fn a_sending_by_telephone_is_refused_before_the_limit_and_the_engine() {
    let mut r = ready(true).await;
    record_the_usual(&r.world, &r.h.name).await;
    let forms = [plus(BRIDGE), without_ninth(BRIDGE), phone(BRIDGE)];
    // More refusals than the limit has places (20 a minute): none of them takes one.
    for round in 0..25 {
        let to = &forms[round % forms.len()];
        assert_eq!(send(&mut r.g, "operador", to).await, AR705, "{to}");
    }
    let file = go(
        &mut r.g,
        "whatsapp_send_file",
        "operador",
        json!({"to": plus(BRIDGE), "kind": "document", "mimetype": "application/pdf",
               "filename": "a.pdf", "data": data_encoding::BASE64.encode(b"%PDF")}),
    )
    .await;
    assert_eq!(refused(&file), AR705);
    assert_eq!(
        requests(&r.world, "check-exists"),
        0,
        "the engine was not asked"
    );
    assert_eq!(sent(&r.world), 0);
    // The limit is intact: twenty sendings to a customer by telephone all go through.
    for n in 0..20 {
        let to = plus(NEW_CUSTOMER);
        let outcome = go(
            &mut r.g,
            "whatsapp_send_text",
            "operador",
            json!({"to": to, "text": format!("m{n}")}),
        )
        .await;
        assert_eq!(refused(&outcome), None, "sending {n}");
    }
}

/// Test 4: every writing verb. A message of the chat is "message not found" (AR-712); the chat
/// itself is "chat not found" (AR-705).
#[tokio::test(flavor = "multi_thread")]
async fn every_verb_that_writes_is_refused_in_that_chat() {
    let mut r = ready(true).await;
    let by_name = chats(&mut r.g, "operador").await;
    let (bridge, ana) = (by_name["Bridge"].clone(), by_name["Ana"].clone());
    let in_bridge = messages(&mut r.g, "operador", &bridge).await;
    let in_ana = messages(&mut r.g, "operador", &ana).await;
    record_the_usual(&r.world, &r.h.name).await;

    for agent in [&mut r.g, &mut r.h] {
        let file = go(
            agent,
            "whatsapp_send_file",
            "operador",
            json!({"to": bridge, "kind": "document", "mimetype": "application/pdf",
                   "filename": "a.pdf", "data": data_encoding::BASE64.encode(b"%PDF")}),
        )
        .await;
        assert_eq!(refused(&file), AR705, "send_file");
        let forward = go(
            agent,
            "whatsapp_forward",
            "operador",
            json!({"message_id": in_ana["hi"], "to": bridge}),
        )
        .await;
        assert_eq!(refused(&forward), AR705, "forward into the chat");
        let seen = go(
            agent,
            "whatsapp_mark_read",
            "operador",
            json!({"chat": bridge}),
        )
        .await;
        assert_eq!(refused(&seen), AR705, "mark_read");
        let typing = go(
            agent,
            "whatsapp_set_typing",
            "operador",
            json!({"chat": bridge, "on": true}),
        )
        .await;
        assert_eq!(refused(&typing), AR705, "set_typing");
        let react = go(
            agent,
            "whatsapp_react",
            "operador",
            json!({"message_id": in_bridge["an order"], "reaction": "\u{1F44D}"}),
        )
        .await;
        assert_eq!(refused(&react), AR712, "react");
        let edit = go(
            agent,
            "whatsapp_edit_message",
            "operador",
            json!({"message_id": in_bridge["my note"], "text": "x"}),
        )
        .await;
        assert_eq!(refused(&edit), AR712, "edit");
        let delete = go(
            agent,
            "whatsapp_delete_message",
            "operador",
            json!({"message_id": in_bridge["my note"]}),
        )
        .await;
        assert_eq!(refused(&delete), AR712, "delete");
    }
    assert!(r.world.fake.sent().is_empty(), "nothing was sent");
    assert!(r.world.fake.edited().is_empty() && r.world.fake.deleted().is_empty());
}

/// Test 5: reading is never refused, in the chat where the orders are given or anywhere; a
/// message there still wakes by the access; forwarding **from** it is reading the source.
#[tokio::test(flavor = "multi_thread")]
async fn reading_is_never_refused_and_the_chat_can_be_forwarded_from() {
    let mut r = ready(true).await;
    let by_name = chats(&mut r.g, "operador").await;
    let (bridge, ana) = (by_name["Bridge"].clone(), by_name["Ana"].clone());
    r.world
        .fake
        .add_contact(&id(BRIDGE), "Bridge", "bridge", "about");
    record_the_usual(&r.world, &r.h.name).await;
    // The access of H also wakes it, at once.
    r.world
        .fixture
        .admin
        .access_wake_wait(&r.h.name, "operador", 0, r.world.now())
        .await
        .unwrap();

    for agent in [&mut r.g, &mut r.h] {
        let listed = chats(agent, "operador").await;
        assert!(listed.contains_key("Bridge"));
        let read = messages(agent, "operador", &bridge).await;
        assert!(read.contains_key("an order"));
        ok(go(
            agent,
            "whatsapp_get_contact",
            "operador",
            json!({"chat": bridge}),
        )
        .await);
        ok(go(agent, "whatsapp_list_groups", "operador", json!({})).await);
        ok(go(agent, "whatsapp_status", "operador", json!({})).await);
    }
    // Forwarding a message of that chat to a customer reads the source.
    let in_bridge = messages(&mut r.g, "operador", &bridge).await;
    let forwarded = go(
        &mut r.g,
        "whatsapp_forward",
        "operador",
        json!({"message_id": in_bridge["an order"], "to": ana}),
    )
    .await;
    assert_eq!(refused(&forwarded), None);
    // A message that arrives there still makes a notice for the Hermes woken by the access.
    let session = r.operador.clone();
    let received = message(&id(BRIDGE), "3EB0OR000099", NOW_S + 50, false, "an order");
    assert_eq!(
        r.world
            .fake
            .emit(&session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    let mut waiting = 0_i64;
    for _ in 0..40 {
        waiting = sqlx::query_scalar(
            "SELECT count(*) FROM notice n JOIN hermes h ON h.id = n.hermes_id \
             WHERE n.kind = 'whatsapp' AND h.name = $1",
        )
        .bind(&r.h.name)
        .fetch_one(r.world.hub().pool())
        .await
        .unwrap();
        if waiting > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(
        waiting, 1,
        "the message of the protected chat wakes by the access"
    );
}

/// Test 6: the refusal is one chat, never the number. On the operator's number the customers,
/// new ones by telephone and the groups are written as before; so are the customers of the
/// bridge's number.
#[tokio::test(flavor = "multi_thread")]
async fn nothing_else_is_refused_when_a_chat_is() {
    let mut r = ready(true).await;
    record_the_usual(&r.world, &r.h.name).await;
    let operador = chats(&mut r.g, "operador").await;
    let ponte = chats(&mut r.g, "ponte").await;
    for (number, to) in [
        ("operador", operador["Ana"].clone()),
        ("operador", operador["Team"].clone()),
        ("operador", plus(NEW_CUSTOMER)),
        ("ponte", ponte["Bia"].clone()),
    ] {
        assert_eq!(send(&mut r.g, number, &to).await, None, "{number} {to}");
        assert_eq!(send(&mut r.h, number, &to).await, None, "{number} {to}");
    }
    assert_eq!(sent(&r.world), 8);
}

/// Without the verb, the ordinary answer: nobody learns from this rule that a chat is protected.
#[tokio::test(flavor = "multi_thread")]
async fn a_hermes_without_the_verb_keeps_its_ordinary_answer() {
    let r = ready(true).await;
    let mut reader = r.world.agent().await;
    r.world
        .grant(
            &reader.name,
            "operador",
            arauto_hub::core::access::Grant {
                scope: arauto_hub::core::access::Scope::All,
                verbs: READ,
                can_start_chats: false,
            },
        )
        .await;
    r.world.grant(&reader.name, "ponte", all(ALL_VERBS)).await;
    let by_name = chats(&mut reader, "operador").await;
    record_the_usual(&r.world, &r.h.name).await;
    let protected = send(&mut reader, "operador", &by_name["Bridge"]).await;
    let ordinary = send(&mut reader, "operador", &by_name["Ana"]).await;
    assert_eq!(ordinary, Some(ErrorCode::Ar701));
    assert_eq!(protected, ordinary, "the same answer for both");
    // By telephone, the same, with the telephone registered in the engine for both.
    r.world.fake.register(&phone(BRIDGE), &id(BRIDGE));
    let protected = send(&mut reader, "operador", &plus(BRIDGE)).await;
    let ordinary = send(&mut reader, "operador", &plus(NEW_CUSTOMER)).await;
    assert_eq!(ordinary, Some(ErrorCode::Ar701));
    assert_eq!(protected, ordinary);
    // A message of the chat: the ordinary answer is AR-701 too, for a verb the number lacks.
    let in_bridge = messages(&mut reader, "operador", &by_name["Bridge"]).await;
    let react = go(
        &mut reader,
        "whatsapp_react",
        "operador",
        json!({"message_id": in_bridge["an order"], "reaction": "x"}),
    )
    .await;
    assert_eq!(refused(&react), Some(ErrorCode::Ar701));
    // A list scope with no line: AR-705 for every chat, as before.
    let mut listed = r.world.agent().await;
    r.world
        .grant(
            &listed.name,
            "operador",
            list(arauto_hub::core::access::Verbs {
                read: false,
                send: true,
                modify: false,
                wake: false,
            }),
        )
        .await;
    assert_eq!(
        send(&mut listed, "operador", &by_name["Bridge"]).await,
        AR705
    );
    assert_eq!(send(&mut listed, "operador", &by_name["Ana"]).await, AR705);
}

/// Test 7: in `self-chat` mode what is refused is the chat of the account with itself.
#[tokio::test(flavor = "multi_thread")]
async fn in_self_chat_mode_the_chat_with_itself_is_refused_and_no_other() {
    let mut r = ready(true).await;
    // The chat of the bridge's account with itself, as the engine lists it.
    r.world.fake.add_chat(&r.ponte, &id(BRIDGE), "Myself");
    r.world.fake.add_message(
        &r.ponte,
        &id(BRIDGE),
        message(&id(BRIDGE), "3EB0PO000003", NOW_S + 5, true, "note to self"),
    );
    record(&r.world, &r.h.name, Some(BRIDGE), &[], true)
        .await
        .unwrap();
    let ponte = chats(&mut r.g, "ponte").await;
    let operador = chats(&mut r.g, "operador").await;
    assert_eq!(send(&mut r.g, "ponte", &ponte["Myself"]).await, AR705);
    assert_eq!(send(&mut r.h, "ponte", &ponte["Myself"]).await, AR705);
    assert_eq!(send(&mut r.g, "ponte", &plus(BRIDGE)).await, AR705);
    // A customer chat of the same number, and the chat with the bridge on the other number
    // (the operator's number has no record): both are usable.
    assert_eq!(send(&mut r.g, "ponte", &ponte["Bia"]).await, None);
    assert_eq!(send(&mut r.g, "operador", &operador["Bridge"]).await, None);
}

/// Test 8: a chat the engine names only by a hidden identifier. Writing there is refused on a
/// number with a recorded counterpart when the engine does not say whose it is (it fails
/// closed), and only there: reading, the lines, every other chat and every other number are as
/// before.
#[tokio::test(flavor = "multi_thread")]
async fn a_hidden_contact_chat_is_held_for_writing_alone_when_the_engine_does_not_say() {
    let mut r = ready(false).await;
    let hidden = ["123456789012345@lid", "223456789012345@lid"];
    for (n, lid) in hidden.iter().enumerate() {
        r.world
            .fake
            .add_chat(&r.operador, lid, &format!("Hidden{n}"));
        r.world.fake.add_message(
            &r.operador,
            lid,
            message(lid, &format!("3EB0HD00000{n}"), NOW_S + 10, false, "hello"),
        );
    }
    let by_name = chats(&mut r.g, "operador").await;
    let (hidden0, hidden1) = (by_name["Hidden0"].clone(), by_name["Hidden1"].clone());
    // No record: nothing is held, and the engine is not asked about hidden identifiers.
    assert_eq!(send(&mut r.g, "operador", &hidden0).await, None);
    assert_eq!(requests(&r.world, "/lids/"), 0);

    record_the_usual(&r.world, &r.h.name).await;
    // The engine does not know it: refused, for G and for H. Reading is untouched.
    assert_eq!(send(&mut r.g, "operador", &hidden0).await, AR705);
    assert_eq!(send(&mut r.h, "operador", &hidden0).await, AR705);
    assert!(requests(&r.world, "/lids/") > 0, "the engine was asked");
    for agent in [&mut r.g, &mut r.h] {
        for chat in [&hidden0, &hidden1] {
            let read = messages(agent, "operador", chat).await;
            assert!(read.contains_key("hello"));
        }
    }
    // No line was made: the other hidden chats of the access are read as before.
    let lines: i64 = sqlx::query_scalar("SELECT count(*) FROM access_chat")
        .fetch_one(r.world.hub().pool())
        .await
        .unwrap();
    assert_eq!(lines, 0, "the gate goes through no line");
    // A customer chat and a group stay writable; so does a chat of the other number.
    assert_eq!(send(&mut r.g, "operador", &by_name["Ana"]).await, None);

    // The engine says the hidden identifier is a customer: matched like any telephone, allowed.
    r.world.fake.add_lid(hidden[0], &id(CUSTOMER));
    assert_eq!(send(&mut r.g, "operador", &hidden0).await, None);
    // ... or says it is the bridge: refused.
    r.world.fake.add_lid(hidden[0], &id(BRIDGE));
    assert_eq!(send(&mut r.g, "operador", &hidden0).await, AR705);
    // The engine fails: refused, in that chat alone.
    r.world.fake.add_lid(hidden[0], &id(CUSTOMER));
    r.world.fake.fail("/lids/", Fault::ServerError);
    assert_eq!(send(&mut r.g, "operador", &hidden0).await, AR705);
    assert_eq!(send(&mut r.g, "operador", &by_name["Ana"]).await, None);
    r.world.fake.heal();
    assert_eq!(send(&mut r.g, "operador", &hidden0).await, None);

    // A number with no recorded counterpart makes no such call and holds nothing.
    let livre = r
        .world
        .connected("livre", &phone(9))
        .await
        .session
        .as_str()
        .to_owned();
    r.world.fake.add_chat(&livre, hidden[0], "Hidden0");
    r.world.fake.add_message(
        &livre,
        hidden[0],
        message(hidden[0], "3EB0HD000020", NOW_S + 12, false, "hello"),
    );
    r.world.grant(&r.g.name, "livre", all(ALL_VERBS)).await;
    let free = chats(&mut r.g, "livre").await;
    let before = requests(&r.world, "/lids/");
    assert_eq!(send(&mut r.g, "livre", &free["Hidden0"]).await, None);
    assert_eq!(requests(&r.world, "/lids/"), before);
}

/// The refusal for a hidden chat is audited with `why = hidden`; one that matched, `matched`.
#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_is_audited_with_its_reason_and_without_any_telephone() {
    let mut r = ready(true).await;
    r.world
        .fake
        .add_chat(&r.operador, "123456789012345@lid", "Hidden0");
    r.world.fake.add_message(
        &r.operador,
        "123456789012345@lid",
        message(
            "123456789012345@lid",
            "3EB0HD000009",
            NOW_S + 11,
            false,
            "hello",
        ),
    );
    let by_name = chats(&mut r.g, "operador").await;
    record_the_usual(&r.world, &r.h.name).await;
    // Many refusals of one Hermes in one chat make one row; another Hermes or another chat,
    // another one.
    for _ in 0..5 {
        assert_eq!(send(&mut r.g, "operador", &by_name["Bridge"]).await, AR705);
    }
    assert_eq!(send(&mut r.h, "operador", &by_name["Bridge"]).await, AR705);
    assert_eq!(send(&mut r.g, "operador", &by_name["Hidden0"]).await, AR705);
    assert_eq!(send(&mut r.g, "operador", &plus(BRIDGE)).await, AR705);
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT actor_kind, details::text FROM audit \
         WHERE action = 'whatsapp.order_chat_refused' ORDER BY seq",
    )
    .fetch_all(r.world.hub().pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 4, "{rows:?}");
    let whys: Vec<&str> = rows
        .iter()
        .map(|(_, details)| {
            if details.contains("\"hidden\"") {
                "hidden"
            } else {
                "matched"
            }
        })
        .collect();
    assert_eq!(whys, ["matched", "matched", "hidden", "matched"]);
    for (actor, details) in &rows {
        assert_eq!(actor, "system");
        for needle in [
            phone(BRIDGE),
            phone(OPERATOR),
            without_ninth(BRIDGE),
            without_ninth(OPERATOR),
        ] {
            assert!(!details.contains(&needle), "{details}");
        }
        assert!(details.contains("\"verb\"") && details.contains("\"why\""));
        assert!(details.contains("\"alias\""));
    }
    // The security events are the record's, not the refusals'.
    let security: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit WHERE action = 'whatsapp.order_chat_refused' AND security",
    )
    .fetch_one(r.world.hub().pool())
    .await
    .unwrap();
    assert_eq!(security, 0);
}

/// Test 9: a chat the hub knows by two identifiers is protected when any of them is.
#[tokio::test(flavor = "multi_thread")]
async fn a_chat_with_a_second_identifier_that_is_protected_is_refused() {
    let mut r = ready(false).await;
    let by_name = chats(&mut r.g, "operador").await;
    let ana = by_name["Ana"].clone();
    record_the_usual(&r.world, &r.h.name).await;
    assert_eq!(
        send(&mut r.g, "operador", &ana).await,
        None,
        "one identifier: a customer"
    );

    let pool = r.world.hub().pool();
    let organization = r.world.organization();
    let key = arauto_hub::repo::device::organization_key(pool, organization, &r.world.vault)
        .await
        .unwrap();
    let number = r.world.number("operador").await;
    let chat: uuid::Uuid = ana.parse().unwrap();
    let second = id(BRIDGE);
    let digest = key.whatsapp_digest(WhatsappField::ChatEngineId, number.id, second.as_bytes());
    let sealed = key
        .seal_whatsapp(WhatsappField::ChatEngineId, chat, second.as_bytes())
        .unwrap();
    sqlx::query(
        "INSERT INTO chat_engine_id (organization_id, number_id, chat_id, id_digest, id_enc, \
                                     created_at) VALUES ($1, $2, $3, $4, $5, 1)",
    )
    .bind(organization.id())
    .bind(number.id)
    .bind(chat)
    .bind(digest.as_slice())
    .bind(sealed)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(send(&mut r.g, "operador", &ana).await, AR705);
    assert_eq!(send(&mut r.h, "operador", &ana).await, AR705);
}

/// Test 11: a removed Hermes loses its record in the same write, audited; the refusal ends.
#[tokio::test(flavor = "multi_thread")]
async fn a_removed_hermes_takes_its_record_with_it() {
    let mut r = ready(true).await;
    let by_name = chats(&mut r.g, "operador").await;
    let bridge = by_name["Bridge"].clone();
    record_the_usual(&r.world, &r.h.name).await;
    assert_eq!(send(&mut r.g, "operador", &bridge).await, AR705);
    let rows = |table: &'static str| {
        let pool = r.world.hub().pool().clone();
        async move {
            let sql = format!("SELECT count(*) FROM {table}");
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql))
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    assert_eq!(rows("hermes_whatsapp_phone").await, 2);
    assert_eq!(rows("hermes_whatsapp_phone_digest").await, 4);
    r.world
        .fixture
        .admin
        .hermes_remove(&r.h.name, r.world.now())
        .await
        .unwrap();
    assert_eq!(rows("hermes_whatsapp_phone").await, 0);
    assert_eq!(rows("hermes_whatsapp_phone_digest").await, 0);
    let removal: Vec<String> = sqlx::query_scalar(
        "SELECT details::text FROM audit WHERE action = 'hermes.whatsapp_orders_changed' \
         ORDER BY seq DESC LIMIT 1",
    )
    .fetch_all(r.world.hub().pool())
    .await
    .unwrap();
    assert!(removal[0].contains("hermes_removed"), "{removal:?}");
    assert_eq!(send(&mut r.g, "operador", &bridge).await, None);
}

/// Test 12: the telephone of a number is read at each call: a number swapped and then confirmed
/// onto the telephone where a Hermes takes orders refuses the chat with the operator.
#[tokio::test(flavor = "multi_thread")]
async fn a_number_swapped_onto_the_bridge_telephone_refuses_the_chat_with_the_operator() {
    let world = World::new().await;
    // Added with one telephone, and another one pairs: the number waits for the admin.
    let number = world.add("troca", &phone(7)).await;
    let session = number.session.as_str().to_owned();
    assert_eq!(
        world.pair("troca", &phone(BRIDGE)).await,
        arauto_hub::admin::whatsapp::PairOutcome::Swapped
    );
    world.fake.add_chat(&session, &id(OPERATOR), "Operator");
    world.fake.add_message(
        &session,
        &id(OPERATOR),
        message(&id(OPERATOR), "3EB0TR000001", NOW_S, false, "an order"),
    );
    world.fake.add_chat(&session, &id(CUSTOMER_B), "Bia");
    world.fake.add_message(
        &session,
        &id(CUSTOMER_B),
        message(&id(CUSTOMER_B), "3EB0TR000002", NOW_S + 1, false, "hi"),
    );
    let mut g = world.agent().await;
    let h = world.agent().await;
    world.grant(&g.name, "troca", grant()).await;
    record(&world, &h.name, Some(BRIDGE), &[OPERATOR], false)
        .await
        .unwrap();
    // Until the admin decides, the accesses of the number wait (tests/whatsapp/tools.rs).
    world
        .fixture
        .admin
        .number_confirm("troca", world.now())
        .await
        .unwrap();
    // The number now has the bridge's telephone: read at this call, its chat with the operator
    // is refused, and its customers are not.
    let by_name = chats(&mut g, "troca").await;
    assert_eq!(send(&mut g, "troca", &by_name["Operator"]).await, AR705);
    assert_eq!(send(&mut g, "troca", &by_name["Bia"]).await, None);
}

/// Test 13: the record in the repository.
#[tokio::test(flavor = "multi_thread")]
async fn the_record_is_audited_with_counts_and_its_rules_hold() {
    let r = ready(true).await;
    let other = r.world.agent().await;
    // A list without the telephone of the bridge; more than 16 telephones.
    let edit = OrdersEdit {
        orders_from: Some(vec![plus(OPERATOR)]),
        ..OrdersEdit::default()
    };
    let admin = &r.world.fixture.admin;
    let refused_list = admin
        .hermes_orders_set(&r.world.vault, &r.h.name, &edit, r.world.now())
        .await;
    assert!(
        matches!(
            refused_list,
            Err(AdminError::OrdersRule(Invalid::NoTelephoneOfTheHermes))
        ),
        "{refused_list:?}"
    );
    let many = OrdersEdit {
        takes_orders_on: Some(Some(plus(BRIDGE))),
        orders_from: Some((100..117).map(plus).collect()),
        ..OrdersEdit::default()
    };
    assert!(matches!(
        admin
            .hermes_orders_set(&r.world.vault, &r.h.name, &many, r.world.now())
            .await,
        Err(AdminError::OrdersRule(Invalid::TooMany))
    ));
    // Self-chat with a list.
    let both = OrdersEdit {
        takes_orders_on: Some(Some(plus(BRIDGE))),
        self_chat: true,
        orders_from: Some(vec![plus(OPERATOR)]),
    };
    assert!(matches!(
        admin
            .hermes_orders_set(&r.world.vault, &r.h.name, &both, r.world.now())
            .await,
        Err(AdminError::OrdersRule(Invalid::SelfChatWithAList))
    ));
    // Not a telephone.
    let bad = OrdersEdit {
        takes_orders_on: Some(Some("not a telephone".to_owned())),
        ..OrdersEdit::default()
    };
    assert!(matches!(
        admin
            .hermes_orders_set(&r.world.vault, &r.h.name, &bad, r.world.now())
            .await,
        Err(AdminError::InvalidPhone)
    ));
    // Nothing of the refusals was written.
    let none: i64 = sqlx::query_scalar("SELECT count(*) FROM hermes_whatsapp_phone")
        .fetch_one(r.world.hub().pool())
        .await
        .unwrap();
    assert_eq!(none, 0);

    // The record is written, and audited with counts: never a telephone.
    record_the_usual(&r.world, &r.h.name).await;
    let details: Vec<String> = sqlx::query_scalar(
        "SELECT details::text FROM audit WHERE action = 'hermes.whatsapp_orders_changed'",
    )
    .fetch_all(r.world.hub().pool())
    .await
    .unwrap();
    assert_eq!(details.len(), 1);
    assert!(details[0].contains("telephones_after") && details[0].contains("mode_after"));
    // The sealed telephones come back to the admin command, in no table in the clear.
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' \
           AND table_name <> '_sqlx_migrations'",
    )
    .fetch_all(r.world.hub().pool())
    .await
    .unwrap();
    assert!(tables.iter().any(|t| t == "hermes_whatsapp_phone"));
    for table in tables {
        let sql = format!("SELECT row_to_json(t)::text FROM {table} t");
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_all(r.world.hub().pool())
            .await
            .unwrap();
        let text = rows.join("\n");
        for needle in [
            phone(BRIDGE),
            phone(OPERATOR),
            without_ninth(BRIDGE),
            without_ninth(OPERATOR),
        ] {
            assert!(
                !text.contains(&needle),
                "{table} holds a telephone in the clear"
            );
        }
    }
    // The same record again changes nothing and writes nothing.
    let again = record(&r.world, &r.h.name, Some(BRIDGE), &[OPERATOR], false)
        .await
        .unwrap();
    assert!(!again.written);
    // The telephone of one bridge is one Hermes: the other is refused, naming the first.
    let taken = record(&r.world, &other.name, Some(BRIDGE), &[], false).await;
    match taken {
        Err(AdminError::OrdersTelephoneTaken(name)) => assert_eq!(name, r.h.name),
        other => panic!("{other:?}"),
    }
    // ... in the other form of the same telephone too.
    let edit = OrdersEdit {
        takes_orders_on: Some(Some(without_ninth(BRIDGE))),
        ..OrdersEdit::default()
    };
    assert!(matches!(
        admin
            .hermes_orders_set(&r.world.vault, &other.name, &edit, r.world.now())
            .await,
        Err(AdminError::OrdersTelephoneTaken(_))
    ));
    // A restored database gets nothing.
    r.world.fixture.restore_an_older_copy().await;
    let restored = record(&r.world, &r.h.name, None, &[], false).await;
    assert!(
        matches!(restored, Err(AdminError::Restored(_))),
        "{restored:?}"
    );
}

/// Test 14: a record changes no list of tools and no answer of `arauto_status`: the same verbs
/// serve every other chat.
#[tokio::test(flavor = "multi_thread")]
async fn a_record_changes_neither_the_list_of_tools_nor_the_status() {
    let mut r = ready(true).await;
    // The lists that the grants above brought have arrived before the record is written.
    let _ = r.g.quiet_for(std::time::Duration::from_millis(900)).await;
    let before_tools = r.g.started_with();
    let before_status = ok(r.g.call("arauto_status", json!({})).await);
    let before_h = ok(r.h.call("arauto_status", json!({})).await);
    record_the_usual(&r.world, &r.h.name).await;
    assert_eq!(
        ok(r.g.call("arauto_status", json!({})).await),
        before_status
    );
    assert_eq!(ok(r.h.call("arauto_status", json!({})).await), before_h);
    // No new list reaches the session in use, and a session that starts after the record
    // starts with the same one.
    assert!(
        r.g.quiet_for(std::time::Duration::from_millis(600)).await,
        "no new list"
    );
    let later = r.world.agent().await;
    r.world.grant(&later.name, "operador", grant()).await;
    r.world.grant(&later.name, "ponte", grant()).await;
    let mut later = later;
    assert_eq!(
        ok(later.call("arauto_status", json!({})).await)["numbers"],
        before_status["numbers"]
    );
    assert_eq!(before_tools, r.g.started_with());
}

// ------------------------------------------------------------------------------------ the command

/// Test 17: the command prints the risk while nothing is protected, and on a change that
/// protects fewer chats; `number list` ends with a line while no Hermes protects a chat.
#[tokio::test(flavor = "multi_thread")]
async fn the_command_says_the_risk_and_number_list_has_its_closing_line() {
    use arauto_hub::admin::render::{orders as render_orders, whatsapp as render_whatsapp};
    let r = ready(true).await;
    let admin = &r.world.fixture.admin;
    let list_text = || async {
        let mut listed = Vec::new();
        for number in admin.number_list(false).await.unwrap() {
            let phone = admin.number_phone(&r.world.vault, &number).await.ok();
            listed.push((number, phone));
        }
        let orders = admin.orders_for_the_list().await.unwrap();
        render_whatsapp::number_list(&listed, &orders).text
    };
    let closing = "No Hermes has recorded who gives it orders on WhatsApp";
    let before = list_text().await;
    assert!(before.contains(closing), "{before}");
    assert!(!before.contains("takes orders here"));

    // Shown with nothing recorded: the risk line.
    let shown = admin
        .hermes_orders_show(&r.world.vault, &r.h.name)
        .await
        .unwrap();
    let text = render_orders::orders(&shown, false).text;
    assert!(text.contains("Warning: nothing is recorded"), "{text}");
    // The bridge alone protects nothing: the same line, when changing.
    let edit = OrdersEdit {
        takes_orders_on: Some(Some(plus(BRIDGE))),
        ..OrdersEdit::default()
    };
    let changed = admin
        .hermes_orders_set(&r.world.vault, &r.h.name, &edit, r.world.now())
        .await
        .unwrap();
    let text = render_orders::orders(&changed, true).text;
    assert!(text.contains("Warning: nothing is recorded"), "{text}");
    assert!(
        list_text().await.contains(closing),
        "a bridge alone protects nothing"
    );

    // One telephone that gives orders: protected, no line, and the closing line is gone.
    let one = OrdersEdit {
        orders_from: Some(vec![plus(OPERATOR)]),
        ..OrdersEdit::default()
    };
    let changed = admin
        .hermes_orders_set(&r.world.vault, &r.h.name, &one, r.world.now())
        .await
        .unwrap();
    let text = render_orders::orders(&changed, true).text;
    assert!(!text.contains("Warning"), "{text}");
    assert!(
        text.contains("No Hermes can write in that chat through Arauto"),
        "{text}"
    );
    let after = list_text().await;
    assert!(!after.contains(closing), "{after}");
    assert!(after.contains("(gives orders to"), "{after}");
    assert!(after.contains("takes orders here)"), "{after}");

    // A change that leaves it protected and adds another: no line.
    let two = OrdersEdit {
        orders_from: Some(vec![plus(OPERATOR), plus(CUSTOMER)]),
        ..OrdersEdit::default()
    };
    let changed = admin
        .hermes_orders_set(&r.world.vault, &r.h.name, &two, r.world.now())
        .await
        .unwrap();
    let text = render_orders::orders(&changed, true).text;
    assert!(!text.contains("Warning"), "{text}");
    // A change that protects fewer chats: the line, though some stay protected.
    let fewer = admin
        .hermes_orders_set(&r.world.vault, &r.h.name, &one, r.world.now())
        .await
        .unwrap();
    let text = render_orders::orders(&fewer, true).text;
    assert!(
        text.contains("can now pass for the person who gives it orders"),
        "{text}"
    );
    assert!(!text.contains("nothing is recorded"), "{text}");
    // self-chat in place of the list: the pair with the operator goes, so the line shows.
    let own = OrdersEdit {
        self_chat: true,
        ..OrdersEdit::default()
    };
    let changed = admin
        .hermes_orders_set(&r.world.vault, &r.h.name, &own, r.world.now())
        .await
        .unwrap();
    let text = render_orders::orders(&changed, true).text;
    assert!(text.contains("itself, self-chat"), "{text}");
    assert!(
        text.contains("can now pass for the person who gives it orders"),
        "{text}"
    );
    // Cleared: both lines.
    let clear = OrdersEdit {
        takes_orders_on: Some(None),
        orders_from: Some(Vec::new()),
        ..OrdersEdit::default()
    };
    let changed = admin
        .hermes_orders_set(&r.world.vault, &r.h.name, &clear, r.world.now())
        .await
        .unwrap();
    let text = render_orders::orders(&changed, true).text;
    assert!(text.contains("Warning: nothing is recorded"), "{text}");
    assert!(text.contains("can now pass for the person"), "{text}");
    assert!(list_text().await.contains(closing));
    // The telephone is never printed.
    assert!(!text.contains(&phone(BRIDGE)));
}
