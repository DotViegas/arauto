//! The local socket on Unix: a domain socket inside the folder.
//!
//! Three things keep other users out: the folder is private to its owner (made so by
//! `create_private_dir`, and checked by the listener before it touches anything), the
//! socket file has mode `0600`, and each side reads the user of the other from the kernel
//! and compares it with the owner of the folder.

use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};

use super::{Attempt, LocalSocketError, SOCKET_FILE_NAME};
use crate::durable::user_creating_in;

/// Both ends of a connection are the same type on Unix.
pub(super) type Stream = UnixStream;

/// The longest socket path the system takes, in bytes: the size of the address field of
/// the system, less its terminator.
#[cfg(any(target_os = "linux", target_os = "android"))]
const MAX_SOCKET_PATH_BYTES: usize = 107;
/// The longest socket path the system takes, in bytes. The smallest address field among
/// the other Unix systems is assumed.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const MAX_SOCKET_PATH_BYTES: usize = 103;

/// Connecting needs the write permission on the socket: only the owner gets it.
const SOCKET_MODE: u32 = 0o600;

/// The socket of a folder, refused when the system could not address it.
fn socket_path(dir: &Path) -> Result<PathBuf, LocalSocketError> {
    let path = dir.join(SOCKET_FILE_NAME);
    let len = path.as_os_str().as_bytes().len();
    if len > MAX_SOCKET_PATH_BYTES {
        return Err(LocalSocketError::PathTooLong {
            path,
            len,
            max: MAX_SOCKET_PATH_BYTES,
        });
    }
    Ok(path)
}

#[derive(Debug)]
pub(super) struct Listener {
    listener: UnixListener,
    path: PathBuf,
    /// The user that owns the folder: the only one whose processes are served.
    owner_uid: u32,
}

impl Listener {
    pub(super) async fn bind(dir: &Path) -> Result<Self, LocalSocketError> {
        let path = socket_path(dir)?;
        let user = user_creating_in(dir).map_err(|error| {
            LocalSocketError::io(
                "tell which user this process runs as in",
                dir,
                io::Error::other(error),
            )
        })?;
        Self::bind_as(dir, path, user).await
    }

