//! Approving with the typed code, denying, and the question of a reconnect.

use std::time::Duration;

use arauto_hub::core::model::{DeviceState, Slot, StateReason};
use arauto_hub::repo::ADMIN_CHANNEL;
use arauto_hub::repo::approval::{
    self, Answer, ApproveOutcome, CODE_ATTEMPTS_MAX, DenyOutcome, DoubleWaitingOutcome, LastUse,
    Recognition, Recognized,
};
use arauto_hub::repo::enroll::CONFIRM_WINDOW_MS;
use arauto_hub::repo::sweep::APPROVED_WINDOW_MS;
use arauto_protocol::names::HermesName;
use sqlx::postgres::PgListener;
use uuid::Uuid;

use crate::support::{World, address, another_code};

async fn deny(world: &World, name: &HermesName) -> DenyOutcome {
    approval::deny(
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

#[tokio::test]
async fn the_right_code_approves_and_the_code_is_erased() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let request = world.pending(&name).await;
    world.clock.advance(3 * 60_000);

    let outcome = world.approve(&name, request.code, None).await;
    let ApproveOutcome::Approved(approved) = outcome else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!(approved.device, request.device);
    assert_eq!(approved.approved_until, world.now() + APPROVED_WINDOW_MS);
    assert_eq!((approved.replaces, approved.quarantined), (None, None));

    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.slot, device.state_since),
        (DeviceState::Approved, Some(Slot::Request), world.now())
    );
    assert_eq!(
        (device.verification_code, device.confirm_until),
        (None, None)
    );
    assert_eq!(device.approved_until, Some(world.now() + 86_400_000));
    assert_eq!(device.approved_by, Some(world.admin.id));
    assert_eq!(device.approved_at, Some(world.now()));
    // Generation zero: approved, and no credential yet.
    assert_eq!(
        (device.current, device.pending, device.max_generation),
        (None, None, 0)
    );

    let event: (bool, String, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT security, actor_kind, actor_id, actor_note FROM audit \
         WHERE organization_id = $1 AND action = 'device.approved' AND target_id = $2",
    )
    .bind(world.org.id())
    .bind(request.device)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(
        event,
        (
            true,
            "admin".to_owned(),
            Some(world.admin.id),
            Some("tester".to_owned())
        )
    );

    // There is no request to approve any more.
    assert_eq!(
        world.approve(&name, request.code, None).await,
        ApproveOutcome::NoRequest
    );
}

#[tokio::test]
async fn five_wrong_codes_close_the_request() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let request = world.pending(&name).await;
    let wrong = another_code(request.code);
    assert_eq!(CODE_ATTEMPTS_MAX, 5);

    for attempts in 1..=4 {
        assert_eq!(
            world.approve(&name, wrong, None).await,
            ApproveOutcome::WrongCode { attempts }
        );
        let device = world.device(request.device).await;
        assert_eq!(
            (device.state, device.code_attempts),
            (DeviceState::Pending, attempts)
        );
        assert_eq!(device.verification_code, Some(request.code));
    }
    assert_eq!(
        world.approve(&name, wrong, None).await,
        ApproveOutcome::Denied {
            device: request.device
        }
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (DeviceState::Denied, Some(StateReason::CodeAttempts), None)
    );
    assert_eq!(
        (device.verification_code, device.confirm_until),
        (None, None)
    );
    assert_eq!(device.code_attempts, 5);

    // The right code does not bring it back.
    assert_eq!(
        world.approve(&name, request.code, None).await,
        ApproveOutcome::NoRequest
    );
    assert_eq!(world.device(request.device).await, device);
    let actions = world.actions_about(request.device).await;
    assert_eq!(
        actions
            .iter()
            .filter(|a| *a == "device.code_rejected")
            .count(),
        5
    );
    assert_eq!(actions.last().map(String::as_str), Some("device.denied"));
}

