//! The enrollment: every line of the table of C3, and the refusals that are counted.

use arauto_hub::core::model::{DeviceState, InviteState, Slot, StateReason};
use arauto_hub::repo::approval::ApproveOutcome;
use arauto_hub::repo::enroll::{self, CONFIRM_WINDOW_MS, EnrollOutcome, Refusal};
use arauto_hub::repo::invite::IssuedInvite;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::MINUTE_MS;
use uuid::Uuid;

use crate::support::{World, address, another_code, identity};

/// A row of the audit about refused enrollments: the organization, when, the origin, the
/// result, the code, whether it is a security event, and the details.
type RefusedRow = (
    Option<Uuid>,
    i64,
    Option<String>,
    String,
    Option<String>,
    bool,
    serde_json::Value,
);

fn refusal(outcome: &EnrollOutcome) -> Refusal {
    match outcome {
        EnrollOutcome::Refused(refusal) => *refusal,
        EnrollOutcome::Enrolled(accepted) => panic!("it was accepted: {accepted:?}"),
    }
}

fn code_of(outcome: &EnrollOutcome) -> ErrorCode {
    refusal(outcome).code
}

async fn invite_state(world: &World, issued: &IssuedInvite) -> (String, Option<Uuid>, Option<i64>) {
    sqlx::query_as(
        "SELECT state, device_id, used_at FROM invite WHERE organization_id = $1 AND id = $2",
    )
    .bind(world.org.id())
    .bind(issued.id)
    .fetch_one(world.pool())
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------- line 1

#[tokio::test]
async fn an_issued_invite_within_its_time_creates_the_pending_device() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let issued = world.invite(&name).await;
    world.clock.advance(14 * MINUTE_MS + 59_999);

    let who = identity();
    let (outcome, code) = world.enroll_from(&issued, &who, address(33)).await;
    let EnrollOutcome::Enrolled(accepted) = outcome else {
        panic!("refused: {outcome:?}");
    };
    assert_eq!(accepted.hermes, hermes);
    assert_eq!(accepted.reply.enrollment.get(), 1);
    assert_eq!(accepted.reply.confirm_within_s.get(), 600);
    assert!(!accepted.code_replaced);

    let device = world.device(accepted.device).await;
    assert_eq!(device.public_key, who);
    assert_eq!(
        (device.state, device.slot, device.state_reason),
        (DeviceState::Pending, Some(Slot::Request), None)
    );
    assert_eq!(device.verification_code, Some(code));
    assert_eq!(device.confirm_until, Some(world.now() + CONFIRM_WINDOW_MS));
    assert_eq!((device.enrollments, device.code_attempts), (1, 0));
    assert_eq!(device.declared.hostname.as_str(), "build-7");
    assert_eq!(device.enrolled_from.as_deref(), Some("192.0.2.33"));
    assert_eq!(device.connector_version.as_deref(), Some("0.1.0"));
    assert_eq!(
        (device.current, device.pending, device.max_generation),
        (None, None, 0)
    );

    // The invite is used, by that device, and that is a security event.
    assert_eq!(
        invite_state(&world, &issued).await,
        (
            InviteState::Used.as_str().to_owned(),
            Some(accepted.device),
            Some(world.now())
        )
    );
    let used: (bool, Option<String>) = sqlx::query_as(
        "SELECT security, address FROM audit WHERE organization_id = $1 AND action = 'invite.used' \
         AND target_id = $2",
    )
    .bind(world.org.id())
    .bind(issued.id)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(used, (true, Some("192.0.2.33".to_owned())));
}

