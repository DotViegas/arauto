//! Approving and denying a request (`docs/04-seguranca-conexao.md`, C4, C5 and C10).
//!
//! The admin **types** the code the Hermes answered. The hub never shows it. A request is
//! approved only when what was typed is the code of the pending request of that Hermes, which
//! is a single one. Five wrong codes close the request, and so do 10 minutes from the first
//! enrollment. The code is erased when the request is resolved, either way.
//!
//! On a **reconnect** the Hermes already has a device. Before approving, the admin is shown
//! the last use of that device and says whether it is recognized. If it is not, a copy was
//! used: the alert goes out, the old device goes to quarantine right there if it was active,
//! and the approval goes on.
//!
//! The answer is about **what was shown**: it comes with the last use the admin had on the
//! screen ([`Recognized`]), and under the lock that is compared with the device as it is
//! now. A use that happened in between is not covered by an answer given before it: nothing
//! is approved, and the admin is shown the new one.

use arauto_protocol::code::VerificationCode;
use arauto_protocol::names::HermesName;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::alert::{self, AlertDetails, AlertKind, NewAlert};
use super::closing::close_device;
use super::sweep::{APPROVED_WINDOW_MS, expire_device, has_passed};
use super::{Admin, Locked, RepoError, lock_hermes_by_name, notify_hub, stored_address};
use crate::audit::{self, Action, Actor, Details, Event, Outcome, TargetKind};
use crate::core::model::{Device, DeviceState, StateReason};
use crate::db::ResolvedOrg;
use crate::mark::Mark;

/// How many wrong codes close a request.
pub const CODE_ATTEMPTS_MAX: i32 = 5;

/// The last use of the device a new one is about to replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastUse {
    /// The device.
    pub device: Uuid,
    /// The state it is in.
    pub state: DeviceState,
    /// The last time a connection of it was accepted.
    pub at: Option<i64>,
    /// The address of that connection, as stored. Show it escaped.
    pub address: Option<String>,
}

impl LastUse {
    fn of(device: &Device) -> Self {
        Self {
            device: device.id,
            state: device.state,
            at: device.last_contact_at,
            address: device.last_address.clone(),
        }
    }
}

/// The device a request would replace: the one in use or, when there is none, the last one
/// that expired for inactivity, whose last use is shown the same way (C10, step 3).
fn previous(locked: &Locked) -> Option<&Device> {
    locked.in_use().or_else(|| {
        locked
            .devices
            .iter()
            .filter(|device| {
                device.state == DeviceState::Expired
                    && device.state_reason == Some(StateReason::Inactivity)
            })
            .max_by_key(|device| device.state_since)
    })
}

/// What the admin said about the last use of the device being replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recognition {
    /// "That was us."
    Recognized,
    /// "That was not us": someone used a copy.
    NotRecognized,
}

/// The request in progress of a Hermes, as the admin sees it before deciding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestView {
    /// The line of the Hermes.
    pub hermes: Uuid,
    /// The request. What it declares is not verified: show it escaped.
    pub request: Device,
    /// The device it would replace, on a reconnect.
    pub replaces: Option<LastUse>,
}

/// The request in progress of the Hermes with this name, if it has one, and the last use of
/// the device it would replace. Changes nothing.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn request_of(
    pool: &PgPool,
    organization: ResolvedOrg,
    name: &HermesName,
) -> Result<Option<RequestView>, RepoError> {
    let Some(hermes) = super::hermes::find(pool, organization, name).await? else {
        return Ok(None);
    };
    let devices: Vec<Device> =
        sqlx::query_as("SELECT * FROM device WHERE organization_id = $1 AND hermes_id = $2")
            .bind(organization.id())
            .bind(hermes.id)
            .fetch_all(pool)
            .await?;
    let locked = Locked { hermes, devices };
    Ok(locked.request().map(|request| RequestView {
        hermes: locked.hermes.id,
        request: request.clone(),
        replaces: previous(&locked).map(LastUse::of),
    }))
}

/// A request that was approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approved {
    /// The device, now approved and at generation zero.
    pub device: Uuid,
    /// Until when it may confirm its first credential.
    pub approved_until: i64,
    /// The device it will replace when it confirms, on a reconnect.
    pub replaces: Option<LastUse>,
    /// The old device that went to quarantine now, because its last use was not recognized.
    pub quarantined: Option<Uuid>,
}