#[tokio::test]
async fn four_wrong_codes_and_then_the_right_one_approve() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let request = world.pending(&name).await;
    for _ in 0..4 {
        world.approve(&name, another_code(request.code), None).await;
    }
    assert!(matches!(
        world.approve(&name, request.code, None).await,
        ApproveOutcome::Approved(_)
    ));
}

#[tokio::test]
async fn the_code_is_typed_within_ten_minutes_of_the_first_enrollment() {
    let world = World::shared().await;

    // One millisecond before the deadline, the right code still approves.
    let name = world.hermes().await;
    let request = world.pending(&name).await;
    world.clock.advance(CONFIRM_WINDOW_MS - 1);
    assert!(matches!(
        world.approve(&name, request.code, None).await,
        ApproveOutcome::Approved(_)
    ));

    // At the deadline it does not, whatever is typed, and the request expires.
    let name = world.hermes().await;
    let request = world.pending(&name).await;
    world.clock.advance(CONFIRM_WINDOW_MS);
    assert_eq!(
        world.approve(&name, request.code, None).await,
        ApproveOutcome::Expired {
            device: request.device
        }
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (
            DeviceState::Expired,
            Some(StateReason::PendingTimeout),
            None
        )
    );
    assert_eq!(
        (device.verification_code, device.confirm_until),
        (None, None)
    );
    assert_eq!(
        world.approve(&name, request.code, None).await,
        ApproveOutcome::NoRequest
    );
}

#[tokio::test]
async fn a_code_only_approves_the_request_of_its_own_hermes() {
    let world = World::shared().await;
    let one = world.hermes().await;
    let two = world.hermes().await;
    let request_one = world.pending(&one).await;
    let request_two = world.pending(&two).await;

    // The code the other Hermes answered is a wrong code here.
    assert_eq!(
        world.approve(&one, request_two.code, None).await,
        ApproveOutcome::WrongCode { attempts: 1 }
    );
    assert_eq!(
        world.device(request_two.device).await.state,
        DeviceState::Pending
    );
    assert!(matches!(
        world.approve(&two, request_two.code, None).await,
        ApproveOutcome::Approved(_)
    ));
    assert_eq!(
        world.device(request_one.device).await.state,
        DeviceState::Pending
    );

    let nobody = HermesName::parse("nobody").unwrap();
    assert_eq!(
        world.approve(&nobody, request_one.code, None).await,
        ApproveOutcome::NotFound
    );
}

#[tokio::test]
async fn denying_closes_only_a_pending_request() {
    let world = World::shared().await;
    let name = world.hermes().await;
    assert_eq!(deny(&world, &name).await, DenyOutcome::NoRequest);

    let request = world.pending(&name).await;
    assert_eq!(
        deny(&world, &name).await,
        DenyOutcome::Denied {
            device: request.device
        }
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (
            device.state,
            device.state_reason,
            device.slot,
            device.verification_code
        ),
        (DeviceState::Denied, Some(StateReason::Admin), None, None)
    );
    assert_eq!(deny(&world, &name).await, DenyOutcome::NoRequest);

    // An approved request is not "denied": it is revoked, or closed by a new invite.
    let approved = world.approved(&name, None).await;
    assert_eq!(deny(&world, &name).await, DenyOutcome::NoRequest);
    assert_eq!(
        world.device(approved.device).await.state,
        DeviceState::Approved
    );

    let nobody = HermesName::parse("nobody").unwrap();
    assert_eq!(deny(&world, &nobody).await, DenyOutcome::NotFound);
}

// ---------------------------------------------------------------------------- reconnect

