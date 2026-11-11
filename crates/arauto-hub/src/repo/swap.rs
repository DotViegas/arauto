//! The credential swap in the database (`docs/04-seguranca-conexao.md`, C7, C9 and C11;
//! `docs/14-protocolo-do-canal.md`, section 6).
//!
//! ```text
//!  issue      T1 of a connection: judge what was presented and write the new pending one
//!  rotate     T1 of the 24-hour swap, inside an open session
//!  confirm    T2: the "saved" of that connection promotes the pending one
//!  look       the presentation without swap: only look, never issue
//!  look_unknown   the same, for an identity the database does not know
//! ```
//!
//! T1 and T2 are short transactions. Each one locks the row of the Hermes and then the rows
//! of its devices, reads the device again and applies the pure rule of
//! [`crate::core::judge`]: nothing here decides, it only writes what the rule returns.
//!
//! - T1 writes the pending credential and the largest generation, durably, **before** the
//!   credential is sent (C7, rule 6).
//! - T2 only promotes when the device is still approved or active and the digest of the
//!   "saved" is the one of the pending credential issued **on that connection**. What was
//!   issued on a connection is an [`IssuedHere`], which only T1 creates.
//! - The first confirmation makes the device active and, on a reconnect, makes the device in
//!   use **replaced** in the same write: the old one leaves its place before the new one
//!   takes it.
//! - Nothing is ever issued on the identity alone after generation zero: that is the rule,
//!   and T1 reads the device under the lock before asking it.
//! - T1 and T2 also read, under that lock, the record of the connection they were called
//!   for. A connection that is closed, or that is of another device, gets nothing; and the
//!   24-hour swap, which issues without anything being presented, only runs on a connection
//!   whose record shows a confirmed swap. The hub does not take the word of its caller for
//!   "this is a session".
//! - A deadline that passed (24 hours approved, 30 days without connecting) is applied by
//!   T1 and T2 themselves when they meet it before the sweep does. Proof of a restoration
//!   is looked at first: it holds in every state, and expiring the device would hide it.

use std::net::IpAddr;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::DAY_MS;
use arauto_protocol::message::{DeclaredText, Saved};
use arauto_protocol::names::HermesName;
use arauto_protocol::noise::PublicKey;
use arauto_protocol::secret::SealedCredential;
use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use super::alert::{self, AlertDetails, AlertKind, NewAlert};
use super::connection::{self, ChannelConnection, OnRecord};
use super::sweep::{expire_device, overdue};
use super::{Locked, RepoError, lock_hermes_of_device, stored_address};
use crate::audit::{self, Action, Actor, Details, Event, SecurityWrite, TargetKind};
use crate::core::judge::{
    self, Credential, Decision, Mode, Presented, QuarantineReason, Transition, TransitionError,
};
use crate::core::model::{self, Device, DeviceState, StateReason};
use crate::db::ResolvedOrg;
use crate::mark::Mark;
use crate::restore::{self, Evidence, ListedAs, Listing, UnknownDevice};
use crate::vault::OrganizationKey;

/// The credential a connection received in T1. Only T1 creates one, and T2 takes it back:
/// that is how "the pending credential issued on that connection" is told from any other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedHere {
    device: Uuid,
    connection: Uuid,
    credential: Credential,
}

impl IssuedHere {
    /// The generation and the digest of what was issued.
    #[must_use]
    pub const fn credential(&self) -> Credential {
        self.credential
    }
}

/// The first confirmation of a device: it became active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Activated {
    /// The device it replaced, on a reconnect. Its connection, if any, gets AR-503.
    pub replaced: Option<Uuid>,
}

/// A credential that T1 wrote as pending and that may now be sent.
#[derive(Debug)]
pub struct Issued {
    /// The block for the `credential` message. A lasting secret: it goes to the connector
    /// and nowhere else.
    pub sealed: SealedCredential,
    /// What this connection received, for T2.
    pub receipt: IssuedHere,
    /// The pending credential was presented and became the current one in this write.
    pub promoted: bool,
    /// That promotion was the first confirmation of the device.
    pub activated: Option<Activated>,
}

