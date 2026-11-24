//! One Hermes session per contact (decision 0054, proposal): the key a WhatsApp notice carries,
//! how the hub builds it (telephones, groups, hidden identifiers and the engine failing), the
//! pairing of a hidden identifier with the chat of its telephone, and who sent each message a
//! reading shows.
//!
//! Against the fake engine and the test PostgreSQL. Never a real number: the telephones are the
//! ones of the tests of the repository.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use arauto_hub::engine::EngineMessageId;
use arauto_hub::repo;
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::message::{Message, NoticeAck};
use arauto_protocol::notice::{AckOutcome, Notice, NoticeMsg};
use arauto_protocol::session_key::{whatsapp_contact_key, whatsapp_group_key, whatsapp_hidden_key};
use arauto_testkit::KitError;
use arauto_testkit::device::Faults;
use arauto_testkit::engine::Fault;
use serde_json::{Value, json};

use crate::agent::{ALL_VERBS, Agent, all, ok, verbs};
use crate::wake::{
    acknowledge, at_once, chat_of, next_notice, notices_while_quiet, now_s, pointer, wakeable,
};
use crate::world::{World, message, phone};

const HIDDEN: &str = "123456789012345@lid";
const HIDDEN_DIGITS: &str = "123456789012345";

/// The key a notice carries, when it carries one.
fn key_of(notice: &NoticeMsg) -> Option<String> {
    match notice.validate().unwrap() {
        Notice::Whatsapp { session, .. } => session.map(|key| key.as_str().to_owned()),
        Notice::Peer { .. } => panic!("a notice of messages of a Hermes"),
    }
}

