//! Local socket between the processes of one user on one machine.
//!
//! The socket belongs to a folder: one listener per folder, who must hold the lock of the
//! folder (the file [`LOCK_FILE_NAME`] inside it), and clients find it by the folder
//! alone. On Unix it is a domain socket file inside the folder; on Windows it is a named
//! pipe whose name is derived from the canonical path of the folder.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::durable::parent_of;
use crate::lock::FileLock;

#[cfg(unix)]
#[path = "unix.rs"]
mod imp;
#[cfg(windows)]
#[path = "windows.rs"]
mod imp;

/// The name of the socket file inside the folder, on Unix. Windows creates no file.
pub const SOCKET_FILE_NAME: &str = "channel.sock";

/// The name of the lock file of a folder: the one file whose lock gives the right to
/// listen on the local socket of the folder, on both systems. Every program that shares
/// the folder must take the lock under this very name, spelled like this.
pub const LOCK_FILE_NAME: &str = "lock";

/// How long a client waits before it tries again while the listener is busy.
const BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// How many times a client tries again while the listener is busy: about five seconds.
const BUSY_RETRIES: u32 = 200;

/// What went wrong with the local socket of a folder.
///
/// The message already carries the cause reported by the system, so it is not repeated as
/// an error source.
#[derive(Debug, thiserror::Error)]
pub enum LocalSocketError {
    /// The call was made outside a Tokio runtime.
    #[error("the local socket needs a running Tokio runtime")]
    NoRuntime,

    /// The folder is too deep for a Unix domain socket. Unix only.
    #[error(
        "{}: the path of the local socket has {len} bytes and this system takes at most \
         {max}; use a folder with a shorter path",
        .path.display()
    )]
    PathTooLong {
        /// The socket path that does not fit.
        path: PathBuf,
        /// Its length in bytes.
        len: usize,
        /// The longest path the system takes, in bytes.
        max: usize,
    },

    /// The lock given as the proof of the right to listen is not the lock of the folder:
    /// its file is not the file [`LOCK_FILE_NAME`] directly inside the folder. The lock of
    /// another folder proves nothing, and neither does the lock of another file of this
    /// folder, which can be held while somebody else holds the lock of the folder.
    #[error(
        "{}: the lock {} is not the file `{name}` of this folder, and only the holder of \
         the lock of a folder listens on its local socket",
        .dir.display(),
        .lock.display(),
        name = LOCK_FILE_NAME
    )]
    LockElsewhere {
        /// The folder.
        dir: PathBuf,
        /// The lock file that was given.
        lock: PathBuf,
    },

    /// The lock given as the proof of the right to listen keeps nobody out any more: its
    /// file was removed or replaced after it was locked. Unix only.
    #[error(
        "{}: the lock file was removed or replaced after it was locked, so it no longer \
         proves the right to listen",
        .lock.display()
    )]
    LockLost {
        /// The lock file that was given.
        lock: PathBuf,
    },

    /// Something already listens on the socket of the folder, although the caller holds
    /// its lock. It is one of three things:
    ///
    /// - a listener of this same process that is not dropped yet (on Windows, together
    ///   with every connection it accepted);
    /// - the listener of a process that died an instant ago, when the system released the
    ///   lock before the socket: trying again a moment later works;
    /// - a process that listens without holding the lock. On Unix that is a process of the
    ///   same user that broke the rule. On Windows it may be any local process, because
    ///   the name of the pipe is predictable: see [`LocalListener`].
    #[error(
        "{}: another process already listens on the local socket of this folder",
        .dir.display()
    )]
    AlreadyListening {
        /// The folder.
        dir: PathBuf,
    },

    /// Something that is not a socket has the name of the socket. It is left untouched.
    /// Unix only.
    #[error("{}: exists and is not a socket", .path.display())]
    NotASocket {
        /// The path of the socket.
        path: PathBuf,
    },

    /// The folder belongs to another user than the one this process runs as, so its
    /// socket cannot be served: none of the peers there could be told apart from
    /// strangers. A socket found there stays, and nothing in the folder is changed: the
    /// empty file that told which user this process runs as was created and removed
    /// before the owner was known. Unix only.
    #[error(
        "{}: the folder belongs to user {owner}, and this process runs as user {user}",
        .dir.display()
    )]
    NotOwner {
        /// The folder.
        dir: PathBuf,
        /// The user id that owns the folder.
        owner: u32,
        /// The user id of this process.
        user: u32,
    },

    /// The folder is open to the group or to other users, and the local socket lives only
    /// in a folder that nobody but its owner reads or writes (14, section 11). A socket
    /// found there stays and the folder is left as open as it was;
    /// [`create_private_dir`](crate::create_private_dir) closes it. Unix only.
    #[error(
        "{}: the folder is open to other users (mode {mode:o}); the local socket needs a \
         folder that only its owner can use",
        .dir.display()
    )]
    FolderOpenToOthers {
        /// The folder.
        dir: PathBuf,
        /// The permission bits of the folder.
        mode: u32,
    },

    /// No process listens on the socket of the folder: there is no socket, or what is
    /// there was left by a listener that died.
    #[error("{}: no process listens on the local socket of this folder", .dir.display())]
    NoListener {
        /// The folder.
        dir: PathBuf,
    },

    /// The listener exists and did not take the connection, however many times it was
    /// tried.
    #[error(
        "{}: the listener of the local socket stayed busy for {attempts} attempts",
        .dir.display()
    )]
    Busy {
        /// The folder.
        dir: PathBuf,
        /// How many times the connection was tried.
        attempts: u32,
    },

    /// The process that listens runs as another user than the owner of the folder. The
    /// connection was dropped before anything was sent. Unix only.
    #[error(
        "{}: the folder belongs to user {owner}, and the listener of its local socket runs \
         as user {peer}",
        .dir.display()
    )]
    PeerNotOwner {
        /// The folder.
        dir: PathBuf,
        /// The user id that owns the folder.
        owner: u32,
        /// The user id of the listener.
        peer: u32,
    },

    /// A call to the system failed.
    #[error("could not {action} {}: {cause}", .path.display())]
    Io {
        /// What was being done, as a verb phrase that reads before the path.
        action: &'static str,
        /// The folder, the socket file or the pipe the action was applied to.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },
}

