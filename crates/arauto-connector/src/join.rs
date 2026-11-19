//! `join HUB KEY INVITE`: the request to connect (`docs/11-conector.md`, section 1;
//! `docs/04-seguranca-conexao.md`, C3, C4, C6 and C10).
//!
//! | Situation | What happens |
//! |---|---|
//! | The same invite again, request pending | The same folder and identity, recognized by the digest of the invite. A new code, which replaces the previous one. |
//! | The same invite again, request approved | AR-104 from the hub. Nothing changes; the registration in Hermes is made if it is missing or lacks the reference to the key. |
//! | The same invite again, request denied or expired | AR-301, AR-302 or AR-303. The folder of that request is discarded only if it has no `credential.seal`. |
//! | The hub refuses the invite (AR-101, AR-102) | The folder created for this request is discarded. Nothing else changes. |
//! | A new invite, with an old request without credential in the folder | The old request is discarded and a new one is created, with a new identity. |
//! | The enrollment worked and the registration in Hermes failed | AR-601 (when the program of Hermes was not found, with a sentence of its own that says where to look). The request stays open on the hub; running it again tries the registration again. |
//! | A device of the same hub is in the folder | A reconnection: the device in use goes on until the new one has its first credential confirmed. The entry in Hermes is saved again only when it lacks the reference to the key of the local API (`hub.json` says so); an entry that has it is left as it is. |
//! | A device of another hub is in the folder | AR-105. The same address with another key is another hub. |
//!
//! Once the hub accepted, the program copies itself to its fixed path (a temporary file and a
//! rename: the old program may be running), registers in Hermes and tells the leader in
//! execution, through the local socket, that there is a new request. It does not ask the leader
//! to end while the folder format is the same (decision 0053): the running program keeps the
//! channel of the device in use and the Arauto tools of Hermes keep working until its gateway
//! restarts, and from then on Hermes runs the new file. [`Program`] says which case happened,
//! and the command prints one fixed line about it. Only a program that cannot read the device
//! folders (another folder format) makes `join` ask the leader to end; no build does that today.
//!
//! `join` holds the lock of the device folders while it runs: two of them never run at once,
//! and no credential reaches a folder while `join` decides to discard it.

use std::path::{Path, PathBuf};

use arauto_protocol::code::VerificationCode;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::Invite;
use arauto_protocol::noise::PublicKey;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::address::{AddressError, HubAddress};
use crate::ask;
use crate::catalog::{Local, Said, Told};
use crate::hermes::{Budget, Hermes, Registration, Setting};
use crate::home::Home;
use crate::settings::Settings;
use crate::short::{self, Enrolled};
use crate::store::{self, Current, DeviceDir, DevicesLockError, HubRecord};

/// What `join` receives.
pub struct JoinArgs {
    /// The address of the hub, from the block.
    pub hub: String,
    /// The key of the hub, from the block.
    pub key: String,
    /// The invite, from the block: a secret, wiped when dropped.
    pub invite: Zeroizing<String>,
    /// Whether to register in Hermes (`--no-register` turns it off).
    pub register: bool,
    /// The program to copy to the fixed path: the one running.
    pub program: PathBuf,
}

impl std::fmt::Debug for JoinArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinArgs")
            .field("register", &self.register)
            .finish_non_exhaustive()
    }
}

/// What `join` did about the entry of the connector in Hermes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// It was not asked for (`--no-register`).
    NotAsked,
    /// The entry was already there with the reference to the key: it was not saved again.
    Kept,
    /// The entry was saved by this run.
    Saved {
        /// With the reference to the key of the local API, or without it.
        registration: Registration,
        /// Whether it replaced an entry this connector had saved before: a running Hermes
        /// keeps the old one until it restarts its gateway.
        replaced: bool,
    },
}

/// How `join` ended when it did not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joined {
    /// The hub accepted the request.
    Enrolled {
        /// The code the Hermes says and the admin types.
        code: VerificationCode,
        /// In how many minutes it has to be typed, rounded up.
        minutes: u32,
        /// Whether it replaces the code of an earlier enrollment of the same request.
        replaces: bool,
        /// What was done about the entry in Hermes.
        entry: Entry,
    },
    /// The request was already approved: nothing to do (AR-104).
    AlreadyApproved {
        /// What was done about the entry in Hermes.
        entry: Entry,
    },
}

