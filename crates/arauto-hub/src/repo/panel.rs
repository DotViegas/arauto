//! The entry and the sessions of the panel of the admin
//! (`docs/decisoes/0040-painel-dentro-do-hub.md`).
//!
//! **The installation code.** It has two kinds. The **start code** is the one the hub writes
//! in its own log when it starts before the first access: it has no deadline, and an unspent
//! one is never purged. The **command code** is the one `arauto-hub admin panel code` issues,
//! in any state of the hub: it is valid for 10 minutes to be typed. The database enforces that
//! a code has a deadline if, and only if, it is a command code. Whoever can run the admin
//! command or read the log of the hub already holds the database and the master key: the code
//! opens no way that was closed.
//!
//! **One code at a time.** Issuing a code of either kind cancels the unspent one in the same
//! write, says which kind cancelled it, and closes the sessions of the cancelled code that
//! never reached the whole panel. The database keeps only the SHA-256 of a code (with its
//! domain prefix) and erases it when the code is spent or cancelled.
//!
//! **Typing is not spending.** Typing a code opens a session that reaches only the
//! registration of a passkey, and points at its code. The code is spent by registering a
//! passkey in that session, in the same write that keeps the passkey, under the lock of the
//! row of the code. Typing the same code again closes the session it opened before, so a
//! person who lost the page (a closed browser, 30 minutes idle, a restarted hub) needs only
//! the latest code.
//!
//! **The first access.** The organization carries a durable mark of when its first passkey
//! was kept, written in that same write and never cleared: losing every passkey later does
//! not bring the start code back, the command is the way back.
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
use crate::audit::{self, Action, AuditError, Details, Event, TargetKind};
use crate::db::ResolvedOrg;
use crate::mark::Mark;
use crate::panel::secret::{EntryCode, PanelDigest, SessionSecret};

/// For how long a command code is accepted when it is typed.
pub const ENTRY_VALIDITY_MS: i64 = 10 * MINUTE_MS;

/// For how long a session of the panel lasts, used or not.
pub const SESSION_VALIDITY_MS: i64 = 12 * HOUR_MS;

/// For how long a session of the panel may stay without a request.
pub const SESSION_IDLE_MS: i64 = 30 * MINUTE_MS;

/// What the audit says ran an action done through the panel, in the place of the system
/// user of the admin command.
pub const PANEL_ACTOR: &str = "panel";

/// Which kind of installation code a row is, and which kind cancelled another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// The one the hub writes in its log at its start: no deadline.
    Start,
    /// The one the admin command issues: valid for [`ENTRY_VALIDITY_MS`].
    Command,
}

impl EntryKind {
    /// How the column says it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Command => "command",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "start" => Some(Self::Start),
            "command" => Some(Self::Command),
            _ => None,
        }
    }
}

/// A command code that was just issued.
#[derive(Debug)]
pub struct IssuedEntry {
    /// The code itself. This is the only moment it exists outside the head of the admin:
    /// show it once and drop it.
    pub code: EntryCode,
    /// The row that holds its digest.
    pub id: Uuid,
    /// When it stops being accepted when typed.
    pub expires_at: i64,
}

/// A start code that was just issued. It has no deadline.
#[derive(Debug)]
pub struct IssuedStart {
    /// The code itself. Its one holder is the task of the first access.
    pub code: EntryCode,
    /// The row that holds its digest.
    pub id: Uuid,
}

/// Issues a command code, valid for [`ENTRY_VALIDITY_MS`], in any state of the hub. The code
/// that was unspent stops being accepted.
///
/// # Errors
/// [`RepoError::Random`] when the generator of the system fails; [`RepoError::Database`].
pub async fn issue_command_code(
    pool: &PgPool,
    organization: ResolvedOrg,
    admin: &Admin,
    now: i64,
) -> Result<IssuedEntry, RepoError> {
    let expires_at = now.saturating_add(ENTRY_VALIDITY_MS);
    let issued = issue(
        pool,
        organization,
        admin,
        EntryKind::Command,
        Some(expires_at),
        now,
    )
    .await?;
    // Only a start code asks about the first access: a command code is issued in every state.
    let Some((code, id)) = issued else {
        return Err(RepoError::Inconsistent(
            "a command code was refused as a start code",
        ));
    };
    Ok(IssuedEntry {
        code,
        id,
        expires_at,
    })
}

