//! The ceilings of `docs/13-conversa-entre-hermes.md`, section 6: each one, on the clock of the
//! hub, counted per link and per recipient and never per thread.

use arauto_protocol::errors::ErrorCode;
use serde_json::json;
use uuid::Uuid;

use crate::world::{Agent, HOUR, MINUTE, SECOND, World, ok, refused, retry_after};

/// AR-706, and its wait.
fn stopped(outcome: &arauto_protocol::message::CallOutcome) -> Option<u32> {
    assert_eq!(refused(outcome), Some(ErrorCode::Ar706), "{outcome:?}");
    retry_after(outcome)
}

async fn kept(world: &World) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM peer_message")
        .fetch_one(&world.fixture.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_loop_without_a_thread_stops_at_the_ceiling_of_new_threads() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    world.link(&a, &b, false).await;

    // What an LLM does when it forgets the thread: a new one at every message.
    let mut first = String::new();
    for round in 0..5 {
        world.advance(SECOND);
        let thread = a.thread_with(&b.name, "again").await;
        if round == 0 {
            first = thread;
        }
    }
    let sixth = a.send_to(&b.name, "again").await;
    let wait = stopped(&sixth).expect("the ceiling says when it frees");
    assert!((3_590..=3_600).contains(&wait), "{wait}");
    assert_eq!(kept(&world).await, 5, "a refused message is not kept");

    // In a thread that exists the conversation goes on: the ceiling is of new threads.
    ok(a.send_in(&first, "in the thread").await);

    // An hour after the first one, there is room for one more.
    world.advance(HOUR);
    ok(a.send_to(&b.name, "again").await);
}

#[tokio::test]
async fn thirty_messages_of_a_sender_in_a_minute_whatever_the_recipient() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    let c = world.agent().await;
    world.link(&a, &b, false).await;
    world.link(&a, &c, false).await;

    let to_b = a.thread_with(&b.name, "1").await;
    let to_c = a.thread_with(&c.name, "1").await;
    for _ in 0..14 {
        ok(a.send_in(&to_b, "x").await);
        ok(a.send_in(&to_c, "x").await);
    }
    // Thirty went: fifteen on each link, far from the thirty an hour of a link.
    let wait = stopped(&a.send_in(&to_b, "x").await).expect("a wait");
    assert!((1..=60).contains(&wait), "{wait}");
    assert_eq!(stopped(&a.send_in(&to_c, "x").await), Some(wait));

    world.advance(MINUTE);
    ok(a.send_in(&to_b, "x").await);
}

#[tokio::test]
async fn thirty_messages_of_a_link_in_an_hour_whatever_the_thread() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    let c = world.agent().await;
    world.link(&a, &b, false).await;
    world.link(&a, &c, false).await;
    world.link(&b, &a, false).await;

    let first = a.thread_with(&b.name, "1").await;
    let second = a.thread_with(&b.name, "1").await;
    for _ in 0..13 {
        ok(a.send_in(&first, "x").await);
    }
    // Past the minute of the sender, the hour of the link still counts.
    world.advance(2 * MINUTE);
    for _ in 0..15 {
        ok(a.send_in(&second, "x").await);
    }
    world.advance(2 * MINUTE);
    let wait = stopped(&a.send_in(&first, "x").await).expect("a wait");
    assert!((3_300..=3_400).contains(&wait), "{wait}");
    assert!(stopped(&a.send_in(&second, "x").await).is_some());
    assert!(stopped(&a.send_to(&b.name, "x").await).is_some());

    // The other link of the sender, and the link back, have their own count.
    ok(a.send_to(&c.name, "x").await);
    ok(b.send_in(&first, "x").await);
}

