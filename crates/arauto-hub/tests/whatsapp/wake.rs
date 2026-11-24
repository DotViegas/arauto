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

pub fn chat_of(n: u32) -> String {
    format!("{}@c.us", phone(n))
}

pub fn now_s() -> i64 {
    SystemClock.now_ms() / 1_000
}

/// The next `notice` of the session; a `tools` on the way is kept.
pub async fn next_notice(agent: &mut Agent) -> NoticeMsg {
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
pub async fn notices_while_quiet(agent: &mut Agent) -> usize {
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

pub async fn acknowledge(agent: &mut Agent, notice: &NoticeMsg, outcome: AckOutcome) {
    let ack = Message::NoticeAck(NoticeAck {
        id: notice.id,
        outcome,
    });
    agent.session.connector().send(&ack).await.unwrap();
    // The answer of a call comes after the hub handled the `notice_ack` before it.
    ok(agent.call("arauto_status", json!({})).await);
}

pub fn pointer(notice: &NoticeMsg) -> (String, String, u16) {
    match notice.validate().unwrap() {
        Notice::Whatsapp {
            number,
            chat,
            count,
            ..
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

/// The messages of `alias` wake `agent` at once: the tests that do not look at the wait.
pub async fn at_once(world: &World, agent: &Agent, alias: &str) {
    world
        .fixture
        .admin
        .access_wake_wait(&agent.name, alias, 0, world.now())
        .await
        .unwrap();
}

/// A Hermes in session. What wakes it by WhatsApp is the verb "be woken" alone (decision 0049),
/// so a new Hermes needs nothing else besides the grant of each test.
pub async fn wakeable(world: &World) -> Agent {
    world.agent().await
}

#[tokio::test]
async fn a_message_wakes_with_a_pointer_that_carries_no_text_and_no_telephone() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let mut agent = wakeable(&world).await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &agent, "balcao").await;

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
    // No telephone in any spelling, the contact's or the number's: the key is a digest.
    for telephone in [phone(101), phone(1)] {
        let without_ninth = format!("{}{}", &telephone[..4], &telephone[5..]);
        for spelling in [
            telephone.clone(),
            format!("+{telephone}"),
            without_ninth.clone(),
            format!("+{without_ninth}"),
            format!("{telephone}@c.us"),
        ] {
            assert!(!travelled.contains(&spelling), "{spelling} in {travelled}");
        }
    }
    // The notice carries the key of the Hermes session of this contact (decision 0054), the
    // one anybody computes from the two telephones.
    let Notice::Whatsapp {
        session: Some(key), ..
    } = notice.validate().unwrap()
    else {
        panic!("a notice of WhatsApp with a key")
    };
    assert_eq!(
        key.as_str(),
        arauto_protocol::session_key::whatsapp_contact_key(&phone(1), &phone(101))
    );
    assert!(travelled.contains(key.as_str()), "{travelled}");
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
    at_once(&world, &agent, "balcao").await;
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
async fn only_who_has_the_verb_in_that_chat_is_woken() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let open_chat = chat_of(101);
    let listed_chat = chat_of(102);
    let blocked_chat = chat_of(103);

    // Every verb on the number: woken wherever no line says otherwise.
    let mut whole = wakeable(&world).await;
    world.grant(&whole.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &whole, "balcao").await;
    world
        .line(&whole.name, "balcao", &blocked_chat, READ_SEND)
        .await;
    // Only the chats of its list, and only one of them wakes.
    let mut listed = wakeable(&world).await;
    world.grant(&listed.name, "balcao", list(READ)).await;
    at_once(&world, &listed, "balcao").await;
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
    at_once(&world, &reader, "balcao").await;
    // Every verb but "be woken".
    let mut asleep = world.agent().await;
    world
        .grant(&asleep.name, "balcao", all(verbs(true, true, true, false)))
        .await;
    at_once(&world, &asleep, "balcao").await;
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
    at_once(&world, &agent, "balcao").await;
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

#[tokio::test]
async fn the_notice_waits_for_the_chat_to_go_quiet_and_wakes_once() {
    let clock = Arc::new(ManualClock::new(SystemClock.now_ms()));
    let world = World::with(|_| {}, clock.clone()).await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let mut agent = wakeable(&world).await;
    // The wait of a new access: six seconds.
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    let chat = chat_of(101);
    let emit = |id: &'static str| {
        let received = message(&chat, id, now_s(), false, "hello");
        world.fake.emit(session, "message.any", received)
    };

    assert_eq!(emit("3EB0EE0001").await.unwrap(), 200);
    assert_eq!(notices_while_quiet(&mut agent).await, 0, "not at once");
    clock.advance(4_000);
    assert_eq!(emit("3EB0EE0002").await.unwrap(), 200);
    // Seven seconds after the first message, three after the last: it still waits.
    clock.advance(3_000);
    assert_eq!(notices_while_quiet(&mut agent).await, 0);
    clock.advance(3_500);
    let notice = next_notice(&mut agent).await;
    assert_eq!(pointer(&notice).2, 2, "one notice, with the two messages");
    assert_eq!(notices_while_quiet(&mut agent).await, 0);

    // Someone who never stops writing does not hold the notice for ever: a minute at most.
    acknowledge(&mut agent, &notice, AckOutcome::Accepted).await;
    clock.advance(11_000);
    assert_eq!(emit("3EB0EE0003").await.unwrap(), 200);
    for id in [
        "3EB0EE0004",
        "3EB0EE0005",
        "3EB0EE0006",
        "3EB0EE0007",
        "3EB0EE0008",
    ] {
        clock.advance(5_000);
        assert_eq!(emit(id).await.unwrap(), 200);
    }
    for id in [
        "3EB0EE0009",
        "3EB0EE0010",
        "3EB0EE0011",
        "3EB0EE0012",
        "3EB0EE0013",
        "3EB0EE0014",
    ] {
        clock.advance(5_000);
        assert_eq!(emit(id).await.unwrap(), 200);
    }
    // 55 seconds of messages every five seconds: the wait of six never passed.
    assert_eq!(notices_while_quiet(&mut agent).await, 0);
    clock.advance(5_500);
    let late = next_notice(&mut agent).await;
    assert_eq!(pointer(&late).2, 12);

    // The wait is of the access, and the admin sets it; out of its range it is refused.
    assert!(matches!(
        world
            .fixture
            .admin
            .access_wake_wait(&agent.name, "balcao", 61, world.now())
            .await,
        Err(arauto_hub::admin::AdminError::WakeWaitRange)
    ));
    assert_eq!(
        world
            .fixture
            .admin
            .access_wake_wait(&agent.name, "balcao", 0, world.now())
            .await
            .unwrap(),
        Some(6)
    );
}

#[tokio::test]
async fn a_group_wakes_only_by_a_line_of_its_own() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let group = "120363012345678901@g.us";
    let in_group = |id: &'static str| {
        let mut received = message(group, id, now_s(), false, "hello everyone");
        received["participant"] = json!(chat_of(201));
        world.fake.emit(session, "message.any", received)
    };

    // Every verb on the whole number: woken by a person, not by a group.
    let mut whole = wakeable(&world).await;
    world.grant(&whole.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &whole, "balcao").await;
    // The same, and a line for this group that wakes.
    let mut in_line = wakeable(&world).await;
    world.grant(&in_line.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &in_line, "balcao").await;
    world
        .line(
            &in_line.name,
            "balcao",
            group,
            verbs(true, false, false, true),
        )
        .await;

    assert_eq!(in_group("3EB0FF0001").await.unwrap(), 200);
    let notice = next_notice(&mut in_line).await;
    assert_eq!(pointer(&notice).2, 1);
    assert_eq!(notices_while_quiet(&mut whole).await, 0);
    let woken: Vec<String> = notices(&world)
        .await
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();
    assert_eq!(woken, [in_line.name.clone()]);

    // A person writing to the number still wakes the one without the line.
    let direct = chat_of(101);
    let received = message(&direct, "3EB0FF0002", now_s(), false, "hello");
    assert_eq!(
        world
            .fake
            .emit(session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    assert_eq!(pointer(&next_notice(&mut whole).await).2, 1);
}

#[tokio::test]
async fn a_number_that_is_coming_back_keeps_its_notices_until_it_is_connected() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let mut agent = wakeable(&world).await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &agent, "balcao").await;
    let status = |status: &str, after_ms: i64| {
        world
            .fake
            .envelope_at(
                session,
                "session.status",
                json!({"status": status, "statuses": []}),
                None,
                SystemClock.now_ms() + after_ms,
            )
            .unwrap()
    };

    // The engine is starting again: the number is coming back, not out of use.
    let starting = status("STARTING", 1_000);
    assert_eq!(world.fake.deliver(session, &starting).await.unwrap(), 200);
    assert_eq!(
        world.state("balcao").await,
        arauto_hub::core::number::NumberState::Reconnecting
    );
    let received = message(&chat_of(101), "3EB0AB0001", now_s(), false, "hello");
    assert_eq!(
        world
            .fake
            .emit(session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    // The notice is made and waits: it neither leaves nor is thrown away.
    assert_eq!(notices_while_quiet(&mut agent).await, 0);
    assert_eq!(
        notices(&world).await,
        [(agent.name.clone(), "pending".to_owned(), 1)]
    );

    let working = status("WORKING", 2_000);
    assert_eq!(world.fake.deliver(session, &working).await.unwrap(), 200);
    let notice = next_notice(&mut agent).await;
    assert_eq!(pointer(&notice).2, 1);
}

#[tokio::test]
async fn a_chat_under_a_hidden_identifier_never_escapes_a_line_of_the_admin() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    // The same person, as the engine may name it in an event: a hidden identifier.
    let hidden = "123456789012345@lid";
    let from_hidden = |id: &'static str| {
        let received = message(hidden, id, now_s(), false, "hello");
        world.fake.emit(session, "message.any", received)
    };

    // No line at all: the number is open, and a hidden identifier wakes like anyone.
    let mut open = wakeable(&world).await;
    world.grant(&open.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &open, "balcao").await;
    // One line that blocks a person by telephone: the hub cannot tell whether the hidden
    // identifier is that person, so it is blocked too.
    let mut lined = wakeable(&world).await;
    world.grant(&lined.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &lined, "balcao").await;
    world
        .line(
            &lined.name,
            "balcao",
            &chat_of(103),
            verbs(false, false, false, false),
        )
        .await;

    assert_eq!(from_hidden("3EB0AC0001").await.unwrap(), 200);
    let notice = next_notice(&mut open).await;
    let hidden_chat = pointer(&notice).1;
    assert_eq!(notices_while_quiet(&mut lined).await, 0);
    // Nor does it read or write there: the chat is the one that does not exist.
    let read = lined
        .call(
            "whatsapp_read_chat",
            json!({"number": "balcao", "chat": hidden_chat}),
        )
        .await;
    assert_eq!(
        crate::agent::refused(&read),
        Some(arauto_protocol::errors::ErrorCode::Ar705)
    );

    // A line of its own, given by the admin for that chat, opens it.
    world
        .line(
            &lined.name,
            "balcao",
            hidden,
            verbs(true, false, false, true),
        )
        .await;
    assert_eq!(from_hidden("3EB0AC0002").await.unwrap(), 200);
    assert_eq!(pointer(&next_notice(&mut lined).await).1, hidden_chat);
}

/// SECURITY RULE: a WhatsApp notice that waits is judged again when it leaves, by the verb "be
/// woken" alone (decision 0049). Turned off on the access, or on the line of the chat that had
/// its own rule, the notice is discarded and nothing leaves.
#[tokio::test]
async fn a_waiting_whatsapp_notice_is_discarded_when_be_woken_goes_off() {
    let clock = Arc::new(ManualClock::new(SystemClock.now_ms()));
    let world = World::with(|_| {}, clock.clone()).await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let chat = chat_of(101);
    let line_chat = chat_of(102);
    // (i) the access has the verb and no line; (ii) the access has the verb and the chat has a
    // line of its own that wakes.
    let mut by_access = wakeable(&world).await;
    world.grant(&by_access.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &by_access, "balcao").await;
    // The chat of the other one is blocked for it, so each Hermes is woken in one chat only.
    world
        .line(&by_access.name, "balcao", &line_chat, READ_SEND)
        .await;
    let mut by_line = wakeable(&world).await;
    world.grant(&by_line.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &by_line, "balcao").await;
    world.line(&by_line.name, "balcao", &chat, READ_SEND).await;
    world
        .line(
            &by_line.name,
            "balcao",
            &line_chat,
            verbs(true, false, false, true),
        )
        .await;
    let emit = |chat: &str, id: &'static str| {
        let received = message(chat, id, now_s(), false, "hello");
        world.fake.emit(session, "message.any", received)
    };

    // The first notice of each leaves at once and is answered.
    assert_eq!(emit(&chat, "3EB0FF0001").await.unwrap(), 200);
    assert_eq!(emit(&line_chat, "3EB0FF0011").await.unwrap(), 200);
    let first = next_notice(&mut by_access).await;
    acknowledge(&mut by_access, &first, AckOutcome::Accepted).await;
    let first = next_notice(&mut by_line).await;
    acknowledge(&mut by_line, &first, AckOutcome::Accepted).await;

    // A second one of each waits its ten seconds; "be woken" goes off meanwhile (the access
    // keeps every other verb): when it would leave, the verb is read again and the notice is
    // discarded.
    assert_eq!(emit(&chat, "3EB0FF0002").await.unwrap(), 200);
    assert_eq!(emit(&line_chat, "3EB0FF0012").await.unwrap(), 200);
    world
        .grant(
            &by_access.name,
            "balcao",
            all(verbs(true, true, true, false)),
        )
        .await;
    world
        .line(
            &by_line.name,
            "balcao",
            &line_chat,
            verbs(true, false, false, false),
        )
        .await;
    clock.advance(11_000);
    assert_eq!(notices_while_quiet(&mut by_access).await, 0);
    assert_eq!(notices_while_quiet(&mut by_line).await, 0);
    let mut states: Vec<String> = notices(&world)
        .await
        .into_iter()
        .map(|(_, state, _)| state)
        .collect();
    states.sort();
    assert_eq!(states, ["confirmed", "confirmed", "discarded", "discarded"]);
}

