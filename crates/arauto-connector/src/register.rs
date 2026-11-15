//! `register`: saves the entry of the connector in Hermes again, for a Hermes that was
//! connected before the entry carried the reference to the key of its local API
//! (`docs/11-conector.md`, section 1; `docs/decisoes/0046`).
//!
//! It touches the entry in Hermes and one field of `hub.json`, and nothing else:
//!
//! - it does not contact the hub, does not copy the program, and asks the leader nothing (no
//!   `restart`, no `new_request`): a running gateway of Hermes does not start a server whose
//!   process died (`referencia/hermes-fatos.md`), so ending the leader would leave Hermes
//!   without Arauto until the gateway restarts;
//! - it does not touch `identity.key`, `credential.seal`, `current`, `state.json` or
//!   `wakes.json`, and does not read or write any file of Hermes;
//! - it does not hold the lock of the device folders while the command of Hermes runs, which can
//!   take 90 seconds: the leader waits at most 30 seconds for that lock before a write.
//!
//! Order: look at the folders under the lock (no command of Hermes has run yet), run the
//! command of Hermes without the lock, write down the result under the lock. When the last step
//! fails, the entry in Hermes is saved all the same, and running `register` again fixes the
//! record.

use crate::catalog::{Local, Said};
use crate::hermes::{Hermes, Registration};
use crate::home::Home;
use crate::join::mark_registered_now;
use crate::settings::Settings;
use crate::store::{self, Current, DevicesLockError, Layout};

/// Saves the entry in Hermes again, with the reference to the key of its local API.
///
/// # Errors
/// What to say: AR-601 when Hermes did not save the entry, or a local problem.
pub async fn register(
    home: &Home,
    hermes: &Hermes,
    settings: &Settings,
) -> Result<Registration, Said> {
    if !home.root().is_dir() {
        return Err(Said::Local(Local::NothingToRegister));
    }
    // Look first: with nothing to register, no command of Hermes runs.
    let layout = locked_layout(home, settings).await?;
    if layout.current().is_none() && layout.request().is_none() {
        return Err(Said::Local(Local::NothingToRegister));
    }
    let registration = hermes
        .add(home, settings)
        .await
        .map_err(|error| error.said())?;
    let reference = registration == Registration::WithKeyReference;
    // The result is written down on what the folder holds now.
    let lock = lock(home, settings).await?;
    let written = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || -> Result<(), Said> {
            let layout = scan(&home)?;
            for folder in [layout.current(), layout.request()].into_iter().flatten() {
                mark_registered_now(&folder.dir, reference)
                    .map_err(|_| Said::Local(Local::Folder))?;
            }
            Ok(())
        })
        .await
    };
    drop(lock);
    written.map_err(|_| Said::Local(Local::Internal))??;
    Ok(registration)
}

async fn lock(home: &Home, settings: &Settings) -> Result<store::DevicesLock, Said> {
    store::lock_devices(home, settings.command_lock_ms)
        .await
        .map_err(|error| match error {
            DevicesLockError::Busy => Said::Local(Local::Busy),
            DevicesLockError::Lock(_) | DevicesLockError::Task => Said::Local(Local::Folder),
        })
}

fn scan(home: &Home) -> Result<Layout, Said> {
    let layout = store::scan(home).map_err(|_| Said::Local(Local::Folder))?;
    if layout.current == Current::Damaged
        || layout.folders.iter().any(|folder| folder.record.is_err())
    {
        return Err(Said::Local(Local::Damaged));
    }
    Ok(layout)
}

/// The folders, read under the lock, which is released before this returns.
async fn locked_layout(home: &Home, settings: &Settings) -> Result<Layout, Said> {
    let lock = lock(home, settings).await?;
    let layout = {
        let home = home.clone();
        tokio::task::spawn_blocking(move || scan(&home)).await
    };
    drop(lock);
    layout.map_err(|_| Said::Local(Local::Internal))?
}