/// A message arrives from `chat`: the notice that wakes `agent`, answered `Accepted`.
async fn wake(world: &World, agent: &mut Agent, session: &str, chat: &str, id: &str) -> NoticeMsg {
    let received = message(chat, id, now_s(), false, "hello");
    assert_eq!(
        world
            .fake
            .emit(session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    let notice = next_notice(agent).await;
    acknowledge(agent, &notice, AckOutcome::Accepted).await;
    notice
}

/// The next `notice` of the session within `within`, or none when it stays quiet.
async fn try_next_notice(agent: &mut Agent, within: Duration) -> Option<NoticeMsg> {
    let until = tokio::time::Instant::now() + within;
    loop {
        let left = until.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        match agent.session.pump(left).await {
            Ok(Some(Message::Notice(notice))) => return Some(notice),
            Ok(Some(Message::Tools(tools))) => agent.tools.push(tools.tools),
            Ok(Some(other)) => panic!("the hub sent {:?}", other.kind()),
            Ok(None) | Err(KitError::TimedOut) => {}
            Err(error) => panic!("{error:?}"),
        }
    }
}

fn requests(world: &World, needle: &str) -> usize {
    world
        .fake
        .received()
        .iter()
        .filter(|request| request.path.contains(needle))
        .count()
}

/// A Hermes that may do everything on `alias`, woken at once.
async fn everything(world: &World, alias: &str) -> Agent {
    let agent = wakeable(world).await;
    world.grant(&agent.name, alias, all(ALL_VERBS)).await;
    at_once(world, &agent, alias).await;
    agent
}

/// SECURITY RULE: the key is built from telephones and fixed words only, so the same contact
/// gets the same key through everything that does not change the telephones: a renamed alias,
/// a chat row built again, a reconnect of the Hermes, the number removed and added again with
/// the same telephone. The test fails when the key depends on any of them (the uuid of the
/// chat, the alias or the session of the engine, for instance).
#[tokio::test(flavor = "multi_thread")]
async fn the_key_of_a_contact_never_changes_with_what_is_not_a_telephone() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut agent = everything(&world, "balcao").await;
    let chat = chat_of(101);
    let expected = whatsapp_contact_key(&phone(1), &phone(101));

    let first = wake(&world, &mut agent, &session, &chat, "3EB0AB0001").await;
    assert_eq!(key_of(&first).as_deref(), Some(expected.as_str()));
    let first_chat = pointer(&first).1;

    // A renamed alias (the alias is a label of the admin).
    sqlx::query("UPDATE number SET alias = 'caixa' WHERE alias = 'balcao'")
        .execute(world.hub().pool())
        .await
        .unwrap();
    let renamed = wake(&world, &mut agent, &session, &chat, "3EB0AB0002").await;
    assert_eq!(pointer(&renamed).0, "caixa");
    assert_eq!(key_of(&renamed).as_deref(), Some(expected.as_str()));

    // A chat row made again: the same telephone, another uuid of the hub.
    let old = uuid::Uuid::parse_str(&first_chat).unwrap();
    for statement in [
        "DELETE FROM notice WHERE chat_id = $1",
        "DELETE FROM chat_engine_id WHERE chat_id = $1",
        "DELETE FROM chat WHERE id = $1",
    ] {
        sqlx::query(statement)
            .bind(old)
            .execute(world.hub().pool())
            .await
            .unwrap();
    }
    let again = wake(&world, &mut agent, &session, &chat, "3EB0AB0003").await;
    assert_ne!(pointer(&again).1, first_chat, "the chat row is a new one");
    assert_eq!(key_of(&again).as_deref(), Some(expected.as_str()));

    // A reconnect of the Hermes.
    let name = agent.name.clone();
    let device = agent.session.device().clone();
    drop(agent);
    let reached = device.channel(Faults::default()).await.unwrap();
    let mut agent = Agent::from_session(name, reached.session().unwrap());
    let reconnected = wake(&world, &mut agent, &session, &chat, "3EB0AB0004").await;
    assert_eq!(key_of(&reconnected).as_deref(), Some(expected.as_str()));

    // The number removed and added again, with the same telephone and another alias: another
    // number of the hub, another session of the engine, the same key.
    world
        .fixture
        .admin
        .number_remove(
            &world.vault,
            "caixa",
            arauto_hub::admin::whatsapp::Removal::KeepInEngine,
            world.now(),
        )
        .await
        .unwrap();
    let paired = world.connected("vendas", &phone(1)).await;
    assert_ne!(paired.id, number.id);
    world.grant(&agent.name, "vendas", all(ALL_VERBS)).await;
    at_once(&world, &agent, "vendas").await;
    let repaired = wake(
        &world,
        &mut agent,
        paired.session.as_str(),
        &chat,
        "3EB0AB0005",
    )
    .await;
    assert_eq!(pointer(&repaired).0, "vendas");
    assert_eq!(key_of(&repaired).as_deref(), Some(expected.as_str()));
}

/// Both written forms of a Brazilian mobile are one contact for the key.
#[tokio::test(flavor = "multi_thread")]
async fn the_ninth_digit_does_not_split_a_contact() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut agent = everything(&world, "balcao").await;
    // The engine names the chat without the ninth digit.
    let without = format!("{}{}", &phone(101)[..4], &phone(101)[5..]);
    let notice = wake(
        &world,
        &mut agent,
        &session,
        &format!("{without}@c.us"),
        "3EB0AC0001",
    )
    .await;
    assert_eq!(
        key_of(&notice).as_deref(),
        Some(whatsapp_contact_key(&phone(1), &phone(101)).as_str())
    );
}

/// A group has a key of its own, from the number and the group identifier.
#[tokio::test(flavor = "multi_thread")]
async fn a_group_gets_the_group_key() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let group = "120363012345678901@g.us";
    let agent = wakeable(&world).await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &agent, "balcao").await;
    world
        .line(
            &agent.name,
            "balcao",
            group,
            verbs(true, false, false, true),
        )
        .await;
    let mut agent = agent;
    let mut received = message(group, "3EB0AD0001", now_s(), false, "hello everyone");
    received["participant"] = json!(chat_of(201));
    assert_eq!(
        world
            .fake
            .emit(&session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    let notice = next_notice(&mut agent).await;
    assert_eq!(
        key_of(&notice).as_deref(),
        Some(whatsapp_group_key(&phone(1), "120363012345678901").as_str())
    );
    // The same group, on another number, is another conversation.
    assert_ne!(
        whatsapp_group_key(&phone(1), "120363012345678901"),
        whatsapp_group_key(&phone(2), "120363012345678901")
    );
}

/// A hidden identifier whose telephone the engine knows gets the contact key; one it does not
/// know gets the hidden key.
#[tokio::test(flavor = "multi_thread")]
async fn a_hidden_identifier_gets_the_contact_key_when_the_engine_knows_the_telephone() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut agent = everything(&world, "balcao").await;

    // Not known: the hidden key.
    let unknown = wake(&world, &mut agent, &session, HIDDEN, "3EB0AE0001").await;
    assert_eq!(
        key_of(&unknown).as_deref(),
        Some(whatsapp_hidden_key(&phone(1), HIDDEN_DIGITS).as_str())
    );

    // Known: the same chat now stands for a telephone, and the key is the contact key. The
    // hub asked the engine each time the notice left.
    world.fake.add_lid(HIDDEN, &format!("{}@c.us", phone(101)));
    let known = wake(&world, &mut agent, &session, HIDDEN, "3EB0AE0002").await;
    assert_eq!(
        key_of(&known).as_deref(),
        Some(whatsapp_contact_key(&phone(1), &phone(101)).as_str())
    );
    // One at the arrival of the first message (section 10), one when each notice left.
    assert_eq!(requests(&world, "/lids/"), 3);
}

