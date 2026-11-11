//! The races and the copies (`docs/14-protocolo-do-canal.md`, section 6;
//! `docs/04-seguranca-conexao.md`, C7, C9 and C10): the late answer of an abandoned attempt,
//! revoking between the credential and the "saved", revoking without the notice, a copy used
//! with the true device on and off, and the established connection that does not answer.

use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::clock::{Clock, ManualClock, SystemClock};
use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{
    ChannelState, CheckNonce, CheckOk, Message, MessageType, Present, Purpose, Saved,
};
use arauto_testkit::connector::TestConnector;
use arauto_testkit::device::{Device, Faults, Reached};

use crate::fixture::{Fixture, PATIENCE, settings, until_the_end};
use crate::support::eventually;

async fn up_to_the_check(
    fixture: &Fixture,
    device: &Device,
) -> (TestConnector, arauto_protocol::message::CheckNonce) {
    fixture.up_to_the_check(device).await
}

/// Rule 2 of the 14: only the newest connection of a device is judged. An attempt the
/// connector abandoned answers its check late, after a newer one opened: it hears AR-206 and
/// decides nothing; the newer one goes on.
#[tokio::test(flavor = "multi_thread")]
async fn the_late_answer_of_an_abandoned_attempt_decides_nothing() {
    let fixture = Fixture::new(settings()).await;
    for first_time in [true, false] {
        let (_, device) = fixture.approved().await;
        if !first_time {
            // An active device, at generation 1.
            drop(
                device
                    .channel(Faults::default())
                    .await
                    .unwrap()
                    .session()
                    .unwrap(),
            );
        }
        let (mut abandoned, late) = up_to_the_check(&fixture, &device).await;
        let (mut newer, nonce) = up_to_the_check(&fixture, &device).await;
        // The answer of the abandoned attempt arrives now, late.
        abandoned
            .send(&Message::CheckOk(CheckOk { nonce: late }))
            .await
            .unwrap();
        assert_eq!(
            abandoned.ended(PATIENCE).await.unwrap().closing,
            Some(ErrorCode::Ar206)
        );
        newer
            .send(&Message::CheckOk(CheckOk { nonce }))
            .await
            .unwrap();
        let Message::Credential(credential) = newer.recv(PATIENCE).await.unwrap() else {
            panic!("the newer attempt gets the credential");
        };
        let saved = Saved {
            generation: credential.generation,
            digest: CredentialDigest::of(&credential.sealed),
        };
        newer.send(&Message::Saved(saved)).await.unwrap();
        assert!(matches!(
            newer.recv(PATIENCE).await.unwrap(),
            Message::Session(_)
        ));
        assert_eq!(fixture.state_of(&device).await.0, "active");
        assert_eq!(fixture.alerts("quarantined", &device).await, 0);
    }

    // Step 6: the newer attempt answers first. The older one, still before its answer, is
    // closed with AR-206 at once, and its answer, when it comes, finds nothing.
    let (_, device) = fixture.approved().await;
    let (mut abandoned, late) = up_to_the_check(&fixture, &device).await;
    let (mut newer, nonce) = up_to_the_check(&fixture, &device).await;
    newer
        .send(&Message::CheckOk(CheckOk { nonce }))
        .await
        .unwrap();
    assert_eq!(
        abandoned.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar206)
    );
    let _ = abandoned
        .send(&Message::CheckOk(CheckOk { nonce: late }))
        .await;
    assert!(matches!(
        newer.recv(PATIENCE).await.unwrap(),
        Message::Credential(_)
    ));
    assert_eq!(fixture.state_of(&device).await.0, "approved");

    // The presentation without swap is judged only on the newest connection too: the late
    // answer of an abandoned attempt of a revoked device hears AR-206, not the code of the
    // device.
    let (name, session) = fixture.connected().await;
    let device = session.device().clone();
    drop(session);
    fixture.admin.revoke(&name, fixture.now()).await.unwrap();
    let (mut abandoned, late) = up_to_the_check(&fixture, &device).await;
    let (mut newer, nonce) = up_to_the_check(&fixture, &device).await;
    abandoned
        .send(&Message::CheckOk(CheckOk { nonce: late }))
        .await
        .unwrap();
    assert_eq!(
        abandoned.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar206)
    );
    newer
        .send(&Message::CheckOk(CheckOk { nonce }))
        .await
        .unwrap();
    assert_eq!(
        newer.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar502)
    );
}

