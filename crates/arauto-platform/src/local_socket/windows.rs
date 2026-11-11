//! The local socket on Windows: a named pipe whose name comes from the folder.
//!
//! Windows is a development platform only, and this is the weakest piece of it. The pipe
//! gets the default protection of the system, which lets everyone connect for reading; its
//! name can be worked out, and taken, by any local process; and a client cannot check who
//! listens. The documentation of `LocalListener` spells out the three limits.

use std::fs;
use std::io;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use data_encoding::BASE32_NOPAD;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

use super::{Attempt, LocalSocketError};

/// Where the pipes of Arauto live in the namespace of the system.
const PIPE_NAME_PREFIX: &str = r"\\.\pipe\arauto-";

/// `ERROR_PIPE_BUSY`: every instance of the pipe is taken by another client.
const ERROR_PIPE_BUSY: i32 = 231;

/// The name of the pipe of a folder.
///
/// The folder is first resolved to its canonical path, so the local spellings of one
/// folder (another case, `.` and `..`, either slash) give one name; two folders differ in
/// that path, so they give two names.
///
/// The name is only as stable as that path. The same folder reached through a network
/// path has another canonical path, hence another name. And the canonical path carries
/// the case stored on disk, so renaming the folder, or a folder above it, to another case
/// changes the name. A listener is safe from the second: the lock file that it keeps open
/// inside the folder makes the system refuse those renames.
fn pipe_name(dir: &Path) -> io::Result<String> {
    fs::canonicalize(dir).map(|canonical| pipe_name_of_canonical(&canonical))
}

/// `arauto-` followed by the lowercase unpadded base32 of the SHA-256 of the canonical
/// path, taken as UTF-16 code units in little-endian order.
///
/// Every version of the program must derive the same name, or its copies stop finding
/// each other: this is a contract, pinned by a test.
fn pipe_name_of_canonical(canonical: &Path) -> String {
    let mut hasher = Sha256::new();
    for unit in canonical.as_os_str().encode_wide() {
        hasher.update(unit.to_le_bytes());
    }
    let mut name = BASE32_NOPAD.encode(&hasher.finalize());
    name.make_ascii_lowercase();
    name.insert_str(0, PIPE_NAME_PREFIX);
    name
}

/// How every instance of the pipe is created. No instance accepts a client from another
/// machine; the first one also demands that the name is not in use.
fn server_options(first_instance: bool) -> ServerOptions {
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first_instance)
        .reject_remote_clients(true);
    options
}

/// Creates one instance of the pipe `name`. Every instance is created here, so that none
/// is created with other options than those of [`server_options`].
fn create_instance(name: &str, first_instance: bool) -> io::Result<NamedPipeServer> {
    let options = server_options(first_instance);
    #[cfg(test)]
    tests::before_instance(&options)?;
    options.create(name)
}

fn pipe_error(action: &'static str, name: &str, cause: io::Error) -> LocalSocketError {
    LocalSocketError::Io {
        action,
        path: PathBuf::from(name),
        cause,
    }
}

/// How many instances of the pipe wait for a client at any time. A client connects to an
/// instance without the listener doing anything, so this is how many clients that arrive
/// together connect at once; the others find the pipe busy and try again a moment later.
const WAITING_INSTANCES: usize = 4;

#[derive(Debug)]
pub(super) struct Listener {
    name: String,
    /// The instances the next clients connect to. All of them are always there, so that
    /// the name never disappears while the listener lives.
    waiting: [NamedPipeServer; WAITING_INSTANCES],
}

impl Listener {
    // Asynchronous only to match the Unix implementation.
    pub(super) async fn bind(dir: &Path) -> Result<Self, LocalSocketError> {
        let name = pipe_name(dir)
            .map_err(|cause| LocalSocketError::io("resolve the folder", dir, cause))?;
        let first = create_instance(&name, true).map_err(|cause| match cause.kind() {
            // What the system answers when the name already has an instance.
            io::ErrorKind::PermissionDenied => LocalSocketError::AlreadyListening {
                dir: dir.to_path_buf(),
            },
            _ => pipe_error("create the named pipe", &name, cause),
        })?;
        let another = || next_instance(&name);
        let waiting = [first, another()?, another()?, another()?];
        Ok(Self { name, waiting })
    }