/// The outcome of [`approve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApproveOutcome {
    /// The code was right.
    Approved(Approved),
    /// The code was wrong. The request is still open.
    WrongCode {
        /// How many wrong codes were typed so far, out of [`CODE_ATTEMPTS_MAX`].
        attempts: i32,
    },
    /// The fifth wrong code: the request was denied.
    Denied {
        /// The device of the request, now denied.
        device: Uuid,
    },
    /// The 10 minutes passed: the request expired.
    Expired {
        /// The device of the request, now expired.
        device: Uuid,
    },
    /// This is a reconnect, and the admin has not said whether the last use of the current
    /// device is recognized, or said it about a last use that is no longer the one of the
    /// device: it was used, or changed state, after the admin was shown it. Nothing was
    /// written and no attempt was counted. This is the last use as it is now.
    NeedsRecognition(LastUse),
    /// The Hermes has no pending request.
    NoRequest,
    /// There is no Hermes with that name.
    NotFound,
}

/// The answer about the last use of the device being replaced, with the last use the admin
/// was shown when giving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recognized {
    /// What the admin said.
    pub answer: Recognition,
    /// The last use that was on the screen: the one of [`RequestView::replaces`].
    pub shown: LastUse,
}

/// What the admin answered when approving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The code that was typed.
    pub code: VerificationCode,
    /// On a reconnect, whether the last use of the current device is recognized.
    pub recognition: Option<Recognized>,
}

/// Approves the pending request of a Hermes, with the code the admin typed.
///
/// In order: the deadline; on a reconnect, the answer about the last use, which must be about
/// the last use the device has now; the code. The comparison of the code takes the same time
/// whatever the digits are.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn approve(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    answer: Answer,
    now: i64,
) -> Result<ApproveOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let Some(locked) = lock_hermes_by_name(write.connection(), organization, name).await? else {
        write.rollback().await?;
        return Ok(ApproveOutcome::NotFound);
    };
    let Some(request) = locked
        .request()
        .filter(|request| request.state == DeviceState::Pending)
    else {
        write.rollback().await?;
        return Ok(ApproveOutcome::NoRequest);
    };
    if has_passed(request.confirm_until, now) {
        expire_device(
            &mut write,
            organization,
            request,
            StateReason::PendingTimeout,
            now,
        )
        .await?;
        notify_hub(write.connection(), organization, "expired", request.id).await?;
        write.commit().await?;
        return Ok(ApproveOutcome::Expired { device: request.id });
    }
    let replaces = match (previous(&locked), answer.recognition) {
        (Some(old), Some(recognized)) if recognized.shown == LastUse::of(old) => {
            Some((old, recognized.answer))
        }
        // No answer yet, or an answer about a last use that is not the one of now.
        (Some(old), _) => {
            let last_use = LastUse::of(old);
            write.rollback().await?;
            return Ok(ApproveOutcome::NeedsRecognition(last_use));
        }
        (None, _) => None,
    };

    let actor = admin.actor();
    // `VerificationCode` compares in constant time.
    if request.verification_code != Some(answer.code) {
        let attempts = request.code_attempts.saturating_add(1);
        sqlx::query("UPDATE device SET code_attempts = $3 WHERE organization_id = $1 AND id = $2")
            .bind(organization.id())
            .bind(request.id)
            .bind(attempts)
            .execute(write.connection())
            .await?;
        write
            .record(
                now,
                &Event::new(Action::DeviceCodeRejected, actor.clone())
                    .in_organization(organization)
                    .about(TargetKind::Device, request.id)
                    .ended(Outcome::Denied, None)
                    .with(Details::new().number("attempt", i64::from(attempts))),
            )
            .await?;
        if attempts < CODE_ATTEMPTS_MAX {
            write.commit().await?;
            return Ok(ApproveOutcome::WrongCode { attempts });
        }
        close_device(
            &mut write,
            organization,
            request,
            StateReason::CodeAttempts,
            &actor,
            None,
            now,
        )
        .await?;
        notify_hub(write.connection(), organization, "denied", request.id).await?;
        write.commit().await?;
        return Ok(ApproveOutcome::Denied { device: request.id });
    }

    let mut quarantined = None;
    if let Some((old, Recognition::NotRecognized)) = replaces {
        alert::raise(
            write.connection(),
            organization,
            now,
            &NewAlert {
                kind: AlertKind::CopySuspected,
                hermes: Some(locked.hermes.id),
                device: Some(old.id),
                details: AlertDetails::new()
                    .word("was", old.state.as_str())
                    .id("request", request.id)
                    .maybe_number("last_use_at", old.last_contact_at)
                    .address("last_use_from", stored_address(old.last_address.as_deref())),
            },
        )
        .await?;
        // A suspended, quarantined or expired device stays as it is.
        if old.state == DeviceState::Active {
            sqlx::query(
                "UPDATE device SET state = 'quarantined', state_reason = $3, state_since = $4 \
                 WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(old.id)
            .bind(StateReason::NotRecognized.as_str())
            .bind(now)
            .execute(write.connection())
            .await?;
            write
                .record(
                    now,
                    &Event::new(Action::DeviceQuarantined, actor.clone())
                        .in_organization(organization)
                        .about(TargetKind::Device, old.id)
                        .with(
                            Details::new()
                                .word("reason", StateReason::NotRecognized.as_str())
                                .id("hermes", locked.hermes.id),
                        ),
                )
                .await?;
            notify_hub(write.connection(), organization, "quarantined", old.id).await?;
            quarantined = Some(old.id);
        }
    }

    let approved_until = now.saturating_add(APPROVED_WINDOW_MS);
    sqlx::query(
        "UPDATE device \
         SET state = 'approved', state_reason = NULL, state_since = $3, \
             verification_code = NULL, confirm_until = NULL, approved_until = $4, \
             approved_by = $5, approved_at = $3 \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(request.id)
    .bind(now)
    .bind(approved_until)
    .bind(admin.id)
    .execute(write.connection())
    .await?;
    let mut details = Details::new()
        .id("hermes", locked.hermes.id)
        .flag("reconnect", replaces.is_some());
    if let Some((old, recognition)) = replaces {
        details = details
            .id("replaces", old.id)
            .flag("recognized", recognition == Recognition::Recognized);
    }
    write
        .record(
            now,
            &Event::new(Action::DeviceApproved, actor)
                .in_organization(organization)
                .about(TargetKind::Device, request.id)
                .with(details),
        )
        .await?;
    notify_hub(write.connection(), organization, "approved", request.id).await?;
    write.commit().await?;
    Ok(ApproveOutcome::Approved(Approved {
        device: request.id,
        approved_until,
        replaces: replaces.map(|(old, _)| LastUse::of(old)),
        quarantined,
    }))
}

/// The outcome of [`deny`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyOutcome {
    /// The pending request was denied.
    Denied {
        /// The device of the request.
        device: Uuid,
    },
    /// The Hermes has no pending request. An approved request is closed by revoking the
    /// Hermes or by issuing a new invite.
    NoRequest,
    /// There is no Hermes with that name.
    NotFound,
}

