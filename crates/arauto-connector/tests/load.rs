//! The reduced load of the first milestone with the connector of the project (`docs/15-mvp.md`,
//! sections 3 and 7): many connectors of the library, each a leader with a folder of its own,
//! against the hub running in this process. The hub restarts and every connector comes back to
//! its session by itself, with the waits of its own reconnection, and no device goes to
//! quarantine.
//!
//! A round of 32 runs in the suite, with the short waits of the tests. The one of 500, with the
//! waits the program ships with (the return in mass, the heartbeat of the hub every second, and
//! `arauto_status` through each connector), is marked `#[ignore]`: run it with
//! `cargo test -p arauto-connector --test load -- --ignored --nocapture five_hundred`.
//!
//! A binary of its own: a round takes seconds of the real clock.
//!
//! These tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_connector::testing::Event;
use arauto_connector::{ChannelStatus, Connector, Hermes, Home, JoinArgs, Joined, Settings};
use arauto_hub::channel::ChannelSettings;
use arauto_hub::db::PoolOptions;
use arauto_hub::repo::approval::ApproveOutcome;
use arauto_protocol::clock::{Clock, SeededRng, SystemClock};
use arauto_protocol::message::CallOutcome;
use arauto_testkit::hub::fixture::{self, Fixture};
use arauto_testkit::load::Latencies;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::time::Instant;
use zeroize::Zeroizing;

/// The patience of a round for what does not depend on its size.
const PATIENCE: Duration = Duration::from_secs(60);

/// How many joins and approvals run at once while the round is prepared.
const PREPARED_AT_ONCE: usize = 8;

/// What the connectors of a round did, from their events.
#[derive(Debug, Default, Clone)]
struct Seen {
    /// Attempts of the channel begun.
    attempts: u64,
    /// Sessions begun.
    sessions: u64,
    /// How the attempts ended, by the code of the `closing` of the hub ("none" without one).
    endings: BTreeMap<String, u64>,
    /// Each wait before a new attempt, in milliseconds, in the order they began.
    waits: Vec<u64>,
    /// A device whose channel is no longer used, and why: never, in a round.
    stopped: Vec<String>,
}

/// Keeps the events of every connector of a round.
fn watch(mut events: UnboundedReceiver<Event>) -> Arc<Mutex<Seen>> {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let kept = Arc::clone(&seen);
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            let mut seen = kept.lock().unwrap();
            match event {
                Event::AttemptStarted { .. } => seen.attempts += 1,
                Event::AttemptEnded { closing, .. } => {
                    let code = closing.map_or("none", |code| code.as_str());
                    *seen.endings.entry(code.to_owned()).or_default() += 1;
                }
                Event::Session { .. } => seen.sessions += 1,
                Event::Waiting { ms, .. } => seen.waits.push(ms),
                Event::Stopped { device, code } => seen.stopped.push(format!("{device}: {code}")),
                _ => {}
            }
        }
    });
    seen
}

/// One connector of the round: its folder, and the connector running on it.
struct One {
    _dir: tempfile::TempDir,
    home: Home,
    connector: Option<Connector>,
}

/// A new Hermes: its connector joins with a folder of its own, and the admin approves it with
/// the code it showed.
async fn joined(fixture: &Fixture, settings: &Settings, program: &Path) -> One {
    let name = fixture.hermes().await;
    let invite = fixture.invite(&name).await;
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(&dir.path().join("arauto")).unwrap();
    let args = JoinArgs {
        hub: fixture.channel_url(),
        key: fixture.hub_key_text(),
        invite: Zeroizing::new(invite.reveal().to_string()),
        register: true,
        program: program.to_path_buf(),
    };
    let joined = arauto_connector::join(
        &home,
        args,
        &Hermes::default(),
        settings,
        SystemClock.now_ms(),
    )
    .await;
    let Ok(Joined::Enrolled { code, .. }) = joined else {
        panic!("the join did not enroll: {joined:?}");
    };
    let approved = fixture.approve(&name, &code.to_string()).await;
    assert!(
        matches!(approved, ApproveOutcome::Approved(_)),
        "{approved:?}"
    );
    One {
        _dir: dir,
        home,
        connector: None,
    }
}

/// `count` connectors joined and approved, none of them running; and the folder of the program
/// they copied, to keep while they run.
async fn prepare(
    fixture: &Fixture,
    settings: &Settings,
    count: usize,
) -> (Vec<One>, tempfile::TempDir) {
    let download = tempfile::tempdir().unwrap();
    let program = download.path().join("arauto-connector-download");
    std::fs::write(&program, b"a program for the tests").unwrap();
    let ones = futures_util::stream::iter(0..count)
        .map(|_| joined(fixture, settings, &program))
        .buffer_unordered(PREPARED_AT_ONCE)
        .collect()
        .await;
    (ones, download)
}