#[tokio::test]
async fn a_reconnect_is_not_approved_before_the_question_of_the_last_use() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let old = world.active(&name).await;
    let used_at = world.now();
    world.clock.advance(3_600_000);
    let request = world.pending(&name).await;

    // What the admin is shown before deciding.
    let view = approval::request_of(world.pool(), world.org, &name)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.request.id, request.device);
    let last_use = LastUse {
        device: old.device,
        state: DeviceState::Active,
        at: Some(used_at),
        address: Some("192.0.2.20".to_owned()),
    };
    assert_eq!(view.replaces, Some(last_use.clone()));

    // Without the answer nothing is approved and nothing is counted, right code or wrong.
    let rows = world.audit_rows().await;
    for code in [request.code, another_code(request.code)] {
        assert_eq!(
            world.approve(&name, code, None).await,
            ApproveOutcome::NeedsRecognition(last_use.clone())
        );
    }
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.code_attempts),
        (DeviceState::Pending, 0)
    );
    assert_eq!(world.audit_rows().await, rows);

    // Recognized: approved, and the device in use goes on as it is until the new one confirms.
    let outcome = world
        .approve(&name, request.code, Some(Recognition::Recognized))
        .await;
    let ApproveOutcome::Approved(approved) = outcome else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!(approved.replaces, Some(last_use));
    assert_eq!(approved.quarantined, None);
    assert_eq!(world.device(old.device).await.state, DeviceState::Active);
    assert!(world.alerts_about(old.device).await.is_empty());
    world.swap_again(&old).await;
}

/// C10, step 3: the admin answers about the last use that was on the screen. A use that
/// happens after that is not covered by the answer: it may be the copy.
#[tokio::test]
async fn an_answer_about_a_last_use_that_is_no_longer_the_last_approves_nothing() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let old = world.active(&name).await;
    let request = world.pending(&name).await;

    // What the admin is shown, and answers about.
    let view = approval::request_of(world.pool(), world.org, &name)
        .await
        .unwrap()
        .unwrap();
    let shown = view.replaces.expect("a reconnect shows the last use");
    assert_eq!(shown.device, old.device);
    let answer = |recognition, shown| Answer {
        code: request.code,
        recognition: Some(Recognized {
            answer: recognition,
            shown,
        }),
    };

    // Between the screen and the answer the old device is used again, from elsewhere.
    world.clock.advance(60_000);
    let connection = world.connect_from(old.device, address(99)).await;
    let issued = world.issued(&connection, Some(&old.sealed)).await;
    world.confirm(&connection, &issued).await;

    let rows = world.audit_rows().await;
    let outcome = world
        .approve_with(&name, answer(Recognition::Recognized, shown.clone()))
        .await;
    let ApproveOutcome::NeedsRecognition(current) = outcome else {
        panic!("a use the admin never saw was taken as recognized: {outcome:?}");
    };
    assert_ne!(current, shown);
    assert_eq!(
        (current.at, current.address.as_deref()),
        (Some(world.now()), Some("192.0.2.99"))
    );
    // Nothing was written, and no attempt was counted.
    assert_eq!(world.audit_rows().await, rows);
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.code_attempts),
        (DeviceState::Pending, 0)
    );
    assert_eq!(world.device(old.device).await.state, DeviceState::Active);
    assert!(world.alerts_about(old.device).await.is_empty());

    // With the answer about the last use as it is now, the approval goes on.
    let outcome = world
        .approve_with(&name, answer(Recognition::NotRecognized, current))
        .await;
    let ApproveOutcome::Approved(approved) = outcome else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!(approved.quarantined, Some(old.device));
    assert_eq!(world.alerts_about(old.device).await, ["copy_suspected"]);
}