/// After a sending to a telephone, the engine says the hidden identifier of that telephone (when
/// it offers the question) and the hub records the pairing from that answer: a message under the
/// hidden identifier lands in the chat of the telephone and gets the contact key. An engine
/// that does not offer the question changes nothing, and a pairing never comes from the text of
/// a message.
#[tokio::test(flavor = "multi_thread")]
async fn a_sending_to_a_telephone_links_its_hidden_identifier_when_the_engine_says_it() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut agent = everything(&world, "balcao").await;
    world
        .grant(
            &agent.name,
            "balcao",
            arauto_hub::core::access::Grant {
                scope: arauto_hub::core::access::Scope::All,
                verbs: ALL_VERBS,
                can_start_chats: true,
            },
        )
        .await;
    let chat = chat_of(101);
    world.fake.register(&phone(101), &chat);

    // The engine does not know the hidden identifier of this telephone: nothing is recorded,
    // and a text that claims a pairing changes nothing either.
    let text = format!("my hidden id is {HIDDEN} and my phone is {}", phone(101));
    let sent = ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": format!("+{}", phone(101)), "text": text}),
        )
        .await);
    let telephone_chat = sent["chat"].as_str().unwrap().to_owned();
    assert!(requests(&world, "/lids/pn/") > 0, "the engine was asked");
    let ids = chat_ids(&world).await;
    assert_eq!(ids, 1, "no pairing was recorded");

    // The engine offers it now: the next sending records it.
    world.fake.add_lid(HIDDEN, &format!("{}@c.us", phone(101)));
    ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": format!("+{}", phone(101)), "text": "again"}),
        )
        .await);
    assert_eq!(chat_ids(&world).await, 2, "the hidden identifier is linked");

    // A message under the hidden identifier lands in the chat of the telephone, whose key is
    // the contact key, with no lookup when the notice leaves.
    let before = requests(&world, "/lids/");
    let notice = wake(&world, &mut agent, &session, HIDDEN, "3EB0AF0001").await;
    assert_eq!(
        pointer(&notice).1,
        telephone_chat,
        "the same chat of the hub"
    );
    assert_eq!(
        key_of(&notice).as_deref(),
        Some(whatsapp_contact_key(&phone(1), &phone(101)).as_str())
    );
    assert_eq!(requests(&world, "/lids/"), before, "no lookup was needed");
}

