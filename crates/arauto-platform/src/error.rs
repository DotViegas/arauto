//! Error of the file operations.

use std::io;
use std::path::{Path, PathBuf};

/// What went wrong in [`write_durable`](crate::write_durable),
/// [`remove_stale_temporaries`](crate::remove_stale_temporaries),
/// [`create_private_dir`](crate::create_private_dir) or
/// [`replace_executable`](crate::replace_executable).
///
/// Two variants report a change that did happen: [`NotDurable`](Self::NotDurable) and
/// [`OldProgramNotRestored`](Self::OldProgramNotRestored). With every other variant, what
/// was under the path before the call is still there.
///
/// The message already carries the cause reported by the system, so it is not repeated as
/// an error source.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    /// The path has no file name, so nothing can be written under it.
    #[error("{}: not a file path", .path.display())]
    NotAFilePath {
        /// The path that was given.
        path: PathBuf,
    },

    /// The path exists and is not a folder.
    #[error("{}: exists and is not a folder", .path.display())]
    NotADirectory {
        /// The path that was given.
        path: PathBuf,
    },

    /// The path is a link, and a private folder must be a folder itself: a link can be
    /// pointed elsewhere by whoever owns it. Nothing is changed, here or where the link
    /// leads. Unix only.
    #[error("{}: is a link, not a folder; name the folder itself", .path.display())]
    IsALink {
        /// The path that was given.
        path: PathBuf,
    },

    /// The folder belongs to another user than the one this process runs as, so it is not
    /// private to this process: its owner can rename and replace whatever is put in it.
    /// The folder is left as it was. Unix only.
    #[error(
        "{}: the folder belongs to user {owner}, and this process runs as user {user}",
        .path.display()
    )]
    NotOwner {
        /// The folder.
        path: PathBuf,
        /// The user id that owns the folder.
        owner: u32,
        /// The user id of this process.
        user: u32,
    },

    /// The path of the program is a folder. It is left untouched.
    #[error("{}: is a folder, not a program", .path.display())]
    IsADirectory {
        /// The path that was given.
        path: PathBuf,
    },

    /// The random generator of the system failed while a temporary name was drawn.
    #[error("the system random generator failed: {0}")]
    Random(getrandom::Error),

    /// Every temporary name that was drawn already existed.
    #[error("{}: no free temporary name in this folder", .dir.display())]
    NoTemporaryName {
        /// The folder where the temporary file was being created.
        dir: PathBuf,
    },

    /// The new file, program or folder **is in place** under `path`, but the folder that
    /// holds it could not be forced to disk, so a power loss may still undo the change.
    ///
    /// The change must not be reported as durable. From here on, what is on disk tells
    /// what is current, not what the caller has in memory. Unix only: Windows never forces
    /// a folder to disk.
    #[error(
        "{} is in place, but its folder could not be forced to disk and a power loss may \
         undo the change: {cause}",
        .path.display()
    )]
    NotDurable {
        /// What was written, replaced or created, and is in place.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },

    /// The new program could not take the place of the old one, and the old one could not
    /// be put back: **no program is left** under `destination`. The old program is the file
    /// `parked`, which stays until a later replacement succeeds. Windows only.
    #[error(
        "{cause}; the program that was there could not be put back under {} ({put_back}) \
         and is now at {}",
        .destination.display(),
        .parked.display()
    )]
    OldProgramNotRestored {
        /// The path of the program, which now holds nothing.
        destination: PathBuf,
        /// Where the old program is.
        parked: PathBuf,
        /// Why the new program did not take the place.
        cause: Box<FsError>,
        /// Why the old program could not be put back.
        put_back: io::Error,
    },

    /// A call to the system failed.
    #[error("could not {action} {}: {cause}", .path.display())]
    Io {
        /// What was being done, as a verb phrase that reads before the path.
        action: &'static str,
        /// The path the action was applied to.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },
}

impl FsError {
    pub(crate) fn io(action: &'static str, path: &Path, cause: io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            cause,
        }
    }
}