/// SECURITY RULE: a message that arrives in a chat whose line has "be woken" off creates no
/// notice, even when the access has the verb.
#[tokio::test]
async fn a_chat_line_without_be_woken_makes_no_notice_even_when_the_access_has_the_verb() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    let mut agent = wakeable(&world).await;
    world.grant(&agent.name, "balcao", all(ALL_VERBS)).await;
    at_once(&world, &agent, "balcao").await;
    let quiet_chat = chat_of(101);
    world
        .line(
            &agent.name,
            "balcao",
            &quiet_chat,
            verbs(true, true, false, false),
        )
        .await;
    let received = message(&quiet_chat, "3EB0FF0101", now_s(), false, "hello");
    assert_eq!(
        world
            .fake
            .emit(session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    assert_eq!(notices_while_quiet(&mut agent).await, 0);
    assert!(notices(&world).await.is_empty());
}

#[tokio::test]
async fn the_verb_gates_whatsapp_and_never_notes() {
    let world = World::new().await;
    let number = world.connected("balcao", &phone(1)).await;
    let session = number.session.as_str();
    // Reads and sends on the number, without "be woken", and a marked link from another agent.
    let mut target = world.agent().await;
    let mut sender = world.agent().await;
    world.grant(&target.name, "balcao", all(READ_SEND)).await;
    at_once(&world, &target, "balcao").await;
    world
        .fixture
        .admin
        .link_add(&sender.name, &target.name, true, false, world.now())
        .await
        .unwrap();

    // A WhatsApp message wakes nobody: the verb is off.
    let received = message(&chat_of(101), "3EB0FF1001", now_s(), false, "hello");
    assert_eq!(
        world
            .fake
            .emit(session, "message.any", received)
            .await
            .unwrap(),
        200
    );
    assert_eq!(notices_while_quiet(&mut target).await, 0);
    assert!(notices(&world).await.is_empty());

    // A note wakes it: the mark of the link is enough.
    ok(sender
        .call(
            "hermes_send_message",
            json!({"to": target.name, "text": "wake up"}),
        )
        .await);
    let notice = next_notice(&mut target).await;
    assert!(matches!(notice.validate().unwrap(), Notice::Peer { .. }));
}
