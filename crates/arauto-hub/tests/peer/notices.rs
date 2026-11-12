//! The queue of notices of new messages (`docs/13-conversa-entre-hermes.md`, section 5;
//! `docs/10-maquinas-de-estado.md`, section 5).

use arauto_protocol::notice::AckOutcome;

use crate::world::{HOUR, MINUTE, SECOND, World, ok, peer_notice};

const CANARY: &str = "CANARY-text-of-a-message-7f3a";

/// A and B in session, B may be woken, and the link from A to B wakes.
async fn woken() -> (World, crate::world::Agent, crate::world::Agent) {
    let world = World::new().await;
    let a = world.agent().await;
    let b = world.agent().await;
    world.may_be_woken(&b, true).await;
    world.link(&a, &b, true).await;
    (world, a, b)
}

fn state(states: &[(String, i32)]) -> Vec<(&str, i32)> {
    states
        .iter()
        .map(|(state, count)| (state.as_str(), *count))
        .collect()
}

#[tokio::test]
async fn a_notice_is_a_pointer_and_carries_no_text_and_no_thread() {
    let (world, mut a, mut b) = woken().await;
    let thread = a.thread_with(&b.name, CANARY).await;

    let notice = b.next_notice().await;
    assert_eq!(peer_notice(&notice), (a.name.clone(), 1));
    let travelled = serde_json::to_string(&notice).unwrap();
    assert!(!travelled.contains(CANARY), "{travelled}");
    assert!(!travelled.contains(&thread), "{travelled}");
    assert_eq!(state(&world.notice_states(&a, &b).await), [("sent", 1)]);

    b.acknowledge(&notice, AckOutcome::Accepted).await;
    // The answer of a call comes after the hub handled the `notice_ack` before it.
    assert_eq!(b.read(&thread).await, [CANARY]);
    assert_eq!(
        state(&world.notice_states(&a, &b).await),
        [("confirmed", 1)]
    );
    assert_eq!(
        b.notices_while_quiet().await,
        0,
        "confirmed: it never leaves again"
    );
}

#[tokio::test]
async fn there_is_no_notice_without_the_wake_mark_or_for_a_hermes_that_may_not_be_woken() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    let mut c = world.agent().await;
    // The link wakes, and B may not be woken; C may be woken, and its link does not wake.
    world.link(&a, &b, true).await;
    world.may_be_woken(&c, true).await;
    world.link(&a, &c, false).await;

    ok(a.send_to(&b.name, "kept").await);
    ok(a.send_to(&c.name, "kept").await);
    assert_eq!(b.notices_while_quiet().await, 0);
    assert_eq!(c.notices_while_quiet().await, 0);
    assert!(world.notice_states(&a, &b).await.is_empty());
    assert!(world.notice_states(&a, &c).await.is_empty());
}

#[tokio::test]
async fn one_pending_notice_per_sender_grows_and_the_next_leaves_ten_seconds_later() {
    let (world, mut a, mut b) = woken().await;
    let thread = a.thread_with(&b.name, "one").await;
    let first = b.next_notice().await;
    assert_eq!(peer_notice(&first).1, 1);
    b.acknowledge(&first, AckOutcome::Accepted).await;

    // Three more before the ten seconds: one notice waits, and grows.
    for text in ["two", "three", "four"] {
        world.advance(SECOND);
        ok(a.send_in(&thread, text).await);
    }
    assert_eq!(b.notices_while_quiet().await, 0, "not before ten seconds");
    assert_eq!(
        state(&world.notice_states(&a, &b).await),
        [("confirmed", 1), ("pending", 3)]
    );

    world.advance(10 * SECOND);
    let second = b.next_notice().await;
    assert_eq!(peer_notice(&second), (a.name.clone(), 3));
    assert_ne!(second.id, first.id);
}