/// What the hub saw in a presentation it did not swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    /// Nothing worth a record.
    Nothing,
    /// A closed device presented a valid seal older than its last ones. The alert "reuse
    /// after closing" was raised (once per device per 24 hours).
    ReuseAfterClosing,
    /// A valid seal the database does not explain, from a device that no longer counts as
    /// proof of a restoration. It was recorded.
    Unexplained,
    /// A request closed by a restoration presented a valid seal: a Hermes connected after the
    /// copy. It was listed with the unknown devices and gets AR-504.
    Listed(UnknownDevice),
}

/// A presentation that was not swapped: the state of the device is the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotSwapped {
    /// The state the device is in.
    pub state: DeviceState,
    /// The code of the `closing`. `None` while the device is pending, approved or active:
    /// then the caller knows why it asked (an outdated connector gets AR-401).
    pub code: Option<ErrorCode>,
    /// What the hub saw.
    pub seen: Seen,
}

/// The outcome of T1.
#[derive(Debug)]
pub enum IssueOutcome {
    /// The new credential is the pending one, durably. Send it.
    Issued(Box<Issued>),
    /// The device went to quarantine, for this reason. The alert was raised.
    Quarantined(StateReason),
    /// A valid seal of a generation above every one the database knows: the hub came back
    /// from an older copy. **Nothing was written.** Turn the restored mode on with
    /// [`crate::restore::enter`] and close the connection.
    Restoration(Evidence),
    /// The device is not approved or active, or was not when T1 read it again.
    NotSwapped(NotSwapped),
    /// [`rotate`] only: a pending credential is still waiting for its "saved".
    NotDue,
    /// [`rotate`] only: no swap was ever confirmed on this connection, so it is not a
    /// session. **Nothing was written**: past generation zero the hub never issues on the
    /// identity alone.
    NotASession,
    /// The connection is not on record as an open connection of this device: it was closed,
    /// or it is of another device or organization. **Nothing was written.**
    ConnectionClosed,
}

/// The outcome of [`look`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookOutcome {
    /// The device went to quarantine, for this reason. The alert was raised.
    Quarantined(StateReason),
    /// Proof of a restoration. **Nothing was written.** Turn the restored mode on with
    /// [`crate::restore::enter`].
    Restoration(Evidence),
    /// The hub looked, and the state of the device is the answer.
    NotSwapped(NotSwapped),
    /// The connection is not on record as an open connection of this device. **Nothing was
    /// written**, and nothing was looked at.
    ConnectionClosed,
}