/// Starts the connector of each folder: each one is the leader of its own.
async fn start(ones: &mut [One], settings: &Settings) {
    let started: Vec<Connector> = futures_util::stream::iter(ones.iter())
        .map(|one| Connector::start(one.home.clone(), settings.clone()))
        .buffered(16)
        .map(Result::unwrap)
        .collect()
        .await;
    for (one, connector) in ones.iter_mut().zip(started) {
        one.connector = Some(connector);
    }
}

fn connector(one: &One) -> &Connector {
    one.connector.as_ref().expect("the connector runs")
}

/// Whether the device in use of this connector is in session.
async fn in_session(connector: &Connector) -> bool {
    matches!(
        connector.status().await.device.map(|device| device.channel),
        Some(ChannelStatus::Session { .. })
    )
}

async fn sessions(ones: &[One]) -> usize {
    let mut count = 0;
    for one in ones {
        count += usize::from(in_session(connector(one)).await);
    }
    count
}

/// Waits until `wanted` of the connectors are in session, and says how long it took.
async fn until_sessions(
    ones: &[One],
    wanted: usize,
    within: Duration,
    seen: &Mutex<Seen>,
) -> Duration {
    let started = Instant::now();
    loop {
        let now = sessions(ones).await;
        if now == wanted {
            return started.elapsed();
        }
        let stopped = seen.lock().unwrap().stopped.clone();
        assert!(
            stopped.is_empty(),
            "a channel is no longer used: {stopped:?}"
        );
        assert!(
            started.elapsed() < within,
            "{now} of {wanted} in session after {:?}",
            started.elapsed()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// One `arauto_status` through the connector: it says connected, with the name of its Hermes.
async fn status_answers(connector: &Connector) -> bool {
    match connector
        .call("arauto_status", serde_json::Map::new())
        .await
    {
        CallOutcome::Ok(content) => content.as_map().get("connected") == Some(&Value::Bool(true)),
        CallOutcome::Refused { .. } => false,
    }
}

async fn devices_in(fixture: &Fixture, state: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM device WHERE state = $1")
        .bind(state)
        .fetch_one(&fixture.pool)
        .await
        .unwrap()
}

/// Stops the hub with every session open, and starts it again on the same port once every
/// connector saw its session end. Returns how long every connector took to be back in session,
/// counted from the new start.
async fn restart(
    fixture: &mut Fixture,
    ones: &[One],
    within: Duration,
    seen: &Mutex<Seen>,
) -> Duration {
    let addr = fixture.addr();
    fixture.stop().await;
    until_sessions(ones, 0, PATIENCE, seen).await;
    fixture.start_at(addr).await;
    until_sessions(ones, ones.len(), within, seen).await
}

async fn shutdown(ones: &mut [One]) {
    let stopping: Vec<_> = ones
        .iter_mut()
        .filter_map(|one| one.connector.take())
        .map(Connector::shutdown)
        .collect();
    futures_util::future::join_all(stopping).await;
}

/// The settings of the connectors of a round: `base`, the command of Hermes answered by the
/// test, and every event to `seen`.
fn connectors(base: Settings) -> (Settings, Arc<Mutex<Seen>>) {
    let (events, received) = unbounded_channel();
    let settings = base.with_hermes(|_| true).with_events(events);
    (settings, watch(received))
}

#[tokio::test(flavor = "multi_thread")]
async fn thirty_two_connectors_come_back_to_their_sessions_after_the_hub_restarts() {
    const COUNT: usize = 32;
    let mut fixture = Fixture::new(fixture::settings()).await;
    let (settings, seen) = connectors(Settings::for_tests());
    let (mut ones, _program) = prepare(&fixture, &settings, COUNT).await;
    start(&mut ones, &settings).await;
    until_sessions(&ones, COUNT, PATIENCE, &seen).await;

    let took = restart(&mut fixture, &ones, PATIENCE, &seen).await;
    // Every connector serves its calls again, through its own session.
    for one in &ones {
        assert!(status_answers(connector(one)).await, "took {took:?}");
    }
    assert_eq!(fixture.hub().sessions_by_device().len(), COUNT);
    assert_eq!(devices_in(&fixture, "quarantined").await, 0);
    assert_eq!(
        devices_in(&fixture, "active").await,
        i64::try_from(COUNT).unwrap()
    );
    let seen = seen.lock().unwrap().clone();
    assert!(seen.stopped.is_empty(), "{seen:?}");
    // The first sessions and the ones after the restart.
    assert!(seen.sessions >= 2 * COUNT as u64, "{seen:?}");
    shutdown(&mut ones).await;
}

/// The process memory, in bytes: private bytes on Windows, the resident set on Linux. `None`
/// when the system does not say.
fn process_memory() -> Option<u64> {
    if cfg!(windows) {
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "(Get-Process -Id {}).PrivateMemorySize64",
                    std::process::id()
                ),
            ])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    } else {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
        let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        kib.checked_mul(1024)
    }
}

