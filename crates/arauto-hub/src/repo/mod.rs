//! The repositories: every read and write of business data.
//!
//! Three rules hold for every function here (`docs/15-mvp.md`, section 2):
//!
//! 1. **The organization comes resolved.** Every `async` function of this module takes a
//!    [`ResolvedOrg`], and every statement filters by it. The organization is found from the
//!    outside only by the entry functions of [`crate::db`]: by the identity of a device, by
//!    the digest of an invite, or by a named task of the hub itself. The functions here
//!    without the parameter are [`enroll::flush_refused`], which writes what was refused to
//!    whoever belongs to no organization, and [`purge::retention`], which reads the days of
//!    each table, the same for every organization. A test reads these files and fails when
//!    another function shows up without it.
//! 2. **A write that changes a device reads it again first.** It locks the row of the Hermes
//!    and then the rows of its devices (`FOR NO KEY UPDATE`, always in that order), reads
//!    them under the lock and only then decides and writes. No transaction stays open
//!    across a wait for the network. The invite of a Hermes only changes under that same
//!    lock, and so does the record of the connections of its devices: after the devices,
//!    the row of the connection.
//! 3. **A change goes in with its events**, through [`crate::audit::begin_security`]: the
//!    database is checked against the audit mark before anything is written, and a database
//!    that came back from an older copy gets nothing ([`RepoError::Restored`]).
//!
//! | Module | What it holds |
//! |---|---|
//! | [`hermes`] | The lines of Hermes: create, list, remove, "can be woken". |
//! | [`invite`] | Issuing an invite, which also closes the request in progress. |
//! | [`enroll`] | The enrollment: every line of the table of C3. |
//! | [`approval`] | Approving with the typed code, and denying. |
//! | [`swap`] | T1 and T2 of the credential swap, the presentation without swap, quarantine. |
//! | [`connection`] | The record of channel connections and the swaps left unconfirmed. |
//! | [`closing`] | Revoking a Hermes and the request of a device to leave. |
//! | [`sweep`] | The deadlines: invite, pending, approved, inactivity. |
//! | [`purge`] | What passed its retention, deleted in small batches. |
//! | [`alert`] | What the panel would show the admin. |
//! | [`audit_log`] | Reading the audit. |
//! | [`device`] | What a connection needs to know about its device. |
//! | [`engine`] | The engine of the organization: its addresses and its sealed secrets. |
//! | [`number`] | The numbers of WhatsApp and their states. |
//! | [`chat`] | The chats of a number, by their identifiers in the engine. |
//! | [`engine_event`] | The inbox of events of the engine. |
//! | [`access`] | The matrix of accesses: what each Hermes may do on each number and in each chat. |

use arauto_protocol::message::DeclaredText;
use sqlx::postgres::{PgConnection, PgPool};
use uuid::Uuid;

use crate::audit::{Actor, AuditError};
use crate::core::judge::TransitionError;
use crate::core::model::{Device, Hermes};
use crate::db::{DbError, ResolvedOrg};
use crate::restore::{Evidence, RestoreError};
use crate::vault::VaultError;

pub mod access;
pub mod alert;
pub mod approval;
pub mod audit_log;
pub mod chat;
pub mod closing;
pub mod connection;
pub mod device;
pub mod engine;
pub mod engine_event;
pub mod enroll;
pub mod hermes;
pub mod invite;
pub mod number;
pub mod purge;
pub mod swap;
pub mod sweep;

/// The channel of PostgreSQL on which the admin command tells the hub that something changed.
/// The text is short, `kind:id`. It is a hint: the sweep of the hub is what guarantees.
pub const ADMIN_CHANNEL: &str = "arauto_admin";

/// What went wrong in a repository function.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    /// The database refused.
    #[error(transparent)]
    Database(#[from] DbError),

    /// The database came back from an older copy. **Nothing was written.** The hub turns the
    /// restored mode on ([`crate::restore::enter`]) with this evidence.
    #[error("the database came back from an older copy ({0}); nothing was written")]
    Restored(Evidence),

    /// The security write failed in another way. With
    /// [`AuditError::MarkNotAdvanced`] the change **is committed**.
    #[error(transparent)]
    Audit(AuditError),

    /// The vault refused: the generator of the system failed, or a stored key does not open.
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// The generator of the system failed.
    #[error("the random generator of the system failed")]
    Random,

    /// The swap rule refused a transition that the judgment had accepted.
    #[error("the credential swap could not be written: {0}")]
    Transition(TransitionError),

    /// A row does not say what the program wrote in it.
    #[error("a stored row is inconsistent: {0}")]
    Inconsistent(&'static str),
}

impl From<AuditError> for RepoError {
    fn from(error: AuditError) -> Self {
        match error {
            AuditError::Restored(evidence) => RepoError::Restored(evidence),
            AuditError::Database(error) => RepoError::Database(error),
            other => RepoError::Audit(other),
        }
    }
}

impl From<TransitionError> for RepoError {
    fn from(error: TransitionError) -> Self {
        RepoError::Transition(error)
    }
}

impl From<RestoreError> for RepoError {
    fn from(error: RestoreError) -> Self {
        match error {
            RestoreError::Database(error) => RepoError::Database(error),
            RestoreError::Mark(error) => RepoError::Audit(AuditError::Mark(error)),
            RestoreError::Audit(error) => error.into(),
            RestoreError::NotInstalled => RepoError::Audit(AuditError::NotInstalled),
            RestoreError::NoSuchDevice => {
                RepoError::Inconsistent("the device of the evidence is not in the database")
            }
            RestoreError::MarkNotAdvanced { seq, cause } => {
                RepoError::Audit(AuditError::MarkNotAdvanced { seq, cause })
            }
        }
    }
}

impl From<sqlx::Error> for RepoError {
    fn from(error: sqlx::Error) -> Self {
        RepoError::Database(DbError::Query(error))
    }
}

/// The local admin: the person running the admin command on the machine of the hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admin {
    /// The row of the admin.
    pub id: Uuid,
    /// The system user that ran the command. Declared, not verified.
    pub os_user: DeclaredText,
}

