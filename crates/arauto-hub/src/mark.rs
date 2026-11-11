//! The audit mark.
//!
//! The mark is one number in a file next to the master key, outside the database: the number
//! of the last security event of the audit (`docs/04-seguranca-conexao.md`, C11). A database
//! whose last security event is behind the mark came back from an older copy; so did a
//! database with data next to no mark at all.
//!
//! Rules of the file:
//!
//! - **It only grows.** A write stores the larger of what is on disk and what was asked, read
//!   and written under a file lock that every process takes, so two processes never move it
//!   back.
//! - **It is durable before anyone is told.** [`Mark::advance`] returns after the file and,
//!   on Linux, its folder were forced to disk.
//! - **Absent is not zero.** A new installation creates the mark at zero; a missing file
//!   beside a database with data is evidence ([`MarkValue::Absent`]).
//! - **Advances that wait join.** While one write is on its way, the others queue up, and
//!   the next write takes the largest number asked so far.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use arauto_platform::{FsError, LockError, write_durable};
use tokio::sync::watch;

use crate::lock_wait::acquire_within;

/// The file of the mark, inside the data folder.
pub const MARK_FILE: &str = "audit.mark";

/// The lock every process takes to read and rewrite the mark.
pub const MARK_LOCK_FILE: &str = "audit.mark.lock";

/// How long a write waits for the lock of another process.
const LOCK_PATIENCE: Duration = Duration::from_secs(5);

/// The longest content of a well-formed mark: 19 digits and a line feed.
const MARK_FILE_MAX_LEN: usize = 20;

/// What is on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkValue {
    /// There is no mark file.
    Absent,
    /// The number of the last security event that was marked. Zero for a new installation.
    At(i64),
}

impl MarkValue {
    /// The number, when there is a mark.
    #[must_use]
    pub const fn number(self) -> Option<i64> {
        match self {
            MarkValue::Absent => None,
            MarkValue::At(number) => Some(number),
        }
    }
}

/// What went wrong with the mark.
#[derive(Debug, thiserror::Error)]
pub enum MarkError {
    /// A mark is never negative.
    #[error("the audit mark is never negative")]
    Negative,

    /// The file is there and does not hold a number. It is not treated as absent: someone
    /// has to look at it.
    #[error("{}: the audit mark is damaged (it must hold one number)", .path.display())]
    Damaged {
        /// The mark file.
        path: PathBuf,
    },

    /// The lock of the mark stayed with another process for too long, or could not be taken.
    #[error("the audit mark is locked: {0}")]
    Lock(LockError),

