//! The durable write of a credential, the step before every "saved" (C7, rule 5).
//!
//! Under the lock of the device folders: a `join` that discards the folder of a request
//! without a credential checks for the credential under the same lock, so a credential never
//! arrives in a folder between that check and its removal.

use std::sync::Arc;

use arauto_platform::FsError;
use arauto_protocol::secret::SealedCredential;

use crate::home::Home;
use crate::settings::Settings;
use crate::store::{self, DeviceDir, DevicesLockError};

/// What a durable write of the credential of one device needs.
#[derive(Debug, Clone)]
pub(crate) struct Saving {
    pub(crate) home: Home,
    pub(crate) dir: DeviceDir,
    pub(crate) settings: Arc<Settings>,
}

impl Saving {
    #[cfg(feature = "testing")]
    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }
}

/// Why the credential was not written. In every case nothing is "saved".
#[derive(Debug, thiserror::Error)]
pub(crate) enum WriteError {
    /// The lock of the device folders stayed taken, or could not be tried.
    #[error("the device folders stayed busy: {0}")]
    Lock(#[from] DevicesLockError),
    /// The folder of the device is gone.
    #[error("the device folder is gone")]
    Gone,
    /// The write failed. With [`FsError::NotDurable`] the new credential is in place, and what
    /// the file holds is what is presented next time.
    #[error(transparent)]
    Fs(#[from] FsError),
    /// The task of the write did not return.
    #[error("the write did not return")]
    Task,
}

/// Writes `credential` into the folder of the device, durably.
pub(crate) async fn save_credential(
    saving: &Saving,
    credential: SealedCredential,
) -> Result<(), WriteError> {
    let _lock = store::lock_devices(&saving.home, saving.settings.devices_lock_ms).await?;
    if !saving.dir.exists() {
        return Err(WriteError::Gone);
    }
    let dir = saving.dir.clone();
    tokio::task::spawn_blocking(move || dir.write_credential(&credential))
        .await
        .map_err(|_| WriteError::Task)??;
    Ok(())
}