#[tokio::test]
async fn a_last_use_that_is_not_recognized_quarantines_the_old_device_right_there() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let old = world.active(&name).await;
    let request = world.pending(&name).await;

    // A wrong code with the answer given counts as a wrong code, and quarantines nothing.
    assert_eq!(
        world
            .approve(
                &name,
                another_code(request.code),
                Some(Recognition::NotRecognized)
            )
            .await,
        ApproveOutcome::WrongCode { attempts: 1 }
    );
    assert_eq!(world.device(old.device).await.state, DeviceState::Active);
    assert!(world.alerts_about(old.device).await.is_empty());

    let outcome = world
        .approve(&name, request.code, Some(Recognition::NotRecognized))
        .await;
    let ApproveOutcome::Approved(approved) = outcome else {
        panic!("the approval goes on: {outcome:?}");
    };
    assert_eq!(approved.quarantined, Some(old.device));
    assert_eq!(
        world.device(request.device).await.state,
        DeviceState::Approved
    );

    let device = world.device(old.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (
            DeviceState::Quarantined,
            Some(StateReason::NotRecognized),
            Some(Slot::InUse)
        )
    );
    let alert: (String, Option<Uuid>, serde_json::Value) = sqlx::query_as(
        "SELECT kind, hermes_id, details FROM alert WHERE organization_id = $1 AND device_id = $2",
    )
    .bind(world.org.id())
    .bind(old.device)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(alert.0, "copy_suspected");
    assert_eq!(alert.1, Some(hermes));
    assert_eq!(alert.2["last_use_from"], "192.0.2.20");
    assert_eq!(
        alert.2["request"],
        request.device.as_hyphenated().to_string()
    );
    assert_eq!(
        world
            .actions_about(old.device)
            .await
            .last()
            .map(String::as_str),
        Some("device.quarantined")
    );

    // The old device is refused from here on, and the new one takes its place when it confirms.
    let connection = world.connect(old.device).await;
    let outcome = world.issue(&connection, Some(&old.sealed)).await;
    assert!(
        matches!(outcome, arauto_hub::repo::swap::IssueOutcome::NotSwapped(_)),
        "{outcome:?}"
    );
    world.first_swap(&request).await;
    assert_eq!(world.device(old.device).await.state, DeviceState::Replaced);
}

#[tokio::test]
async fn an_old_device_that_is_not_active_stays_as_it_is() {
    let world = World::shared().await;
    for (state, reason) in [("suspended", "restored"), ("quarantined", "reuse")] {
        let name = world.hermes().await;
        let old = world.active(&name).await;
        sqlx::query("UPDATE device SET state = $2, state_reason = $3 WHERE id = $1")
            .bind(old.device)
            .bind(state)
            .bind(reason)
            .execute(world.pool())
            .await
            .unwrap();
        let before = world.device(old.device).await;
        let request = world.pending(&name).await;
        assert!(matches!(
            world.approve(&name, request.code, None).await,
            ApproveOutcome::NeedsRecognition(_)
        ));
        let outcome = world
            .approve(&name, request.code, Some(Recognition::NotRecognized))
            .await;
        let ApproveOutcome::Approved(approved) = outcome else {
            panic!("not approved: {outcome:?}");
        };
        assert_eq!(approved.quarantined, None, "{state}");
        assert_eq!(world.device(old.device).await, before, "{state}");
        assert_eq!(world.alerts_about(old.device).await, ["copy_suspected"]);
    }
}

#[tokio::test]
async fn a_device_that_expired_for_inactivity_has_its_last_use_shown_the_same_way() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let old = world.active(&name).await;
    let used_at = world.now();
    world.clock.advance(31 * 86_400_000);
    arauto_hub::repo::sweep::sweep(world.pool(), &world.home.mark, world.org, world.now())
        .await
        .unwrap();
    assert_eq!(world.device(old.device).await.state, DeviceState::Expired);

    let request = world.pending(&name).await;
    let ApproveOutcome::NeedsRecognition(last_use) = world.approve(&name, request.code, None).await
    else {
        panic!("the last use of the expired device was not asked about");
    };
    assert_eq!(
        (last_use.device, last_use.state, last_use.at),
        (old.device, DeviceState::Expired, Some(used_at))
    );
    let outcome = world
        .approve(&name, request.code, Some(Recognition::NotRecognized))
        .await;
    let ApproveOutcome::Approved(approved) = outcome else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!(approved.quarantined, None);
    // It stays expired, and the alert of a copy went out.
    let device = world.device(old.device).await;
    assert_eq!(
        (device.state, device.state_reason),
        (DeviceState::Expired, Some(StateReason::Inactivity))
    );
    assert_eq!(world.alerts_about(old.device).await, ["copy_suspected"]);
    // The new device confirms with no device in use to replace.
    world.first_swap(&request).await;
    assert_eq!(world.device(old.device).await.state, DeviceState::Expired);
}

