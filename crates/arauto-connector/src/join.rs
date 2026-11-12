//! `join HUB KEY INVITE`: the request to connect (`docs/11-conector.md`, section 1;
//! `docs/04-seguranca-conexao.md`, C3, C4, C6 and C10).
//!
//! | Situation | What happens |
//! |---|---|
//! | The same invite again, request pending | The same folder and identity, recognized by the digest of the invite. A new code, which replaces the previous one. |
//! | The same invite again, request approved | AR-104 from the hub. Nothing changes; the registration in Hermes is made if it is missing. |
//! | The same invite again, request denied or expired | AR-301, AR-302 or AR-303. The folder of that request is discarded only if it has no `credential.seal`. |
//! | The hub refuses the invite (AR-101, AR-102) | The folder created for this request is discarded. Nothing else changes. |
//! | A new invite, with an old request without credential in the folder | The old request is discarded and a new one is created, with a new identity. |
//! | The enrollment worked and the registration in Hermes failed | AR-601. The request stays open on the hub; running it again tries the registration again. |
//! | A device of the same hub is in the folder | A reconnection: the device in use goes on until the new one has its first credential confirmed. |
//! | A device of another hub is in the folder | AR-105. The same address with another key is another hub. |
//!
//! Once the hub accepted, the program copies itself to its fixed path (a temporary file and a
//! rename: the old program may be running), registers in Hermes and tells the leader in
//! execution, through the local socket, that there is a new request; when the program changed,
//! it also asks the leader to end, so that Hermes starts the new version.
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
use crate::catalog::{Local, Said};
use crate::hermes::Hermes;
use crate::home::Home;
use crate::settings::Settings;
use crate::short::{self, Enrolled};
use crate::store::{self, Current, DeviceDir, DevicesLockError};

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
        /// Whether the connector is registered in Hermes now.
        registered: bool,
    },
    /// The request was already approved: nothing to do (AR-104).
    AlreadyApproved {
        /// Whether the connector is registered in Hermes now.
        registered: bool,
    },
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
    if let Ok((_, changed)) = &joined {
        tell_the_leader(home, *changed, settings).await;
    }
    joined.map(|(joined, _)| joined)
}

/// What `join` was given, read: the hub, and what to do once it accepts.
struct Block {
    address: HubAddress,
    key: PublicKey,
    program: PathBuf,
    register: bool,
}

/// The part of `join` that runs under the lock of the device folders. Returns also whether
/// the program at the fixed path changed.
async fn locked(
    home: &Home,
    block: &Block,
    invite: Invite,
    hermes: &Hermes,
    settings: &Settings,
    now: i64,
) -> Result<(Joined, bool), Said> {
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
    let current = layout.current().map(|folder| folder.id().clone());
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
            let registered = if register {
                hermes
                    .add(home, settings)
                    .await
                    .map_err(|_| Said::code(ErrorCode::Ar601))?;
                mark_registered(&dir).await;
                true
            } else {
                false
            };
            Ok((
                Joined::Enrolled {
                    code,
                    minutes: confirm_within.get().div_ceil(60),
                    replaces: enrollment.get() > 1,
                    registered,
                },
                changed,
            ))
        }
        Enrolled::Refused(ErrorCode::Ar104) => {
            // Nothing changes: only the registration, when it is missing, and with it the
            // program it names.
            let registered = dir.record().is_ok_and(|record| record.registered);
            if !register || registered {
                return Ok((Joined::AlreadyApproved { registered }, false));
            }
            let changed = install_program(program, &home.program()).await?;
            hermes
                .add(home, settings)
                .await
                .map_err(|_| Said::code(ErrorCode::Ar601))?;
            mark_registered(&dir).await;
            Ok((Joined::AlreadyApproved { registered: true }, changed))
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

async fn mark_registered(dir: &DeviceDir) {
    let dir = dir.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let mut record = dir.record()?;
        record.registered = true;
        dir.write_record(&record)
    })
    .await;
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

/// Tells the leader in execution that there is a new request, and asks it to end when the
/// program changed. No leader is fine: Hermes starts one.
async fn tell_the_leader(home: &Home, changed: bool, settings: &Settings) {
    let patience = settings.local_answer_ms;
    match ask::ask(home, ask::operation("new_request"), patience).await {
        Ok(_) if changed => {
            let _ = ask::ask(home, ask::operation("restart"), patience).await;
        }
        Ok(_) | Err(_) => {}
    }
}