impl Entry {
    /// Whether the entry carries the reference to the key of the local API: only then would a
    /// key made now wake anything, so only then does `join` look for one.
    #[must_use]
    pub const fn carries_key_reference(self) -> bool {
        matches!(
            self,
            Entry::Kept
                | Entry::Saved {
                    registration: Registration::WithKeyReference,
                    ..
                }
        )
    }
}

/// How the key step ended (`docs/11-conector.md`, section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyOutcome {
    /// `API_SERVER_KEY` is already set in Hermes: nothing was made and nothing is said.
    AlreadySet,
    /// The connector drew a key and Hermes's own command saved it.
    Made,
    /// A key was not confirmed: a guard said no, a call failed or the time ran out.
    NotMade,
}

/// The longest the listener guard waits for an answer.
const PROBE_LIMIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The key step of `join`: gives a Hermes that has no key for its local API one, within the
/// time of `settings.key_step_ms` in all. It runs after `join` printed its lines, only in
/// `join`, and never changes how `join` ends.
pub(crate) async fn give_key(hermes: &Hermes, settings: &Settings) -> KeyOutcome {
    // A test that answers the commands of Hermes itself must never reach a real Hermes here:
    // the key step is refused, as "not confirmed", instead of running the program.
    #[cfg(feature = "testing")]
    if settings.testing.hermes.is_some() {
        return KeyOutcome::NotMade;
    }
    give_key_using(hermes, settings, draw_key).await
}

// BEGIN key step
/// The steps of `docs/11-conector.md`, section 6, with `draw` as the source of the key.
async fn give_key_using(
    hermes: &Hermes,
    settings: &Settings,
    draw: impl FnOnce() -> Option<Zeroizing<String>>,
) -> KeyOutcome {
    let budget = Budget::new(settings.key_step_ms);
    // 1. A key is set: stop. Anything but a plain "not set" is "do not know".
    match hermes
        .is_set(Setting::ApiServerKey, budget.call_limit(settings))
        .await
    {
        Ok(true) => return KeyOutcome::AlreadySet,
        Ok(false) => {}
        Err(_) => return KeyOutcome::NotMade,
    }
    // 2. Listener guard: something already answers where the local API would be.
    if listener_answers(settings.key_probe, budget.left().min(PROBE_LIMIT)).await {
        return KeyOutcome::NotMade;
    }
    // 3. Address guard: no host named for the local API anywhere the terminal sees.
    for setting in Setting::ADDRESS {
        match hermes.is_set(setting, budget.call_limit(settings)).await {
            Ok(false) => {}
            Ok(true) | Err(_) => return KeyOutcome::NotMade,
        }
    }
    // 4. The key, from the generator of the system and from nothing else.
    let Some(key) = draw() else {
        return KeyOutcome::NotMade;
    };
    // 5. Hermes's own command saves it. The key is wiped when `key` goes out of scope.
    match hermes.save_key(&key, budget.call_limit(settings)).await {
        Ok(()) => KeyOutcome::Made,
        Err(_) => KeyOutcome::NotMade,
    }
}

/// 32 bytes of the generator of the system, as 64 lowercase hexadecimal characters. Both the
/// bytes and the text are wiped when dropped; `None` when the generator fails.
fn draw_key() -> Option<Zeroizing<String>> {
    let mut bytes = Zeroizing::new([0_u8; 32]);
    getrandom::fill(bytes.as_mut_slice()).ok()?;
    let mut text = Zeroizing::new(String::with_capacity(64));
    for byte in bytes.iter() {
        text.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(HEX_DIGITS[usize::from(byte & 15)]));
    }
    Some(text)
}
// END key step

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Whether a TCP connection to `address` is accepted within `limit`. Nothing is sent and the
/// connection is closed at once.
async fn listener_answers(address: std::net::SocketAddr, limit: std::time::Duration) -> bool {
    matches!(
        tokio::time::timeout(limit, tokio::net::TcpStream::connect(address)).await,
        Ok(Ok(_))
    )
}

/// Whether the entry that `record` describes is saved with the reference to the key.
fn up_to_date(record: &HubRecord) -> bool {
    record.registered && record.key_reference == Some(true)
}

