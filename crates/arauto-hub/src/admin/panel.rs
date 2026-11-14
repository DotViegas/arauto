//! `arauto-hub admin panel code`: the installation code of the panel. And
//! `arauto-hub admin database rotate-password`, which the panel does by itself after the first
//! passkey when the configuration asks for it.
//!
//! The panel has no password: it is entered with a passkey. The code is the way to register
//! one when there is none yet, or when the only one was lost. Whoever can run this command
//! on the machine of the hub, or read the log of its start, already holds the database and
//! the master key: the code is drawn and shown once. Typed, it opens one session that only
//! reaches the registration of a passkey, and that registration is what spends the code
//! (`docs/decisoes/0040-painel-dentro-do-hub.md`).

use std::path::PathBuf;

use arauto_protocol::message::DeclaredText;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::{AdminError, Session};
use crate::audit::AuditError;
use crate::config::StorageConfig;
use crate::db::{self, ResolvedOrg, RotationError, When};
use crate::db_password::DATABASE_SECRET_FILE;
use crate::mark::Mark;
use crate::repo::panel::{IssuedEntry, IssuedStart, PANEL_ACTOR};
use crate::repo::{self, Admin};

impl Session {
    /// The session of an admin who came in through the panel: the functions of the admin
    /// command, with the same checks, the same audit and the same mark. Where the command
    /// writes the system user that ran it, the audit gets the fixed word of the panel.
    ///
    /// `admin` is the admin of an open session of the panel of `organization`: whoever
    /// calls this has just read that session from the database.
    #[must_use]
    pub fn of_panel(pool: PgPool, mark: Mark, organization: ResolvedOrg, admin: Uuid) -> Self {
        Self {
            pool,
            mark,
            organization,
            admin: Admin {
                id: admin,
                os_user: DeclaredText::lossy(PANEL_ACTOR),
            },
        }
    }

    /// Issues a command code of the panel (`arauto-hub admin panel code`): valid for 10
    /// minutes to be typed, in every state of the hub. The unspent code, of either kind,
    /// stops being accepted.
    ///
    /// # Errors
    /// What the database or the generator of the system return.
    pub async fn panel_code(&self, now: i64) -> Result<IssuedEntry, AdminError> {
        Ok(
            repo::panel::issue_command_code(&self.pool, self.organization, &self.admin, now)
                .await?,
        )
    }

    /// `admin database rotate-password`: replaces the password of the database role with one
    /// the hub draws, which nobody sees, and keeps it in the data folder
    /// ([`db::rotate_password`]). Returns the file it is kept in.
    ///
    /// # Errors
    /// [`AdminError::Restored`] when the database came back from an older copy (nothing was
    /// done); [`AdminError::DatabasePassword`] otherwise, whose text says whether the
    /// password was replaced.
    pub async fn rotate_database_password(
        &self,
        storage: &StorageConfig,
        now: i64,
    ) -> Result<PathBuf, AdminError> {
        db::rotate_password(
            storage,
            &[&self.pool],
            &self.mark,
            self.admin.actor(),
            When::Asked,
            now,
        )
        .await
        .map_err(|error| match error {
            RotationError::Audit(AuditError::Restored(evidence)) => AdminError::Restored(evidence),
            other => AdminError::DatabasePassword(other),
        })?;
        Ok(storage.data_dir.join(DATABASE_SECRET_FILE))
    }

    /// What the hub does when it starts with a panel: while the first access was never done,
    /// it issues a start code, with no deadline, for the log of the start. `None` once the
    /// organization has kept its first passkey: the durable mark stays even if every passkey
    /// is removed or lost later, and then a code only comes out of the command, asked for by a
    /// person.
    ///
    /// The mark is read in the same write that would issue the code
    /// ([`repo::panel::issue_start_code`]), so a first passkey kept at the same moment by
    /// another hub never leaves a start code behind it.
    ///
    /// # Errors
    /// What the database or the generator of the system return.
    pub async fn installation_code(&self, now: i64) -> Result<Option<IssuedStart>, AdminError> {
        Ok(repo::panel::issue_start_code(&self.pool, self.organization, &self.admin, now).await?)
    }
}
