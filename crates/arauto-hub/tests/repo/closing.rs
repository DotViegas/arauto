//! Revoking a Hermes, a device asking to leave, the alerts and the reading of the audit.

use arauto_hub::core::model::{DeviceState, StateReason};
use arauto_hub::repo::alert::{self, AlertFilter, AlertKind};
use arauto_hub::repo::approval::Recognition;
use arauto_hub::repo::audit_log;
use arauto_hub::repo::closing::{self, ClosedDevice, RevokeOutcome};
use arauto_hub::repo::swap::IssueOutcome;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::LeftStatus;
use arauto_protocol::names::HermesName;
use uuid::Uuid;

use crate::common;
use crate::support::{World, address};

async fn revoke(world: &World, name: &HermesName) -> RevokeOutcome {
    closing::revoke(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        name,
        world.now(),
    )
    .await
    .unwrap()
}

async fn leave(world: &World, device: Uuid) -> closing::Left {
    closing::leave(
        world.pool(),
        &world.home.mark,
        world.org,
        device,
        address(150),
        world.now(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn revoking_closes_the_device_the_request_and_the_invite() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let old = world.active(&name).await;
    // A reconnect on its way: a request approved, and then a newer invite still unused.
    let request = world.approved(&name, Some(Recognition::Recognized)).await;
    world.clock.advance(2_000);

    let outcome = revoke(&world, &name).await;
    let RevokeOutcome::Revoked(closed) = outcome else {
        panic!("not revoked: {outcome:?}");
    };
    let mut devices = closed.devices.clone();
    devices.sort_by_key(|device| device.was.as_str());
    assert_eq!(
        devices,
        [
            ClosedDevice {
                id: old.device,
                was: DeviceState::Active,
                now: DeviceState::Revoked
            },
            ClosedDevice {
                id: request.device,
                was: DeviceState::Approved,
                now: DeviceState::Revoked
            },
        ]
    );
    assert_eq!(
        closed.invite, None,
        "the invite of the request was already used"
    );
    for device in [old.device, request.device] {
        let row = world.device(device).await;
        assert_eq!(
            (row.state, row.state_reason, row.slot, row.state_since),
            (
                DeviceState::Revoked,
                Some(StateReason::Admin),
                None,
                world.now()
            )
        );
        assert_eq!(row.approved_until, None);
    }
    // The row stays as a record, with what the hub knew of its credential.
    assert_eq!(world.device(old.device).await.current, Some(old.credential));
    // The next call is already refused, and the request cannot finish its first swap.
    let connection = world.connect(old.device).await;
    let outcome = world.issue(&connection, Some(&old.sealed)).await;
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(not_swapped.code, Some(ErrorCode::Ar502));
    let connection = world.connect(request.device).await;
    assert!(matches!(
        world.issue(&connection, None).await,
        IssueOutcome::NotSwapped(_)
    ));

    // Revoking again finds nothing to close; the line is still there for a new invite.
    assert_eq!(
        revoke(&world, &name).await,
        RevokeOutcome::Revoked(closing::Closed::default())
    );
    assert_eq!(
        revoke(&world, &HermesName::parse("nobody").unwrap()).await,
        RevokeOutcome::NotFound
    );
    let again = world.active(&name).await;
    assert_eq!(world.device(again.device).await.state, DeviceState::Active);
}

#[tokio::test]
async fn revoking_denies_a_pending_request_and_cancels_the_issued_invite() {
    let world = World::shared().await;

    let name = world.hermes().await;
    let pending = world.pending(&name).await;
    let RevokeOutcome::Revoked(closed) = revoke(&world, &name).await else {
        panic!("not revoked");
    };
    assert_eq!(
        closed.devices,
        [ClosedDevice {
            id: pending.device,
            was: DeviceState::Pending,
            now: DeviceState::Denied
        }]
    );
    let row = world.device(pending.device).await;
    assert_eq!(
        (row.state, row.state_reason, row.verification_code),
        (DeviceState::Denied, Some(StateReason::Admin), None)
    );

    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let RevokeOutcome::Revoked(closed) = revoke(&world, &name).await else {
        panic!("not revoked");
    };
    assert_eq!((closed.devices.len(), closed.invite), (0, Some(issued.id)));
    // The cancelled invite creates nothing.
    let (outcome, _) = world.enroll(&issued, &crate::support::identity()).await;
    assert!(matches!(
        outcome,
        arauto_hub::repo::enroll::EnrollOutcome::Refused(refusal) if refusal.code == ErrorCode::Ar101
    ));
}

#[tokio::test]
async fn a_device_that_asks_to_leave_is_closed_and_the_admin_is_alerted() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let active = world.active(&name).await;
    let used_at = world.now();
    world.clock.advance(7_200_000);

    let left = leave(&world, active.device).await;
    assert_eq!(left.status, LeftStatus::Closed);
    assert_eq!(
        left.closed,
        Some(ClosedDevice {
            id: active.device,
            was: DeviceState::Active,
            now: DeviceState::Revoked
        })
    );
    let row = world.device(active.device).await;
    assert_eq!(
        (row.state, row.state_reason, row.slot),
        (DeviceState::Revoked, Some(StateReason::Left), None)
    );
    // The alert says where the request came from and the last use of the device.
    let alert: (String, Option<Uuid>, serde_json::Value) = sqlx::query_as(
        "SELECT kind, hermes_id, details FROM alert WHERE organization_id = $1 AND device_id = $2",
    )
    .bind(world.org.id())
    .bind(active.device)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!((alert.0.as_str(), alert.1), ("device_left", Some(hermes)));
    assert_eq!(
        alert.2,
        serde_json::json!({
            "was": "active",
            "from": "192.0.2.150",
            "last_use_at": used_at,
            "last_use_from": "192.0.2.20",
        })
    );
    let event: (String, Option<Uuid>, Option<String>, bool) = sqlx::query_as(
        "SELECT actor_kind, actor_id, address, security FROM audit \
         WHERE organization_id = $1 AND action = 'device.revoked' AND target_id = $2",
    )
    .bind(world.org.id())
    .bind(active.device)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(
        event,
        (
            "device".to_owned(),
            Some(active.device),
            Some("192.0.2.150".to_owned()),
            true
        )
    );

    // Asking again: already closed, and no second alert.
    let again = leave(&world, active.device).await;
    assert_eq!(
        (again.status, again.closed),
        (LeftStatus::AlreadyClosed, None)
    );
    assert_eq!(world.alerts_about(active.device).await, ["device_left"]);
    // An identity that is no device of this organization.
    let unknown = leave(&world, Uuid::new_v4()).await;
    assert_eq!(
        (unknown.status, unknown.closed),
        (LeftStatus::Unknown, None)
    );
}

#[tokio::test]
async fn leaving_closes_each_state_the_way_the_state_machine_says() {
    let world = World::shared().await;

    // A pending request that gives up is denied.
    let name = world.hermes().await;
    let pending = world.pending(&name).await;
    let left = leave(&world, pending.device).await;
    assert_eq!(
        left.closed.map(|closed| closed.now),
        Some(DeviceState::Denied)
    );
    let row = world.device(pending.device).await;
    assert_eq!(
        (row.state, row.state_reason, row.verification_code),
        (DeviceState::Denied, Some(StateReason::Left), None)
    );
    assert_eq!(world.alerts_about(pending.device).await, ["device_left"]);

    // Approved, suspended and quarantined are revoked.
    let name = world.hermes().await;
    let approved = world.approved(&name, None).await;
    let left = leave(&world, approved.device).await;
    assert_eq!(
        left.closed.map(|closed| closed.now),
        Some(DeviceState::Revoked)
    );
    for state in ["suspended", "quarantined"] {
        let hermes = common::hermes(world.pool(), world.org).await;
        let (device, _) = common::device(
            world.pool(),
            world.org,
            hermes,
            state,
            common::Confirmed::Yes,
        )
        .await;
        let left = leave(&world, device).await;
        assert_eq!(left.status, LeftStatus::Closed, "{state}");
        assert_eq!(
            world.device(device).await.state,
            DeviceState::Revoked,
            "{state}"
        );
        assert_eq!(world.alerts_about(device).await, ["device_left"], "{state}");
    }
    // What is already final answers "already closed".
    for state in ["denied", "expired", "replaced", "revoked"] {
        let hermes = common::hermes(world.pool(), world.org).await;
        let (device, _) = common::device(
            world.pool(),
            world.org,
            hermes,
            state,
            common::Confirmed::Never,
        )
        .await;
        let left = leave(&world, device).await;
        assert_eq!(left.status, LeftStatus::AlreadyClosed, "{state}");
        assert!(world.alerts_about(device).await.is_empty(), "{state}");
    }
}

#[tokio::test]
async fn leaving_one_device_does_not_touch_the_other_of_the_same_hermes() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let old = world.active(&name).await;
    let request = world.approved(&name, Some(Recognition::Recognized)).await;
    leave(&world, request.device).await;
    assert_eq!(
        world.device(request.device).await.state,
        DeviceState::Revoked
    );
    assert_eq!(world.device(old.device).await.state, DeviceState::Active);
    world.swap_again(&old).await;
}