/// What `join` did to the program at the fixed path, and what it says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program {
    /// The program at the fixed path was not replaced (the same bytes, or none was there).
    Same,
    /// Replaced; the running connector answered `new_request` and was left running.
    ReplacedRunning,
    /// Replaced; no running connector answered.
    ReplacedNoneRunning,
    /// Replaced by a program of another folder format: the running one was asked to end.
    ReplacedEnded,
}

impl Program {
    /// The line `join` prints about it, when there is one.
    #[must_use]
    pub const fn told(self) -> Option<Told> {
        match self {
            Self::Same => None,
            Self::ReplacedRunning => Some(Told::ProgramReplaced),
            Self::ReplacedNoneRunning => Some(Told::ProgramReplacedNoneRunning),
            Self::ReplacedEnded => Some(Told::ProgramReplacedEnded),
        }
    }
}

/// Whether `join` asks the running leader to end, and what it says (decision 0053).
pub(crate) const fn after_install(
    changed: bool,
    answered: bool,
    same_format: bool,
) -> (bool, Program) {
    if !changed {
        (false, Program::Same)
    } else if !answered {
        (false, Program::ReplacedNoneRunning)
    } else if same_format {
        (false, Program::ReplacedRunning)
    } else {
        (true, Program::ReplacedEnded)
    }
}

/// Runs `join`.
///
/// # Errors
/// What to say: a code of the catalog, or a local problem.
pub async fn join(
    home: &Home,
    args: JoinArgs,
    hermes: &Hermes,
    settings: &Settings,
    now: i64,
) -> Result<Joined, Said> {
    join_reporting(home, args, hermes, settings, now)
        .await
        .map(|(joined, _)| joined)
}

/// Runs `join` and says also what happened to the program at the fixed path.
///
/// # Errors
/// What to say: a code of the catalog, or a local problem.
pub async fn join_reporting(
    home: &Home,
    args: JoinArgs,
    hermes: &Hermes,
    settings: &Settings,
    now: i64,
) -> Result<(Joined, Program), Said> {
    let address = HubAddress::parse(&args.hub).map_err(|error| match error {
        AddressError::Form => Said::Local(Local::HubAddress),
        AddressError::PlainNotLocal => Said::Local(Local::PlainNotLocal),
    })?;
    if address.is_secure() && !cfg!(feature = "tls") {
        return Err(Said::Local(Local::NoTls));
    }
    let key = PublicKey::parse_text(&args.key).map_err(|_| Said::Local(Local::HubKey))?;
    // An invite out of its format never reaches the hub.
    let invite = Invite::parse(&args.invite).map_err(|_| Said::code(ErrorCode::Ar101))?;
    drop(args.invite);
    {
        let home = home.clone();
        tokio::task::spawn_blocking(move || home.prepare())
            .await
            .map_err(|_| Said::Local(Local::Internal))?
            .map_err(|_| Said::Local(Local::Folder))?;
    }
    let lock = store::lock_devices(home, settings.command_lock_ms)
        .await
        .map_err(|error| match error {
            DevicesLockError::Busy => Said::Local(Local::Busy),
            DevicesLockError::Lock(_) | DevicesLockError::Task => Said::Local(Local::Folder),
        })?;
    let block = Block {
        address,
        key,
        program: args.program,
        register: args.register,
    };
    let joined = locked(home, &block, invite, hermes, settings, now).await;
    drop(lock);
    match joined {
        Ok((joined, changed, same_format)) => {
            let program = tell_the_leader(home, changed, same_format, settings).await;
            Ok((joined, program))
        }
        Err(said) => Err(said),
    }
}

/// What `join` was given, read: the hub, and what to do once it accepts.
struct Block {
    address: HubAddress,
    key: PublicKey,
    program: PathBuf,
    register: bool,
}

