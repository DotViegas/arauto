//! The credential swap against the database: T1, T2, the presentation without swap, the
//! reconnect and the record of connections.

use std::time::Duration;

use arauto_hub::audit::AuditError;
use arauto_hub::core::model::{DeviceState, Slot, StateReason};
use arauto_hub::db::ResolvedOrg;
use arauto_hub::mark::{MARK_LOCK_FILE, Mark, MarkValue};
use arauto_hub::repo::RepoError;
use arauto_hub::repo::approval::Recognition;
use arauto_hub::repo::connection::{self, ChannelConnection, CloseReason};
use arauto_hub::repo::swap::{
    self, Activated, ConfirmOutcome, DoubleConnectionOutcome, IssueOutcome, LookOutcome,
    NotSwapped, SavedRefused, Seen, Stranger, UnknownOutcome,
};
use arauto_hub::restore::{self, EnterOutcome, Evidence};
use arauto_protocol::digest::CredentialDigest;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::{DAY_MS, HOUR_MS};
use arauto_protocol::message::{DeclaredText, Generation, Saved};
use arauto_protocol::noise::PublicKey;
use arauto_protocol::secret::SealedCredential;
use uuid::Uuid;

use crate::common;
use crate::support::{Connected, World, address, identity};

/// A valid seal of this organization for `identity`, at any generation the test wants.
fn seal(world: &World, identity: &PublicKey, generation: u64) -> SealedCredential {
    world
        .key
        .seal_credential(identity, Generation::new(generation).unwrap(), world.now())
        .unwrap()
        .sealed
}

/// Makes what a device presents out of what it holds.
type Forge = Box<dyn Fn(&Connected) -> SealedCredential>;

fn garbage() -> SealedCredential {
    SealedCredential::from_slice(&[0xa5; 150]).unwrap()
}

async fn look(
    world: &World,
    connection: &ChannelConnection,
    presented: Option<&SealedCredential>,
) -> LookOutcome {
    swap::look(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.key,
        connection,
        presented.map(SealedCredential::expose),
        world.now(),
    )
    .await
    .unwrap()
}

async fn rotate(world: &World, connection: &ChannelConnection) -> IssueOutcome {
    swap::rotate(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.key,
        connection,
        world.now(),
    )
    .await
    .unwrap()
}

/// The generations a device row holds: current, pending and the largest.
async fn generations(world: &World, device: Uuid) -> (u64, Option<u64>, u64) {
    let device = world.device(device).await;
    (
        device.current.map_or(0, |c| c.generation.get()),
        device.pending.map(|c| c.generation.get()),
        device.max_generation,
    )
}

/// The transaction that last wrote each of these device rows.
async fn written_by(world: &World, devices: &[Uuid]) -> Vec<String> {
    let mut found = Vec::new();
    for device in devices {
        let xmin: String = sqlx::query_scalar("SELECT xmin::text FROM device WHERE id = $1")
            .bind(device)
            .fetch_one(world.pool())
            .await
            .unwrap();
        found.push(xmin);
    }
    found
}

// ---------------------------------------------------------------------------- the table of the swap

#[tokio::test]
async fn line_1_nothing_at_generation_zero_is_accepted_and_the_first_saved_activates() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let request = world.approved(&name, None).await;
    world.clock.advance(5_000);
    let connection = world.connect(request.device).await;

    // T1: the first credential is the pending one, durably, before it is sent.
    let issued = world.issued(&connection, None).await;
    assert!(!issued.promoted);
    assert_eq!(issued.activated, None);
    assert_eq!(issued.receipt.credential().generation.get(), 1);
    assert_eq!(
        issued.receipt.credential().digest,
        CredentialDigest::of(&issued.sealed),
        "the hub keeps the SHA-256 of the block it sends"
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.slot, device.first_confirmed_at),
        (DeviceState::Approved, Some(Slot::Request), None),
        "still at generation zero until the saved"
    );
    assert_eq!(device.current, None);
    assert_eq!(device.pending, Some(issued.receipt.credential()));
    assert_eq!(device.max_generation, 1);
    // What was sent opens for this device, and says the generation that was written.
    let opened = world
        .key
        .open_credential(issued.sealed.expose(), &request.identity)
        .unwrap();
    assert_eq!(opened.generation.get(), 1);
    assert_eq!(opened.issued_at, world.now());

    // T2: the pending one is the current one, and the device is in use.
    world.clock.advance(700);
    let confirmed = world.confirm(&connection, &issued).await;
    let ConfirmOutcome::Confirmed { activated, session } = confirmed else {
        panic!("not confirmed: {confirmed:?}");
    };
    assert_eq!(activated, Some(Activated { replaced: None }));
    assert_eq!(
        (session.hermes, session.name, session.can_be_woken),
        (hermes, name, false)
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (
            device.state,
            device.slot,
            device.first_confirmed_at,
            device.state_since
        ),
        (
            DeviceState::Active,
            Some(Slot::InUse),
            Some(world.now()),
            world.now()
        )
    );
    assert_eq!(device.current, Some(issued.receipt.credential()));
    assert_eq!((device.pending, device.max_generation), (None, 1));
    assert_eq!(device.approved_until, None);
    assert_eq!(
        (
            device.last_contact_at,
            device.last_address.as_deref(),
            device.proto
        ),
        (Some(world.now()), Some("192.0.2.20"), Some(1))
    );
    assert_eq!(
        world.actions_about(request.device).await,
        [
            "device.approved",
            "device.activated",
            "credential.confirmed"
        ]
    );
}

