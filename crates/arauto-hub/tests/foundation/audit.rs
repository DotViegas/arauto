//! The audit: append-only, and the security write against the mark.

use arauto_hub::audit::{
    self, Action, Actor, AuditError, Details, Event, Outcome, SecurityWrite, TargetKind,
};
use arauto_hub::mark::{Mark, MarkValue};
use arauto_hub::restore::Evidence;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::DeclaredText;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::common::{self, NOW, TestDb, TestHome};

/// A database of its own with the installation row and the mark at zero, as after `init`.
async fn installed() -> (TestDb, TestHome) {
    let db = TestDb::own().await;
    let home = TestHome::new();
    common::install(&db.pool).await;
    home.mark.advance(0).await.unwrap();
    (db, home)
}

fn revoked(device: Uuid) -> Event {
    Event::new(Action::DeviceRevoked, Actor::System).about(TargetKind::Device, device)
}

async fn write_security_event(pool: &PgPool, mark: &Mark) -> i64 {
    let mut write = audit::begin_security(pool, mark).await.unwrap();
    write.record(NOW, &revoked(Uuid::new_v4())).await.unwrap();
    write.commit().await.unwrap().unwrap()
}

async fn rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn restored(outcome: Result<SecurityWrite, AuditError>) -> Evidence {
    match outcome {
        Err(AuditError::Restored(evidence)) => evidence,
        Err(other) => panic!("another error: {other}"),
        Ok(_) => panic!("a security write began on a restored database"),
    }
}

// ---------------------------------------------------------------------------- append-only

#[tokio::test]
async fn the_audit_cannot_be_changed_erased_or_emptied() {
    let (db, home) = installed().await;
    let ordinary = audit::record(
        &db.pool,
        NOW,
        &Event::new(Action::InviteIssued, Actor::System),
    )
    .await
    .unwrap();
    let security = write_security_event(&db.pool, &home.mark).await;
    assert_eq!(rows(&db.pool).await, 2);

    let attempts = [
        "UPDATE audit SET action = 'nothing.happened'",
        "UPDATE audit SET security = FALSE WHERE security",
        "UPDATE audit SET details = '{}'::jsonb",
        "DELETE FROM audit",
        "DELETE FROM audit WHERE NOT security",
        "TRUNCATE audit",
        "TRUNCATE audit CASCADE",
        "TRUNCATE audit RESTART IDENTITY",
    ];
    for attempt in attempts {
        let refused = sqlx::query(attempt).execute(&db.pool).await;
        let error = refused.expect_err(attempt).to_string();
        assert!(error.contains("audit is append-only"), "{attempt}: {error}");
    }
    // Inside a transaction too, and the rest of it goes with the attempt.
    let mut transaction = db.pool.begin().await.unwrap();
    assert!(
        sqlx::query("DELETE FROM audit WHERE seq = $1")
            .bind(security)
            .execute(&mut *transaction)
            .await
            .is_err()
    );
    drop(transaction);

    let left: Vec<(i64, String, bool)> =
        sqlx::query_as("SELECT seq, action, security FROM audit ORDER BY seq")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        left,
        [
            (ordinary, "invite.issued".to_owned(), false),
            (security, "device.revoked".to_owned(), true)
        ]
    );
}

// ---------------------------------------------------------------------------- ordinary events

