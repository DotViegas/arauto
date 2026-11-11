//! The restored mode of C11, effect by effect.

use std::net::IpAddr;
use std::time::Duration;

use arauto_hub::audit::{self, Action, Actor, AuditError, Event};
use arauto_hub::db::ResolvedOrg;
use arauto_hub::mark::{Mark, MarkValue};
use arauto_hub::restore::{
    self, EnterOutcome, Entered, Evidence, Installation, ListedAs, Listing, RestoreError, Standing,
    UnknownDevice,
};
use arauto_protocol::message::DeclaredText;
use arauto_protocol::noise::PublicKey;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::common::{self, Confirmed, NOW, TestDb, TestHome};

/// A row of `unknown_device`: the identifier, the identity, what was declared, the address
/// and when it was seen.
type ListedRow = (Uuid, Vec<u8>, serde_json::Value, Option<String>, i64);

/// When the restoration is noticed, in these tests.
const LATER: i64 = NOW + 86_400_000;

async fn installed() -> (TestDb, TestHome, ResolvedOrg) {
    let db = TestDb::own().await;
    let home = TestHome::new();
    common::install(&db.pool).await;
    home.mark.advance(0).await.unwrap();
    let organization = common::organization(&db.pool, &home).await;
    (db, home, organization)
}

async fn security_event(pool: &PgPool, mark: &Mark) -> i64 {
    let mut write = audit::begin_security(pool, mark).await.unwrap();
    write
        .record(NOW, &Event::new(Action::DeviceRevoked, Actor::System))
        .await
        .unwrap();
    write.commit().await.unwrap().unwrap()
}

/// Makes the database look like an older copy: the mark knows of `lost` security events that
/// the database does not have.
async fn lose_events(mark: &Mark, lost: i64) -> i64 {
    let MarkValue::At(at) = mark.read().await.unwrap() else {
        panic!("there is a mark");
    };
    mark.advance(at + lost).await.unwrap()
}

async fn entered(pool: &PgPool, mark: &Mark, evidence: Evidence) -> Entered {
    match restore::enter(pool, mark, LATER, evidence).await.unwrap() {
        EnterOutcome::Entered(entered) => entered,
        EnterOutcome::AlreadyHandled => panic!("the evidence was taken as already handled"),
    }
}

async fn restored_evidence(pool: &PgPool, mark: &Mark) -> Evidence {
    match restore::check(pool, mark).await.unwrap() {
        Standing::Restored(evidence) => evidence,
        other => panic!("the check did not notice the restoration: {other:?}"),
    }
}