#[tokio::test]
async fn line_1_the_current_one_is_accepted_and_a_waiting_pending_one_is_discarded() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    assert_eq!(generations(&world, active.device).await, (1, None, 1));

    let connection = world.connect(active.device).await;
    let first = world.issued(&connection, Some(&active.sealed)).await;
    assert_eq!(generations(&world, active.device).await, (1, Some(2), 2));
    assert_eq!(
        world.device(active.device).await.current,
        Some(active.credential)
    );

    // The connector never saved it and comes back with the current one: accepted again, and
    // the pending one that was waiting is discarded for a newer one.
    let connection = world.connect(active.device).await;
    let second = world.issued(&connection, Some(&active.sealed)).await;
    assert!(!second.promoted);
    assert_eq!(generations(&world, active.device).await, (1, Some(3), 3));
    assert_ne!(second.receipt.credential(), first.receipt.credential());

    // The discarded one is dead: presenting it is reuse.
    let confirmed = world.confirm(&connection, &second).await;
    assert!(matches!(
        confirmed,
        ConfirmOutcome::Confirmed {
            activated: None,
            ..
        }
    ));
    assert_eq!(generations(&world, active.device).await, (3, None, 3));
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, Some(&first.sealed)).await;
    assert!(
        matches!(outcome, IssueOutcome::Quarantined(StateReason::Reuse)),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn line_2_the_pending_one_is_promoted_and_another_is_issued() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;

    // The connector saved the new one and the hub never heard the "saved".
    let connection = world.connect(active.device).await;
    let saved_unheard = world.issued(&connection, Some(&active.sealed)).await;
    assert_eq!(generations(&world, active.device).await, (1, Some(2), 2));

    world.clock.advance(9_000);
    let connection = world.connect(active.device).await;
    let next = world.issued(&connection, Some(&saved_unheard.sealed)).await;
    assert!(next.promoted);
    assert_eq!(next.activated, None, "it was already active");
    let device = world.device(active.device).await;
    assert_eq!(device.current, Some(saved_unheard.receipt.credential()));
    assert_eq!(device.pending, Some(next.receipt.credential()));
    assert_eq!(generations(&world, active.device).await, (2, Some(3), 3));
    let confirmed: Vec<(i64, String)> = sqlx::query_as(
        "SELECT (details->>'generation')::bigint, details->>'by' FROM audit \
         WHERE organization_id = $1 AND action = 'credential.confirmed' AND target_id = $2 \
         ORDER BY seq",
    )
    .bind(world.org.id())
    .bind(active.device)
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert_eq!(
        confirmed,
        [(1, "saved".to_owned()), (2, "presentation".to_owned())]
    );

    // The one that was current died with the promotion.
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, Some(&active.sealed)).await;
    assert!(
        matches!(outcome, IssueOutcome::Quarantined(StateReason::Reuse)),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn line_2_at_generation_zero_presenting_the_first_one_activates_the_device() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    let connection = world.connect(request.device).await;
    let first = world.issued(&connection, None).await;

    // Nothing again: still generation zero, another first one.
    let connection = world.connect(request.device).await;
    let another = world.issued(&connection, None).await;
    assert_eq!(generations(&world, request.device).await, (0, Some(2), 2));
    assert_eq!(
        world.device(request.device).await.state,
        DeviceState::Approved
    );
    drop(first);

    // It saved that one, and the hub did not hear: presenting it is the first confirmation.
    let connection = world.connect(request.device).await;
    let next = world.issued(&connection, Some(&another.sealed)).await;
    assert!(next.promoted);
    assert_eq!(next.activated, Some(Activated { replaced: None }));
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.slot, device.first_confirmed_at),
        (DeviceState::Active, Some(Slot::InUse), Some(world.now()))
    );
    assert_eq!(generations(&world, request.device).await, (2, Some(3), 3));

    // Past generation zero, the identity alone is never enough again.
    let connection = world.connect(request.device).await;
    let outcome = world.issue(&connection, None).await;
    assert!(
        matches!(outcome, IssueOutcome::Quarantined(StateReason::Reuse)),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn line_3_a_swapped_credential_or_nothing_past_generation_zero_quarantines() {
    let world = World::shared().await;

    // A credential that was already swapped.
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let first = world.active(&name).await;
    let second = world.swap_again(&first).await;
    world.clock.advance(60_000);
    let connection = world.connect_from(first.device, address(99)).await;
    let before = world.device(first.device).await;
    let outcome = world.issue(&connection, Some(&first.sealed)).await;
    assert!(
        matches!(outcome, IssueOutcome::Quarantined(StateReason::Reuse)),
        "{outcome:?}"
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
            DeviceState::Quarantined,
            Some(StateReason::Reuse),
            Some(Slot::InUse),
            world.now()
        )
    );
    // Nothing was issued, and what the hub knows of the credentials stays for later.
    assert_eq!(
        (device.current, device.pending, device.max_generation),
        (before.current, before.pending, before.max_generation)
    );
    let alert: (String, Option<Uuid>, serde_json::Value) = sqlx::query_as(
        "SELECT kind, hermes_id, details FROM alert WHERE organization_id = $1 AND device_id = $2",
    )
    .bind(world.org.id())
    .bind(first.device)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!((alert.0.as_str(), alert.1), ("quarantined", Some(hermes)));
    assert_eq!(alert.2["reason"], "reuse");
    assert_eq!(alert.2["from"], "192.0.2.99");
    let event: (bool, Option<String>) = sqlx::query_as(
        "SELECT security, details->>'reason' FROM audit \
         WHERE organization_id = $1 AND action = 'device.quarantined' AND target_id = $2",
    )
    .bind(world.org.id())
    .bind(first.device)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(event, (true, Some("reuse".to_owned())));

    // There is no "release": the real one, with the real credential, is refused too.
    let connection = world.connect(first.device).await;
    let outcome = world.issue(&connection, Some(&second.sealed)).await;
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("a quarantined device swapped: {outcome:?}");
    };
    assert_eq!(
        not_swapped,
        NotSwapped {
            state: DeviceState::Quarantined,
            code: Some(ErrorCode::Ar501),
            seen: Seen::Nothing
        }
    );

    // Nothing, from a device that already confirmed a credential.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, None).await;
    assert!(
        matches!(outcome, IssueOutcome::Quarantined(StateReason::Reuse)),
        "{outcome:?}"
    );

    // The right number with another digest: the hub compares the digest, not the number.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let same_number = seal(&world, &active.identity, 1);
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, Some(&same_number)).await;
    assert!(
        matches!(outcome, IssueOutcome::Quarantined(StateReason::Reuse)),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn line_4_a_seal_that_does_not_open_quarantines_as_unreadable() {
    let world = World::shared().await;
    let other_name = world.hermes().await;
    let other = world.active(&other_name).await;

    let mut changed = Vec::new();
    let cases: [(&str, Forge); 3] = [
        ("garbage", Box::new(|_| garbage())),
        // A valid seal, of another device.
        ("of another device", {
            let sealed = SealedCredential::from_slice(other.sealed.expose()).unwrap();
            Box::new(move |_| SealedCredential::from_slice(sealed.expose()).unwrap())
        }),
        (
            "its own, with one bit changed",
            Box::new(|own: &Connected| {
                let mut bytes = own.sealed.expose().to_vec();
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
                SealedCredential::new(bytes).unwrap()
            }),
        ),
    ];
    for (what, make) in &cases {
        let name = world.hermes().await;
        let active = world.active(&name).await;
        let connection = world.connect(active.device).await;
        let outcome = world.issue(&connection, Some(&make(&active))).await;
        assert!(
            matches!(
                outcome,
                IssueOutcome::Quarantined(StateReason::UnreadableCredential)
            ),
            "{what}: {outcome:?}"
        );
        let device = world.device(active.device).await;
        assert_eq!(
            (device.state, device.state_reason),
            (
                DeviceState::Quarantined,
                Some(StateReason::UnreadableCredential)
            ),
            "{what}"
        );
        changed.push(active.device);
    }
    // The device whose seal was borrowed is not touched.
    assert_eq!(world.device(other.device).await.state, DeviceState::Active);
    for device in changed {
        assert_eq!(world.alerts_about(device).await, ["quarantined"]);
    }

    // At generation zero too.
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    let connection = world.connect(request.device).await;
    let outcome = world.issue(&connection, Some(&garbage())).await;
    assert!(
        matches!(
            outcome,
            IssueOutcome::Quarantined(StateReason::UnreadableCredential)
        ),
        "{outcome:?}"
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.slot),
        (DeviceState::Quarantined, Some(Slot::Request)),
        "a request that never confirmed keeps the place of a request"
    );
}

#[tokio::test]
async fn line_5_a_seal_beyond_the_database_is_proof_of_a_restoration_and_writes_nothing() {
    let world = World::own().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let bystander_name = world.hermes().await;
    let bystander = world.active(&bystander_name).await;
    let from_the_lost_timeline = seal(&world, &active.identity, 7);
    let before = world.device(active.device).await;
    let rows = world.audit_rows().await;

    let connection = world.connect(active.device).await;
    let outcome = world
        .issue(&connection, Some(&from_the_lost_timeline))
        .await;
    let IssueOutcome::Restoration(evidence) = outcome else {
        panic!("not taken for a restoration: {outcome:?}");
    };
    assert_eq!(
        evidence,
        Evidence::SealBeyondDatabase {
            device: active.device
        }
    );
    assert_eq!(
        world.device(active.device).await,
        before,
        "nothing was written"
    );
    assert_eq!(world.audit_rows().await, rows);

    // The hub turns the mode on with that evidence: every active device is suspended.
    let entered = restore::enter(world.pool(), &world.home.mark, world.now(), evidence)
        .await
        .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));
    for device in [active.device, bystander.device] {
        let device = world.device(device).await;
        assert_eq!(
            (device.state, device.marked_by_restoration),
            (DeviceState::Suspended, true)
        );
    }

    // A marked device is no proof any more: the same seal is only recorded, once.
    for _ in 0..3 {
        let connection = world.connect(active.device).await;
        let outcome = world
            .issue(&connection, Some(&from_the_lost_timeline))
            .await;
        let IssueOutcome::NotSwapped(not_swapped) = outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(
            not_swapped,
            NotSwapped {
                state: DeviceState::Suspended,
                code: Some(ErrorCode::Ar506),
                seen: Seen::Unexplained
            }
        );
    }
    let recorded = world.actions_about(active.device).await;
    assert_eq!(
        recorded
            .iter()
            .filter(|a| *a == "credential.unexplained")
            .count(),
        1,
        "{recorded:?}"
    );
    // A suspended device gets no credential, with the right one either.
    let connection = world.connect(bystander.device).await;
    let outcome = world.issue(&connection, Some(&bystander.sealed)).await;
    assert!(
        matches!(outcome, IssueOutcome::NotSwapped(_)),
        "{outcome:?}"
    );
}

// ---------------------------------------------------------------------------- T2

