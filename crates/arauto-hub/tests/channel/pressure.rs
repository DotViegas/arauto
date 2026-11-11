//! Backpressure and memory (`docs/15-mvp.md`, section 3; `docs/14-protocolo-do-canal.md`,
//! section 4): a connector that does not read slows down only whoever writes to it, until a
//! frame takes the write deadline or the control queue fills; the budget of the whole hub for
//! messages under reassembly closes with AR-205 the connection that asks for more, and comes
//! back when a connection ends.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{Message, Ping, Pong, Purpose};
use arauto_testkit::connector::{TestConnector, identity};
use arauto_testkit::hostile;

use crate::support::{self, Behavior, TestHub, TestPurposes};

const PATIENCE: Duration = Duration::from_secs(20);
const MIB: usize = 1024 * 1024;

async fn in_session(hub: &TestHub) -> TestConnector {
    let mut connector = TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap();
    connector.hello(Purpose::Channel).await.unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::Session(_)
    ));
    connector
}

fn flood(count: usize) -> (TestPurposes, Arc<AtomicBool>) {
    let blocked = Arc::new(AtomicBool::new(false));
    (
        TestPurposes(Behavior::Flood {
            count,
            size: MIB,
            blocked: Arc::clone(&blocked),
        }),
        blocked,
    )
}

/// A connector that stops reading for less than the write deadline loses nothing: the hub
/// waited for room instead of closing.
#[tokio::test(flavor = "multi_thread")]
async fn a_connector_that_does_not_read_is_not_expelled_before_the_deadline() {
    let mut settings = support::settings();
    settings.write_stall_ms = 4_000;
    let (purposes, blocked) = flood(48);
    let hub = support::start(settings, purposes).await;
    let mut connector = in_session(&hub).await;
    tokio::time::sleep(Duration::from_millis(2_000)).await;
    assert!(
        blocked.load(Ordering::SeqCst),
        "the flood never had to wait: the test did not fill the network"
    );
    assert_eq!(hub.hub().links().len(), 1);
    let mut results = 0;
    loop {
        match connector.recv(PATIENCE).await.unwrap() {
            Message::Result(_) => results += 1,
            Message::Tools(_) => break,
            other => panic!("{:?}", other.kind()),
        }
    }
    assert_eq!(results, 48);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_frame_that_does_not_leave_within_the_deadline_closes_the_connection() {
    let mut settings = support::settings();
    settings.write_stall_ms = 2_000;
    let (purposes, blocked) = flood(10_000);
    let hub = support::start(settings, purposes).await;
    let _connector = in_session(&hub).await;
    // Not read at all from here on.
    assert!(support::eventually(PATIENCE, || blocked.load(Ordering::SeqCst)).await);
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        hub.hub().links().len(),
        1,
        "closed before the write deadline"
    );
    assert!(hub.links_become(0, PATIENCE).await);
}

/// The writer is stuck on a connector that does not read; the `pong`s for its `ping`s pile up
/// in the control queue, and the 17th closes the connection long before the write deadline.
#[tokio::test(flavor = "multi_thread")]
async fn a_full_control_queue_closes_the_connection() {
    let mut settings = support::settings();
    settings.write_stall_ms = 120_000;
    let (purposes, blocked) = flood(10_000);
    let hub = support::start(settings, purposes).await;
    let mut connector = in_session(&hub).await;
    assert!(support::eventually(PATIENCE, || blocked.load(Ordering::SeqCst)).await);
    for n in 1..=40 {
        if connector.send(&Message::Ping(Ping { n })).await.is_err() {
            break;
        }
    }
    let (closed, took) = support::timed(hub.links_become(0, PATIENCE)).await;
    assert!(closed);
    assert!(took < Duration::from_secs(10), "{took:?}");
}

/// The budget is of the whole hub: one connection holding part of it leaves the other short,
/// and the room comes back when the first one ends.
#[tokio::test(flavor = "multi_thread")]
async fn the_reassembly_budget_of_the_hub_closes_with_ar205_who_asks_for_more() {
    let mut settings = support::settings();
    settings.reassembly_budget_bytes = 150_000;
    let hub = support::start(settings, TestPurposes(Behavior::Session)).await;
    let big = hostile::big_call(200_000).unwrap();

    // The first connection keeps 120,000 bytes under reassembly.
    let mut first = in_session(&hub).await;
    let frames = first.frames_of(&big).unwrap();
    first.send_frame(&frames[0]).await.unwrap();
    first.send_frame(&frames[1]).await.unwrap();
    // Let the hub take them before the second connection asks.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // The second one asks for 60,000 more than there is.
    let mut second = in_session(&hub).await;
    let frames = second.frames_of(&big).unwrap();
    second.send_frame(&frames[0]).await.unwrap();
    let ended = second.ended(PATIENCE).await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar205));

    // The first one is still there; once it ends, its bytes are free.
    assert!(hub.links_become(1, PATIENCE).await);
    drop(first);
    assert!(hub.links_become(0, PATIENCE).await);
    let mut third = in_session(&hub).await;
    third
        .send(&hostile::big_call(100_000).unwrap())
        .await
        .unwrap();
    assert!(matches!(
        third.recv(PATIENCE).await.unwrap(),
        Message::Tools(_)
    ));
}

/// A message above the size read inline goes through the blocking reader and arrives the
/// same.
#[tokio::test(flavor = "multi_thread")]
async fn a_large_message_is_read_away_from_the_network_tasks_and_arrives_whole() {
    let mut settings = support::settings();
    settings.inline_decode_max_bytes = 1_000;
    let hub = support::start(settings, TestPurposes(Behavior::Session)).await;
    let mut connector = in_session(&hub).await;
    connector
        .send(&hostile::big_call(300 * 1024).unwrap())
        .await
        .unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::Tools(_)
    ));
}

/// `docs/15-mvp.md`, section 3: the writer looks at the control queue before every frame of
/// data. With the data queue full (here 256 frames, 128 results) and refilled without end,
/// the `pong` of a `ping` comes ahead of what was queued: only what the system already held
/// on its way comes before it.
#[tokio::test(flavor = "multi_thread")]
async fn a_control_frame_passes_ahead_of_the_data_queue() {
    const RESULTS: usize = 400;
    const QUEUED_RESULTS: usize = 128;
    let mut settings = support::settings();
    settings.data_queue_frames = QUEUED_RESULTS * 2;
    let blocked = Arc::new(AtomicBool::new(false));
    let purposes = TestPurposes(Behavior::Flood {
        count: RESULTS,
        size: 64 * 1024,
        blocked: Arc::clone(&blocked),
    });
    let hub = support::start(settings, purposes).await;
    let mut connector = in_session(&hub).await;
    // Not read until the data queue is full and the flood waits for room.
    assert!(support::eventually(PATIENCE, || blocked.load(Ordering::SeqCst)).await);
    connector
        .send(&Message::Ping(Ping { n: 777 }))
        .await
        .unwrap();
    let mut ahead = 0;
    loop {
        match connector.recv_any(PATIENCE).await.unwrap() {
            Message::Pong(Pong { n: 777 }) => break,
            Message::Result(_) => ahead += 1,
            Message::Ping(_) => {}
            other => panic!("{:?} before the pong, after {ahead} results", other.kind()),
        }
    }
    assert!(
        ahead < QUEUED_RESULTS / 2,
        "{ahead} results came before the pong"
    );
}