    pub(super) async fn accept(&mut self) -> Result<Stream, LocalSocketError> {
        let [first, second, third, fourth] = &mut self.waiting;
        // Whichever instance gets a client first. A wait that is dropped loses no client:
        // the instance keeps it, and the next wait on that instance finds it.
        let (connected, outcome) = tokio::select! {
            outcome = first.connect() => (first, outcome),
            outcome = second.connect() => (second, outcome),
            outcome = third.connect() => (third, outcome),
            outcome = fourth.connect() => (fourth, outcome),
        };
        outcome
            .map_err(|cause| pipe_error("accept a client on the named pipe", &self.name, cause))?;
        // The next instance exists before the connected one is handed out. If it cannot be
        // created, the connected client stays where it is and the next call takes it.
        let next = next_instance(&self.name)?;
        Ok(Stream::Accepted(mem::replace(connected, next)))
    }
}

impl Drop for Listener {
    /// Stops the listening at once.
    ///
    /// Dropping an instance does not close it at once: the wait for a client that is still
    /// in progress on it must first be cancelled, and the runtime only sees that later.
    /// Until then the instance would still take a client, and that client would be left
    /// connected to nobody, for as long as this process lives. An instance that is
    /// disconnected takes no client, and the client it already had learns at once that
    /// nobody is there.
    fn drop(&mut self) {
        for instance in &self.waiting {
            // Best effort: an instance that cannot be disconnected is still closed, later.
            let _ = instance.disconnect();
        }
    }
}

/// Creates one more instance of a pipe that already has its first one.
fn next_instance(name: &str) -> Result<NamedPipeServer, LocalSocketError> {
    create_instance(name, false)
        .map_err(|cause| pipe_error("create an instance of the named pipe", name, cause))
}

/// One end of a connection: the two ends are different types on Windows.
#[derive(Debug)]
pub(super) enum Stream {
    Accepted(NamedPipeServer),
    Connected(NamedPipeClient),
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Accepted(pipe) => Pin::new(pipe).poll_read(cx, buf),
            Self::Connected(pipe) => Pin::new(pipe).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Accepted(pipe) => Pin::new(pipe).poll_write(cx, buf),
            Self::Connected(pipe) => Pin::new(pipe).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Accepted(pipe) => Pin::new(pipe).poll_flush(cx),
            Self::Connected(pipe) => Pin::new(pipe).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Accepted(pipe) => Pin::new(pipe).poll_shutdown(cx),
            Self::Connected(pipe) => Pin::new(pipe).poll_shutdown(cx),
        }
    }
}

/// Where a client connects: resolved once, tried as many times as the pipe is busy.
#[derive(Debug)]
pub(super) struct Target {
    dir: PathBuf,
    name: String,
}

pub(super) fn resolve(dir: &Path) -> Result<Target, LocalSocketError> {
    match pipe_name(dir) {
        Ok(name) => Ok(Target {
            dir: dir.to_path_buf(),
            name,
        }),
        // A folder that does not exist has no listener.
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            Err(LocalSocketError::NoListener {
                dir: dir.to_path_buf(),
            })
        }
        Err(cause) => Err(LocalSocketError::io("resolve the folder", dir, cause)),
    }
}