#[tokio::test]
async fn a_saved_that_is_not_about_the_pending_one_of_the_connection_is_refused() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    let issued = world.issued(&connection, Some(&active.sealed)).await;
    let before = world.device(active.device).await;
    let recorded = world.actions_about(active.device).await;
    let right = World::saved(&issued);
    let refused = ConfirmOutcome::Refused(SavedRefused::NotThePending);

    let confirm = |connection: ChannelConnection, receipt, saved: Saved| {
        let world = &world;
        async move {
            swap::confirm(
                world.pool(),
                &world.home.mark,
                world.org,
                &connection,
                receipt,
                &saved,
                world.now(),
            )
            .await
            .unwrap()
        }
    };

    // Another digest; another generation; the digest of the current one.
    let wrong_digest = Saved {
        digest: CredentialDigest::from_bytes([9; 32]),
        ..right
    };
    let wrong_generation = Saved {
        generation: Generation::new(1).unwrap(),
        ..right
    };
    let of_the_current = Saved {
        generation: active.credential.generation,
        digest: active.credential.digest,
    };
    for saved in [wrong_digest, wrong_generation, of_the_current] {
        assert_eq!(
            confirm(connection.clone(), &issued.receipt, saved).await,
            refused,
            "{saved:?}"
        );
    }
    // The right "saved", on another connection of the same device.
    let elsewhere = world.connect(active.device).await;
    assert_eq!(confirm(elsewhere, &issued.receipt, right).await, refused);
    // The receipt of another device.
    let other_name = world.hermes().await;
    let other = world.active(&other_name).await;
    let other_connection = world.connect(other.device).await;
    let other_issued = world.issued(&other_connection, Some(&other.sealed)).await;
    assert_eq!(
        confirm(
            connection.clone(),
            &other_issued.receipt,
            World::saved(&other_issued)
        )
        .await,
        refused
    );

    assert_eq!(
        world.device(active.device).await,
        before,
        "nothing was written"
    );
    assert_eq!(world.actions_about(active.device).await, recorded);

    // A pending one that a newer T1 discarded is not promoted by a late "saved".
    let newer_connection = world.connect(active.device).await;
    let newer = world.issued(&newer_connection, Some(&active.sealed)).await;
    assert_eq!(
        confirm(connection.clone(), &issued.receipt, right).await,
        refused
    );
    assert_eq!(
        world.device(active.device).await.pending,
        Some(newer.receipt.credential())
    );

    // And a "saved" heard twice does not promote what the 24-hour swap issued meanwhile.
    assert!(matches!(
        world.confirm(&newer_connection, &newer).await,
        ConfirmOutcome::Confirmed { .. }
    ));
    let IssueOutcome::Issued(rotated) = rotate(&world, &newer_connection).await else {
        panic!("the rotation did not issue");
    };
    assert_eq!(
        confirm(
            newer_connection.clone(),
            &newer.receipt,
            World::saved(&newer)
        )
        .await,
        refused
    );
    let device = world.device(active.device).await;
    assert_eq!(device.current, Some(newer.receipt.credential()));
    assert_eq!(device.pending, Some(rotated.receipt.credential()));
}

#[tokio::test]
async fn revoking_between_the_credential_and_the_saved_never_ends_in_active() {
    let world = World::shared().await;
    let revoke = |name| {
        let world = &world;
        async move {
            arauto_hub::repo::closing::revoke(
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
    };

    // An active device, in the middle of a swap.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    let issued = world.issued(&connection, Some(&active.sealed)).await;
    revoke(&name).await;
    let confirmed = world.confirm(&connection, &issued).await;
    assert_eq!(
        confirmed,
        ConfirmOutcome::Refused(SavedRefused::DeviceEnded {
            state: DeviceState::Revoked,
            code: Some(ErrorCode::Ar502)
        })
    );
    let device = world.device(active.device).await;
    assert_eq!((device.state, device.slot), (DeviceState::Revoked, None));
    assert_eq!(
        device.current,
        Some(active.credential),
        "the promotion was not written"
    );

    // An approved device, in the middle of its first swap: it never becomes active.
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    let connection = world.connect(request.device).await;
    let issued = world.issued(&connection, None).await;
    revoke(&name).await;
    let confirmed = world.confirm(&connection, &issued).await;
    assert!(
        matches!(
            confirmed,
            ConfirmOutcome::Refused(SavedRefused::DeviceEnded {
                state: DeviceState::Revoked,
                ..
            })
        ),
        "{confirmed:?}"
    );
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.slot, device.first_confirmed_at),
        (DeviceState::Revoked, None, None)
    );
    assert!(
        !world
            .actions_about(request.device)
            .await
            .contains(&"device.activated".to_owned())
    );
    // Nor by presenting what it saved, on a new connection.
    let connection = world.connect(request.device).await;
    let outcome = world.issue(&connection, Some(&issued.sealed)).await;
    assert!(
        matches!(outcome, IssueOutcome::NotSwapped(_)),
        "{outcome:?}"
    );
    assert_eq!(
        world.device(request.device).await.state,
        DeviceState::Revoked
    );

    // The same with a quarantine in the middle.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    let issued = world.issued(&connection, Some(&active.sealed)).await;
    let second = world.connect(active.device).await;
    assert!(matches!(
        world.issue(&second, None).await,
        IssueOutcome::Quarantined(StateReason::Reuse)
    ));
    let confirmed = world.confirm(&connection, &issued).await;
    assert!(
        matches!(
            confirmed,
            ConfirmOutcome::Refused(SavedRefused::DeviceEnded {
                state: DeviceState::Quarantined,
                code: Some(ErrorCode::Ar501)
            })
        ),
        "{confirmed:?}"
    );
    assert_eq!(
        world.device(active.device).await.state,
        DeviceState::Quarantined
    );
}

// ---------------------------------------------------------------------------- the order of the writes

#[tokio::test]
async fn a_swap_waits_for_whoever_holds_the_hermes_and_reads_the_device_again() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;

    // Another writer holds the row of the Hermes, the first lock of every write.
    let other_pool = world.db.second_pool().await;
    let mut holder = other_pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM hermes WHERE id = $1 FOR NO KEY UPDATE")
        .bind(hermes)
        .execute(&mut *holder)
        .await
        .unwrap();

    let waiting = {
        let pool = world.pool().clone();
        let mark = world.home.mark.clone();
        let org = world.org;
        let key = arauto_hub::repo::device::organization_key(&pool, org, &world.home.vault)
            .await
            .unwrap();
        let connection = connection.clone();
        let sealed = active.sealed.expose().to_vec();
        let now = world.now();
        tokio::spawn(async move {
            swap::issue(&pool, &mark, org, &key, &connection, Some(&sealed), now).await
        })
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !waiting.is_finished(),
        "T1 did not wait for the lock of the Hermes"
    );

    // While it waits, the device is revoked by the holder of the lock.
    sqlx::query(
        "UPDATE device SET state = 'revoked', state_reason = 'admin', slot = NULL WHERE id = $1",
    )
    .bind(active.device)
    .execute(&mut *holder)
    .await
    .unwrap();
    holder.commit().await.unwrap();

    // T1 goes on, reads the device again and issues nothing.
    let outcome = waiting.await.unwrap().unwrap();
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("T1 decided on what it read before the lock: {outcome:?}");
    };
    assert_eq!(
        (not_swapped.state, not_swapped.code),
        (DeviceState::Revoked, Some(ErrorCode::Ar502))
    );
    assert_eq!(generations(&world, active.device).await, (1, None, 1));
}

