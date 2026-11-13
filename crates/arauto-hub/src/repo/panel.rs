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
//!
//! **How far a session reaches.** A session opened by a code only reaches the registration of
//! a passkey. It reaches the whole panel once a passkey is registered in it, and a session
//! opened by a passkey reaches it from the start. The code is the way in when there is no
//! passkey yet, or when the only one was lost: it never stands in the place of one.
//!
//! **The passkey.** The hub keeps what checks it: the public key and its counters, as the
//! library of the ceremonies encodes them. Nothing here can sign in the place of the person.

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

/// For how long the code the hub writes when it starts with no passkey is accepted: the time
/// to read it in the log and open the panel.
pub const INSTALLATION_VALIDITY_MS: i64 = 30 * MINUTE_MS;

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
    issue_entry_for(pool, organization, admin, ENTRY_VALIDITY_MS, now).await
}

/// [`issue_entry`], with the time the code is accepted for.
///
/// # Errors
/// As [`issue_entry`].
pub async fn issue_entry_for(
    pool: &PgPool,
    organization: ResolvedOrg,
    admin: &Admin,
    validity_ms: i64,
    now: i64,
) -> Result<IssuedEntry, RepoError> {
    let code = EntryCode::generate().map_err(|_| RepoError::Random)?;
    let id = Uuid::new_v4();
    let expires_at = now.saturating_add(validity_ms);
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
    /// The session only reaches the registration of a passkey.
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
    /// Whether it reaches the whole panel: a passkey was shown in it. Without that it only
    /// reaches the registration of a passkey.
    pub full: bool,
}

/// How the row of a session says it reaches the whole panel.
const REACH_FULL: &str = "full";

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
    let found: Option<(Uuid, Uuid, i64, i64, String)> = sqlx::query_as(
        "SELECT id, admin_id, created_at, expires_at, reach FROM panel_session \
         WHERE organization_id = $1 AND digest = $2 AND state = 'open'",
    )
    .bind(organization.id())
    .bind(digest.as_bytes().as_slice())
    .fetch_optional(pool)
    .await?;
    Ok(
        found.map(|(id, admin, created_at, expires_at, reach)| PanelSession {
            id,
            admin,
            created_at,
            expires_at,
            full: reach == REACH_FULL,
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

// ---------------------------------------------------------------------------- the passkeys

/// What a registration ceremony leaves to be kept, as bytes: this module does not read them.
#[derive(Clone, PartialEq, Eq)]
pub struct NewPasskey {
    /// What the authenticator calls the credential.
    pub credential_id: Vec<u8>,
    /// What the authenticator calls the user.
    pub user_handle: Vec<u8>,
    /// The public key and what never changes, as the library encodes it.
    pub static_state: Vec<u8>,
    /// The counters, as the library encodes them.
    pub dynamic_state: Vec<u8>,
}

/// A passkey as the database has it.
#[derive(Clone, PartialEq, Eq)]
pub struct StoredPasskey {
    /// The row.
    pub id: Uuid,
    /// The admin it signs in.
    pub admin: Uuid,
    /// What the authenticator calls the credential.
    pub credential_id: Vec<u8>,
    /// What the authenticator calls the user.
    pub user_handle: Vec<u8>,
    /// The public key and what never changes.
    pub static_state: Vec<u8>,
    /// The counters, as of the last use.
    pub dynamic_state: Vec<u8>,
}

/// Whether any admin of the organization has a passkey. While none has, the hub is waiting
/// for its first access.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn has_passkey(pool: &PgPool, organization: ResolvedOrg) -> Result<bool, RepoError> {
    let found: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM panel_passkey \
                        WHERE organization_id = $1 AND state = 'active')",
    )
    .bind(organization.id())
    .fetch_one(pool)
    .await?;
    Ok(found)
}

/// The credentials of the active passkeys of an admin: what a new registration must not
/// repeat on the same authenticator.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn passkey_credentials(
    pool: &PgPool,
    organization: ResolvedOrg,
    admin: Uuid,
) -> Result<Vec<Vec<u8>>, RepoError> {
    let found: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT credential_id FROM panel_passkey \
         WHERE organization_id = $1 AND admin_id = $2 AND state = 'active' ORDER BY created_at",
    )
    .bind(organization.id())
    .bind(admin)
    .fetch_all(pool)
    .await?;
    Ok(found.into_iter().map(|(credential,)| credential).collect())
}

/// A row of `panel_passkey`, in the order [`passkey`] reads it.
type PasskeyRow = (Uuid, Uuid, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

/// The active passkey with this credential.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn passkey(
    pool: &PgPool,
    organization: ResolvedOrg,
    credential_id: &[u8],
) -> Result<Option<StoredPasskey>, RepoError> {
    let found: Option<PasskeyRow> = sqlx::query_as(
        "SELECT id, admin_id, credential_id, user_handle, static_state, dynamic_state \
         FROM panel_passkey \
         WHERE organization_id = $1 AND credential_id = $2 AND state = 'active'",
    )
    .bind(organization.id())
    .bind(credential_id)
    .fetch_optional(pool)
    .await?;
    Ok(found.map(
        |(id, admin, credential_id, user_handle, static_state, dynamic_state)| StoredPasskey {
            id,
            admin,
            credential_id,
            user_handle,
            static_state,
            dynamic_state,
        },
    ))
}