#[tokio::test]
async fn a_notice_without_answer_leaves_again_and_each_answer_has_its_wait() {
    let (world, mut a, mut b) = woken().await;
    ok(a.send_to(&b.name, "wake up").await);
    let notice = b.next_notice().await;

    // No answer: it leaves again when its lease of a minute ends, the same notice.
    world.advance(30 * SECOND);
    assert_eq!(b.notices_while_quiet().await, 0);
    world.advance(31 * SECOND);
    let again = b.next_notice().await;
    assert_eq!(again.id, notice.id);

    // The local API failed: again, after a wait that grows (10 s at the second attempt).
    b.acknowledge(&again, AckOutcome::Failed).await;
    world.advance(9 * SECOND);
    assert_eq!(b.notices_while_quiet().await, 0);
    world.advance(2 * SECOND);
    let third = b.next_notice().await;
    assert_eq!(third.id, notice.id);

    // The connector reached its own ceiling: in five minutes.
    b.acknowledge(&third, AckOutcome::Throttled).await;
    world.advance(4 * MINUTE);
    assert_eq!(b.notices_while_quiet().await, 0);
    world.advance(MINUTE + SECOND);
    let fourth = b.next_notice().await;
    assert_eq!(fourth.id, notice.id);

    // Out of format for the connector: discarded, and never again.
    b.acknowledge(&fourth, AckOutcome::Rejected).await;
    world.advance(HOUR);
    assert_eq!(b.notices_while_quiet().await, 0);
    assert_eq!(
        state(&world.notice_states(&a, &b).await),
        [("discarded", 1)]
    );
}

#[tokio::test]
async fn a_notice_is_judged_again_when_it_leaves() {
    let (world, mut a, mut b) = woken().await;
    let thread = a.thread_with(&b.name, "one").await;
    let first = b.next_notice().await;
    b.acknowledge(&first, AckOutcome::Accepted).await;

    // A second one waits its ten seconds; meanwhile B may no longer be woken.
    world.advance(SECOND);
    ok(a.send_in(&thread, "two").await);
    world.may_be_woken(&b, false).await;
    world.advance(10 * SECOND);
    assert_eq!(b.notices_while_quiet().await, 0);
    assert_eq!(
        state(&world.notice_states(&a, &b).await),
        [("confirmed", 1), ("discarded", 1)]
    );

    // And the link that loses its wake mark, or goes, takes its notices with it at once:
    // one that is out and unanswered never leaves again.
    world.may_be_woken(&b, true).await;
    world.advance(SECOND);
    ok(a.send_in(&thread, "three").await);
    let third = b.next_notice().await;
    assert_ne!(third.id, first.id);
    world.link(&a, &b, false).await;
    world.advance(2 * MINUTE);
    assert_eq!(b.notices_while_quiet().await, 0);
    let states = world.notice_states(&a, &b).await;
    assert_eq!(states.last().unwrap().0, "discarded", "{states:?}");
    // The messages themselves were kept, and are read.
    assert_eq!(b.read(&thread).await, ["one", "two", "three"]);
}

#[tokio::test]
async fn ten_notices_of_a_link_in_an_hour_and_then_the_message_is_kept_without_one() {
    let (world, mut a, mut b) = woken().await;
    let thread = a.thread_with(&b.name, "1").await;
    let first = b.next_notice().await;
    b.acknowledge(&first, AckOutcome::Accepted).await;
    for _ in 1..10 {
        world.advance(11 * SECOND);
        ok(a.send_in(&thread, "x").await);
        let notice = b.next_notice().await;
        b.acknowledge(&notice, AckOutcome::Accepted).await;
    }
    // The eleventh message of the hour is kept, and wakes nobody: dropped, never put off.
    world.advance(11 * SECOND);
    ok(a.send_in(&thread, "eleventh").await);
    assert_eq!(b.notices_while_quiet().await, 0);
    world.advance(2 * HOUR);
    assert_eq!(
        b.notices_while_quiet().await,
        0,
        "it does not start again by itself"
    );
    assert_eq!(world.notice_states(&a, &b).await.len(), 10);

    // A new message, past the hour, wakes again.
    ok(a.send_in(&thread, "later").await);
    assert_eq!(peer_notice(&b.next_notice().await).1, 1);
}

#[tokio::test]
async fn a_notice_expires_after_a_day_in_the_queue() {
    let (world, mut a, mut b) = woken().await;
    ok(a.send_to(&b.name, "wake up").await);
    let notice = b.next_notice().await;
    b.acknowledge(&notice, AckOutcome::Throttled).await;
    // The 24-hour swap of the credential is not what this test looks at: just under a day,
    // in one step, with the notice still waiting its five minutes at the start.
    world.advance(24 * HOUR - MINUTE);
    let late = b.next_notice().await;
    b.acknowledge(&late, AckOutcome::Throttled).await;
    world.advance(2 * MINUTE);
    assert_eq!(b.notices_while_quiet().await, 0);
    assert_eq!(state(&world.notice_states(&a, &b).await), [("expired", 1)]);
}