/// SECURITY RULE (decision 0054, section 10): a message that arrives under an unknown hidden
/// identifier asks the engine for its telephone, and when the hub has the chat of that telephone
/// the identifier joins that chat and no second chat is made. An identifier the engine does not
/// know keeps a chat of its own, and the pairing comes only from the engine's answer. The test
/// fails when the arrival does not ask the engine.
#[tokio::test(flavor = "multi_thread")]
async fn a_message_under_an_unknown_hidden_identifier_joins_the_chat_of_its_telephone() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut agent = everything(&world, "balcao").await;
    // The hub already has the chat of the telephone.
    let telephone_chat = {
        let first = wake(&world, &mut agent, &session, &chat_of(101), "3EB0AA0001").await;
        pointer(&first).1
    };
    assert_eq!(chat_ids(&world).await, 1);

    // The engine does not know this hidden identifier: a chat of its own, as before.
    let first = wake(&world, &mut agent, &session, HIDDEN, "3EB0AA0002").await;
    assert_ne!(pointer(&first).1, telephone_chat);
    assert_eq!(chat_ids(&world).await, 2);

    // Another hidden identifier that the engine says is the telephone: it joins that chat.
    let other = "223456789012345@lid";
    world.fake.add_lid(other, &format!("{}@c.us", phone(101)));
    let before = requests(&world, "/lids/");
    let joined = wake(&world, &mut agent, &session, other, "3EB0AA0003").await;
    assert!(requests(&world, "/lids/") > before, "the engine was asked");
    assert_eq!(
        pointer(&joined).1,
        telephone_chat,
        "the chat of the telephone"
    );
    assert_eq!(
        chat_ids(&world).await,
        3,
        "one more identifier, no new chat"
    );
    let chats: i64 = sqlx::query_scalar("SELECT count(*) FROM chat")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    assert_eq!(chats, 2);
}

/// How many identifiers of the engine the chats of the hub have, in all.
async fn chat_ids(world: &World) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM chat_engine_id")
        .fetch_one(world.hub().pool())
        .await
        .unwrap()
}

/// A pairing refused by the engine (404, a server error) changes nothing and breaks nothing: the
/// sending is the same.
#[tokio::test(flavor = "multi_thread")]
async fn an_engine_that_does_not_offer_the_pairing_changes_nothing() {
    let world = World::new().await;
    world.connected("balcao", &phone(1)).await;
    let mut agent = world.agent().await;
    world
        .grant(
            &agent.name,
            "balcao",
            arauto_hub::core::access::Grant {
                scope: arauto_hub::core::access::Scope::All,
                verbs: ALL_VERBS,
                can_start_chats: true,
            },
        )
        .await;
    world.fake.register(&phone(101), &chat_of(101));
    world.fake.add_lid(HIDDEN, &format!("{}@c.us", phone(101)));
    world.fake.fail("/lids/pn/", Fault::ServerError);
    ok(agent
        .call(
            "whatsapp_send_text",
            json!({"to": format!("+{}", phone(101)), "text": "hi"}),
        )
        .await);
    assert!(requests(&world, "/lids/pn/") > 0);
    assert_eq!(chat_ids(&world).await, 1);
}