#[tokio::test]
async fn a_write_holds_the_hermes_and_every_device_of_it_until_it_commits() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let old = world.active(&name).await;
    let request = world.approved(&name, Some(Recognition::Recognized)).await;
    let connection = world.connect(request.device).await;

    // T1 is stopped at the row of its connection, the last lock it takes, with everything
    // it locked before still in its hands.
    let other_pool = world.db.second_pool().await;
    let mut holder = other_pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM channel_connection WHERE id = $1 FOR UPDATE")
        .bind(connection.id)
        .execute(&mut *holder)
        .await
        .unwrap();
    let waiting = {
        let pool = world.pool().clone();
        let mark = world.home.mark.clone();
        let org = world.org;
        let key = arauto_hub::repo::device::organization_key(&pool, org, &world.home.vault)
            .await
            .unwrap();
        let connection = connection.clone();
        let now = world.now();
        tokio::spawn(
            async move { swap::issue(&pool, &mark, org, &key, &connection, None, now).await },
        )
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(!waiting.is_finished());

    // The row of the Hermes, the device that is swapping and the other device of the same
    // Hermes, which this swap does not write: all three are locked against another writer.
    let probe = |table: &'static str, id: Uuid, strength: &'static str| {
        let pool = other_pool.clone();
        async move {
            let mut transaction = pool.begin().await.unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "SELECT id FROM {table} WHERE id = $1 FOR {strength} NOWAIT"
            )))
            .bind(id)
            .execute(&mut *transaction)
            .await
        }
    };
    for (table, id) in [
        ("hermes", hermes),
        ("device", request.device),
        ("device", old.device),
    ] {
        let refused = probe(table, id, "NO KEY UPDATE").await;
        let code = refused
            .expect_err("the row was not locked")
            .as_database_error()
            .and_then(|error| error.code().map(|code| code.into_owned()));
        assert_eq!(
            code.as_deref(),
            Some("55P03"),
            "{table}: lock not available"
        );
        // The lock is the one that lets a row that points at this one be inserted
        // meanwhile: NO KEY UPDATE, and not the stronger one.
        assert!(probe(table, id, "KEY SHARE").await.is_ok(), "{table}");
    }

    holder.rollback().await.unwrap();
    let outcome = waiting.await.unwrap().unwrap();
    assert!(matches!(outcome, IssueOutcome::Issued(_)), "{outcome:?}");
    // Committed: the locks are gone.
    assert!(probe("device", old.device, "NO KEY UPDATE").await.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_swaps_of_one_device_queue_up_and_each_gets_its_own_generation() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    const TASKS: usize = 6;

    let mut tasks = Vec::new();
    for _ in 0..TASKS {
        let connection = world.connect(active.device).await;
        let pool = world.pool().clone();
        let mark = world.home.mark.clone();
        let org: ResolvedOrg = world.org;
        let key = arauto_hub::repo::device::organization_key(&pool, org, &world.home.vault)
            .await
            .unwrap();
        let sealed = active.sealed.expose().to_vec();
        let now = world.now();
        tasks.push(tokio::spawn(async move {
            swap::issue(&pool, &mark, org, &key, &connection, Some(&sealed), now).await
        }));
    }
    let mut generations_issued = Vec::new();
    let mut last = None;
    for task in tasks {
        match task.await.unwrap().unwrap() {
            IssueOutcome::Issued(issued) => {
                generations_issued.push(issued.receipt.credential().generation.get());
                last = last.max(Some(issued.receipt.credential().generation.get()));
            }
            other => panic!("the current credential was not accepted: {other:?}"),
        }
    }
    // Each one read what the one before wrote: no generation was issued twice.
    generations_issued.sort_unstable();
    assert_eq!(generations_issued, [2, 3, 4, 5, 6, 7]);
    assert_eq!(generations(&world, active.device).await, (1, last, 7));
}

// ---------------------------------------------------------------------------- reconnect

#[tokio::test]
async fn the_first_confirmation_of_a_new_device_replaces_the_old_one_in_the_same_write() {
    let world = World::shared().await;
    for old_state in ["active", "suspended", "quarantined"] {
        let name = world.hermes().await;
        let hermes = world.hermes_id(&name).await;
        let old = world.active(&name).await;
        if old_state != "active" {
            sqlx::query("UPDATE device SET state = $2, state_reason = 'reuse' WHERE id = $1")
                .bind(old.device)
                .bind(old_state)
                .execute(world.pool())
                .await
                .unwrap();
        }
        let request = world.approved(&name, Some(Recognition::Recognized)).await;
        world.clock.advance(30_000);

        // Until the new one confirms, each one is in its own place.
        let connection = world.connect(request.device).await;
        let issued = world.issued(&connection, None).await;
        let slots = |device: Uuid| {
            let world = &world;
            async move { world.device(device).await.slot }
        };
        assert_eq!(slots(old.device).await, Some(Slot::InUse));
        assert_eq!(slots(request.device).await, Some(Slot::Request));

        let confirmed = world.confirm(&connection, &issued).await;
        let ConfirmOutcome::Confirmed { activated, .. } = confirmed else {
            panic!("{old_state}: not confirmed: {confirmed:?}");
        };
        assert_eq!(
            activated,
            Some(Activated {
                replaced: Some(old.device)
            })
        );

        let new = world.device(request.device).await;
        let replaced = world.device(old.device).await;
        assert_eq!(
            (new.state, new.slot),
            (DeviceState::Active, Some(Slot::InUse))
        );
        assert_eq!(
            (
                replaced.state,
                replaced.state_reason,
                replaced.slot,
                replaced.state_since
            ),
            (
                DeviceState::Replaced,
                Some(StateReason::Reconnected),
                None,
                world.now()
            ),
            "{old_state}"
        );
        // What the hub knows of the old credential stays, for a reuse that comes later.
        assert_eq!(replaced.current, Some(old.credential));
        let writers = written_by(&world, &[request.device, old.device]).await;
        assert_eq!(writers[0], writers[1], "{old_state}: two writes, not one");
        let in_use: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM device WHERE hermes_id = $1 AND slot = 'in_use'",
        )
        .bind(hermes)
        .fetch_one(world.pool())
        .await
        .unwrap();
        assert_eq!(in_use, 1);
        assert_eq!(
            world
                .actions_about(old.device)
                .await
                .last()
                .map(String::as_str),
            Some("device.replaced")
        );

        // The old device is told it was replaced, and swaps nothing.
        let connection = world.connect(old.device).await;
        let outcome = world.issue(&connection, Some(&old.sealed)).await;
        let IssueOutcome::NotSwapped(not_swapped) = outcome else {
            panic!("{old_state}: {outcome:?}");
        };
        assert_eq!(
            (not_swapped.state, not_swapped.code),
            (DeviceState::Replaced, Some(ErrorCode::Ar503))
        );
    }
}

#[tokio::test]
async fn a_first_confirmation_by_presentation_replaces_the_old_device_too() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let old = world.active(&name).await;
    let request = world.approved(&name, Some(Recognition::Recognized)).await;
    let connection = world.connect(request.device).await;
    let first = world.issued(&connection, None).await;
    assert_eq!(world.device(old.device).await.state, DeviceState::Active);

    // The new connector saved the first one, the hub did not hear, and it comes back with it.
    let connection = world.connect(request.device).await;
    let next = world.issued(&connection, Some(&first.sealed)).await;
    assert_eq!(
        next.activated,
        Some(Activated {
            replaced: Some(old.device)
        })
    );
    let writers = written_by(&world, &[request.device, old.device]).await;
    assert_eq!(writers[0], writers[1]);
    assert_eq!(world.device(old.device).await.state, DeviceState::Replaced);
    assert_eq!(
        world.device(request.device).await.state,
        DeviceState::Active
    );
}

#[tokio::test]
async fn the_database_refuses_a_second_device_in_use_of_the_same_hermes() {
    // The rule the write above depends on, held by the schema itself.
    let world = World::shared().await;
    let hermes = common::hermes(world.pool(), world.org).await;
    common::device(
        world.pool(),
        world.org,
        hermes,
        "active",
        common::Confirmed::Yes,
    )
    .await;
    let second = sqlx::query(
        "INSERT INTO device (id, organization_id, hermes_id, public_key, state, state_since, \
                             slot, first_confirmed_at, created_at) \
         VALUES ($1, $2, $3, $4, 'quarantined', 1, 'in_use', 1, 1)",
    )
    .bind(Uuid::new_v4())
    .bind(world.org.id())
    .bind(hermes)
    .bind(identity().as_bytes().as_slice())
    .execute(world.pool())
    .await;
    let error = second.unwrap_err().to_string();
    assert!(error.contains("device_slot"), "{error}");
}

// ---------------------------------------------------------------------------- the 24-hour swap