/// Rule 2 of the 14, from step 5 to T1: an attempt that passed its step 5 and is no longer
/// the newest when its T1 comes issues nothing. Here it waits 3 seconds on the test of an
/// established connection that does not answer, and a newer attempt opens meanwhile.
#[tokio::test(flavor = "multi_thread")]
async fn an_attempt_that_is_no_longer_the_newest_at_its_t1_issues_nothing() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    let device = session.device().clone();
    session.answer_checks = false;
    let muted = tokio::spawn(async move { session.ended(Duration::from_secs(30)).await });
    let issued = || async {
        let generation: i64 =
            sqlx::query_scalar("SELECT max_generation FROM device WHERE public_key = $1")
                .bind(device.public().as_bytes().as_slice())
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        generation
    };
    let before = issued().await;
    let (mut older, nonce) = up_to_the_check(&fixture, &device).await;
    older
        .send(&Message::CheckOk(CheckOk { nonce }))
        .await
        .unwrap();
    // The newer attempt opens while the older one tests the established connection.
    let mut newer = TestConnector::connect(
        fixture.addr(),
        device.identity(),
        &fixture.installation.hub_key,
    )
    .await
    .unwrap();
    newer.hello(Purpose::Channel).await.unwrap();
    assert!(matches!(
        newer.recv(PATIENCE).await.unwrap(),
        Message::State(ChannelState::Active)
    ));
    match older.recv(PATIENCE).await {
        Err(arauto_testkit::KitError::Ended(ended)) => {
            assert_eq!(ended.closing, Some(ErrorCode::Ar206));
        }
        other => panic!("the older attempt got {other:?}"),
    }
    assert_eq!(
        muted.await.unwrap().unwrap().closing,
        Some(ErrorCode::Ar208)
    );
    assert_eq!(
        issued().await,
        before,
        "nothing was issued to the older attempt"
    );
    // The newer one goes on to its session.
    newer
        .send(&Message::Present(Present {
            credential: device.disk().credential(),
        }))
        .await
        .unwrap();
    let Message::Check(check) = newer.recv(PATIENCE).await.unwrap() else {
        panic!("no check");
    };
    newer
        .send(&Message::CheckOk(CheckOk { nonce: check.nonce }))
        .await
        .unwrap();
    assert!(matches!(
        newer.recv(PATIENCE).await.unwrap(),
        Message::Credential(_)
    ));
    assert_eq!(issued().await, before + 1);
    assert_eq!(fixture.state_of(&device).await.0, "active");
}

/// C7: a device revoked between the credential and the "saved" never ends active. The
/// "saved" that comes after the revocation hears AR-502, and nothing is promoted.
#[tokio::test(flavor = "multi_thread")]
async fn revoking_between_the_credential_and_the_saved_never_ends_active() {
    let fixture = Fixture::new(settings()).await;
    for first_time in [true, false] {
        let (name, device) = fixture.approved().await;
        if !first_time {
            drop(
                device
                    .channel(Faults::default())
                    .await
                    .unwrap()
                    .session()
                    .unwrap(),
            );
        }
        let before: i64 =
            sqlx::query_scalar("SELECT current_generation FROM device WHERE public_key = $1")
                .bind(device.public().as_bytes().as_slice())
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        let (mut connector, nonce) = up_to_the_check(&fixture, &device).await;
        connector
            .send(&Message::CheckOk(CheckOk { nonce }))
            .await
            .unwrap();
        let Message::Credential(credential) = connector.recv(PATIENCE).await.unwrap() else {
            panic!("no credential");
        };
        fixture.admin.revoke(&name, fixture.now()).await.unwrap();
        let saved = Saved {
            generation: credential.generation,
            digest: CredentialDigest::of(&credential.sealed),
        };
        connector.send(&Message::Saved(saved)).await.unwrap();
        assert_eq!(
            connector.ended(PATIENCE).await.unwrap().closing,
            Some(ErrorCode::Ar502)
        );
        let after: (String, i64) =
            sqlx::query_as("SELECT state, current_generation FROM device WHERE public_key = $1")
                .bind(device.public().as_bytes().as_slice())
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(after, ("revoked".to_owned(), before));
    }

    // Rule 7: a "saved" that is not about the pending credential of this connection closes
    // it, and promotes nothing.
    let (_, device) = fixture.approved().await;
    let (mut connector, nonce) = up_to_the_check(&fixture, &device).await;
    connector
        .send(&Message::CheckOk(CheckOk { nonce }))
        .await
        .unwrap();
    let Message::Credential(credential) = connector.recv(PATIENCE).await.unwrap() else {
        panic!("no credential");
    };
    let other = arauto_protocol::secret::SealedCredential::from_slice(&[1u8; 64]).unwrap();
    let saved = Saved {
        generation: credential.generation,
        digest: CredentialDigest::of(&other),
    };
    connector.send(&Message::Saved(saved)).await.unwrap();
    assert_eq!(
        connector.ended(PATIENCE).await.unwrap().closing,
        Some(ErrorCode::Ar204)
    );
    let after: (String, i64) =
        sqlx::query_as("SELECT state, current_generation FROM device WHERE public_key = $1")
            .bind(device.public().as_bytes().as_slice())
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(after, ("approved".to_owned(), 0));
}

