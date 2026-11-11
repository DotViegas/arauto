//! Operating-system specifics of Arauto.
//!
//! Everything that changes from one system to another lives in this crate, behind one
//! interface. Linux is the production platform; Windows is a development platform only, and
//! some guarantees are weaker there (each item says which).
//!
//! | Piece | Linux | Windows |
//! |---|---|---|
//! | [`write_durable`] | temporary file, `fsync`, rename, `fsync` of the folder | the same, without the folder |
//! | [`remove_stale_temporaries`] | removes the temporary files of interrupted writes, one folder at a time | the same |
//! | [`create_private_dir`] | mode `0700`, tightened when it is more open; a link, or the folder of another user, is refused | inherited permissions |
//! | [`replace_executable`] | temporary file and rename over the program | the program in use is renamed away first |
//! | [`FileLock`] | `flock` on an empty file; a removed lock file can only be noticed | `LockFileEx` on an empty file that cannot be removed while locked |
//! | [`LocalListener`], [`LocalStream`] | Unix domain socket in a folder of the user that nobody else can use, owner only, peer user checked | named pipe, first instance, no remote client |
//! | [`shutdown_signal`] | `SIGTERM` or `SIGINT` | Ctrl+C |
//! | [`default_home`] | `$HOME/.arauto` | `%USERPROFILE%\.arauto` |
//!
//! The file operations block the calling thread: from asynchronous code, run them with
//! `tokio::task::spawn_blocking`.
//!
//! How the pieces fit when several copies of a program share one folder:
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use arauto_platform::{
//!     FileLock, LOCK_FILE_NAME, LocalListener, LocalStream, LockError, create_private_dir,
//!     default_home, remove_stale_temporaries,
//! };
//!
//! # async fn start() -> Result<(), Box<dyn std::error::Error>> {
//! let home = default_home()?;
//! // Also refuses, on Unix, a folder that is not this user's own.
//! create_private_dir(&home)?;
//! match FileLock::try_acquire(&home.join(LOCK_FILE_NAME)) {
//!     // The first copy. The lock of the folder, and no other lock, is what entitles it
//!     // to create the socket; the listener keeps the lock for as long as it lives.
//!     Ok(lock) => {
//!         // The leftovers of interrupted writes, in each folder apart: subfolders are
//!         // not visited, and the secrets may well live in one. The lock keeps out the
//!         // other copies only. A command that writes here without contending for this
//!         // lock is not kept out, and a file it is writing may be removed under it: such
//!         // a command takes a lock that the cleanup takes too, or cleans what it writes.
//!         remove_stale_temporaries(&home)?;
//!         remove_stale_temporaries(&home.join("a-subfolder-with-secrets"))?;
//!         let mut listener = LocalListener::bind(&home, Arc::new(lock)).await?;
//!         let _client = listener.accept().await?;
//!     }
//!     // Every other copy talks to the first one.
//!     Err(LockError::AlreadyHeld { .. }) => {
//!         let _leader = LocalStream::connect(&home).await?;
//!     }
//!     Err(error) => return Err(error.into()),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Files that must survive a power loss, and the request to stop:
//!
//! ```no_run
//! use arauto_platform::{FsError, replace_executable, shutdown_signal, write_durable};
//!
//! # async fn run(home: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
//! // Listens from here on, so a request during the start-up is not missed.
//! let stop = shutdown_signal()?;
//! match write_durable(&home.join("credential.seal"), b"sealed bytes") {
//!     // On disk (on Windows, without the folder): only now may the caller say "saved".
//!     Ok(()) => {}
//!     // The new file is in place and may not survive a power loss: nothing is "saved",
//!     // and what the file holds, not what is in memory, is the credential from now on.
//!     Err(FsError::NotDurable { .. }) => {}
//!     // The previous file, or no file, is still there.
//!     Err(error) => return Err(error.into()),
//! }
//! // Works while the program at the destination is running.
//! replace_executable(&std::env::current_exe()?, &home.join("arauto-connector"))?;
//! // SIGTERM or SIGINT on Unix, Ctrl+C on Windows.
//! stop.await;
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(not(any(unix, windows)))]
compile_error!("arauto-platform supports Unix and Windows only");

mod durable;
mod error;
mod executable;
mod home;
mod local_socket;
mod lock;
mod private_dir;
mod shutdown;

#[cfg(test)]
mod test_support;

pub use durable::{remove_stale_temporaries, write_durable};
pub use error::FsError;
pub use executable::replace_executable;
pub use home::{HomeError, default_home};
pub use local_socket::{
    LOCK_FILE_NAME, LocalListener, LocalSocketError, LocalStream, SOCKET_FILE_NAME,
};
pub use lock::{FileLock, LockError};
pub use private_dir::create_private_dir;
pub use shutdown::{ShutdownSignal, ShutdownSignalError, shutdown_signal};

#[cfg(test)]
mod tests {
    use super::*;

    fn moves_into_a_task<T: Send + 'static>() {}
    fn is_shared_between_tasks<T: Send + Sync + 'static>() {}
    fn is_an_error_for_any_caller<T: std::error::Error + Send + Sync + 'static>() {}
    fn is_used_by_reference<T: Unpin>() {}

    /// What the other crates count on when they spawn tasks and convert errors. It holds
    /// on both systems or it does not compile.
    #[test]
    fn public_types_fit_asynchronous_callers() {
        is_shared_between_tasks::<FileLock>();
        is_shared_between_tasks::<LocalListener>();
        is_shared_between_tasks::<LocalStream>();
        moves_into_a_task::<ShutdownSignal>();
        is_used_by_reference::<LocalStream>();
        is_used_by_reference::<ShutdownSignal>();

        is_an_error_for_any_caller::<FsError>();
        is_an_error_for_any_caller::<LockError>();
        is_an_error_for_any_caller::<LocalSocketError>();
        is_an_error_for_any_caller::<ShutdownSignalError>();
        is_an_error_for_any_caller::<HomeError>();
    }
}