#[tokio::test]
async fn a_first_connection_asks_nothing_about_a_last_use() {
    let world = World::shared().await;
    let name = world.hermes().await;
    // A device that was revoked is not "the current device".
    let old = world.active(&name).await;
    arauto_hub::repo::closing::revoke(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &name,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(world.device(old.device).await.state, DeviceState::Revoked);
    let request = world.pending(&name).await;
    let view = approval::request_of(world.pool(), world.org, &name)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.replaces, None);
    // An answer nobody asked for changes nothing.
    let outcome = world
        .approve(&name, request.code, Some(Recognition::NotRecognized))
        .await;
    let ApproveOutcome::Approved(approved) = outcome else {
        panic!("not approved: {outcome:?}");
    };
    assert_eq!((approved.replaces, approved.quarantined), (None, None));
    assert!(world.alerts_about(old.device).await.is_empty());
}

// ---------------------------------------------------------------------------- the hub is told

#[tokio::test]
async fn two_waiting_connections_with_the_same_identity_deny_the_request() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let request = world.pending(&name).await;
    let outcome = approval::deny_double_waiting(
        world.pool(),
        &world.home.mark,
        world.org,
        request.device,
        address(77),
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, DoubleWaitingOutcome::Denied);
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.verification_code),
        (DeviceState::Denied, Some(StateReason::DoubleWaiting), None)
    );
    assert_eq!(
        world.alerts_about(request.device).await,
        ["identity_copied"]
    );

    // Only a pending request: an approved one is not touched.
    let approved = world.approved(&name, None).await;
    let outcome = approval::deny_double_waiting(
        world.pool(),
        &world.home.mark,
        world.org,
        approved.device,
        address(77),
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, DoubleWaitingOutcome::NotPending);
    assert_eq!(
        world.device(approved.device).await.state,
        DeviceState::Approved
    );
}

#[tokio::test]
async fn the_admin_functions_tell_the_hub_what_changed() {
    let world = World::shared().await;
    let mut listener = PgListener::connect(world.db.url().expose()).await.unwrap();
    listener.listen(ADMIN_CHANNEL).await.unwrap();
    assert_eq!(ADMIN_CHANNEL, "arauto_admin");

    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let old = world.active(&name).await;
    let request = world.pending(&name).await;
    world
        .approve(&name, request.code, Some(Recognition::NotRecognized))
        .await;
    arauto_hub::repo::hermes::set_wake(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &name,
        true,
        world.now(),
    )
    .await
    .unwrap();
    arauto_hub::repo::closing::revoke(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &name,
        world.now(),
    )
    .await
    .unwrap();
    let other = world.hermes().await;
    let denied = world.pending(&other).await;
    deny(&world, &other).await;

    let id = |id: Uuid| id.as_hyphenated().to_string();
    let mut expected = vec![
        format!("quarantined:{}", id(old.device)),
        format!("approved:{}", id(request.device)),
        format!("hermes:{}", id(hermes)),
        format!("revoked:{}", id(old.device)),
        format!("revoked:{}", id(request.device)),
        format!("denied:{}", id(denied.device)),
    ];
    // Other tests share the database and the channel: wait for ours.
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        while !expected.is_empty() {
            let notification = listener.recv().await.unwrap();
            assert_eq!(notification.channel(), ADMIN_CHANNEL);
            let payload = notification.payload();
            assert!(payload.len() < 64, "a short text: {payload}");
            expected.retain(|wanted| wanted != payload);
        }
    })
    .await;
    assert!(waited.is_ok(), "the hub was never told of: {expected:?}");
}