/// What the first transaction was asked to do.
#[derive(Clone, Copy)]
enum Asked<'a> {
    /// The swap, with what the connection presented.
    Swap(Option<&'a [u8]>),
    /// Only to look at what the connection presented.
    Look(Option<&'a [u8]>),
    /// The 24-hour swap of an open session.
    Rotation,
}

fn opened(key: &OrganizationKey, device: &Device, sealed: Option<&[u8]>) -> Presented {
    match sealed {
        None => Presented::Nothing,
        Some(sealed) => match key.open_credential(sealed, &device.public_key) {
            Ok(seal) => Presented::Opened(Credential {
                generation: seal.generation,
                digest: seal.digest,
            }),
            Err(_) => Presented::Unreadable,
        },
    }
}

const fn quarantine_reason(reason: QuarantineReason) -> StateReason {
    match reason {
        QuarantineReason::Reuse => StateReason::Reuse,
        QuarantineReason::Unreadable => StateReason::UnreadableCredential,
    }
}

/// Puts a device in quarantine: the state, the security event and the alert. The device
/// keeps its place: there is no "release", only reconnecting or revoking.
async fn quarantine_device(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    device: &Device,
    reason: StateReason,
    address: Option<IpAddr>,
    now: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE device SET state = 'quarantined', state_reason = $3, state_since = $4 \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(device.id)
    .bind(reason.as_str())
    .bind(now)
    .execute(write.connection())
    .await?;
    let mut event = Event::new(Action::DeviceQuarantined, Actor::System)
        .in_organization(organization)
        .about(TargetKind::Device, device.id)
        .ended(crate::audit::Outcome::Ok, Some(ErrorCode::Ar501))
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
    alert::raise(
        write.connection(),
        organization,
        now,
        &NewAlert {
            kind: AlertKind::Quarantined,
            hermes: Some(device.hermes_id),
            device: Some(device.id),
            details: AlertDetails::new()
                .word("reason", reason.as_str())
                .address("from", address)
                .maybe_number("last_use_at", device.last_contact_at)
                .address(
                    "last_use_from",
                    stored_address(device.last_address.as_deref()),
                ),
        },
    )
    .await?;
    Ok(())
}

/// Writes the device after a transition of the swap rule. On a first confirmation the device
/// in use, if there is one, becomes replaced first: it leaves its place before the new one
/// takes it.
async fn write_transition(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    locked: &Locked,
    device: &Device,
    transition: &Transition,
    connection: &ChannelConnection,
    now: i64,
) -> Result<Option<Activated>, RepoError> {
    let after = &transition.device;
    let first = transition.first_confirmation;
    let mut activated = None;
    if first {
        let mut replaced = None;
        if let Some(old) = locked.in_use().filter(|old| old.id != device.id) {
            sqlx::query(
                "UPDATE device \
                 SET state = 'replaced', state_reason = $3, state_since = $4, slot = NULL \
                 WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(old.id)
            .bind(StateReason::Reconnected.as_str())
            .bind(now)
            .execute(write.connection())
            .await?;
            write
                .record(
                    now,
                    &Event::new(Action::DeviceReplaced, Actor::System)
                        .in_organization(organization)
                        .about(TargetKind::Device, old.id)
                        .with(
                            Details::new()
                                .word("was", old.state.as_str())
                                .id("by", device.id)
                                .id("hermes", device.hermes_id),
                        ),
                )
                .await?;
            replaced = Some(old.id);
        }
        activated = Some(Activated { replaced });
    }
    sqlx::query(
        "UPDATE device \
         SET state = $3, state_reason = NULL, \
             state_since = CASE WHEN $4 THEN $5 ELSE state_since END, \
             slot = CASE WHEN $4 THEN 'in_use' ELSE slot END, \
             first_confirmed_at = CASE WHEN $4 THEN $5 ELSE first_confirmed_at END, \
             approved_until = CASE WHEN $4 THEN NULL ELSE approved_until END, \
             current_generation = $6, current_digest = $7, \
             pending_generation = $8, pending_digest = $9, max_generation = $10, \
             last_contact_at = $5, last_address = $11, connector_version = $12, proto = $13 \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(device.id)
    .bind(after.state.as_str())
    .bind(first)
    .bind(now)
    .bind(after.current.map_or(0, |c| c.generation.as_i64()))
    .bind(after.current.map(|c| c.digest.as_bytes().to_vec()))
    .bind(after.pending.map(|c| c.generation.as_i64()))
    .bind(after.pending.map(|c| c.digest.as_bytes().to_vec()))
    .bind(i64::try_from(after.max_generation).unwrap_or(i64::MAX))
    .bind(connection.address.map(|address| address.to_string()))
    .bind(connection.connector_version.as_str())
    .bind(i32::try_from(connection.proto).unwrap_or(i32::MAX))
    .execute(write.connection())
    .await?;
    if let Some(activated) = activated {
        let mut details = Details::new().id("hermes", device.hermes_id);
        if let Some(replaced) = activated.replaced {
            details = details.id("replaced", replaced);
        }
        let mut event = Event::new(Action::DeviceActivated, Actor::Device { id: device.id })
            .in_organization(organization)
            .about(TargetKind::Device, device.id)
            .with(details);
        if let Some(address) = connection.address {
            event = event.from_address(address);
        }
        write.record(now, &event).await?;
    }
    Ok(activated)
}

/// The security event of a confirmed swap.
async fn record_confirmed(
    write: &mut SecurityWrite,
    organization: ResolvedOrg,
    device: &Device,
    confirmed: Credential,
    by: &'static str,
    connection: &ChannelConnection,
    now: i64,
) -> Result<(), RepoError> {
    let mut event = Event::new(Action::CredentialConfirmed, Actor::Device { id: device.id })
        .in_organization(organization)
        .about(TargetKind::Device, device.id)
        .with(
            Details::new()
                .number("generation", confirmed.generation.as_i64())
                .word("by", by),
        );
    if let Some(address) = connection.address {
        event = event.from_address(address);
    }
    write.record(now, &event).await?;
    connection::mark_confirmed(write.connection(), organization, connection.id, now).await?;
    Ok(())
}

/// Whether an event of `action` about `target` was recorded after `since`.
async fn recorded_since(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    action: Action,
    target: Uuid,
    since: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM audit WHERE organization_id = $1 AND at > $2 \
                        AND action = $3 AND target_id = $4)",
    )
    .bind(organization.id())
    .bind(since)
    .bind(action.as_str())
    .bind(target)
    .fetch_one(connection)
    .await
}

/// What a judged presentation ended in.
enum Judged {
    Issued(Box<Issued>),
    Quarantined(StateReason),
    Restoration(Evidence),
    NotSwapped(NotSwapped),
    NotDue,
    NotASession,
    ConnectionClosed,
}

impl From<Judged> for IssueOutcome {
    fn from(judged: Judged) -> Self {
        match judged {
            Judged::Issued(issued) => IssueOutcome::Issued(issued),
            Judged::Quarantined(reason) => IssueOutcome::Quarantined(reason),
            Judged::Restoration(evidence) => IssueOutcome::Restoration(evidence),
            Judged::NotSwapped(not_swapped) => IssueOutcome::NotSwapped(not_swapped),
            Judged::NotDue => IssueOutcome::NotDue,
            Judged::NotASession => IssueOutcome::NotASession,
            Judged::ConnectionClosed => IssueOutcome::ConnectionClosed,
        }
    }
}

/// The one transaction behind [`issue`], [`rotate`] and [`look`].
async fn first_transaction(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    connection: &ChannelConnection,
    asked: Asked<'_>,
    now: i64,
) -> Result<Judged, RepoError> {
    if key.organization_id() != organization.id() {
        return Err(RepoError::Inconsistent(
            "the key is of another organization",
        ));
    }
    let mut write = audit::begin_security(pool, mark).await?;
    let locked = lock_hermes_of_device(write.connection(), organization, connection.device)
        .await?
        .ok_or(RepoError::Inconsistent(
            "the device of a connection is not in its organization",
        ))?;
    let device = locked
        .device(connection.device)
        .ok_or(RepoError::Inconsistent(
            "the device of a connection is not in its organization",
        ))?;
    // The record of the connection is read under the lock too: nothing is issued, promoted
    // or judged on a connection that is not on record as open and as being of this device,
    // and the 24-hour swap only runs where a swap was already confirmed. After generation
    // zero the hub never issues on the identity alone (C7, rule 8), and it does not take the
    // word of its caller for that.
    match connection::lock_on_record(write.connection(), organization, connection).await? {
        OnRecord::Closed => {
            write.rollback().await?;
            return Ok(Judged::ConnectionClosed);
        }
        OnRecord::Open {
            swap_confirmed: false,
        } if matches!(asked, Asked::Rotation) => {
            write.rollback().await?;
            return Ok(Judged::NotASession);
        }
        OnRecord::Open { .. } => {}
    }
    let snapshot = device.snapshot();
    let (presented, decision) = match asked {
        Asked::Swap(sealed) => {
            let presented = opened(key, device, sealed);
            (presented, judge::judge(&snapshot, &presented, Mode::Swap))
        }
        Asked::Look(sealed) => {
            let presented = opened(key, device, sealed);
            (
                presented,
                judge::judge(&snapshot, &presented, Mode::PresentationOnly),
            )
        }
        Asked::Rotation => (Presented::Nothing, judge::rotation(&snapshot)),
    };
    // A deadline that passed and that the sweep has not applied yet is applied here: the
    // answer does not depend on when the sweep last ran. Proof of a restoration comes first:
    // it holds in every state (C7), and expiring the device would hide it.
    if decision != Decision::RestorationProof
        && let Some(reason) = overdue(device, now)
    {
        expire_device(&mut write, organization, device, reason, now).await?;
        write.commit().await?;
        return Ok(Judged::NotSwapped(NotSwapped {
            state: DeviceState::Expired,
            code: model::ended_code(DeviceState::Expired, Some(reason)),
            seen: Seen::Nothing,
        }));
    }
    let not_swapped = |seen: Seen| NotSwapped {
        state: device.state,
        code: device.ended_code(),
        seen,
    };
    // A request the restored mode closed, with a valid seal, is a Hermes connected after the
    // copy: whatever else the rule says of the seal, it is listed and told the hub does not
    // recognize it (C11).
    let from_lost_timeline = matches!(presented, Presented::Opened(_))
        && device
            .state_reason
            .is_some_and(StateReason::closed_by_restoration);

    match decision {
        Decision::Accept { promote_pending } => {
            let generation = judge::next_generation(&snapshot)?;
            let seal = key.seal_credential(&device.public_key, generation, now)?;
            let new = Credential {
                generation: seal.generation,
                digest: seal.digest,
            };
            let transition = judge::issue(&snapshot, promote_pending, new)?;
            let activated = write_transition(
                &mut write,
                organization,
                &locked,
                device,
                &transition,
                connection,
                now,
            )
            .await?;
            if promote_pending && let Some(promoted) = transition.device.current {
                record_confirmed(
                    &mut write,
                    organization,
                    device,
                    promoted,
                    "presentation",
                    connection,
                    now,
                )
                .await?;
            }
            connection::mark_issued(write.connection(), organization, connection.id, now).await?;
            write.commit().await?;
            Ok(Judged::Issued(Box::new(Issued {
                sealed: seal.sealed,
                receipt: IssuedHere {
                    device: device.id,
                    connection: connection.id,
                    credential: new,
                },
                promoted: promote_pending,
                activated,
            })))
        }
        Decision::Quarantine(reason) => {
            let reason = quarantine_reason(reason);
            quarantine_device(
                &mut write,
                organization,
                device,
                reason,
                connection.address,
                now,
            )
            .await?;
            write.commit().await?;
            Ok(Judged::Quarantined(reason))
        }
        Decision::RestorationProof => {
            write.rollback().await?;
            Ok(Judged::Restoration(Evidence::SealBeyondDatabase {
                device: device.id,
            }))
        }
        Decision::ReuseAfterClosing => {
            let since = now.saturating_sub(DAY_MS);
            let already = alert::raised_since(
                write.connection(),
                organization,
                AlertKind::ReuseAfterClosing,
                device.id,
                since,
            )
            .await?;
            if already {
                write.rollback().await?;
                return Ok(Judged::NotSwapped(not_swapped(Seen::ReuseAfterClosing)));
            }
            alert::raise(
                write.connection(),
                organization,
                now,
                &NewAlert {
                    kind: AlertKind::ReuseAfterClosing,
                    hermes: Some(device.hermes_id),
                    device: Some(device.id),
                    details: AlertDetails::new()
                        .word("state", device.state.as_str())
                        .address("from", connection.address)
                        .maybe_number("last_use_at", device.last_contact_at)
                        .address(
                            "last_use_from",
                            stored_address(device.last_address.as_deref()),
                        ),
                },
            )
            .await?;
            let mut event = Event::new(
                Action::CredentialReuseAfterClosing,
                Actor::Device { id: device.id },
            )
            .in_organization(organization)
            .about(TargetKind::Device, device.id)
            .with(Details::new().word("state", device.state.as_str()));
            if let Some(address) = connection.address {
                event = event.from_address(address);
            }
            write.record(now, &event).await?;
            write.commit().await?;
            Ok(Judged::NotSwapped(not_swapped(Seen::ReuseAfterClosing)))
        }
        Decision::RecordOnly => {
            let since = now.saturating_sub(DAY_MS);
            let already = recorded_since(
                write.connection(),
                organization,
                Action::CredentialUnexplained,
                device.id,
                since,
            )
            .await?;
            if already {
                write.rollback().await?;
            } else {
                let mut event = Event::new(
                    Action::CredentialUnexplained,
                    Actor::Device { id: device.id },
                )
                .in_organization(organization)
                .about(TargetKind::Device, device.id)
                .with(Details::new().word("state", device.state.as_str()));
                if let Some(address) = connection.address {
                    event = event.from_address(address);
                }
                write.record(now, &event).await?;
                write.commit().await?;
            }
            if from_lost_timeline {
                return listed_after_restoration(pool, organization, device, connection, now).await;
            }
            Ok(Judged::NotSwapped(not_swapped(Seen::Unexplained)))
        }
        Decision::Nothing => {
            write.rollback().await?;
            if matches!(asked, Asked::Rotation) && device.state.serves() {
                return Ok(Judged::NotDue);
            }
            if from_lost_timeline {
                return listed_after_restoration(pool, organization, device, connection, now).await;
            }
            Ok(Judged::NotSwapped(not_swapped(Seen::Nothing)))
        }
    }
}

/// Lists a request the restored mode closed and that presented a valid seal, and answers
/// AR-504: the hub does not recognize it, and its way back is reconnecting (C11).
async fn listed_after_restoration(
    pool: &PgPool,
    organization: ResolvedOrg,
    device: &Device,
    connection: &ChannelConnection,
    now: i64,
) -> Result<Judged, RepoError> {
    let unknown = restore::list_unknown_device(
        pool,
        organization,
        &Listing {
            identity: &device.public_key,
            connector_version: Some(&connection.connector_version),
            address: connection.address,
            listed_as: ListedAs::ClosedRequest,
        },
        now,
    )
    .await?;
    Ok(Judged::NotSwapped(NotSwapped {
        state: device.state,
        code: Some(ErrorCode::Ar504),
        seen: Seen::Listed(unknown),
    }))
}

/// T1 of a connection: judges what the newest connection of a device presented and, when it
/// is accepted, writes the new credential as the pending one.
///
/// `presented` is the block of the `present` message, or `None` for `null`. Call it only for
/// a connection that proved the identity in the handshake, answered its check and is the
/// newest one of the device (steps 3 to 8 of the swap): this function cannot see that. What
/// it does see, under the lock, is the record of the connection: one that is closed, or that
/// is not of this device, gets [`IssueOutcome::ConnectionClosed`] and nothing is written.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
/// With [`crate::audit::AuditError::MarkNotAdvanced`] the write **is committed** and the
/// credential is lost with the error: the connection closes, and the connector comes back
/// presenting what it has.
pub async fn issue(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    connection: &ChannelConnection,
    presented: Option<&[u8]>,
    now: i64,
) -> Result<IssueOutcome, RepoError> {
    let judged = first_transaction(
        pool,
        mark,
        organization,
        key,
        connection,
        Asked::Swap(presented),
        now,
    )
    .await?;
    Ok(judged.into())
}

/// T1 of the 24-hour swap, inside an open session: issues as if the current credential had
/// been presented. Nothing is issued while a pending credential waits for its "saved".
///
/// "As if the current one had been presented" only holds inside a session. The record of the
/// connection is read under the lock: when no swap was ever confirmed on it, the answer is
/// [`IssueOutcome::NotASession`] and nothing is issued. A connection that only proved the
/// identity never gets a credential here (C7, rule 8).
///
/// # Errors
/// As [`issue`].
pub async fn rotate(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    connection: &ChannelConnection,
    now: i64,
) -> Result<IssueOutcome, RepoError> {
    let judged = first_transaction(
        pool,
        mark,
        organization,
        key,
        connection,
        Asked::Rotation,
        now,
    )
    .await?;
    Ok(judged.into())
}

/// The presentation without swap, of a known device: an ended device, or an approved or
/// active one whose connector is outdated. The hub looks at what was presented and never
/// issues.
///
/// - Approved or active (outdated connector): reuse and an unreadable seal quarantine, as in
///   the swap; what would be accepted is nothing. The caller answers AR-401.
/// - Revoked, replaced or expired for inactivity, with a valid seal older than its last
///   ones: the alert "reuse after closing".
/// - Any state, a valid seal above every generation the database knows: proof of a
///   restoration, unless the device is marked by an earlier one.
/// - A request closed by a restoration, with a valid seal: listed, AR-504.
///
/// # Errors
/// As [`issue`].
pub async fn look(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    connection: &ChannelConnection,
    presented: Option<&[u8]>,
    now: i64,
) -> Result<LookOutcome, RepoError> {
    let judged = first_transaction(
        pool,
        mark,
        organization,
        key,
        connection,
        Asked::Look(presented),
        now,
    )
    .await?;
    match judged {
        Judged::Quarantined(reason) => Ok(LookOutcome::Quarantined(reason)),
        Judged::Restoration(evidence) => Ok(LookOutcome::Restoration(evidence)),
        Judged::NotSwapped(not_swapped) => Ok(LookOutcome::NotSwapped(not_swapped)),
        Judged::ConnectionClosed => Ok(LookOutcome::ConnectionClosed),
        // The rule never accepts in this mode, and only the 24-hour swap asks for a session.
        Judged::Issued(_) | Judged::NotDue | Judged::NotASession => Err(RepoError::Inconsistent(
            "a presentation without swap was accepted",
        )),
    }
}

/// What the hub found in a presentation of an identity it does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownOutcome {
    /// `null`, or a block that does not open under the key of this organization. Nothing is
    /// recorded for each attempt.
    NotOurs,
    /// A valid seal of this organization: a Hermes connected, or reconnected, after the copy
    /// the hub came back from. It is in the list of unknown devices; the answer is AR-504.
    Listed {
        /// Its entry in the list.
        unknown: UnknownDevice,
        /// Proof of a restoration, while the restored mode is off and no restored mode handled
        /// this listing yet: the same evidence comes back at every presentation until
        /// [`crate::restore::enter`] turns the mode on with it.
        evidence: Option<Evidence>,
    },
}

