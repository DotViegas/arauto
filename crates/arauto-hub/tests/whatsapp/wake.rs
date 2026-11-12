//! A message that arrives on a number wakes the Hermes that may be woken in that chat, and
//! only them (`docs/08-permissoes.md`, section 4; `docs/09-contrato-mcp.md`, section 5).

use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::message::{Message, NoticeAck};
use arauto_protocol::notice::{AckOutcome, Notice, NoticeMsg};
use arauto_testkit::KitError;
use serde_json::json;

use crate::agent::{ALL_VERBS, Agent, PATIENCE, READ, READ_SEND, all, list, ok, verbs};
use crate::world::{World, message, phone};

const CANARY: &str = "CANARY-text-of-a-whatsapp-message-91c2";

/// How long a test waits to say that no notice came: the hub looks at its queues every 200 ms.
const QUIET: Duration = Duration::from_millis(900);

fn chat_of(n: u32) -> String {
    format!("{}@c.us", phone(n))
}

fn now_s() -> i64 {
    SystemClock.now_ms() / 1_000
}

/// The next `notice` of the session; a `tools` on the way is kept.
async fn next_notice(agent: &mut Agent) -> NoticeMsg {
    loop {
        match agent.session.pump(PATIENCE).await {
            Ok(Some(Message::Notice(notice))) => return notice,
            Ok(Some(Message::Tools(tools))) => agent.tools.push(tools.tools),
            Ok(Some(other)) => panic!("the hub sent {:?}", other.kind()),
            Ok(None) => {}
            Err(KitError::TimedOut) => panic!("no `notice` came"),
            Err(error) => panic!("{error:?}"),
        }
    }
}

/// How many notices come while the session is kept for [`QUIET`].
async fn notices_while_quiet(agent: &mut Agent) -> usize {
    let until = tokio::time::Instant::now() + QUIET;
    let mut came = 0;
    loop {
        let left = until.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return came;
        }
        match agent.session.pump(left).await {
            Ok(Some(Message::Notice(_))) => came += 1,
            Ok(Some(Message::Tools(tools))) => agent.tools.push(tools.tools),
            Ok(Some(other)) => panic!("the hub sent {:?}", other.kind()),
            Ok(None) | Err(KitError::TimedOut) => {}
            Err(error) => panic!("{error:?}"),
        }
    }
}

async fn acknowledge(agent: &mut Agent, notice: &NoticeMsg, outcome: AckOutcome) {
    let ack = Message::NoticeAck(NoticeAck {
        id: notice.id,
        outcome,
    });
    agent.session.connector().send(&ack).await.unwrap();
    // The answer of a call comes after the hub handled the `notice_ack` before it.
    ok(agent.call("arauto_status", json!({})).await);
}

fn pointer(notice: &NoticeMsg) -> (String, String, u16) {
    match notice.validate().unwrap() {
        Notice::Whatsapp {
            number,
            chat,
            count,
        } => (
            number.as_str().to_owned(),
            chat.as_str().to_owned(),
            count.get(),
        ),
        Notice::Peer { .. } => panic!("a notice of messages of a Hermes"),
    }
}

/// The notices of WhatsApp in the database: the Hermes, the state and the count.
async fn notices(world: &World) -> Vec<(String, String, i32)> {
    sqlx::query_as(
        "SELECT h.name, n.state, n.count FROM notice n JOIN hermes h ON h.id = n.hermes_id \
         WHERE n.kind = 'whatsapp' ORDER BY n.created_at, n.id",
    )
    .fetch_all(world.hub().pool())
    .await
    .unwrap()
}

async fn open_events(world: &World) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM engine_event WHERE processed_at IS NULL")
        .fetch_one(world.hub().pool())
        .await
        .unwrap()
}

/// A Hermes in session that may be woken.
async fn wakeable(world: &World) -> Agent {
    let agent = world.agent().await;
    world
        .fixture
        .admin
        .wake(&agent.name, true, world.now())
        .await
        .unwrap();
    agent
}

#[tokio::test]
async fn a_message_wakes_with_a_pointer_that_carries_no_text_and_no_telephone() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let mut agent = wakeable(&world).await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;

    let chat = chat_of(101);
    let received = message(&chat, "3EB0AA0001", now_s(), false, CANARY);
    world.fake.add_chat(session, &chat, "Someone");
    world.fake.add_message(session, &chat, received.clone());
    assert_eq!(
        world
            .fake
            .emit(session, "message.any", received)
            .await
            .unwrap(),
        200
    );

    let notice = next_notice(&mut agent).await;
    let (alias, chat_id, count) = pointer(&notice);
    assert_eq!((alias.as_str(), count), ("balcao", 1));
    let travelled = serde_json::to_string(&notice).unwrap();
    assert!(!travelled.contains(CANARY), "{travelled}");
    assert!(!travelled.contains(&phone(101)), "{travelled}");
    assert!(!travelled.contains("Someone"), "{travelled}");
    assert_eq!(open_events(&world).await, 0, "the event was handled");

    acknowledge(&mut agent, &notice, AckOutcome::Accepted).await;
    assert_eq!(
        notices(&world).await,
        [(agent.name.clone(), "confirmed".to_owned(), 1)]
    );
    // The chat of the notice is the one the tools take: the Hermes reads what woke it.
    let read = ok(agent
        .call(
            "whatsapp_read_chat",
            json!({"number": "balcao", "chat": chat_id}),
        )
        .await);
    assert!(read.get("messages").unwrap().to_string().contains(CANARY));
}

