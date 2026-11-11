//! The deadlines (`docs/10-maquinas-de-estado.md`, section 7), applied by a sweep the hub
//! calls for each organization:
//!
//! | What | Deadline | What happens |
//! |---|---|---|
//! | Issued invite | 15 minutes | Expired; its digest is erased. |
//! | Pending request | 10 minutes from the first enrollment | Expired. |
//! | Approved request | 24 hours without confirming the first credential | Expired. |
//! | Active device | 30 days without connecting | Expired; reconnecting is the way back. |
//!
//! A deadline has passed when the time of the hub **reached** it. The functions that meet a
//! request past its deadline before the sweep does (the enrollment, the approval) apply the
//! same rule on the spot, so the answer never depends on when the sweep last ran.

use arauto_protocol::limits::{DAY_MS, HOUR_MS};
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::{RepoError, lock_hermes};
use crate::audit::{self, Action, Actor, Details, Event, SecurityWrite, TargetKind};
use crate::core::model::{Device, DeviceState, StateReason};
use crate::db::ResolvedOrg;
use crate::mark::Mark;

/// How long an approved device has to confirm its first credential.
pub const APPROVED_WINDOW_MS: i64 = 24 * HOUR_MS;

/// How long an active device may stay without connecting.
pub const INACTIVITY_MS: i64 = 30 * DAY_MS;

/// Whether a deadline has passed: the time of the hub reached it.
pub(crate) const fn has_passed(deadline: Option<i64>, now: i64) -> bool {
    match deadline {
        Some(deadline) => now >= deadline,
        None => false,
    }
}

/// The deadline that passed for this device, if one did.
pub(crate) fn overdue(device: &Device, now: i64) -> Option<StateReason> {
    match device.state {
        DeviceState::Pending if has_passed(device.confirm_until, now) => {
            Some(StateReason::PendingTimeout)
        }
        DeviceState::Approved if has_passed(device.approved_until, now) => {
            Some(StateReason::ApprovedTimeout)
        }
        DeviceState::Active => {
            let last = device
                .last_contact_at
                .or(device.first_confirmed_at)
                .unwrap_or(device.state_since);
            has_passed(Some(last.saturating_add(INACTIVITY_MS)), now)
                .then_some(StateReason::Inactivity)
        }
        _ => None,
    }
}

/// Moves a device to expired, for `reason`. Called under the lock of its Hermes.
pub(crate) async fn expire_device(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    device: &Device,
    reason: StateReason,
    now: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE device \
         SET state = 'expired', state_reason = $3, state_since = $4, slot = NULL, \
             verification_code = NULL, confirm_until = NULL, approved_until = NULL \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(device.id)
    .bind(reason.as_str())
    .bind(now)
    .execute(write.connection())
    .await?;
    write
        .record(
            now,
            &Event::new(Action::DeviceExpired, Actor::System)
                .in_organization(organization)
                .about(TargetKind::Device, device.id)
                .with(
                    Details::new()
                        .word("reason", reason.as_str())
                        .word("was", device.state.as_str())
                        .id("hermes", device.hermes_id),
                ),
        )
        .await?;
    Ok(())
}

/// A device that a sweep expired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpiredDevice {
    /// The device. Its open connection, if any, must be told and closed.
    pub id: Uuid,
    /// Which deadline passed.
    pub reason: StateReason,
}

/// What a sweep did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Swept {
    /// Invites that expired unused.
    pub invites: Vec<Uuid>,
    /// Devices that expired.
    pub devices: Vec<ExpiredDevice>,
}

impl Swept {
    /// Whether nothing was due.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.invites.is_empty() && self.devices.is_empty()
    }
}

/// Applies every deadline of `organization` that has passed at `now`.
///
/// It first looks, without locks, for the Hermes that have something due; with nothing due it
/// writes nothing. Then, one Hermes at a time, it locks the Hermes and its devices, reads
/// again and expires what is still due.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn sweep(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    now: i64,
) -> Result<Swept, RepoError> {
    let due: Vec<Uuid> = sqlx::query_scalar(
        "SELECT hermes_id FROM invite \
         WHERE organization_id = $1 AND state = 'issued' AND expires_at <= $2 \
         UNION \
         SELECT hermes_id FROM device \
         WHERE organization_id = $1 AND slot IS NOT NULL \
           AND ((state = 'pending' AND confirm_until <= $2) \
             OR (state = 'approved' AND approved_until <= $2) \
             OR (state = 'active' \
                 AND COALESCE(last_contact_at, first_confirmed_at, state_since) <= $3)) \
         ORDER BY 1",
    )
    .bind(organization.id())
    .bind(now)
    .bind(now.saturating_sub(INACTIVITY_MS))
    .fetch_all(pool)
    .await?;

    let mut swept = Swept::default();
    for hermes in due {
        let mut write = audit::begin_security(pool, mark).await?;
        let Some(locked) = lock_hermes(write.connection(), organization, hermes).await? else {
            write.rollback().await?;
            continue;
        };
        let mut changed = false;
        let expired_invite: Option<Uuid> = sqlx::query_scalar(
            "UPDATE invite SET state = 'expired', digest = NULL \
             WHERE organization_id = $1 AND hermes_id = $2 AND state = 'issued' \
               AND expires_at <= $3 \
             RETURNING id",
        )
        .bind(organization.id())
        .bind(hermes)
        .bind(now)
        .fetch_optional(write.connection())
        .await?;
        if let Some(invite) = expired_invite {
            write
                .record(
                    now,
                    &Event::new(Action::InviteExpired, Actor::System)
                        .in_organization(organization)
                        .about(TargetKind::Invite, invite)
                        .with(Details::new().id("hermes", hermes)),
                )
                .await?;
            swept.invites.push(invite);
            changed = true;
        }
        let mut expired = Vec::new();
        for device in &locked.devices {
            if let Some(reason) = overdue(device, now) {
                expire_device(&mut write, organization, device, reason, now).await?;
                expired.push(ExpiredDevice {
                    id: device.id,
                    reason,
                });
                changed = true;
            }
        }
        if changed {
            write.commit().await?;
            swept.devices.append(&mut expired);
        } else {
            write.rollback().await?;
        }
    }
    Ok(swept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deadline_passes_when_the_time_reaches_it() {
        assert!(!has_passed(Some(1_000), 999));
        assert!(has_passed(Some(1_000), 1_000));
        assert!(has_passed(Some(1_000), 1_001));
        assert!(!has_passed(None, i64::MAX));
    }

    #[test]
    fn the_windows_are_the_documented_ones() {
        assert_eq!(APPROVED_WINDOW_MS, 86_400_000);
        assert_eq!(INACTIVITY_MS, 30 * 86_400_000);
    }
}
