//! The deadlines of `docs/14-protocolo-do-canal.md`, section 4, with short values: `hello`,
//! each answer of the connector, the 90 seconds to become a session (apart from the wait for
//! approval), the heartbeat, and a stream that stops in the middle.

use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{Message, Ping, Purpose};
use arauto_testkit::connector::{TestConnector, identity};
use arauto_testkit::hostile;

use crate::support::{self, Behavior, TestHub, TestPurposes};

const PATIENCE: Duration = Duration::from_secs(15);

async fn connect(hub: &TestHub) -> TestConnector {
    TestConnector::connect(hub.addr(), &identity().unwrap(), &hub.hub_key)
        .await
        .unwrap()
}

fn between(after: Duration, low_ms: u64, high_ms: u64) -> bool {
    after >= Duration::from_millis(low_ms) && after < Duration::from_millis(high_ms)
}

#[tokio::test(flavor = "multi_thread")]
async fn hello_comes_within_its_deadline_or_the_hub_closes_with_ar204() {
    let mut settings = support::settings();
    settings.deadlines.hello_ms = 400;
    let hub = support::start(settings, TestPurposes(Behavior::Park)).await;
    let mut connector = connect(&hub).await;
    let ended = connector.ended(PATIENCE).await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));
    assert!(between(ended.after, 350, 4_000), "{ended:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn each_answer_of_the_connector_has_its_deadline() {
    let mut settings = support::settings();
    settings.deadlines.connector_reply_ms = 400;
    let hub = support::start(settings, TestPurposes(Behavior::Park)).await;
    let mut connector = connect(&hub).await;
    connector.hello(Purpose::Leave).await.unwrap();
    let (ended, took) = support::timed(connector.ended(PATIENCE)).await;
    assert_eq!(ended.unwrap().closing, Some(ErrorCode::Ar204));
    assert!(between(took, 350, 4_000), "{took:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_that_does_not_become_a_session_is_closed() {
    let mut settings = support::settings();
    settings.deadlines.become_session_ms = 800;
    settings.deadlines.ping_interval_ms = 150;
    let hub = support::start(settings, TestPurposes(Behavior::Park)).await;
    let mut connector = connect(&hub).await;
    connector.hello(Purpose::Channel).await.unwrap();
    // The connector answers every ping: the connection is alive, only late.
    let ended = connector.ended(PATIENCE).await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));
    assert!(between(ended.after, 750, 4_000), "{ended:?}");
}

/// The 90 seconds stop while the device waits for the admin and go on from where they were.
#[tokio::test(flavor = "multi_thread")]
async fn the_wait_for_approval_does_not_count_toward_becoming_a_session() {
    let mut settings = support::settings();
    settings.deadlines.become_session_ms = 800;
    settings.deadlines.ping_interval_ms = 150;
    let hub = support::start(settings, TestPurposes(Behavior::Wait { ms: 1_200 })).await;
    let mut connector = connect(&hub).await;
    connector.hello(Purpose::Channel).await.unwrap();
    let ended = connector.ended(PATIENCE).await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));
    // About 800 ms before the session in all, plus the 1,200 of the wait.
    assert!(between(ended.after, 1_850, 6_000), "{ended:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn ninety_seconds_without_a_frame_close_and_any_frame_counts_as_life() {
    let mut settings = support::settings();
    settings.deadlines.silence_ms = 600;
    let hub = support::start(settings, TestPurposes(Behavior::Wait { ms: 60_000 })).await;
    // Quiet after `hello_ok`: closed.
    let mut quiet = connect(&hub).await;
    quiet.hello(Purpose::Channel).await.unwrap();
    let (ended, took) = support::timed(quiet.ended(PATIENCE)).await;
    assert_eq!(ended.unwrap().closing, Some(ErrorCode::Ar204));
    assert!(between(took, 450, 4_000), "{took:?}");
    // A ping every 200 ms keeps the connection.
    let mut alive = connect(&hub).await;
    alive.hello(Purpose::Channel).await.unwrap();
    for n in 1..=8 {
        alive.send(&Message::Ping(Ping { n })).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    alive.send(&Message::Ping(Ping { n: 9 })).await.unwrap();
    loop {
        match alive.recv_any(PATIENCE).await.unwrap() {
            Message::Pong(pong) if pong.n == 9 => break,
            Message::Pong(_) => {}
            other => panic!("{:?}", other.kind()),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_hub_pings_from_hello_ok_on() {
    let mut settings = support::settings();
    settings.deadlines.ping_interval_ms = 100;
    settings.deadlines.hello_ms = 2_000;
    let hub = support::start(settings, TestPurposes(Behavior::Wait { ms: 60_000 })).await;
    let mut connector = connect(&hub).await;
    // Before `hello`, nothing comes.
    assert!(matches!(
        connector.recv_any(Duration::from_millis(400)).await,
        Err(arauto_testkit::KitError::TimedOut)
    ));
    connector.hello(Purpose::Channel).await.unwrap();
    let mut numbers = Vec::new();
    while numbers.len() < 3 {
        if let Message::Ping(Ping { n }) = connector.recv_any(PATIENCE).await.unwrap() {
            numbers.push(n);
        }
    }
    assert_eq!(numbers, [1, 2, 3]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_that_stops_in_the_middle_closes_the_connection() {
    let mut settings = support::settings();
    settings.deadlines.stream_idle_ms = 500;
    let hub = support::start(settings, TestPurposes(Behavior::Session)).await;
    let mut connector = connect(&hub).await;
    connector.hello(Purpose::Channel).await.unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::Session(_)
    ));
    let frames = connector
        .frames_of(&hostile::big_call(70_000).unwrap())
        .unwrap();
    connector.send_frame(&frames[0]).await.unwrap();
    let (ended, took) = support::timed(connector.ended(PATIENCE)).await;
    assert_eq!(ended.unwrap().closing, Some(ErrorCode::Ar204));
    assert!(between(took, 400, 4_000), "{took:?}");
}