    /// The mark could not be written durably. With [`FsError::NotDurable`] the new number is
    /// in the file and may not survive a power loss.
    #[error(transparent)]
    Fs(#[from] FsError),

    /// The mark could not be read.
    #[error("could not read {}: {cause}", .path.display())]
    Read {
        /// The mark file.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },

    /// The mark could not be erased.
    #[error("could not erase {}: {cause}", .path.display())]
    Erase {
        /// The mark file.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },

    /// The thread that was reading or writing the mark died.
    #[error("the task that handles the audit mark failed")]
    Task,
}

/// The mark of one data folder. Clones share the same queue of writes.
#[derive(Debug, Clone)]
pub struct Mark {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    path: PathBuf,
    lock_path: PathBuf,
    lock_patience: Duration,
    state: Mutex<State>,
    /// Bumped at the end of every write, to wake the advances that queued behind it.
    finished: watch::Sender<u64>,
}

#[derive(Debug, Default)]
struct State {
    /// The largest number this process knows to be durable on disk.
    durable: Option<i64>,
    /// The largest number anyone asked for.
    wanted: i64,
    /// Whether a write is on its way.
    writing: bool,
    /// How many writes this process made.
    writes: u64,
}

impl Mark {
    /// The mark kept in `data_dir`. Nothing is read or created yet.
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self::with_lock_patience(data_dir, LOCK_PATIENCE)
    }

    /// As [`Mark::new`], giving up on the lock of another process after `patience`.
    #[must_use]
    pub fn with_lock_patience(data_dir: &Path, patience: Duration) -> Self {
        let (finished, _) = watch::channel(0);
        Self {
            shared: Arc::new(Shared {
                path: data_dir.join(MARK_FILE),
                lock_path: data_dir.join(MARK_LOCK_FILE),
                lock_patience: patience,
                state: Mutex::new(State::default()),
                finished,
            }),
        }
    }

    /// The mark file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.shared.path
    }

    /// Reads the mark from disk. It blocks the calling thread for one small read.
    ///
    /// The file is replaced whole by every write, so a read needs no lock: it sees a number
    /// that was on disk at some moment.
    ///
    /// # Errors
    /// [`MarkError::Damaged`] when the file does not hold a number; [`MarkError::Read`] when
    /// it cannot be read.
    pub fn read_blocking(&self) -> Result<MarkValue, MarkError> {
        read_file(&self.shared.path)
    }

    /// [`Mark::read_blocking`], off the asynchronous threads.
    ///
    /// # Errors
    /// The same, and [`MarkError::Task`] when the reading thread dies.
    pub async fn read(&self) -> Result<MarkValue, MarkError> {
        let shared = Arc::clone(&self.shared);
        tokio::task::spawn_blocking(move || read_file(&shared.path))
            .await
            .map_err(|_| MarkError::Task)?
    }

    /// Makes the mark at least `to`, durably, and returns the number that is on disk.
    ///
    /// The mark only grows: when the file already holds a larger number, it keeps it. A
    /// missing file is created, so `advance(0)` is how a new installation gets its mark.
    /// Callers that arrive while a write is on its way wait for it and then share one write
    /// for the largest number among them.
    ///
    /// Call it only after the event with that number is committed and durable in the
    /// database: a mark ahead of the database reads as a restored database.
    ///
    /// # Errors
    /// [`MarkError::Negative`]; [`MarkError::Lock`] when another process kept the lock;
    /// [`MarkError::Damaged`] when the file on disk is not a number (it is left as it is);
    /// [`MarkError::Fs`] when the write failed or is not durable.
    pub async fn advance(&self, to: i64) -> Result<i64, MarkError> {
        if to < 0 {
            return Err(MarkError::Negative);
        }
        loop {
            let turn = {
                let mut state = self.shared.state();
                if let Some(durable) = state.durable
                    && durable >= to
                {
                    return Ok(durable);
                }
                state.wanted = state.wanted.max(to);
                if state.writing {
                    // Subscribed under the lock: the end of the write in progress is seen.
                    Turn::Wait(self.shared.finished.subscribe())
                } else {
                    state.writing = true;
                    Turn::Write(state.wanted)
                }
            };
            match turn {
                Turn::Wait(mut finished) => {
                    // The sender lives in `shared`, which this call holds: no error here.
                    let _ = finished.changed().await;
                }
                Turn::Write(target) => {
                    let writing = Writing(&self.shared);
                    let shared = Arc::clone(&self.shared);
                    let written = tokio::task::spawn_blocking(move || {
                        write_at_least(
                            &shared.path,
                            &shared.lock_path,
                            shared.lock_patience,
                            target,
                        )
                    })
                    .await
                    .map_err(|_| MarkError::Task)
                    .and_then(|outcome| outcome);
                    if let Ok(on_disk) = written {
                        let mut state = self.shared.state();
                        state.durable = Some(state.durable.map_or(on_disk, |d| d.max(on_disk)));
                        state.writes = state.writes.saturating_add(1);
                    }
                    drop(writing);
                    written?;
                }
            }
        }
    }

    /// Erases the mark file and what this value remembers of it.
    ///
    /// **Only `dev-reset` calls this**, together with erasing the database: one without the
    /// other reads as a restored database, which is the right reading. The file goes away
    /// under the same lock every write takes.
    ///
    /// # Errors
    /// [`MarkError::Lock`] when another process kept the lock; [`MarkError::Erase`] when the
    /// file could not be removed.
    pub async fn erase(&self) -> Result<(), MarkError> {
        let shared = Arc::clone(&self.shared);
        tokio::task::spawn_blocking(move || {
            if shared.path.exists() {
                let _lock = acquire_within(&shared.lock_path, shared.lock_patience)
                    .map_err(MarkError::Lock)?;
                match std::fs::remove_file(&shared.path) {
                    Ok(()) => {}
                    Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                    Err(cause) => {
                        return Err(MarkError::Erase {
                            path: shared.path.clone(),
                            cause,
                        });
                    }
                }
            }
            let mut state = shared.state();
            state.durable = None;
            state.wanted = 0;
            Ok(())
        })
        .await
        .map_err(|_| MarkError::Task)?
    }

    /// How many durable writes this value and its clones made. Fewer than the calls to
    /// [`Mark::advance`] when they joined.
    #[must_use]
    pub fn writes(&self) -> u64 {
        self.shared.state().writes
    }
}

/// What a call to `advance` does next.
enum Turn {
    /// Another call is writing: wait for it to finish and look again.
    Wait(watch::Receiver<u64>),
    /// This call writes, for everyone who asked so far.
    Write(i64),
}

/// Held by the call that is writing. Its end, also when the call is cancelled, frees the turn
/// and wakes whoever waits.
struct Writing<'a>(&'a Shared);