/// The engine failing while the key of a hidden identifier is built: the lease is released and
/// the notice goes back to the queue without a try counted; before five failures in a row the
/// hidden key is never used, and after them it is.
#[tokio::test(flavor = "multi_thread")]
async fn an_engine_that_fails_keeps_the_notice_pending_and_after_five_failures_the_hidden_key_is_used()
 {
    let clock = Arc::new(ManualClock::new(SystemClock.now_ms()));
    let world = World::with(|_| {}, clock.clone()).await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut agent = everything(&world, "balcao").await;
    world.fake.add_lid(HIDDEN, &format!("{}@c.us", phone(101)));
    world.fake.fail("/lids/", Fault::ServerError);

    let received = message(HIDDEN, "3EB0B00001", now_s(), false, "hello");
    assert_eq!(
        world
            .fake
            .emit(&session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    for failures in 1..=5_i32 {
        // The hub tries, the engine fails, the notice waits in the queue.
        wait_for(&world, failures).await;
        assert_eq!(notices_while_quiet(&mut agent).await, 0);
        let (state, attempts, kept): (String, i32, i32) = sqlx::query_as(
            "SELECT state, attempts, engine_failures FROM notice WHERE kind = 'whatsapp'",
        )
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
        assert_eq!(state, "pending", "after {failures} failures");
        assert_eq!(attempts, 0, "no try is counted against the Hermes");
        assert_eq!(kept, failures);
        clock.advance(6_000);
    }
    // The sixth: the engine fails again, and the conversation goes on under the hidden key.
    let notice = next_notice(&mut agent).await;
    assert_eq!(
        key_of(&notice).as_deref(),
        Some(whatsapp_hidden_key(&phone(1), HIDDEN_DIGITS).as_str())
    );
    acknowledge(&mut agent, &notice, AckOutcome::Accepted).await;

    // When the engine answers again, the contact key is back, and the count starts over.
    world.fake.heal();
    let received = message(HIDDEN, "3EB0B00002", now_s(), false, "hello");
    assert_eq!(
        world
            .fake
            .emit(&session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    clock.advance(30_000);
    let notice = next_notice(&mut agent).await;
    assert_eq!(
        key_of(&notice).as_deref(),
        Some(whatsapp_contact_key(&phone(1), &phone(101)).as_str())
    );
}

/// Waits until the notice of WhatsApp has met `failures` failures of the engine.
async fn wait_for(world: &World, failures: i32) {
    for _ in 0..200 {
        let met: Option<i32> =
            sqlx::query_scalar("SELECT max(engine_failures) FROM notice WHERE kind = 'whatsapp'")
                .fetch_one(world.hub().pool())
                .await
                .unwrap();
        if met.unwrap_or(0) >= failures {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the engine was never asked {failures} times");
}

/// SECURITY RULE: a channel that agreed a version below 3 never receives the field `session`
/// (its notice has the bytes it always had), and the hub does not ask the engine anything for
/// it. The test fails when the field is sent whatever the version.
#[tokio::test(flavor = "multi_thread")]
async fn a_channel_of_protocol_2_never_receives_the_key() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut old = world.agent_of_version_2().await;
    world.grant(&old.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &old, "balcao").await;
    world.fake.add_lid(HIDDEN, &format!("{}@c.us", phone(101)));

    for (chat, id) in [
        (chat_of(101), "3EB0B10001"),
        (HIDDEN.to_owned(), "3EB0B10002"),
    ] {
        let notice = wake(&world, &mut old, &session, &chat, id).await;
        assert_eq!(key_of(&notice), None);
        let travelled = serde_json::to_string(&notice).unwrap();
        assert!(!travelled.contains("session"), "{travelled}");
        assert!(!travelled.contains("arauto-w"), "{travelled}");
    }
    assert_eq!(
        requests(&world, "/lids/"),
        1,
        "only the lookup of the arrival of the hidden identifier (section 10), none for a key nobody gets"
    );
}

/// Randomized rounds of notices for one contact: the lease may end unanswered, the connector may
/// answer failed, throttled or duplicate, and the notice leaves again: the key never varies.
#[tokio::test(flavor = "multi_thread")]
async fn the_key_never_varies_through_drops_delays_and_repetitions() {
    let clock = Arc::new(ManualClock::new(SystemClock.now_ms()));
    let world = World::with(|_| {}, clock.clone()).await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let mut agent = everything(&world, "balcao").await;
    let chat = chat_of(101);
    let expected = whatsapp_contact_key(&phone(1), &phone(101));

    // A small generator of the test, with a seed that is written down.
    let mut state: u64 = 0x2026_1124_0054_0001;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut seen = BTreeSet::new();
    let mut leaves = 0_usize;
    for round in 0..8 {
        let received = message(&chat, &format!("3EB0C{round:05}"), now_s(), false, "hello");
        assert_eq!(
            world
                .fake
                .emit(&session, "message.any", received)
                .await
                .unwrap(),
            200
        );
        for _ in 0..6 {
            let notice = next_notice(&mut agent).await;
            leaves += 1;
            seen.insert(key_of(&notice).expect("a key on every leaving"));
            let (answer, done) = match next() % 5 {
                0 => (Some(AckOutcome::Accepted), true),
                1 => (Some(AckOutcome::Duplicate), true),
                2 => (Some(AckOutcome::Failed), false),
                3 => (Some(AckOutcome::Throttled), false),
                // Dropped: no answer, the lease ends.
                _ => (None, false),
            };
            if let Some(outcome) = answer {
                let ack = Message::NoticeAck(NoticeAck {
                    id: notice.id,
                    outcome,
                });
                agent.session.connector().send(&ack).await.unwrap();
            }
            if done {
                break;
            }
            // The wait of a retry, or the end of the lease: once the hub has the answer.
            tokio::time::sleep(Duration::from_millis(300)).await;
            clock.advance(301_000);
        }
        // Whatever is left of the round is settled before the next message.
        tokio::time::sleep(Duration::from_millis(300)).await;
        clock.advance(301_000);
        while let Some(notice) = try_next_notice(&mut agent, Duration::from_millis(1_500)).await {
            leaves += 1;
            seen.insert(key_of(&notice).unwrap());
            let ack = Message::NoticeAck(NoticeAck {
                id: notice.id,
                outcome: AckOutcome::Accepted,
            });
            agent.session.connector().send(&ack).await.unwrap();
        }
    }
    assert!(leaves >= 8, "{leaves} notices left");
    assert_eq!(seen, BTreeSet::from([expected]));
}

// ---------------------------------------------------------------------------- who sent it

const SENT: &str = "3EB00000000000000001";

/// Two Hermes with every verb on a number, and one chat with a message the number sent that no
/// Hermes sent.
async fn two_readers() -> (World, String, Agent, Agent, String) {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let chat = chat_of(101);
    world.fake.add_chat(&session, &chat, "Ana");
    world.fake.register(&phone(101), &chat);
    world.fake.add_message(
        &session,
        &chat,
        message(&chat, "3EB0EE0001", 1_800_000_000, false, "hi there"),
    );
    // Sent from the app of the telephone.
    world.fake.add_message(
        &session,
        &chat,
        message(&chat, "3EB0EE0002", 1_800_000_001, true, "from the app"),
    );
    let first = world.agent().await;
    world.grant(&first.name, "balcao", all(ALL_VERBS)).await;
    let second = world.agent().await;
    world.grant(&second.name, "balcao", all(ALL_VERBS)).await;
    let listed_chat = {
        let mut first = first;
        let listed = ok(first.call("whatsapp_list_chats", json!({})).await);
        let id = listed["chats"][0]["chat"].as_str().unwrap().to_owned();
        (first, id)
    };
    let (first, id) = listed_chat;
    (world, session, first, second, id)
}

fn sent_by_of(read: &serde_json::Map<String, Value>) -> Vec<(String, Value)> {
    read["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["from_me"] == true)
        .map(|item| {
            (
                item["text"].as_str().unwrap().to_owned(),
                item["sent_by"].clone(),
            )
        })
        .collect()
}

/// The three values of `sent_by`, `outside_arauto` for what no record names (the app, and a
/// sending whose identifier the engine did not return), the name of another Hermes never given,
/// and the telephone of the contact on the chat.
#[tokio::test(flavor = "multi_thread")]
async fn a_reading_says_who_sent_each_message_and_never_names_another_hermes() {
    let (world, session, mut first, mut second, chat) = two_readers().await;

    // The first Hermes sends through Arauto; the engine gives the identifier SENT.
    let sent = ok(first
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "from the first"}),
        )
        .await);
    assert!(sent["message_id"].is_string());
    world.fake.add_message(
        &session,
        &chat_of(101),
        message(&chat_of(101), SENT, 1_800_000_002, true, "from the first"),
    );
    // A sending the engine accepted without saying the identifier of the message.
    world.fake.fail("/api/sendText", Fault::Malformed);
    let unknown = ok(first
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "no id came back"}),
        )
        .await);
    assert_eq!(unknown["message_id"], Value::Null);
    world.fake.heal();
    world.fake.add_message(
        &session,
        &chat_of(101),
        message(
            &chat_of(101),
            "3EB0EE0003",
            1_800_000_003,
            true,
            "no id came back",
        ),
    );
    // The second Hermes sends too: another identifier.
    ok(second
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "from the second"}),
        )
        .await);
    world.fake.add_message(
        &session,
        &chat_of(101),
        message(
            &chat_of(101),
            "3EB00000000000000002",
            1_800_000_004,
            true,
            "from the second",
        ),
    );

    let as_first = ok(first
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let as_second = ok(second
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let label = |value: &str| json!(value);
    let mut for_first = sent_by_of(&as_first);
    for_first.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        for_first,
        [
            ("from the app".to_owned(), label("outside_arauto")),
            ("from the first".to_owned(), label("you_through_arauto")),
            (
                "from the second".to_owned(),
                label("another_hermes_through_arauto")
            ),
            ("no id came back".to_owned(), label("outside_arauto")),
        ]
    );
    let mut for_second = sent_by_of(&as_second);
    for_second.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        for_second[1],
        (
            "from the first".to_owned(),
            label("another_hermes_through_arauto")
        )
    );
    assert_eq!(
        for_second[2],
        ("from the second".to_owned(), label("you_through_arauto"))
    );
    // A message of the other side has no label, and the chat says whose telephone it is.
    let theirs = as_first["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["from_me"] == false)
        .unwrap();
    assert!(theirs.get("sent_by").is_none(), "{theirs}");
    assert_eq!(as_first["phone"], format!("+{}", phone(101)));
    // The name of a Hermes never reaches another one.
    let shown = Value::Object(as_second.clone()).to_string();
    assert!(!shown.contains(&first.name), "{shown}");
    let shown = Value::Object(as_first.clone()).to_string();
    assert!(!shown.contains(&second.name), "{shown}");
}