/// The part of `join` that runs under the lock of the device folders. Returns also whether
/// the program at the fixed path changed, and whether the folders keep the format the new
/// program reads.
async fn locked(
    home: &Home,
    block: &Block,
    invite: Invite,
    hermes: &Hermes,
    settings: &Settings,
    now: i64,
) -> Result<(Joined, bool, bool), Said> {
    let Block {
        address,
        key,
        program,
        register,
    } = block;
    let register = *register;
    let digest = invite.digest();
    let layout = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || {
            let _ = store::clean_leftovers(&home);
            store::scan(&home)
        })
        .await
        .map_err(|_| Said::Local(Local::Internal))?
        .map_err(|_| Said::Local(Local::Folder))?
    };
    if layout.current == Current::Damaged
        || layout.folders.iter().any(|folder| folder.record.is_err())
    {
        return Err(Said::Local(Local::Damaged));
    }
    // Every folder `join` gets past is of `FORMAT_VERSION` (`store::scan` reads another format as
    // damaged), so the program it installs reads it. The first format change sets this from the
    // format of the device in use.
    let same_format = true;
    let current = layout.current().map(|folder| folder.id().clone());
    // The device in use is already registered in Hermes, under the fixed path of the program:
    // a reconnection registers nothing again when that entry has the reference to the key.
    // Saving it again would replace the entry, with whatever was added to it by hand.
    let current_record = layout.current().and_then(|folder| folder.record()).cloned();
    // A device of another hub: only after leaving it (C10).
    let other_hub = layout.folders.iter().any(|folder| {
        let counts = folder.has_credential || Some(folder.id()) == current.as_ref();
        counts
            && folder
                .record()
                .is_some_and(|record| !record.same_hub(address, key))
    });
    if other_hub {
        return Err(Said::code(ErrorCode::Ar105));
    }
    // The same invite again: the same request, with the same identity (C3, step 1).
    let same = layout.folders.iter().find(|folder| {
        folder
            .record()
            .is_some_and(|record| record.invite == digest && record.same_hub(address, key))
    });
    let (dir, created) = match same {
        Some(folder) => (folder.dir.clone(), false),
        None => {
            // A new invite, a new identity: the old requests without a credential go.
            for folder in &layout.folders {
                if Some(folder.id()) != current.as_ref() && !folder.has_credential {
                    let home = home.clone();
                    let id = folder.id().clone();
                    let discarded =
                        tokio::task::spawn_blocking(move || store::discard_request(&home, &id))
                            .await;
                    if !matches!(discarded, Ok(Ok(_))) {
                        return Err(Said::Local(Local::Folder));
                    }
                }
            }
            let home = home.clone();
            let address = address.clone();
            let key = *key;
            let dir = tokio::task::spawn_blocking(move || {
                store::create_request(&home, &address, &key, digest, now)
            })
            .await
            .map_err(|_| Said::Local(Local::Internal))?
            .map_err(|error| match error {
                store::StoreError::Random => Said::Local(Local::Random),
                _ => Said::Local(Local::Folder),
            })?;
            (dir, true)
        }
    };
    // What is known about the entry in Hermes before this run: the device in use, and the
    // folder of this request.
    let prior: Vec<HubRecord> = [current_record, dir.record().ok()]
        .into_iter()
        .flatten()
        .collect();
    let identity = dir.identity().map_err(|_| Said::Local(Local::Damaged))?;
    let enrolled = short::enroll(address, key, &identity, invite, settings).await;
    drop(identity);
    match enrolled {
        Enrolled::Accepted {
            code,
            confirm_within,
            enrollment,
        } => {
            let changed = install_program(program, &home.program()).await?;
            let entry = if !register {
                Entry::NotAsked
            } else if prior.iter().any(up_to_date) {
                mark_registered(&dir, true).await;
                Entry::Kept
            } else {
                let registration = hermes
                    .add(home, settings)
                    .await
                    .map_err(|error| error.said())?;
                mark_registered(&dir, registration == Registration::WithKeyReference).await;
                Entry::Saved {
                    registration,
                    replaced: prior.iter().any(|record| record.registered),
                }
            };
            Ok((
                Joined::Enrolled {
                    code,
                    minutes: confirm_within.get().div_ceil(60),
                    replaces: enrollment.get() > 1,
                    entry,
                },
                changed,
                same_format,
            ))
        }
        Enrolled::Refused(ErrorCode::Ar104) => {
            // Nothing changes: only the registration, when it is missing or lacks the
            // reference to the key, and with it the program it names.
            let record = dir.record().ok();
            if !register {
                let entry = Entry::NotAsked;
                return Ok((Joined::AlreadyApproved { entry }, false, same_format));
            }
            if record.as_ref().is_some_and(up_to_date) {
                let entry = Entry::Kept;
                return Ok((Joined::AlreadyApproved { entry }, false, same_format));
            }
            let changed = install_program(program, &home.program()).await?;
            let registration = hermes
                .add(home, settings)
                .await
                .map_err(|error| error.said())?;
            mark_registered(&dir, registration == Registration::WithKeyReference).await;
            let entry = Entry::Saved {
                registration,
                replaced: record.is_some_and(|record| record.registered),
            };
            Ok((Joined::AlreadyApproved { entry }, changed, same_format))
        }
        Enrolled::Refused(
            code @ (ErrorCode::Ar101
            | ErrorCode::Ar102
            | ErrorCode::Ar301
            | ErrorCode::Ar302
            | ErrorCode::Ar303),
        ) => {
            // The hub refused the invite, or the request is over: its folder goes, but only
            // while it has no credential.
            discard(home, &dir).await;
            Err(Said::code(code))
        }
        Enrolled::Refused(code) => {
            // A version refused, or the hub stopping: the hub read no invite.
            if created && matches!(code, ErrorCode::Ar401 | ErrorCode::Ar402) {
                discard(home, &dir).await;
            }
            Err(Said::code(code))
        }
        Enrolled::Failed { said, sent } => {
            // Before `enroll` left, the hub knows nothing of this identity. After, it may have
            // recorded the request: the folder stays, and the same invite reuses it.
            if created && !sent {
                discard(home, &dir).await;
            }
            Err(said)
        }
    }
}

