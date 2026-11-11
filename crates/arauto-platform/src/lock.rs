//! Exclusive lock between processes, on a file that exists only for that.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

/// How many times the lock file is opened again when it is replaced while being locked.
/// One is enough unless somebody replaces the file on purpose.
const REPLACED_FILE_ATTEMPTS: usize = 8;

/// Why a [`FileLock`] was not acquired.
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    /// Another process, or another handle in this process, holds the lock. This is the
    /// expected answer for every copy but the first, not a failure of the system.
    #[error("{}: locked by another process", .path.display())]
    AlreadyHeld {
        /// The lock file.
        path: PathBuf,
    },

    /// The lock file was removed or replaced every time it was being locked, so no lock
    /// that guards the path could be taken. Unix only.
    #[error("{}: the lock file keeps being removed or replaced", .path.display())]
    Replaced {
        /// The lock file.
        path: PathBuf,
    },

    /// The lock file could not be opened or locked.
    #[error("could not lock {}: {cause}", .path.display())]
    Io {
        /// The lock file.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },
}

/// An exclusive lock held on a file, released when the value is dropped or the process
/// ends, however it ends.
///
/// The lock file is empty and is never written or removed by this type. **Nobody may
/// remove or rename it**, not even when the folder is being emptied for good: the lock is
/// on the file, not on the name, so a process that then creates the file again locks a
/// different file, and both believe they hold the lock.
///
/// - On Windows the system enforces that: while the lock is held, removing or renaming the
///   lock file fails, and so does renaming the folder that holds it.
/// - On Unix nothing stops the removal. The lock is only handed out for the file that is
///   under the path at that moment, and [`is_in_place`](Self::is_in_place) lets the holder
///   find out later that its file is gone.
///
/// The lock is advisory and needs a file system that supports file locks.
///
/// Any file can be locked. The lock that gives the right to listen on the local socket of
/// a folder ([`LocalListener`](crate::LocalListener)) is the one taken on the file
/// [`LOCK_FILE_NAME`](crate::LOCK_FILE_NAME) of that folder; the locks of other files,
/// in that folder or elsewhere, are independent of it.
#[derive(Debug)]
pub struct FileLock {
    file: File,
    path: PathBuf,
}

impl FileLock {
    /// Tries to take the lock on `path` without waiting, creating the file if needed (with
    /// mode `0600` on Unix). The folder must exist.
    ///
    /// # Errors
    ///
    /// [`LockError::AlreadyHeld`] when the lock belongs to someone else;
    /// [`LockError::Replaced`] when the file under `path` never stayed the same for long
    /// enough to be locked; [`LockError::Io`] for any failure of the system.
    pub fn try_acquire(path: &Path) -> Result<Self, LockError> {
        Self::try_acquire_opening(path, open)
    }

    /// [`try_acquire`](Self::try_acquire), with the opening of the lock file apart so that
    /// tests can replace the file right after it is opened.
    fn try_acquire_opening(
        path: &Path,
        mut open: impl FnMut(&Path) -> io::Result<File>,
    ) -> Result<Self, LockError> {
        let failed = |cause| LockError::Io {
            path: path.to_path_buf(),
            cause,
        };
        for _ in 0..REPLACED_FILE_ATTEMPTS {
            let file = open(path).map_err(failed)?;
            match file.try_lock() {
                Ok(()) => {}
                Err(TryLockError::WouldBlock) => {
                    return Err(LockError::AlreadyHeld {
                        path: path.to_path_buf(),
                    });
                }
                Err(TryLockError::Error(cause)) => return Err(failed(cause)),
            }
            let lock = Self {
                file,
                path: path.to_path_buf(),
            };
            // A file removed or replaced between the opening and the locking is locked for
            // nothing: whoever opens the path now gets another file.
            if lock.is_in_place().map_err(failed)? {
                return Ok(lock);
            }
        }
        Err(LockError::Replaced {
            path: path.to_path_buf(),
        })
    }

    /// The lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the file that is locked is still the one under [`path`](Self::path).
    ///
    /// `false` means that the lock file was removed or replaced after the lock was taken:
    /// this lock no longer keeps anybody out, and its holder must stop acting as the only
    /// one. On Windows the answer is always `true`, because the system refuses to remove or
    /// rename the file while the lock is held.
    ///
    /// # Errors
    ///
    /// The error of the system when the path cannot be inspected.
    pub fn is_in_place(&self) -> io::Result<bool> {
        is_file_at(&self.file, &self.path)
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // Closing the file releases the lock only when no duplicate of the handle is open
        // anywhere (a child process that is being started holds one for an instant on
        // Unix), and Windows may take a moment even then. Unlocking releases it at once.
        let _ = self.file.unlock();
    }
}