#[tokio::test]
async fn an_invite_past_its_fifteen_minutes_is_refused_without_saying_why() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let issued_at = world.now();
    let issued = world.invite(&name).await;
    world.clock.advance(15 * MINUTE_MS);

    let (outcome, _) = world.enroll(&issued, &identity()).await;
    assert_eq!(
        refusal(&outcome),
        Refusal {
            code: ErrorCode::Ar101,
            closed: None
        }
    );
    // It expires right there, where it was met, as the sweep would have done: the hub does
    // not wait for the sweep to erase the digest (C1).
    let stored = || async {
        sqlx::query_as::<_, (String, Option<Vec<u8>>)>(
            "SELECT state, digest FROM invite WHERE organization_id = $1 AND id = $2",
        )
        .bind(world.org.id())
        .bind(issued.id)
        .fetch_one(world.pool())
        .await
        .unwrap()
    };
    assert_eq!(stored().await, ("expired".to_owned(), None));
    assert_eq!(
        world.actions_about(issued.id).await,
        ["invite.issued", "invite.expired"]
    );
    assert_eq!(world.refusals.waiting(), 1);

    // A clock that goes back does not make it hold again.
    world.clock.set(issued_at + 1);
    let rows = world.audit_rows().await;
    let (outcome, _) = world.enroll(&issued, &identity()).await;
    assert_eq!(
        refusal(&outcome),
        Refusal {
            code: ErrorCode::Ar101,
            closed: None
        }
    );
    assert_eq!(stored().await, ("expired".to_owned(), None));
    assert_eq!(
        world.audit_rows().await,
        rows,
        "a refusal is not a row of its own"
    );
    assert!(
        arauto_hub::repo::hermes::list(world.pool(), world.org, false)
            .await
            .unwrap()
            .iter()
            .all(|line| line.request.is_none()),
        "no request was created"
    );
}

// ---------------------------------------------------------------------------- line 2

#[tokio::test]
async fn the_same_identity_enrolling_again_is_the_same_request_with_the_newest_code() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    let (first, first_code) = world.enroll(&issued, &who).await;
    let EnrollOutcome::Enrolled(first) = first else {
        panic!("refused");
    };
    let deadline = world.device(first.device).await.confirm_until;
    // One wrong code, to see that the attempts are not reset either.
    assert_eq!(
        world.approve(&name, another_code(first_code), None).await,
        ApproveOutcome::WrongCode { attempts: 1 }
    );

    world.clock.advance(4 * MINUTE_MS + 500);
    let (second, second_code) = world.enroll(&issued, &who).await;
    let EnrollOutcome::Enrolled(second) = second else {
        panic!("refused: {second:?}");
    };
    assert_eq!(second.device, first.device);
    assert_eq!(second.reply.enrollment.get(), 2);
    assert!(second.code_replaced);
    // What is left of the same 10 minutes: 5 min 59.5 s, rounded up.
    assert_eq!(second.reply.confirm_within_s.get(), 360);

    let device = world.device(first.device).await;
    assert_eq!(device.state, DeviceState::Pending);
    assert_eq!(device.confirm_until, deadline, "the deadline does not move");
    assert_eq!(device.code_attempts, 1, "the attempts do not reset");
    assert_eq!(device.enrollments, 2);
    assert_eq!(device.verification_code, Some(second_code));
    assert_ne!(first_code, second_code);

    // The code before stopped being valid: the newest one is the one that approves.
    assert_eq!(
        world.approve(&name, first_code, None).await,
        ApproveOutcome::WrongCode { attempts: 2 }
    );
    assert!(matches!(
        world.approve(&name, second_code, None).await,
        ApproveOutcome::Approved(_)
    ));
    assert_eq!(
        world.actions_about(first.device).await,
        [
            "device.code_rejected",
            "device.enrolled_again",
            "device.code_rejected",
            "device.approved"
        ]
    );
}

