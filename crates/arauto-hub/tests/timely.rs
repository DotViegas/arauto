//! What the hub promises in seconds, measured end to end: the wait for approval ends by the
//! notice and by the sweep; a revoked device loses its session in less than 5 seconds without
//! the notice; a database restored under the running hub closes every session at the next
//! sweep (`docs/10-maquinas-de-estado.md`, section 7; `docs/15-mvp.md`, section 2).
//!
//! A binary of its own: the other tests of the channel create databases from a template, and
//! each creation stalls the server for seconds while it forces its files to disk. A deadline
//! of the hub is measured here, away from that.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "common/mod.rs"]
mod common;

#[allow(dead_code)]
#[path = "channel/support.rs"]
mod support;

#[path = "channel/fixture.rs"]
mod fixture;

use std::sync::Arc;
use std::time::Duration;

use arauto_hub::repo::approval::ApproveOutcome;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::ChannelState;
use arauto_testkit::device::{Faults, Reached};

use crate::fixture::{Fixture, PATIENCE, settings};
use crate::support::eventually;

/// The wait ends by the notice of the admin command, and also without it, by the sweep:
/// approved, the swap follows; denied, AR-301.
#[tokio::test(flavor = "multi_thread")]
async fn the_wait_ends_by_the_notice_and_by_the_sweep_approved_or_denied() {
    // With the notice, and a sweep that does not come during the test.
    let mut by_notice = settings();
    by_notice.sweep_interval_ms = 600_000;
    // Without the notice, with the sweep.
    let mut by_sweep = settings();
    by_sweep.listen_admin = false;
    for (case, chosen) in [("notice", by_notice), ("sweep", by_sweep)] {
        let fixture = Fixture::new(chosen).await;
        // Let the listener connect (its first sweep runs at that moment).
        tokio::time::sleep(Duration::from_millis(500)).await;
        for approve in [true, false] {
            let name = fixture.hermes().await;
            let device = fixture.device();
            let code = fixture.enroll(&name, &device).await;
            let waiting = {
                let device = device.clone();
                tokio::spawn(async move {
                    device
                        .channel_waiting(Faults::default(), Duration::from_secs(30))
                        .await
                })
            };
            let hub = Arc::clone(fixture.hub());
            let id = fixture.id_of(&device).await;
            assert!(
                eventually(PATIENCE, || hub
                    .links()
                    .iter()
                    .any(|link| link.phase() == arauto_hub::channel::Phase::Waiting))
                .await,
                "{case}"
            );
            let started = tokio::time::Instant::now();
            if approve {
                assert!(matches!(
                    fixture.approve(&name, &code).await,
                    ApproveOutcome::Approved(_)
                ));
                let session = waiting.await.unwrap().unwrap();
                assert!(
                    matches!(session, Reached::Session(_)),
                    "{case}: {session:?}"
                );
            } else {
                fixture.admin.deny(&name, fixture.now()).await.unwrap();
                let reached = waiting.await.unwrap().unwrap();
                assert_eq!(reached.closing(), Some(ErrorCode::Ar301), "{case}");
                let Reached::Closed { state, .. } = reached else {
                    panic!("closed");
                };
                assert_eq!(state, Some(ChannelState::Waiting));
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{case}: {:?}",
                started.elapsed()
            );
            let _ = id;
        }
    }
}

/// C10 and `docs/15-mvp.md`, section 2: revoking closes the session in less than 5 seconds
/// with the notice of the admin command lost. The sweep, every 2 seconds, is what does it.
#[tokio::test(flavor = "multi_thread")]
async fn revoking_closes_the_session_in_less_than_5_seconds_without_the_notice() {
    let mut chosen = settings();
    chosen.listen_admin = false;
    chosen.sweep_interval_ms = arauto_hub::channel::settings::SWEEP_INTERVAL_MS;
    let fixture = Fixture::new(chosen).await;
    let (name, mut session) = fixture.connected().await;
    fixture.admin.revoke(&name, fixture.now()).await.unwrap();
    let revoked = tokio::time::Instant::now();
    let ended = session.ended(Duration::from_secs(10)).await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar502));
    assert!(
        revoked.elapsed() < Duration::from_secs(5),
        "{:?}",
        revoked.elapsed()
    );
}

/// The database comes back from an older copy while the hub serves: the sweep sees the
/// database behind the mark, turns the restored mode on, and closes every session with the
/// code of its device (AR-506, suspended).
#[tokio::test(flavor = "multi_thread")]
async fn a_database_restored_with_the_hub_running_suspends_and_closes_the_sessions() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut first) = fixture.connected().await;
    let (_, mut second) = fixture.connected().await;
    fixture.restore_an_older_copy().await;
    let started = tokio::time::Instant::now();
    for session in [&mut first, &mut second] {
        let ended = session.ended(PATIENCE).await.unwrap();
        assert_eq!(ended.closing, Some(ErrorCode::Ar506));
    }
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(fixture.restored_mode().await);
    let device = first.device().clone();
    assert_eq!(
        fixture.state_of(&device).await,
        ("suspended".to_owned(), Some("restored".to_owned()))
    );
    // Coming back, the device is told the state and is never given a credential.
    let back = device.channel(Faults::default()).await.unwrap();
    let Reached::Closed { state, ended } = back else {
        panic!("{back:?}");
    };
    assert_eq!(state, Some(ChannelState::Ended(ErrorCode::Ar506)));
    assert_eq!(ended.closing, Some(ErrorCode::Ar506));
}

/// `docs/15-mvp.md`, section 2: the sweep reads through a pool of its own. With every
/// connection of the pool of the connectors busy (here, the calls of another Hermes waiting
/// for a table the test holds) and the notice lost, revoking still closes the session in less
/// than 5 seconds.
#[tokio::test(flavor = "multi_thread")]
async fn revoking_with_the_pool_of_the_connectors_busy_still_closes_in_less_than_5_seconds() {
    let mut chosen = settings();
    chosen.listen_admin = false;
    chosen.sweep_interval_ms = arauto_hub::channel::settings::SWEEP_INTERVAL_MS;
    let fixture = Fixture::new(chosen).await;
    let (name, mut session) = fixture.connected().await;
    let (_, mut busy) = fixture.connected().await;
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE peer_link IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *holder)
        .await
        .unwrap();
    for id in 1..=32 {
        busy.send_call(
            id,
            arauto_protocol::tools::ARAUTO_STATUS,
            serde_json::json!({}),
        )
        .await
        .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    fixture.admin.revoke(&name, fixture.now()).await.unwrap();
    let revoked = tokio::time::Instant::now();
    let ended = session.ended(Duration::from_secs(15)).await.unwrap();
    let took = revoked.elapsed();
    holder.rollback().await.unwrap();
    assert_eq!(ended.closing, Some(ErrorCode::Ar502));
    assert!(took < Duration::from_secs(5), "{took:?}");
}