#[tokio::test]
async fn the_24_hour_swap_issues_inside_the_session_and_never_over_a_waiting_one() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    world.clock.advance(DAY_MS);

    let IssueOutcome::Issued(rotated) = rotate(&world, &active.connection).await else {
        panic!("the rotation did not issue");
    };
    assert!(!rotated.promoted);
    assert_eq!(generations(&world, active.device).await, (1, Some(2), 2));
    // While that one waits for its "saved", nothing else is issued.
    assert!(matches!(
        rotate(&world, &active.connection).await,
        IssueOutcome::NotDue
    ));
    assert_eq!(generations(&world, active.device).await, (1, Some(2), 2));

    let confirmed = world.confirm(&active.connection, &rotated).await;
    assert!(matches!(
        confirmed,
        ConfirmOutcome::Confirmed {
            activated: None,
            ..
        }
    ));
    assert_eq!(generations(&world, active.device).await, (2, None, 2));
    assert_eq!(
        world.device(active.device).await.last_contact_at,
        Some(world.now())
    );

    // A device that is no longer active rotates nothing.
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
    let outcome = rotate(&world, &active.connection).await;
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(not_swapped.code, Some(ErrorCode::Ar502));
    // Nor an approved one, which has no current credential to rotate and no session.
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    let connection = world.connect(request.device).await;
    assert!(matches!(
        rotate(&world, &connection).await,
        IssueOutcome::NotASession
    ));
    assert_eq!(generations(&world, request.device).await, (0, None, 0));
}

/// C7, rule 8: past generation zero the hub never issues on the identity alone. The 24-hour
/// swap issues "as if the current credential had been presented", which is only true of a
/// session: the hub reads that in the record of the connection, under the lock.
#[tokio::test]
async fn the_24_hour_swap_never_issues_on_a_connection_that_only_proved_the_identity() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let before = world.device(active.device).await;

    // Whoever holds only the identity of the device opens a connection and presents nothing.
    let intruder = world.connect_from(active.device, address(66)).await;
    assert!(matches!(
        rotate(&world, &intruder).await,
        IssueOutcome::NotASession
    ));
    assert_eq!(world.device(active.device).await, before);
    assert!(world.alerts_about(active.device).await.is_empty());

    // The device that is really there goes on: its credential is still the current one.
    let swapped = world.swap_again(&active).await;
    assert_eq!(generations(&world, active.device).await, (2, None, 2));

    // A connection that received a credential and did not confirm it is no session either.
    let half = world.connect(active.device).await;
    let issued = world.issued(&half, Some(&swapped.sealed)).await;
    assert!(matches!(
        rotate(&world, &half).await,
        IssueOutcome::NotASession
    ));
    // Once it confirms, it is one, and the 24-hour swap issues on it.
    assert!(matches!(
        world.confirm(&half, &issued).await,
        ConfirmOutcome::Confirmed { .. }
    ));
    assert!(matches!(
        rotate(&world, &half).await,
        IssueOutcome::Issued(_)
    ));
}

/// Nothing is issued, promoted or looked at on a connection the record shows as closed, or
/// that is not a connection of that device.
#[tokio::test]
async fn a_connection_that_already_ended_gets_no_credential_and_no_promotion() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;

    // An attempt that was replaced by a newer one (AR-206) and arrives late at T1.
    let late = world.connect(active.device).await;
    connection::close(
        world.pool(),
        world.org,
        &late,
        CloseReason::ReplacedAttempt,
        world.now(),
    )
    .await
    .unwrap();
    let before = world.device(active.device).await;
    assert!(matches!(
        world.issue(&late, Some(&active.sealed)).await,
        IssueOutcome::ConnectionClosed
    ));
    assert_eq!(
        look(&world, &late, Some(&active.sealed)).await,
        LookOutcome::ConnectionClosed
    );
    // Not even what would quarantine is judged there: it decides nothing.
    assert!(matches!(
        world.issue(&late, None).await,
        IssueOutcome::ConnectionClosed
    ));
    assert_eq!(world.device(active.device).await, before);

    // A session that was closed rotates nothing.
    connection::close(
        world.pool(),
        world.org,
        &active.connection,
        CloseReason::Dropped,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(
        rotate(&world, &active.connection).await,
        IssueOutcome::ConnectionClosed
    ));

    // T2: the "saved" of a connection that was closed after its credential promotes nothing.
    let open = world.connect(active.device).await;
    let issued = world.issued(&open, Some(&active.sealed)).await;
    connection::close(
        world.pool(),
        world.org,
        &open,
        CloseReason::SavedTimeout,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(
        world.confirm(&open, &issued).await,
        ConfirmOutcome::Refused(SavedRefused::ConnectionClosed)
    );
    assert_eq!(generations(&world, active.device).await, (1, Some(2), 2));

    // A connection of another device is not a connection of this one.
    let other_name = world.hermes().await;
    let other = world.active(&other_name).await;
    let borrowed = ChannelConnection {
        device: active.device,
        ..other.connection.clone()
    };
    assert!(matches!(
        world.issue(&borrowed, Some(&active.sealed)).await,
        IssueOutcome::ConnectionClosed
    ));
    assert!(matches!(
        rotate(&world, &borrowed).await,
        IssueOutcome::ConnectionClosed
    ));
    assert_eq!(generations(&world, active.device).await, (1, Some(2), 2));
    assert_eq!(generations(&world, other.device).await, (1, None, 1));
}

// ---------------------------------------------------------------------------- the presentation without swap

#[tokio::test]
async fn an_outdated_connector_is_looked_at_and_never_given_a_credential() {
    let world = World::shared().await;

    // What would be accepted in a swap is nothing here: the caller answers AR-401.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let before = world.device(active.device).await;
    let connection = world.connect(active.device).await;
    let outcome = look(&world, &connection, Some(&active.sealed)).await;
    assert_eq!(
        outcome,
        LookOutcome::NotSwapped(NotSwapped {
            state: DeviceState::Active,
            code: None,
            seen: Seen::Nothing
        })
    );
    assert_eq!(world.device(active.device).await, before);

    // The last three lines of the table still hold.
    let swapped = world.swap_again(&active).await;
    let connection = world.connect(active.device).await;
    assert_eq!(
        look(&world, &connection, Some(&active.sealed)).await,
        LookOutcome::Quarantined(StateReason::Reuse)
    );
    assert_eq!(
        world.device(active.device).await.state,
        DeviceState::Quarantined
    );
    drop(swapped);

    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    assert_eq!(
        look(&world, &connection, Some(&garbage())).await,
        LookOutcome::Quarantined(StateReason::UnreadableCredential)
    );

    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    assert_eq!(
        look(&world, &connection, None).await,
        LookOutcome::Quarantined(StateReason::Reuse)
    );

    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    assert_eq!(
        look(
            &world,
            &connection,
            Some(&seal(&world, &active.identity, 40))
        )
        .await,
        LookOutcome::Restoration(Evidence::SealBeyondDatabase {
            device: active.device
        })
    );
    assert_eq!(world.device(active.device).await.state, DeviceState::Active);
}

#[tokio::test]
async fn a_closed_device_with_an_older_seal_raises_reuse_after_closing() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let first = world.active(&name).await;
    let second = world.swap_again(&first).await;
    let third = world.swap_again(&second).await;
    // Someone holds a copy of the first credential; the device is revoked before the real
    // one comes back.
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
    world.clock.advance(HOUR_MS);

    // The last current one, presented by the real device: only its state.
    let connection = world.connect(first.device).await;
    let ended = |seen| {
        LookOutcome::NotSwapped(NotSwapped {
            state: DeviceState::Revoked,
            code: Some(ErrorCode::Ar502),
            seen,
        })
    };
    assert_eq!(
        look(&world, &connection, Some(&third.sealed)).await,
        ended(Seen::Nothing)
    );
    assert_eq!(look(&world, &connection, None).await, ended(Seen::Nothing));
    assert_eq!(
        look(&world, &connection, Some(&garbage())).await,
        ended(Seen::Nothing)
    );
    assert!(world.alerts_about(first.device).await.is_empty());

    // An older one: the alert.
    let connection = world.connect_from(first.device, address(123)).await;
    for old in [&first.sealed, &second.sealed, &first.sealed] {
        assert_eq!(
            look(&world, &connection, Some(old)).await,
            ended(Seen::ReuseAfterClosing)
        );
    }
    let alerts: Vec<(String, Option<Uuid>, serde_json::Value)> = sqlx::query_as(
        "SELECT kind, hermes_id, details FROM alert WHERE organization_id = $1 AND device_id = $2",
    )
    .bind(world.org.id())
    .bind(first.device)
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert_eq!(
        alerts.len(),
        1,
        "one alert per device per 24 hours: {alerts:?}"
    );
    assert_eq!(
        (alerts[0].0.as_str(), alerts[0].1),
        ("reuse_after_closing", Some(hermes))
    );
    assert_eq!(alerts[0].2["from"], "192.0.2.123");
    assert_eq!(alerts[0].2["state"], "revoked");
    // The same through the swap path, for a connection that was told `active` and lost a race.
    let outcome = world.issue(&connection, Some(&first.sealed)).await;
    assert!(
        matches!(
            outcome,
            IssueOutcome::NotSwapped(NotSwapped {
                seen: Seen::ReuseAfterClosing,
                ..
            })
        ),
        "{outcome:?}"
    );
    // A day later it alerts again.
    world.clock.advance(DAY_MS);
    look(&world, &connection, Some(&first.sealed)).await;
    assert_eq!(world.alerts_about(first.device).await.len(), 2);
    assert_eq!(world.device(first.device).await.state, DeviceState::Revoked);
}