/// The record is kept as a digest and goes with the retention: past it, the message is
/// `outside_arauto` again, and the table is empty.
#[tokio::test(flavor = "multi_thread")]
async fn the_record_of_a_sent_message_is_purged_with_its_retention() {
    let (world, session, mut first, _second, chat) = two_readers().await;
    ok(first
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "from the first"}),
        )
        .await);
    world.fake.add_message(
        &session,
        &chat_of(101),
        message(&chat_of(101), SENT, 1_800_000_002, true, "from the first"),
    );
    let read = ok(first
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    assert!(
        sent_by_of(&read)
            .iter()
            .any(|(_, by)| by == "you_through_arauto")
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM sent_message")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    assert_eq!(rows, 1);

    // The days of the retention, written in the table, from the sending.
    let days: i32 =
        sqlx::query_scalar("SELECT days FROM retention WHERE table_name = 'sent_message'")
            .fetch_one(world.hub().pool())
            .await
            .unwrap();
    assert_eq!(days, 30);
    let retention = repo::purge::retention(world.hub().pool()).await.unwrap();
    let day = 86_400_000;
    // Just before the thirty days, nothing goes; after them, the record goes.
    let sent_at: i64 = sqlx::query_scalar("SELECT sent_at FROM sent_message")
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    let early = repo::purge::purge(
        world.hub().pool(),
        world.organization(),
        &retention,
        sent_at + 29 * day,
        100,
        || true,
    )
    .await
    .unwrap();
    assert_eq!(early.of(repo::purge::Target::SentMessages).rows, 0);
    let late = repo::purge::purge(
        world.hub().pool(),
        world.organization(),
        &retention,
        sent_at + 31 * day,
        100,
        || true,
    )
    .await
    .unwrap();
    assert_eq!(late.of(repo::purge::Target::SentMessages).rows, 1);
    let read = ok(first
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    assert!(
        sent_by_of(&read)
            .iter()
            .all(|(_, by)| by == "outside_arauto")
    );
}

/// A Hermes removed from the hub leaves its records, which then say "another Hermes", never a
/// name: the column goes to null.
#[tokio::test(flavor = "multi_thread")]
async fn the_record_of_a_removed_hermes_says_another_hermes() {
    let (world, session, mut first, mut second, chat) = two_readers().await;
    ok(first
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "from the first"}),
        )
        .await);
    world.fake.add_message(
        &session,
        &chat_of(101),
        message(&chat_of(101), SENT, 1_800_000_002, true, "from the first"),
    );
    // The row of a Hermes that goes (a hard delete): the record stays and keeps no Hermes.
    let gone = world.fixture.hermes().await;
    let gone_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM hermes WHERE name = $1")
        .bind(&gone)
        .fetch_one(world.hub().pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sent_message (id, organization_id, number_id, chat_id, hermes_id, \
                                   id_digest, sent_at, purge_at) \
         SELECT gen_random_uuid(), organization_id, number_id, chat_id, $1, \
                decode(md5('a') || md5('b'), 'hex'), sent_at, purge_at FROM sent_message",
    )
    .bind(gone_id)
    .execute(world.hub().pool())
    .await
    .unwrap();
    sqlx::query("DELETE FROM hermes WHERE id = $1")
        .bind(gone_id)
        .execute(world.hub().pool())
        .await
        .unwrap();
    let orphans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sent_message WHERE hermes_id IS NULL")
            .fetch_one(world.hub().pool())
            .await
            .unwrap();
    assert_eq!(orphans, 1, "the record stays, with no Hermes");
    // The same state for the record of the first Hermes: another Hermes to whoever reads.
    sqlx::query("UPDATE sent_message SET hermes_id = NULL")
        .execute(world.hub().pool())
        .await
        .unwrap();
    let read = ok(second
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let labels = sent_by_of(&read);
    assert!(
        labels
            .iter()
            .any(|(text, by)| text == "from the first" && by == "another_hermes_through_arauto"),
        "{labels:?}"
    );
}