/// C9: a copy of the files used while the true device is connected and answering. Its
/// presentation would be accepted, the hub tests the established connection, it answers:
/// two holders of the same files. Quarantine, the alert, and both connections close.
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_used_with_the_true_one_on_is_a_double_connection() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    let device = session.device().clone();
    let copy = device.copy_files();
    let true_one = tokio::spawn(async move {
        session
            .ended(Duration::from_secs(30))
            .await
            .map(|ended| ended.closing)
    });
    let reached = copy.channel(Faults::default()).await.unwrap();
    assert_eq!(reached.closing(), Some(ErrorCode::Ar501), "{reached:?}");
    assert_eq!(true_one.await.unwrap().unwrap(), Some(ErrorCode::Ar501));
    assert_eq!(
        fixture.state_of(&device).await,
        (
            "quarantined".to_owned(),
            Some("double_connection".to_owned())
        )
    );
    assert_eq!(fixture.alerts("quarantined", &device).await, 1);
}

/// C9 and section 6 of the 04, item 3: a copy used while the true device is off works until
/// the true one comes back; then it presents a credential the copy already swapped, and the
/// device goes to quarantine.
#[tokio::test(flavor = "multi_thread")]
async fn a_copy_used_with_the_true_one_off_is_found_when_it_comes_back() {
    let fixture = Fixture::new(settings()).await;
    let (_, session) = fixture.connected().await;
    let device = session.device().clone();
    let id = fixture.id_of(&device).await;
    drop(session);
    let hub = Arc::clone(fixture.hub());
    assert!(eventually(PATIENCE, || !hub.sessions_by_device().contains_key(&id)).await);

    let copy = device.copy_files();
    let copied = copy
        .channel(Faults::default())
        .await
        .unwrap()
        .session()
        .unwrap();
    drop(copied);
    assert!(eventually(PATIENCE, || !hub.sessions_by_device().contains_key(&id)).await);
    assert_eq!(fixture.state_of(&device).await.0, "active");

    let back = device.channel(Faults::default()).await.unwrap();
    assert_eq!(back.closing(), Some(ErrorCode::Ar501), "{back:?}");
    assert_eq!(
        fixture.state_of(&device).await,
        ("quarantined".to_owned(), Some("reuse".to_owned()))
    );
}

/// C9: the established connection does not answer its test in 3 seconds. It is given up for
/// dead with AR-208, the admin is told the connection was taken, and the new one goes on.
#[tokio::test(flavor = "multi_thread")]
async fn an_established_connection_that_does_not_answer_is_given_up_with_ar208() {
    let fixture = Fixture::new(settings()).await;
    let (_, mut session) = fixture.connected().await;
    let device = session.device().clone();
    session.answer_checks = false;
    let muted = tokio::spawn(async move {
        session
            .ended(Duration::from_secs(30))
            .await
            .map(|ended| ended.closing)
    });
    let started = tokio::time::Instant::now();
    let reached = device.channel(Faults::default()).await.unwrap();
    assert!(matches!(reached, Reached::Session(_)), "{reached:?}");
    assert!(started.elapsed() >= Duration::from_secs(3));
    assert_eq!(muted.await.unwrap().unwrap(), Some(ErrorCode::Ar208));
    assert_eq!(fixture.state_of(&device).await.0, "active");
    assert_eq!(fixture.alerts("connection_taken", &device).await, 1);
    assert_eq!(fixture.alerts("quarantined", &device).await, 0);
}