async fn discard(home: &Home, dir: &DeviceDir) {
    let home = home.clone();
    let id = dir.id().clone();
    let _ = tokio::task::spawn_blocking(move || store::discard_request(&home, &id)).await;
}

/// Writes down in `hub.json` that the entry is saved, and whether it has the reference.
pub(crate) async fn mark_registered(dir: &DeviceDir, key_reference: bool) {
    let dir = dir.clone();
    let _ = tokio::task::spawn_blocking(move || mark_registered_now(&dir, key_reference)).await;
}

/// [`mark_registered`], for code that already runs on a blocking thread.
pub(crate) fn mark_registered_now(
    dir: &DeviceDir,
    key_reference: bool,
) -> Result<(), store::StoreError> {
    let mut record = dir.record()?;
    record.registered = true;
    record.key_reference = Some(key_reference);
    dir.write_record(&record)
}

/// SHA-256 of a file, or `None` when it cannot be read.
fn file_digest(path: &Path) -> Option<[u8; 32]> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(buffer.get(..read)?);
    }
    Some(hasher.finalize().into())
}

/// Copies the program to its fixed path, by a temporary file and a rename. Returns whether a
/// program of other content was there.
async fn install_program(program: &Path, destination: &Path) -> Result<bool, Said> {
    let program = program.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let before = file_digest(&destination);
        let now = file_digest(&program);
        if before.is_some() && before == now {
            return Ok(false);
        }
        arauto_platform::replace_executable(&program, &destination)
            .map(|()| before.is_some())
            .map_err(|_| Said::Local(Local::Program))
    })
    .await
    .map_err(|_| Said::Local(Local::Internal))?
}

