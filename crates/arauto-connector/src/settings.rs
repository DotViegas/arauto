//! Every number the connector waits for.
//!
//! [`Settings::default`] holds the documented values: the deadlines of the protocol
//! (`docs/14-protocolo-do-canal.md`, section 4) and the waits of this machine that the
//! documents leave to the connector. The program only ever uses the default. The tests of this
//! crate shorten them through the `testing` feature, which a shipped build never has.

use arauto_protocol::limits::{Deadlines, SECOND_MS};

/// The waits and sizes of the connector.
#[derive(Clone)]
pub struct Settings {
    /// The deadlines of the protocol: the connector reads the ones of its own side.
    pub(crate) deadlines: Deadlines,
    /// The TCP connection, the TLS and the upgrade to websocket.
    pub(crate) connect_ms: i64,
    /// The three messages of the handshake.
    pub(crate) handshake_ms: i64,
    /// A frame that does not leave in this time ends the connection.
    pub(crate) write_stall_ms: i64,
    /// Frames of the control queue.
    pub(crate) control_queue_frames: usize,
    /// Frames of the data queue.
    pub(crate) data_queue_frames: usize,
    /// How long a command waits for an answer of the leader on the local socket.
    pub(crate) local_answer_ms: i64,
    /// How long `leave` waits for the leader: it asks the hub for each identity.
    pub(crate) local_leave_ms: i64,
    /// The pause of a copy that is not the leader before it tries the lock or the socket again.
    pub(crate) relay_retry_ms: i64,
    /// How long the leader waits for the lock of the device folders before a write.
    pub(crate) devices_lock_ms: i64,
    /// How long a command waits for the lock of the device folders.
    pub(crate) command_lock_ms: i64,
    /// The command of Hermes that registers or removes the connector.
    pub(crate) hermes_ms: i64,
    /// The whole key step of `join` (`docs/11-conector.md`, section 6): every call of Hermes in
    /// it and the probe of the listener, together.
    pub(crate) key_step_ms: i64,
    /// Where the listener guard of the key step connects: the default address of the local API
    /// of Hermes.
    pub(crate) key_probe: std::net::SocketAddr,
    /// The orderly close of the channel at the end, below the 2 seconds Hermes gives.
    pub(crate) shutdown_ms: i64,
    /// What wakes the Hermes: one for the whole process, whatever connection is up.
    pub(crate) waker: std::sync::Arc<crate::wake::Waker>,
    /// How long a minute of `await-reply` lasts, in milliseconds.
    pub(crate) await_minute_ms: i64,
    /// How often the leader lists the threads for the waits it holds.
    pub(crate) await_poll_ms: i64,
    /// How long after a wait ended by a notice the leader checks that the answer was read.
    pub(crate) await_grace_ms: i64,
    /// The pause of `await-reply` before it asks the leader again.
    pub(crate) await_retry_ms: i64,
    /// The waits of `await-reply` that this process holds: one for the whole process.
    pub(crate) waiters: std::sync::Arc<crate::waiters::Waiters>,
    #[cfg(feature = "testing")]
    pub(crate) testing: crate::testing::Testing,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            deadlines: Deadlines::DOCUMENTED,
            connect_ms: 10 * SECOND_MS,
            handshake_ms: 10 * SECOND_MS,
            write_stall_ms: 30 * SECOND_MS,
            control_queue_frames: 16,
            data_queue_frames: 64,
            local_answer_ms: 5 * SECOND_MS,
            local_leave_ms: 120 * SECOND_MS,
            relay_retry_ms: 200,
            devices_lock_ms: 30 * SECOND_MS,
            command_lock_ms: 3 * SECOND_MS,
            // The first command of Hermes in a data folder it never used took 47 s on a
            // Windows machine (14/11/2026); in a folder in use it takes 2 to 4 s.
            hermes_ms: 90 * SECOND_MS,
            key_step_ms: 60 * SECOND_MS,
            key_probe: std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 8642)),
            shutdown_ms: 1_500,
            waker: std::sync::Arc::default(),
            await_minute_ms: 60 * SECOND_MS,
            await_poll_ms: 30 * SECOND_MS,
            await_grace_ms: 600 * SECOND_MS,
            await_retry_ms: 5 * SECOND_MS,
            waiters: std::sync::Arc::default(),
            #[cfg(feature = "testing")]
            testing: crate::testing::Testing::default(),
        }
    }
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("deadlines", &self.deadlines)
            .finish_non_exhaustive()
    }
}

impl Settings {
    /// These settings, waking the Hermes through `target`. Without one, no notice wakes.
    #[must_use]
    pub fn waking(mut self, target: Option<crate::wake::WakeTarget>) -> Self {
        self.waker = std::sync::Arc::new(crate::wake::Waker::new(target));
        self
    }
}

/// A number of milliseconds as a duration. Zero and below are no wait at all.
pub(crate) fn duration(ms: i64) -> std::time::Duration {
    std::time::Duration::from_millis(u64::try_from(ms).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_values() {
        let settings = Settings::default();
        assert_eq!(settings.deadlines, Deadlines::DOCUMENTED);
        assert_eq!(settings.write_stall_ms, 30_000);
        assert_eq!(
            (settings.control_queue_frames, settings.data_queue_frames),
            (16, 64)
        );
        // Hermes waits 2 seconds before it ends the connector by force.
        assert!(settings.shutdown_ms < 2_000);
        // The key step: 60 seconds in all, and the default address of the local API of Hermes.
        assert_eq!(settings.key_step_ms, 60_000);
        assert_eq!(settings.key_probe.to_string(), "127.0.0.1:8642");
        // The leader gives up a write before the hub gives up the "saved".
        assert!(settings.devices_lock_ms < settings.deadlines.saved_ms);
        // The waits of await-reply: a minute, a 30 s poll, a 10-minute grace, a 5 s retry.
        assert_eq!(
            (
                settings.await_minute_ms,
                settings.await_poll_ms,
                settings.await_grace_ms,
                settings.await_retry_ms
            ),
            (60_000, 30_000, 600_000, 5_000)
        );
    }

    #[test]
    fn a_negative_wait_is_no_wait() {
        assert_eq!(duration(-5), std::time::Duration::ZERO);
        assert_eq!(duration(1_500), std::time::Duration::from_millis(1_500));
    }
}