/// Issues a start code, with no deadline, while the first access of the organization was not
/// done. The code that was unspent stops being accepted.
///
/// Returns `None`, with no code issued, when the first access was done: the mark is set, or a
/// passkey exists with the mark not set (it is set from the oldest passkey, in the same write).
/// That is read in the same write that issues the code, after the row of the unspent code is
/// locked and under a lock of the row of the organization: a first passkey kept by another hub
/// at the same moment either comes before, and nothing is issued, or waits for this write.
/// When nothing is issued, a command code that is unspent stays accepted; a start code left
/// from before is cancelled.
/// Two versions of the hub must not serve the same database at once
/// (`docs/decisoes/0040-painel-dentro-do-hub.md`).
///
/// # Errors
/// As [`issue_command_code`].
pub async fn issue_start_code(
    pool: &PgPool,
    organization: ResolvedOrg,
    admin: &Admin,
    now: i64,
) -> Result<Option<IssuedStart>, RepoError> {
    let issued = issue(pool, organization, admin, EntryKind::Start, None, now).await?;
    Ok(issued.map(|(code, id)| IssuedStart { code, id }))
}

async fn issue(
    pool: &PgPool,
    organization: ResolvedOrg,
    admin: &Admin,
    kind: EntryKind,
    expires_at: Option<i64>,
    now: i64,
) -> Result<Option<(EntryCode, Uuid)>, RepoError> {
    let code = EntryCode::generate().map_err(|_| RepoError::Random)?;
    let id = Uuid::new_v4();
    let mut transaction = pool.begin().await?;
    // Two issuers at once: the second waits for the first, instead of failing on the index
    // that allows one unspent code.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 15))")
        .bind(organization.id().to_string())
        .execute(&mut *transaction)
        .await?;
    // The unspent code, locked before the first access is read: a first passkey being kept
    // with it holds this row, and this write waits for it.
    let unspent: Vec<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, kind = 'start' FROM panel_entry \
         WHERE organization_id = $1 AND state = 'issued' FOR UPDATE",
    )
    .bind(organization.id())
    .fetch_all(&mut *transaction)
    .await?;
    if kind == EntryKind::Start {
        // The first access, read here and not before: the code row was locked above, and the
        // organization row is locked now, in the order every write that touches both takes
        // them (`register_passkey`).
        // A passkey of the organization, in any state, also says the first access was done:
        // an old version of the hub may have kept one without the mark. Then the mark is set
        // from the oldest of them, here, and no code is issued.
        let (marked, oldest): (bool, Option<i64>) = sqlx::query_as(
            "SELECT panel_first_passkey_at IS NOT NULL, \
                    (SELECT min(created_at) FROM panel_passkey WHERE organization_id = $1) \
             FROM organization WHERE id = $1 FOR UPDATE",
        )
        .bind(organization.id())
        .fetch_one(&mut *transaction)
        .await?;
        if marked {
            transaction.rollback().await?;
            return Ok(None);
        }
        if let Some(oldest) = oldest {
            sqlx::query(
                "UPDATE organization SET panel_first_passkey_at = $2 \
                 WHERE id = $1 AND panel_first_passkey_at IS NULL",
            )
            .bind(organization.id())
            .bind(oldest)
            .execute(&mut *transaction)
            .await?;
            // A start code left from before is dead with the first access done; a command
            // code someone issued is not replaced by a start that issues nothing.
            let starts: Vec<Uuid> = unspent
                .into_iter()
                .filter_map(|(id, start)| start.then_some(id))
                .collect();
            cancel(&mut transaction, organization, starts, kind, now).await?;
            transaction.commit().await?;
            return Ok(None);
        }
    }
    let replaced: Vec<Uuid> = unspent.into_iter().map(|(id, _)| id).collect();
    cancel(&mut transaction, organization, replaced, kind, now).await?;
    sqlx::query(
        "INSERT INTO panel_entry (id, organization_id, digest, state, kind, created_by, \
                                  created_at, expires_at) \
         VALUES ($1, $2, $3, 'issued', $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(code.digest().as_bytes().as_slice())
    .bind(kind.as_str())
    .bind(admin.id)
    .bind(now)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await?;
    let mut details = Details::new().word("kind", kind.as_str());
    if let Some(expires_at) = expires_at {
        details = details.number("expires_at", expires_at);
    }
    audit::record(
        &mut *transaction,
        now,
        &Event::new(Action::PanelCodeIssued, admin.actor())
            .in_organization(organization)
            .about(TargetKind::Admin, admin.id)
            .with(details),
    )
    .await?;
    transaction.commit().await?;
    Ok(Some((code, id)))
}