/// Denies the pending request of a Hermes. The code is erased with it.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn deny(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    now: i64,
) -> Result<DenyOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let Some(locked) = lock_hermes_by_name(write.connection(), organization, name).await? else {
        write.rollback().await?;
        return Ok(DenyOutcome::NotFound);
    };
    let Some(request) = locked
        .request()
        .filter(|request| request.state == DeviceState::Pending)
    else {
        write.rollback().await?;
        return Ok(DenyOutcome::NoRequest);
    };
    close_device(
        &mut write,
        organization,
        request,
        StateReason::Admin,
        &admin.actor(),
        None,
        now,
    )
    .await?;
    notify_hub(write.connection(), organization, "denied", request.id).await?;
    write.commit().await?;
    Ok(DenyOutcome::Denied { device: request.id })
}

/// What [`deny_double_waiting`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleWaitingOutcome {
    /// The request was denied and the alert raised.
    Denied,
    /// The device is no longer a pending request: nothing was written.
    NotPending,
}

/// Two waiting connections with the same identity, and the first one answered: the identity
/// was copied before the approval. The request is denied and the admin is alerted (C6).
///
/// The hub calls it after the older waiting connection answered its probe.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn deny_double_waiting(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    device: Uuid,
    address: Option<std::net::IpAddr>,
    now: i64,
) -> Result<DoubleWaitingOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let locked = super::lock_hermes_of_device(write.connection(), organization, device).await?;
    let Some(request) = locked
        .as_ref()
        .and_then(|locked| locked.device(device))
        .filter(|request| request.state == DeviceState::Pending)
    else {
        write.rollback().await?;
        return Ok(DoubleWaitingOutcome::NotPending);
    };
    close_device(
        &mut write,
        organization,
        request,
        StateReason::DoubleWaiting,
        &Actor::System,
        address,
        now,
    )
    .await?;
    alert::raise(
        write.connection(),
        organization,
        now,
        &NewAlert {
            kind: AlertKind::IdentityCopied,
            hermes: Some(request.hermes_id),
            device: Some(device),
            details: AlertDetails::new().address("from", address),
        },
    )
    .await?;
    write.commit().await?;
    Ok(DoubleWaitingOutcome::Denied)
}
