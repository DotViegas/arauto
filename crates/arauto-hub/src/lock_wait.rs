//! Waiting for a file lock.
//!
//! The lock of `arauto-platform` only tries: whoever must wait loops, with a deadline.

use std::path::Path;
use std::time::{Duration, Instant};

use arauto_platform::{FileLock, LockError};

/// How long a holder is given before the next try.
const RETRY_EVERY: Duration = Duration::from_millis(5);

/// Takes the lock on `path`, trying again for up to `patience` while another holder has it.
///
/// It blocks the calling thread. The deadline is a wait bound measured on the monotonic clock
/// of the process; it is not a time that is ever stored or compared with the time of the hub.
///
/// # Errors
/// [`LockError::AlreadyHeld`] when the lock was still taken at the deadline, and the other
/// errors of [`FileLock::try_acquire`] as they come.
pub(crate) fn acquire_within(path: &Path, patience: Duration) -> Result<FileLock, LockError> {
    let started = Instant::now();
    loop {
        match FileLock::try_acquire(path) {
            Err(LockError::AlreadyHeld { path }) => {
                if started.elapsed() >= patience {
                    return Err(LockError::AlreadyHeld { path });
                }
                std::thread::sleep(RETRY_EVERY);
            }
            outcome => return outcome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_free_lock_is_taken_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let lock = acquire_within(&dir.path().join("x.lock"), Duration::ZERO);
        assert!(lock.is_ok());
    }

    #[test]
    fn a_held_lock_is_waited_for_and_then_given_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.lock");
        let held = acquire_within(&path, Duration::ZERO).unwrap();
        let refused = acquire_within(&path, Duration::from_millis(40));
        assert!(matches!(refused, Err(LockError::AlreadyHeld { .. })));
        drop(held);
        assert!(acquire_within(&path, Duration::from_millis(40)).is_ok());
    }

    #[test]
    fn a_lock_released_during_the_wait_is_taken() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.lock");
        let held = acquire_within(&path, Duration::ZERO).unwrap();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            drop(held);
        });
        let taken = acquire_within(&path, Duration::from_secs(20));
        releaser.join().unwrap();
        assert!(taken.is_ok());
    }
}
