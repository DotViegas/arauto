//! The swap of the credential on the side of the connector (`docs/04-seguranca-conexao.md`,
//! C7): the 24-hour swap with the clock of the hub moved forward, and a fault at each point of
//! the durable write and of the swap, the first one and the one of the 24 hours. None of them
//! locks the legitimate device out or sends it to quarantine.

use std::sync::Arc;
use std::time::Duration;

use arauto_connector::testing::{self, Attempted, Event, Point};
use arauto_connector::{Connector, Settings};
use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::limits::{DAY_MS, SECOND_MS};
use arauto_testkit::hub::fixture::{Fixture, settings};

use crate::support::{self, Folder, Once, PATIENCE, approved, connected, status_works};

/// Clean attempts, until one reaches the session: the device is never locked out.
async fn reaches_the_session(folder: &Folder, device: &str, settings: &Settings) {
    match support::clean(folder, device, settings).await {
        Ok(Attempted::Session(_)) => {}
        other => panic!("the device was locked out: {other:?}"),
    }
}

async fn not_in_quarantine(fixture: &Fixture, folder: &Folder, device: &str) {
    let identity = folder.public(device);
    assert_eq!(fixture.state_of_key(&identity).await.0, "active");
    assert_eq!(fixture.alerts_of_key("quarantined", &identity).await, 0);
}

/// The hub moves its clock 24 hours: the swap happens inside the open session, the new
/// credential is written before "saved", and the calls go on.
#[tokio::test(flavor = "multi_thread")]
async fn the_24_hour_swap_with_the_clock_moved_forward() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let fixture = Fixture::with_clock(settings(), Arc::new(clock.clone())).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s, &mut watch).await;
    let device = folder.current().unwrap();
    let identity = folder.public(&device);
    for _ in 0..2 {
        let before = folder.credential(&device).unwrap();
        let generation = fixture.max_generation_of_key(&identity).await;
        watch.forget();
        clock.advance(DAY_MS + SECOND_MS);
        watch
            .until(PATIENCE, |event| matches!(event, Event::Rotated { .. }))
            .await;
        assert_ne!(folder.credential(&device).unwrap(), before);
        assert_eq!(
            fixture.max_generation_of_key(&identity).await,
            generation + 1
        );
        assert!(status_works(&connector).await);
        assert_eq!(fixture.state_of_key(&identity).await.0, "active");
    }
    // The session was never dropped for it.
    assert!(
        !watch
            .drain()
            .iter()
            .any(|event| matches!(event, Event::AttemptStarted { .. })),
        "{:?}",
        watch.seen
    );
    connector.shutdown().await;
}

/// A fault at each point of the first swap: the next clean attempt reaches the session, and
/// the device is never in quarantine.
#[tokio::test(flavor = "multi_thread")]
async fn a_fault_at_each_point_of_the_first_swap_never_locks_the_device_out() {
    let fixture = Fixture::new(settings()).await;
    let (clean, _watch, _registrations) = support::settings();
    for point in Point::SWAP {
        let (_, folder) = approved(&fixture, &clean).await;
        let device = folder.only_device();
        let faulty = clean.clone().with_faults(Once::at(point));
        let attempted = testing::attempt(&folder.home, &device, &faulty, Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(attempted, Attempted::Faulted(point), "{point:?}");
        // Nothing half written: the credential is a whole one, or none.
        if matches!(
            point,
            Point::BeforePresent
                | Point::BeforeCheckOk
                | Point::BeforeSaving
                | Point::WriteCredential
        ) {
            assert_eq!(folder.credential(&device), None, "{point:?}");
        } else {
            assert!(folder.credential(&device).is_some(), "{point:?}");
        }
        reaches_the_session(&folder, &device, &clean).await;
        not_in_quarantine(&fixture, &folder, &device).await;
    }
}

/// How long the session of a faulty attempt is kept, and how long the swap may take to reach
/// the fault: the T1 of the hub writes with the disk forced, and a disk busy with the
/// databases the other tests create can take seconds. The attempt ends at the fault, so this
/// is only waited for when something is wrong.
const SWAP_PATIENCE: Duration = Duration::from_secs(60);

/// A fault at each point of the 24-hour swap, inside an open session: the next clean attempt
/// reaches the session, without quarantine.
#[tokio::test(flavor = "multi_thread")]
async fn a_fault_at_each_point_of_the_24_hour_swap_never_locks_the_device_out() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let fixture = Fixture::with_clock(settings(), Arc::new(clock.clone())).await;
    let (clean, _watch, _registrations) = support::settings();
    for point in Point::ROTATION {
        let (_, folder) = approved(&fixture, &clean).await;
        let device = folder.only_device();
        reaches_the_session(&folder, &device, &clean).await;
        let (events, mut heard) = tokio::sync::mpsc::unbounded_channel();
        let faulty = clean
            .clone()
            .with_faults(Once::at(point))
            .with_events(events);
        let running = {
            let home = folder.home.clone();
            let device = device.clone();
            tokio::spawn(
                async move { testing::attempt(&home, &device, &faulty, SWAP_PATIENCE).await },
            )
        };
        // In session: the clock of the hub moves, and the swap begins.
        loop {
            let event = tokio::time::timeout(PATIENCE, heard.recv())
                .await
                .unwrap()
                .unwrap();
            if matches!(event, Event::Session { .. }) {
                break;
            }
        }
        clock.advance(DAY_MS + SECOND_MS);
        loop {
            match tokio::time::timeout(SWAP_PATIENCE, heard.recv()).await {
                Ok(Some(Event::Faulted { point: found, .. })) if found == point => break,
                Ok(Some(_)) => {}
                Ok(None) => panic!(
                    "{point:?}: the attempt ended before the fault: {:?}",
                    running.await
                ),
                Err(_) => panic!("{point:?}: the swap did not reach the fault in time"),
            }
        }
        let attempted = tokio::time::timeout(PATIENCE, running)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            matches!(attempted, Attempted::Session(_)),
            "{point:?}: {attempted:?}"
        );
        reaches_the_session(&folder, &device, &clean).await;
        not_in_quarantine(&fixture, &folder, &device).await;
    }
}

/// A connector killed in the middle of a write leaves the old credential or the new one, never
/// a part: the file is replaced by a rename, and the leftover of the temporary file is removed
/// at the next start.
#[tokio::test(flavor = "multi_thread")]
async fn the_leftovers_of_an_interrupted_write_are_removed_at_the_start() {
    let fixture = Fixture::new(settings()).await;
    let (s, mut watch, _registrations) = support::settings();
    let (_, folder, connector) = connected(&fixture, s.clone(), &mut watch).await;
    connector.shutdown().await;
    let device = folder.current().unwrap();
    let leftover = folder
        .device_path(&device)
        .join(".credential.seal.0123456789abcdef.tmp");
    std::fs::write(&leftover, b"half of a credential").unwrap();
    let connector = Connector::start(folder.home.clone(), s).await.unwrap();
    assert!(!leftover.exists());
    watch.forget();
    watch
        .until(PATIENCE, |event| matches!(event, Event::Serving { .. }))
        .await;
    assert!(status_works(&connector).await);
    connector.shutdown().await;
}