impl LocalSocketError {
    fn io(action: &'static str, path: &Path, cause: io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            cause,
        }
    }
}

/// The result of one attempt to connect.
#[derive(Debug)]
enum Attempt {
    Connected(imp::Stream),
    /// The listener exists and has no room for the connection right now.
    Busy,
}

/// Tokio panics, instead of failing, when a socket is created outside a runtime.
fn require_runtime() -> Result<(), LocalSocketError> {
    tokio::runtime::Handle::try_current()
        .map(drop)
        .map_err(|_| LocalSocketError::NoRuntime)
}

/// Asks a lock whether its file is still under its path: [`FileLock::is_in_place`], apart
/// so that a test can answer for a system that never loses a lock file.
type InPlace = fn(&FileLock) -> io::Result<bool>;

/// Only the holder of the lock of a folder listens on its socket (14, section 11): the
/// file of `lock` must be the file [`LOCK_FILE_NAME`] directly inside `dir`, whatever the
/// spelling of either folder, and must still be the file that was locked.
///
/// The name is compared as it is written. A lock taken under another spelling of the name
/// is refused even where the system would find the same file, which errs on the safe side.
fn require_lock_of(dir: &Path, lock: &FileLock, in_place: InPlace) -> Result<(), LocalSocketError> {
    let folder = fs::canonicalize(dir)
        .map_err(|cause| LocalSocketError::io("resolve the folder", dir, cause))?;
    let lock_folder = fs::canonicalize(parent_of(lock.path())).map_err(|cause| {
        LocalSocketError::io("resolve the folder of the lock", lock.path(), cause)
    })?;
    if lock_folder != folder || lock.path().file_name() != Some(OsStr::new(LOCK_FILE_NAME)) {
        return Err(LocalSocketError::LockElsewhere {
            dir: dir.to_path_buf(),
            lock: lock.path().to_path_buf(),
        });
    }
    let in_place = in_place(lock)
        .map_err(|cause| LocalSocketError::io("inspect the lock", lock.path(), cause))?;
    if !in_place {
        return Err(LocalSocketError::LockLost {
            lock: lock.path().to_path_buf(),
        });
    }
    Ok(())
}

/// The listening side of the local socket of a folder.
///
/// On Unix it is the socket `channel.sock` inside the folder, with mode `0600`, and only
/// processes of the user that owns the folder are served: the user of each peer is read
/// from the kernel, and a stranger is hung up on without reaching the caller.
///
/// On Windows it is a named pipe that must not exist yet and that refuses clients from
/// other machines. That is all the protection there is, and it is why Windows is a
/// development platform only:
///
/// - **Who can connect.** The pipe has the default protection of the system: full access
///   for its creator, for administrators and for the system account, and *read* access for
///   everyone, anonymous users included. A client that opens the pipe only for reading is
///   handed out by [`accept`](Self::accept) like any other: it reads whatever is written
///   to it, and its own writes are refused by the system. By that protection, a process of
///   another local user can be such a client. Narrowing it needs a call that is not
///   available without `unsafe` code.
/// - **Who can listen.** The name of the pipe is derived from the path of the folder and
///   nothing else, so any local process can create the pipe first. The holder of the lock
///   then gets [`LocalSocketError::AlreadyListening`], and the clients of the folder talk
///   to that other process: a client cannot check who is on the other side.
/// - **Which folder.** The name follows the canonical path as the system reports it. The
///   local spellings of one folder (another case, `.` and `..`, either slash) give one
///   name, but the same folder reached through a network path (`\\server\share\...`)
///   gives another one, and so does a folder after it is renamed to another case. The
///   second cannot happen to a live listener: the system refuses to rename a folder, or a
///   folder above it, while the lock file inside it is open, and the listener keeps its
///   lock for as long as it lives.
///
/// Dropping the listener stops the listening at once: no client connects any more, and a
/// client that had connected and was not accepted yet sees its stream end. On Unix the
/// socket file stays, and the next listener removes it. On Windows the name may look busy
/// for an instant longer, to clients and to the next listener.
///
/// The listener keeps the lock that entitled it to listen: the lock is released only when
/// the listener is gone, and after it has stopped listening.
#[derive(Debug)]
pub struct LocalListener {
    // Dropped in this order: the listening stops before the lock can be released.
    inner: imp::Listener,
    lock: Arc<FileLock>,
}