async fn device_row(
    pool: &PgPool,
    id: Uuid,
) -> (String, Option<String>, Option<String>, bool, i64) {
    sqlx::query_as(
        "SELECT state, state_reason, slot, marked_by_restoration, state_since \
         FROM device WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn audit_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn installation(pool: &PgPool) -> Installation {
    restore::installation(pool).await.unwrap().unwrap()
}

/// Lists an identity the database does not know, as the presentation without swap does.
async fn list_stranger(
    pool: &PgPool,
    organization: ResolvedOrg,
    identity: &PublicKey,
    connector_version: Option<&DeclaredText>,
    address: Option<IpAddr>,
    at: i64,
) -> UnknownDevice {
    restore::list_unknown_device(
        pool,
        organization,
        &Listing {
            identity,
            connector_version,
            address,
            listed_as: ListedAs::Stranger,
        },
        at,
    )
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------- noticing

#[tokio::test]
async fn a_database_that_follows_its_mark_accuses_nothing() {
    let (db, home, _) = installed().await;
    assert_eq!(
        restore::check(&db.pool, &home.mark).await.unwrap(),
        Standing::Consistent
    );
    for _ in 0..3 {
        security_event(&db.pool, &home.mark).await;
        assert_eq!(
            restore::check(&db.pool, &home.mark).await.unwrap(),
            Standing::Consistent
        );
    }
    let state = installation(&db.pool).await;
    assert!(!state.restored_mode);
    assert_eq!((state.restored_at, state.restored_copy_of), (None, None));
    assert_eq!(state.created_at, NOW);
}

#[tokio::test]
async fn before_init_the_check_says_not_installed() {
    let db = TestDb::own().await;
    let home = TestHome::new();
    assert_eq!(
        restore::check(&db.pool, &home.mark).await.unwrap(),
        Standing::NotInstalled
    );
    assert_eq!(restore::installation(&db.pool).await.unwrap(), None);
    assert!(matches!(
        restore::enter(&db.pool, &home.mark, LATER, Evidence::MarkAbsent).await,
        Err(RestoreError::NotInstalled)
    ));
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::Absent);
}

#[tokio::test]
async fn a_damaged_mark_is_an_error_not_a_guess() {
    let (db, home, _) = installed().await;
    std::fs::write(home.mark.path(), b"not a number").unwrap();
    assert!(matches!(
        restore::check(&db.pool, &home.mark).await,
        Err(RestoreError::Mark(_))
    ));
    assert!(matches!(
        audit::begin_security(&db.pool, &home.mark).await,
        Err(AuditError::Mark(_))
    ));
}

// ---------------------------------------------------------------------------- every effect

#[tokio::test]
async fn turning_the_mode_on_does_every_effect_of_c11_in_one_write() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    let new_hermes = || common::hermes(pool, organization);
    let new_device = |hermes: Uuid, state: &'static str, confirmed: Confirmed| async move {
        common::device(pool, organization, hermes, state, confirmed)
            .await
            .0
    };

    // One Hermes with a device in use and a request in progress beside it.
    let both = new_hermes().await;
    let active = new_device(both, "active", Confirmed::Yes).await;
    let pending = new_device(both, "pending", Confirmed::Never).await;
    // One with only an approved request.
    let approved = new_device(new_hermes().await, "approved", Confirmed::Never).await;
    // Another active one, and the devices that already were out of use.
    let other_active = new_device(new_hermes().await, "active", Confirmed::Yes).await;
    let quarantined_request = new_device(new_hermes().await, "quarantined", Confirmed::Never).await;
    let quarantined_in_use = new_device(new_hermes().await, "quarantined", Confirmed::Yes).await;
    let already_suspended = new_device(new_hermes().await, "suspended", Confirmed::Yes).await;
    let finals = new_hermes().await;
    let denied = new_device(finals, "denied", Confirmed::Never).await;
    let expired = new_device(finals, "expired", Confirmed::Yes).await;
    let replaced = new_device(finals, "replaced", Confirmed::Yes).await;
    let revoked = new_device(finals, "revoked", Confirmed::Yes).await;

    let (issued, _) = common::invite(pool, organization, finals, "issued", None).await;
    let (used, _) = common::invite(pool, organization, both, "used", Some(pending)).await;
    let (cancelled, _) = common::invite(pool, organization, both, "cancelled", None).await;
    let (expired_invite, _) = common::invite(pool, organization, both, "expired", None).await;

    let last_event = security_event(pool, &home.mark).await;
    let mark_before = lose_events(&home.mark, 7).await;
    assert_eq!(mark_before, last_event + 7);
    let rows_before = audit_rows(pool).await;

    let evidence = restored_evidence(pool, &home.mark).await;
    assert_eq!(
        evidence,
        Evidence::BehindMark {
            mark: mark_before,
            database: last_event
        }
    );
    let done = entered(pool, &home.mark, evidence).await;

    // Every active device is suspended.
    assert_eq!(done.suspended, 2);
    for device in [active, other_active] {
        assert_eq!(
            device_row(pool, device).await,
            (
                "suspended".to_owned(),
                Some("restored".to_owned()),
                Some("in_use".to_owned()),
                true,
                LATER
            )
        );
    }

    // Requests in progress are closed: the pending ones and the approved ones.
    assert_eq!(done.requests_closed, 2);
    assert_eq!(
        device_row(pool, pending).await,
        (
            "expired".to_owned(),
            Some("restored_pending".to_owned()),
            None,
            true,
            LATER
        )
    );
    assert_eq!(
        device_row(pool, approved).await,
        (
            "expired".to_owned(),
            Some("restored_approved".to_owned()),
            None,
            true,
            LATER
        )
    );
    assert_eq!(
        common::text_of(pool, "device", "verification_code", pending).await,
        None
    );
    assert_eq!(
        common::text_of(pool, "device", "confirm_until", pending).await,
        None
    );

    // What was already in quarantine stays as it is, request or not; so does the rest.
    assert_eq!(
        device_row(pool, quarantined_request).await,
        (
            "quarantined".to_owned(),
            None,
            Some("request".to_owned()),
            true,
            NOW
        )
    );
    for (device, state, slot) in [
        (quarantined_in_use, "quarantined", Some("in_use")),
        (already_suspended, "suspended", Some("in_use")),
        (denied, "denied", None),
        (expired, "expired", None),
        (replaced, "replaced", None),
        (revoked, "revoked", None),
    ] {
        assert_eq!(
            device_row(pool, device).await,
            (state.to_owned(), None, slot.map(str::to_owned), true, NOW),
            "{state}"
        );
    }

    // Every device that existed is marked, in any state.
    assert_eq!(done.marked, 11);
    let unmarked: i64 =
        sqlx::query_scalar("SELECT count(*) FROM device WHERE NOT marked_by_restoration")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(unmarked, 0);

    // Issued invites are cancelled, and the digest goes with them; the others keep their
    // state.
    assert_eq!(done.invites_cancelled, 1);
    assert_eq!(
        common::text_of(pool, "invite", "digest", issued).await,
        None,
        "nothing of a cancelled invite is kept"
    );
    assert!(
        common::text_of(pool, "invite", "digest", used)
            .await
            .is_some(),
        "the digest of a used invite stays: it is how its request is found again"
    );
    for (invite, state) in [
        (issued, "cancelled"),
        (used, "used"),
        (cancelled, "cancelled"),
        (expired_invite, "expired"),
    ] {
        assert_eq!(
            common::text_of(pool, "invite", "state", invite).await,
            Some(state.to_owned())
        );
    }

    // The mode is on, with when it began and what the copy is from.
    assert_eq!(
        installation(pool).await,
        Installation {
            created_at: NOW,
            restored_mode: true,
            restored_at: Some(LATER),
            restored_copy_of: Some(NOW),
        }
    );

    // The numbering of the audit continues from the mark: one new event, right above it.
    assert_eq!(done.seq, mark_before + 1);
    assert_eq!(audit_rows(pool).await, rows_before + 1);
    let (action, security, at, details): (String, bool, i64, serde_json::Value) =
        sqlx::query_as("SELECT action, security, at, details FROM audit WHERE seq = $1")
            .bind(done.seq)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        (action.as_str(), security, at),
        ("restored_mode.entered", true, LATER)
    );
    assert_eq!(
        details,
        serde_json::json!({
            "evidence": "behind_mark",
            "mode_was_on": false,
            "suspended": 2,
            "requests_closed": 2,
            "invites_cancelled": 1,
            "marked": 11,
            "mark": mark_before,
            "database": last_event,
        })
    );

    // And the mark advanced to that event.
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(done.seq));
}

