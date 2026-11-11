//! The lines of Hermes, the invites and what belongs to each organization.

use arauto_hub::core::model::{DeviceState, InviteState, StateReason};
use arauto_hub::repo::RepoError;
use arauto_hub::repo::hermes::{self, CreateOutcome, RemoveOutcome, WakeOutcome};
use arauto_hub::repo::invite::{self, INVITE_VALIDITY_MS, IssueOutcome};
use arauto_hub::repo::swap::IssueOutcome as SwapOutcome;
use arauto_protocol::invite::Invite as InviteSecret;
use arauto_protocol::names::{HermesName, NameError, RESERVED_NAMES};
use uuid::Uuid;

use crate::common;
use crate::support::World;

fn name(text: &str) -> HermesName {
    HermesName::parse(text).unwrap()
}

async fn invite_row(world: &World, id: Uuid) -> (String, Option<Vec<u8>>, i64) {
    sqlx::query_as(
        "SELECT state, digest, expires_at FROM invite WHERE organization_id = $1 AND id = $2",
    )
    .bind(world.org.id())
    .bind(id)
    .fetch_one(world.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn a_hermes_is_created_listed_and_its_name_is_never_reused() {
    let world = World::shared().await;
    let vendas = name("vendas");
    let created = hermes::create(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &vendas,
        world.now(),
    )
    .await
    .unwrap();
    let CreateOutcome::Created(line) = created else {
        panic!("not created: {created:?}");
    };
    assert_eq!(line.name, vendas);
    assert!(!line.can_be_woken);
    assert_eq!((line.created_at, line.deleted_at), (world.now(), None));
    assert_eq!(world.actions_about(line.id).await, ["hermes.created"]);

    // The same name again, while the line exists.
    let again = hermes::create(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &vendas,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(again, CreateOutcome::NameTaken);

    let suporte = world.hermes().await;
    let listed = hermes::list(world.pool(), world.org, false).await.unwrap();
    let names: Vec<&str> = listed.iter().map(|l| l.hermes.name.as_str()).collect();
    assert_eq!(names, [suporte.as_str(), "vendas"]);
    assert!(
        listed
            .iter()
            .all(|l| l.in_use.is_none() && l.request.is_none())
    );

    // Removed: the row stays with the date, and the name stays taken for ever.
    world.clock.advance(1_000);
    let removed = hermes::remove(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &vendas,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(removed, RemoveOutcome::Removed(_)));
    let deleted_at: Option<i64> = sqlx::query_scalar("SELECT deleted_at FROM hermes WHERE id = $1")
        .bind(line.id)
        .fetch_one(world.pool())
        .await
        .unwrap();
    assert_eq!(deleted_at, Some(world.now()));
    assert_eq!(
        hermes::find(world.pool(), world.org, &vendas)
            .await
            .unwrap(),
        None
    );
    let reused = hermes::create(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &vendas,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(reused, CreateOutcome::NameRetired);
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM hermes WHERE organization_id = $1 AND name = 'vendas'",
    )
    .bind(world.org.id())
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(rows, 1);

    let listed = hermes::list(world.pool(), world.org, false).await.unwrap();
    assert_eq!(listed.len(), 1);
    let with_removed = hermes::list(world.pool(), world.org, true).await.unwrap();
    assert_eq!(with_removed.len(), 2);
    // Removing what is gone, or what never was.
    for gone in [&vendas, &name("nobody")] {
        let outcome = hermes::remove(
            world.pool(),
            &world.home.mark,
            world.org,
            &world.admin,
            gone,
            world.now(),
        )
        .await
        .unwrap();
        assert_eq!(outcome, RemoveOutcome::NotFound);
    }
}

#[test]
fn a_name_outside_the_format_or_reserved_is_not_a_name() {
    // The repository only takes a `HermesName`: the format and the reserved names are
    // refused before any function of it can be called.
    for (text, why) in [
        ("", NameError::Length),
        ("Vendas", NameError::FirstCharacter),
        ("-vendas", NameError::FirstCharacter),
        ("vendas 2", NameError::Character),
        ("vend\u{e1}s", NameError::Character),
        ("a123456789012345678901234567890123", NameError::Length),
    ] {
        assert_eq!(HermesName::parse(text), Err(why), "{text:?}");
    }
    for reserved in RESERVED_NAMES {
        assert_eq!(HermesName::parse(reserved), Err(NameError::Reserved));
    }
}

#[tokio::test]
async fn removing_a_hermes_takes_its_links_and_closes_what_is_connected() {
    let world = World::shared().await;
    let leaving = world.hermes().await;
    let staying = world.hermes().await;
    let other = world.hermes().await;
    let (leaving_id, staying_id, other_id) = (
        world.hermes_id(&leaving).await,
        world.hermes_id(&staying).await,
        world.hermes_id(&other).await,
    );
    for (from, to) in [
        (leaving_id, staying_id),
        (staying_id, leaving_id),
        (staying_id, other_id),
    ] {
        sqlx::query(
            "INSERT INTO peer_link (organization_id, from_hermes_id, to_hermes_id, created_at, \
                                    changed_at) VALUES ($1, $2, $3, $4, $4)",
        )
        .bind(world.org.id())
        .bind(from)
        .bind(to)
        .bind(world.now())
        .execute(world.pool())
        .await
        .unwrap();
    }
    let active = world.active(&leaving).await;
    let issued = world.invite(&leaving).await;
    hermes::set_wake(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &leaving,
        true,
        world.now(),
    )
    .await
    .unwrap();

    let removed = hermes::remove(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &leaving,
        world.now(),
    )
    .await
    .unwrap();
    let RemoveOutcome::Removed(removed) = removed else {
        panic!("not removed");
    };
    assert_eq!(removed.links_removed, 2);
    assert_eq!(removed.closed.invite, Some(issued.id));
    assert_eq!(removed.closed.devices.len(), 1);

    let links: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT from_hermes_id, to_hermes_id FROM peer_link WHERE organization_id = $1",
    )
    .bind(world.org.id())
    .fetch_all(world.pool())
    .await
    .unwrap();
    assert_eq!(links, [(staying_id, other_id)]);
    let device = world.device(active.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (DeviceState::Revoked, Some(StateReason::Admin), None)
    );
    assert_eq!(invite_row(&world, issued.id).await.0, "cancelled");
    let can_be_woken: bool = sqlx::query_scalar("SELECT can_be_woken FROM hermes WHERE id = $1")
        .bind(leaving_id)
        .fetch_one(world.pool())
        .await
        .unwrap();
    assert!(!can_be_woken);
    let actions = world.actions_about(leaving_id).await;
    assert!(
        actions.contains(&"hermes.removed".to_owned()),
        "{actions:?}"
    );
    assert!(
        actions.contains(&"peer_link.removed".to_owned()),
        "{actions:?}"
    );

    // A removed Hermes gets no invite and its device no credential.
    let refused = invite::issue(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &leaving,
        world.now(),
    )
    .await
    .unwrap();
    assert!(matches!(refused, IssueOutcome::NotFound));
    let connection = world.connect(active.device).await;
    let outcome = world.issue(&connection, Some(&active.sealed)).await;
    assert!(matches!(outcome, SwapOutcome::NotSwapped(_)), "{outcome:?}");
}

#[tokio::test]
async fn can_be_woken_is_marked_and_unmarked() {
    let world = World::shared().await;
    let hermes_name = world.hermes().await;
    let id = world.hermes_id(&hermes_name).await;
    let set = |on: bool| {
        hermes::set_wake(
            world.pool(),
            &world.home.mark,
            world.org,
            &world.admin,
            &hermes_name,
            on,
            world.now(),
        )
    };
    assert_eq!(set(false).await.unwrap(), WakeOutcome::Unchanged);
    assert_eq!(set(true).await.unwrap(), WakeOutcome::Changed);
    assert_eq!(set(true).await.unwrap(), WakeOutcome::Unchanged);
    assert!(
        hermes::find(world.pool(), world.org, &hermes_name)
            .await
            .unwrap()
            .unwrap()
            .can_be_woken
    );
    assert_eq!(set(false).await.unwrap(), WakeOutcome::Changed);
    assert_eq!(
        world.actions_about(id).await,
        [
            "hermes.created",
            "hermes.wake_changed",
            "hermes.wake_changed"
        ]
    );
    let missing = hermes::set_wake(
        world.pool(),
        &world.home.mark,
        world.org,
        &world.admin,
        &name("nobody"),
        true,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(missing, WakeOutcome::NotFound);
}

#[tokio::test]
async fn an_invite_is_drawn_stored_as_a_digest_and_valid_for_fifteen_minutes() {
    let world = World::shared().await;
    let hermes_name = world.hermes().await;
    let issued = world.invite(&hermes_name).await;
    assert_eq!(issued.expires_at, world.now() + 15 * 60 * 1_000);
    assert_eq!(INVITE_VALIDITY_MS, 900_000);
    assert!(!issued.reconnect);
    assert_eq!((issued.cancelled, issued.closed_request), (None, None));

    // What is shown is a well-formed invite, and what is stored is only its SHA-256.
    let text = issued.invite.reveal();
    assert_eq!(InviteSecret::parse(&text).unwrap(), issued.invite);
    let (state, digest, expires_at) = invite_row(&world, issued.id).await;
    assert_eq!(state, InviteState::Issued.as_str());
    assert_eq!(
        digest.as_deref(),
        Some(issued.invite.digest().as_bytes().as_slice())
    );
    assert_eq!(expires_at, issued.expires_at);
    let everything: String = sqlx::query_scalar(
        "SELECT (SELECT string_agg(i::text, ' ') FROM invite i WHERE i.organization_id = $1) \
             || (SELECT string_agg(a::text, ' ') FROM audit a WHERE a.organization_id = $1)",
    )
    .bind(world.org.id())
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert!(!everything.contains(text.as_str()));
    assert!(!everything.contains(&text.as_str()[4..]));

    // Two invites are never the same.
    let other = world.hermes().await;
    let second = world.invite(&other).await;
    assert_ne!(second.invite, issued.invite);
    assert_eq!(world.actions_about(issued.id).await, ["invite.issued"]);
}

#[tokio::test]
async fn a_new_invite_cancels_the_issued_one_and_closes_the_request_in_progress() {
    let world = World::shared().await;

    // An issued invite nobody used.
    let unused = world.hermes().await;
    let first = world.invite(&unused).await;
    let second = world.invite(&unused).await;
    assert_eq!(second.cancelled, Some(first.id));
    // Nothing of a cancelled invite is kept: its digest goes with it.
    let cancelled = invite_row(&world, first.id).await;
    assert_eq!((cancelled.0.as_str(), cancelled.1), ("cancelled", None));
    assert_eq!(invite_row(&world, second.id).await.0, "issued");
    assert_eq!(
        world.actions_about(first.id).await,
        ["invite.issued", "invite.cancelled"]
    );

    // A pending request becomes denied.
    let pending_name = world.hermes().await;
    let pending = world.pending(&pending_name).await;
    let issued = world.invite(&pending_name).await;
    let closed = issued.closed_request.unwrap();
    assert_eq!(
        (closed.id, closed.was, closed.now),
        (pending.device, DeviceState::Pending, DeviceState::Denied)
    );
    let device = world.device(pending.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (DeviceState::Denied, Some(StateReason::NewInvite), None)
    );
    assert_eq!(
        (device.verification_code, device.confirm_until),
        (None, None)
    );

    // An approved request that never confirmed becomes revoked.
    let approved_name = world.hermes().await;
    let approved = world.approved(&approved_name, None).await;
    let issued = world.invite(&approved_name).await;
    assert_eq!(issued.closed_request.unwrap().now, DeviceState::Revoked);
    let device = world.device(approved.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (DeviceState::Revoked, Some(StateReason::NewInvite), None)
    );

    // So does the request that went to quarantine before confirming its first credential.
    let quarantined_name = world.hermes().await;
    let quarantined = world.approved(&quarantined_name, None).await;
    let connection = world.connect(quarantined.device).await;
    let garbage = arauto_protocol::secret::SealedCredential::from_slice(&[7u8; 120]).unwrap();
    let outcome = world.issue(&connection, Some(&garbage)).await;
    assert!(
        matches!(outcome, SwapOutcome::Quarantined(_)),
        "{outcome:?}"
    );
    let before = world.device(quarantined.device).await;
    assert_eq!(
        (before.state, before.slot.map(|s| s.as_str())),
        (DeviceState::Quarantined, Some("request"))
    );
    let issued = world.invite(&quarantined_name).await;
    assert_eq!(issued.closed_request.unwrap().was, DeviceState::Quarantined);
    let device = world.device(quarantined.device).await;
    assert_eq!(
        (device.state, device.state_reason, device.slot),
        (DeviceState::Revoked, Some(StateReason::NewInvite), None)
    );
}

#[tokio::test]
async fn a_new_invite_does_not_touch_the_device_in_use() {
    let world = World::shared().await;
    let hermes_name = world.hermes().await;
    let active = world.active(&hermes_name).await;
    let before = world.device(active.device).await;

    let issued = world.invite(&hermes_name).await;
    assert!(issued.reconnect);
    assert_eq!(issued.closed_request, None);
    assert_eq!(world.device(active.device).await, before);

    // And it still swaps.
    let again = world.swap_again(&active).await;
    assert_eq!(again.credential.generation.get(), 2);
}

#[tokio::test]
async fn what_is_of_one_organization_is_not_reached_from_another() {
    let world = World::shared().await;
    let other = common::organization(world.pool(), &world.home).await;
    let hermes_name = world.hermes().await;
    let active = world.active(&hermes_name).await;

    // The same name is free in the other organization, and names nothing there.
    assert_eq!(
        hermes::find(world.pool(), other, &hermes_name)
            .await
            .unwrap(),
        None
    );
    let removed = hermes::remove(
        world.pool(),
        &world.home.mark,
        other,
        &world.admin,
        &hermes_name,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(removed, RemoveOutcome::NotFound);
    let revoked = arauto_hub::repo::closing::revoke(
        world.pool(),
        &world.home.mark,
        other,
        &world.admin,
        &hermes_name,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(revoked, arauto_hub::repo::closing::RevokeOutcome::NotFound);
    assert!(
        hermes::list(world.pool(), other, true)
            .await
            .unwrap()
            .is_empty()
    );

    // The device is not found by the other organization, by identity or by row.
    assert_eq!(
        arauto_hub::repo::device::by_identity(world.pool(), other, &active.identity)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        arauto_hub::repo::device::by_id(world.pool(), other, active.device)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        arauto_hub::repo::device::standing(world.pool(), other, active.device)
            .await
            .unwrap(),
        None
    );
    let left = arauto_hub::repo::closing::leave(
        world.pool(),
        &world.home.mark,
        other,
        active.device,
        None,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(left.status, arauto_protocol::message::LeftStatus::Unknown);
    // A swap asked with the wrong organization is refused before anything is read.
    let other_key =
        arauto_hub::repo::device::organization_key(world.pool(), other, &world.home.vault)
            .await
            .unwrap();
    let refused = arauto_hub::repo::swap::issue(
        world.pool(),
        &world.home.mark,
        other,
        &world.key,
        &active.connection,
        Some(active.sealed.expose()),
        world.now(),
    )
    .await;
    assert!(
        matches!(refused, Err(RepoError::Inconsistent(_))),
        "{refused:?}"
    );
    let refused = arauto_hub::repo::swap::issue(
        world.pool(),
        &world.home.mark,
        other,
        &other_key,
        &active.connection,
        Some(active.sealed.expose()),
        world.now(),
    )
    .await;
    assert!(
        matches!(refused, Err(RepoError::Inconsistent(_))),
        "{refused:?}"
    );

    // The record of connections is of each organization too: nothing is opened, closed or
    // alerted about a device from the organization next door.
    let connection_rows = || async {
        sqlx::query_as::<_, (Uuid, Option<i64>)>(
            "SELECT id, closed_at FROM channel_connection WHERE device_id = $1 ORDER BY id",
        )
        .bind(active.device)
        .fetch_all(world.pool())
        .await
        .unwrap()
    };
    let rows_before = connection_rows().await;
    assert_eq!(rows_before, [(active.connection.id, None)]);
    let version = arauto_protocol::message::DeclaredText::parse("0.1.0").unwrap();
    let opened = arauto_hub::repo::connection::open(
        world.pool(),
        other,
        active.device,
        None,
        &version,
        1,
        world.now(),
    )
    .await;
    assert!(
        matches!(opened, Err(RepoError::Inconsistent(_))),
        "{opened:?}"
    );
    let closed = arauto_hub::repo::connection::close(
        world.pool(),
        other,
        &active.connection,
        arauto_hub::repo::connection::CloseReason::Dropped,
        world.now(),
    )
    .await
    .unwrap();
    assert_eq!(
        closed,
        arauto_hub::repo::connection::ClosedConnection::default()
    );
    assert_eq!(
        arauto_hub::repo::connection::close_left_open(world.pool(), other, world.now())
            .await
            .unwrap(),
        0
    );
    arauto_hub::repo::connection::taken(world.pool(), other, active.device, None, world.now())
        .await
        .unwrap();
    assert_eq!(connection_rows().await, rows_before);
    let other_rows: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM channel_connection WHERE organization_id = $1) \
              + (SELECT count(*) FROM alert WHERE organization_id = $1) \
              + (SELECT count(*) FROM audit WHERE organization_id = $1)",
    )
    .bind(other.id())
    .fetch_one(world.pool())
    .await
    .unwrap();
    assert_eq!(other_rows, 0);
    assert!(world.alerts_about(active.device).await.is_empty());
    // And the database itself refuses a row of one organization about a device of another.
    for statement in [
        "INSERT INTO channel_connection (id, organization_id, device_id, opened_at) \
         VALUES ($1, $2, $3, 1)",
        "INSERT INTO invite (id, organization_id, hermes_id, state, created_at, expires_at, \
                             used_at, device_id) \
         SELECT $1, $2, h.id, 'used', 1, 2, 1, $3 FROM hermes h WHERE h.organization_id = $2 \
         LIMIT 1",
    ] {
        common::hermes(world.pool(), other).await;
        let refused = sqlx::query(statement)
            .bind(Uuid::new_v4())
            .bind(other.id())
            .bind(active.device)
            .execute(world.pool())
            .await;
        let code = refused
            .expect_err("the database took a row that crosses organizations")
            .as_database_error()
            .and_then(|error| error.code().map(|code| code.into_owned()));
        assert_eq!(code.as_deref(), Some("23503"), "foreign key violation");
    }

    // Nothing of it changed.
    let device = world.device(active.device).await;
    assert_eq!(device.state, DeviceState::Active);
    assert_eq!(device.current, Some(active.credential));
    assert_eq!(device.pending, None);
    assert!(
        hermes::find(world.pool(), world.org, &hermes_name)
            .await
            .unwrap()
            .is_some()
    );
}
