//! `arauto-hub admin panel code`: the entry code of the panel.
//!
//! The panel has no password. Whoever can run this command on the machine of the hub
//! already holds the database and the master key, and is who the panel lets in: the command
//! draws a code of one use, shows it once, and the panel opens one session for whoever
//! types it within 10 minutes (`docs/decisoes/0040-painel-dentro-do-hub.md`).

use arauto_protocol::message::DeclaredText;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::{AdminError, Session};
use crate::db::ResolvedOrg;
use crate::mark::Mark;
use crate::repo::panel::{IssuedEntry, PANEL_ACTOR};
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
}
