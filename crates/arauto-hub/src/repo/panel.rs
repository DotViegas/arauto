//! The entry and the sessions of the panel of the admin
//! (`docs/decisoes/0040-painel-dentro-do-hub.md`).
//!
//! **The entry code.** The admin command draws it on the machine of the hub and shows it
//! once. The hub keeps its SHA-256 for 10 minutes. Typed in the panel, it opens one session
//! and dies; issuing another cancels the one before. Whoever can run the admin command
//! already holds the database and the master key: the code opens no way that was closed.
//!
//! **The session.** The identifier that goes in the cookie is drawn here and kept only as its
//! SHA-256. A session lasts 12 hours; how long it has been idle (30 minutes ends it) is kept
//! in the memory of the hub, so that reading a page writes nothing. For the same reason a
//! session that passed its hours is only marked as closed by the next write: a sign-in, a
//! sign-out, or the start of the hub.

use std::net::IpAddr;

use arauto_protocol::limits::{HOUR_MS, MINUTE_MS};
use arauto_protocol::message::DeclaredText;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::{Admin, RepoError};
use crate::audit::{self, Action, Details, Event, TargetKind};
use crate::db::ResolvedOrg;
use crate::mark::Mark;
use crate::panel::secret::{EntryCode, PanelDigest, SessionSecret};

/// For how long an entry code is accepted.
pub const ENTRY_VALIDITY_MS: i64 = 10 * MINUTE_MS;

/// For how long a session of the panel lasts, used or not.
pub const SESSION_VALIDITY_MS: i64 = 12 * HOUR_MS;

/// For how long a session of the panel may stay without a request.
pub const SESSION_IDLE_MS: i64 = 30 * MINUTE_MS;

/// What the audit says ran an action done through the panel, in the place of the system
/// user of the admin command.
pub const PANEL_ACTOR: &str = "panel";

/// An entry code that was just issued.
#[derive(Debug)]
pub struct IssuedEntry {
    /// The code itself. This is the only moment it exists outside the head of the admin:
    /// show it once and drop it.
    pub code: EntryCode,
    /// The row that holds its digest.
    pub id: Uuid,
    /// When it stops being accepted.
    pub expires_at: i64,
}

/// Issues an entry code. The one issued before, used or not, stops being accepted.
///
/// # Errors
/// [`RepoError::Random`] when the generator of the system fails; [`RepoError::Database`].
pub async fn issue_entry(
    pool: &PgPool,
    organization: ResolvedOrg,
    admin: &Admin,
    now: i64,
) -> Result<IssuedEntry, RepoError> {
    let code = EntryCode::generate().map_err(|_| RepoError::Random)?;
    let id = Uuid::new_v4();
    let expires_at = now.saturating_add(ENTRY_VALIDITY_MS);
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "UPDATE panel_entry SET state = 'cancelled', digest = NULL \
         WHERE organization_id = $1 AND state = 'issued'",
    )
    .bind(organization.id())
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO panel_entry (id, organization_id, digest, state, created_by, created_at, \
                                  expires_at) \
         VALUES ($1, $2, $3, 'issued', $4, $5, $6)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(code.digest().as_bytes().as_slice())
    .bind(admin.id)
    .bind(now)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await?;
    audit::record(
        &mut *transaction,
        now,
        &Event::new(Action::PanelCodeIssued, admin.actor())
            .in_organization(organization)
            .about(TargetKind::Admin, admin.id)
            .with(Details::new().number("expires_at", expires_at)),
    )
    .await?;
    transaction.commit().await?;
    Ok(IssuedEntry {
        code,
        id,
        expires_at,
    })
}

/// A session of the panel that was just opened.
#[derive(Debug)]
pub struct OpenedSession {
    /// The identifier for the cookie. It exists here and in the browser, nowhere else.
    pub secret: SessionSecret,
    /// The row that holds its digest.
    pub id: Uuid,
    /// When it ends, used or not.
    pub expires_at: i64,
}

/// The outcome of [`sign_in`].
#[derive(Debug)]
pub enum SignIn {
    /// The code was the one issued and was in its minutes: it is spent and a session is open.
    Opened(OpenedSession),
    /// The code is not the one issued, or its minutes passed. Nothing was written, and the
    /// answer does not say which.
    Refused,
}

