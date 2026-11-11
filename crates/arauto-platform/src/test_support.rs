//! Helpers for the tests that need a second process.
//!
//! Such a test starts a copy of the test program itself, asking for the same test by its
//! exact name, with a role in the environment of the child. The test function looks at the
//! role first and, when there is one, plays it instead of running the test.

use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus};
use std::sync::Arc;
use std::time::Duration;

use crate::{FileLock, LOCK_FILE_NAME, LocalListener, LockError};

/// Environment variable, set only on the child, that names the role it plays.
const CHILD_ROLE_VARIABLE: &str = "ARAUTO_PLATFORM_TEST_CHILD_ROLE";

/// The line a child prints once it holds what the parent waits for.
pub(crate) const CHILD_READY: &str = "arauto-platform-test-child-ready";

/// The role this process was started with, if it is the child of another test.
pub(crate) fn child_role() -> Option<String> {
    std::env::var(CHILD_ROLE_VARIABLE).ok()
}

/// Turns an asynchronous step that would hang into a step that fails.
pub(crate) async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .expect("the step did not finish in 30 seconds")
}

/// A temporary folder that only its owner can use, as the folder of a listener must be.
/// The folders that `tempfile` creates by default are open to everyone on Unix.
#[cfg(unix)]
pub(crate) fn private_tempdir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .expect("a private temporary folder can be created")
}

/// A temporary folder for a listener. On Windows the inherited permissions stand.
#[cfg(windows)]
pub(crate) fn private_tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary folder can be created")
}

/// Takes the lock of the folder `dir`, which nobody holds.
pub(crate) fn lock_of(dir: &Path) -> Arc<FileLock> {
    let lock = FileLock::try_acquire(&dir.join(LOCK_FILE_NAME));
    Arc::new(lock.expect("the lock of the folder is free"))
}

/// Takes the lock of the folder `dir` once the system has released it from a process that
/// just died, which may take a moment.
pub(crate) fn lock_left_by_a_dead_process(dir: &Path) -> Arc<FileLock> {
    let path = dir.join(LOCK_FILE_NAME);
    for _ in 0..100 {
        match FileLock::try_acquire(&path) {
            Err(LockError::AlreadyHeld { .. }) => std::thread::sleep(Duration::from_millis(50)),
            outcome => return Arc::new(outcome.expect("the lock of the folder can be taken")),
        }
    }
    panic!("the lock of the dead process was never released");
}

/// Does what the first copy of a program does: takes the lock of the folder `dir` and
/// listens on its local socket. The listener keeps the lock; the caller gets its own copy.
pub(crate) async fn listen(dir: &Path) -> (Arc<FileLock>, LocalListener) {
    let lock = lock_of(dir);
    let listener = LocalListener::bind(dir, lock.clone())
        .await
        .expect("the holder of the lock listens");
    (lock, listener)
}

/// A child process that never outlives the test: it is killed when the guard goes away.
pub(crate) struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    /// Starts `command` with `role` in its environment.
    pub(crate) fn spawn(command: &mut Command, role: &str) -> Self {
        let child = command
            .env(CHILD_ROLE_VARIABLE, role)
            .spawn()
            .expect("the child process starts");
        Self { child }
    }

    /// The output of the child. The command must have asked for a pipe; works once.
    pub(crate) fn stdout(&mut self) -> ChildStdout {
        self.child.stdout.take().expect("stdout was piped")
    }

    /// Closes the input of the child: a role that reads it finishes when it ends.
    pub(crate) fn close_stdin(&mut self) {
        drop(self.child.stdin.take());
    }

    /// The process id, for the tests that signal the child.
    #[cfg(unix)]
    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn is_running(&mut self) -> bool {
        self.child
            .try_wait()
            .expect("the child can be polled")
            .is_none()
    }

    pub(crate) fn kill(&mut self) {
        self.child.kill().expect("the child can be killed");
    }

    pub(crate) fn wait(&mut self) -> ExitStatus {
        self.child.wait().expect("the child can be waited for")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
