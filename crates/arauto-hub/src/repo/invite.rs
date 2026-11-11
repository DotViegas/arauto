//! Issuing an invite (`docs/04-seguranca-conexao.md`, C1 and C3).
//!
//! The invite is 32 bytes from the generator of the system. The hub keeps only its SHA-256,
//! for 15 minutes; the invite itself is returned once, to be shown inside the block, and is
//! never stored.
//!
//! **One request per Hermes.** Issuing an invite for a Hermes cancels the invite that was
//! issued and closes the request in progress: the pending one, the approved one that has not
//! confirmed a credential yet, and the one that went to quarantine before confirming. The
//! device in use is not touched: it goes on until the new one confirms its first credential.

use arauto_protocol::invite::Invite as InviteSecret;
use arauto_protocol::limits::MINUTE_MS;
use arauto_protocol::names::HermesName;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::closing::{ClosedDevice, cancel_issued_invite, close_device};
use super::{Admin, RepoError, lock_hermes_by_name, notify_hub};
use crate::audit::{self, Action, Details, Event, TargetKind};
use crate::core::model::{DeviceState, Hermes, StateReason};
use crate::db::ResolvedOrg;
use crate::mark::Mark;

/// For how long an invite is accepted.
pub const INVITE_VALIDITY_MS: i64 = 15 * MINUTE_MS;

/// An invite that was just issued.
#[derive(Debug)]
pub struct IssuedInvite {
    /// The invite itself. This is the only moment it exists outside the connector: show it
    /// once, inside the block, and drop it.
    pub invite: InviteSecret,
    /// The row that holds its digest.
    pub id: Uuid,
    /// When it stops being accepted.
    pub expires_at: i64,
    /// The Hermes it is for.
    pub hermes: Hermes,
    /// Whether the Hermes has a device in use: this is a reconnect.
    pub reconnect: bool,
    /// The invite that was issued before and is now cancelled.
    pub cancelled: Option<Uuid>,
    /// The request in progress that was closed.
    pub closed_request: Option<ClosedDevice>,
}

/// The outcome of [`issue`].
#[derive(Debug)]
pub enum IssueOutcome {
    /// The invite was issued.
    Issued(Box<IssuedInvite>),
    /// There is no Hermes with that name.
    NotFound,
}

/// Issues an invite for a Hermes, for a first connection or for a reconnect.
///
/// In one write: the invite issued before is cancelled, the request in progress is closed
/// (a pending one becomes denied; an approved or quarantined one that never confirmed a
/// credential becomes revoked), and the digest of the new invite is stored.
///
/// # Errors
/// [`RepoError::Random`] when the generator of the system fails; [`RepoError::Restored`] when
/// the database is behind the audit mark. Nothing is written in either case.
pub async fn issue(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    name: &HermesName,
    now: i64,
) -> Result<IssueOutcome, RepoError> {
    let invite = InviteSecret::generate().map_err(|_| RepoError::Random)?;
    let mut write = audit::begin_security(pool, mark).await?;
    let Some(locked) = lock_hermes_by_name(write.connection(), organization, name).await? else {
        write.rollback().await?;
        return Ok(IssueOutcome::NotFound);
    };
    let actor = admin.actor();
    let cancelled = cancel_issued_invite(
        &mut write,
        organization,
        locked.hermes.id,
        &actor,
        "new_invite",
        now,
    )
    .await?;
    let closed_request = match locked.request() {
        Some(request) => {
            let closed = close_device(
                &mut write,
                organization,
                request,
                StateReason::NewInvite,
                &actor,
                None,
                now,
            )
            .await?;
            let kind = if closed.now == DeviceState::Denied {
                "denied"
            } else {
                "revoked"
            };
            notify_hub(write.connection(), organization, kind, request.id).await?;
            Some(closed)
        }
        None => None,
    };

    let id = Uuid::new_v4();
    let expires_at = now.saturating_add(INVITE_VALIDITY_MS);
    sqlx::query(
        "INSERT INTO invite (id, organization_id, hermes_id, digest, state, created_by, \
                             created_at, expires_at) \
         VALUES ($1, $2, $3, $4, 'issued', $5, $6, $7)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(locked.hermes.id)
    .bind(invite.digest().as_bytes().as_slice())
    .bind(admin.id)
    .bind(now)
    .bind(expires_at)
    .execute(write.connection())
    .await?;
    let reconnect = locked.in_use().is_some();
    write
        .record(
            now,
            &Event::new(Action::InviteIssued, actor)
                .in_organization(organization)
                .about(TargetKind::Invite, id)
                .with(
                    Details::new()
                        .id("hermes", locked.hermes.id)
                        .flag("reconnect", reconnect)
                        .number("expires_at", expires_at),
                ),
        )
        .await?;
    write.commit().await?;
    Ok(IssueOutcome::Issued(Box::new(IssuedInvite {
        invite,
        id,
        expires_at,
        hermes: locked.hermes,
        reconnect,
        cancelled,
        closed_request,
    })))
}
