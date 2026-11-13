//! `arauto-hub admin panel code`: the installation code of the panel.
//!
//! The panel has no password: it is entered with a passkey. The code is the way to register
//! one when there is none yet, or when the only one was lost. Whoever can run this command
//! on the machine of the hub, or read the log of its start, already holds the database and
//! the master key: the code is drawn, shown once, and opens one session that only reaches
//! the registration of a passkey (`docs/decisoes/0040-painel-dentro-do-hub.md`).

use arauto_protocol::message::DeclaredText;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::{AdminError, Session};
use crate::db::ResolvedOrg;
use crate::mark::Mark;
use crate::repo::panel::{INSTALLATION_VALIDITY_MS, IssuedEntry, PANEL_ACTOR};
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

    /// Issues an entry code of the panel. The one issued before stops being accepted.
    ///
    /// # Errors
    /// What the database or the generator of the system return.
    pub async fn panel_code(&self, now: i64) -> Result<IssuedEntry, AdminError> {
        Ok(repo::panel::issue_entry(&self.pool, self.organization, &self.admin, now).await?)
    }

    /// What the hub does when it starts with a panel: while no admin has a passkey, it
    /// issues an installation code, for the log of the start. `None` once a passkey exists:
    /// then a code only comes out of the command, asked for by a person.
    ///
    /// # Errors
    /// What the database or the generator of the system return.
    pub async fn installation_code(&self, now: i64) -> Result<Option<IssuedEntry>, AdminError> {
        if repo::panel::has_passkey(&self.pool, self.organization).await? {
            return Ok(None);
        }
        Ok(Some(
            repo::panel::issue_entry_for(
                &self.pool,
                self.organization,
                &self.admin,
                INSTALLATION_VALIDITY_MS,
                now,
            )
            .await?,
        ))
    }
}
