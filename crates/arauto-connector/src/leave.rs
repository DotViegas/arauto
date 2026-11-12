//! `leave` and `leave --local` (`docs/11-conector.md`, section 1; `docs/04-seguranca-conexao.md`,
//! C10).
//!
//! | Situation | What happens |
//! |---|---|
//! | The leader has the channel open | The request goes through the leader. With the confirmation, everything is deleted and the registration removed. |
//! | More than one identity in the folder | The closing of each one is asked; nothing is deleted before every answer. |
//! | Only a pending request | The hub is asked to close it, and it becomes denied. With the answer, it is deleted. |
//! | No channel (quarantine, suspended, revoked, or no connector running) | A short connection proves the identity and asks. |
//! | The hub answers "already closed", or that it does not know the device | It counts as a confirmation. |
//! | The hub does not answer, or is not the hub of the stored key | **The identity is not deleted.** AR-505. |
//! | `leave --local` | Deletes the files and the registration without talking to the hub. For a person, when the hub is gone or changed its key. |

use std::time::Duration;

use arauto_platform::{FileLock, LockError};
use arauto_protocol::errors::ErrorCode;
use serde_json::Value;

use crate::ask::{self, AskError};
use crate::catalog::{Local, Said};
use crate::hermes::Hermes;
use crate::home::Home;
use crate::settings::Settings;
use crate::short;
use crate::store::{self, DevicesLockError};

/// How long `leave` waits for a leader that is starting.
const LEADER_STARTING: Duration = Duration::from_millis(100);

/// How many times `leave` tries again while a leader is starting.
const LEADER_STARTING_TRIES: u32 = 50;

/// How `leave` ended when it did not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Left {
    /// The files were deleted. `unregistered` says whether the registration in Hermes was
    /// removed too.
    Done {
        /// How many identities the hub confirmed; zero for the local leave.
        identities: usize,
        /// Whether the registration in Hermes was removed.
        unregistered: bool,
    },
    /// There was no device in the folder.
    Nothing,
}

/// Runs `leave`, or `leave --local` with `local`.
///
/// # Errors
/// AR-505 when an identity was not confirmed, and nothing was deleted; a local problem.
pub async fn leave(
    home: &Home,
    local: bool,
    hermes: &Hermes,
    settings: &Settings,
) -> Result<Left, Said> {
    if !home.root().is_dir() {
        return Ok(Left::Nothing);
    }
    let identities = if local {
        leave_locally(home, settings).await?;
        0
    } else {
        match leave_through_the_hub(home, settings).await? {
            Some(identities) => identities,
            None => return Ok(Left::Nothing),
        }
    };
    let unregistered = hermes.remove(settings).await.is_ok();
    Ok(Left::Done {
        identities,
        unregistered,
    })
}

async fn leave_locally(home: &Home, settings: &Settings) -> Result<(), Said> {
    let lock = lock(home, settings).await?;
    let removed = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || store::remove_everything(&home)).await
    };
    drop(lock);
    if !matches!(removed, Ok(Ok(()))) {
        return Err(Said::Local(Local::Folder));
    }
    // A leader in execution reads the folder again, and finds nothing to use.
    let _ = ask::ask(
        home,
        ask::operation("new_request"),
        settings.local_answer_ms,
    )
    .await;
    Ok(())
}

async fn lock(home: &Home, settings: &Settings) -> Result<store::DevicesLock, Said> {
    store::lock_devices(home, settings.command_lock_ms)
        .await
        .map_err(|error| match error {
            DevicesLockError::Busy => Said::Local(Local::Busy),
            DevicesLockError::Lock(_) | DevicesLockError::Task => Said::Local(Local::Folder),
        })
}

/// The leave that asks the hub. `None` when there was nothing to leave.
async fn leave_through_the_hub(home: &Home, settings: &Settings) -> Result<Option<usize>, Said> {
    for _ in 0..LEADER_STARTING_TRIES {
        match ask::ask(home, ask::operation("leave"), settings.local_leave_ms).await {
            Ok(answer) => return through_the_leader(&answer),
            Err(AskError::NoAnswer) => return Err(Said::code(ErrorCode::Ar505)),
            Err(AskError::NoLeader) => {}
        }
        // No leader: this command does it, holding the lock of the leader so that none
        // starts meanwhile.
        let path = home.lock_file();
        let tried = tokio::task::spawn_blocking(move || FileLock::try_acquire(&path)).await;
        match tried {
            Ok(Ok(leader_lock)) => {
                let left = without_a_leader(home, settings).await;
                drop(leader_lock);
                return left;
            }
            // A leader is starting: it is asked in a moment.
            Ok(Err(LockError::AlreadyHeld { .. })) => tokio::time::sleep(LEADER_STARTING).await,
            Ok(Err(_)) | Err(_) => return Err(Said::Local(Local::Folder)),
        }
    }
    Err(Said::code(ErrorCode::Ar505))
}

fn through_the_leader(answer: &serde_json::Map<String, Value>) -> Result<Option<usize>, Said> {
    let left = answer
        .get("left")
        .and_then(Value::as_array)
        .ok_or(Said::code(ErrorCode::Ar505))?;
    if left.is_empty() && answer.get("done").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    match answer.get("done").and_then(Value::as_bool) {
        Some(true) => Ok(Some(left.len())),
        _ => Err(Said::code(ErrorCode::Ar505)),
    }
}

async fn without_a_leader(home: &Home, settings: &Settings) -> Result<Option<usize>, Said> {
    let lock = lock(home, settings).await?;
    let layout = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || store::scan(&home))
            .await
            .map_err(|_| Said::Local(Local::Internal))?
            .map_err(|_| Said::Local(Local::Folder))?
    };
    if layout.folders.is_empty() {
        let home = home.clone();
        let _ = tokio::task::spawn_blocking(move || store::remove_everything(&home)).await;
        return Ok(None);
    }
    let mut confirmed = 0usize;
    for folder in &layout.folders {
        let (Some(record), Ok(identity)) = (folder.record(), folder.dir.identity()) else {
            continue;
        };
        if short::leave(&record.address, &record.key, &identity, settings)
            .await
            .is_ok()
        {
            confirmed = confirmed.saturating_add(1);
        }
    }
    // Only after every answer (C10): otherwise nothing is deleted.
    if confirmed != layout.folders.len() {
        return Err(Said::code(ErrorCode::Ar505));
    }
    let removed = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || store::remove_everything(&home)).await
    };
    drop(lock);
    if !matches!(removed, Ok(Ok(()))) {
        return Err(Said::Local(Local::Folder));
    }
    Ok(Some(confirmed))
}