// ---------------------------------------------------------------------------- alerts and audit

#[tokio::test]
async fn alerts_are_listed_newest_first_and_marked_as_seen() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let first = world.active(&name).await;
    leave(&world, first.device).await;
    world.clock.advance(1_000);
    let second = world.active(&name).await;
    leave(&world, second.device).await;

    let unseen = alert::list(world.pool(), world.org, AlertFilter::Unseen, 10)
        .await
        .unwrap();
    assert_eq!(unseen.len(), 2);
    assert_eq!(unseen[0].device_id, Some(second.device), "newest first");
    assert!(
        unseen
            .iter()
            .all(|alert| alert.kind == AlertKind::DeviceLeft)
    );
    assert_eq!(unseen[0].hermes_name.as_deref(), Some(name.as_str()));
    assert_eq!(unseen[0].seen_at, None);
    let limited = alert::list(world.pool(), world.org, AlertFilter::Unseen, 1)
        .await
        .unwrap();
    assert_eq!(limited.len(), 1);

    world.clock.advance(500);
    let acknowledge = |id| alert::acknowledge(world.pool(), world.org, id, world.now());
    assert!(acknowledge(unseen[1].id).await.unwrap());
    assert!(!acknowledge(unseen[1].id).await.unwrap(), "already seen");
    assert!(!acknowledge(Uuid::new_v4()).await.unwrap());
    // Not from another organization.
    let other = common::organization(world.pool(), &world.home).await;
    assert!(
        !alert::acknowledge(world.pool(), other, unseen[0].id, world.now())
            .await
            .unwrap()
    );
    assert!(
        alert::list(world.pool(), other, AlertFilter::All, 10)
            .await
            .unwrap()
            .is_empty()
    );

    let unseen_now = alert::list(world.pool(), world.org, AlertFilter::Unseen, 10)
        .await
        .unwrap();
    assert_eq!(unseen_now.len(), 1);
    let all = alert::list(world.pool(), world.org, AlertFilter::All, 10)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].seen_at, Some(world.now()));
}

