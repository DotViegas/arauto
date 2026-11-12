//! The purge (`docs/15-mvp.md`, section 2, rule 11): what passed its retention is deleted, in
//! small batches, by the time of the hub.
//!
//! The days of each table are its row in `retention`, the same for every organization. What of
//! each table goes, and from when the days count:
//!
//! | Target | What goes | Counted from |
//! |---|---|---|
//! | `peer_message` | every message, read or not (`docs/13-conversa-entre-hermes.md`, section 7) | its `purge_at`, written with the message: the sending plus the days of `retention` |
//! | `peer_thread` | a thread left without a message, in the same purge | its last message, with the days of `peer_message` |
//! | `tool_call` | every row | `at` |
//! | `notice` | a resolved one: confirmed, expired or discarded | `resolved_at` |
//! | `channel_connection` | a closed one: the row of an open connection is the one T1, T2 and the end of the connection still write | `closed_at` |
//! | `unknown_device` | a listing that is no pending proof of a restoration: that proof stays until a restored mode handles it (`docs/14-protocolo-do-canal.md`, section 6) | `seen_at`, the first time the identity presented its seal |
//!
//! A deadline has passed when the time of the hub **reached** it, as in [`super::sweep`]. A
//! table without a row in `retention` is left as it is.
//!
//! **Batches.** Each batch is one statement on its own, of at most the rows it is given, that
//! skips the rows another transaction holds: the database does a batch whole or not at all. A
//! purge asked to stop stops between two batches; a batch that fails ends the purge there. In
//! both cases what was deleted is whole batches of rows past their retention, and the next run
//! goes on from where this one stopped.
//!
//! After the purge, an unknown identity that presents its seal again is listed anew, and with
//! the restored mode off that listing is proof of a restoration once more: an open point
//! (`docs/14-protocolo-do-canal.md`, section 13).

use std::collections::BTreeMap;

use arauto_protocol::limits::DAY_MS;
use sqlx::postgres::PgPool;

use super::RepoError;
use crate::db::ResolvedOrg;

/// What the purge deletes, in the order a run goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    /// `peer_message`: every message past its `purge_at`, read or not.
    PeerMessages,
    /// `peer_thread`: the threads left without a message, the days of `peer_message` after
    /// their last one.
    EmptyThreads,
    /// `tool_call`: every row.
    ToolCalls,
    /// `notice`: the resolved ones.
    ResolvedNotices,
    /// `channel_connection`: the closed ones.
    ClosedConnections,
    /// `unknown_device`: the listings that are no pending proof of a restoration.
    UnknownDevices,
    /// `engine_event`: the events of the engine already handled. One still to be handled is
    /// never purged: it waits for whoever reads the inbox.
    EngineEvents,
}

impl Target {
    /// Every target, in the order of a run: the messages before the threads they leave empty.
    pub const ALL: [Target; 7] = [
        Target::PeerMessages,
        Target::EmptyThreads,
        Target::ToolCalls,
        Target::ResolvedNotices,
        Target::ClosedConnections,
        Target::UnknownDevices,
        Target::EngineEvents,
    ];