#[tokio::test]
async fn starting_again_does_not_accuse_the_same_restoration() {
    let (db, home, organization) = installed().await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (device, _) =
        common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    security_event(&db.pool, &home.mark).await;
    lose_events(&home.mark, 3).await;
    let evidence = restored_evidence(&db.pool, &home.mark).await;
    let done = entered(&db.pool, &home.mark, evidence).await;
    let rows = audit_rows(&db.pool).await;

    // A new process: another pool, another handle on the mark.
    let pool = db.second_pool().await;
    let mark = home.second_mark();
    assert_eq!(
        restore::check(&pool, &mark).await.unwrap(),
        Standing::Consistent
    );
    // The evidence that was already handled changes nothing when it is brought again.
    assert_eq!(
        restore::enter(&pool, &mark, LATER + 1, evidence)
            .await
            .unwrap(),
        EnterOutcome::AlreadyHandled
    );
    assert_eq!(audit_rows(&pool).await, rows);
    assert_eq!(mark.read().await.unwrap(), MarkValue::At(done.seq));
    assert_eq!(installation(&pool).await.restored_at, Some(LATER));
    assert_eq!(device_row(&pool, device).await.4, LATER);

    // Security events are written again, above the mark.
    let next = security_event(&pool, &mark).await;
    assert_eq!(next, done.seq + 1);
    assert_eq!(mark.read().await.unwrap(), MarkValue::At(next));
}

#[tokio::test]
async fn the_mark_only_advances_after_the_write_and_a_lost_advance_accuses_nothing() {
    let (db, home, organization) = installed().await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (device, _) =
        common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    security_event(&db.pool, &home.mark).await;
    let mark_before = lose_events(&home.mark, 5).await;
    let evidence = restored_evidence(&db.pool, &home.mark).await;

    // The hub stops between the commit and the mark: here, the mark cannot be written.
    let impatient = Mark::with_lock_patience(&home.data_dir(), Duration::from_millis(50));
    let other_process = arauto_platform::FileLock::try_acquire(
        &home.data_dir().join(arauto_hub::mark::MARK_LOCK_FILE),
    )
    .unwrap();
    let outcome = restore::enter(&db.pool, &impatient, LATER, evidence).await;
    drop(other_process);
    let Err(RestoreError::MarkNotAdvanced { seq, .. }) = outcome else {
        panic!("the mark was written under the lock of another process: {outcome:?}");
    };

    // The write is whole in the database, and the mark is where it was.
    assert_eq!(seq, mark_before + 1);
    assert!(installation(&db.pool).await.restored_mode);
    assert_eq!(device_row(&db.pool, device).await.0, "suspended");
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(mark_before));

    // The database is ahead of the mark again: the next start accuses nothing.
    assert_eq!(
        restore::check(&db.pool, &home.mark).await.unwrap(),
        Standing::Consistent
    );
    assert_eq!(
        restore::enter(&db.pool, &home.mark, LATER + 1, evidence)
            .await
            .unwrap(),
        EnterOutcome::AlreadyHandled
    );
}

