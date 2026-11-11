//! The return in mass (`docs/15-mvp.md`, section 3): the hub restarts and every connector
//! comes back at once. The ceiling of connections without a session holds the first wave;
//! whoever is refused comes back after a random wait. No device goes to quarantine.
//!
//! A small round runs in the suite. The one with 500 connectors (return in mass, heartbeat
//! and local reads) is marked `#[ignore]`: run it with
//! `cargo test -p arauto-hub --test channel -- --ignored --nocapture five_hundred`.

use std::time::Duration;

use arauto_hub::db::PoolOptions;
use arauto_testkit::device::{Device, SessionClient};
use arauto_testkit::load::{self, Backoff};
use futures_util::StreamExt;

use crate::fixture::{Fixture, settings};

/// Brings `count` devices to a session, a few at a time.
async fn connect_many(fixture: &Fixture, count: usize) -> Vec<SessionClient> {
    futures_util::stream::iter(0..count)
        .map(|_| async { fixture.connected().await.1 })
        .buffer_unordered(6)
        .collect()
        .await
}

async fn quarantined(fixture: &Fixture) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM device WHERE state = 'quarantined'")
        .fetch_one(&fixture.pool)
        .await
        .unwrap()
}

/// Stops the hub with every session open, starts it again, and brings every device back.
async fn restart_and_come_back(
    fixture: &mut Fixture,
    sessions: Vec<SessionClient>,
    backoff: Backoff,
) -> load::Comeback {
    let devices: Vec<Device> = sessions
        .iter()
        .map(|session| session.device().clone())
        .collect();
    fixture.stop().await;
    drop(sessions);
    fixture.start().await;
    let addr = fixture.addr();
    let devices = devices.into_iter().map(|device| device.at(addr)).collect();
    load::come_back(devices, Duration::from_secs(120), backoff, 0x00ba_c0e5).await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_return_in_mass_after_a_restart_brings_every_device_back_without_quarantine() {
    const COUNT: usize = 32;
    let mut fixture = Fixture::new(settings()).await;
    let sessions = connect_many(&fixture, COUNT).await;
    let comeback = restart_and_come_back(
        &mut fixture,
        sessions,
        Backoff {
            first_ms: 50,
            max_ms: 1_000,
        },
    )
    .await;
    assert!(comeback.failures.is_empty(), "{:?}", comeback.failures);
    assert_eq!(comeback.sessions.len(), COUNT);
    assert_eq!(quarantined(&fixture).await, 0);
    assert_eq!(fixture.hub().sessions_by_device().len(), COUNT);
}

/// The reduced load test of the first milestone: 500 connectors, each with its identity.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "about 500 connectors: run it on purpose and read the numbers it prints"]
async fn five_hundred_connectors_come_back_keep_the_heartbeat_and_read_locally() {
    const COUNT: usize = 500;
    let mut chosen = settings();
    chosen.pool = PoolOptions::default();
    chosen.deadlines.ping_interval_ms = 1_000;
    chosen.sweep_interval_ms = arauto_hub::channel::settings::SWEEP_INTERVAL_MS;
    let mut fixture = Fixture::new(chosen).await;
    let enrolled = tokio::time::Instant::now();
    let sessions = connect_many(&fixture, COUNT).await;
    println!(
        "{COUNT} devices enrolled, approved and in session in {:?}",
        enrolled.elapsed()
    );
    let comeback = restart_and_come_back(&mut fixture, sessions, Backoff::CONNECTOR).await;
    println!(
        "return in mass: {} sessions in {:?}, {} attempts refused and tried again, {} failures",
        comeback.sessions.len(),
        comeback.elapsed,
        comeback.retries,
        comeback.failures.len()
    );
    assert!(comeback.failures.is_empty(), "{:?}", comeback.failures);
    assert_eq!(comeback.sessions.len(), COUNT);
    // The heartbeat runs every second meanwhile; each one reads locally every second.
    let reads = load::local_reads(comeback.sessions, 10, Duration::from_secs(1), 0x0f_ace).await;
    println!(
        "arauto_status: {} calls, p50 {:?}, p95 {:?}, p99 {:?}, max {:?}; {} failures",
        reads.latencies.count,
        reads.latencies.p50,
        reads.latencies.p95,
        reads.latencies.p99,
        reads.latencies.max,
        reads.failures.len()
    );
    assert!(reads.failures.is_empty(), "{:?}", reads.failures);
    let quarantined = quarantined(&fixture).await;
    println!("quarantined devices: {quarantined}");
    assert_eq!(quarantined, 0);
}