/// Cancels the codes, locked by the caller, in favour of a code of the kind `by`, and closes
/// the panel sessions they opened that never reached the whole panel.
async fn cancel(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization: ResolvedOrg,
    codes: Vec<Uuid>,
    by: EntryKind,
    now: i64,
) -> Result<(), RepoError> {
    if codes.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE panel_entry \
         SET state = 'cancelled', digest = NULL, cancelled_at = $2, cancelled_by = $3 \
         WHERE organization_id = $1 AND state = 'issued' AND id = ANY($4)",
    )
    .bind(organization.id())
    .bind(now)
    .bind(by.as_str())
    .bind(&codes)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE panel_session \
         SET state = 'closed', digest = NULL, closed_at = $2, close_reason = $4 \
         WHERE organization_id = $1 AND state = 'open' AND reach = 'setup' \
           AND entry_id = ANY($3)",
    )
    .bind(organization.id())
    .bind(now)
    .bind(codes)
    .bind(CODE_REPLACED)
    .execute(&mut **transaction)
    .await?;
    Ok(())
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
    /// The code is the one issued and, for a command code, was in its minutes: a session is
    /// open. It only reaches the registration of a passkey, and the code is **not** spent.
    Opened(OpenedSession),
    /// The code is not the one issued, or its minutes passed. Nothing was written, and the
    /// answer does not say which.
    Refused,
}

/// Opens a session of the panel for a code that was typed. The code is not spent: the
/// registration of a passkey in that session spends it ([`register_passkey`]). A session this
/// same code opened before is closed: one session per unspent code.
///
/// The row of the code is read under its lock.
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
    let found: Option<(Uuid, Uuid, Option<i64>)> = sqlx::query_as(
        "SELECT id, created_by, expires_at FROM panel_entry \
         WHERE organization_id = $1 AND digest = $2 AND state = 'issued' FOR UPDATE",
    )
    .bind(organization.id())
    .bind(entry.as_bytes().as_slice())
    .fetch_optional(write.connection())
    .await?;
    // A deadline passed when the time of the hub reached it. A start code has none.
    let Some((entry_id, admin, _)) =
        found.filter(|(_, _, expires_at)| expires_at.is_none_or(|deadline| deadline > now))
    else {
        write.rollback().await?;
        return Ok(SignIn::Refused);
    };
    sqlx::query(
        "UPDATE panel_session \
         SET state = 'closed', digest = NULL, closed_at = $3, close_reason = 'code_retyped' \
         WHERE organization_id = $1 AND entry_id = $2 AND state = 'open'",
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
                                    created_at, expires_at, entry_id) \
         VALUES ($1, $2, $3, $4, 'open', $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(admin)
    .bind(secret.digest().as_bytes().as_slice())
    .bind(address.map(|address| address.to_string()))
    .bind(now)
    .bind(expires_at)
    .bind(entry_id)
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

/// Whether the first access of the organization was done: its first passkey was kept. The
/// mark is durable and is never cleared, so removing or losing every passkey later does not
/// make it false. Only reads.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn first_access_done(
    pool: &PgPool,
    organization: ResolvedOrg,
) -> Result<bool, RepoError> {
    let done: bool = sqlx::query_scalar(
        "SELECT panel_first_passkey_at IS NOT NULL FROM organization WHERE id = $1",
    )
    .bind(organization.id())
    .fetch_one(pool)
    .await?;
    Ok(done)
}