#[tokio::test]
async fn a_database_with_data_and_no_mark_came_back_from_a_copy() {
    // The data folder was lost, or the database was put beside a new one.
    let db = TestDb::own().await;
    let home = TestHome::new();
    common::install(&db.pool).await;
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (device, _) =
        common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    // The events the copy brought with it.
    let last: i64 = sqlx::query_scalar(
        "INSERT INTO audit (at, actor_kind, action, result, security) \
         VALUES ($1, 'system', 'device.activated', 'ok', TRUE), \
                ($1 + 1, 'system', 'invite.issued', 'ok', FALSE) \
         RETURNING seq",
    )
    .bind(NOW)
    .fetch_all(&db.pool)
    .await
    .unwrap()
    .into_iter()
    .max()
    .unwrap();

    assert_eq!(
        restored_evidence(&db.pool, &home.mark).await,
        Evidence::MarkAbsent
    );
    let done = entered(&db.pool, &home.mark, Evidence::MarkAbsent).await;

    // The numbering follows the last number of the database, and the mark is created again.
    assert_eq!(done.seq, last + 1);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(done.seq));
    assert_eq!(done.suspended, 1);
    assert_eq!(device_row(&db.pool, device).await.0, "suspended");
    let state = installation(&db.pool).await;
    assert!(state.restored_mode);
    assert_eq!(state.restored_copy_of, Some(NOW + 1));
    assert_eq!(
        restore::check(&db.pool, &home.mark).await.unwrap(),
        Standing::Consistent
    );
}

// ---------------------------------------------------------------------------- a new restoration

#[tokio::test]
async fn a_new_restoration_with_the_mode_on_starts_it_again_for_everyone() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    let old_hermes = common::hermes(pool, organization).await;
    let (old, _) = common::device(pool, organization, old_hermes, "active", Confirmed::Yes).await;
    security_event(pool, &home.mark).await;
    lose_events(&home.mark, 2).await;
    let first = entered(pool, &home.mark, restored_evidence(pool, &home.mark).await).await;
    assert_eq!((first.suspended, first.marked), (1, 1));

    // During the mode: a Hermes reconnects (a new device, not marked), another one starts a
    // request, and an invite is issued.
    let reconnected_hermes = common::hermes(pool, organization).await;
    let (reconnected, _) = common::device(
        pool,
        organization,
        reconnected_hermes,
        "active",
        Confirmed::Yes,
    )
    .await;
    let requesting_hermes = common::hermes(pool, organization).await;
    let (requesting, _) = common::device(
        pool,
        organization,
        requesting_hermes,
        "pending",
        Confirmed::Never,
    )
    .await;
    let (invite, _) = common::invite(pool, organization, old_hermes, "issued", None).await;
    assert!(!device_row(pool, reconnected).await.3);
    security_event(pool, &home.mark).await;

    // The database is restored again.
    let mark_before = lose_events(&home.mark, 4).await;
    let evidence = restored_evidence(pool, &home.mark).await;
    assert!(matches!(evidence, Evidence::BehindMark { .. }));
    let second = restore::enter(pool, &home.mark, LATER + 500, evidence)
        .await
        .unwrap();
    let EnterOutcome::Entered(second) = second else {
        panic!("a new restoration was taken as already handled");
    };

    // It holds for every device, the ones created during the first mode among them.
    assert_eq!(second.seq, mark_before + 1);
    assert_eq!((second.suspended, second.requests_closed), (1, 1));
    assert_eq!(second.marked, 2);
    assert_eq!(second.invites_cancelled, 1);
    assert_eq!(
        device_row(pool, reconnected).await,
        (
            "suspended".to_owned(),
            Some("restored".to_owned()),
            Some("in_use".to_owned()),
            true,
            LATER + 500
        )
    );
    assert_eq!(device_row(pool, requesting).await.0, "expired");
    assert_eq!(
        common::text_of(pool, "invite", "state", invite).await,
        Some("cancelled".to_owned())
    );
    // The device of the first restoration stays suspended since then.
    assert_eq!(device_row(pool, old).await.0, "suspended");
    assert_eq!(device_row(pool, old).await.4, LATER);

    let state = installation(pool).await;
    assert!(state.restored_mode);
    assert_eq!(state.restored_at, Some(LATER + 500));
    let was_on: bool =
        sqlx::query_scalar("SELECT (details->>'mode_was_on')::boolean FROM audit WHERE seq = $1")
            .bind(second.seq)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(was_on);
    assert_eq!(
        restore::check(pool, &home.mark).await.unwrap(),
        Standing::Consistent
    );
}