impl LocalListener {
    /// Starts listening on the local socket of the folder `dir`, which must exist.
    ///
    /// `lock` is the proof of the right to listen: the lock of the folder, taken on the
    /// file [`LOCK_FILE_NAME`] directly inside `dir`. The listener keeps its copy of the
    /// `Arc`, so the lock cannot be released while the listener lives, whatever the caller
    /// does with its own copy; a call that fails drops the copy it was given. Finding out
    /// that nobody listens and creating the socket are separate steps, and on Unix a third
    /// one removes the socket file that a dead listener left: without the lock, two
    /// processes that start together could both pass, and one could remove the socket of
    /// the other.
    ///
    /// On Unix the folder must belong to the user this process runs as and be closed to
    /// the group and to others, as [`create_private_dir`](crate::create_private_dir)
    /// leaves it. Both are checked before anything in the folder is removed or served. The
    /// user of the process is found by creating an empty file in the folder and removing
    /// it at once.
    ///
    /// A socket that still has a listener is never taken over: the call fails with
    /// [`LocalSocketError::AlreadyListening`], which also says what that means for the
    /// holder of the lock.
    ///
    /// On Unix the check for a live listener is a connection that is closed at once: a
    /// listener that is alive meets it as a client that leaves without sending anything.
    ///
    /// # Errors
    ///
    /// [`LocalSocketError::LockElsewhere`] when the file of `lock` is not the file
    /// [`LOCK_FILE_NAME`] directly inside `dir`, and [`LocalSocketError::LockLost`] when
    /// that file was removed or replaced after it was locked; nothing in the folder is
    /// touched in either case. [`LocalSocketError::AlreadyListening`] as described; on
    /// Unix also [`LocalSocketError::PathTooLong`], [`LocalSocketError::NotOwner`],
    /// [`LocalSocketError::FolderOpenToOthers`] and [`LocalSocketError::NotASocket`];
    /// [`LocalSocketError::NoRuntime`] outside a runtime; [`LocalSocketError::Io`] for a
    /// failure of the system, which includes a folder that does not exist.
    ///
    /// # Panics
    ///
    /// Tokio panics when the runtime was built without its I/O driver (`enable_io` or
    /// `enable_all`).
    pub async fn bind(dir: &Path, lock: Arc<FileLock>) -> Result<Self, LocalSocketError> {
        Self::bind_checking(dir, lock, FileLock::is_in_place).await
    }

    /// [`bind`](Self::bind), with the question "is the lock file still in place?" apart.
    async fn bind_checking(
        dir: &Path,
        lock: Arc<FileLock>,
        in_place: InPlace,
    ) -> Result<Self, LocalSocketError> {
        require_runtime()?;
        require_lock_of(dir, &lock, in_place)?;
        let inner = imp::Listener::bind(dir).await?;
        Ok(Self { inner, lock })
    }

    /// The lock that entitles this listener to listen. On Unix its holder should ask it
    /// from time to time whether it is still in place ([`FileLock::is_in_place`]).
    pub fn lock(&self) -> &FileLock {
        &self.lock
    }

    /// Waits for the next client.
    ///
    /// A client may be gone by the time it is accepted: its stream just ends. Cancelling
    /// the wait loses no client. An error concerns one attempt only and leaves the listener
    /// usable: wait a moment and call again.
    ///
    /// # Errors
    ///
    /// [`LocalSocketError::Io`] when the system fails to accept.
    pub async fn accept(&mut self) -> Result<LocalStream, LocalSocketError> {
        let inner = self.inner.accept().await?;
        Ok(LocalStream { inner })
    }
}

/// One connection of the local socket, as a stream of bytes in both directions.
///
/// Once the other side is gone, reading returns zero bytes, or an error when the other
/// side left without reading what it was sent. `shutdown` only flushes: a named pipe cannot
/// close one direction, so on both systems the other side sees the end of the stream when
/// this value is dropped, not before.
#[derive(Debug)]
pub struct LocalStream {
    inner: imp::Stream,
}