    /// [`bind`](Self::bind) for a process that runs as `user`, with the socket at `path`.
    async fn bind_as(dir: &Path, path: PathBuf, user: u32) -> Result<Self, LocalSocketError> {
        let folder = fs::metadata(dir)
            .map_err(|cause| LocalSocketError::io("inspect the folder", dir, cause))?;
        // Before the socket of a dead listener is removed: in the folder of somebody
        // else, nothing is this process's to remove.
        let owner_uid = require_own_private_folder(dir, &folder, user)?;
        remove_dead_socket(dir, &path).await?;

        let listener = UnixListener::bind(&path).map_err(|cause| match cause.kind() {
            // Somebody bound the path after the check above.
            io::ErrorKind::AddrInUse => LocalSocketError::AlreadyListening {
                dir: dir.to_path_buf(),
            },
            _ => LocalSocketError::io("create the socket", &path, cause),
        })?;
        if let Err(error) = protect(dir, &path, owner_uid) {
            drop(listener);
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        Ok(Self {
            listener,
            path,
            owner_uid,
        })
    }

    pub(super) async fn accept(&mut self) -> Result<Stream, LocalSocketError> {
        loop {
            let (stream, _) = self.listener.accept().await.map_err(|cause| {
                LocalSocketError::io("accept a client on the socket", &self.path, cause)
            })?;
            // A peer whose user cannot be read is a stranger too. Strangers are hung up
            // on and never reach the caller.
            if stream
                .peer_cred()
                .is_ok_and(|peer| peer.uid() == self.owner_uid)
            {
                return Ok(stream);
            }
        }
    }
}

/// The folder of a listener belongs to the user the process runs as and is closed to the
/// group and to others (14, section 11). Gives the user that owns it.
///
/// The folder of another user is refused even when this process may write in it: whoever
/// owns a folder can replace what is in it, the socket included.
fn require_own_private_folder(
    dir: &Path,
    folder: &fs::Metadata,
    user: u32,
) -> Result<u32, LocalSocketError> {
    let owner = folder.uid();
    if owner != user {
        return Err(LocalSocketError::NotOwner {
            dir: dir.to_path_buf(),
            owner,
            user,
        });
    }
    let mode = folder.permissions().mode() & 0o7777;
    if mode & 0o077 != 0 {
        return Err(LocalSocketError::FolderOpenToOthers {
            dir: dir.to_path_buf(),
            mode,
        });
    }
    Ok(owner)
}

/// Removes the socket a dead listener left behind. A socket that still answers, and
/// anything that is not a socket, is left alone and reported.
///
/// Probing, removing and then creating are three steps. They are safe only because the
/// caller holds the lock of the folder, which `LocalListener::bind` checks: nobody else is
/// entitled to create a socket here in the meantime.
async fn remove_dead_socket(dir: &Path, path: &Path) -> Result<(), LocalSocketError> {
    match fs::symlink_metadata(path) {
        Ok(existing) if existing.file_type().is_socket() => {}
        Ok(_) => {
            return Err(LocalSocketError::NotASocket {
                path: path.to_path_buf(),
            });
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(LocalSocketError::io("inspect", path, cause)),
    }
    let alive = || LocalSocketError::AlreadyListening {
        dir: dir.to_path_buf(),
    };
    match UnixStream::connect(path).await {
        Ok(_) => Err(alive()),
        Err(cause) => match cause.kind() {
            // The queue of the listener is full: it exists.
            io::ErrorKind::WouldBlock => Err(alive()),
            // Nobody holds the socket open any more: this is what a dead listener leaves.
            io::ErrorKind::ConnectionRefused => match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(cause) => Err(LocalSocketError::io(
                    "remove the socket left by a dead listener",
                    path,
                    cause,
                )),
            },
            io::ErrorKind::NotFound => Ok(()),
            _ => Err(LocalSocketError::io("probe the socket", path, cause)),
        },
    }
}

/// Closes a socket just created to everyone but its owner, and refuses to serve a socket
/// that does not belong to the owner of the folder: none of the peers there could be told
/// apart from strangers. The folder was checked before the socket was created; this looks
/// at the very file that is about to be served.
fn protect(dir: &Path, path: &Path, owner_uid: u32) -> Result<(), LocalSocketError> {
    fs::set_permissions(path, fs::Permissions::from_mode(SOCKET_MODE))
        .map_err(|cause| LocalSocketError::io("close to other users the socket", path, cause))?;
    // The socket belongs to the user this process runs as.
    let user = fs::symlink_metadata(path)
        .map_err(|cause| LocalSocketError::io("inspect the socket", path, cause))?
        .uid();
    if user != owner_uid {
        return Err(LocalSocketError::NotOwner {
            dir: dir.to_path_buf(),
            owner: owner_uid,
            user,
        });
    }
    Ok(())
}

/// Where a client connects: resolved once, tried as many times as the listener is busy.
#[derive(Debug)]
pub(super) struct Target {
    dir: PathBuf,
    path: PathBuf,
    /// The user that owns the folder: the only one accepted as the listener.
    owner_uid: u32,
}

pub(super) fn resolve(dir: &Path) -> Result<Target, LocalSocketError> {
    let path = socket_path(dir)?;
    match fs::metadata(dir) {
        Ok(folder) => Ok(Target {
            dir: dir.to_path_buf(),
            path,
            owner_uid: folder.uid(),
        }),
        // A folder that does not exist has no listener.
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            Err(LocalSocketError::NoListener {
                dir: dir.to_path_buf(),
            })
        }
        Err(cause) => Err(LocalSocketError::io("inspect the folder", dir, cause)),
    }
}