/// C6: a second waiting connection of a pending identity, and the first answers: the
/// identity was copied before the approval. The request is denied and the admin is alerted.
#[tokio::test(flavor = "multi_thread")]
async fn two_waiting_connections_of_one_identity_deny_the_request() {
    let fixture = Fixture::new(settings()).await;
    let name = fixture.hermes().await;
    let device = fixture.device();
    fixture.enroll(&name, &device).await;
    let first = {
        let device = device.clone();
        tokio::spawn(async move {
            device
                .channel_waiting(Faults::default(), Duration::from_secs(30))
                .await
        })
    };
    let hub = Arc::clone(fixture.hub());
    assert!(
        eventually(PATIENCE, || hub
            .links()
            .iter()
            .any(|link| link.phase() == arauto_hub::channel::Phase::Waiting))
        .await
    );
    let copy = device.copy_files();
    let second = copy
        .channel_waiting(Faults::default(), Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(second.closing(), Some(ErrorCode::Ar301));
    assert_eq!(
        first.await.unwrap().unwrap().closing(),
        Some(ErrorCode::Ar301)
    );
    assert_eq!(
        fixture.state_of(&device).await,
        ("denied".to_owned(), Some("double_waiting".to_owned()))
    );
    assert_eq!(fixture.alerts("identity_copied", &device).await, 1);
}

/// C7, rule 3, and the 14, section 6, steps 3 and 4: whoever presents answers the `check` of
/// this very connection, after reading it. A `check_ok` with another nonce, or one sent with
/// the `present` before the `check` came, closes with AR-204 and decides nothing: no
/// credential, the device as it was, and the next attempt goes on as usual.
#[tokio::test(flavor = "multi_thread")]
async fn a_check_ok_that_does_not_answer_the_check_of_this_connection_decides_nothing() {
    let fixture = Fixture::new(settings()).await;
    let (_, device) = fixture.approved().await;
    let before = fixture.max_generation(&device).await;

    // The nonce of the check, one byte off.
    let (mut connector, nonce) = up_to_the_check(&fixture, &device).await;
    let mut wrong = *nonce.as_bytes();
    wrong[0] ^= 1;
    connector
        .send(&Message::CheckOk(CheckOk {
            nonce: CheckNonce::from_bytes(wrong),
        }))
        .await
        .unwrap();
    let (seen, ended) = until_the_end(&mut connector).await;
    assert_eq!(seen, [], "the hub sent something after a wrong check_ok");
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));

    // An answer sent with the presentation, before the check was read: whatever nonce it
    // carries, it cannot be the one the hub draws after it.
    let mut connector = TestConnector::connect(
        fixture.addr(),
        device.identity(),
        &fixture.installation.hub_key,
    )
    .await
    .unwrap();
    connector.hello(Purpose::Channel).await.unwrap();
    assert!(matches!(
        connector.recv(PATIENCE).await.unwrap(),
        Message::State(ChannelState::Approved)
    ));
    connector
        .send(&Message::Present(Present { credential: None }))
        .await
        .unwrap();
    connector
        .send(&Message::CheckOk(CheckOk {
            nonce: CheckNonce::random().unwrap(),
        }))
        .await
        .unwrap();
    let (seen, ended) = until_the_end(&mut connector).await;
    assert_eq!(seen, [MessageType::Check]);
    assert_eq!(ended.closing, Some(ErrorCode::Ar204));

    assert_eq!(
        fixture.max_generation(&device).await,
        before,
        "nothing issued"
    );
    assert_eq!(fixture.state_of(&device).await.0, "approved");
    let back = device.channel(Faults::default()).await.unwrap();
    assert!(matches!(back, Reached::Session(_)), "{back:?}");
}

/// The 14, section 6, from step 5 to the end of T1: one attempt of an identity is judged at a
/// time. While an attempt is in its T1 (here it waits for its Hermes, which the test holds), a
/// newer attempt that answered its check waits for its turn: it closes nothing and judges
/// nothing until that T1 ends. Then it takes the turn, the older hears AR-206, and the newer
/// reaches the session without any quarantine.
#[tokio::test(flavor = "multi_thread")]
async fn one_attempt_of_an_identity_is_judged_at_a_time_from_step_5_to_t1() {
    let fixture = Fixture::new(settings()).await;
    let (name, device) = fixture.approved().await;
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM hermes WHERE name = $1 FOR UPDATE")
        .bind(&name)
        .execute(&mut *holder)
        .await
        .unwrap();
    let (mut older, nonce) = up_to_the_check(&fixture, &device).await;
    older
        .send(&Message::CheckOk(CheckOk { nonce }))
        .await
        .unwrap();
    // The older attempt is in its T1, waiting for the Hermes.
    fixture.until_a_query_waits_for_a_lock().await;
    let (mut newer, nonce) = up_to_the_check(&fixture, &device).await;
    newer
        .send(&Message::CheckOk(CheckOk { nonce }))
        .await
        .unwrap();
    // The newer one waits for its turn: the older one is not closed meanwhile.
    match older.recv(Duration::from_secs(1)).await {
        Err(arauto_testkit::KitError::TimedOut) => {}
        other => panic!("the older attempt was told {other:?} during its T1"),
    }
    holder.commit().await.unwrap();

    // Its T1 ends and issues to it; then the newer one takes the turn and closes it.
    let (seen, ended) = until_the_end(&mut older).await;
    assert_eq!(seen, [MessageType::Credential]);
    assert_eq!(ended.closing, Some(ErrorCode::Ar206));
    let Message::Credential(credential) = newer.recv(PATIENCE).await.unwrap() else {
        panic!("the newer attempt gets the credential");
    };
    let saved = Saved {
        generation: credential.generation,
        digest: CredentialDigest::of(&credential.sealed),
    };
    newer.send(&Message::Saved(saved)).await.unwrap();
    assert!(matches!(
        newer.recv(PATIENCE).await.unwrap(),
        Message::Session(_)
    ));
    assert_eq!(fixture.state_of(&device).await.0, "active");
    assert_eq!(fixture.alerts("quarantined", &device).await, 0);
}