/// The reading still cuts at whole messages within 256 KiB when every message of the number
/// carries its label, and following `next_before` reads each one once.
#[tokio::test(flavor = "multi_thread")]
async fn the_budget_of_a_reading_still_cuts_at_whole_messages_with_the_labels() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(3)).await;
    let session = number.session.as_str().to_owned();
    let chat = chat_of(70);
    world.fake.add_chat(&session, &chat, "Big");
    let text = "x".repeat(60 * 1024);
    for n in 0..12 {
        let at = 1_800_000_000 + n / 3;
        world.fake.add_message(
            &session,
            &chat,
            message(&chat, &format!("3EB0D{n:07}"), at, true, &text),
        );
    }
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    let chat_id = listed["chats"][0]["chat"].as_str().unwrap().to_owned();
    let mut seen = Vec::new();
    let mut before: Option<i64> = None;
    for _ in 0..12 {
        let mut args = json!({"chat": chat_id, "limit": 200});
        if let Some(before) = before {
            args["before"] = json!(before);
        }
        let page = ok(agent.call("whatsapp_read_chat", args).await);
        let size = Value::Object(page.clone()).to_string().len();
        assert!(size <= 256 * 1024, "{size} bytes");
        for item in page["messages"].as_array().unwrap() {
            assert_eq!(item["sent_by"], "outside_arauto");
            assert_eq!(item["text"].as_str().unwrap().len(), text.len());
            seen.push(item["message_id"].as_str().unwrap().to_owned());
        }
        if page["more"] == false {
            break;
        }
        before = page["next_before"].as_i64();
        assert!(before.is_some());
    }
    let unique: BTreeSet<&String> = seen.iter().collect();
    assert_eq!(seen.len(), 12, "every message once");
    assert_eq!(unique.len(), 12);
}