/// The outcome of [`register_passkey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registered {
    /// The passkey is kept, and the session it was registered in reaches the whole panel.
    Kept {
        /// The row of the passkey.
        id: Uuid,
    },
    /// The session is no longer open. Nothing was written.
    NoSession,
    /// The hub already has that credential. Nothing was written.
    Repeated,
}

/// Keeps a passkey registered in a session of the panel, and lets that session reach the
/// whole panel: the person has just shown the passkey in it. One write.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark (nothing is written);
/// [`RepoError::Audit`] with `MarkNotAdvanced` when the passkey **is** kept and the mark did
/// not follow.
pub async fn register_passkey(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    session: PanelSession,
    new: &NewPasskey,
    address: Option<IpAddr>,
    now: i64,
) -> Result<Registered, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let open: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM panel_session \
         WHERE organization_id = $1 AND id = $2 AND state = 'open' AND expires_at > $3 FOR UPDATE",
    )
    .bind(organization.id())
    .bind(session.id)
    .bind(now)
    .fetch_optional(write.connection())
    .await?;
    if open.is_none() {
        write.rollback().await?;
        return Ok(Registered::NoSession);
    }
    let id = Uuid::new_v4();
    let kept = sqlx::query(
        "INSERT INTO panel_passkey (id, organization_id, admin_id, credential_id, user_handle, \
                                    static_state, dynamic_state, state, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', $8) \
         ON CONFLICT (credential_id) DO NOTHING",
    )
    .bind(id)
    .bind(organization.id())
    .bind(session.admin)
    .bind(new.credential_id.as_slice())
    .bind(new.user_handle.as_slice())
    .bind(new.static_state.as_slice())
    .bind(new.dynamic_state.as_slice())
    .bind(now)
    .execute(write.connection())
    .await?
    .rows_affected();
    if kept == 0 {
        write.rollback().await?;
        return Ok(Registered::Repeated);
    }
    sqlx::query("UPDATE panel_session SET reach = 'full' WHERE organization_id = $1 AND id = $2")
        .bind(organization.id())
        .bind(session.id)
        .execute(write.connection())
        .await?;
    let mut event = Event::new(
        Action::PanelPasskeyRegistered,
        panel_admin(session.admin).actor(),
    )
    .in_organization(organization)
    .about(TargetKind::Admin, session.admin)
    .with(Details::new().id("passkey", id).id("session", session.id));
    if let Some(address) = address {
        event = event.from_address(address);
    }
    write.record(now, &event).await?;
    write.commit().await?;
    Ok(Registered::Kept { id })
}

/// Opens a session of the panel for a passkey that was just checked, and keeps the counters
/// the check left. The session reaches the whole panel. One write.
///
/// Returns `None`, with nothing written, when the passkey is no longer active.
///
/// # Errors
/// As [`sign_in`].
pub async fn sign_in_with_passkey(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    passkey: &StoredPasskey,
    dynamic_state: Option<&[u8]>,
    address: Option<IpAddr>,
    now: i64,
) -> Result<Option<OpenedSession>, RepoError> {
    let secret = SessionSecret::generate().map_err(|_| RepoError::Random)?;
    let mut write = audit::begin_security(pool, mark).await?;
    let used = sqlx::query(
        "UPDATE panel_passkey SET last_used_at = $3, dynamic_state = COALESCE($4, dynamic_state) \
         WHERE organization_id = $1 AND id = $2 AND state = 'active'",
    )
    .bind(organization.id())
    .bind(passkey.id)
    .bind(now)
    .bind(dynamic_state)
    .execute(write.connection())
    .await?
    .rows_affected();
    if used == 0 {
        write.rollback().await?;
        return Ok(None);
    }
    close_expired(write.connection(), organization, now).await?;

    let id = Uuid::new_v4();
    let expires_at = now.saturating_add(SESSION_VALIDITY_MS);
    sqlx::query(
        "INSERT INTO panel_session (id, organization_id, admin_id, digest, state, opened_from, \
                                    created_at, expires_at, reach) \
         VALUES ($1, $2, $3, $4, 'open', $5, $6, $7, 'full')",
    )
    .bind(id)
    .bind(organization.id())
    .bind(passkey.admin)
    .bind(secret.digest().as_bytes().as_slice())
    .bind(address.map(|address| address.to_string()))
    .bind(now)
    .bind(expires_at)
    .execute(write.connection())
    .await?;
    let mut event = Event::new(Action::PanelSignedIn, panel_admin(passkey.admin).actor())
        .in_organization(organization)
        .about(TargetKind::Admin, passkey.admin)
        .with(
            Details::new()
                .id("session", id)
                .id("passkey", passkey.id)
                .number("expires_at", expires_at),
        );
    if let Some(address) = address {
        event = event.from_address(address);
    }
    write.record(now, &event).await?;
    write.commit().await?;
    Ok(Some(OpenedSession {
        secret,
        id,
        expires_at,
    }))
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