/// Messages `a` sent to `to` long enough ago for the ceilings of the minute and of the hour,
/// written straight into the database: 500 real ones would take 17 recipients.
async fn old_messages(world: &World, from: &Agent, to: &Agent, count: i64, size: i32) {
    let ids: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT a.id, b.id FROM hermes a, hermes b WHERE a.name = $1 AND b.name = $2",
    )
    .bind(&from.name)
    .bind(&to.name)
    .fetch_all(&world.fixture.pool)
    .await
    .unwrap();
    let (from, to) = ids[0];
    let thread = Uuid::new_v4();
    let at = world.now() - 2 * HOUR;
    sqlx::query(
        "INSERT INTO peer_thread (id, organization_id, hermes_a, hermes_b, created_at, \
                                  last_message_at) \
         SELECT $1, organization_id, LEAST($2, $3), GREATEST($2, $3), $4, $4 \
         FROM hermes WHERE id = $2",
    )
    .bind(thread)
    .bind(from)
    .bind(to)
    .bind(at)
    .execute(&world.fixture.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO peer_message (id, organization_id, thread_id, from_hermes_id, \
                                   to_hermes_id, sent_at, size, nonce, text_enc, read_at, \
                                   purge_at) \
         SELECT gen_random_uuid(), h.organization_id, $1, $2, $3, $4 - n, $6, \
                decode(repeat('00', 24), 'hex'), '\\x00', $4, $4 + 86400000 \
         FROM hermes h, generate_series(1, $5) AS n WHERE h.id = $2",
    )
    .bind(thread)
    .bind(from)
    .bind(to)
    .bind(at)
    .bind(count)
    .bind(size)
    .execute(&world.fixture.pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn five_hundred_messages_of_a_sender_in_a_day() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    let c = world.agent().await;
    world.link(&a, &b, false).await;
    old_messages(&world, &a, &c, 499, 10).await;

    ok(a.send_to(&b.name, "the five hundredth").await);
    let wait = stopped(&a.send_to(&b.name, "one more").await).expect("a wait");
    // The oldest one leaves the day in a little less than 22 hours.
    assert!((21 * 3_600..=22 * 3_600).contains(&wait), "{wait}");
}

#[tokio::test]
async fn four_mebibytes_of_a_sender_in_a_day() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    let c = world.agent().await;
    world.link(&a, &b, false).await;
    // 255 messages of 16 KiB: 16 KiB short of the 4 MiB.
    old_messages(&world, &a, &c, 255, 16_384).await;

    let big = "x".repeat(16_384);
    ok(a.send_to(&b.name, &big).await);
    assert!(stopped(&a.send_to(&b.name, "x").await).is_some());
}

#[tokio::test]
async fn two_hundred_unread_messages_of_a_link_fill_the_box_until_the_recipient_reads() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let mut b = world.agent().await;
    world.link(&a, &b, false).await;
    let thread = a.thread_with(&b.name, "1").await;
    // 199 more, unread, written straight into the database with the times of an old day.
    let thread_id = Uuid::try_parse(&thread).unwrap();
    sqlx::query(
        "INSERT INTO peer_message (id, organization_id, thread_id, from_hermes_id, \
                                   to_hermes_id, sent_at, size, nonce, text_enc, purge_at) \
         SELECT gen_random_uuid(), m.organization_id, m.thread_id, m.from_hermes_id, \
                m.to_hermes_id, m.sent_at - 172800000 - n, 1, m.nonce, m.text_enc, m.purge_at \
         FROM peer_message m, generate_series(1, 199) AS n WHERE m.thread_id = $1",
    )
    .bind(thread_id)
    .execute(&world.fixture.pool)
    .await
    .unwrap();

    let full = a.send_in(&thread, "one too many").await;
    assert_eq!(
        stopped(&full),
        None,
        "only the recipient frees it, by reading"
    );

    // The recipient reads one page (the rows written by hand do not open and are skipped,
    // but the one real message is read): there is room again.
    world.advance(2 * MINUTE);
    sqlx::query("UPDATE peer_message SET read_at = $2 WHERE thread_id = $1 AND size = 1")
        .bind(thread_id)
        .bind(world.now())
        .execute(&world.fixture.pool)
        .await
        .unwrap();
    assert_eq!(b.read(&thread).await, ["1"]);
    ok(a.send_in(&thread, "now it fits").await);
}

#[tokio::test]
async fn a_ceiling_is_audited_once_per_link_and_hour_and_never_per_message() {
    let world = World::new().await;
    let mut a = world.agent().await;
    let b = world.agent().await;
    world.link(&a, &b, false).await;
    for _ in 0..5 {
        world.advance(SECOND);
        ok(a.send_to(&b.name, "again").await);
    }
    for _ in 0..4 {
        assert!(stopped(&a.send_to(&b.name, "again").await).is_some());
    }
    let events: Vec<(String, serde_json::Value)> =
        sqlx::query_as("SELECT action, details FROM audit WHERE action LIKE 'peer.%' ORDER BY seq")
            .fetch_all(&world.fixture.pool)
            .await
            .unwrap();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, "peer.ceiling_reached");
    assert_eq!(events[0].1["ceiling"], json!("new_threads"));
    // Nothing of a message is audited: no event per message, and never the text.
    let all: Vec<serde_json::Value> = sqlx::query_scalar("SELECT details FROM audit")
        .fetch_all(&world.fixture.pool)
        .await
        .unwrap();
    assert!(
        all.iter()
            .all(|details| !details.to_string().contains("again"))
    );
}
