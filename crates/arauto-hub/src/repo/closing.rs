//! Closing: revoking a Hermes, and a device asking for its own closing (C10).
//!
//! **Revoke** closes everything of a Hermes: the device in use, the request in progress and
//! the issued invite. The rows stay as a record.
//!
//! **Leave** is the only thing a device does that is close to admin work: it asks for its own
//! closing. Whoever holds the identity of a device can close it, so every closing asked by a
//! device raises an alert with the address it came from and the last use of the device.

use std::net::IpAddr;

use arauto_protocol::message::LeftStatus;
use arauto_protocol::names::HermesName;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::alert::{self, AlertDetails, AlertKind, NewAlert};
use super::invite::close_join_page;
use super::{Admin, Locked, RepoError, lock_hermes_by_name, lock_hermes_of_device, notify_hub};
use crate::audit::{self, Action, Actor, Details, Event, SecurityWrite, TargetKind};
use crate::core::model::{Device, DeviceState, StateReason};
use crate::db::ResolvedOrg;
use crate::mark::Mark;

/// A device that a write closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedDevice {
    /// The device.
    pub id: Uuid,
    /// The state it was in.
    pub was: DeviceState,
    /// The final state it went to: denied for a pending request, revoked otherwise.
    pub now: DeviceState,
}

/// Closes one device that is not in a final state: a pending request becomes denied, any
/// other becomes revoked (`docs/10-maquinas-de-estado.md`, section 2). The code and the
/// deadlines of the request are erased; the generations and digests stay, for the reuse
/// that may show up later.
pub(crate) async fn close_device(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    device: &Device,
    reason: StateReason,
    actor: &Actor,
    address: Option<IpAddr>,
    now: i64,
) -> Result<ClosedDevice, RepoError> {
    let (state, action) = if device.state == DeviceState::Pending {
        (DeviceState::Denied, Action::DeviceDenied)
    } else {
        (DeviceState::Revoked, Action::DeviceRevoked)
    };
    sqlx::query(
        "UPDATE device \
         SET state = $3, state_reason = $4, state_since = $5, slot = NULL, \
             verification_code = NULL, confirm_until = NULL, approved_until = NULL \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(device.id)
    .bind(state.as_str())
    .bind(reason.as_str())
    .bind(now)
    .execute(write.connection())
    .await?;
    close_join_page(write.connection(), organization, device.id).await?;
    let mut event = Event::new(action, actor.clone())
        .in_organization(organization)
        .about(TargetKind::Device, device.id)
        .with(
            Details::new()
                .word("reason", reason.as_str())
                .word("was", device.state.as_str())
                .id("hermes", device.hermes_id),
        );
    if let Some(address) = address {
        event = event.from_address(address);
    }
    write.record(now, &event).await?;
    Ok(ClosedDevice {
        id: device.id,
        was: device.state,
        now: state,
    })
}

/// Cancels the issued invite of a Hermes, if there is one, and erases its digest: nothing of
/// an invite that no longer holds is kept (C1). Called under the lock of the Hermes.
pub(crate) async fn cancel_issued_invite(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    hermes: Uuid,
    actor: &Actor,
    reason: &'static str,
    now: i64,
) -> Result<Option<Uuid>, RepoError> {
    let cancelled: Option<Uuid> = sqlx::query_scalar(
        "UPDATE invite SET state = 'cancelled', digest = NULL, join_digest = NULL \
         WHERE organization_id = $1 AND hermes_id = $2 AND state = 'issued' \
         RETURNING id",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_optional(write.connection())
    .await?;
    if let Some(invite) = cancelled {
        write
            .record(
                now,
                &Event::new(Action::InviteCancelled, actor.clone())
                    .in_organization(organization)
                    .about(TargetKind::Invite, invite)
                    .with(Details::new().word("reason", reason).id("hermes", hermes)),
            )
            .await?;
    }
    Ok(cancelled)
}

/// What closing everything of a Hermes did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Closed {
    /// The devices that were closed: the one in use, the request in progress, or both.
    pub devices: Vec<ClosedDevice>,
    /// The issued invite that was cancelled.
    pub invite: Option<Uuid>,
}

/// Closes the device in use, the request in progress and the issued invite of a locked
/// Hermes, and tells the hub about each device.
pub(crate) async fn close_everything(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    locked: &Locked,
    admin: &Admin,
    now: i64,
) -> Result<Closed, RepoError> {
    let actor = admin.actor();
    let mut closed = Closed::default();
    for device in locked.devices.iter().filter(|d| !d.state.is_final()) {
        let done = close_device(
            write,
            organization,
            device,
            StateReason::Admin,
            &actor,
            None,
            now,
        )
        .await?;
        let kind = if done.now == DeviceState::Denied {
            "denied"
        } else {
            "revoked"
        };
        notify_hub(write.connection(), organization, kind, device.id).await?;
        closed.devices.push(done);
    }
    closed.invite = cancel_issued_invite(
        write,
        organization,
        locked.hermes.id,
        &actor,
        "revoked",
        now,
    )
    .await?;
    Ok(closed)
}

/// The outcome of [`revoke`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevokeOutcome {
    /// Everything of the Hermes was closed. Empty when there was nothing to close.
    Revoked(Closed),
    /// There is no Hermes with that name.
    NotFound,
}