#[tokio::test]
async fn the_audit_is_read_in_order_and_holds_no_secret() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let issued = world.invite(&name).await;
    let who = crate::support::identity();
    let (_, code) = world.enroll(&issued, &who).await;
    world.approve(&name, code, None).await;
    let device = arauto_hub::repo::device::by_identity(world.pool(), world.org, &who)
        .await
        .unwrap()
        .unwrap();
    let request = crate::support::Request {
        device: device.id,
        identity: who,
        code,
    };
    let active = world.first_swap(&request).await;
    revoke(&world, &name).await;

    let rows = audit_log::tail(world.pool(), world.org, 1_000)
        .await
        .unwrap();
    let mine: Vec<&str> = rows
        .iter()
        .filter(|row| row.of_organization)
        .map(|row| row.action.as_str())
        .collect();
    assert_eq!(
        mine,
        [
            "hermes.created",
            "invite.issued",
            "invite.used",
            "device.approved",
            "device.activated",
            "credential.confirmed",
            "device.revoked",
        ]
    );
    assert!(rows.windows(2).all(|pair| pair[0].seq < pair[1].seq));
    let security: Vec<bool> = rows
        .iter()
        .filter(|row| row.of_organization)
        .map(|row| row.security)
        .collect();
    assert_eq!(security, [true, false, true, true, true, true, true]);
    let approved = rows
        .iter()
        .find(|row| row.action == "device.approved")
        .unwrap();
    assert_eq!(
        (approved.actor_kind.as_str(), approved.actor_note.as_deref()),
        ("admin", Some("tester"))
    );
    // Only the last ones, when fewer are asked for.
    let last_two = audit_log::tail(world.pool(), world.org, 2).await.unwrap();
    assert_eq!(last_two.len(), 2);
    assert!(last_two[0].seq < last_two[1].seq);

    // Nothing that was a secret along the way is anywhere in the audit or in the alerts: the
    // invite, its digest, the sealed credential and its digest.
    let everything: String = sqlx::query_scalar(
        "SELECT coalesce((SELECT string_agg(a::text, ' ') FROM audit a \
                          WHERE a.organization_id = $1), '') \
             || coalesce((SELECT string_agg(l::text, ' ') FROM alert l \
                          WHERE l.organization_id = $1), '')",
    )
    .bind(world.org.id())
    .fetch_one(world.pool())
    .await
    .unwrap();
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let invite = issued.invite.reveal();
    let secrets = [
        invite.as_str().to_owned(),
        hex(issued.invite.digest().as_bytes()),
        hex(active.sealed.expose()),
        hex(active.credential.digest.as_bytes()),
    ];
    for secret in &secrets {
        assert!(
            !everything.contains(secret.as_str()),
            "the audit holds {secret}"
        );
    }
}

