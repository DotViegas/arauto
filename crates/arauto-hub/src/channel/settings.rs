//! Every number of the transport: the deadlines of `docs/14-protocolo-do-canal.md`
//! (sections 1 to 4 and 7) and the sizes of `docs/15-mvp.md` (section 3).
//!
//! [`ChannelSettings::default`] holds the documented values. Tests build their own with short
//! deadlines and small ceilings; nothing in the channel reads the constants of the protocol
//! once a `ChannelSettings` is given.

use std::time::Duration;

use arauto_protocol::limits::{
    CONNECTIONS_WITHOUT_SESSION_MAX, CONNECTIONS_WITHOUT_SESSION_PER_ORIGIN_MAX, Deadlines,
    MINUTE_MS, REASSEMBLY_BUDGET_BYTES, SECOND_MS,
};
use arauto_protocol::message::ProtoRange;

use crate::db::PoolOptions;

/// Frames the control queue of one connection holds. Full, it closes the connection.
pub const CONTROL_QUEUE_FRAMES: usize = 16;

/// Frames the data queue of one connection holds. Full, whoever produces data waits.
pub const DATA_QUEUE_FRAMES: usize = 64;

/// A frame that does not leave in this time closes the connection: the network stopped.
pub const WRITE_STALL_MS: i64 = 30 * SECOND_MS;

/// Messages above this size are read on a blocking thread, away from the network tasks.
pub const INLINE_DECODE_MAX_BYTES: usize = 256 * 1024;

/// After `closing`, how long the hub keeps reading (and discarding) what the connector still
/// sends, so that the connection ends with the connector's own close and not with a reset
/// that could throw away the `closing` before the connector reads it.
pub const LINGER_MS: i64 = SECOND_MS;

/// The size of the read buffer of each websocket. The library starts with 128 KiB; a frame
/// never passes 64 KiB, and a parked session should cost little memory.
pub const READ_BUFFER_BYTES: usize = 16 * 1024;

/// How often the hub sweeps: the audit mark, the deadlines, the state of every device with an
/// open connection, the refused enrollments and the end of the restored mode
/// (`docs/10-maquinas-de-estado.md`, section 7).
pub const SWEEP_INTERVAL_MS: i64 = 2 * SECOND_MS;

/// Connections of the pool of the sweep, apart from the pool of the connectors
/// ([`ChannelSettings::pool`]). The sweep runs one query at a time; the second connection is
/// for the reading of the states that the restored mode asks for from a connection, which
/// may come while the sweep runs.
pub const SWEEP_POOL_SIZE: u32 = 2;

/// How often the purge runs (`docs/15-mvp.md`, section 2, rule 11): the first run comes one
/// interval after the start, away from the return in mass of a restart. The retentions are in
/// days; ten minutes keeps each run short.
pub const PURGE_INTERVAL_MS: i64 = 10 * MINUTE_MS;

/// The most rows one batch of the purge deletes: each batch is one short statement, and the
/// connections of the pool and the locks of the rows are let go between two of them.
pub const PURGE_BATCH_ROWS: u32 = 500;

/// How often the purge runs, and how big its batches are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgeSettings {
    /// From the start to the first run, and then between two runs.
    pub interval_ms: i64,
    /// The most rows of one batch; at least one.
    pub batch_rows: u32,
}

impl Default for PurgeSettings {
    fn default() -> Self {
        Self {
            interval_ms: PURGE_INTERVAL_MS,
            batch_rows: PURGE_BATCH_ROWS,
        }
    }
}

/// What the channel needs besides the environment: deadlines, queues and ceilings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSettings {
    /// The deadlines of the protocol. The hub reads the ones of its own side.
    pub deadlines: Deadlines,
    /// A frame that takes longer than this to leave closes the connection.
    pub write_stall_ms: i64,
    /// Frames of the control queue of each connection.
    pub control_queue_frames: usize,
    /// Frames of the data queue of each connection.
    pub data_queue_frames: usize,
    /// Connections without a session in the whole hub, apart from the waiting ones.
    pub without_session_max: usize,
    /// Connections without a session per origin, apart from the waiting ones.
    pub without_session_per_origin_max: usize,
    /// Bytes of every message under reassembly, in the whole hub.
    pub reassembly_budget_bytes: usize,
    /// Messages above this size are read away from the network tasks.
    pub inline_decode_max_bytes: usize,
    /// How long the hub keeps draining a connection after `closing`.
    pub linger_ms: i64,
    /// The pool of database connections of `serve`.
    pub pool: PoolOptions,
    /// The versions of the protocol the hub speaks. A test raises the lowest one to have a
    /// connector that is too old.
    pub versions: ProtoRange,
    /// How often the hub sweeps. The sweep, not the notice of the admin command, is what
    /// closes a revoked device in less than 5 seconds.
    pub sweep_interval_ms: i64,
    /// Whether the hub listens to the notices of the admin command (`NOTIFY`). Off, only the
    /// sweep finds what the admin changed: a test turns it off to prove that.
    pub listen_admin: bool,
    /// The purge of what passed its retention.
    pub purge: PurgeSettings,
}

impl Default for ChannelSettings {
    fn default() -> Self {
        Self {
            deadlines: Deadlines::default(),
            write_stall_ms: WRITE_STALL_MS,
            control_queue_frames: CONTROL_QUEUE_FRAMES,
            data_queue_frames: DATA_QUEUE_FRAMES,
            without_session_max: CONNECTIONS_WITHOUT_SESSION_MAX,
            without_session_per_origin_max: CONNECTIONS_WITHOUT_SESSION_PER_ORIGIN_MAX,
            reassembly_budget_bytes: REASSEMBLY_BUDGET_BYTES,
            inline_decode_max_bytes: INLINE_DECODE_MAX_BYTES,
            linger_ms: LINGER_MS,
            pool: PoolOptions::default(),
            versions: ProtoRange::CURRENT,
            sweep_interval_ms: SWEEP_INTERVAL_MS,
            listen_admin: true,
            purge: PurgeSettings::default(),
        }
    }
}

/// A number of milliseconds as a duration. Zero and below are no wait at all.
pub(crate) fn duration(ms: i64) -> Duration {
    Duration::from_millis(u64::try_from(ms).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers of the documents, written a second time.
    #[test]
    fn the_defaults_are_the_documented_values() {
        let settings = ChannelSettings::default();
        assert_eq!(settings.deadlines, Deadlines::DOCUMENTED);
        assert_eq!(settings.write_stall_ms, 30_000);
        assert_eq!(
            (settings.control_queue_frames, settings.data_queue_frames),
            (16, 64)
        );
        assert_eq!(
            (
                settings.without_session_max,
                settings.without_session_per_origin_max
            ),
            (1_024, 16)
        );
        assert_eq!(settings.reassembly_budget_bytes, 256 * 1024 * 1024);
        assert_eq!(settings.inline_decode_max_bytes, 256 * 1024);
        assert_eq!(settings.pool, PoolOptions::default());
        assert_eq!(settings.versions, ProtoRange::CURRENT);
        assert_eq!(settings.sweep_interval_ms, 2_000);
        assert!(settings.listen_admin);
        assert_eq!(
            settings.purge,
            PurgeSettings {
                interval_ms: 600_000,
                batch_rows: 500
            }
        );
    }

    #[test]
    fn a_negative_wait_is_no_wait() {
        assert_eq!(duration(-5), Duration::ZERO);
        assert_eq!(duration(1_500), Duration::from_millis(1_500));
    }
}