/// A connection of an identity the database does not know, and what it presented.
#[derive(Debug, Clone, Copy)]
pub struct Stranger<'a> {
    /// The identity it proved in the handshake.
    pub identity: &'a PublicKey,
    /// The block of its `present`, or `None` for `null`.
    pub presented: Option<&'a [u8]>,
    /// The version of the connector, from `hello`. Not verified.
    pub connector_version: Option<&'a DeclaredText>,
    /// The address the connection came from.
    pub address: Option<IpAddr>,
}

/// The presentation without swap, of an identity the database does not know (C11). The hub
/// tries the key of each organization ([`crate::db::SystemTask::UnknownSeal`]) and calls this
/// for each one, until one of them lists the identity.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn look_unknown(
    pool: &PgPool,
    organization: ResolvedOrg,
    key: &OrganizationKey,
    stranger: &Stranger<'_>,
    now: i64,
) -> Result<UnknownOutcome, RepoError> {
    if key.organization_id() != organization.id() {
        return Err(RepoError::Inconsistent(
            "the key is of another organization",
        ));
    }
    let Some(seal) = stranger
        .presented
        .and_then(|sealed| key.open_credential(sealed, stranger.identity).ok())
    else {
        return Ok(UnknownOutcome::NotOurs);
    };
    let unknown = restore::list_unknown_device(
        pool,
        organization,
        &Listing {
            identity: stranger.identity,
            connector_version: stranger.connector_version,
            address: stranger.address,
            listed_as: ListedAs::Stranger,
        },
        now,
    )
    .await?;
    let restored_mode = restore::installation(pool)
        .await?
        .is_some_and(|installation| installation.restored_mode);
    let presented = Presented::Opened(Credential {
        generation: seal.generation,
        digest: seal.digest,
    });
    let evidence = match judge::judge_unknown(&presented, unknown.proof_pending, restored_mode) {
        Decision::RestorationProof => Some(Evidence::UnknownDeviceSeal {
            unknown_device: unknown.id,
        }),
        _ => None,
    };
    Ok(UnknownOutcome::Listed { unknown, evidence })
}