// ---------------------------------------------------------------------------- a restored database

#[tokio::test]
async fn no_write_of_the_hub_goes_into_a_database_behind_the_mark() {
    use arauto_hub::mark::MarkValue;
    use arauto_hub::repo::RepoError;
    use arauto_hub::restore::Evidence;

    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let waiting_name = world.hermes().await;
    let waiting = world.pending(&waiting_name).await;
    let fresh_name = world.hermes().await;
    let fresh = world.invite(&fresh_name).await;
    let connection = world.connect(active.device).await;
    let issued = world.issued(&connection, Some(&active.sealed)).await;
    let rows = world.audit_rows().await;
    let alerts: i64 = sqlx::query_scalar("SELECT count(*) FROM alert WHERE organization_id = $1")
        .bind(world.org.id())
        .fetch_one(world.pool())
        .await
        .unwrap();
    let devices = [
        world.device(active.device).await,
        world.device(waiting.device).await,
    ];

    // The mark of this folder now knows of events the database does not have. Other tests
    // write to the same database: far enough that none of them catches up.
    let MarkValue::At(at) = world.home.mark.read().await.unwrap() else {
        panic!("there is a mark");
    };
    let ahead = world.home.mark.advance(at + 1_000_000_000).await.unwrap();
    world.clock.advance(40 * 86_400_000);

    let restored = |outcome: RepoError| match outcome {
        RepoError::Restored(Evidence::BehindMark { mark, .. }) if mark == ahead => {}
        other => panic!("not refused as restored: {other}"),
    };
    let pool = world.pool();
    let mark = &world.home.mark;
    let (org, now) = (world.org, world.now());
    restored(
        arauto_hub::repo::swap::issue(pool, mark, org, &world.key, &connection, None, now)
            .await
            .unwrap_err(),
    );
    restored(
        arauto_hub::repo::swap::rotate(pool, mark, org, &world.key, &active.connection, now)
            .await
            .unwrap_err(),
    );
    restored(
        arauto_hub::repo::swap::look(pool, mark, org, &world.key, &connection, None, now)
            .await
            .unwrap_err(),
    );
    restored(
        arauto_hub::repo::swap::confirm(
            pool,
            mark,
            org,
            &connection,
            &issued.receipt,
            &World::saved(&issued),
            now,
        )
        .await
        .unwrap_err(),
    );
    restored(
        arauto_hub::repo::swap::quarantine_double_connection(
            pool,
            mark,
            org,
            active.device,
            None,
            now,
        )
        .await
        .unwrap_err(),
    );
    restored(
        closing::leave(pool, mark, org, active.device, None, now)
            .await
            .unwrap_err(),
    );
    restored(
        arauto_hub::repo::approval::deny_double_waiting(pool, mark, org, waiting.device, None, now)
            .await
            .unwrap_err(),
    );
    restored(
        arauto_hub::repo::sweep::sweep(pool, mark, org, now)
            .await
            .unwrap_err(),
    );
    let hash = arauto_protocol::noise::HandshakeHash::from_bytes([3; 32]);
    let nonce = arauto_hub::repo::enroll::draw_nonce(&hash).unwrap();
    restored(
        arauto_hub::repo::enroll::enroll(
            pool,
            mark,
            org,
            &world.refusals,
            &fresh.invite.digest(),
            &arauto_hub::repo::enroll::Enrollment {
                identity: &crate::support::identity(),
                declared: &Default::default(),
                connector_version: &Default::default(),
                origin: None,
                nonce: &nonce,
                handshake_hash: &hash,
            },
            now,
        )
        .await
        .unwrap_err(),
    );

    // Nothing of this organization changed, and the mark stayed where it was.
    assert_eq!(world.audit_rows().await, rows);
    let alerts_now: i64 =
        sqlx::query_scalar("SELECT count(*) FROM alert WHERE organization_id = $1")
            .bind(world.org.id())
            .fetch_one(world.pool())
            .await
            .unwrap();
    assert_eq!(alerts_now, alerts);
    assert_eq!(
        [
            world.device(active.device).await,
            world.device(waiting.device).await
        ],
        devices
    );
    let state: String = sqlx::query_scalar("SELECT state FROM invite WHERE id = $1")
        .bind(fresh.id)
        .fetch_one(world.pool())
        .await
        .unwrap();
    assert_eq!(state, "issued");
    assert_eq!(world.home.mark.read().await.unwrap(), MarkValue::At(ahead));
}
