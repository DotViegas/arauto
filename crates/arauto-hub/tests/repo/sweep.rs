//! Each deadline of the sweep, at its exact millisecond.

use arauto_hub::core::model::{DeviceState, StateReason};
use arauto_hub::repo::approval::ApproveOutcome;
use arauto_hub::repo::enroll::{CONFIRM_WINDOW_MS, EnrollOutcome};
use arauto_hub::repo::invite::INVITE_VALIDITY_MS;
use arauto_hub::repo::swap::IssueOutcome;
use arauto_hub::repo::sweep::{self, APPROVED_WINDOW_MS, ExpiredDevice, INACTIVITY_MS, Swept};
use arauto_protocol::errors::ErrorCode;
use uuid::Uuid;

use crate::support::{World, identity};

async fn run(world: &World) -> Swept {
    sweep::sweep(world.pool(), &world.home.mark, world.org, world.now())
        .await
        .unwrap()
}

async fn invite_row(world: &World, id: Uuid) -> (String, Option<Vec<u8>>) {
    sqlx::query_as("SELECT state, digest FROM invite WHERE organization_id = $1 AND id = $2")
        .bind(world.org.id())
        .bind(id)
        .fetch_one(world.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn an_invite_expires_after_fifteen_minutes_and_its_digest_is_erased() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let rows = world.audit_rows().await;

    world.clock.advance(INVITE_VALIDITY_MS - 1);
    assert!(run(&world).await.is_empty());
    assert_eq!(
        world.audit_rows().await,
        rows,
        "a sweep with nothing due writes nothing"
    );
    assert_eq!(invite_row(&world, issued.id).await.0, "issued");

    world.clock.advance(1);
    let swept = run(&world).await;
    assert_eq!(
        swept,
        Swept {
            invites: vec![issued.id],
            devices: vec![]
        }
    );
    assert_eq!(
        invite_row(&world, issued.id).await,
        ("expired".to_owned(), None)
    );
    assert_eq!(
        world.actions_about(issued.id).await,
        ["invite.issued", "invite.expired"]
    );
    // A clock that goes back does not make it valid again: the digest is gone.
    world.clock.advance(-INVITE_VALIDITY_MS);
    let (outcome, _) = world.enroll(&issued, &identity()).await;
    assert!(matches!(
        outcome,
        EnrollOutcome::Refused(refusal) if refusal.code == ErrorCode::Ar101
    ));
    assert!(run(&world).await.is_empty(), "nothing is expired twice");

    // A used invite does not expire: it already has its final state.
    let name = world.hermes().await;
    let request = world.pending(&name).await;
    let used: Uuid = sqlx::query_scalar("SELECT id FROM invite WHERE device_id = $1")
        .bind(request.device)
        .fetch_one(world.pool())
        .await
        .unwrap();
    world.clock.advance(INVITE_VALIDITY_MS + CONFIRM_WINDOW_MS);
    run(&world).await;
    assert_eq!(invite_row(&world, used).await.0, "used");
}

#[tokio::test]
async fn a_pending_request_expires_ten_minutes_after_its_first_enrollment() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = identity();
    let (first, _) = world.enroll(&issued, &who).await;
    let EnrollOutcome::Enrolled(first) = first else {
        panic!("refused");
    };
    // A second enrollment, minutes later, does not move the deadline.
    world.clock.advance(6 * 60_000);
    let (_, code) = world.enroll(&issued, &who).await;

    world.clock.advance(CONFIRM_WINDOW_MS - 6 * 60_000 - 1);
    assert!(run(&world).await.is_empty());
    assert_eq!(world.device(first.device).await.state, DeviceState::Pending);

    world.clock.advance(1);
    let swept = run(&world).await;
    assert_eq!(
        swept.devices,
        [ExpiredDevice {
            id: first.device,
            reason: StateReason::PendingTimeout
        }]
    );
    let device = world.device(first.device).await;
    assert_eq!(
        (
            device.state,
            device.state_reason,
            device.slot,
            device.state_since
        ),
        (
            DeviceState::Expired,
            Some(StateReason::PendingTimeout),
            None,
            world.now()
        )
    );
    assert_eq!(
        (device.verification_code, device.confirm_until),
        (None, None)
    );
    assert_eq!(device.ended_code(), Some(ErrorCode::Ar302));
    assert_eq!(
        world
            .actions_about(first.device)
            .await
            .last()
            .map(String::as_str),
        Some("device.expired")
    );
    assert_eq!(
        world.approve(&name, code, None).await,
        ApproveOutcome::NoRequest
    );
    assert!(run(&world).await.is_empty());
}