impl Drop for Writing<'_> {
    fn drop(&mut self) {
        self.0.state().writing = false;
        self.0
            .finished
            .send_modify(|count| *count = count.wrapping_add(1));
    }
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        // The state is a few numbers, valid after any interrupted update.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Under the lock of the mark: reads what is on disk and writes, durably, the larger of it
/// and `target`. Returns what is on disk at the end.
fn write_at_least(
    path: &Path,
    lock_path: &Path,
    patience: Duration,
    target: i64,
) -> Result<i64, MarkError> {
    let _lock = acquire_within(lock_path, patience).map_err(MarkError::Lock)?;
    let number = match read_file(path)? {
        MarkValue::Absent => target,
        MarkValue::At(on_disk) => on_disk.max(target),
    };
    // Written even when the file already holds the number: whoever wrote it may have failed
    // to make it durable, and this call is about to say that it is.
    write_durable(path, format!("{number}\n").as_bytes())?;
    Ok(number)
}

fn read_file(path: &Path) -> Result<MarkValue, MarkError> {
    let read_failed = |cause| MarkError::Read {
        path: path.to_path_buf(),
        cause,
    };
    let file = match File::open(path) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(MarkValue::Absent),
        Err(cause) => return Err(read_failed(cause)),
    };
    // One byte more than the longest mark, to notice a file that is too long.
    let mut content = Vec::with_capacity(MARK_FILE_MAX_LEN + 1);
    file.take(MARK_FILE_MAX_LEN as u64 + 1)
        .read_to_end(&mut content)
        .map_err(read_failed)?;
    parse(&content)
        .map(MarkValue::At)
        .ok_or_else(|| MarkError::Damaged {
            path: path.to_path_buf(),
        })
}