#[tokio::test]
async fn a_seal_the_database_does_not_explain_turns_the_mode_on_once_per_device() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    let hermes = common::hermes(pool, organization).await;
    let (device, _) = common::device(pool, organization, hermes, "active", Confirmed::Yes).await;
    let before = security_event(pool, &home.mark).await;
    let evidence = Evidence::SealBeyondDatabase { device };

    // The mark says nothing: the whole server came back from an image.
    assert_eq!(
        restore::check(pool, &home.mark).await.unwrap(),
        Standing::Consistent
    );
    let done = entered(pool, &home.mark, evidence).await;
    assert_eq!(done.seq, before + 1);
    assert_eq!((done.suspended, done.marked), (1, 1));
    assert_eq!(device_row(pool, device).await.0, "suspended");
    assert!(installation(pool).await.restored_mode);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(done.seq));
    let target: (Option<String>, Option<Uuid>) =
        sqlx::query_as("SELECT target_kind, target_id FROM audit WHERE seq = $1")
            .bind(done.seq)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(target, (Some("device".to_owned()), Some(device)));

    // The device is marked now: the same seal, brought again, is no proof.
    let rows = audit_rows(pool).await;
    assert_eq!(
        restore::enter(pool, &home.mark, LATER + 1, evidence)
            .await
            .unwrap(),
        EnterOutcome::AlreadyHandled
    );
    assert_eq!(audit_rows(pool).await, rows);

    // A device created after the mode began is proof of another restoration.
    let later_hermes = common::hermes(pool, organization).await;
    let (later_device, _) =
        common::device(pool, organization, later_hermes, "active", Confirmed::Yes).await;
    let again = restore::enter(
        pool,
        &home.mark,
        LATER + 2,
        Evidence::SealBeyondDatabase {
            device: later_device,
        },
    )
    .await
    .unwrap();
    assert!(matches!(again, EnterOutcome::Entered(_)));
    assert_eq!(device_row(pool, later_device).await.0, "suspended");

    // And an evidence about a device that does not exist suspends nobody.
    let rows = audit_rows(pool).await;
    assert!(matches!(
        restore::enter(
            pool,
            &home.mark,
            LATER + 3,
            Evidence::SealBeyondDatabase {
                device: Uuid::new_v4()
            }
        )
        .await,
        Err(RestoreError::NoSuchDevice)
    ));
    assert_eq!(audit_rows(pool).await, rows);
}

// ---------------------------------------------------------------------------- unknown devices

#[tokio::test]
async fn an_unknown_device_with_a_valid_seal_is_listed_once() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    let other_organization = common::organization(pool, &home).await;
    let identity = PublicKey::from_bytes([0x5a; 32]);
    let version = DeclaredText::parse("0.1.0").unwrap();

    let first = list_stranger(
        pool,
        organization,
        &identity,
        Some(&version),
        Some("192.0.2.80".parse().unwrap()),
        NOW,
    )
    .await;
    assert!(first.newly_listed);

    // The same identity again, later and from elsewhere: the same row, not a new one.
    for attempt in 1..4 {
        let again = list_stranger(
            pool,
            organization,
            &identity,
            None,
            Some("198.51.100.7".parse().unwrap()),
            NOW + attempt,
        )
        .await;
        assert_eq!(again.id, first.id);
        assert!(!again.newly_listed);
    }
    let listed: Vec<ListedRow> = sqlx::query_as(
        "SELECT id, public_key, declared, address, seen_at FROM unknown_device \
         WHERE organization_id = $1",
    )
    .bind(organization.id())
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        listed,
        [(
            first.id,
            vec![0x5a; 32],
            serde_json::json!({ "connector_version": "0.1.0" }),
            Some("192.0.2.80".to_owned()),
            NOW
        )]
    );
    let recorded: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit WHERE action = 'unknown_device.listed'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(recorded, 1);

    // The list is of each organization.
    let elsewhere = list_stranger(pool, other_organization, &identity, None, None, NOW).await;
    assert!(elsewhere.newly_listed);
    assert_ne!(elsewhere.id, first.id);
}

#[tokio::test]
async fn an_unknown_device_is_proof_only_while_the_mode_is_off() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    let hermes = common::hermes(pool, organization).await;
    let (device, _) = common::device(pool, organization, hermes, "active", Confirmed::Yes).await;
    security_event(pool, &home.mark).await;

    let first = list_stranger(
        pool,
        organization,
        &PublicKey::from_bytes([1; 32]),
        None,
        None,
        NOW,
    )
    .await;
    assert!(first.proof_pending);
    let done = entered(
        pool,
        &home.mark,
        Evidence::UnknownDeviceSeal {
            unknown_device: first.id,
        },
    )
    .await;
    assert_eq!(done.suspended, 1);
    assert_eq!(device_row(pool, device).await.0, "suspended");

    // With the mode on, a Hermes reconnects; then another unknown device shows up. It is
    // listed, and the Hermes that just reconnected is not suspended again.
    let reconnected_hermes = common::hermes(pool, organization).await;
    let (reconnected, _) = common::device(
        pool,
        organization,
        reconnected_hermes,
        "active",
        Confirmed::Yes,
    )
    .await;
    let second = list_stranger(
        pool,
        organization,
        &PublicKey::from_bytes([2; 32]),
        None,
        None,
        NOW,
    )
    .await;
    assert!(second.newly_listed);
    assert!(
        !second.proof_pending,
        "with the mode on, an unknown device is listed and is no proof"
    );
    assert_eq!(
        restore::enter(
            pool,
            &home.mark,
            LATER + 1,
            Evidence::UnknownDeviceSeal {
                unknown_device: second.id
            }
        )
        .await
        .unwrap(),
        EnterOutcome::AlreadyHandled
    );
    assert_eq!(device_row(pool, reconnected).await.0, "active");
}