/// Opens the lock file, creating it for its owner only when it does not exist.
#[cfg(unix)]
fn open(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    // Locking needs a handle open for reading or writing; creating needs writing.
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
}

/// Opens the lock file, creating it when it does not exist, in a way that keeps it where
/// it is for as long as it is open. A new file takes the permissions of its folder.
#[cfg(windows)]
fn open(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    /// `FILE_SHARE_READ | FILE_SHARE_WRITE`, without `FILE_SHARE_DELETE`: other handles
    /// may read and write (which locking needs), and none may remove or rename the file.
    const SHARE_ALL_BUT_DELETE: u32 = 0x1 | 0x2;

    // Locking needs a handle open for reading or writing; creating needs writing.
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(SHARE_ALL_BUT_DELETE)
        .open(path)
}

/// Whether `file` is the file that `path` names now, by device and file number.
#[cfg(unix)]
fn is_file_at(file: &File, path: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;

    let held = file.metadata()?;
    match std::fs::metadata(path) {
        Ok(named) => Ok(named.dev() == held.dev() && named.ino() == held.ino()),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(cause) => Err(cause),
    }
}

/// On Windows an open lock file cannot be removed or renamed (see `open`), so it is always
/// the file that its path names.
#[cfg(windows)]
fn is_file_at(_file: &File, _path: &Path) -> io::Result<bool> {
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read};
    use std::process::{Command, Stdio};
    use std::time::Duration;

    use super::*;
    use crate::test_support::{CHILD_READY, ChildGuard, child_role};

    /// Role of the second process that takes the lock and keeps it until it is killed.
    const ROLE_HOLD_LOCK: &str = "hold-lock";
    /// Where the second process finds the lock file.
    const LOCK_PATH_VARIABLE: &str = "ARAUTO_PLATFORM_TEST_LOCK_PATH";

    #[test]
    fn second_attempt_says_the_lock_has_an_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");

        let first = FileLock::try_acquire(&path).unwrap();
        let second = FileLock::try_acquire(&path);

        assert!(
            matches!(&second, Err(LockError::AlreadyHeld { path: held }) if *held == path),
            "{second:?}"
        );
        assert_eq!(first.path(), path);
    }

    #[test]
    fn lock_is_free_again_after_the_owner_drops_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");

        let first = FileLock::try_acquire(&path).unwrap();
        assert!(matches!(
            FileLock::try_acquire(&path),
            Err(LockError::AlreadyHeld { .. })
        ));
        drop(first);

        let second = FileLock::try_acquire(&path).unwrap();
        assert!(matches!(
            FileLock::try_acquire(&path),
            Err(LockError::AlreadyHeld { .. })
        ));
        drop(second);
    }

    /// The release does not wait for the system to close the file: a duplicate of the
    /// handle, still open somewhere, does not keep the lock.
    #[test]
    fn dropped_lock_is_free_even_while_a_duplicate_of_its_handle_is_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let first = FileLock::try_acquire(&path).unwrap();
        let duplicate = first.file.try_clone().unwrap();

        drop(first);

        let second = FileLock::try_acquire(&path);
        assert!(second.is_ok(), "{second:?}");
        drop(second);
        drop(duplicate);
    }

    #[test]
    fn lock_file_is_created_empty_and_is_left_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");

        let lock = FileLock::try_acquire(&path).unwrap();
        assert!(lock.is_in_place().unwrap());
        drop(lock);

        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
    }

    /// One owner at a time depends on the lock file staying where it is. Windows refuses
    /// to remove or rename it, and to rename its folder, while the lock is held.
    #[cfg(windows)]
    #[test]
    fn lock_file_cannot_be_removed_or_renamed_while_the_lock_is_held() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Home");
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("lock");
        let lock = FileLock::try_acquire(&path).unwrap();

        let removed = std::fs::remove_file(&path);
        let renamed = std::fs::rename(&path, folder.join("lock.away"));
        let folder_renamed = std::fs::rename(&folder, dir.path().join("home"));

        assert!(removed.is_err(), "the lock file was removed");
        assert!(renamed.is_err(), "the lock file was renamed");
        assert!(
            folder_renamed.is_err(),
            "the folder of the lock was renamed"
        );
        assert!(lock.is_in_place().unwrap());
        let second = FileLock::try_acquire(&path);
        assert!(
            matches!(second, Err(LockError::AlreadyHeld { .. })),
            "{second:?}"
        );

        // Without the lock, the file is a file like any other.
        drop(lock);
        std::fs::rename(&folder, dir.path().join("home")).unwrap();
        std::fs::remove_file(dir.path().join("home").join("lock")).unwrap();
    }

    #[cfg(unix)]
    mod unix {
        use std::os::unix::fs::PermissionsExt;

        use super::*;

        #[test]
        fn lock_file_is_for_the_owner_only() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("lock");

            let _lock = FileLock::try_acquire(&path).unwrap();

            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o7777;
            assert_eq!(mode, 0o600);
        }

        /// Unix lets anybody with the folder remove the lock file. The holder cannot stop
        /// it, but it can find out.
        #[test]
        fn holder_finds_out_that_its_lock_file_was_removed() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("lock");
            let first = FileLock::try_acquire(&path).unwrap();
            assert!(first.is_in_place().unwrap());

            std::fs::remove_file(&path).unwrap();
            assert!(!first.is_in_place().unwrap(), "no file under the path");

            // The next process locks a new file: the first lock keeps nobody out any more.
            let second = FileLock::try_acquire(&path).unwrap();
            assert!(second.is_in_place().unwrap());
            assert!(!first.is_in_place().unwrap(), "another file under the path");
        }

        #[test]
        fn holder_finds_out_that_its_lock_file_was_replaced() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("lock");
            let lock = FileLock::try_acquire(&path).unwrap();
            let other = dir.path().join("other");
            std::fs::write(&other, b"").unwrap();

            std::fs::rename(&other, &path).unwrap();

            assert!(!lock.is_in_place().unwrap());
        }

        /// A file that leaves the path between being opened and being locked is not the
        /// lock of the path: the file that is there now is locked instead.
        #[test]
        fn file_removed_while_it_was_being_locked_is_not_taken_for_the_lock() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("lock");
            let mut removals = 3;

            let lock = FileLock::try_acquire_opening(&path, |path| {
                let file = open(path)?;
                if removals > 0 {
                    removals -= 1;
                    std::fs::remove_file(path)?;
                }
                Ok(file)
            })
            .unwrap();

            assert_eq!(removals, 0);
            assert!(lock.is_in_place().unwrap());
            let second = FileLock::try_acquire(&path);
            assert!(
                matches!(second, Err(LockError::AlreadyHeld { .. })),
                "{second:?}"
            );
        }

        #[test]
        fn file_that_never_stays_under_the_path_ends_in_an_error() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("lock");
            let mut openings = 0;

            let outcome = FileLock::try_acquire_opening(&path, |path| {
                openings += 1;
                let file = open(path)?;
                std::fs::remove_file(path)?;
                Ok(file)
            });

            assert!(
                matches!(&outcome, Err(LockError::Replaced { path: lost }) if *lost == path),
                "{outcome:?}"
            );
            assert_eq!(openings, REPLACED_FILE_ATTEMPTS);
        }
    }

    #[test]
    fn existing_content_of_the_lock_file_is_not_touched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        std::fs::write(&path, b"left by someone").unwrap();

        drop(FileLock::try_acquire(&path).unwrap());

        assert_eq!(std::fs::read(&path).unwrap(), b"left by someone");
    }

    #[test]
    fn missing_folder_is_an_error_and_not_a_held_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("lock");

        let outcome = FileLock::try_acquire(&path);

        assert!(matches!(outcome, Err(LockError::Io { .. })), "{outcome:?}");
    }

    #[test]
    fn two_lock_files_do_not_interfere() {
        let dir = tempfile::tempdir().unwrap();

        let _first = FileLock::try_acquire(&dir.path().join("lock-a")).unwrap();
        let _second = FileLock::try_acquire(&dir.path().join("lock-b")).unwrap();
    }

    /// What makes the next copy of the connector take over: the system releases the lock
    /// of a process that dies, with no cooperation from it.
    #[test]
    fn lock_of_a_process_that_dies_is_released() {
        if child_role().as_deref() == Some(ROLE_HOLD_LOCK) {
            let path = PathBuf::from(std::env::var_os(LOCK_PATH_VARIABLE).unwrap());
            let _lock = FileLock::try_acquire(&path).unwrap();
            println!("{CHILD_READY}");
            let mut rest = Vec::new();
            let _ = std::io::stdin().read_to_end(&mut rest);
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let mut holder = ChildGuard::spawn(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "lock::tests::lock_of_a_process_that_dies_is_released",
                    "--exact",
                    "--nocapture",
                ])
                .env(LOCK_PATH_VARIABLE, &path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
            ROLE_HOLD_LOCK,
        );
        let mut output = BufReader::new(holder.stdout());
        let mut line = String::new();
        while line.trim_end() != CHILD_READY {
            line.clear();
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "the holder never locked"
            );
        }

        let while_alive = FileLock::try_acquire(&path);
        assert!(
            matches!(while_alive, Err(LockError::AlreadyHeld { .. })),
            "{while_alive:?}"
        );

        holder.kill();
        holder.wait();

        // The system frees the lock as part of ending the process; allow it a moment.
        let mut after_death = FileLock::try_acquire(&path);
        for _ in 0..100 {
            if after_death.is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
            after_death = FileLock::try_acquire(&path);
        }
        assert!(after_death.is_ok(), "{after_death:?}");
    }
}