/// Revokes a Hermes: closes its device, its request in progress and its issued invite, in
/// one write. The line of the Hermes stays, and can be connected again with a new invite.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn revoke(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    now: i64,
) -> Result<RevokeOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let Some(locked) = lock_hermes_by_name(write.connection(), organization, name).await? else {
        write.rollback().await?;
        return Ok(RevokeOutcome::NotFound);
    };
    let closed = close_everything(&mut write, organization, &locked, admin, now).await?;
    write.commit().await?;
    Ok(RevokeOutcome::Revoked(closed))
}

/// What the hub did with a request to leave, and the device it was about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Left {
    /// The answer for the connector. Any of them lets it delete the folder of that identity.
    pub status: LeftStatus,
    /// The state the device ended in, when it was closed now.
    pub closed: Option<ClosedDevice>,
}

/// A device asks for its own closing (`leave`). A pending request becomes denied; an
/// approved, active, suspended or quarantined device becomes revoked; a device already in a
/// final state answers "already closed" and nothing is written.
///
/// Every closing raises the alert, with the address the request came from and the last use
/// of the device: whoever used a copy could otherwise close the device to erase the trail.
///
/// The caller proved the identity of `device` in the handshake. An identity the database does
/// not know has no organization: the hub answers `unknown` without calling this.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn leave(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    device: Uuid,
    address: Option<IpAddr>,
    now: i64,
) -> Result<Left, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let locked = lock_hermes_of_device(write.connection(), organization, device).await?;
    let Some(found) = locked.as_ref().and_then(|locked| locked.device(device)) else {
        write.rollback().await?;
        return Ok(Left {
            status: LeftStatus::Unknown,
            closed: None,
        });
    };
    if found.state.is_final() {
        write.rollback().await?;
        return Ok(Left {
            status: LeftStatus::AlreadyClosed,
            closed: None,
        });
    }
    let closed = close_device(
        &mut write,
        organization,
        found,
        StateReason::Left,
        &Actor::Device { id: device },
        address,
        now,
    )
    .await?;
    alert::raise(
        write.connection(),
        organization,
        now,
        &NewAlert {
            kind: AlertKind::DeviceLeft,
            hermes: Some(found.hermes_id),
            device: Some(device),
            details: AlertDetails::new()
                .word("was", found.state.as_str())
                .address("from", address)
                .maybe_number("last_use_at", found.last_contact_at)
                .address(
                    "last_use_from",
                    super::stored_address(found.last_address.as_deref()),
                ),
        },
    )
    .await?;
    write.commit().await?;
    Ok(Left {
        status: LeftStatus::Closed,
        closed: Some(closed),
    })
}