#[tokio::test]
async fn an_ordinary_event_is_one_row_with_what_was_given() {
    let db = TestDb::shared().await;
    let home = TestHome::new();
    let organization = common::organization(&db.pool, &home).await;
    let admin = Uuid::new_v4();
    let invite = Uuid::new_v4();
    let event = Event::new(
        Action::InviteRefused,
        Actor::Admin {
            id: admin,
            os_user: DeclaredText::parse("operator").unwrap(),
        },
    )
    .in_organization(organization)
    .about(TargetKind::Invite, invite)
    .from_address("192.0.2.44".parse().unwrap())
    .ended(Outcome::Denied, Some(ErrorCode::Ar101))
    .with(Details::new().number("count", 12).word("reason", "unknown"));
    let seq = audit::record(&db.pool, NOW + 5, &event).await.unwrap();

    let who: (Option<Uuid>, i64, String, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT organization_id, at, actor_kind, actor_id, actor_note FROM audit WHERE seq = $1",
    )
    .bind(seq)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        who,
        (
            Some(organization.id()),
            NOW + 5,
            "admin".to_owned(),
            Some(admin),
            Some("operator".to_owned()),
        )
    );
    #[allow(clippy::type_complexity)]
    let what: (
        String,
        Option<String>,
        Option<Uuid>,
        Option<String>,
        String,
        Option<String>,
        bool,
        serde_json::Value,
    ) = sqlx::query_as(
        "SELECT action, target_kind, target_id, address, result, code, security, details          FROM audit WHERE seq = $1",
    )
    .bind(seq)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        what,
        (
            "invite.refused".to_owned(),
            Some("invite".to_owned()),
            Some(invite),
            Some("192.0.2.44".to_owned()),
            "denied".to_owned(),
            Some("AR-101".to_owned()),
            false,
            serde_json::json!({ "count": 12, "reason": "unknown" }),
        )
    );
}

#[tokio::test]
async fn the_actors_are_stored_as_the_schema_names_them() {
    let db = TestDb::shared().await;
    let device = Uuid::new_v4();
    let by_device = audit::record(
        &db.pool,
        NOW,
        &Event::new(Action::CredentialUnconfirmed, Actor::Device { id: device }),
    )
    .await
    .unwrap();
    let by_system = audit::record(
        &db.pool,
        NOW,
        &Event::new(Action::InviteExpired, Actor::System),
    )
    .await
    .unwrap();
    let actor = |seq: i64| {
        let pool = db.pool.clone();
        async move {
            sqlx::query_as::<_, (String, Option<Uuid>, Option<String>, Option<Uuid>)>(
                "SELECT actor_kind, actor_id, actor_note, organization_id FROM audit WHERE seq = $1",
            )
            .bind(seq)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(
        actor(by_device).await,
        ("device".to_owned(), Some(device), None, None)
    );
    assert_eq!(
        actor(by_system).await,
        ("system".to_owned(), None, None, None)
    );
    assert!(by_system > by_device);
}

#[tokio::test]
async fn a_security_event_is_refused_outside_a_security_write() {
    let db = TestDb::shared().await;
    for action in Action::ALL.iter().filter(|action| action.is_security()) {
        let refused = audit::record(&db.pool, NOW, &Event::new(*action, Actor::System)).await;
        assert!(
            matches!(refused, Err(AuditError::NeedsSecurityWrite(refused)) if refused == *action),
            "{action}"
        );
    }
}

// ---------------------------------------------------------------------------- security events

#[tokio::test]
async fn a_security_event_is_written_and_only_then_the_mark_advances() {
    let (db, home) = installed().await;
    let device = Uuid::new_v4();

    let mut write = audit::begin_security(&db.pool, &home.mark).await.unwrap();
    let seq = write.record(NOW, &revoked(device)).await.unwrap();
    // Not committed yet: nobody else sees the event, and the mark has not moved.
    assert_eq!(rows(&db.pool).await, 0);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(0));

    assert_eq!(write.commit().await.unwrap(), Some(seq));
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(seq));
    let stored: (String, bool, Option<Uuid>) =
        sqlx::query_as("SELECT action, security, target_id FROM audit WHERE seq = $1")
            .bind(seq)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(stored, ("device.revoked".to_owned(), true, Some(device)));
}