/// C11: after a restoration of the whole server by image, an unknown device is the only way
/// the hub has of noticing. The listing commits before the mode is turned on, so a hub that
/// stops between the two must find the proof again.
#[tokio::test]
async fn the_proof_of_an_unknown_device_stays_until_the_mode_is_turned_on() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    let hermes = common::hermes(pool, organization).await;
    let (device, _) = common::device(pool, organization, hermes, "active", Confirmed::Yes).await;
    security_event(pool, &home.mark).await;
    let identity = PublicKey::from_bytes([7; 32]);

    let first = list_stranger(pool, organization, &identity, None, None, NOW).await;
    assert!(first.newly_listed && first.proof_pending);
    // The hub stops here, before turning the mode on. The same device presents again, as
    // many times as it takes: it is still proof.
    for attempt in 1..4 {
        let again = list_stranger(pool, organization, &identity, None, None, NOW + attempt).await;
        assert_eq!(again.id, first.id);
        assert!(!again.newly_listed);
        assert!(again.proof_pending, "the proof was lost with the listing");
    }
    assert!(!installation(pool).await.restored_mode);
    assert_eq!(device_row(pool, device).await.0, "active");

    // The mode is turned on with it, once.
    let evidence = Evidence::UnknownDeviceSeal {
        unknown_device: first.id,
    };
    let done = entered(pool, &home.mark, evidence).await;
    assert_eq!(done.suspended, 1);
    let after = list_stranger(pool, organization, &identity, None, None, LATER + 1).await;
    assert!(!after.proof_pending, "a handled listing is no proof again");
    assert_eq!(
        restore::enter(pool, &home.mark, LATER + 2, evidence)
            .await
            .unwrap(),
        EnterOutcome::AlreadyHandled
    );

    // Neither is it after the mode ended: the device is not suspended a second time by an
    // identity that was already listed.
    sqlx::query("UPDATE device SET state = 'revoked', slot = NULL WHERE id = $1")
        .bind(device)
        .execute(pool)
        .await
        .unwrap();
    assert!(
        restore::end_when_done(pool, &home.mark, LATER + 3)
            .await
            .unwrap()
    );
    let later = list_stranger(pool, organization, &identity, None, None, LATER + 4).await;
    assert!(!later.proof_pending);
    assert_eq!(
        restore::enter(pool, &home.mark, LATER + 5, evidence)
            .await
            .unwrap(),
        EnterOutcome::AlreadyHandled
    );
    // And an evidence about a listing that does not exist turns nothing on.
    assert_eq!(
        restore::enter(
            pool,
            &home.mark,
            LATER + 6,
            Evidence::UnknownDeviceSeal {
                unknown_device: Uuid::new_v4()
            }
        )
        .await
        .unwrap(),
        EnterOutcome::AlreadyHandled
    );
    assert!(!installation(pool).await.restored_mode);
}

#[tokio::test]
async fn turning_the_mode_on_handles_every_proof_listed_so_far() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    security_event(pool, &home.mark).await;
    let one = list_stranger(
        pool,
        organization,
        &PublicKey::from_bytes([8; 32]),
        None,
        None,
        NOW,
    )
    .await;
    let two = list_stranger(
        pool,
        organization,
        &PublicKey::from_bytes([9; 32]),
        None,
        None,
        NOW,
    )
    .await;
    assert!(one.proof_pending && two.proof_pending);
    // A request the restored mode closed is listed too, and is never proof.
    let closed_request = restore::list_unknown_device(
        pool,
        organization,
        &Listing {
            identity: &PublicKey::from_bytes([10; 32]),
            connector_version: None,
            address: None,
            listed_as: ListedAs::ClosedRequest,
        },
        NOW,
    )
    .await
    .unwrap();
    assert!(closed_request.newly_listed && !closed_request.proof_pending);

    entered(
        pool,
        &home.mark,
        Evidence::UnknownDeviceSeal {
            unknown_device: one.id,
        },
    )
    .await;
    // The other one was of the same restoration: it does not start the mode again.
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM unknown_device WHERE proof_pending")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    assert_eq!(
        restore::enter(
            pool,
            &home.mark,
            LATER + 1,
            Evidence::UnknownDeviceSeal {
                unknown_device: two.id
            }
        )
        .await
        .unwrap(),
        EnterOutcome::AlreadyHandled
    );
}

// ---------------------------------------------------------------------------- the write of the mode