/// The reduced load test of the first milestone with the connector of the project: 500
/// connectors, each with its folder and identity, with the waits the program ships with.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "500 connectors: run it on purpose and read the numbers it prints"]
async fn five_hundred_connectors_come_back_keep_the_heartbeat_and_read_through_the_connector() {
    const COUNT: usize = 500;
    const READS: usize = 10;
    let mut chosen: ChannelSettings = fixture::settings();
    chosen.pool = PoolOptions::default();
    chosen.deadlines.ping_interval_ms = 1_000;
    chosen.sweep_interval_ms = arauto_hub::channel::settings::SWEEP_INTERVAL_MS;
    let mut fixture = Fixture::new(chosen).await;
    let (settings, seen) = connectors(Settings::default());

    let prepared = Instant::now();
    let (mut ones, _program) = prepare(&fixture, &settings, COUNT).await;
    println!(
        "{COUNT} connectors joined and approved in {:?}",
        prepared.elapsed()
    );
    let before = process_memory();

    let started = Instant::now();
    start(&mut ones, &settings).await;
    let first = until_sessions(&ones, COUNT, Duration::from_secs(600), &seen).await;
    println!(
        "first sessions: {COUNT} in {:?} from the start of the connectors ({:?} to start them)",
        started.elapsed(),
        started.elapsed().saturating_sub(first)
    );
    // Parked sessions: the heartbeat of the hub every second, and nothing else.
    tokio::time::sleep(Duration::from_secs(5)).await;
    let parked = process_memory();
    if let (Some(before), Some(parked)) = (before, parked) {
        println!(
            "memory of the process: {before} bytes before the connectors, {parked} with \
             {COUNT} sessions parked: {} bytes per session, hub and connector together",
            parked.saturating_sub(before) / COUNT as u64
        );
    } else {
        println!("memory of the process: not measured on this system");
    }

    let attempts = seen.lock().unwrap().attempts;
    let endings = seen.lock().unwrap().endings.clone();
    let waits = seen.lock().unwrap().waits.len();
    let stopping = Instant::now();
    let took = restart(&mut fixture, &ones, Duration::from_secs(600), &seen).await;
    let from_the_stop = stopping.elapsed();
    let after = seen.lock().unwrap().clone();
    let mut endings_now = after.endings.clone();
    for (code, count) in &endings {
        if let Some(now) = endings_now.get_mut(code) {
            *now -= count;
        }
    }
    // The waits drawn after the `closing` of the stop (AR-205: from 1 to 10 seconds) and after
    // the refusals of the return.
    let waits_now: Vec<u64> = after.waits.iter().skip(waits).copied().collect();
    println!(
        "return in mass: {COUNT} sessions in {took:?} from the new start ({from_the_stop:?} \
         from the stop); {} attempts of the channel since the stop; endings since the stop: \
         {endings_now:?}; {} waits since the stop, the longest {} ms",
        after.attempts - attempts,
        waits_now.len(),
        waits_now.iter().max().copied().unwrap_or(0)
    );
    assert!(after.stopped.is_empty(), "{after:?}");

    // Each connector reads through its own session, once a second, from a moment of the first
    // second drawn from a seed.
    let reading = Instant::now();
    let reads: Vec<Vec<Duration>> = futures_util::stream::iter(ones.iter().enumerate())
        .map(|(index, one)| async move {
            let mut rng = SeededRng::new(0x0f_ace + index as u64);
            tokio::time::sleep(Duration::from_millis(rng.below(1_000))).await;
            let mut measured = Vec::with_capacity(READS);
            for _ in 0..READS {
                let asked = Instant::now();
                assert!(status_answers(connector(one)).await, "a read was refused");
                measured.push(asked.elapsed());
                tokio::time::sleep(Duration::from_secs(1).saturating_sub(asked.elapsed())).await;
            }
            measured
        })
        .buffer_unordered(COUNT)
        .collect()
        .await;
    let latencies = Latencies::of(reads.into_iter().flatten().collect());
    println!(
        "arauto_status through the connector: {} calls in {:?}, p50 {:?}, p95 {:?}, p99 {:?}, \
         max {:?}",
        latencies.count,
        reading.elapsed(),
        latencies.p50,
        latencies.p95,
        latencies.p99,
        latencies.max
    );
    assert_eq!(latencies.count, COUNT * READS);

    let quarantined = devices_in(&fixture, "quarantined").await;
    println!("quarantined devices: {quarantined}");
    assert_eq!(quarantined, 0);
    assert_eq!(
        devices_in(&fixture, "active").await,
        i64::try_from(COUNT).unwrap()
    );
    assert_eq!(fixture.hub().sessions_by_device().len(), COUNT);
    shutdown(&mut ones).await;
}