    /// The row of `retention` whose days this target keeps.
    #[must_use]
    pub const fn retention_row(self) -> &'static str {
        match self {
            Target::PeerMessages | Target::EmptyThreads => "peer_message",
            Target::ToolCalls => "tool_call",
            Target::ResolvedNotices => "notice",
            Target::ClosedConnections => "channel_connection",
            Target::UnknownDevices => "unknown_device",
            Target::EngineEvents => "engine_event",
        }
    }

    /// The table the target deletes from.
    #[must_use]
    pub const fn table(self) -> &'static str {
        match self {
            Target::PeerMessages => "peer_message",
            Target::EmptyThreads => "peer_thread",
            Target::ToolCalls => "tool_call",
            Target::ResolvedNotices => "notice",
            Target::ClosedConnections => "channel_connection",
            Target::UnknownDevices => "unknown_device",
            Target::EngineEvents => "engine_event",
        }
    }

    /// The time a row of this target goes at: a row whose time reached it is past its
    /// retention. A message carries its own time, `purge_at`: for it, the deadline is now.
    fn deadline(self, days: i64, now: i64) -> i64 {
        match self {
            Target::PeerMessages => now,
            _ => now.saturating_sub(days.saturating_mul(DAY_MS)),
        }
    }

    /// One batch: `$1` is the organization, `$2` the deadline, `$3` the most rows. The rows
    /// another transaction holds are skipped, never waited for.
    const fn statement(self) -> &'static str {
        match self {
            Target::PeerMessages => {
                "DELETE FROM peer_message WHERE organization_id = $1 AND id IN ( \
                     SELECT id FROM peer_message \
                     WHERE organization_id = $1 AND purge_at <= $2 \
                     LIMIT $3 FOR UPDATE SKIP LOCKED)"
            }
            // A thread with a message is never one of them, even past the days after its last
            // one (a message kept to a later `purge_at`): left to the key of `peer_message`,
            // it would fail every batch, and the purge would never reach the tables after it.
            // The key still refuses the delete of a thread that got a message meanwhile: that
            // batch fails whole.
            Target::EmptyThreads => {
                "DELETE FROM peer_thread WHERE organization_id = $1 AND id IN ( \
                     SELECT t.id FROM peer_thread t \
                     WHERE t.organization_id = $1 AND t.last_message_at <= $2 \
                       AND NOT EXISTS (SELECT 1 FROM peer_message m WHERE m.thread_id = t.id) \
                     LIMIT $3 FOR UPDATE OF t SKIP LOCKED)"
            }
            Target::ToolCalls => {
                "DELETE FROM tool_call WHERE organization_id = $1 AND id IN ( \
                     SELECT id FROM tool_call \
                     WHERE organization_id = $1 AND at <= $2 \
                     LIMIT $3 FOR UPDATE SKIP LOCKED)"
            }
            Target::ResolvedNotices => {
                "DELETE FROM notice WHERE organization_id = $1 AND id IN ( \
                     SELECT id FROM notice \
                     WHERE organization_id = $1 AND resolved_at <= $2 \
                       AND state IN ('confirmed', 'expired', 'discarded') \
                     LIMIT $3 FOR UPDATE SKIP LOCKED)"
            }
            // The index is on the opening; a connection closes after it opens, and one whose
            // clock went back meanwhile only waits a little longer.
            Target::ClosedConnections => {
                "DELETE FROM channel_connection WHERE organization_id = $1 AND id IN ( \
                     SELECT id FROM channel_connection \
                     WHERE organization_id = $1 AND opened_at <= $2 AND closed_at <= $2 \
                     LIMIT $3 FOR UPDATE SKIP LOCKED)"
            }
            Target::UnknownDevices => {
                "DELETE FROM unknown_device WHERE organization_id = $1 AND id IN ( \
                     SELECT id FROM unknown_device \
                     WHERE organization_id = $1 AND seen_at <= $2 AND NOT proof_pending \
                     LIMIT $3 FOR UPDATE SKIP LOCKED)"
            }
            Target::EngineEvents => {
                "DELETE FROM engine_event WHERE organization_id = $1 AND id IN ( \
                     SELECT id FROM engine_event \
                     WHERE organization_id = $1 AND processed_at <= $2 \
                     LIMIT $3 FOR UPDATE SKIP LOCKED)"
            }
        }
    }
}

/// The days of each table of `retention`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Retention {
    days: BTreeMap<String, i64>,
}

impl Retention {
    /// The days of `table`, when `retention` has a row for it.
    #[must_use]
    pub fn days(&self, table: &str) -> Option<i64> {
        self.days.get(table).copied()
    }

    /// The tables `retention` names, in order.
    pub fn tables(&self) -> impl Iterator<Item = &str> {
        self.days.keys().map(String::as_str)
    }
}

/// Reads `retention`. It is of the installation: the same days for every organization.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn retention(pool: &PgPool) -> Result<Retention, RepoError> {
    let rows: Vec<(String, i32)> = sqlx::query_as("SELECT table_name, days FROM retention")
        .fetch_all(pool)
        .await?;
    Ok(Retention {
        days: rows
            .into_iter()
            .map(|(table, days)| (table, i64::from(days)))
            .collect(),
    })
}

/// What a purge did to one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgedTarget {
    /// The target.
    pub target: Target,
    /// The rows deleted.
    pub rows: u64,
    /// The batches run, the last one included: the one that found fewer rows than its size.
    pub batches: u32,
}