#[tokio::test]
async fn a_request_closed_by_a_restoration_with_a_valid_seal_is_listed_and_not_recognized() {
    let world = World::own().await;
    let name = world.hermes().await;
    let request = world.approved(&name, None).await;
    let connection = world.connect(request.device).await;
    let issued = world.issued(&connection, None).await;

    // The hub comes back from a copy: the approved request is closed and marked.
    let entered = restore::enter(
        world.pool(),
        &world.home.mark,
        world.now(),
        Evidence::SealBeyondDatabase {
            device: request.device,
        },
    )
    .await
    .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));
    let device = world.device(request.device).await;
    assert_eq!(
        (device.state, device.state_reason),
        (DeviceState::Expired, Some(StateReason::RestoredApproved))
    );

    // With nothing, or with something that does not open: the code of its state.
    let connection = world.connect_from(request.device, address(44)).await;
    for presented in [None, Some(garbage())] {
        let outcome = look(&world, &connection, presented.as_ref()).await;
        let LookOutcome::NotSwapped(not_swapped) = outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(
            (not_swapped.code, not_swapped.seen),
            (Some(ErrorCode::Ar303), Seen::Nothing)
        );
    }
    // With the seal the lost timeline gave it: listed, and told the hub does not know it.
    let outcome = look(&world, &connection, Some(&issued.sealed)).await;
    let LookOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(not_swapped.code, Some(ErrorCode::Ar504));
    let Seen::Listed(listed) = not_swapped.seen else {
        panic!("not listed: {not_swapped:?}");
    };
    assert!(listed.newly_listed);
    assert!(
        !listed.proof_pending,
        "a known request is never proof again"
    );
    let row: (Vec<u8>, Option<String>) = sqlx::query_as(
        "SELECT public_key, address FROM unknown_device WHERE organization_id = $1 AND id = $2",
    )
    .bind(world.org.id())
    .bind(listed.id)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            request.identity.as_bytes().to_vec(),
            Some("192.0.2.44".to_owned())
        )
    );
    // Listed once.
    let LookOutcome::NotSwapped(again) = look(&world, &connection, Some(&issued.sealed)).await
    else {
        panic!("not looked at");
    };
    assert_eq!(
        again.seen,
        Seen::Listed(arauto_hub::restore::UnknownDevice {
            id: listed.id,
            newly_listed: false,
            proof_pending: false
        })
    );
    assert_eq!(
        world.device(request.device).await.state,
        DeviceState::Expired
    );
}

/// C11: what the lost timeline gave such a device is, almost always, a seal of a generation
/// above every one the copy knows: in the copy the request was pending, or approved with
/// nothing issued. The rule calls that seal "unexplained"; the device is listed all the same.
#[tokio::test]
async fn a_request_the_restoration_closed_is_listed_whatever_the_generation_of_its_seal() {
    let world = World::own().await;
    let pending_name = world.hermes().await;
    let pending = world.pending(&pending_name).await;
    let approved_name = world.hermes().await;
    let approved = world.approved(&approved_name, None).await;

    // The hub comes back from that copy. After it, both requests had been approved, had
    // confirmed and had swapped up to generation 3.
    let entered = restore::enter(
        world.pool(),
        &world.home.mark,
        world.now(),
        Evidence::SealBeyondDatabase {
            device: approved.device,
        },
    )
    .await
    .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));
    let restored_at = restore::installation(world.pool())
        .await
        .unwrap()
        .unwrap()
        .restored_at;

    for (request, code_of_state) in [(&pending, ErrorCode::Ar302), (&approved, ErrorCode::Ar303)] {
        let device = world.device(request.device).await;
        assert!(device.marked_by_restoration && device.max_generation == 0);
        let connection = world.connect_from(request.device, address(45)).await;
        // With nothing, the code of its state, as always.
        let LookOutcome::NotSwapped(nothing) = look(&world, &connection, None).await else {
            panic!("not looked at");
        };
        assert_eq!(
            (nothing.code, nothing.seen),
            (Some(code_of_state), Seen::Nothing)
        );

        let from_lost_timeline = seal(&world, &request.identity, 3);
        let outcome = look(&world, &connection, Some(&from_lost_timeline)).await;
        let LookOutcome::NotSwapped(not_swapped) = outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(
            not_swapped.code,
            Some(ErrorCode::Ar504),
            "a Hermes connected after the copy is told the hub does not recognize it"
        );
        let Seen::Listed(listed) = not_swapped.seen else {
            panic!("not listed: {not_swapped:?}");
        };
        assert!(listed.newly_listed && !listed.proof_pending);
        let row: (Vec<u8>, Option<String>) = sqlx::query_as(
            "SELECT public_key, address FROM unknown_device \
             WHERE organization_id = $1 AND id = $2",
        )
        .bind(world.org.id())
        .bind(listed.id)
        .fetch_one(world.pool())
        .await
        .unwrap();
        assert_eq!(
            row,
            (
                request.identity.as_bytes().to_vec(),
                Some("192.0.2.45".to_owned())
            )
        );
        // The seal the database does not explain is on record as well, once.
        let again = look(&world, &connection, Some(&from_lost_timeline)).await;
        assert!(
            matches!(
                again,
                LookOutcome::NotSwapped(NotSwapped {
                    code: Some(ErrorCode::Ar504),
                    seen: Seen::Listed(again),
                    ..
                }) if again.id == listed.id && !again.newly_listed
            ),
            "{again:?}"
        );
        let recorded = world.actions_about(request.device).await;
        assert_eq!(
            recorded
                .iter()
                .filter(|action| *action == "credential.unexplained")
                .count(),
            1,
            "{recorded:?}"
        );
        // The same through the swap path, for a connection that lost a race.
        let outcome = world.issue(&connection, Some(&from_lost_timeline)).await;
        assert!(
            matches!(
                outcome,
                IssueOutcome::NotSwapped(NotSwapped {
                    code: Some(ErrorCode::Ar504),
                    seen: Seen::Listed(_),
                    ..
                })
            ),
            "{outcome:?}"
        );
        assert_eq!(
            world.device(request.device).await.state,
            DeviceState::Expired
        );
    }
    // The device is marked: it listed itself, and it did not start the mode again.
    let listed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM unknown_device WHERE organization_id = $1 AND NOT proof_pending",
    )
    .bind(world.org.id())
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(listed, 2);
    assert_eq!(
        restore::installation(world.pool())
            .await
            .unwrap()
            .unwrap()
            .restored_at,
        restored_at
    );
}

/// C7: a valid seal of a generation above every one the database knows is looked at in every
/// state. A deadline that passed is applied by the swap itself when it meets it, and that
/// must not come first: the device would be expired and the proof would not be seen.
#[tokio::test]
async fn proof_of_a_restoration_is_seen_before_a_deadline_that_passed_is_applied() {
    let world = World::shared().await;

    // An active device that has not connected for 31 days.
    let name = world.hermes().await;
    let active = world.active(&name).await;
    world.clock.advance(31 * DAY_MS);
    let before = world.device(active.device).await;
    let beyond = seal(&world, &active.identity, 40);
    let proof = Evidence::SealBeyondDatabase {
        device: active.device,
    };
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, Some(&beyond)).await;
    assert!(
        matches!(outcome, IssueOutcome::Restoration(evidence) if evidence == proof),
        "{outcome:?}"
    );
    assert_eq!(
        look(&world, &connection, Some(&beyond)).await,
        LookOutcome::Restoration(proof)
    );
    // Nothing was written: the device was not expired on the way.
    assert_eq!(world.device(active.device).await, before);

    // An approved request, more than 24 hours after its approval.
    let name = world.hermes().await;
    let approved = world.approved(&name, None).await;
    world.clock.advance(DAY_MS);
    let connection = world.connect(approved.device).await;
    let outcome = world
        .issue(&connection, Some(&seal(&world, &approved.identity, 1)))
        .await;
    assert!(
        matches!(outcome, IssueOutcome::Restoration(_)),
        "{outcome:?}"
    );
    assert_eq!(
        world.device(approved.device).await.state,
        DeviceState::Approved
    );

    // With anything else the deadline is applied as before.
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, Some(&active.sealed)).await;
    let IssueOutcome::NotSwapped(not_swapped) = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(
        (not_swapped.state, not_swapped.code),
        (DeviceState::Expired, Some(ErrorCode::Ar507))
    );
}