#[tokio::test]
async fn an_approved_request_expires_after_24_hours_without_its_first_credential() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    // It even received a first credential, and never said "saved".
    let connection = world.connect(request.device).await;
    let issued = world.issued(&connection, None).await;

    world.clock.advance(APPROVED_WINDOW_MS - 1);
    assert!(run(&world).await.is_empty());
    world.clock.advance(1);
    assert_eq!(
        run(&world).await.devices,
        [ExpiredDevice {
            id: request.device,
            reason: StateReason::ApprovedTimeout
        }]
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (
            device.state,
            device.state_reason,
            device.slot,
            device.approved_until
        ),
        (
            DeviceState::Expired,
            Some(StateReason::ApprovedTimeout),
            None,
            None
        )
    );
    assert_eq!(device.ended_code(), Some(ErrorCode::Ar303));
    // Too late to confirm, by the "saved" or by presenting what it saved.
    assert!(matches!(
        world.confirm(&connection, &issued).await,
        arauto_hub::repo::swap::ConfirmOutcome::Refused(_)
    ));
    let connection = world.connect(request.device).await;
    assert!(matches!(
        world.issue(&connection, Some(&issued.sealed)).await,
        IssueOutcome::NotSwapped(_)
    ));
    assert_eq!(
        world.device(request.device).await.state,
        DeviceState::Expired
    );
}

#[tokio::test]
async fn an_approved_request_that_confirms_in_time_does_not_expire() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    world.clock.advance(APPROVED_WINDOW_MS - 1);
    let active = world.first_swap(&request).await;
    world.clock.advance(10);
    assert!(run(&world).await.is_empty());
    assert_eq!(world.device(active.device).await.state, DeviceState::Active);
}

#[tokio::test]
async fn an_active_device_expires_after_thirty_days_without_connecting() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let quiet_name = world.hermes().await;
    let quiet = world.active(&quiet_name).await;

    // One of them keeps swapping; the other one goes quiet.
    world.clock.advance(INACTIVITY_MS - 1);
    assert!(run(&world).await.is_empty());
    let active = world.swap_again(&active).await;
    world.clock.advance(1);
    assert_eq!(
        run(&world).await.devices,
        [ExpiredDevice {
            id: quiet.device,
            reason: StateReason::Inactivity
        }]
    );
    let device = world.device(quiet.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (DeviceState::Expired, Some(StateReason::Inactivity), None)
    );
    assert_eq!(device.ended_code(), Some(ErrorCode::Ar507));
    assert_eq!(
        device.current,
        Some(quiet.credential),
        "kept, for a reuse that comes later"
    );
    assert_eq!(world.device(active.device).await.state, DeviceState::Active);

    // It gets no credential any more: reconnecting is the way back.
    let connection = world.connect(quiet.device).await;
    let outcome = world.issue(&connection, Some(&quiet.sealed)).await;
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(not_swapped.code, Some(ErrorCode::Ar507));
}

#[tokio::test]
async fn quarantined_and_suspended_devices_do_not_expire_for_inactivity() {
    let world = World::shared().await;
    let mut kept = Vec::new();
    for state in ["quarantined", "suspended"] {
        let name = world.hermes().await;
        let active = world.active(&name).await;
        sqlx::query("UPDATE device SET state = $2, state_reason = 'reuse' WHERE id = $1")
            .bind(active.device)
            .bind(state)
            .execute(world.pool())
            .await
            .unwrap();
        kept.push((active.device, state));
    }
    world.clock.advance(INACTIVITY_MS * 3);
    assert!(run(&world).await.is_empty());
    for (device, state) in kept {
        assert_eq!(world.device(device).await.state.as_str(), state);
    }
}