#[tokio::test]
async fn the_sixth_enrollment_denies_the_request() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    let mut device = None;
    for expected in 1..=5u8 {
        let (outcome, _) = world.enroll(&issued, &who).await;
        let EnrollOutcome::Enrolled(accepted) = outcome else {
            panic!("enrollment {expected} was refused: {outcome:?}");
        };
        assert_eq!(accepted.reply.enrollment.get(), expected);
        device = Some(accepted.device);
    }
    let device = device.unwrap();
    assert_eq!(world.device(device).await.state, DeviceState::Pending);

    let (sixth, _) = world.enroll(&issued, &who).await;
    assert_eq!(
        refusal(&sixth),
        Refusal {
            code: ErrorCode::Ar301,
            closed: Some(device)
        }
    );
    let row = world.device(device).await;
    assert_eq!(
        (row.state, row.state_reason, row.slot),
        (DeviceState::Denied, Some(StateReason::Enrollments), None)
    );
    assert_eq!(
        row.verification_code, None,
        "the code is erased when the request is resolved"
    );
    assert_eq!(row.enrollments, 5);

    // From here on it is a resolved request: the exact situation, and nothing changes.
    let (seventh, _) = world.enroll(&issued, &who).await;
    assert_eq!(
        refusal(&seventh),
        Refusal {
            code: ErrorCode::Ar301,
            closed: None
        }
    );
    assert_eq!(world.device(device).await, row);
}

// ---------------------------------------------------------------------------- line 3

#[tokio::test]
async fn the_same_identity_is_told_the_exact_situation_of_a_resolved_request() {
    let world = World::shared().await;

    // Approved: AR-104.
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    let (first, code) = world.enroll(&issued, &who).await;
    let EnrollOutcome::Enrolled(first) = first else {
        panic!("refused");
    };
    assert!(matches!(
        world.approve(&name, code, None).await,
        ApproveOutcome::Approved(_)
    ));
    let before = world.device(first.device).await;
    let (again, _) = world.enroll(&issued, &who).await;
    assert_eq!(
        refusal(&again),
        Refusal {
            code: ErrorCode::Ar104,
            closed: None
        }
    );
    assert_eq!(world.device(first.device).await, before, "nothing changes");

    // Active, after the first credential: still AR-104.
    let request = crate::support::Request {
        device: first.device,
        identity: who,
        code,
    };
    world.first_swap(&request).await;
    let (again, _) = world.enroll(&issued, &who).await;
    assert_eq!(code_of(&again), ErrorCode::Ar104);

    // Denied by the admin: AR-301.
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    world.enroll(&issued, &who).await;
    arauto_hub::repo::approval::deny(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &name,
        world.now(),
    )
    .await
    .unwrap();
    let (again, _) = world.enroll(&issued, &who).await;
    assert_eq!(
        refusal(&again),
        Refusal {
            code: ErrorCode::Ar301,
            closed: None
        }
    );

    // Expired while pending: AR-302. The enrollment that meets the deadline applies it.
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    let (first, _) = world.enroll(&issued, &who).await;
    let EnrollOutcome::Enrolled(first) = first else {
        panic!("refused");
    };
    world.clock.advance(CONFIRM_WINDOW_MS);
    let (late, _) = world.enroll(&issued, &who).await;
    assert_eq!(
        refusal(&late),
        Refusal {
            code: ErrorCode::Ar302,
            closed: Some(first.device)
        }
    );
    let row = world.device(first.device).await;
    assert_eq!(
        (row.state, row.state_reason, row.slot, row.verification_code),
        (
            DeviceState::Expired,
            Some(StateReason::PendingTimeout),
            None,
            None
        )
    );
    let (again, _) = world.enroll(&issued, &who).await;
    assert_eq!(
        refusal(&again),
        Refusal {
            code: ErrorCode::Ar302,
            closed: None
        }
    );

    // Expired while approved: AR-303.
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    let (first, code) = world.enroll(&issued, &who).await;
    let EnrollOutcome::Enrolled(first) = first else {
        panic!("refused");
    };
    world.approve(&name, code, None).await;
    world.clock.advance(24 * 60 * MINUTE_MS);
    let swept =
        arauto_hub::repo::sweep::sweep(world.pool(), &world.home.mark, world.org, world.now())
            .await
            .unwrap();
    assert!(swept.devices.iter().any(|d| d.id == first.device));
    let (again, _) = world.enroll(&issued, &who).await;
    assert_eq!(
        refusal(&again),
        Refusal {
            code: ErrorCode::Ar303,
            closed: None
        }
    );
}

// ---------------------------------------------------------------------------- line 4

