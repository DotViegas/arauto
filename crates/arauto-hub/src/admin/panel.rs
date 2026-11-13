//! `arauto-hub admin panel code`: the entry code of the panel.
//!
//! The panel has no password. Whoever can run this command on the machine of the hub
//! already holds the database and the master key, and is who the panel lets in: the command
//! draws a code of one use, shows it once, and the panel opens one session for whoever
//! types it within 10 minutes (`docs/decisoes/0040-painel-dentro-do-hub.md`).

use super::{AdminError, Session};
use crate::repo;
use crate::repo::panel::IssuedEntry;

impl Session {
    /// Issues an entry code of the panel. The one issued before stops being accepted.
    ///
    /// # Errors
    /// What the database or the generator of the system return.
    pub async fn panel_code(&self, now: i64) -> Result<IssuedEntry, AdminError> {
        Ok(repo::panel::issue_entry(&self.pool, self.organization, &self.admin, now).await?)
    }
}