/// What a session starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStart {
    /// The Hermes of the device.
    pub hermes: Uuid,
    /// Its name, for the `session` message.
    pub name: HermesName,
    /// Whether it can be woken.
    pub can_be_woken: bool,
}

/// Why a "saved" was refused. The connection is closed; nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavedRefused {
    /// It is not about the pending credential issued on this connection (C7, rule 7).
    NotThePending,
    /// The connection is not on record as an open connection of this device: nothing is
    /// promoted on a connection that already ended.
    ConnectionClosed,
    /// The device is no longer approved or active: it was revoked, quarantined or replaced
    /// between the credential and the "saved". The code is the one of its state.
    DeviceEnded {
        /// The state the device is in.
        state: DeviceState,
        /// The code of the `closing`.
        code: Option<ErrorCode>,
    },
}

/// The outcome of T2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmOutcome {
    /// The pending credential is now the current one, and the one before it is dead. Only
    /// now may the session serve.
    Confirmed {
        /// This was the first confirmation of the device.
        activated: Option<Activated>,
        /// What the `session` message says.
        session: SessionStart,
    },
    /// The "saved" was refused.
    Refused(SavedRefused),
}

/// T2: the "saved" of a connection promotes the pending credential.
///
/// It only writes when the device is still approved or active **and** the generation and the
/// digest of `saved` are the ones of `receipt`, the credential T1 issued on this very
/// connection, and that credential is still the pending one.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
///
/// With [`crate::audit::AuditError::MarkNotAdvanced`] the promotion **is committed** and the
/// outcome is lost with the error, on purpose: the hub opens no session while the mark, which
/// is how it notices a restored database, cannot follow the events. The device is not locked
/// out: the connector saved the credential before saying so, it presents it when it comes
/// back, and that credential is the current one. The caller closes the connection and tells
/// the admin that the mark could not be written.
pub async fn confirm(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    connection: &ChannelConnection,
    receipt: &IssuedHere,
    saved: &Saved,
    now: i64,
) -> Result<ConfirmOutcome, RepoError> {
    if receipt.device != connection.device || receipt.connection != connection.id {
        return Ok(ConfirmOutcome::Refused(SavedRefused::NotThePending));
    }
    let mut write = audit::begin_security(pool, mark).await?;
    let locked = lock_hermes_of_device(write.connection(), organization, connection.device)
        .await?
        .ok_or(RepoError::Inconsistent(
            "the device of a connection is not in its organization",
        ))?;
    let device = locked
        .device(connection.device)
        .ok_or(RepoError::Inconsistent(
            "the device of a connection is not in its organization",
        ))?;
    if connection::lock_on_record(write.connection(), organization, connection).await?
        == OnRecord::Closed
    {
        write.rollback().await?;
        return Ok(ConfirmOutcome::Refused(SavedRefused::ConnectionClosed));
    }
    if let Some(reason) = overdue(device, now) {
        expire_device(&mut write, organization, device, reason, now).await?;
        write.commit().await?;
        return Ok(ConfirmOutcome::Refused(SavedRefused::DeviceEnded {
            state: DeviceState::Expired,
            code: model::ended_code(DeviceState::Expired, Some(reason)),
        }));
    }
    let said = Credential {
        generation: saved.generation,
        digest: saved.digest,
    };
    let transition = match judge::confirm(&device.snapshot(), &receipt.credential, &said) {
        Ok(transition) => transition,
        Err(TransitionError::NotServing) => {
            write.rollback().await?;
            return Ok(ConfirmOutcome::Refused(SavedRefused::DeviceEnded {
                state: device.state,
                code: device.ended_code(),
            }));
        }
        Err(TransitionError::NotThePending) => {
            write.rollback().await?;
            return Ok(ConfirmOutcome::Refused(SavedRefused::NotThePending));
        }
        Err(other) => return Err(other.into()),
    };
    let activated = write_transition(
        &mut write,
        organization,
        &locked,
        device,
        &transition,
        connection,
        now,
    )
    .await?;
    record_confirmed(
        &mut write,
        organization,
        device,
        receipt.credential,
        "saved",
        connection,
        now,
    )
    .await?;
    write.commit().await?;
    Ok(ConfirmOutcome::Confirmed {
        activated,
        session: SessionStart {
            hermes: locked.hermes.id,
            name: locked.hermes.name.clone(),
            can_be_woken: locked.hermes.can_be_woken,
        },
    })
}