#[tokio::test]
async fn what_the_number_sent_and_what_is_no_message_wake_nobody() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let mut agent = wakeable(&world).await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    let chat = chat_of(101);

    let own = message(&chat, "3EB0BB0001", now_s(), true, "sent by the number");
    assert_eq!(
        world.fake.emit(session, "message.any", own).await.unwrap(),
        200
    );
    let reaction = json!({"id": "false_x", "reaction": {"text": "x", "messageId": "3EB0BB0001"}});
    assert_eq!(
        world
            .fake
            .emit(session, "message.reaction", reaction)
            .await
            .unwrap(),
        200
    );
    assert_eq!(notices_while_quiet(&mut agent).await, 0);
    assert!(notices(&world).await.is_empty());
    assert_eq!(open_events(&world).await, 0, "recorded and handled");
}

#[tokio::test]
async fn only_who_has_the_verb_in_that_chat_and_may_be_woken_is_woken() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let open_chat = chat_of(101);
    let listed_chat = chat_of(102);
    let blocked_chat = chat_of(103);

    // Every verb on the number, and may be woken: woken wherever no line says otherwise.
    let mut whole = wakeable(&world).await;
    world.grant(&whole.name, "balcao", all(ALL_VERBS)).await;
    world
        .line(&whole.name, "balcao", &blocked_chat, READ_SEND)
        .await;
    // Only the chats of its list, and only one of them wakes.
    let mut listed = wakeable(&world).await;
    world.grant(&listed.name, "balcao", list(READ)).await;
    world
        .line(
            &listed.name,
            "balcao",
            &listed_chat,
            verbs(true, false, false, true),
        )
        .await;
    // Reads and sends, without the verb.
    let mut reader = wakeable(&world).await;
    world.grant(&reader.name, "balcao", all(READ_SEND)).await;
    // Every verb, and the Hermes may not be woken.
    let mut asleep = world.agent().await;
    world.grant(&asleep.name, "balcao", all(ALL_VERBS)).await;
    // No access to the number at all.
    let mut stranger = wakeable(&world).await;

    for (chat, id) in [
        (&open_chat, "3EB0CC0001"),
        (&listed_chat, "3EB0CC0002"),
        (&blocked_chat, "3EB0CC0003"),
    ] {
        let received = message(chat, id, now_s(), false, "hello");
        assert_eq!(
            world
                .fake
                .emit(session, "message.any", received)
                .await
                .unwrap(),
            200
        );
    }
    // `whole`: the open chat and the listed one (its line of the third has no wake).
    let first = next_notice(&mut whole).await;
    let second = next_notice(&mut whole).await;
    assert_ne!(pointer(&first).1, pointer(&second).1, "one notice per chat");
    assert_eq!(notices_while_quiet(&mut whole).await, 0);
    // `listed`: only the chat whose line wakes.
    let only = next_notice(&mut listed).await;
    assert_eq!(pointer(&only).2, 1);
    assert_eq!(notices_while_quiet(&mut listed).await, 0);
    for agent in [&mut reader, &mut asleep, &mut stranger] {
        assert_eq!(notices_while_quiet(agent).await, 0, "{}", agent.name);
    }
    let woken: Vec<String> = notices(&world)
        .await
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();
    let mut expected = vec![whole.name.clone(), whole.name.clone(), listed.name.clone()];
    expected.sort();
    let mut woken_sorted = woken;
    woken_sorted.sort();
    assert_eq!(woken_sorted, expected);
}

#[tokio::test]
async fn one_pending_notice_per_chat_grows_and_is_judged_again_when_it_leaves() {
    // The clock of the hub is moved by the test, from the time of the system: the events of
    // the fake engine carry the time of the system.
    let clock = Arc::new(ManualClock::new(SystemClock.now_ms()));
    let world = World::with(|_| {}, clock.clone()).await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let mut agent = wakeable(&world).await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    let chat = chat_of(101);
    let emit = |id: &'static str| {
        let received = message(&chat, id, now_s(), false, "hello");
        world.fake.emit(session, "message.any", received)
    };

    assert_eq!(emit("3EB0DD0001").await.unwrap(), 200);
    let first = next_notice(&mut agent).await;
    acknowledge(&mut agent, &first, AckOutcome::Accepted).await;

    // Two more before the ten seconds: one notice waits, with the two.
    assert_eq!(emit("3EB0DD0002").await.unwrap(), 200);
    assert_eq!(emit("3EB0DD0003").await.unwrap(), 200);
    assert_eq!(notices_while_quiet(&mut agent).await, 0);
    let mut states: Vec<(String, i32)> = notices(&world)
        .await
        .into_iter()
        .map(|(_, state, count)| (state, count))
        .collect();
    states.sort();
    assert_eq!(
        states,
        [("confirmed".to_owned(), 1), ("pending".to_owned(), 2)]
    );

    clock.advance(11_000);
    let second = next_notice(&mut agent).await;
    assert_eq!(pointer(&second).2, 2);
    assert_eq!(pointer(&second).1, pointer(&first).1, "the same chat");
    acknowledge(&mut agent, &second, AckOutcome::Accepted).await;

    // A third one waits; the access goes meanwhile, and the notice with it when it would leave.
    assert_eq!(emit("3EB0DD0004").await.unwrap(), 200);
    world.take_away(&agent.name, "balcao").await;
    clock.advance(11_000);
    assert_eq!(notices_while_quiet(&mut agent).await, 0);
    let mut states: Vec<(String, i32)> = notices(&world)
        .await
        .into_iter()
        .map(|(_, state, count)| (state, count))
        .collect();
    states.sort();
    assert_eq!(
        states,
        [
            ("confirmed".to_owned(), 1),
            ("confirmed".to_owned(), 2),
            ("discarded".to_owned(), 1)
        ]
    );
}