impl Admin {
    /// The actor that goes into the audit.
    #[must_use]
    pub fn actor(&self) -> Actor {
        Actor::Admin {
            id: self.id,
            os_user: self.os_user.clone(),
        }
    }
}

/// The row of the local admin of `organization`, with the system user that is running the
/// command as its declared text. `None` before `init`.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn local_admin(
    pool: &PgPool,
    organization: ResolvedOrg,
    os_user: DeclaredText,
) -> Result<Option<Admin>, RepoError> {
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM admin WHERE organization_id = $1 AND kind = 'local' \
         ORDER BY created_at, id LIMIT 1",
    )
    .bind(organization.id())
    .fetch_optional(pool)
    .await?;
    Ok(id.map(|id| Admin { id, os_user }))
}

/// A Hermes and its devices, read under their locks.
#[derive(Debug)]
pub(crate) struct Locked {
    pub(crate) hermes: Hermes,
    pub(crate) devices: Vec<Device>,
}

impl Locked {
    /// The device in use: the one that already confirmed a credential.
    pub(crate) fn in_use(&self) -> Option<&Device> {
        use crate::core::model::Slot;
        self.devices
            .iter()
            .find(|device| device.slot == Some(Slot::InUse))
    }

    /// The request in progress: the one that has not confirmed a credential yet.
    pub(crate) fn request(&self) -> Option<&Device> {
        use crate::core::model::Slot;
        self.devices
            .iter()
            .find(|device| device.slot == Some(Slot::Request))
    }

    pub(crate) fn device(&self, id: Uuid) -> Option<&Device> {
        self.devices.iter().find(|device| device.id == id)
    }
}

/// Locks the row of a Hermes and then the rows of its devices, and reads them. The order of
/// every write that changes a device (`docs/14-protocolo-do-canal.md`, section 6).
pub(crate) async fn lock_hermes(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    hermes: Uuid,
) -> Result<Option<Locked>, sqlx::Error> {
    let line: Option<Hermes> = sqlx::query_as(
        "SELECT * FROM hermes WHERE organization_id = $1 AND id = $2 FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_optional(&mut *connection)
    .await?;
    let Some(line) = line else {
        return Ok(None);
    };
    let devices: Vec<Device> = sqlx::query_as(
        "SELECT * FROM device WHERE organization_id = $1 AND hermes_id = $2 \
         ORDER BY id FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .bind(hermes)
    .fetch_all(&mut *connection)
    .await?;
    Ok(Some(Locked {
        hermes: line,
        devices,
    }))
}

/// [`lock_hermes`] for the Hermes of a device. The Hermes of a device never changes, so
/// reading it before the lock is safe.
pub(crate) async fn lock_hermes_of_device(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    device: Uuid,
) -> Result<Option<Locked>, sqlx::Error> {
    let hermes: Option<Uuid> =
        sqlx::query_scalar("SELECT hermes_id FROM device WHERE organization_id = $1 AND id = $2")
            .bind(organization.id())
            .bind(device)
            .fetch_optional(&mut *connection)
            .await?;
    match hermes {
        Some(hermes) => lock_hermes(connection, organization, hermes).await,
        None => Ok(None),
    }
}

/// [`lock_hermes`] for the Hermes with this name that was not removed.
pub(crate) async fn lock_hermes_by_name(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    name: &arauto_protocol::names::HermesName,
) -> Result<Option<Locked>, sqlx::Error> {
    let hermes: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM hermes WHERE organization_id = $1 AND name = $2 AND deleted_at IS NULL",
    )
    .bind(organization.id())
    .bind(name.as_str())
    .fetch_optional(&mut *connection)
    .await?;
    let Some(hermes) = hermes else {
        return Ok(None);
    };
    // Read again under the lock: the line may have been removed meanwhile.
    Ok(lock_hermes(connection, organization, hermes)
        .await?
        .filter(|locked| locked.hermes.deleted_at.is_none()))
}

/// Tells the hub, when the transaction commits, that the admin command changed something.
pub(crate) async fn notify_hub(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
    kind: &'static str,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    // The organization is not part of the text: the hub reads the row again by its id.
    let _ = organization;
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(ADMIN_CHANNEL)
        .bind(format!("{kind}:{}", id.as_hyphenated()))
        .execute(connection)
        .await?;
    Ok(())
}

/// A stored address, back as an address. A text that is not one is dropped.
pub(crate) fn stored_address(text: Option<&str>) -> Option<std::net::IpAddr> {
    text.and_then(|text| text.parse().ok())
}