/// From 1 to 19 decimal digits that fit an `i64`, with at most one line feed after them.
fn parse(content: &[u8]) -> Option<i64> {
    if content.len() > MARK_FILE_MAX_LEN {
        return None;
    }
    let digits = content.strip_suffix(b"\n").unwrap_or(content);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arauto_platform::FileLock;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn folder() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn the_file_holds_one_number() {
        assert_eq!(parse(b"0\n"), Some(0));
        assert_eq!(parse(b"0"), Some(0));
        assert_eq!(parse(b"42\n"), Some(42));
        assert_eq!(parse(b"9223372036854775807\n"), Some(i64::MAX));
        for damaged in [
            &b""[..],
            b"\n",
            b"-1\n",
            b"+1\n",
            b" 1\n",
            b"1 \n",
            b"1\n\n",
            b"1\r\n",
            b"0x10\n",
            b"12a\n",
            b"9223372036854775808\n",
            b"00000000000000000000001\n",
            b"\xff\xfe",
        ] {
            assert_eq!(parse(damaged), None, "{damaged:?}");
        }
    }

    #[tokio::test]
    async fn absent_is_not_zero() {
        let dir = folder();
        let mark = Mark::new(dir.path());
        assert_eq!(mark.read().await.unwrap(), MarkValue::Absent);
        assert_eq!(MarkValue::Absent.number(), None);
        assert_eq!(MarkValue::At(0).number(), Some(0));

        assert_eq!(mark.advance(0).await.unwrap(), 0);
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(0));
        assert_eq!(mark.read_blocking().unwrap(), MarkValue::At(0));
        assert_eq!(fs::read(dir.path().join(MARK_FILE)).unwrap(), b"0\n");
    }

    #[tokio::test]
    async fn the_mark_only_grows() {
        let dir = folder();
        let mark = Mark::new(dir.path());
        assert_eq!(mark.advance(10).await.unwrap(), 10);
        assert_eq!(mark.advance(5).await.unwrap(), 10);
        assert_eq!(mark.advance(10).await.unwrap(), 10);
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(10));
        assert_eq!(mark.advance(11).await.unwrap(), 11);
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(11));
        assert!(matches!(mark.advance(-1).await, Err(MarkError::Negative)));
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(11));
    }

    #[tokio::test]
    async fn a_process_that_knows_less_does_not_move_the_mark_back() {
        // Two values over the same folder do not share memory, like two processes: the
        // second one learns what is on disk only under the lock.
        let dir = folder();
        let hub = Mark::new(dir.path());
        let admin = Mark::new(dir.path());
        assert_eq!(hub.advance(40).await.unwrap(), 40);
        assert_eq!(admin.advance(7).await.unwrap(), 40);
        assert_eq!(admin.read().await.unwrap(), MarkValue::At(40));
        assert_eq!(hub.read().await.unwrap(), MarkValue::At(40));
        assert_eq!(admin.advance(41).await.unwrap(), 41);
        assert_eq!(hub.advance(41).await.unwrap(), 41);
    }

    #[tokio::test]
    async fn a_damaged_mark_is_an_error_and_is_left_alone() {
        let dir = folder();
        let path = dir.path().join(MARK_FILE);
        fs::write(&path, b"twelve\n").unwrap();
        let mark = Mark::new(dir.path());
        assert!(matches!(mark.read().await, Err(MarkError::Damaged { .. })));
        assert!(matches!(
            mark.advance(3).await,
            Err(MarkError::Damaged { .. })
        ));
        assert_eq!(fs::read(&path).unwrap(), b"twelve\n");
        // The failure did not leave the queue stuck.
        fs::write(&path, b"2\n").unwrap();
        assert_eq!(mark.advance(3).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn the_write_happens_under_the_lock_of_the_mark() {
        let dir = folder();
        let mark = Mark::with_lock_patience(dir.path(), Duration::from_millis(60));
        mark.advance(1).await.unwrap();

        let other_process = FileLock::try_acquire(&dir.path().join(MARK_LOCK_FILE)).unwrap();
        assert!(matches!(mark.advance(2).await, Err(MarkError::Lock(_))));
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(1));
        drop(other_process);

        assert_eq!(mark.advance(2).await.unwrap(), 2);
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(2));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn advances_that_wait_join_in_one_write() {
        let dir = folder();
        let mark = Mark::new(dir.path());
        mark.advance(0).await.unwrap();
        assert_eq!(mark.writes(), 1);

        // The lock is held as if by another process, so the first write stays on its way
        // while the others arrive.
        let other_process = FileLock::try_acquire(&dir.path().join(MARK_LOCK_FILE)).unwrap();
        let mut waiting = Vec::new();
        for number in 1..=60i64 {
            let mark = mark.clone();
            waiting.push(tokio::spawn(
                async move { (number, mark.advance(number).await) },
            ));
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(0));
        drop(other_process);

        for task in waiting {
            let (asked, on_disk) = task.await.unwrap();
            assert!(on_disk.unwrap() >= asked);
        }
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(60));
        // The first one, the write that was waiting for the lock, and one for everyone else.
        assert!(
            mark.writes() <= 3,
            "{} writes for 60 advances",
            mark.writes()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn simultaneous_advances_of_two_processes_end_at_the_largest() {
        let dir = folder();
        // What is under test is the order of the writes, not how long one waits for the
        // other: on a busy machine 120 writes take their time, and none may give up.
        let patient = Duration::from_secs(120);
        let hub = Mark::with_lock_patience(dir.path(), patient);
        let admin = Mark::with_lock_patience(dir.path(), patient);
        hub.advance(0).await.unwrap();

        let stop = Arc::new(AtomicBool::new(false));
        let watcher = {
            let stop = Arc::clone(&stop);
            let reader = Mark::new(dir.path());
            std::thread::spawn(move || {
                let mut last = 0;
                while !stop.load(Ordering::SeqCst) {
                    if let Ok(MarkValue::At(now)) = reader.read_blocking() {
                        assert!(now >= last, "the mark went back from {last} to {now}");
                        last = now;
                    }
                }
                last
            })
        };

        let mut tasks = Vec::new();
        for number in 1..=120i64 {
            // Out of order on purpose, and split between the two.
            let mark = if number % 2 == 0 { &hub } else { &admin }.clone();
            let asked = (number * 37) % 121;
            tasks.push(tokio::spawn(async move {
                let on_disk = mark.advance(asked).await.unwrap();
                assert!(on_disk >= asked);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        stop.store(true, Ordering::SeqCst);
        let last_seen = watcher.join().unwrap();
        assert!(last_seen <= 120);
        assert_eq!(hub.read().await.unwrap(), MarkValue::At(120));
        assert!(hub.writes() + admin.writes() < 120);
    }

    #[tokio::test]
    async fn a_cancelled_advance_does_not_block_the_next() {
        let dir = folder();
        let mark = Mark::new(dir.path());
        let other_process = FileLock::try_acquire(&dir.path().join(MARK_LOCK_FILE)).unwrap();
        let cancelled = tokio::time::timeout(Duration::from_millis(50), mark.advance(5)).await;
        assert!(cancelled.is_err());
        drop(other_process);
        assert_eq!(mark.advance(6).await.unwrap(), 6);
    }

    #[tokio::test]
    async fn no_temporary_file_is_left_behind() {
        let dir = folder();
        let mark = Mark::new(dir.path());
        for number in 0..5 {
            mark.advance(number).await.unwrap();
        }
        let mut names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, [MARK_FILE, MARK_LOCK_FILE]);
    }

    #[tokio::test]
    async fn erasing_the_mark_removes_the_file_and_what_was_remembered() {
        let dir = folder();
        let mark = Mark::new(dir.path());
        // Nothing to erase, in a folder with no mark and in a folder that does not exist.
        mark.erase().await.unwrap();
        Mark::new(&dir.path().join("missing"))
            .erase()
            .await
            .unwrap();

        assert_eq!(mark.advance(41).await.unwrap(), 41);
        mark.erase().await.unwrap();
        assert!(!dir.path().join(MARK_FILE).exists());
        assert_eq!(mark.read().await.unwrap(), MarkValue::Absent);
        // The same value starts again from the disk, not from what it knew.
        assert_eq!(mark.advance(0).await.unwrap(), 0);
        assert_eq!(mark.read().await.unwrap(), MarkValue::At(0));

        // It takes the lock every write takes.
        let slow = Mark::with_lock_patience(dir.path(), Duration::from_millis(50));
        let other_process = FileLock::try_acquire(&dir.path().join(MARK_LOCK_FILE)).unwrap();
        assert!(matches!(slow.erase().await, Err(MarkError::Lock(_))));
        drop(other_process);
        assert_eq!(slow.read().await.unwrap(), MarkValue::At(0));
    }
}