/// The mark was absent and could not be created after the mode was turned on: the database is
/// accused again, and the second call must only create the mark.
#[tokio::test]
async fn a_mark_that_was_absent_and_did_not_follow_is_created_without_a_second_restoration() {
    let db = TestDb::own().await;
    let home = TestHome::new();
    common::install(&db.pool).await;
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (device, _) =
        common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    // The events the copy brought with it; the data folder, with the mark, was lost.
    sqlx::query(
        "INSERT INTO audit (at, actor_kind, action, result, security) \
         VALUES ($1, 'system', 'device.activated', 'ok', TRUE)",
    )
    .bind(NOW)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        restored_evidence(&db.pool, &home.mark).await,
        Evidence::MarkAbsent
    );

    // The mode is turned on, and the mark cannot be written.
    let impatient = Mark::with_lock_patience(&home.data_dir(), Duration::from_millis(50));
    let other_process = arauto_platform::FileLock::try_acquire(
        &home.data_dir().join(arauto_hub::mark::MARK_LOCK_FILE),
    )
    .unwrap();
    let outcome = restore::enter(&db.pool, &impatient, LATER, Evidence::MarkAbsent).await;
    drop(other_process);
    let Err(RestoreError::MarkNotAdvanced { seq, .. }) = outcome else {
        panic!("the mark was written under the lock of another process: {outcome:?}");
    };
    let state = installation(&db.pool).await;
    assert!(state.restored_mode);
    assert_eq!(state.restored_copy_of, Some(NOW));
    assert_eq!(device_row(&db.pool, device).await.0, "suspended");
    let rows = audit_rows(&db.pool).await;

    // The mark is still absent, so the database is still accused and nothing is written...
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::Absent);
    assert_eq!(
        restored_evidence(&db.pool, &home.mark).await,
        Evidence::MarkAbsent
    );
    assert!(matches!(
        audit::begin_security(&db.pool, &home.mark).await,
        Err(AuditError::Restored(Evidence::MarkAbsent))
    ));

    // ...until the hub handles that evidence: only the mark is created, at the event that
    // turned the mode on. No second restoration, and the date of the copy is the same.
    assert_eq!(
        restore::enter(
            &db.pool,
            &home.mark,
            LATER + 86_400_000,
            Evidence::MarkAbsent
        )
        .await
        .unwrap(),
        EnterOutcome::AlreadyHandled
    );
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(seq));
    assert_eq!(audit_rows(&db.pool).await, rows);
    let entered_events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit WHERE action = 'restored_mode.entered'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(entered_events, 1);
    assert_eq!(installation(&db.pool).await, state);
    assert_eq!(device_row(&db.pool, device).await.4, LATER);
    assert_eq!(
        restore::check(&db.pool, &home.mark).await.unwrap(),
        Standing::Consistent
    );
    // Security events are written again.
    let next = security_event(&db.pool, &home.mark).await;
    assert_eq!(next, seq + 1);

    // A mark that goes missing after other security events is a restoration like any other.
    home.mark.erase().await.unwrap();
    let again = entered(&db.pool, &home.mark, Evidence::MarkAbsent).await;
    assert_eq!(again.seq, next + 1);
}

/// The sequence of the audit only moves forward. With ordinary events numbered past the mark,
/// moving it "to the mark" would give the next event a number that is already taken.
#[tokio::test]
async fn the_numbering_of_the_audit_is_never_moved_back() {
    let (db, home, _) = installed().await;
    let pool = &db.pool;
    let last_security = security_event(pool, &home.mark).await;
    let mark = lose_events(&home.mark, 2).await;
    // Ordinary events do not move the mark, and their numbers went past it.
    let mut last = 0;
    for _ in 0..5 {
        last = audit::record(pool, NOW, &Event::new(Action::InviteIssued, Actor::System))
            .await
            .unwrap();
    }
    assert!(last > mark, "the test needs numbers past the mark");

    let evidence = restored_evidence(pool, &home.mark).await;
    assert_eq!(
        evidence,
        Evidence::BehindMark {
            mark,
            database: last_security
        }
    );
    let done = entered(pool, &home.mark, evidence).await;
    assert_eq!(done.seq, last + 1);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(done.seq));
    assert_eq!(security_event(pool, &home.mark).await, done.seq + 1);
}

/// Rule 7 of the database: turning the mode on is a security write, and its commit waits for
/// the disk whatever the session says.
#[tokio::test]
async fn turning_the_mode_on_is_a_durable_write() {
    let (db, home, _) = installed().await;
    security_event(&db.pool, &home.mark).await;
    lose_events(&home.mark, 1).await;
    // Every session of this database commits without waiting for the disk, unless the
    // transaction says otherwise; and what the transaction of `enter` says is written down
    // by the row it changes.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER DATABASE \"{}\" SET synchronous_commit = off",
        db.name()
    )))
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE commit_probe (setting TEXT NOT NULL); \
         CREATE FUNCTION probe_commit() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN \
             INSERT INTO commit_probe VALUES (current_setting('synchronous_commit')); \
             RETURN NEW; \
         END; $$; \
         CREATE TRIGGER probe_commit BEFORE UPDATE ON installation \
             FOR EACH ROW EXECUTE FUNCTION probe_commit();",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let pool = db.second_pool().await;
    let session_default: String = sqlx::query_scalar("SHOW synchronous_commit")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        session_default, "off",
        "the test needs a session that does not wait"
    );

    let evidence = restored_evidence(&pool, &home.mark).await;
    entered(&pool, &home.mark, evidence).await;
    let seen: Vec<String> = sqlx::query_scalar("SELECT setting FROM commit_probe")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(seen, ["on"], "the write of the restored mode is durable");
}