/// C9: a session that already ended, and whose end is still being recorded (here the record
/// waits for the Hermes, which the test holds), is in the record of connections but is
/// nobody's to take. The same device coming back is not tested against it, and no alert of a
/// taken connection is raised.
#[tokio::test(flavor = "multi_thread")]
async fn a_session_that_already_ended_is_not_reported_as_taken() {
    let fixture = Fixture::new(settings()).await;
    let (name, session) = fixture.connected().await;
    let device = session.device().clone();
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM hermes WHERE name = $1 FOR UPDATE")
        .bind(&name)
        .execute(&mut *holder)
        .await
        .unwrap();
    drop(session);
    let hub = Arc::clone(fixture.hub());
    assert!(eventually(PATIENCE, || hub.links().iter().all(|link| !link.is_open())).await);
    let back = {
        let device = device.clone();
        tokio::spawn(async move { device.channel(Faults::default()).await })
    };
    // The new connection passes step 8 while the end of the old one is still on its way.
    tokio::time::sleep(Duration::from_millis(1_000)).await;
    holder.commit().await.unwrap();
    let back = back.await.unwrap().unwrap();
    assert!(matches!(back, Reached::Session(_)), "{back:?}");
    assert_eq!(fixture.alerts("connection_taken", &device).await, 0);
    assert_eq!(fixture.state_of(&device).await.0, "active");
}

/// C9 and the 14, section 6, step 8: the answer to the test of a newer connection never waits
/// for the session, which may be busy with the database. Here the session is in the T1 of its
/// 24-hour swap, waiting for the Hermes the test holds, when a copy of its files comes: the
/// true one still answers in time, and the copy is seen.
#[tokio::test(flavor = "multi_thread")]
async fn a_session_busy_with_the_database_still_answers_the_test_of_a_newer_connection() {
    let clock = ManualClock::new(SystemClock.now_ms());
    let mut chosen = settings();
    chosen.sweep_interval_ms = 100;
    chosen.deadlines.old_connection_probe_ms = 1_000;
    let fixture = Fixture::with_clock(chosen, Arc::new(clock.clone())).await;
    let (name, mut session) = fixture.connected().await;
    let device = session.device().clone();
    let copy = device.copy_files();
    let mut holder = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM hermes WHERE name = $1 FOR UPDATE")
        .bind(&name)
        .execute(&mut *holder)
        .await
        .unwrap();
    let true_one = tokio::spawn(async move {
        session
            .ended(Duration::from_secs(30))
            .await
            .map(|ended| ended.closing)
    });
    clock.advance(24 * 3_600_000);
    // The session is in the T1 of its swap, waiting for the Hermes.
    fixture.until_a_query_waits_for_a_lock().await;
    let reached = tokio::spawn(async move { copy.channel(Faults::default()).await });
    // Past the test of the copy, and before the lock of the database gives up on the T1.
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    holder.commit().await.unwrap();
    let reached = reached.await.unwrap().unwrap();
    assert_eq!(reached.closing(), Some(ErrorCode::Ar501), "{reached:?}");
    assert_eq!(true_one.await.unwrap().unwrap(), Some(ErrorCode::Ar501));
    assert_eq!(
        fixture.state_of(&device).await,
        (
            "quarantined".to_owned(),
            Some("double_connection".to_owned())
        )
    );
    assert_eq!(fixture.alerts("connection_taken", &device).await, 0);
}