#[tokio::test]
async fn an_unknown_identity_is_listed_only_with_a_valid_seal_of_the_organization() {
    let world = World::own().await;
    let other_org = common::organization(world.pool(), &world.home).await;
    let other_key =
        arauto_hub::repo::device::organization_key(world.pool(), other_org, &world.home.vault)
            .await
            .unwrap();
    let who = identity();
    let version = DeclaredText::parse("0.1.0").unwrap();
    let listed_rows = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM unknown_device")
            .fetch_one(world.pool())
            .await
            .unwrap()
    };
    let ask = |org: ResolvedOrg, key, presented: Option<SealedCredential>| {
        let world = &world;
        let version = &version;
        async move {
            swap::look_unknown(
                world.pool(),
                org,
                key,
                &Stranger {
                    identity: &who,
                    presented: presented.as_ref().map(SealedCredential::expose),
                    connector_version: Some(version),
                    address: address(55),
                },
                world.now(),
            )
            .await
        }
    };

    // Nothing, garbage, a seal for another identity, and a seal of another organization:
    // nothing is recorded for each attempt.
    let rows = world.audit_rows().await;
    for presented in [
        None,
        Some(garbage()),
        Some(seal(&world, &identity(), 3)),
        Some(
            other_key
                .seal_credential(&who, Generation::new(3).unwrap(), 1)
                .unwrap()
                .sealed,
        ),
    ] {
        assert_eq!(
            ask(world.org, &world.key, presented).await.unwrap(),
            UnknownOutcome::NotOurs
        );
    }
    assert_eq!(listed_rows().await, 0);
    assert_eq!(world.audit_rows().await, rows);
    // A key that is not of the organization asked about is refused.
    assert!(matches!(
        ask(world.org, &other_key, None).await,
        Err(RepoError::Inconsistent(_))
    ));

    // A valid seal: listed, and proof of a restoration the first time.
    let valid = || seal(&world, &who, 3);
    let outcome = ask(world.org, &world.key, Some(valid())).await.unwrap();
    let UnknownOutcome::Listed { unknown, evidence } = outcome else {
        panic!("not listed: {outcome:?}");
    };
    assert!(unknown.newly_listed && unknown.proof_pending);
    assert_eq!(
        evidence,
        Some(Evidence::UnknownDeviceSeal {
            unknown_device: unknown.id
        })
    );
    // The hub did not get to turn the mode on (it stopped, or the write failed). The same
    // device presents again: the row is the same one, and it is still proof. After a
    // restoration of the whole server by image this may be the only device that can tell.
    let again = ask(world.org, &world.key, Some(valid())).await.unwrap();
    assert_eq!(
        again,
        UnknownOutcome::Listed {
            unknown: arauto_hub::restore::UnknownDevice {
                id: unknown.id,
                newly_listed: false,
                proof_pending: true
            },
            evidence
        }
    );
    assert_eq!(listed_rows().await, 1);

    // The mode is turned on with it. From then on the same device is no proof again.
    let entered = restore::enter(
        world.pool(),
        &world.home.mark,
        world.now(),
        evidence.unwrap(),
    )
    .await
    .unwrap();
    assert!(matches!(entered, EnterOutcome::Entered(_)));
    assert_eq!(
        ask(world.org, &world.key, Some(valid())).await.unwrap(),
        UnknownOutcome::Listed {
            unknown: arauto_hub::restore::UnknownDevice {
                id: unknown.id,
                newly_listed: false,
                proof_pending: false
            },
            evidence: None
        }
    );
    // With the restored mode on, a new unknown device is listed and is no new proof.
    let stranger = identity();
    let outcome = swap::look_unknown(
        world.pool(),
        world.org,
        &world.key,
        &Stranger {
            identity: &stranger,
            presented: Some(seal(&world, &stranger, 1).expose()),
            connector_version: None,
            address: None,
        },
        world.now(),
    )
    .await
    .unwrap();
    assert!(
        matches!(
            outcome,
            UnknownOutcome::Listed { unknown, evidence: None } if unknown.newly_listed
        ),
        "{outcome:?}"
    );
}

// ---------------------------------------------------------------------------- double connection

#[tokio::test]
async fn a_double_connection_quarantines_the_device() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let quarantine = |device| {
        let world = &world;
        async move {
            swap::quarantine_double_connection(
                world.pool(),
                &world.home.mark,
                world.org,
                device,
                address(88),
                world.now(),
            )
            .await
            .unwrap()
        }
    };
    assert_eq!(
        quarantine(active.device).await,
        DoubleConnectionOutcome::Quarantined
    );
    let device = world.device(active.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (
            DeviceState::Quarantined,
            Some(StateReason::DoubleConnection),
            Some(Slot::InUse)
        )
    );
    assert_eq!(world.alerts_about(active.device).await, ["quarantined"]);
    // Read again before writing: a device that is no longer served is left as it is.
    assert_eq!(
        quarantine(active.device).await,
        DoubleConnectionOutcome::NotServing(DeviceState::Quarantined)
    );
    assert_eq!(world.alerts_about(active.device).await.len(), 1);
}

// ---------------------------------------------------------------------------- the record of connections

async fn close(world: &World, connection: &ChannelConnection) -> connection::ClosedConnection {
    connection::close(
        world.pool(),
        world.org,
        connection,
        CloseReason::SavedTimeout,
        world.now(),
    )
    .await
    .unwrap()
}

/// One connection that receives a credential and ends without its "saved".
async fn unconfirmed(world: &World, active: &Connected) -> connection::ClosedConnection {
    let connection = world.connect(active.device).await;
    world.issued(&connection, Some(&active.sealed)).await;
    close(world, &connection).await
}

#[tokio::test]
async fn three_connections_without_the_saved_in_24_hours_raise_the_alert() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let mut active = world.active(&name).await;

    // A confirmed connection is no count at all.
    let closed = close(&world, &active.connection).await;
    assert!(!closed.unconfirmed && !closed.alerted);
    // A connection that only opened and closed, neither.
    let idle = world.connect(active.device).await;
    assert!(!close(&world, &idle).await.unconfirmed);
    // Closing twice records once.
    assert!(!close(&world, &idle).await.unconfirmed);

    let first = unconfirmed(&world, &active).await;
    assert!(first.unconfirmed && !first.alerted);
    world.clock.advance(8 * HOUR_MS);
    let second = unconfirmed(&world, &active).await;
    assert!(second.unconfirmed && !second.alerted);
    // Confirmed connections in between do not clear the count.
    active = world.swap_again(&active).await;
    assert!(!close(&world, &active.connection).await.unconfirmed);
    world.clock.advance(8 * HOUR_MS);
    assert!(world.alerts_about(active.device).await.is_empty());

    let third = unconfirmed(&world, &active).await;
    assert!(third.unconfirmed && third.alerted);
    let alert: (String, serde_json::Value) = sqlx::query_as(
        "SELECT kind, details FROM alert WHERE organization_id = $1 AND device_id = $2",
    )
    .bind(world.org.id())
    .bind(active.device)
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(alert.0, "unconfirmed_swaps");
    assert_eq!(alert.1["count"], 3);
    // One alert per 24 hours, however many follow.
    let fourth = unconfirmed(&world, &active).await;
    assert!(fourth.unconfirmed && !fourth.alerted);
    assert_eq!(world.alerts_about(active.device).await.len(), 1);
    let recorded = world.actions_about(active.device).await;
    assert_eq!(
        recorded
            .iter()
            .filter(|a| *a == "credential.unconfirmed")
            .count(),
        4
    );

    // The rows say what happened to each connection.
    let rows: Vec<(bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT credential_issued_at IS NOT NULL, swap_confirmed_at IS NOT NULL, close_reason \
         FROM channel_connection WHERE organization_id = $1 AND device_id = $2 \
         ORDER BY opened_at, credential_issued_at NULLS FIRST",
    )
    .bind(world.org.id())
    .bind(active.device)
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert_eq!(
        rows.iter()
            .filter(|(issued, confirmed, _)| *issued && !*confirmed)
            .count(),
        4
    );
    assert!(
        rows.iter()
            .all(|(_, _, reason)| reason.as_deref() == Some("saved_timeout"))
    );
}