/// Rule 6 of the database: every write that changes devices takes the rows of the Hermes
/// first. Turning the mode on changes the devices of every Hermes.
#[tokio::test]
async fn turning_the_mode_on_waits_for_whoever_holds_a_hermes() {
    let (db, home, organization) = installed().await;
    let hermes = common::hermes(&db.pool, organization).await;
    let (device, _) =
        common::device(&db.pool, organization, hermes, "active", Confirmed::Yes).await;
    security_event(&db.pool, &home.mark).await;
    lose_events(&home.mark, 1).await;
    let evidence = restored_evidence(&db.pool, &home.mark).await;

    // Another writer is in the middle of a change to that Hermes: it holds the row of the
    // Hermes, and only that.
    let other_pool = db.second_pool().await;
    let mut holder = other_pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM hermes WHERE id = $1 FOR NO KEY UPDATE")
        .bind(hermes)
        .execute(&mut *holder)
        .await
        .unwrap();

    let waiting = {
        let pool = db.pool.clone();
        let mark = home.mark.clone();
        tokio::spawn(async move { restore::enter(&pool, &mark, LATER, evidence).await })
    };
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !waiting.is_finished(),
        "the restored mode changed the devices of a Hermes another writer holds"
    );
    assert_eq!(device_row(&other_pool, device).await.0, "active");

    holder.commit().await.unwrap();
    assert!(matches!(
        waiting.await.unwrap().unwrap(),
        EnterOutcome::Entered(_)
    ));
    assert_eq!(device_row(&db.pool, device).await.0, "suspended");
}

// ---------------------------------------------------------------------------- the end of the mode

#[tokio::test]
async fn the_mode_ends_when_no_suspended_device_is_left() {
    let (db, home, organization) = installed().await;
    let pool = &db.pool;
    let first_hermes = common::hermes(pool, organization).await;
    let second_hermes = common::hermes(pool, organization).await;
    let (first, _) =
        common::device(pool, organization, first_hermes, "active", Confirmed::Yes).await;
    let (second, _) =
        common::device(pool, organization, second_hermes, "active", Confirmed::Yes).await;

    // With the mode off there is nothing to end.
    assert!(
        !restore::end_when_done(pool, &home.mark, LATER)
            .await
            .unwrap()
    );

    security_event(pool, &home.mark).await;
    lose_events(&home.mark, 1).await;
    let done = entered(pool, &home.mark, restored_evidence(pool, &home.mark).await).await;
    assert_eq!(done.suspended, 2);

    let leave_suspended = |device: Uuid, state: &'static str| async move {
        sqlx::query("UPDATE device SET state = $2, slot = NULL WHERE id = $1")
            .bind(device)
            .bind(state)
            .execute(pool)
            .await
            .unwrap();
    };
    // One Hermes is revoked: the other is still suspended.
    leave_suspended(first, "revoked").await;
    assert!(
        !restore::end_when_done(pool, &home.mark, LATER + 1)
            .await
            .unwrap()
    );
    assert!(installation(pool).await.restored_mode);
    let rows = audit_rows(pool).await;

    // The other one reconnects: its old device is replaced.
    leave_suspended(second, "replaced").await;
    assert!(
        restore::end_when_done(pool, &home.mark, LATER + 2)
            .await
            .unwrap()
    );
    assert_eq!(
        installation(pool).await,
        Installation {
            created_at: NOW,
            restored_mode: false,
            restored_at: None,
            restored_copy_of: None,
        }
    );
    assert_eq!(audit_rows(pool).await, rows + 1);
    let (seq, action, security): (i64, String, bool) =
        sqlx::query_as("SELECT seq, action, security FROM audit ORDER BY seq DESC LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!((action.as_str(), security), ("restored_mode.ended", true));
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(seq));

    // Once.
    assert!(
        !restore::end_when_done(pool, &home.mark, LATER + 3)
            .await
            .unwrap()
    );
    assert_eq!(audit_rows(pool).await, rows + 1);
}

#[tokio::test]
async fn the_mode_does_not_end_on_a_database_that_was_restored_again() {
    let (db, home, _) = installed().await;
    security_event(&db.pool, &home.mark).await;
    lose_events(&home.mark, 1).await;
    entered(
        &db.pool,
        &home.mark,
        restored_evidence(&db.pool, &home.mark).await,
    )
    .await;
    // No device at all is suspended, so the mode could end; but the database is behind the
    // mark again, and ending is a security write like any other.
    lose_events(&home.mark, 1).await;
    assert!(matches!(
        restore::end_when_done(&db.pool, &home.mark, LATER).await,
        Err(RestoreError::Audit(AuditError::Restored(
            Evidence::BehindMark { .. }
        )))
    ));
    assert!(installation(&db.pool).await.restored_mode);
}