impl LocalStream {
    /// Connects to the listener of the folder `dir`.
    ///
    /// While the listener exists but has no room for the connection, the attempt is
    /// repeated every 25 milliseconds, for about five seconds. On Unix that takes a full
    /// queue of clients waiting to be accepted. On Windows the listener keeps four
    /// instances of the pipe waiting and replaces each one as it accepts a client: of the
    /// clients that arrive together, four connect at once and the others wait their turn,
    /// 25 milliseconds at a time. Measured on one machine, with one task accepting on a
    /// runtime of eight threads: of 100 clients that arrived together the last one
    /// connected after about half a second, and of 400, after about one second (two
    /// seconds on a runtime of one thread).
    ///
    /// On Unix the listener must run as the user that owns the folder, or the connection
    /// is dropped before anything is sent. Whether the folder belongs to the *caller* is
    /// not checked here: a client pointed at the folder of another user reaches whatever
    /// that user has listening there. A [`LocalListener`] hangs up on it as on any
    /// stranger, and the client only sees its stream end; a program of that user that
    /// listens by other means may serve it. [`create_private_dir`](crate::create_private_dir)
    /// is what refuses such a folder, with an error that says so: the copies of a program
    /// call it before they connect.
    ///
    /// # Errors
    ///
    /// [`LocalSocketError::NoListener`] when nobody listens (which includes a folder that
    /// does not exist); [`LocalSocketError::Busy`] when the listener never took the
    /// connection; on Unix also [`LocalSocketError::PathTooLong`] and
    /// [`LocalSocketError::PeerNotOwner`]; [`LocalSocketError::NoRuntime`] outside a
    /// runtime; [`LocalSocketError::Io`] for a failure of the system.
    ///
    /// # Panics
    ///
    /// Tokio panics when the runtime was built without its I/O and time drivers
    /// (`enable_all`).
    pub async fn connect(dir: &Path) -> Result<Self, LocalSocketError> {
        Self::connect_with(dir, BUSY_RETRY_INTERVAL, BUSY_RETRIES).await
    }

    async fn connect_with(
        dir: &Path,
        retry_interval: Duration,
        retries: u32,
    ) -> Result<Self, LocalSocketError> {
        require_runtime()?;
        let target = imp::resolve(dir)?;
        let mut attempts: u32 = 0;
        loop {
            attempts = attempts.saturating_add(1);
            match imp::connect_once(&target).await? {
                Attempt::Connected(inner) => return Ok(Self { inner }),
                Attempt::Busy if attempts > retries => {
                    return Err(LocalSocketError::Busy {
                        dir: dir.to_path_buf(),
                        attempts,
                    });
                }
                Attempt::Busy => tokio::time::sleep(retry_interval).await,
            }
        }
    }
}