/// What became of a code that was issued, as the task of the first access sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStanding {
    /// Still unspent.
    Issued,
    /// Spent by the registration of a passkey.
    Used,
    /// Cancelled by the issue of a code of this kind.
    Cancelled(EntryKind),
    /// The row is not there.
    Missing,
}

/// The first access and one code, in one read: whether the mark of the first access is set,
/// and what became of the code `entry`. Only reads.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn first_access_standing(
    pool: &PgPool,
    organization: ResolvedOrg,
    entry: Uuid,
) -> Result<(bool, EntryStanding), RepoError> {
    let found: Option<(bool, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT o.panel_first_passkey_at IS NOT NULL, e.state, e.cancelled_by \
         FROM organization o \
         LEFT JOIN panel_entry e ON e.id = $2 AND e.organization_id = o.id \
         WHERE o.id = $1",
    )
    .bind(organization.id())
    .bind(entry)
    .fetch_optional(pool)
    .await?;
    let Some((done, state, cancelled_by)) = found else {
        return Ok((false, EntryStanding::Missing));
    };
    let standing = match state.as_deref() {
        Some("issued") => EntryStanding::Issued,
        Some("used") => EntryStanding::Used,
        Some("cancelled") => cancelled_by
            .as_deref()
            .and_then(EntryKind::parse)
            .map_or(EntryStanding::Missing, EntryStanding::Cancelled),
        _ => EntryStanding::Missing,
    };
    Ok((done, standing))
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
#[derive(Debug)]
pub enum Registered {
    /// The passkey is kept, and the session it was registered in reaches the whole panel.
    Kept {
        /// The row of the passkey.
        id: Uuid,
        /// Whether this is the first passkey of the organization: the mark of the first
        /// access was written in the same write.
        first: bool,
    },
    /// The passkey **is** kept, as with [`Registered::Kept`], and the audit mark did not
    /// follow the write: `error` is the [`AuditError::MarkNotAdvanced`] the hub tells the
    /// admin about, as for every security write. It is an outcome and not an error so that
    /// whoever registered still learns whether this was the first passkey.
    KeptMarkBehind {
        /// The row of the passkey.
        id: Uuid,
        /// Whether this is the first passkey of the organization.
        first: bool,
        /// What did not follow.
        error: RepoError,
    },
    /// The session is no longer open, or it was opened by a code that is no longer
    /// unspent (that session is closed). Nothing was kept.
    NoSession,
    /// The hub already has that credential. Nothing was written.
    Repeated,
}

/// Keeps a passkey registered in a session of the panel, and lets that session reach the
/// whole panel: the person has just shown the passkey in it. One write.
///
/// A session opened by a code spends that code here, in the same write and under the lock of
/// its row, and only while the code is still unspent: otherwise nothing is kept and the
/// session is closed. A session that already reaches the whole panel depends on no code. The
/// first passkey of the organization also writes the durable mark of the first access.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark (nothing is written).
/// When the passkey is kept and the mark did not follow, the outcome is
/// [`Registered::KeptMarkBehind`], not an error.
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
    // The code before the session, as every write that touches both does (`issue`).
    let known: Option<(Option<Uuid>, String)> = sqlx::query_as(
        "SELECT entry_id, reach FROM panel_session \
         WHERE organization_id = $1 AND id = $2 AND state = 'open' AND expires_at > $3",
    )
    .bind(organization.id())
    .bind(session.id)
    .bind(now)
    .fetch_optional(write.connection())
    .await?;
    let Some((entry_id, reach)) = known else {
        write.rollback().await?;
        return Ok(Registered::NoSession);
    };
    let spends = if reach == REACH_FULL {
        None
    } else {
        // The row of the code, under its lock: an issue, a typing or another registration of
        // the same code waits here, or is waited for.
        let code: Option<String> = match entry_id {
            Some(entry_id) => {
                sqlx::query_scalar(
                    "SELECT state FROM panel_entry \
                     WHERE organization_id = $1 AND id = $2 FOR UPDATE",
                )
                .bind(organization.id())
                .bind(entry_id)
                .fetch_optional(write.connection())
                .await?
            }
            None => None,
        };
        match (entry_id, code.as_deref()) {
            (Some(entry_id), Some("issued")) => Some(entry_id),
            (_, code) => {
                // The code is no longer unspent. Another registration in this same session may
                // have spent it a moment ago: then the session reaches the whole panel, and
                // this passkey depends on no code.
                let now_reach: Option<String> = sqlx::query_scalar(
                    "SELECT reach FROM panel_session \
                     WHERE organization_id = $1 AND id = $2 AND state = 'open' \
                       AND expires_at > $3 FOR UPDATE",
                )
                .bind(organization.id())
                .bind(session.id)
                .bind(now)
                .fetch_optional(write.connection())
                .await?;
                match now_reach.as_deref() {
                    Some(REACH_FULL) => None,
                    Some(_) => {
                        write.rollback().await?;
                        let reason = if code == Some("cancelled") {
                            CODE_REPLACED
                        } else {
                            CODE_SPENT
                        };
                        close_session(pool, organization, session.id, reason, now).await?;
                        return Ok(Registered::NoSession);
                    }
                    None => {
                        write.rollback().await?;
                        return Ok(Registered::NoSession);
                    }
                }
            }
        }
    };
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
        // The code stays unspent: nothing was kept.
        write.rollback().await?;
        return Ok(Registered::Repeated);
    }
    if let Some(entry_id) = spends {
        // Still unspent, said again where it is written: the lock above is what makes this
        // true, and this keeps the write right should that lock ever be missing.
        let spent = sqlx::query(
            "UPDATE panel_entry SET state = 'used', digest = NULL, used_at = $3 \
             WHERE organization_id = $1 AND id = $2 AND state = 'issued'",
        )
        .bind(organization.id())
        .bind(entry_id)
        .bind(now)
        .execute(write.connection())
        .await?
        .rows_affected();
        if spent != 1 {
            write.rollback().await?;
            close_session(pool, organization, session.id, CODE_SPENT, now).await?;
            return Ok(Registered::NoSession);
        }
    }
    sqlx::query("UPDATE panel_session SET reach = 'full' WHERE organization_id = $1 AND id = $2")
        .bind(organization.id())
        .bind(session.id)
        .execute(write.connection())
        .await?;
    // The durable mark: written once, by the first passkey of the organization.
    let first = sqlx::query(
        "UPDATE organization SET panel_first_passkey_at = $2 \
         WHERE id = $1 AND panel_first_passkey_at IS NULL",
    )
    .bind(organization.id())
    .bind(now)
    .execute(write.connection())
    .await?
    .rows_affected()
        == 1;
    let mut event = Event::new(
        Action::PanelPasskeyRegistered,
        panel_admin(session.admin).actor(),
    )
    .in_organization(organization)
    .about(TargetKind::Admin, session.admin)
    .with(
        Details::new()
            .id("passkey", id)
            .id("session", session.id)
            .flag("first", first),
    );
    if let Some(address) = address {
        event = event.from_address(address);
    }
    write.record(now, &event).await?;
    match write.commit().await {
        Ok(_) => Ok(Registered::Kept { id, first }),
        Err(error @ AuditError::MarkNotAdvanced { .. }) => Ok(Registered::KeptMarkBehind {
            id,
            first,
            error: RepoError::from(error),
        }),
        Err(error) => Err(error.into()),
    }
}

/// How a session closed because another code replaced its own says it.
const CODE_REPLACED: &str = "code_replaced";

/// How a session closed because its code was spent elsewhere, or is gone, says it.
const CODE_SPENT: &str = "code_spent";

/// Closes a session whose code is no longer unspent: what the person typed no longer opens
/// anything. A session that reaches the whole panel is left open.
async fn close_session(
    pool: &PgPool,
    organization: ResolvedOrg,
    session: Uuid,
    reason: &str,
    now: i64,
) -> Result<(), RepoError> {
    sqlx::query(
        "UPDATE panel_session \
         SET state = 'closed', digest = NULL, closed_at = $3, close_reason = $4 \
         WHERE organization_id = $1 AND id = $2 AND state = 'open' AND reach = 'setup'",
    )
    .bind(organization.id())
    .bind(session)
    .bind(now)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(())
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