/// The outcome of [`quarantine_double_connection`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleConnectionOutcome {
    /// The device went to quarantine and the alert was raised. Both connections close.
    Quarantined,
    /// The device was no longer approved or active when the hub read it again: nothing was
    /// written, and its state is the answer.
    NotServing(DeviceState),
}

/// A presentation that would be accepted arrived while another connection of the same device
/// answered its probe: two holders of the same files (C9). The second one is refused and the
/// device goes to quarantine.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn quarantine_double_connection(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    device: Uuid,
    address: Option<IpAddr>,
    now: i64,
) -> Result<DoubleConnectionOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let locked = lock_hermes_of_device(write.connection(), organization, device)
        .await?
        .ok_or(RepoError::Inconsistent(
            "the device of a connection is not in its organization",
        ))?;
    let found = locked.device(device).ok_or(RepoError::Inconsistent(
        "the device of a connection is not in its organization",
    ))?;
    if !found.state.serves() {
        write.rollback().await?;
        return Ok(DoubleConnectionOutcome::NotServing(found.state));
    }
    quarantine_device(
        &mut write,
        organization,
        found,
        StateReason::DoubleConnection,
        address,
        now,
    )
    .await?;
    write.commit().await?;
    Ok(DoubleConnectionOutcome::Quarantined)
}
