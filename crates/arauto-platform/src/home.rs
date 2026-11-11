//! The default folder of the user.

use std::ffi::OsString;
use std::path::PathBuf;

/// The environment variable that holds the home folder of the user.
#[cfg(unix)]
const HOME_VARIABLE: &str = "HOME";
/// The environment variable that holds the home folder of the user.
#[cfg(windows)]
const HOME_VARIABLE: &str = "USERPROFILE";

/// The name of the Arauto folder inside the home folder of the user.
const FOLDER_NAME: &str = ".arauto";

/// Why the default folder of the user could not be named.
#[derive(Debug, thiserror::Error)]
pub enum HomeError {
    /// The variable is not set, or is set to nothing.
    #[error(
        "the environment variable {variable} is not set, so there is no default folder; \
         name the folder explicitly"
    )]
    NotSet {
        /// The variable that was looked up.
        variable: &'static str,
    },

    /// The variable holds a relative path, which would name a different folder from each
    /// working directory.
    #[error(
        "the environment variable {variable} holds {}, which is not an absolute path; \
         name the folder explicitly",
        .value.display()
    )]
    NotAbsolute {
        /// The variable that was looked up.
        variable: &'static str,
        /// What it holds.
        value: PathBuf,
    },
}

/// The default Arauto folder of the user: `$HOME/.arauto` on Unix and
/// `%USERPROFILE%\.arauto` on Windows.
///
/// The folder is only named, not created; see
/// [`create_private_dir`](crate::create_private_dir).
///
/// # Errors
///
/// [`HomeError::NotSet`] when the variable is missing or empty, and
/// [`HomeError::NotAbsolute`] when it does not hold an absolute path.
pub fn default_home() -> Result<PathBuf, HomeError> {
    home_under(std::env::var_os(HOME_VARIABLE))
}

/// The rule of [`default_home`], apart from where the value comes from.
fn home_under(value: Option<OsString>) -> Result<PathBuf, HomeError> {
    let base = match value {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => {
            return Err(HomeError::NotSet {
                variable: HOME_VARIABLE,
            });
        }
    };
    if !base.is_absolute() {
        return Err(HomeError::NotAbsolute {
            variable: HOME_VARIABLE,
            value: base,
        });
    }
    Ok(base.join(FOLDER_NAME))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[cfg(unix)]
    const SOME_HOME: &str = "/home/hermes";
    #[cfg(windows)]
    const SOME_HOME: &str = r"C:\Users\hermes";

    #[test]
    fn with_the_variable_the_folder_is_inside_the_home_of_the_user() {
        let home = home_under(Some(OsString::from(SOME_HOME))).unwrap();

        assert_eq!(home, Path::new(SOME_HOME).join(".arauto"));
        assert_eq!(home.parent(), Some(Path::new(SOME_HOME)));
        assert_eq!(home.file_name().unwrap(), ".arauto");
    }

    #[test]
    fn without_the_variable_the_error_names_it() {
        let error = home_under(None).unwrap_err();

        assert!(matches!(error, HomeError::NotSet { variable } if variable == HOME_VARIABLE));
        assert!(error.to_string().contains(HOME_VARIABLE), "{error}");
    }

    #[test]
    fn empty_variable_counts_as_missing() {
        let error = home_under(Some(OsString::new())).unwrap_err();

        assert!(matches!(error, HomeError::NotSet { .. }), "{error}");
    }

    /// A relative home would make the folder, and with it the identity, depend on the
    /// working directory.
    #[test]
    fn relative_variable_is_refused() {
        for relative in ["hermes", ".", "..", "home/hermes"] {
            let error = home_under(Some(OsString::from(relative))).unwrap_err();

            assert!(
                matches!(&error, HomeError::NotAbsolute { value, .. } if value == Path::new(relative)),
                "{relative}: {error}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn path_without_a_drive_is_refused() {
        let error = home_under(Some(OsString::from(r"\Users\hermes"))).unwrap_err();

        assert!(matches!(error, HomeError::NotAbsolute { .. }), "{error}");
    }

    #[test]
    fn default_home_reads_the_variable_of_this_system() {
        // Reads the environment and never changes it: other tests run in this process.
        #[cfg(unix)]
        let variable = "HOME";
        #[cfg(windows)]
        let variable = "USERPROFILE";

        match std::env::var_os(variable) {
            Some(value) if Path::new(&value).is_absolute() => {
                assert_eq!(default_home().unwrap(), Path::new(&value).join(".arauto"));
            }
            _ => assert!(default_home().is_err()),
        }
    }
}