#[tokio::test]
async fn one_sweep_applies_every_deadline_that_is_due_and_only_in_its_organization() {
    let world = World::shared().await;
    let elsewhere = World::shared().await;
    let foreign_name = elsewhere.hermes().await;
    let foreign = elsewhere.invite(&foreign_name).await;

    let unused = world.hermes().await;
    let invite = world.invite(&unused).await;
    let waiting = world.hermes().await;
    let pending = world.pending(&waiting).await;
    let late = world.hermes().await;
    let approved = world.approved(&late, None).await;
    // A reconnect whose new invite nobody used, on a Hermes whose device went quiet.
    let both = world.hermes().await;
    let old = world.active(&both).await;
    world.clock.advance(INACTIVITY_MS - 60_000);
    let second_invite = world.invite(&both).await;
    world.clock.advance(INVITE_VALIDITY_MS);

    let mut swept = run(&world).await;
    swept.invites.sort_unstable();
    swept.devices.sort_by_key(|device| device.reason.as_str());
    let mut invites = vec![invite.id, second_invite.id];
    invites.sort_unstable();
    assert_eq!(swept.invites, invites);
    assert_eq!(
        swept.devices,
        [
            ExpiredDevice {
                id: approved.device,
                reason: StateReason::ApprovedTimeout
            },
            ExpiredDevice {
                id: old.device,
                reason: StateReason::Inactivity
            },
            ExpiredDevice {
                id: pending.device,
                reason: StateReason::PendingTimeout
            },
        ]
    );
    // The other organization was not swept by this call.
    let state: String = sqlx::query_scalar("SELECT state FROM invite WHERE id = $1")
        .bind(foreign.id)
        .fetch_one(world.pool())
        .await
        .unwrap();
    assert_eq!(state, "issued");
    elsewhere.clock.set(world.now());
    assert_eq!(run(&elsewhere).await.invites, [foreign.id]);
}

#[tokio::test]
async fn a_swap_that_meets_a_deadline_before_the_sweep_applies_it() {
    let world = World::shared().await;

    // Approved, and the first connection comes at the 24 hours, before any sweep.
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    world.clock.advance(APPROVED_WINDOW_MS);
    let connection = world.connect(request.device).await;
    let outcome = world.issue(&connection, None).await;
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("a credential was issued past the deadline: {outcome:?}");
    };
    assert_eq!(
        (not_swapped.state, not_swapped.code),
        (DeviceState::Expired, Some(ErrorCode::Ar303))
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (
            DeviceState::Expired,
            Some(StateReason::ApprovedTimeout),
            None
        )
    );
    assert_eq!(
        (device.pending, device.max_generation),
        (None, 0),
        "nothing was issued"
    );
    assert!(
        run(&world).await.is_empty(),
        "the sweep finds it already applied"
    );

    // The credential in time and the "saved" past the deadline: never active.
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    world.clock.advance(APPROVED_WINDOW_MS - 1);
    let connection = world.connect(request.device).await;
    let issued = world.issued(&connection, None).await;
    world.clock.advance(1);
    let confirmed = world.confirm(&connection, &issued).await;
    assert_eq!(
        confirmed,
        arauto_hub::repo::swap::ConfirmOutcome::Refused(
            arauto_hub::repo::swap::SavedRefused::DeviceEnded {
                state: DeviceState::Expired,
                code: Some(ErrorCode::Ar303)
            }
        )
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.first_confirmed_at),
        (DeviceState::Expired, None)
    );

    // Active, and it comes back at the 30 days.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    world.clock.advance(INACTIVITY_MS);
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, Some(&active.sealed)).await;
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("a credential was issued past the deadline: {outcome:?}");
    };
    assert_eq!(
        (not_swapped.state, not_swapped.code),
        (DeviceState::Expired, Some(ErrorCode::Ar507))
    );
    assert_eq!(
        world.device(active.device).await.state,
        DeviceState::Expired
    );
}