/// The endings of the connections of one device are counted one after the other. Several can
/// end at the same moment (the hub stopping, a double connection, a replaced attempt), and
/// each one has to see the ones before it: without that, the third swap left unconfirmed
/// raises no alert, or raises two.
#[tokio::test]
async fn the_ending_of_a_connection_waits_for_whoever_holds_the_hermes_of_its_device() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    world.issued(&connection, Some(&active.sealed)).await;

    // Another ending of the same device is in the middle of its count.
    let other_pool = world.db.second_pool().await;
    let mut holder = other_pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM hermes WHERE id = $1 FOR NO KEY UPDATE")
        .bind(hermes)
        .execute(&mut *holder)
        .await
        .unwrap();
    let waiting = {
        let pool = world.pool().clone();
        let org = world.org;
        let connection = connection.clone();
        let now = world.now();
        tokio::spawn(async move {
            connection::close(&pool, org, &connection, CloseReason::Dropped, now).await
        })
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !waiting.is_finished(),
        "the ending did not wait for the lock of the Hermes"
    );
    let still_open: Option<i64> =
        sqlx::query_scalar("SELECT closed_at FROM channel_connection WHERE id = $1")
            .bind(connection.id)
            .fetch_one(&other_pool)
            .await
            .unwrap();
    assert_eq!(still_open, None);

    holder.commit().await.unwrap();
    let closed = waiting.await.unwrap().unwrap();
    assert!(closed.unconfirmed && !closed.alerted);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connections_that_end_at_the_same_moment_raise_the_alert_once_and_never_miss_it() {
    let world = World::shared().await;
    // `earlier` endings without the "saved" on record, then two more at the same moment.
    for (earlier, round) in [(1, 0), (1, 1), (1, 2), (2, 0), (2, 1), (2, 2)] {
        let name = world.hermes().await;
        let active = world.active(&name).await;
        for _ in 0..earlier {
            assert!(unconfirmed(&world, &active).await.unconfirmed);
        }
        let mut ending = Vec::new();
        for _ in 0..2 {
            let connection = world.connect(active.device).await;
            world.issued(&connection, Some(&active.sealed)).await;
            ending.push(connection);
        }
        let mut tasks = Vec::new();
        for connection in ending {
            let pool = world.pool().clone();
            let org = world.org;
            let now = world.now();
            tasks.push(tokio::spawn(async move {
                connection::close(&pool, org, &connection, CloseReason::HubStopping, now).await
            }));
        }
        let mut alerted = 0;
        for task in tasks {
            let closed = task.await.unwrap().unwrap();
            assert!(closed.unconfirmed);
            alerted += u32::from(closed.alerted);
        }
        assert_eq!(
            (alerted, world.alerts_about(active.device).await),
            (1, vec!["unconfirmed_swaps".to_owned()]),
            "{earlier} earlier, round {round}: three or more in 24 hours raise one alert"
        );
    }
}

/// A "saved" whose event the mark could not follow: the promotion is committed, no session is
/// opened, and the device is not locked out.
#[tokio::test]
async fn a_saved_the_mark_could_not_follow_opens_no_session_and_locks_nobody_out() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let connection = world.connect(active.device).await;
    let issued = world.issued(&connection, Some(&active.sealed)).await;

    // Another process holds the lock of the mark for longer than this one waits.
    let impatient = Mark::with_lock_patience(&world.home.data_dir(), Duration::from_millis(50));
    let other_process =
        arauto_platform::FileLock::try_acquire(&world.home.data_dir().join(MARK_LOCK_FILE))
            .unwrap();
    let mark_before = world.home.mark.read().await.unwrap();
    let outcome = swap::confirm(
        world.pool(),
        &impatient,
        world.org,
        &connection,
        &issued.receipt,
        &World::saved(&issued),
        world.now(),
    )
    .await;
    drop(other_process);
    assert!(
        matches!(
            outcome,
            Err(RepoError::Audit(AuditError::MarkNotAdvanced { .. }))
        ),
        "{outcome:?}"
    );
    // The promotion is in the database; the mark is one event behind, which accuses nothing.
    assert_eq!(generations(&world, active.device).await, (2, None, 2));
    assert_eq!(world.home.mark.read().await.unwrap(), mark_before);

    // The connector saved the credential before saying so. It comes back with it, and it is
    // the current one: a new swap, and this time a session.
    let back = world.connect(active.device).await;
    let reissued = world.issued(&back, Some(&issued.sealed)).await;
    assert!(!reissued.promoted);
    assert!(matches!(
        world.confirm(&back, &reissued).await,
        ConfirmOutcome::Confirmed { .. }
    ));
    assert_eq!(generations(&world, active.device).await, (3, None, 3));
    assert_eq!(world.device(active.device).await.state, DeviceState::Active);
    assert!(matches!(
        world.home.mark.read().await.unwrap(),
        MarkValue::At(at) if Some(at) > mark_before.number()
    ));
}

#[tokio::test]
async fn two_in_24_hours_and_a_third_a_day_later_raise_nothing() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    unconfirmed(&world, &active).await;
    unconfirmed(&world, &active).await;
    world.clock.advance(DAY_MS);
    let third = unconfirmed(&world, &active).await;
    assert!(third.unconfirmed && !third.alerted);
    assert!(world.alerts_about(active.device).await.is_empty());
}

#[tokio::test]
async fn a_rotation_that_is_never_saved_counts_even_after_a_confirmed_swap() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    // The session confirmed its first swap; the 24-hour one, in the same millisecond, did not.
    let IssueOutcome::Issued(_) = rotate(&world, &active.connection).await else {
        panic!("the rotation did not issue");
    };
    let closed = close(&world, &active.connection).await;
    assert!(closed.unconfirmed);
}

#[tokio::test]
async fn the_rows_a_stopped_hub_left_open_are_closed_when_it_starts() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let active = world.active(&name).await;
    let waiting = world.connect(active.device).await;
    world.issued(&waiting, Some(&active.sealed)).await;
    world.clock.advance(60_000);

    let closed = connection::close_left_open(world.pool(), world.org, world.now())
        .await
        .unwrap();
    assert_eq!(closed, 2);
    let rows: Vec<(Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT closed_at, close_reason FROM channel_connection WHERE organization_id = $1",
    )
    .bind(world.org.id())
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert!(
        rows.iter()
            .all(|row| *row == (Some(world.now()), Some("hub_stopped".to_owned())))
    );
    assert_eq!(
        world
            .actions_about(active.device)
            .await
            .iter()
            .filter(|a| *a == "credential.unconfirmed")
            .count(),
        1
    );
    assert_eq!(
        connection::close_left_open(world.pool(), world.org, world.now())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn a_session_taken_by_another_connection_raises_the_alert_every_time() {
    let world = World::shared().await;
    let name = world.hermes().await;
    let hermes = world.hermes_id(&name).await;
    let active = world.active(&name).await;
    for _ in 0..2 {
        connection::taken(
            world.pool(),
            world.org,
            active.device,
            address(200),
            world.now(),
        )
        .await
        .unwrap();
    }
    let alerts: Vec<(String, Option<Uuid>, serde_json::Value)> = sqlx::query_as(
        "SELECT kind, hermes_id, details FROM alert WHERE organization_id = $1 AND device_id = $2",
    )
    .bind(world.org.id())
    .bind(active.device)
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert_eq!(alerts.len(), 2);
    assert!(alerts.iter().all(|alert| {
        alert.0 == "connection_taken" && alert.1 == Some(hermes) && alert.2["from"] == "192.0.2.200"
    }));
}