/// What a purge of one organization did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Purged {
    /// Each target it got to, in the order of [`Target::ALL`].
    pub targets: Vec<PurgedTarget>,
    /// It stopped before the end because it was asked to.
    pub stopped: bool,
}

impl Purged {
    /// Every row deleted.
    #[must_use]
    pub fn rows(&self) -> u64 {
        self.targets
            .iter()
            .fold(0u64, |rows, target| rows.saturating_add(target.rows))
    }

    /// What it did to `target`: nothing, when it did not get to it.
    #[must_use]
    pub fn of(&self, target: Target) -> PurgedTarget {
        self.targets
            .iter()
            .find(|done| done.target == target)
            .copied()
            .unwrap_or(PurgedTarget {
                target,
                rows: 0,
                batches: 0,
            })
    }
}

/// Deletes what of `organization` passed its retention at `now`: target by target, in batches
/// of at most `batch_rows` rows (at least one), each one a statement of its own. Before each
/// batch it asks `keep_going`; when the answer is no, it stops there and says so.
///
/// # Errors
/// [`RepoError::Database`] when a batch fails. That batch deleted nothing; the ones before it
/// stay done, and the targets after it wait for the next run.
pub async fn purge(
    pool: &PgPool,
    organization: ResolvedOrg,
    retention: &Retention,
    now: i64,
    batch_rows: u32,
    mut keep_going: impl FnMut() -> bool,
) -> Result<Purged, RepoError> {
    let limit = batch_rows.max(1);
    let mut purged = Purged::default();
    for target in Target::ALL {
        let Some(days) = retention.days(target.retention_row()) else {
            continue;
        };
        let deadline = target.deadline(days, now);
        let mut done = PurgedTarget {
            target,
            rows: 0,
            batches: 0,
        };
        loop {
            if !keep_going() {
                purged.targets.push(done);
                purged.stopped = true;
                return Ok(purged);
            }
            let rows = batch(pool, organization, target, deadline, limit).await?;
            done.rows = done.rows.saturating_add(rows);
            done.batches = done.batches.saturating_add(1);
            if rows < u64::from(limit) {
                break;
            }
        }
        purged.targets.push(done);
    }
    Ok(purged)
}

/// One batch of `target` in `organization`: at most `limit` rows whose time reached
/// `deadline`. Returns how many it deleted.
async fn batch(
    pool: &PgPool,
    organization: ResolvedOrg,
    target: Target,
    deadline: i64,
    limit: u32,
) -> Result<u64, RepoError> {
    Ok(sqlx::query(target.statement())
        .bind(organization.id())
        .bind(deadline)
        .bind(i64::from(limit))
        .execute(pool)
        .await?
        .rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_names_its_table_and_its_retention() {
        let tables: Vec<&str> = Target::ALL.iter().map(|target| target.table()).collect();
        assert_eq!(
            tables,
            [
                "peer_message",
                "peer_thread",
                "tool_call",
                "notice",
                "channel_connection",
                "unknown_device",
                "engine_event"
            ]
        );
        // The threads follow the days of the messages that leave them empty.
        assert_eq!(Target::EmptyThreads.retention_row(), "peer_message");
        for target in Target::ALL {
            let statement = target.statement();
            assert!(
                statement.starts_with(&format!("DELETE FROM {} ", target.table())),
                "{statement}"
            );
            // Every batch keeps to the organization it was given, inside and outside, and
            // never waits for a row another transaction holds.
            assert_eq!(statement.matches("organization_id = $1").count(), 2);
            assert!(statement.contains("LIMIT $3"), "{statement}");
            assert!(statement.contains("SKIP LOCKED"), "{statement}");
        }
    }

    #[test]
    fn a_deadline_is_the_days_of_the_table_before_now_and_never_overflows() {
        let now = 100 * DAY_MS;
        assert_eq!(Target::ToolCalls.deadline(7, now), 93 * DAY_MS);
        assert_eq!(Target::UnknownDevices.deadline(90, now), 10 * DAY_MS);
        // A message carries its own time.
        assert_eq!(Target::PeerMessages.deadline(30, now), now);
        assert_eq!(Target::EmptyThreads.deadline(30, now), 70 * DAY_MS);
        assert_eq!(Target::ToolCalls.deadline(i64::MAX, i64::MIN), i64::MIN);
    }
}
