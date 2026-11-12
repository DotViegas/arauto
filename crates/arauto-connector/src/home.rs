//! The folder of the connector (`docs/11-conector.md`, section 2).
//!
//! ```text
//! ~/.arauto/
//!   arauto-connector      the program, at the fixed path registered in Hermes
//!   current               which device folder is in use
//!   devices/{id}/         one folder per device or request
//!     identity.key        SECRET: the identity of the device
//!     credential.seal     SECRET: the latest credential received from the hub
//!     hub.json            address and key of the hub, digest of the invite, local situation
//!   state.json            the tools of the last session
//!   lock                  the lock of the leader
//!   devices.lock          the lock of the device folders: join, leave and the writes of the leader
//!   channel.sock          the local socket between the copies (Unix)
//! ```
//!
//! `--home`, else `ARAUTO_HOME`, else `~/.arauto`. Another folder than the default goes into
//! the registration in Hermes, which starts the connector with a filtered environment.

use std::path::{Path, PathBuf};

use arauto_platform::{FsError, LOCK_FILE_NAME, create_private_dir, default_home};

/// The name of the program at its fixed path.
pub const PROGRAM_NAME: &str = "arauto-connector";

/// The folder of the device folders.
pub(crate) const DEVICES_DIR: &str = "devices";

/// The file that says which device folder is in use.
pub(crate) const CURRENT_FILE: &str = "current";

/// The tools of the last session.
pub(crate) const STATE_FILE: &str = "state.json";

/// The lock of the device folders. It is not the lock of the leader: `join` takes it while a
/// leader runs.
pub(crate) const DEVICES_LOCK_FILE: &str = "devices.lock";

/// Why the folder of the connector could not be named.
#[derive(Debug, thiserror::Error)]
pub enum HomeError {
    /// The folder given is not an absolute path.
    #[error("the folder of the connector must be an absolute path")]
    NotAbsolute,
    /// No folder was given and the default one could not be named.
    #[error(transparent)]
    NoDefault(#[from] arauto_platform::HomeError),
}

/// The folder of the connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    root: PathBuf,
    default: bool,
}

impl Home {
    /// The folder of the connector: `explicit` (from `--home` or `ARAUTO_HOME`), else the
    /// default of the user.
    ///
    /// # Errors
    /// [`HomeError::NotAbsolute`] for a relative path; [`HomeError::NoDefault`] when no folder
    /// was given and the user has none.
    pub fn resolve(explicit: Option<&Path>) -> Result<Self, HomeError> {
        match explicit {
            Some(path) => Self::at(path),
            None => Ok(Self {
                root: default_home()?,
                default: true,
            }),
        }
    }

    /// The folder `path`, which must be absolute.
    ///
    /// # Errors
    /// [`HomeError::NotAbsolute`].
    pub fn at(path: &Path) -> Result<Self, HomeError> {
        if !path.is_absolute() {
            return Err(HomeError::NotAbsolute);
        }
        let default = default_home().is_ok_and(|default| default == path);
        Ok(Self {
            root: path.to_path_buf(),
            default,
        })
    }

    /// The folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether this is the default folder of the user: otherwise the registration in Hermes
    /// names it.
    #[must_use]
    pub const fn is_default(&self) -> bool {
        self.default
    }

    /// The fixed path of the program, the one registered in Hermes.
    #[must_use]
    pub fn program(&self) -> PathBuf {
        self.root
            .join(format!("{PROGRAM_NAME}{}", std::env::consts::EXE_SUFFIX))
    }

    pub(crate) fn devices(&self) -> PathBuf {
        self.root.join(DEVICES_DIR)
    }

    pub(crate) fn current_file(&self) -> PathBuf {
        self.root.join(CURRENT_FILE)
    }

    pub(crate) fn state_file(&self) -> PathBuf {
        self.root.join(STATE_FILE)
    }

    pub(crate) fn lock_file(&self) -> PathBuf {
        self.root.join(LOCK_FILE_NAME)
    }

    pub(crate) fn devices_lock_file(&self) -> PathBuf {
        self.root.join(DEVICES_LOCK_FILE)
    }

    /// Creates the folder and the folder of the devices, closed to other users, or checks that
    /// they are.
    ///
    /// # Errors
    /// [`FsError`]: the folder is not this user's, is a link, or cannot be created.
    pub fn prepare(&self) -> Result<(), FsError> {
        create_private_dir(&self.root)?;
        create_private_dir(&self.devices())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_folder_is_refused() {
        assert!(matches!(
            Home::at(Path::new("relative/folder")),
            Err(HomeError::NotAbsolute)
        ));
        assert!(matches!(
            Home::resolve(Some(Path::new("x"))),
            Err(HomeError::NotAbsolute)
        ));
    }

    #[test]
    fn the_files_live_in_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(dir.path()).unwrap();
        assert!(!home.is_default());
        assert_eq!(home.lock_file(), dir.path().join("lock"));
        assert_eq!(home.devices(), dir.path().join("devices"));
        assert_eq!(home.current_file(), dir.path().join("current"));
        assert_eq!(home.state_file(), dir.path().join("state.json"));
        assert!(
            home.program()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("arauto-connector")
        );
        home.prepare().unwrap();
        assert!(home.devices().is_dir());
        // Twice is fine.
        home.prepare().unwrap();
    }

    #[test]
    fn the_default_folder_is_known_as_such() {
        if let Ok(default) = default_home() {
            assert!(Home::at(&default).unwrap().is_default());
            assert!(Home::resolve(None).unwrap().is_default());
        }
    }
}