#[tokio::test]
async fn the_security_write_commits_synchronously_and_carries_the_change_with_it() {
    let (db, home) = installed().await;
    let organization = common::organization(&db.pool, &home).await;
    let hermes = common::hermes(&db.pool, organization).await;

    let mut write = audit::begin_security(&db.pool, &home.mark).await.unwrap();
    let setting: String = sqlx::query_scalar("SHOW synchronous_commit")
        .fetch_one(write.connection())
        .await
        .unwrap();
    assert_eq!(setting, "on");
    // The change and its event are one transaction.
    sqlx::query("UPDATE hermes SET can_be_woken = TRUE WHERE id = $1")
        .bind(hermes)
        .execute(write.connection())
        .await
        .unwrap();
    write
        .record(
            NOW,
            &Event::new(Action::HermesWakeChanged, Actor::System)
                .in_organization(organization)
                .about(TargetKind::Hermes, hermes),
        )
        .await
        .unwrap();
    drop(write);

    // Dropped without commit: neither the change nor the event, and the mark stays.
    let can_be_woken: bool = sqlx::query_scalar("SELECT can_be_woken FROM hermes WHERE id = $1")
        .bind(hermes)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(!can_be_woken);
    assert_eq!(rows(&db.pool).await, 0);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(0));

    let mut write = audit::begin_security(&db.pool, &home.mark).await.unwrap();
    write
        .record(NOW, &Event::new(Action::HermesWakeChanged, Actor::System))
        .await
        .unwrap();
    write.rollback().await.unwrap();
    assert_eq!(rows(&db.pool).await, 0);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(0));
}

#[tokio::test]
async fn ordinary_events_in_a_security_write_do_not_move_the_mark() {
    let (db, home) = installed().await;
    let first = write_security_event(&db.pool, &home.mark).await;

    let mut write = audit::begin_security(&db.pool, &home.mark).await.unwrap();
    let ordinary = write
        .record(NOW, &Event::new(Action::InviteIssued, Actor::System))
        .await
        .unwrap();
    assert!(ordinary > first);
    assert_eq!(write.commit().await.unwrap(), None);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(first));

    // With both kinds, the mark goes to the last security event, not to the last event.
    let mut write = audit::begin_security(&db.pool, &home.mark).await.unwrap();
    let security = write.record(NOW, &revoked(Uuid::new_v4())).await.unwrap();
    let after = write
        .record(NOW, &Event::new(Action::InviteIssued, Actor::System))
        .await
        .unwrap();
    assert!(after > security);
    assert_eq!(write.commit().await.unwrap(), Some(security));
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(security));
}

#[tokio::test]
async fn a_database_behind_the_mark_is_restored_and_nothing_is_written() {
    let (db, home) = installed().await;
    let last = write_security_event(&db.pool, &home.mark).await;
    let before = rows(&db.pool).await;

    // The database of an older copy: the mark knows of events it does not have.
    home.mark.advance(last + 10).await.unwrap();

    let evidence = restored(audit::begin_security(&db.pool, &home.mark).await);
    assert_eq!(
        evidence,
        Evidence::BehindMark {
            mark: last + 10,
            database: last
        }
    );
    assert_eq!(rows(&db.pool).await, before);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(last + 10));

    // The admin command, another process with its own pool and its own handle on the mark,
    // run before the hub: the same answer, and the proof is still there afterwards.
    let admin_pool = db.second_pool().await;
    let admin_mark = home.second_mark();
    for _ in 0..3 {
        let evidence = restored(audit::begin_security(&admin_pool, &admin_mark).await);
        assert!(matches!(evidence, Evidence::BehindMark { .. }));
    }
    assert_eq!(rows(&db.pool).await, before);
    assert_eq!(admin_mark.read().await.unwrap(), MarkValue::At(last + 10));
}

