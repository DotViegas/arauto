//! The send limit through the session (`docs/08-permissoes.md`, section 7): 20 per minute,
//! per Hermes and per number, on the clock of the hub; the refusal with `retry_after_s`; calls
//! at the same time; a second session of the same Hermes; what does not count.

use std::sync::Arc;

use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::CallOutcome;
use arauto_testkit::device::Faults;
use serde_json::json;

use crate::agent::{Agent, READ_SEND, all, ok, refused};
use crate::world::{World, message, phone};

/// A world on a clock the test moves, with the number `balcao`, one chat and a Hermes that
/// may read and send on it. Returns the chat as the hub names it.
async fn ready() -> (World, ManualClock, Agent, String) {
    let clock = ManualClock::new(SystemClock.now_ms());
    let world = World::with(|_| {}, Arc::new(clock.clone())).await;
    let number = world.connected("balcao", &phone(6)).await;
    let session = number.session.as_str().to_owned();
    let ana = format!("{}@c.us", phone(56));
    world.fake.add_chat(&session, &ana, "Ana");
    world.fake.add_message(
        &session,
        &ana,
        message(&ana, "3EB0G0000001", 1_800_000_000, false, "hi"),
    );
    let mut agent = world.agent().await;
    world.grant(&agent.name, "balcao", all(READ_SEND)).await;
    let listed = ok(agent.call("whatsapp_list_chats", json!({})).await);
    let chat = listed["chats"][0]["chat"].as_str().unwrap().to_owned();
    (world, clock, agent, chat)
}

fn wait_of(outcome: &CallOutcome) -> Option<u32> {
    match outcome {
        CallOutcome::Refused {
            code: ErrorCode::Ar704,
            retry_after,
        } => retry_after.map(|wait| wait.get()),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn twenty_sendings_a_minute_then_ar704_with_the_wait() {
    let (world, clock, mut agent, chat) = ready().await;
    for n in 0..20 {
        let sent = agent
            .call(
                "whatsapp_send_text",
                json!({"to": chat, "text": format!("m{n}")}),
            )
            .await;
        assert_eq!(refused(&sent), None, "sending {n}");
        clock.advance(1_000);
    }
    // The first was 20 seconds ago: it leaves the window in 40.
    let refused_now = agent
        .call(
            "whatsapp_send_text",
            json!({"to": chat, "text": "one more"}),
        )
        .await;
    assert_eq!(wait_of(&refused_now), Some(40));
    // Every sending counts in the same limit, and a reaction is one.
    let read = ok(agent
        .call("whatsapp_read_chat", json!({"chat": chat}))
        .await);
    let message_id = read["messages"][0]["message_id"].clone();
    let react = agent
        .call(
            "whatsapp_react",
            json!({"message_id": message_id, "reaction": "x"}),
        )
        .await;
    assert!(wait_of(&react).is_some());
    let check = agent
        .call("whatsapp_check_number", json!({"phone": phone(57)}))
        .await;
    assert!(wait_of(&check).is_some());
    // What does not write anything new does not count: reading, marking as read, typing.
    ok(agent
        .call("whatsapp_mark_read", json!({"chat": chat}))
        .await);
    ok(agent
        .call("whatsapp_set_typing", json!({"chat": chat, "on": false}))
        .await);
    assert_eq!(
        world
            .fake
            .sent()
            .iter()
            .filter(|(a, _)| a == "sendText")
            .count(),
        20
    );
    // Refusals did not count: at 60 seconds the first left the window, and only it.
    clock.advance(40_000);
    ok(agent
        .call("whatsapp_send_text", json!({"to": chat, "text": "again"}))
        .await);
    let full = agent
        .call("whatsapp_send_text", json!({"to": chat, "text": "full"}))
        .await;
    assert_eq!(wait_of(&full), Some(1));

    // Another Hermes on the same number has its own count.
    let mut second = world.agent().await;
    world.grant(&second.name, "balcao", all(READ_SEND)).await;
    ok(second
        .call("whatsapp_send_text", json!({"to": chat, "text": "mine"}))
        .await);
}

#[tokio::test(flavor = "multi_thread")]
async fn of_calls_at_the_same_time_exactly_twenty_go_through() {
    let (world, _clock, mut agent, chat) = ready().await;
    let ids: Vec<u64> = (1..=28).collect();
    for id in &ids {
        agent
            .send(
                *id,
                "whatsapp_send_text",
                json!({"to": chat, "text": format!("m{id}")}),
            )
            .await;
    }
    let outcomes = agent.results(&ids).await;
    let passed = outcomes.iter().filter(|o| refused(o).is_none()).count();
    let limited = outcomes.iter().filter(|o| wait_of(o).is_some()).count();
    assert_eq!((passed, limited), (20, 8), "{outcomes:?}");
    assert_eq!(world.fake.sent().len(), 20);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_count_is_of_the_hermes_and_not_of_the_session() {
    let (world, clock, mut agent, chat) = ready().await;
    let ids: Vec<u64> = (100..120).collect();
    for id in &ids {
        agent
            .send(*id, "whatsapp_send_text", json!({"to": chat, "text": "x"}))
            .await;
    }
    for outcome in agent.results(&ids).await {
        assert_eq!(refused(&outcome), None);
    }
    let Agent { name, session, .. } = agent;
    // The same device connects again: a new session, the same count.
    let device = session.device().clone();
    drop(session);
    let again = device
        .channel(Faults::default())
        .await
        .unwrap()
        .session()
        .unwrap();
    let mut agent = Agent::from_session(name, again);
    let limited = agent
        .call("whatsapp_send_text", json!({"to": chat, "text": "x"}))
        .await;
    assert!(wait_of(&limited).is_some(), "{limited:?}");
    clock.advance(60_000);
    ok(agent
        .call("whatsapp_send_text", json!({"to": chat, "text": "x"}))
        .await);
    assert_eq!(world.fake.sent().len(), 21);
}