/// Spends an entry code and opens a session of the panel, in one write.
///
/// The row of the code is read under its lock: of two requests with the same code, one opens
/// a session and the other is refused.
///
/// # Errors
/// [`RepoError::Random`]; [`RepoError::Restored`] when the database is behind the audit mark
/// (nothing is written); [`RepoError::Audit`] with `MarkNotAdvanced` when the session **is**
/// open and the mark did not follow.
pub async fn sign_in(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    entry: &PanelDigest,
    address: Option<IpAddr>,
    now: i64,
) -> Result<SignIn, RepoError> {
    let secret = SessionSecret::generate().map_err(|_| RepoError::Random)?;
    let mut write = audit::begin_security(pool, mark).await?;
    let found: Option<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT id, created_by, expires_at FROM panel_entry \
         WHERE organization_id = $1 AND digest = $2 AND state = 'issued' FOR UPDATE",
    )
    .bind(organization.id())
    .bind(entry.as_bytes().as_slice())
    .fetch_optional(write.connection())
    .await?;
    // A deadline passed when the time of the hub reached it.
    let Some((entry_id, admin, _)) = found.filter(|(_, _, expires_at)| *expires_at > now) else {
        write.rollback().await?;
        return Ok(SignIn::Refused);
    };
    sqlx::query(
        "UPDATE panel_entry SET state = 'used', digest = NULL, used_at = $3 \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(entry_id)
    .bind(now)
    .execute(write.connection())
    .await?;
    close_expired(write.connection(), organization, now).await?;

    let id = Uuid::new_v4();
    let expires_at = now.saturating_add(SESSION_VALIDITY_MS);
    sqlx::query(
        "INSERT INTO panel_session (id, organization_id, admin_id, digest, state, opened_from, \
                                    created_at, expires_at) \
         VALUES ($1, $2, $3, $4, 'open', $5, $6, $7)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(admin)
    .bind(secret.digest().as_bytes().as_slice())
    .bind(address.map(|address| address.to_string()))
    .bind(now)
    .bind(expires_at)
    .execute(write.connection())
    .await?;
    let mut event = Event::new(Action::PanelSignedIn, panel_admin(admin).actor())
        .in_organization(organization)
        .about(TargetKind::Admin, admin)
        .with(
            Details::new()
                .id("session", id)
                .id("entry", entry_id)
                .number("expires_at", expires_at),
        );
    if let Some(address) = address {
        event = event.from_address(address);
    }
    write.record(now, &event).await?;
    write.commit().await?;
    Ok(SignIn::Opened(OpenedSession {
        secret,
        id,
        expires_at,
    }))
}

/// An open session of the panel, as the database has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelSession {
    /// The row.
    pub id: Uuid,
    /// The admin it belongs to.
    pub admin: Uuid,
    /// When it was opened.
    pub created_at: i64,
    /// When it ends, used or not.
    pub expires_at: i64,
}

/// The open session with this digest. Only reads: a session that passed its hours is still
/// returned, and whoever asks compares `expires_at` with the time of the hub.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn session(
    pool: &PgPool,
    organization: ResolvedOrg,
    digest: &PanelDigest,
) -> Result<Option<PanelSession>, RepoError> {
    let found: Option<(Uuid, Uuid, i64, i64)> = sqlx::query_as(
        "SELECT id, admin_id, created_at, expires_at FROM panel_session \
         WHERE organization_id = $1 AND digest = $2 AND state = 'open'",
    )
    .bind(organization.id())
    .bind(digest.as_bytes().as_slice())
    .fetch_optional(pool)
    .await?;
    Ok(
        found.map(|(id, admin, created_at, expires_at)| PanelSession {
            id,
            admin,
            created_at,
            expires_at,
        }),
    )
}

/// Closes a session at the request of whoever holds it. Returns whether it was open.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn sign_out(
    pool: &PgPool,
    organization: ResolvedOrg,
    session: PanelSession,
    address: Option<IpAddr>,
    now: i64,
) -> Result<bool, RepoError> {
    let mut transaction = pool.begin().await?;
    let closed = sqlx::query(
        "UPDATE panel_session \
         SET state = 'closed', digest = NULL, closed_at = $3, close_reason = 'signed_out' \
         WHERE organization_id = $1 AND id = $2 AND state = 'open'",
    )
    .bind(organization.id())
    .bind(session.id)
    .bind(now)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if closed == 0 {
        transaction.rollback().await?;
        return Ok(false);
    }
    close_expired(&mut transaction, organization, now).await?;
    let mut event = Event::new(Action::PanelSignedOut, panel_admin(session.admin).actor())
        .in_organization(organization)
        .about(TargetKind::Admin, session.admin)
        .with(Details::new().id("session", session.id));
    if let Some(address) = address {
        event = event.from_address(address);
    }
    audit::record(&mut *transaction, now, &event).await?;
    transaction.commit().await?;
    Ok(true)
}

/// Closes every open session of the organization: the hub is starting, and the memory that
/// said how long each one had been idle is gone. Returns how many there were.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn close_left_open(
    pool: &PgPool,
    organization: ResolvedOrg,
    now: i64,
) -> Result<u64, RepoError> {
    let closed = sqlx::query(
        "UPDATE panel_session \
         SET state = 'closed', digest = NULL, closed_at = $2, close_reason = 'hub_stopped' \
         WHERE organization_id = $1 AND state = 'open'",
    )
    .bind(organization.id())
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(closed)
}

/// Marks as closed the sessions that passed their hours, and erases their digests.
async fn close_expired(
    connection: &mut sqlx::postgres::PgConnection,
    organization: ResolvedOrg,
    now: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE panel_session \
         SET state = 'closed', digest = NULL, closed_at = $2, close_reason = 'expired' \
         WHERE organization_id = $1 AND state = 'open' AND expires_at <= $2",
    )
    .bind(organization.id())
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// The admin of a session of the panel, as the audit names it: the row of the admin, with a
/// fixed word where the admin command puts the system user.
fn panel_admin(id: Uuid) -> Admin {
    Admin {
        id,
        os_user: DeclaredText::lossy(PANEL_ACTOR),
    }
}