impl AsyncRead for LocalStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for LocalStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Flush only, on both systems: see the documentation of the type.
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    use super::*;
    use crate::test_support::{
        CHILD_READY, ChildGuard, child_role, listen, lock_left_by_a_dead_process, lock_of,
        private_tempdir, within,
    };
    use crate::{LockError, create_private_dir};

    /// Role of the second process that listens on a folder until it is killed.
    const ROLE_LISTEN: &str = "listen";
    /// Where the second process finds the folder.
    const FOLDER_VARIABLE: &str = "ARAUTO_PLATFORM_TEST_SOCKET_FOLDER";

    /// Accepts one client while a new client connects, and returns both ends.
    async fn connected_pair(
        listener: &mut LocalListener,
        dir: &Path,
    ) -> (LocalStream, LocalStream) {
        let (accepted, connected) =
            within(async { tokio::join!(listener.accept(), LocalStream::connect(dir)) }).await;
        (accepted.unwrap(), connected.unwrap())
    }

    /// Answers each line with the same line in upper case, until the client leaves.
    async fn shout_back(stream: LocalStream) {
        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let answer = format!("{}\n", line.to_uppercase());
            if writer.write_all(answer.as_bytes()).await.is_err() {
                break;
            }
        }
    }

    /// Serves every client of the listener, each in its own task.
    fn serve(mut listener: LocalListener) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let stream = listener.accept().await.unwrap();
                tokio::spawn(shout_back(stream));
            }
        })
    }

    /// Connects, sends `count` lines, one at a time, and checks each answer.
    async fn talk(dir: PathBuf, who: &str, count: usize) {
        let stream = LocalStream::connect(&dir).await.unwrap();
        exchange(stream, who, count).await;
    }

    /// Sends `count` lines over `stream`, one at a time, and checks each answer.
    async fn exchange(stream: LocalStream, who: &str, count: usize) {
        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();
        for number in 0..count {
            let line = format!("{{\"v\":1,\"from\":\"{who}\",\"n\":{number}}}");
            writer
                .write_all(format!("{line}\n").as_bytes())
                .await
                .unwrap();
            let answer = lines.next_line().await.unwrap();
            assert_eq!(answer, Some(line.to_uppercase()));
        }
    }

    /// Sends `message` one way while the other side reads it, and compares.
    async fn cross(from: &mut LocalStream, to: &mut LocalStream, message: &[u8]) {
        let mut received = vec![0u8; message.len()];
        let (sent, read) = tokio::join!(from.write_all(message), to.read_exact(&mut received));
        sent.unwrap();
        read.unwrap();
        assert!(received == message, "the bytes arrived changed");
    }

    #[tokio::test]
    async fn several_lines_go_and_come_back() {
        let root = private_tempdir();
        let (_lock, listener) = listen(root.path()).await;
        let server = serve(listener);

        within(talk(root.path().to_path_buf(), "one", 25)).await;

        server.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn two_clients_talk_at_the_same_time() {
        let root = private_tempdir();
        let (_lock, listener) = listen(root.path()).await;
        let server = serve(listener);

        // Neither client sends a line before both are connected.
        let both_connected = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let client = |who: &'static str| {
            let dir = root.path().to_path_buf();
            let both_connected = both_connected.clone();
            tokio::spawn(async move {
                let stream = LocalStream::connect(&dir).await.unwrap();
                both_connected.wait().await;
                exchange(stream, who, 200).await;
            })
        };
        let (first, second) = (client("first"), client("second"));
        within(first).await.unwrap();
        within(second).await.unwrap();

        server.abort();
    }

    #[tokio::test]
    async fn two_connections_are_open_side_by_side() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        let (mut first_server, mut first_client) = connected_pair(&mut listener, root.path()).await;
        let (mut second_server, mut second_client) =
            connected_pair(&mut listener, root.path()).await;

        // Interleaved on purpose: each connection carries its own bytes.
        second_client.write_all(b"second\n").await.unwrap();
        first_client.write_all(b"first\n").await.unwrap();
        let mut from_first = [0u8; 6];
        let mut from_second = [0u8; 7];
        within(first_server.read_exact(&mut from_first))
            .await
            .unwrap();
        within(second_server.read_exact(&mut from_second))
            .await
            .unwrap();

        assert_eq!(&from_first, b"first\n");
        assert_eq!(&from_second, b"second\n");
    }

    #[tokio::test]
    async fn large_message_crosses_whole_in_both_directions() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        let (mut server, mut client) = connected_pair(&mut listener, root.path()).await;
        // Larger than any buffer of the system, so both sides must take turns.
        let message: Vec<u8> = (0..1_000_000u32).map(|n| (n % 251) as u8).collect();

        within(cross(&mut client, &mut server, &message)).await;
        within(cross(&mut server, &mut client, &message)).await;
    }

    #[tokio::test]
    async fn second_listener_in_the_same_folder_fails() {
        let root = private_tempdir();
        let (lock, mut first) = listen(root.path()).await;

        let second = within(LocalListener::bind(root.path(), lock.clone())).await;

        assert!(
            matches!(&second, Err(LocalSocketError::AlreadyListening { dir }) if dir == root.path()),
            "{second:?}"
        );
        // The first one keeps serving. On Unix the listener that was refused had to knock
        // to learn that somebody was there: the first one meets that visit as a client that
        // left without a word, before the client that talks.
        let mut client = within(LocalStream::connect(root.path())).await.unwrap();
        client.write_all(b"still there?").await.unwrap();
        let mut server = within(async {
            loop {
                let mut accepted = first.accept().await.unwrap();
                let mut question = [0u8; 12];
                if accepted.read_exact(&mut question).await.is_ok() {
                    assert_eq!(&question, b"still there?");
                    break accepted;
                }
            }
        })
        .await;
        within(cross(&mut server, &mut client, b"still here")).await;
    }

    #[tokio::test]
    async fn listeners_of_different_folders_do_not_meet() {
        let root = private_tempdir();
        let (one, other) = (root.path().join("one"), root.path().join("other"));
        create_private_dir(&one).unwrap();
        create_private_dir(&other).unwrap();
        let (_lock_one, mut listener_one) = listen(&one).await;
        let (_lock_other, mut listener_other) = listen(&other).await;

        let (mut server, mut client) = connected_pair(&mut listener_other, &other).await;
        within(cross(&mut client, &mut server, b"x")).await;

        let nobody = tokio::time::timeout(Duration::from_millis(100), listener_one.accept()).await;
        assert!(
            nobody.is_err(),
            "the client of one folder reached the other"
        );
    }

    #[tokio::test]
    async fn folder_without_a_listener_says_so() {
        let root = private_tempdir();

        let empty = within(LocalStream::connect(root.path())).await;
        let missing = within(LocalStream::connect(&root.path().join("missing"))).await;

        assert!(
            matches!(&empty, Err(LocalSocketError::NoListener { dir }) if dir == root.path()),
            "{empty:?}"
        );
        assert!(
            matches!(missing, Err(LocalSocketError::NoListener { .. })),
            "{missing:?}"
        );
    }

    #[tokio::test]
    async fn folder_that_does_not_exist_cannot_be_listened_on() {
        let root = private_tempdir();
        let lock = lock_of(root.path());

        let outcome = within(LocalListener::bind(
            &root.path().join("missing"),
            lock.clone(),
        ))
        .await;

        assert!(
            matches!(outcome, Err(LocalSocketError::Io { .. })),
            "{outcome:?}"
        );
    }

    /// The rule of the leader (14, section 11): only the holder of the lock of the folder
    /// listens on its socket. The lock of any other folder proves nothing.
    #[tokio::test]
    async fn lock_of_another_folder_does_not_let_anybody_listen() {
        let root = private_tempdir();
        let (home, other) = (root.path().join("home"), root.path().join("other"));
        create_private_dir(&home).unwrap();
        create_private_dir(&other).unwrap();
        let lock_of_a_sibling = lock_of(&other);
        let lock_of_the_folder_above = lock_of(root.path());

        for lock in [&lock_of_a_sibling, &lock_of_the_folder_above] {
            let outcome = within(LocalListener::bind(&home, lock.clone())).await;

            assert!(
                matches!(
                    &outcome,
                    Err(LocalSocketError::LockElsewhere { dir, lock: given })
                        if *dir == home && given.as_path() == lock.path()
                ),
                "{outcome:?}"
            );
        }
        // Nothing was created in the folder, and nobody listens on it.
        assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
        let nobody = within(LocalStream::connect(&home)).await;
        assert!(
            matches!(nobody, Err(LocalSocketError::NoListener { .. })),
            "{nobody:?}"
        );
    }

    /// The lock of the folder is one file, [`LOCK_FILE_NAME`]. The lock of any other file
    /// of the folder can be held while somebody else holds the lock of the folder, so it
    /// proves nothing either.
    #[tokio::test]
    async fn lock_of_another_file_of_the_folder_does_not_let_anybody_listen() {
        let root = private_tempdir();
        let holder = lock_of(root.path());

        for name in ["hub.json", "join.lock", "lock.old", "lock2", ".lock"] {
            let another = FileLock::try_acquire(&root.path().join(name)).unwrap();
            let another = Arc::new(another);

            let outcome = within(LocalListener::bind(root.path(), another.clone())).await;

            assert!(
                matches!(
                    &outcome,
                    Err(LocalSocketError::LockElsewhere { dir, lock: given })
                        if dir == root.path() && given.as_path() == another.path()
                ),
                "{name}: {outcome:?}"
            );
        }
        // No socket was created, and nobody listens on the folder.
        assert!(!root.path().join(SOCKET_FILE_NAME).exists());
        let nobody = within(LocalStream::connect(root.path())).await;
        assert!(
            matches!(nobody, Err(LocalSocketError::NoListener { .. })),
            "{nobody:?}"
        );

        // The holder of the lock of the folder is the one that listens.
        let mut listener = within(LocalListener::bind(root.path(), holder))
            .await
            .unwrap();
        let (mut server, mut client) = connected_pair(&mut listener, root.path()).await;
        within(cross(&mut client, &mut server, b"x")).await;
    }

    /// The name of the lock file is taken as it is written: another spelling is refused
    /// even on a system that would find the same file under it.
    #[tokio::test]
    async fn lock_under_another_spelling_of_its_name_is_refused() {
        let root = private_tempdir();
        let lock = FileLock::try_acquire(&root.path().join("LOCK")).unwrap();

        let outcome = within(LocalListener::bind(root.path(), Arc::new(lock))).await;

        assert!(
            matches!(outcome, Err(LocalSocketError::LockElsewhere { .. })),
            "{outcome:?}"
        );
    }

    /// The listener keeps the lock that let it listen. Its holder cannot let go of the
    /// lock and go on listening while another process becomes the holder.
    #[tokio::test]
    async fn lock_is_held_for_as_long_as_the_listener_lives() {
        let root = private_tempdir();
        let (lock, listener) = listen(root.path()).await;
        let path = lock.path().to_path_buf();
        assert!(std::ptr::eq(listener.lock(), Arc::as_ptr(&lock)));

        drop(lock);
        let while_listening = FileLock::try_acquire(&path);
        assert!(
            matches!(while_listening, Err(LockError::AlreadyHeld { .. })),
            "{while_listening:?}"
        );

        drop(listener);
        let afterwards = FileLock::try_acquire(&path);
        assert!(afterwards.is_ok(), "{afterwards:?}");
    }

    /// A lock whose file is no longer under its path keeps nobody out, so it proves
    /// nothing. Only Unix can lose a lock file, and it has a test with a file that is
    /// really removed; here the rule is checked on both systems, by answering for the
    /// lock.
    #[tokio::test]
    async fn lock_that_is_no_longer_in_place_does_not_let_anybody_listen() {
        let root = private_tempdir();
        let lock = lock_of(root.path());

        let lost = within(LocalListener::bind_checking(
            root.path(),
            lock.clone(),
            |_| Ok(false),
        ))
        .await;
        let unknown = within(LocalListener::bind_checking(
            root.path(),
            lock.clone(),
            |_| Err(io::Error::other("the test could not look")),
        ))
        .await;

        assert!(
            matches!(
                &lost,
                Err(LocalSocketError::LockLost { lock: given }) if given.as_path() == lock.path()
            ),
            "{lost:?}"
        );
        assert!(
            matches!(
                &unknown,
                Err(LocalSocketError::Io {
                    action: "inspect the lock",
                    ..
                })
            ),
            "{unknown:?}"
        );
        // No socket was created, and nobody listens on the folder.
        assert!(!root.path().join(SOCKET_FILE_NAME).exists());
        let nobody = within(LocalStream::connect(root.path())).await;
        assert!(
            matches!(nobody, Err(LocalSocketError::NoListener { .. })),
            "{nobody:?}"
        );

        // With its file in place, the same lock lets its holder listen.
        within(LocalListener::bind(root.path(), lock))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn lock_of_the_folder_is_recognised_under_another_spelling() {
        let root = private_tempdir();
        let (home, other) = (root.path().join("home"), root.path().join("other"));
        create_private_dir(&home).unwrap();
        create_private_dir(&other).unwrap();
        let lock = lock_of(&other.join("..").join("home"));

        let mut listener = within(LocalListener::bind(&home.join("."), lock.clone()))
            .await
            .unwrap();

        let (mut server, mut client) = connected_pair(&mut listener, &home).await;
        within(cross(&mut client, &mut server, b"x")).await;
    }

    /// What happens when the first copy dies: the next one must be able to listen, even
    /// while clients of the dead one still hold their connections.
    #[tokio::test]
    async fn folder_is_free_again_after_the_listener_is_gone() {
        let root = private_tempdir();
        let (lock, mut first) = listen(root.path()).await;
        let (mut old_server, mut old_client) = connected_pair(&mut first, root.path()).await;
        // Everything the listening side held goes away, as when its process ends.
        old_server.write_all(b"bye\n").await.unwrap();
        drop(old_server);
        drop(first);

        // Windows frees the name a moment after the last handle of that side is dropped.
        let mut second = LocalListener::bind(root.path(), lock.clone()).await;
        for _ in 0..250 {
            if !matches!(second, Err(LocalSocketError::AlreadyListening { .. })) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            second = LocalListener::bind(root.path(), lock.clone()).await;
        }
        let mut second = second.unwrap();

        // The client of the old listener reads what was sent to it and then the end.
        let mut rest = Vec::new();
        within(old_client.read_to_end(&mut rest)).await.unwrap();
        assert_eq!(rest, b"bye\n");
        // New clients reach the new listener.
        let (mut server, mut client) = connected_pair(&mut second, root.path()).await;
        within(cross(&mut client, &mut server, b"new listener")).await;
    }

    /// The real arrangement: the listener is another process of the same user, and when it
    /// dies with clients connected, the next process takes its place.
    #[test]
    fn processes_talk_and_another_takes_over_when_the_listener_dies() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        if child_role().as_deref() == Some(ROLE_LISTEN) {
            let folder = PathBuf::from(std::env::var_os(FOLDER_VARIABLE).unwrap());
            runtime.block_on(async {
                let (_lock, listener) = listen(&folder).await;
                println!("{CHILD_READY}");
                // Serves until the parent kills this process.
                let _ = serve(listener).await;
            });
            return;
        }

        let root = private_tempdir();
        let mut listening_process = ChildGuard::spawn(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "local_socket::tests::processes_talk_and_another_takes_over_when_the_listener_dies",
                    "--exact",
                    "--nocapture",
                ])
                .env(FOLDER_VARIABLE, root.path())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
            ROLE_LISTEN,
        );
        let mut output = std::io::BufReader::new(listening_process.stdout());
        let mut line = String::new();
        while line.trim_end() != CHILD_READY {
            line.clear();
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "the other process never listened"
            );
        }

        runtime.block_on(async {
            within(talk(root.path().to_path_buf(), "another process", 25)).await;

            // While the listener lives, nobody else listens on its folder: the lock is
            // taken, and the system refuses a second listener even to a process that
            // skips the lock.
            let second_copy = FileLock::try_acquire(&root.path().join("lock"));
            assert!(
                matches!(second_copy, Err(LockError::AlreadyHeld { .. })),
                "{second_copy:?}"
            );
            let intruder = within(imp::Listener::bind(root.path())).await;
            assert!(
                matches!(intruder, Err(LocalSocketError::AlreadyListening { .. })),
                "{intruder:?}"
            );
            within(talk(root.path().to_path_buf(), "still served", 3)).await;

            // The listener dies with a client connected.
            let mut orphan = within(LocalStream::connect(root.path())).await.unwrap();
            listening_process.kill();
            listening_process.wait();
            // The client learns it: the stream ends, cleanly or with an error.
            let mut rest = Vec::new();
            let _ = within(orphan.read_to_end(&mut rest)).await;
            assert!(rest.is_empty());
            let nobody = within(LocalStream::connect(root.path())).await;
            assert!(
                matches!(nobody, Err(LocalSocketError::NoListener { .. })),
                "{nobody:?}"
            );

            // The next process takes the lock that the system released and listens on
            // the same folder, with the orphan still open.
            let lock = lock_left_by_a_dead_process(root.path());
            let mut successor = within(LocalListener::bind(root.path(), lock.clone()))
                .await
                .unwrap();
            let (mut server, mut client) = connected_pair(&mut successor, root.path()).await;
            within(cross(&mut client, &mut server, b"served again")).await;
            drop(orphan);
        });
    }

    #[tokio::test]
    async fn listener_that_went_away_leaves_no_listener() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        let (server, _old_client) = connected_pair(&mut listener, root.path()).await;
        drop(server);
        drop(listener);

        // On Windows the name may look busy for an instant, which the client waits out.
        let found = within(LocalStream::connect(root.path())).await;

        assert!(
            matches!(found, Err(LocalSocketError::NoListener { .. })),
            "{found:?}"
        );
    }

    /// Dropping the listener stops the listening at that moment, also when it was in the
    /// middle of waiting for a client: nobody connects to what is left of it, not even
    /// before the runtime runs again.
    #[tokio::test]
    async fn listener_dropped_while_it_waited_for_a_client_takes_no_more_clients() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        let nobody = tokio::time::timeout(Duration::from_millis(50), listener.accept()).await;
        assert!(nobody.is_err());
        drop(listener);

        // One attempt, made before anything else runs.
        let at_once = LocalStream::connect_with(root.path(), Duration::ZERO, 0).await;
        let later = within(LocalStream::connect(root.path())).await;

        // On Windows the name may still look busy at the first attempt; it never connects.
        assert!(
            matches!(
                at_once,
                Err(LocalSocketError::NoListener { .. } | LocalSocketError::Busy { .. })
            ),
            "{at_once:?}"
        );
        assert!(
            matches!(later, Err(LocalSocketError::NoListener { .. })),
            "{later:?}"
        );
    }

    /// A client that connected and was never accepted is not left waiting for a listener
    /// that went away: its stream ends.
    #[tokio::test]
    async fn client_that_was_never_accepted_learns_that_the_listener_went_away() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        // The listener was waiting, and gave up the wait before the client came.
        let nobody = tokio::time::timeout(Duration::from_millis(50), listener.accept()).await;
        assert!(nobody.is_err());
        let mut never_accepted = within(LocalStream::connect(root.path())).await.unwrap();

        drop(listener);

        // Cleanly or with an error, and with nothing to read.
        let mut rest = Vec::new();
        let _ = within(never_accepted.read_to_end(&mut rest)).await;
        assert!(rest.is_empty());
    }

    #[tokio::test]
    async fn client_that_leaves_at_once_does_not_break_the_listener() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;

        drop(within(LocalStream::connect(root.path())).await.unwrap());
        let mut gone = within(listener.accept()).await.unwrap();
        let mut rest = Vec::new();
        assert_eq!(within(gone.read_to_end(&mut rest)).await.unwrap(), 0);

        let (mut server, mut client) = connected_pair(&mut listener, root.path()).await;
        within(cross(&mut client, &mut server, b"next client")).await;
    }

    #[tokio::test]
    async fn waiting_for_a_client_can_be_cancelled_without_losing_one() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;

        let nobody = tokio::time::timeout(Duration::from_millis(50), listener.accept()).await;
        assert!(nobody.is_err());

        let mut client = within(LocalStream::connect(root.path())).await.unwrap();
        client.write_all(b"x").await.unwrap();
        let mut server = within(listener.accept()).await.unwrap();
        let mut byte = [0u8; 1];
        within(server.read_exact(&mut byte)).await.unwrap();
        assert_eq!(&byte, b"x");
    }

    #[tokio::test]
    async fn shutdown_flushes_and_keeps_the_stream_open() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        let (mut server, mut client) = connected_pair(&mut listener, root.path()).await;

        client.write_all(b"question\n").await.unwrap();
        client.shutdown().await.unwrap();

        // The same on both systems: the other side still answers on the same stream.
        let mut question = [0u8; 9];
        within(server.read_exact(&mut question)).await.unwrap();
        within(cross(&mut server, &mut client, b"answer\n")).await;
        let more = tokio::time::timeout(Duration::from_millis(100), server.read(&mut question));
        assert!(
            more.await.is_err(),
            "shutdown ended the stream for the other side"
        );
    }

    #[test]
    fn outside_a_runtime_it_is_an_error_and_not_a_panic() {
        let root = private_tempdir();
        let lock = lock_of(root.path());
        let mut cx = Context::from_waker(std::task::Waker::noop());

        let bind = std::pin::pin!(LocalListener::bind(root.path(), lock.clone()));
        let connect = std::pin::pin!(LocalStream::connect(root.path()));

        assert!(matches!(
            bind.poll(&mut cx),
            Poll::Ready(Err(LocalSocketError::NoRuntime))
        ));
        assert!(matches!(
            connect.poll(&mut cx),
            Poll::Ready(Err(LocalSocketError::NoRuntime))
        ));
    }
}