#[tokio::test]
async fn one_event_behind_is_enough() {
    let (db, home) = installed().await;
    let last = write_security_event(&db.pool, &home.mark).await;
    // Ordinary events after it do not count: the mark follows the security events.
    audit::record(
        &db.pool,
        NOW,
        &Event::new(Action::InviteIssued, Actor::System),
    )
    .await
    .unwrap();
    home.mark.advance(last + 1).await.unwrap();
    let evidence = restored(audit::begin_security(&db.pool, &home.mark).await);
    assert_eq!(
        evidence,
        Evidence::BehindMark {
            mark: last + 1,
            database: last
        }
    );
}

#[tokio::test]
async fn a_database_ahead_of_the_mark_accuses_nothing() {
    // The hub stopped between the commit and the mark: the mark is one event behind.
    let (db, home) = installed().await;
    let first = write_security_event(&db.pool, &home.mark).await;
    sqlx::query(
        "INSERT INTO audit (at, actor_kind, action, result, security) \
         VALUES ($1, 'system', 'device.revoked', 'ok', TRUE)",
    )
    .bind(NOW)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(first));

    let next = write_security_event(&db.pool, &home.mark).await;
    assert_eq!(next, first + 2);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(next));
}

#[tokio::test]
async fn a_database_with_data_and_no_mark_is_restored() {
    let db = TestDb::own().await;
    let home = TestHome::new();
    common::install(&db.pool).await;
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::Absent);

    let evidence = restored(audit::begin_security(&db.pool, &home.mark).await);
    assert_eq!(evidence, Evidence::MarkAbsent);
    assert_eq!(rows(&db.pool).await, 0);
    // The check does not create the mark: that would erase the proof.
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::Absent);
}

#[tokio::test]
async fn before_init_there_is_nothing_to_write_on() {
    let db = TestDb::own().await;
    let home = TestHome::new();
    assert!(matches!(
        audit::begin_security(&db.pool, &home.mark).await,
        Err(AuditError::NotInstalled)
    ));
    // `init` creates the mark at zero first, and then its first event goes in.
    home.mark.advance(0).await.unwrap();
    let mut write = audit::begin_security(&db.pool, &home.mark).await.unwrap();
    let seq = write
        .record(NOW, &Event::new(Action::InstallationCreated, Actor::System))
        .await
        .unwrap();
    assert_eq!(write.commit().await.unwrap(), Some(seq));
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(seq));
}

#[tokio::test]
async fn a_mark_that_cannot_be_written_does_not_undo_the_event() {
    let (db, home) = installed().await;
    let impatient =
        Mark::with_lock_patience(&home.data_dir(), std::time::Duration::from_millis(50));
    let mut write = audit::begin_security(&db.pool, &impatient).await.unwrap();
    let seq = write.record(NOW, &revoked(Uuid::new_v4())).await.unwrap();

    let other_process = arauto_platform::FileLock::try_acquire(
        &home.data_dir().join(arauto_hub::mark::MARK_LOCK_FILE),
    )
    .unwrap();
    let outcome = write.commit().await;
    drop(other_process);

    let Err(AuditError::MarkNotAdvanced { seq: committed, .. }) = outcome else {
        panic!("the mark was written under the lock of another process: {outcome:?}");
    };
    assert_eq!(committed, seq);
    assert_eq!(rows(&db.pool).await, 1);
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(0));
    // One event behind is not a restoration, and the next event brings the mark up.
    let next = write_security_event(&db.pool, &home.mark).await;
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(next));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_security_writes_leave_the_mark_at_the_last_one() {
    let (db, home) = installed().await;
    let mut tasks = Vec::new();
    for _ in 0..24 {
        let pool = db.pool.clone();
        let mark = home.mark.clone();
        tasks.push(tokio::spawn(async move {
            write_security_event(&pool, &mark).await
        }));
    }
    let mut numbers = Vec::new();
    for task in tasks {
        numbers.push(task.await.unwrap());
    }
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), 24);
    let last = *numbers.last().unwrap();
    assert_eq!(home.mark.read().await.unwrap(), MarkValue::At(last));
    assert_eq!(rows(&db.pool).await, 24);
    assert!(home.mark.writes() <= 25);
}