/// Tells the leader in execution that there is a new request, and says what happened to the
/// program. The leader is asked to end only when [`after_install`] says so (another folder
/// format). No leader is fine: Hermes starts one.
async fn tell_the_leader(
    home: &Home,
    changed: bool,
    same_format: bool,
    settings: &Settings,
) -> Program {
    let patience = settings.local_answer_ms;
    let answered = ask::ask(home, ask::operation("new_request"), patience)
        .await
        .is_ok();
    let (end, program) = after_install(changed, answered, same_format);
    if end {
        let _ = ask::ask(home, ask::operation("restart"), patience).await;
    }
    program
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Program, Told, after_install};

    #[test]
    fn after_install_ends_the_old_leader_only_when_the_format_differs() {
        for changed in [false, true] {
            for answered in [false, true] {
                for same_format in [false, true] {
                    let expected = if !changed {
                        (false, Program::Same)
                    } else if !answered {
                        (false, Program::ReplacedNoneRunning)
                    } else if same_format {
                        (false, Program::ReplacedRunning)
                    } else {
                        (true, Program::ReplacedEnded)
                    };
                    assert_eq!(after_install(changed, answered, same_format), expected);
                }
            }
        }
        assert_eq!(Program::Same.told(), None);
        assert_eq!(Program::ReplacedRunning.told(), Some(Told::ProgramReplaced));
        assert_eq!(
            Program::ReplacedNoneRunning.told(),
            Some(Told::ProgramReplacedNoneRunning)
        );
        assert_eq!(
            Program::ReplacedEnded.told(),
            Some(Told::ProgramReplacedEnded)
        );
    }

    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    use arauto_testkit::hermes::{FakeHermes, built_example};

    use super::*;

    fn fake() -> FakeHermes {
        FakeHermes::new(&built_example("fake_hermes"))
    }

    fn hermes_of(fake: &FakeHermes) -> Hermes {
        Hermes::new(Some(fake.program().into_os_string()))
    }

    /// An address where nothing listens: a port that was bound and let go.
    fn nothing_listens() -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    }

    fn settings_with(probe: SocketAddr, step_ms: i64) -> Settings {
        Settings::default()
            .with_key_probe(probe)
            .with_key_step_ms(step_ms)
    }

    fn names(calls: &[Vec<String>]) -> Vec<String> {
        calls.iter().map(|call| call.join(" ")).collect()
    }

    /// The five reads of the key step, in order, as the fake records them.
    const READS: [&str; 5] = [
        "config get API_SERVER_KEY",
        "config get API_SERVER_HOST",
        "config get gateway.platforms.api_server",
        "config get gateway.api_server.host",
        "config get gateway.api_server.extra",
    ];

    fn is_key(text: &str) -> bool {
        text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    /// T2 (principle 2 and D1): a Hermes whose key is set is left alone: one read, nothing
    /// else, nothing written. Without the first step the connector would go on to the guards and
    /// make a key, and the calls and the outcome below would be different.
    #[tokio::test]
    async fn a_key_that_is_set_stops_the_step_after_one_read() {
        let fake = fake();
        let settings = settings_with(nothing_listens(), 30_000);
        let outcome = give_key(&hermes_of(&fake), &settings).await;
        assert_eq!(outcome, KeyOutcome::AlreadySet);
        assert_eq!(names(&fake.config_calls()), [READS[0]]);
        assert_eq!(fake.secret("API_SERVER_KEY"), None);
    }

    /// T3: something accepts on the probe address, so no key is made, and the address guard is
    /// not even asked. Without the listener guard the same Hermes gets its key.
    #[tokio::test]
    async fn a_listener_on_the_local_api_address_stops_the_step() {
        let fake = fake();
        fake.without_key();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let settings = settings_with(listener.local_addr().unwrap(), 30_000);
        let outcome = give_key(&hermes_of(&fake), &settings).await;
        assert_eq!(outcome, KeyOutcome::NotMade);
        assert_eq!(names(&fake.config_calls()), [READS[0]]);
        assert_eq!(fake.secret("API_SERVER_KEY"), None);
        // The same Hermes with nothing listening gets one, so the guard is what stopped it.
        drop(listener);
        let settings = settings_with(nothing_listens(), 30_000);
        assert_eq!(
            give_key(&hermes_of(&fake), &settings).await,
            KeyOutcome::Made
        );
    }

    /// T4: each of the four names of the address guard, set one at a time, stops the step with
    /// no `set`; all four unset is one `set`, with the key as the last argument (T9).
    #[tokio::test]
    async fn a_host_named_for_the_local_api_stops_the_step_and_none_makes_one_set() {
        for (at, name) in [
            "API_SERVER_HOST",
            "gateway.platforms.api_server",
            "gateway.api_server.host",
            "gateway.api_server.extra",
        ]
        .into_iter()
        .enumerate()
        {
            let fake = fake();
            fake.without_key();
            fake.has_setting(name);
            let settings = settings_with(nothing_listens(), 30_000);
            let outcome = give_key(&hermes_of(&fake), &settings).await;
            assert_eq!(outcome, KeyOutcome::NotMade, "{name}");
            // The reads stop at the first that says "set".
            assert_eq!(names(&fake.config_calls()), READS[..at + 2], "{name}");
            assert_eq!(fake.secret("API_SERVER_KEY"), None, "{name}");
        }
        let fake = fake();
        fake.without_key();
        let settings = settings_with(nothing_listens(), 30_000);
        assert_eq!(
            give_key(&hermes_of(&fake), &settings).await,
            KeyOutcome::Made
        );
        let calls = fake.config_calls();
        assert_eq!(names(&calls[..5]), READS);
        assert_eq!(calls.len(), 6);
        // T9, the arguments: exactly `config set API_SERVER_KEY <64 lowercase hexadecimal>`.
        let set = &calls[5];
        assert_eq!(set.len(), 4, "{set:?}");
        assert_eq!(&set[..3], ["config", "set", "API_SERVER_KEY"]);
        assert!(is_key(&set[3]), "{}", set[3].len());
        assert_eq!(
            fake.secret("API_SERVER_KEY").as_deref(),
            Some(set[3].as_str())
        );
        // A second run finds it at the first step and does nothing more.
        assert_eq!(
            give_key(&hermes_of(&fake), &settings).await,
            KeyOutcome::AlreadySet
        );
        assert_eq!(fake.config_calls().len(), 7);
    }

    /// T5: "do not know" is "no". A first read that exits with another code, a program that is
    /// missing and a call that hangs past the deadline each end with no `set` and no key.
    #[tokio::test]
    async fn not_knowing_is_no_key() {
        // Step 1 exits with a code that is neither 0 nor 1.
        let failing = fake();
        failing.without_key();
        failing.failing_config_get(true);
        let settings = settings_with(nothing_listens(), 30_000);
        assert_eq!(
            give_key(&hermes_of(&failing), &settings).await,
            KeyOutcome::NotMade
        );
        assert_eq!(names(&failing.config_calls()), [READS[0]]);
        assert_eq!(failing.secret("API_SERVER_KEY"), None);

        // The program is not there.
        let missing = Hermes::new(Some(
            std::env::temp_dir()
                .join("no-such-hermes-for-the-key-step")
                .into_os_string(),
        ));
        assert_eq!(give_key(&missing, &settings).await, KeyOutcome::NotMade);

        // A call that hangs: the deadline ends it, long before the 90 s of one call.
        for (hanging, what) in [("get", "the first read"), ("set", "the save")] {
            let fake = fake();
            fake.without_key();
            fake.hanging(Some(hanging));
            let settings = settings_with(nothing_listens(), 600);
            let started = Instant::now();
            let outcome = give_key(&hermes_of(&fake), &settings).await;
            let took = started.elapsed();
            fake.hanging(None);
            assert_eq!(outcome, KeyOutcome::NotMade, "{what}");
            assert!(took < Duration::from_secs(20), "{what}: {took:?}");
            assert!(took >= Duration::from_millis(500), "{what}: {took:?}");
        }
    }

    /// T6: the key is 32 bytes of the generator, as 64 lowercase hexadecimal characters, never
    /// the same twice; when the generator fails there is no `set`.
    #[tokio::test]
    async fn the_key_comes_from_the_generator_and_a_failure_makes_none() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1_000 {
            let key = draw_key().unwrap();
            assert!(is_key(&key), "{}", key.len());
            assert!(seen.insert(key.to_string()));
        }
        let fake = fake();
        fake.without_key();
        let settings = settings_with(nothing_listens(), 30_000);
        let outcome = give_key_using(&hermes_of(&fake), &settings, || None).await;
        assert_eq!(outcome, KeyOutcome::NotMade);
        // Every read ran, and nothing was saved.
        assert_eq!(names(&fake.config_calls()), READS);
        assert_eq!(fake.secret("API_SERVER_KEY"), None);
    }

    /// T8 (the part of the unit): only an entry with the reference looks for a key.
    #[test]
    fn only_an_entry_with_the_reference_looks_for_a_key() {
        assert!(Entry::Kept.carries_key_reference());
        let saved = |registration| Entry::Saved {
            registration,
            replaced: false,
        };
        assert!(saved(Registration::WithKeyReference).carries_key_reference());
        assert!(!saved(Registration::WithoutKeyReference).carries_key_reference());
        assert!(!Entry::NotAsked.carries_key_reference());
    }
}