// Asynchronous only to match the Unix implementation.
pub(super) async fn connect_once(target: &Target) -> Result<Attempt, LocalSocketError> {
    match ClientOptions::new().open(&target.name) {
        Ok(pipe) => Ok(Attempt::Connected(Stream::Connected(pipe))),
        Err(cause) if cause.raw_os_error() == Some(ERROR_PIPE_BUSY) => Ok(Attempt::Busy),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            Err(LocalSocketError::NoListener {
                dir: target.dir.clone(),
            })
        }
        Err(cause) => Err(pipe_error("connect to the named pipe", &target.name, cause)),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::super::{LocalListener, LocalStream};
    use super::*;
    use crate::test_support::{listen, lock_of, private_tempdir, within};

    thread_local! {
        /// The options of each instance of a pipe created by this thread, once a test
        /// asked for them with [`record_instances`].
        static INSTANCES: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };

        /// Whether this thread is refused every new instance of a pipe, as when the system
        /// has no room for one more.
        static INSTANCES_REFUSED: Cell<bool> = const { Cell::new(false) };
    }

    /// Makes the creation of instances by this thread fail, or work again.
    fn refuse_instances(refused: bool) {
        INSTANCES_REFUSED.with(|slot| slot.set(refused));
    }

    /// Starts recording the options of the instances created by this thread.
    fn record_instances() {
        INSTANCES.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
    }

    /// Stops recording, and gives what was recorded. The options are private to Tokio, so
    /// each record is their debug form.
    fn recorded_instances() -> Vec<String> {
        INSTANCES
            .with(|slot| slot.borrow_mut().take())
            .unwrap_or_default()
    }

    /// Called before the creation of every instance of a pipe: records its options, and
    /// fails when the test asked for that with [`refuse_instances`].
    pub(super) fn before_instance(options: &ServerOptions) -> io::Result<()> {
        if INSTANCES_REFUSED.with(Cell::get) {
            return Err(io::Error::other("the test refused one more instance"));
        }
        INSTANCES.with(|slot| {
            if let Some(recorded) = slot.borrow_mut().as_mut() {
                recorded.push(format!("{options:?}"));
            }
        });
        Ok(())
    }

    /// Connects one client to each instance that waits, with the listener accepting none:
    /// from then on, and until the listener accepts, the pipe is busy.
    async fn take_every_waiting_instance(dir: &Path) -> Vec<LocalStream> {
        let mut clients = Vec::new();
        for _ in 0..WAITING_INSTANCES {
            // No second attempt: each of them finds an instance at once.
            let client = LocalStream::connect_with(dir, Duration::ZERO, 0).await;
            clients.push(client.unwrap());
        }
        clients
    }

    /// The derivation is a contract between versions. These values were computed apart
    /// from this code: SHA-256 of the UTF-16LE path, base32 without padding, lowercase.
    #[test]
    fn name_derivation_is_pinned() {
        assert_eq!(
            pipe_name_of_canonical(Path::new(r"\\?\C:\Users\hermes\.arauto")),
            r"\\.\pipe\arauto-skn2tq2o4jz64h5hjmtpc4kaf45pilfbsu7algw36r4c5g7tubmq"
        );
        assert_eq!(
            pipe_name_of_canonical(Path::new("\\\\?\\C:\\Users\\herm\u{e9}s\\.arauto")),
            r"\\.\pipe\arauto-s6d3mhn3t6izp2gvylyidndzru5iofdbsr57pc4w2u7ajvigazqq"
        );
    }

    #[test]
    fn different_folders_get_different_names() {
        let root = private_tempdir();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let nested = first.join("first");
        for dir in [&first, &second, &nested] {
            fs::create_dir(dir).unwrap();
        }

        let names = [
            pipe_name(&first).unwrap(),
            pipe_name(&second).unwrap(),
            pipe_name(&nested).unwrap(),
            pipe_name(root.path()).unwrap(),
        ];

        for (index, name) in names.iter().enumerate() {
            assert!(name.starts_with(r"\\.\pipe\arauto-"), "{name}");
            assert_eq!(name.len(), r"\\.\pipe\arauto-".len() + 52, "{name}");
            for other in names.iter().skip(index + 1) {
                assert_ne!(name, other);
            }
        }
    }

    #[test]
    fn local_spellings_of_one_folder_get_the_same_name() {
        let root = private_tempdir();
        let dir = root.path().join("Arauto Home");
        fs::create_dir(&dir).unwrap();
        fs::create_dir(root.path().join("other")).unwrap();
        let expected = pipe_name(&dir).unwrap();

        // `C:\...` written as `c:\...`, or the other way around.
        let other_drive_case: String = dir
            .display()
            .to_string()
            .char_indices()
            .map(|(index, letter)| match index {
                0 if letter.is_ascii_lowercase() => letter.to_ascii_uppercase(),
                0 => letter.to_ascii_lowercase(),
                _ => letter,
            })
            .collect();
        let spellings = [
            root.path().join("arauto home"),
            root.path().join("ARAUTO HOME"),
            root.path().join("Arauto Home").join("."),
            root.path().join("other").join("..").join("Arauto Home"),
            PathBuf::from(format!("{}\\", dir.display())),
            PathBuf::from(dir.display().to_string().replace('\\', "/")),
            PathBuf::from(other_drive_case),
            fs::canonicalize(&dir).unwrap(),
        ];

        for spelling in spellings {
            assert_eq!(
                pipe_name(&spelling).unwrap(),
                expected,
                "{}",
                spelling.display()
            );
        }
    }

    #[test]
    fn folder_that_does_not_exist_has_no_name() {
        let root = private_tempdir();

        let error = pipe_name(&root.path().join("missing")).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    /// A limit, pinned: the name follows the canonical path to the letter, so the same
    /// folder reached through a network path, which has another canonical path, has
    /// another name. Only the derivation is checked: the suite does not go to the network.
    #[test]
    fn network_path_of_a_folder_gets_another_name() {
        assert_ne!(
            pipe_name_of_canonical(Path::new(r"\\?\UNC\localhost\c$\Users\hermes\.arauto")),
            pipe_name_of_canonical(Path::new(r"\\?\C:\Users\hermes\.arauto"))
        );
    }

    /// A limit, pinned: the canonical path carries the case stored on disk, so a folder
    /// renamed to another case gets another name, and so does every folder below it.
    #[test]
    fn folder_renamed_to_another_case_gets_another_name() {
        let root = private_tempdir();
        let (dir, below) = (root.path().join("Home"), root.path().join("Home").join("x"));
        fs::create_dir_all(&below).unwrap();
        let before = (pipe_name(&dir).unwrap(), pipe_name(&below).unwrap());

        fs::rename(&dir, root.path().join("home")).unwrap();

        // The old spelling still finds the folder; the name it gives is another.
        let after = (pipe_name(&dir).unwrap(), pipe_name(&below).unwrap());
        assert_ne!(before.0, after.0);
        assert_ne!(before.1, after.1);
    }

    /// What keeps that limit away from a listener: its lock file, open inside the folder,
    /// makes the system refuse to rename the folder and the folders above it.
    #[tokio::test]
    async fn folder_of_a_listener_keeps_its_name_while_the_listener_lives() {
        let root = private_tempdir();
        let above = root.path().join("Above");
        let dir = above.join("Home");
        fs::create_dir_all(&dir).unwrap();
        let (lock, listener) = listen(&dir).await;
        let name = pipe_name(&dir).unwrap();

        let renamed = fs::rename(&dir, above.join("home"));
        let renamed_above = fs::rename(&above, root.path().join("above"));

        assert!(renamed.is_err(), "the folder was renamed");
        assert!(renamed_above.is_err(), "the folder above was renamed");
        assert_eq!(pipe_name(&dir).unwrap(), name);
        within(LocalStream::connect(&dir)).await.unwrap();
        drop(listener);
        drop(lock);
    }

    /// No instance of the pipe may accept a client from another machine, and only the
    /// first may be created while the name is free. Every instance that a listener creates
    /// is looked at: those of `bind`, and the one that each `accept` leaves for the next
    /// client.
    #[tokio::test]
    async fn every_instance_refuses_remote_clients() {
        let root = private_tempdir();
        record_instances();

        let (_lock, mut listener) = listen(root.path()).await;
        let mut connections = Vec::new();
        for _ in 0..3 {
            let (accepted, connected) = within(async {
                tokio::join!(listener.accept(), LocalStream::connect(root.path()))
            })
            .await;
            connections.push((accepted.unwrap(), connected.unwrap()));
        }

        let instances = recorded_instances();
        assert_eq!(
            instances.len(),
            WAITING_INSTANCES + 3,
            "the instances of bind and one from each accept: {instances:?}"
        );
        for (index, options) in instances.iter().enumerate() {
            let first = index == 0;
            assert!(options.contains("reject_remote_clients: true"), "{options}");
            assert!(
                options.contains(&format!("first_pipe_instance: {first}")),
                "instance {index}: {options}"
            );
        }
    }

    /// A limit, pinned: the name of the pipe can be worked out by anybody, so a process
    /// that does not hold the lock can take it. The holder of the lock is told that
    /// somebody listens, and the clients of the folder reach that other process.
    #[tokio::test]
    async fn name_taken_by_a_process_without_the_lock_is_not_taken_back() {
        let root = private_tempdir();
        let lock = lock_of(root.path());
        let mut squatter = ServerOptions::new()
            .create(pipe_name(root.path()).unwrap())
            .unwrap();

        let holder = within(LocalListener::bind(root.path(), lock.clone())).await;
        let mut client = within(LocalStream::connect(root.path())).await.unwrap();

        assert!(
            matches!(holder, Err(LocalSocketError::AlreadyListening { .. })),
            "{holder:?}"
        );
        // The client cannot tell: it talks to whoever has the name.
        within(squatter.connect()).await.unwrap();
        client.write_all(b"x").await.unwrap();
        let mut byte = [0u8; 1];
        within(squatter.read_exact(&mut byte)).await.unwrap();
        assert_eq!(&byte, b"x");
    }

    /// A limit, pinned: the default protection of the pipe lets it be opened for reading
    /// by more than its owner, and such a client is handed out like any other. It reads
    /// what is written to it and cannot write.
    #[tokio::test]
    async fn client_that_opened_the_pipe_only_for_reading_is_handed_out() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        let mut reader = ClientOptions::new()
            .write(false)
            .open(pipe_name(root.path()).unwrap())
            .unwrap();

        let mut accepted = within(listener.accept()).await.unwrap();
        accepted
            .write_all(b"said to whoever connected\n")
            .await
            .unwrap();

        let mut heard = [0u8; 26];
        within(reader.read_exact(&mut heard)).await.unwrap();
        assert_eq!(&heard, b"said to whoever connected\n");
        assert!(
            within(reader.write_all(b"x")).await.is_err(),
            "the client that asked only to read could write"
        );
    }

    /// The clients that arrive together do not queue behind one another: as many as the
    /// instances that wait connect before the listener accepts any, each on a connection
    /// of its own, and every accept leaves room for one more.
    #[tokio::test]
    async fn several_clients_connect_before_any_is_accepted() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        // A wait that is given up, with every instance waiting, loses no client.
        let nobody = tokio::time::timeout(Duration::from_millis(50), listener.accept()).await;
        assert!(nobody.is_err());

        let mut clients = take_every_waiting_instance(root.path()).await;
        let target = resolve(root.path()).unwrap();
        assert!(matches!(connect_once(&target).await, Ok(Attempt::Busy)));

        for (number, client) in clients.iter_mut().enumerate() {
            client.write_all(&[number as u8]).await.unwrap();
        }
        let mut heard = Vec::new();
        let mut accepted = Vec::new();
        for _ in 0..WAITING_INSTANCES {
            let mut server = within(listener.accept()).await.unwrap();
            let mut byte = [0u8; 1];
            within(server.read_exact(&mut byte)).await.unwrap();
            heard.push(usize::from(byte[0]));
            accepted.push(server);
        }
        heard.sort_unstable();
        assert_eq!(heard, (0..WAITING_INSTANCES).collect::<Vec<_>>());

        // Each accept put a new instance in the place of the one it handed out.
        let _more = take_every_waiting_instance(root.path()).await;
        assert!(matches!(connect_once(&target).await, Ok(Attempt::Busy)));
    }

    /// An accept that cannot put a new instance in the place of the one that got a client
    /// fails without losing that client: it stays connected, the next accept hands it
    /// out, and the listener is left with as many instances as it had.
    #[tokio::test]
    async fn client_is_kept_when_no_instance_can_take_the_place_of_its_own() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        let mut client = within(LocalStream::connect(root.path())).await.unwrap();
        client.write_all(b"x").await.unwrap();

        refuse_instances(true);
        let refused = within(listener.accept()).await;
        refuse_instances(false);

        assert!(
            matches!(
                &refused,
                Err(LocalSocketError::Io {
                    action: "create an instance of the named pipe",
                    ..
                })
            ),
            "{refused:?}"
        );
        // The same client: what it sent is read from the connection handed out next.
        let mut accepted = within(listener.accept()).await.unwrap();
        let mut byte = [0u8; 1];
        within(accepted.read_exact(&mut byte)).await.unwrap();
        assert_eq!(&byte, b"x");
        within(accepted.write_all(b"y")).await.unwrap();
        within(client.read_exact(&mut byte)).await.unwrap();
        assert_eq!(&byte, b"y");
        // No instance was lost on the way: as many clients as ever connect at once.
        let _waiting = take_every_waiting_instance(root.path()).await;
        let target = resolve(root.path()).unwrap();
        assert!(matches!(connect_once(&target).await, Ok(Attempt::Busy)));
    }

    #[tokio::test]
    async fn busy_pipe_makes_the_client_try_again_until_it_is_accepted() {
        let root = private_tempdir();
        let (_lock, mut listener) = listen(root.path()).await;
        // Until the listener accepts, the pipe is busy.
        let _waiting = take_every_waiting_instance(root.path()).await;

        let target = resolve(root.path()).unwrap();
        assert!(matches!(connect_once(&target).await, Ok(Attempt::Busy)));

        let dir = root.path().to_path_buf();
        let second = tokio::spawn(async move { LocalStream::connect(&dir).await });
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!second.is_finished(), "the second client is still waiting");

        let _accepted = listener.accept().await.unwrap();

        tokio::time::timeout(Duration::from_secs(30), second)
            .await
            .expect("the second client connects once there is room")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn pipe_that_stays_busy_ends_in_a_clear_error() {
        let root = private_tempdir();
        let (_lock, _listener) = listen(root.path()).await;
        let _waiting = take_every_waiting_instance(root.path()).await;

        let patient = LocalStream::connect_with(root.path(), Duration::from_millis(5), 4);
        let outcome = tokio::time::timeout(Duration::from_secs(30), patient)
            .await
            .expect("the client gives up after the last attempt");

        assert!(
            matches!(&outcome, Err(LocalSocketError::Busy { attempts: 5, .. })),
            "{outcome:?}"
        );
    }
}