/// The phone of the chat in a reading: a contact's telephone, `null` for a group, and for a
/// hidden identifier what the engine says (null when it does not know).
#[tokio::test(flavor = "multi_thread")]
async fn a_reading_gives_the_phone_of_the_chat_or_null() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str().to_owned();
    let group = "120363012345678901@g.us";
    world.fake.add_chat(&session, group, "Team");
    world.fake.add_chat(&session, HIDDEN, "Hidden");
    world.fake.add_chat(&session, &chat_of(101), "Ana");
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    let listed = ok(agent
        .call("whatsapp_list_chats", json!({"limit": 50}))
        .await);
    let chat_named = |name: &str| {
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
    for (name, expected) in [
        ("Ana", json!(format!("+{}", phone(101)))),
        ("Team", Value::Null),
        ("Hidden", Value::Null),
    ] {
        let read = ok(agent
            .call("whatsapp_read_chat", json!({"chat": chat_named(name)}))
            .await);
        assert_eq!(read["phone"], expected, "{name}");
    }
    world.fake.add_lid(HIDDEN, &format!("{}@c.us", phone(102)));
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat_named("Hidden")}))
        .await);
    assert_eq!(read["phone"], format!("+{}", phone(102)));
}

/// The engine message id is kept as a digest: the table never holds it in the clear, whichever
/// way it is written.
#[tokio::test(flavor = "multi_thread")]
async fn the_engine_message_id_is_never_in_the_table_in_the_clear() {
    let (world, _session, mut first, _second, chat) = two_readers().await;
    ok(first
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "from the first"}),
        )
        .await);
    let rows: Vec<String> = sqlx::query_scalar("SELECT row_to_json(t)::text FROM sent_message t")
        .fetch_all(world.hub().pool())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let stored = rows.join("\n");
    let full = EngineMessageId::parse(&format!("true_{}_{SENT}", chat_of(101))).unwrap();
    assert_eq!(full.whatsapp_id(), SENT);
    for needle in [SENT, &SENT[4..], &phone(101), &phone(1)] {
        assert!(!stored.contains(needle), "{needle} in {stored}");
    }
}
