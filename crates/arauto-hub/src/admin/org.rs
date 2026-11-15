//! `arauto-hub admin org ...`: the org chart of the Hermes (`docs/decisoes/0043-organograma-e-niveis.md`).
//!
//! The command and the panel are the same two functions: [`Session::org_chart`] reads and
//! [`Session::org_publish`] writes the whole wanted state in one write. There is no separate
//! path, so the rules, the audit and the mark are the ones of the panel.

use super::{AdminError, Session};
use crate::core::org::{Chart, ChartVersion, Draft};
use crate::repo;
use crate::repo::org::PublishOutcome;

impl Session {
    /// `admin org show`: the chart, as the database holds it.
    ///
    /// # Errors
    /// [`AdminError::Repo`] when the database refuses.
    pub async fn org_chart(&self) -> Result<Chart, AdminError> {
        Ok(repo::org::chart(&self.pool, self.organization).await?)
    }

    /// `admin org publish FILE --version HEX`: writes the draft, if it started from `base`.
    ///
    /// # Errors
    /// [`AdminError::Restored`] when the database came back from an older copy: nothing is
    /// written.
    pub async fn org_publish(
        &self,
        draft: &Draft,
        base: ChartVersion,
        now: i64,
    ) -> Result<PublishOutcome, AdminError> {
        Ok(repo::org::publish(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            draft,
            base,
            now,
        )
        .await?)
    }
}