#[tokio::test]
async fn another_identity_with_the_request_pending_is_refused_and_alerts_once() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let issued = world.invite(&name).await;
    let owner = identity();
    let (first, code) = world.enroll(&issued, &owner).await;
    let EnrollOutcome::Enrolled(first) = first else {
        panic!("refused");
    };
    let before = world.device(first.device).await;

    let intruder = identity();
    let (outcome, _) = world.enroll_from(&issued, &intruder, address(66)).await;
    assert_eq!(
        refusal(&outcome),
        Refusal {
            code: ErrorCode::Ar102,
            closed: None
        }
    );

    // The first request is not touched, and no row was made for the intruder.
    assert_eq!(world.device(first.device).await, before);
    assert_eq!(
        arauto_hub::repo::device::by_identity(world.pool(), world.org, &intruder)
            .await
            .unwrap(),
        None
    );
    let alerts: Vec<(String, Option<Uuid>, serde_json::Value)> = sqlx::query_as(
        "SELECT kind, hermes_id, details FROM alert WHERE organization_id = $1 AND device_id = $2",
    )
    .bind(world.org.id())
    .bind(first.device)
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert_eq!(
        alerts,
        [(
            "invite_misused".to_owned(),
            Some(hermes),
            serde_json::json!({ "from": "192.0.2.66" })
        )]
    );
    let events = world.actions_about(issued.id).await;
    assert_eq!(events, ["invite.issued", "invite.used", "invite.misused"]);

    // Insisting does not fill the tables: it is counted.
    let rows = world.audit_rows().await;
    for _ in 0..20 {
        let (outcome, _) = world.enroll_from(&issued, &identity(), address(66)).await;
        assert_eq!(code_of(&outcome), ErrorCode::Ar102);
    }
    assert_eq!(world.alerts_about(first.device).await.len(), 1);
    assert_eq!(world.audit_rows().await, rows);
    assert_eq!(world.refusals.waiting(), 20);

    // And the real request is still approved with its code.
    assert!(matches!(
        world.approve(&name, code, None).await,
        ApproveOutcome::Approved(_)
    ));
    // With the request resolved, another identity gets the single refusal.
    let (outcome, _) = world.enroll(&issued, &identity()).await;
    assert_eq!(code_of(&outcome), ErrorCode::Ar101);
}

// ---------------------------------------------------------------------------- line 5

#[tokio::test]
async fn any_other_case_gets_the_single_refusal() {
    let world = World::shared().await;

    // A cancelled invite.
    let name = world.hermes().await;
    let cancelled = world.invite(&name).await;
    let current = world.invite(&name).await;
    let (outcome, _) = world.enroll(&cancelled, &identity()).await;
    assert_eq!(
        refusal(&outcome),
        Refusal {
            code: ErrorCode::Ar101,
            closed: None
        }
    );

    // An identity that already is a device, with a new invite: a new invite asks for a new
    // identity.
    let other = world.hermes().await;
    let active = world.active(&other).await;
    let (outcome, _) = world.enroll(&current, &active.identity).await;
    assert_eq!(code_of(&outcome), ErrorCode::Ar101);
    assert_eq!(
        invite_state(&world, &current).await.0,
        "issued",
        "the invite was not spent"
    );
    assert_eq!(world.device(active.device).await.state, DeviceState::Active);

    // A used invite whose request expired, presented by another identity.
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let (first, _) = world.enroll(&issued, &identity()).await;
    let EnrollOutcome::Enrolled(first) = first else {
        panic!("refused");
    };
    world.clock.advance(CONFIRM_WINDOW_MS);
    let (outcome, _) = world.enroll(&issued, &identity()).await;
    assert_eq!(code_of(&outcome), ErrorCode::Ar101);
    // The request of the first one was not touched by the stranger: the sweep expires it.
    assert_eq!(world.device(first.device).await.state, DeviceState::Pending);
    assert!(world.alerts_about(first.device).await.is_empty());

    // An invite of a Hermes that was removed.
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    arauto_hub::repo::hermes::remove(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &name,
        world.now(),
    )
    .await
    .unwrap();
    let (outcome, _) = world.enroll(&issued, &identity()).await;
    assert_eq!(code_of(&outcome), ErrorCode::Ar101);

    // An invite of another organization: the digest names nothing here.
    let elsewhere = World::shared().await;
    let foreign = elsewhere.invite(&elsewhere.hermes().await).await;
    let (outcome, _) = world.enroll(&foreign, &identity()).await;
    assert_eq!(code_of(&outcome), ErrorCode::Ar101);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM invite WHERE id = $1")
            .bind(foreign.id)
            .fetch_one(world.pool())
            .await
            .unwrap(),
        "issued"
    );
}