pub(super) async fn connect_once(target: &Target) -> Result<Attempt, LocalSocketError> {
    let stream = match UnixStream::connect(&target.path).await {
        Ok(stream) => stream,
        Err(cause) => {
            return match cause.kind() {
                // No socket, or a socket that nobody holds open any more.
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => {
                    Err(LocalSocketError::NoListener {
                        dir: target.dir.clone(),
                    })
                }
                // The queue of the listener is full.
                io::ErrorKind::WouldBlock => Ok(Attempt::Busy),
                _ => Err(LocalSocketError::io(
                    "connect to the socket",
                    &target.path,
                    cause,
                )),
            };
        }
    };
    let peer = stream
        .peer_cred()
        .map_err(|cause| {
            LocalSocketError::io(
                "read the user that listens on the socket",
                &target.path,
                cause,
            )
        })?
        .uid();
    if peer != target.owner_uid {
        return Err(LocalSocketError::PeerNotOwner {
            dir: target.dir.clone(),
            owner: target.owner_uid,
            peer,
        });
    }
    Ok(Attempt::Connected(stream))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::AsyncReadExt;

    use super::super::{LocalListener, LocalStream};
    use super::*;
    use crate::test_support::{listen, lock_of, private_tempdir, within};

    fn socket_in(dir: &Path) -> PathBuf {
        dir.join(SOCKET_FILE_NAME)
    }

    /// Leaves a socket file with nobody behind it, like a listener that died.
    fn leave_dead_socket(dir: &Path) {
        drop(std::os::unix::net::UnixListener::bind(socket_in(dir)).unwrap());
        assert!(
            socket_in(dir).exists(),
            "the standard listener keeps its file"
        );
    }

    #[tokio::test]
    async fn socket_is_for_the_owner_only() {
        let root = private_tempdir();

        let (_lock, _listener) = listen(root.path()).await;

        let socket = fs::symlink_metadata(socket_in(root.path())).unwrap();
        assert!(socket.file_type().is_socket());
        assert_eq!(socket.permissions().mode() & 0o7777, 0o600);
        // The file that told which user this process runs as is gone.
        let mut names: Vec<_> = fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, [SOCKET_FILE_NAME, crate::LOCK_FILE_NAME]);
    }

    #[tokio::test]
    async fn socket_left_by_a_dead_listener_is_replaced() {
        let root = private_tempdir();
        leave_dead_socket(root.path());
        let found = within(LocalStream::connect(root.path())).await;
        assert!(
            matches!(found, Err(LocalSocketError::NoListener { .. })),
            "{found:?}"
        );

        let lock = lock_of(root.path());
        let mut listener = within(LocalListener::bind(root.path(), lock.clone()))
            .await
            .unwrap();

        let (accepted, connected) =
            within(async { tokio::join!(listener.accept(), LocalStream::connect(root.path())) })
                .await;
        accepted.unwrap();
        connected.unwrap();
    }

    #[tokio::test]
    async fn something_that_is_not_a_socket_is_left_alone() {
        let root = private_tempdir();
        fs::write(socket_in(root.path()), b"not a socket").unwrap();
        let lock = lock_of(root.path());

        let outcome = LocalListener::bind(root.path(), lock.clone()).await;

        assert!(
            matches!(outcome, Err(LocalSocketError::NotASocket { .. })),
            "{outcome:?}"
        );
        assert_eq!(fs::read(socket_in(root.path())).unwrap(), b"not a socket");
    }

    #[tokio::test]
    async fn link_under_the_socket_name_is_left_alone() {
        let root = private_tempdir();
        let elsewhere = root.path().join("elsewhere.sock");
        drop(std::os::unix::net::UnixListener::bind(&elsewhere).unwrap());
        std::os::unix::fs::symlink(&elsewhere, socket_in(root.path())).unwrap();
        let lock = lock_of(root.path());

        let outcome = LocalListener::bind(root.path(), lock.clone()).await;

        assert!(
            matches!(outcome, Err(LocalSocketError::NotASocket { .. })),
            "{outcome:?}"
        );
        assert!(elsewhere.exists());
    }

    /// The socket that a dead listener left is removed only for the holder of the lock
    /// of the folder. For anybody else it may be the socket of a listener that another
    /// process created an instant ago.
    #[tokio::test]
    async fn socket_is_not_removed_without_the_lock_of_the_folder() {
        let root = private_tempdir();
        let (home, other) = (root.path().join("home"), root.path().join("other"));
        fs::create_dir(&home).unwrap();
        fs::create_dir(&other).unwrap();
        leave_dead_socket(&home);
        let lock_of_another_folder = lock_of(&other);
        // Held while somebody else may hold the lock of the folder, so it proves no more.
        let lock_of_another_file = crate::FileLock::try_acquire(&home.join("hub.json")).unwrap();
        let lock_of_another_file = std::sync::Arc::new(lock_of_another_file);

        for lock in [lock_of_another_folder, lock_of_another_file] {
            let outcome = LocalListener::bind(&home, lock).await;

            assert!(
                matches!(outcome, Err(LocalSocketError::LockElsewhere { .. })),
                "{outcome:?}"
            );
            assert!(
                socket_in(&home).exists(),
                "the socket was removed without the lock"
            );
        }
    }

    /// A lock whose file was removed keeps nobody out, so it proves nothing either.
    #[tokio::test]
    async fn lock_whose_file_was_removed_does_not_let_anybody_listen() {
        let root = private_tempdir();
        leave_dead_socket(root.path());
        let lock = lock_of(root.path());
        fs::remove_file(lock.path()).unwrap();

        let outcome = LocalListener::bind(root.path(), lock.clone()).await;

        assert!(
            matches!(
                &outcome,
                Err(LocalSocketError::LockLost { lock: lost }) if lost.as_path() == lock.path()
            ),
            "{outcome:?}"
        );
        assert!(
            socket_in(root.path()).exists(),
            "the socket was removed without a lock that holds"
        );
    }

    /// In the folder of another user nothing is this process's to remove: the folder is
    /// refused before the socket that a dead listener left there is looked at.
    #[tokio::test]
    async fn socket_in_the_folder_of_another_user_is_not_removed() {
        let root = private_tempdir();
        leave_dead_socket(root.path());
        let owner = fs::metadata(root.path()).unwrap().uid();
        // As if this process ran as somebody else: the folder becomes a stranger's.
        let somebody_else = owner.wrapping_add(1);

        let outcome = Listener::bind_as(root.path(), socket_in(root.path()), somebody_else).await;

        assert!(
            matches!(
                &outcome,
                Err(LocalSocketError::NotOwner { dir, owner: found, user })
                    if dir == root.path() && *found == owner && *user == somebody_else
            ),
            "{outcome:?}"
        );
        assert!(
            socket_in(root.path()).exists(),
            "the socket in the folder of another user was removed"
        );

        // The owner of the folder replaces the dead socket and listens.
        let listener = Listener::bind_as(root.path(), socket_in(root.path()), owner)
            .await
            .unwrap();
        assert_eq!(listener.owner_uid, owner);
    }

    /// The user this process runs as is the one the system reports, not a guess: the
    /// listener of a folder serves the owner of the folder, which is that user.
    #[tokio::test]
    async fn listener_serves_the_user_this_process_runs_as() {
        let root = private_tempdir();

        let listener = Listener::bind(root.path()).await.unwrap();

        assert_eq!(listener.owner_uid, fs::metadata(root.path()).unwrap().uid());
        assert_eq!(
            listener.owner_uid,
            fs::symlink_metadata(socket_in(root.path())).unwrap().uid()
        );
    }

    /// The user of the process is not taken on trust: where the file that tells it cannot
    /// be created, the folder is not listened on.
    #[tokio::test]
    async fn folder_where_the_user_cannot_be_told_is_not_listened_on() {
        let root = private_tempdir();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o500)).unwrap();

        let outcome = Listener::bind(root.path()).await;

        // The administrator creates files anywhere: for it the folder is simply its own.
        if fs::metadata(root.path()).unwrap().uid() == 0 {
            outcome.unwrap();
        } else {
            assert!(
                matches!(
                    &outcome,
                    Err(LocalSocketError::Io {
                        action: "tell which user this process runs as in",
                        ..
                    })
                ),
                "{outcome:?}"
            );
            assert!(!socket_in(root.path()).exists());
        }
        // Lets the temporary folder be removed.
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }

    /// The local socket lives in a folder that only its owner can use (14, section 11).
    /// A folder open to the group or to others is refused as it is found, and the socket
    /// that a dead listener left there stays.
    #[tokio::test]
    async fn folder_open_to_others_is_refused_and_its_socket_is_kept() {
        let root = private_tempdir();
        leave_dead_socket(root.path());
        let lock = lock_of(root.path());

        for open in [0o750, 0o705, 0o770, 0o701, 0o1777] {
            fs::set_permissions(root.path(), fs::Permissions::from_mode(open)).unwrap();

            let outcome = LocalListener::bind(root.path(), lock.clone()).await;

            assert!(
                matches!(
                    &outcome,
                    Err(LocalSocketError::FolderOpenToOthers { dir, mode })
                        if dir == root.path() && *mode == open
                ),
                "{open:o}: {outcome:?}"
            );
            assert!(
                socket_in(root.path()).exists(),
                "{open:o}: the socket was removed from a folder that is not private"
            );
            let mode = fs::metadata(root.path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o7777, open, "the folder was changed");
        }

        // Closed, the same folder is listened on.
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        within(LocalListener::bind(root.path(), lock))
            .await
            .unwrap();
    }

    #[test]
    fn path_at_the_limit_is_taken_and_one_byte_more_is_refused() {
        let fixed = "/".len() + "/".len() + SOCKET_FILE_NAME.len();
        let fits = Path::new("/").join("a".repeat(MAX_SOCKET_PATH_BYTES - fixed));
        let too_long = Path::new("/").join("a".repeat(MAX_SOCKET_PATH_BYTES - fixed + 1));

        let path = socket_path(&fits).unwrap();
        assert_eq!(path.as_os_str().as_bytes().len(), MAX_SOCKET_PATH_BYTES);

        let error = socket_path(&too_long).unwrap_err();
        assert!(
            matches!(
                &error,
                LocalSocketError::PathTooLong { len, max, .. }
                    if *len == MAX_SOCKET_PATH_BYTES + 1 && *max == MAX_SOCKET_PATH_BYTES
            ),
            "{error}"
        );
    }

    #[tokio::test]
    async fn folder_too_deep_is_a_clear_error_for_both_sides() {
        let root = private_tempdir();
        let deep = root.path().join("d".repeat(MAX_SOCKET_PATH_BYTES));
        fs::create_dir(&deep).unwrap();
        let lock = lock_of(&deep);

        let listener = LocalListener::bind(&deep, lock.clone()).await;
        let client = LocalStream::connect(&deep).await;

        assert!(
            matches!(listener, Err(LocalSocketError::PathTooLong { .. })),
            "{listener:?}"
        );
        assert!(
            matches!(client, Err(LocalSocketError::PathTooLong { .. })),
            "{client:?}"
        );
        assert!(!socket_in(&deep).exists());
    }

    /// The listener serves only the user that owns the folder.
    #[tokio::test]
    async fn peer_of_another_user_is_hung_up_on_and_never_served() {
        let root = private_tempdir();
        let mut listener = Listener::bind(root.path()).await.unwrap();
        let owner = listener.owner_uid;
        // As if the folder belonged to somebody else: this process becomes the stranger.
        listener.owner_uid = owner.wrapping_add(1);

        let mut stranger = within(UnixStream::connect(socket_in(root.path())))
            .await
            .unwrap();
        let served = tokio::time::timeout(Duration::from_millis(300), listener.accept()).await;
        assert!(served.is_err(), "a stranger reached the caller");
        let mut byte = [0u8; 1];
        assert_eq!(
            within(stranger.read(&mut byte)).await.unwrap(),
            0,
            "the stranger was kept"
        );

        listener.owner_uid = owner;
        let (accepted, connected) = within(async {
            tokio::join!(
                listener.accept(),
                UnixStream::connect(socket_in(root.path()))
            )
        })
        .await;
        accepted.unwrap();
        connected.unwrap();
    }

    /// The client talks only to a listener of the user that owns the folder.
    #[tokio::test]
    async fn client_refuses_a_listener_of_another_user() {
        let root = private_tempdir();
        let _listener = Listener::bind(root.path()).await.unwrap();
        let mut target = resolve(root.path()).unwrap();
        let owner = target.owner_uid;
        // As if the folder belonged to somebody else: the listener becomes the stranger.
        target.owner_uid = owner.wrapping_add(1);

        let outcome = within(connect_once(&target)).await;

        assert!(
            matches!(&outcome, Err(LocalSocketError::PeerNotOwner { peer, .. }) if *peer == owner),
            "{outcome:?}"
        );
        target.owner_uid = owner;
        assert!(matches!(
            within(connect_once(&target)).await,
            Ok(Attempt::Connected(_))
        ));
    }

    /// The second look, at the socket itself: one that does not belong to the owner of the
    /// folder is not served.
    #[test]
    fn socket_that_does_not_belong_to_the_owner_of_the_folder_is_refused() {
        let root = private_tempdir();
        leave_dead_socket(root.path());
        let socket = socket_in(root.path());
        let user = fs::symlink_metadata(&socket).unwrap().uid();

        let refused = protect(root.path(), &socket, user.wrapping_add(1));
        assert!(
            matches!(&refused, Err(LocalSocketError::NotOwner { user: found, .. }) if *found == user),
            "{refused:?}"
        );

        protect(root.path(), &socket, user).unwrap();
        let mode = fs::symlink_metadata(&socket).unwrap().permissions().mode();
        assert_eq!(mode & 0o7777, 0o600);
    }
}