#[tokio::test]
async fn an_invite_never_takes_a_connected_hermes_down() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let before = world.device(active.device).await;

    // The invite it came in with, presented again by anyone, in every way.
    let issued = world.invite(&name).await;
    for who in [identity(), active.identity] {
        world.enroll(&issued, &who).await;
    }
    world.clock.advance(20 * MINUTE_MS);
    world.enroll(&issued, &identity()).await;
    let now = world.device(active.device).await;
    assert_eq!(
        (now.state, now.slot, now.current),
        (before.state, before.slot, before.current)
    );
}

// ---------------------------------------------------------------------------- the refusals

#[tokio::test]
async fn refusals_are_written_one_row_per_origin_and_per_minute() {
    let world = World::own().await;
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    world.enroll(&issued, &identity()).await;
    let minute = world.now().div_euclid(MINUTE_MS) * MINUTE_MS;
    let cancelled = {
        let other = world.hermes().await;
        let cancelled = world.invite(&other).await;
        world.invite(&other).await;
        cancelled
    };
    let rows_before: i64 = sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(world.pool())
        .await
        .unwrap();

    // Three hundred refusals from one origin, forty from another, and some with no invite at
    // all, which belong to no organization.
    for _ in 0..300 {
        let (outcome, _) = world.enroll_from(&cancelled, &identity(), address(1)).await;
        assert_eq!(code_of(&outcome), ErrorCode::Ar101);
    }
    for _ in 0..40 {
        world.enroll_from(&cancelled, &identity(), address(2)).await;
    }
    for _ in 0..7 {
        world
            .refusals
            .note(None, address(1), ErrorCode::Ar101, world.now());
    }
    let rows_now: i64 = sqlx::query_scalar("SELECT count(*) FROM audit")
        .fetch_one(world.pool())
        .await
        .unwrap();
    assert_eq!(rows_now, rows_before, "347 refusals wrote no row");

    // The minute is still open: nothing to write.
    assert_eq!(
        enroll::flush_refused(world.pool(), &world.refusals, world.now())
            .await
            .unwrap(),
        0
    );
    world.clock.set(minute + MINUTE_MS);
    assert_eq!(
        enroll::flush_refused(world.pool(), &world.refusals, world.now())
            .await
            .unwrap(),
        3
    );
    let written: Vec<RefusedRow> = sqlx::query_as(
        "SELECT organization_id, at, address, result, code, security, details FROM audit \
             WHERE action = 'invite.refused' ORDER BY address, organization_id NULLS FIRST",
    )
    .fetch_all(world.pool())
    .await
    .unwrap();
    let org = Some(world.org.id());
    let details =
        |count: i64| serde_json::json!({ "count": count, "minute": minute, "origin_known": true });
    let row = |org: Option<Uuid>, address: &str, count: i64| {
        (
            org,
            minute + MINUTE_MS,
            Some(address.to_owned()),
            "denied".to_owned(),
            Some("AR-101".to_owned()),
            false,
            details(count),
        )
    };
    assert_eq!(
        written,
        [
            row(None, "192.0.2.1", 7),
            row(org, "192.0.2.1", 300),
            row(org, "192.0.2.2", 40),
        ]
    );
    assert_eq!(world.refusals.waiting(), 0);
    assert_eq!(
        enroll::flush_refused(world.pool(), &world.refusals, world.now())
            .await
            .unwrap(),
        0
    );
}
