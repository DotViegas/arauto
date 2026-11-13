//! The password of the database role of the hub, kept in the data folder.
//!
//! An installation through a web panel may start PostgreSQL with a provisional password,
//! the same for everyone and written in the file that describes the installation. The hub
//! replaces it with one it draws and nobody ever sees ([`crate::db::rotate_password`]), and
//! keeps it here. From then on every place that opens the database uses the address of
//! `ARAUTO_DATABASE_URL` with the password of this file ([`target`]).
//!
//! ```text
//! database.secret        the password in use: 64 lowercase hexadecimal characters
//! database.secret.next   the password being put in place: only between two steps of a
//!                        replacement, or after a crash in the middle of one
//! database.secret.lock   taken while a password is replaced
//! ```
//!
//! Known pieces only: 32 bytes from the generator of the system, written in hexadecimal, so
//! the password is plain ASCII and can never close the quotes of the statement that sets it.
//!
//! The password is never logged, printed or put in an error: [`DatabasePassword`] and
//! [`DatabaseTarget`] print `[redacted]`, and an error names the file, never its content.
//!
//! The functions that read or write the files block the calling thread.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use arauto_platform::{FsError, write_durable};
use data_encoding::HEXLOWER;
use sqlx::postgres::PgConnectOptions;
use zeroize::Zeroizing;

use crate::config::DatabaseUrl;

/// The file of the password in use, inside the data folder.
pub const DATABASE_SECRET_FILE: &str = "database.secret";

/// The file of the password being put in place, inside the data folder.
pub const DATABASE_SECRET_NEXT_FILE: &str = "database.secret.next";

/// The lock taken while a password is replaced, so that two replacements never cross.
pub const DATABASE_SECRET_LOCK_FILE: &str = "database.secret.lock";

/// How many random bytes a password has.
const PASSWORD_BYTES: usize = 32;

/// How many characters a password has: two hexadecimal digits per byte.
pub const PASSWORD_LEN: usize = PASSWORD_BYTES * 2;

/// What went wrong with the files of the password. No variant carries the password, nor what
/// a file holds.
#[derive(Debug, thiserror::Error)]
pub enum DatabasePasswordError {
    /// The file is there and does not hold a password.
    #[error(
        "{}: the file of the database password is damaged (it must hold exactly 64 lowercase \
         hexadecimal characters); its content is not shown",
        .path.display()
    )]
    Damaged {
        /// The file.
        path: PathBuf,
    },

    /// The file could not be read.
    #[error("could not read {}: {cause}", .path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },

    /// The file could not be written durably.
    #[error(transparent)]
    Fs(#[from] FsError),

    /// The leftover file of a replacement could not be removed.
    #[error("could not remove {}: {cause}", .path.display())]
    Remove {
        /// The file.
        path: PathBuf,
        /// The error reported by the system.
        cause: io::Error,
    },

    /// `ARAUTO_DATABASE_URL` is not an address PostgreSQL takes. The address is not shown: it
    /// may carry a password.
    #[error("ARAUTO_DATABASE_URL is not an address PostgreSQL takes")]
    Address,

    /// The random generator of the operating system failed.
    #[error("the random generator of the operating system failed")]
    Random,

    /// A blocking task did not return.
    #[error("a task that reads or writes the database password did not return")]
    Task,
}

/// A password of the database role: 64 lowercase hexadecimal characters. It prints
/// `[redacted]` and is wiped from memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct DatabasePassword(Zeroizing<String>);

impl fmt::Debug for DatabasePassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabasePassword([redacted])")
    }
}

impl DatabasePassword {
    /// Draws a new password: 32 bytes from the generator of the system.
    ///
    /// # Errors
    /// [`DatabasePasswordError::Random`] when the generator fails.
    pub fn generate() -> Result<Self, DatabasePasswordError> {
        let mut bytes = Zeroizing::new([0u8; PASSWORD_BYTES]);
        getrandom::fill(bytes.as_mut_slice()).map_err(|_| DatabasePasswordError::Random)?;
        Ok(Self(Zeroizing::new(HEXLOWER.encode(bytes.as_slice()))))
    }

    /// Reads a password: exactly 64 lowercase hexadecimal characters, and nothing else.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let valid = bytes.len() == PASSWORD_LEN
            && bytes
                .iter()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        let text = std::str::from_utf8(bytes).ok().filter(|_| valid)?;
        Some(Self(Zeroizing::new(text.to_owned())))
    }

    /// The password. Whoever calls this answers for where it goes: only into the options of
    /// a connection, the statement that sets it and its file.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// Where a pool connects: the options of PostgreSQL, password included. It prints
/// `[redacted]`.
#[derive(Clone)]
pub struct DatabaseTarget(PgConnectOptions);

impl fmt::Debug for DatabaseTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseTarget([redacted])")
    }
}

impl DatabaseTarget {
    /// The options, to open a pool with. Whoever calls this answers for where they go: their
    /// own `Debug` shows the password.
    #[must_use]
    pub fn options(&self) -> &PgConnectOptions {
        &self.0
    }

    /// The same target, with `password` in place of the one it had.
    #[must_use]
    pub fn with_password(&self, password: &DatabasePassword) -> Self {
        Self(self.0.clone().password(password.expose()))
    }
}

/// The address of `url`, with its password replaced by `password` when there is one. Every
/// other part of the address (user, host, port, database, options) stays as written. The
/// password given here wins over one written in the address, in its user part or in a
/// `password=` option.
///
/// # Errors
/// [`DatabasePasswordError::Address`] when PostgreSQL does not take the address.
pub fn target(
    url: &DatabaseUrl,
    password: Option<&DatabasePassword>,
) -> Result<DatabaseTarget, DatabasePasswordError> {
    let options =
        PgConnectOptions::from_str(url.expose()).map_err(|_| DatabasePasswordError::Address)?;
    let target = DatabaseTarget(options);
    Ok(match password {
        Some(password) => target.with_password(password),
        None => target,
    })
}

/// Reads the password of `path`. `None` when there is no such file: the address is then used
/// as given.
///
/// # Errors
/// [`DatabasePasswordError::Damaged`] when the file does not hold exactly one password;
/// [`DatabasePasswordError::Read`] when it cannot be read.
pub fn read(path: &Path) -> Result<Option<DatabasePassword>, DatabasePasswordError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => {
            return Err(DatabasePasswordError::Read {
                path: path.to_path_buf(),
                cause,
            });
        }
    };
    // One byte more than a password, to notice a file that is too long.
    let mut buffer = Zeroizing::new([0u8; PASSWORD_LEN + 1]);
    let mut filled = 0usize;
    while let Some(free) = buffer.get_mut(filled..).filter(|free| !free.is_empty()) {
        match file.read(free) {
            Ok(0) => break,
            Ok(count) => filled = filled.saturating_add(count),
            Err(cause) if cause.kind() == io::ErrorKind::Interrupted => {}
            Err(cause) => {
                return Err(DatabasePasswordError::Read {
                    path: path.to_path_buf(),
                    cause,
                });
            }
        }
    }
    buffer
        .get(..filled)
        .and_then(DatabasePassword::parse)
        .map(Some)
        .ok_or_else(|| DatabasePasswordError::Damaged {
            path: path.to_path_buf(),
        })
}

/// The password in use, from `database.secret` in `data_dir`.
///
/// # Errors
/// As [`read`].
pub fn read_current(data_dir: &Path) -> Result<Option<DatabasePassword>, DatabasePasswordError> {
    read(&data_dir.join(DATABASE_SECRET_FILE))
}

/// The password being put in place, from `database.secret.next` in `data_dir`.
///
/// # Errors
/// As [`read`].
pub fn read_next(data_dir: &Path) -> Result<Option<DatabasePassword>, DatabasePasswordError> {
    read(&data_dir.join(DATABASE_SECRET_NEXT_FILE))
}

/// Writes `password` to `database.secret.next`, durably, readable by the owner only: the first
/// step of a replacement, before the database is told.
///
/// # Errors
/// [`DatabasePasswordError::Fs`] when the file could not be made durable.
pub(crate) fn write_next(
    data_dir: &Path,
    password: &DatabasePassword,
) -> Result<(), DatabasePasswordError> {
    Ok(write_durable(
        &data_dir.join(DATABASE_SECRET_NEXT_FILE),
        password.expose().as_bytes(),
    )?)
}

/// Puts `password`, the one of `database.secret.next` that the database now has, under
/// `database.secret`, durably and readable by the owner only, and then removes
/// `database.secret.next`. A crash between the two leaves both files with the same password,
/// which the next opening settles.
///
/// # Errors
/// [`DatabasePasswordError::Fs`] when `database.secret` could not be made durable (the
/// password still waits in `database.secret.next`); [`DatabasePasswordError::Remove`] when
/// only the leftover stayed.
pub(crate) fn promote(
    data_dir: &Path,
    password: &DatabasePassword,
) -> Result<(), DatabasePasswordError> {
    write_durable(
        &data_dir.join(DATABASE_SECRET_FILE),
        password.expose().as_bytes(),
    )?;
    discard_next(data_dir)
}

/// Removes `database.secret.next`: the password in it was never given to the database, or is
/// already under `database.secret`.
///
/// # Errors
/// [`DatabasePasswordError::Remove`] when the file is there and could not be removed.
pub(crate) fn discard_next(data_dir: &Path) -> Result<(), DatabasePasswordError> {
    let path = data_dir.join(DATABASE_SECRET_NEXT_FILE);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(DatabasePasswordError::Remove { path, cause }),
    }
}

// ---------------------------------------------------------------------------- after a crash

/// How the first attempt to open the database ended, with the password in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstAttempt {
    /// The database took the connection.
    Connected,
    /// PostgreSQL refused the password (SQLSTATE `28P01` or `28000`).
    PasswordRefused,
    /// Anything else: the network, a database that does not answer, an address it refuses.
    Failed,
}

/// What the opening of the database does after its first attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextStep {
    /// The pool of the first attempt is the one. `discard_next`: a replacement stopped before
    /// the database had the new password, and `database.secret.next` goes away.
    Keep {
        /// Whether `database.secret.next` is removed.
        discard_next: bool,
    },
    /// A replacement stopped after the database had the new password: the password of
    /// `database.secret.next` is tried, and put under `database.secret` when it works.
    TryNext,
    /// The first attempt failed, and nothing else is tried.
    GiveUp,
}

/// The decision of the opening, apart from the connections. The password of
/// `database.secret.next` is only tried when PostgreSQL refused the password in use: a
/// network error says nothing about which password the database has.
#[must_use]
pub const fn next_step(first: FirstAttempt, next_exists: bool) -> NextStep {
    match (first, next_exists) {
        (FirstAttempt::Connected, discard_next) => NextStep::Keep { discard_next },
        (FirstAttempt::PasswordRefused, true) => NextStep::TryNext,
        (FirstAttempt::PasswordRefused | FirstAttempt::Failed, _) => NextStep::GiveUp,
    }
}

/// Whether a SQLSTATE is PostgreSQL refusing the password: `28P01` (the password is wrong) or
/// `28000` (the authorization is not valid).
#[must_use]
pub fn is_password_refusal(sqlstate: Option<&str>) -> bool {
    matches!(sqlstate, Some("28P01" | "28000"))
}

/// How a failed attempt to open the database ended.
#[must_use]
pub fn classify(error: &sqlx::Error) -> FirstAttempt {
    let code = error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code);
    if is_password_refusal(code.as_deref()) {
        FirstAttempt::PasswordRefused
    } else {
        FirstAttempt::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::ConnectOptions;

    const CANARY: &str = "c4n4ry-0f-th3-d4t4b4s3-p4ssw0rd";

    fn password(text: &str) -> DatabasePassword {
        DatabasePassword::parse(text.as_bytes()).unwrap()
    }

    fn a_password() -> DatabasePassword {
        password(&"0123456789abcdef".repeat(4))
    }

    /// The password of a target, as PostgreSQL would be sent it.
    fn password_of(target: &DatabaseTarget) -> Option<String> {
        let url = target.options().to_url_lossy();
        url.password().map(str::to_owned)
    }

    fn of(url: &str, password: Option<&DatabasePassword>) -> DatabaseTarget {
        target(&DatabaseUrl::new(url.to_owned()), password).unwrap()
    }

    #[test]
    fn a_drawn_password_is_64_lowercase_hexadecimal_characters_and_never_the_same() {
        let first = DatabasePassword::generate().unwrap();
        let second = DatabasePassword::generate().unwrap();
        assert_eq!(first.expose().len(), PASSWORD_LEN);
        assert!(
            first
                .expose()
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        );
        assert_ne!(first, second);
        assert_eq!(
            DatabasePassword::parse(first.expose().as_bytes()),
            Some(first)
        );
    }

    #[test]
    fn only_64_lowercase_hexadecimal_characters_are_a_password() {
        let good = "0123456789abcdef".repeat(4);
        assert!(DatabasePassword::parse(good.as_bytes()).is_some());
        for bad in [
            String::new(),
            good[..63].to_owned(),
            format!("{good}0"),
            format!("{good}\n"),
            format!(" {}", &good[1..]),
            good.to_ascii_uppercase(),
            good.replacen('0', "g", 1),
            good.replacen('0', "'", 1),
            CANARY.to_owned(),
        ] {
            assert!(DatabasePassword::parse(bad.as_bytes()).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn the_password_and_the_target_show_in_no_debug() {
        let secret = a_password();
        let target = of("postgres://hub@db.example.com/arauto", Some(&secret));
        for shown in [
            format!("{secret:?}"),
            format!("{secret:#?}"),
            format!("{target:?}"),
            format!("{target:#?}"),
        ] {
            assert!(!shown.contains(secret.expose()), "{shown}");
            assert!(shown.contains("[redacted]"), "{shown}");
        }
    }

    // ------------------------------------------------------------------ the address

    #[test]
    fn the_address_gets_the_password_of_the_file_and_keeps_everything_else() {
        let secret = a_password();
        for url in [
            // No password in the address.
            "postgres://hub@db.example.com:5433/arauto",
            // A password with characters that must be percent-encoded.
            "postgres://hub:pro%40vis%3Aional%2Fpass@db.example.com:5433/arauto",
            // The `postgresql://` scheme.
            "postgresql://hub:provisional@db.example.com:5433/arauto",
            // A query string, one of whose options is a password too.
            "postgres://hub:provisional@db.example.com:5433/arauto?sslmode=disable&password=other&application_name=hub",
        ] {
            let target = of(url, Some(&secret));
            let options = target.options();
            assert_eq!(
                password_of(&target).as_deref(),
                Some(secret.expose()),
                "{url}"
            );
            assert_eq!(options.get_username(), "hub", "{url}");
            assert_eq!(options.get_host(), "db.example.com", "{url}");
            assert_eq!(options.get_port(), 5433, "{url}");
            assert_eq!(options.get_database(), Some("arauto"), "{url}");
        }
        let with_query = of(
            "postgres://hub:provisional@db.example.com/arauto?sslmode=disable&application_name=hub",
            Some(&secret),
        );
        assert!(matches!(
            with_query.options().get_ssl_mode(),
            sqlx::postgres::PgSslMode::Disable
        ));
        assert_eq!(with_query.options().get_application_name(), Some("hub"));
    }

    #[test]
    fn without_the_file_the_address_is_used_as_given() {
        let target = of(
            "postgres://hub:pro%40vis%3Aional@db.example.com/arauto",
            None,
        );
        assert_eq!(password_of(&target).as_deref(), Some("pro%40vis%3Aional"));
        let without = of("postgres://hub@db.example.com/arauto", None);
        assert_eq!(without.options().get_username(), "hub");
    }

    #[test]
    fn a_user_without_a_password_gets_the_one_of_the_file() {
        let secret = a_password();
        let target = of("postgres://hub@db.example.com/arauto", Some(&secret));
        assert_eq!(password_of(&target).as_deref(), Some(secret.expose()));
        assert_eq!(target.options().get_username(), "hub");
        // And the password is replaced again, not added to.
        let other = DatabasePassword::generate().unwrap();
        let again = target.with_password(&other);
        assert_eq!(password_of(&again).as_deref(), Some(other.expose()));
    }

    #[test]
    fn an_address_postgresql_does_not_take_is_refused_without_being_repeated() {
        let url = format!("postgres://hub:{CANARY}@[not-a-host/arauto");
        let error = target(&DatabaseUrl::new(url), None).unwrap_err();
        assert!(matches!(error, DatabasePasswordError::Address));
        assert!(!error.to_string().contains(CANARY));
        assert!(!format!("{error:?}").contains(CANARY));
    }

    // ------------------------------------------------------------------ the files

    #[test]
    fn a_missing_file_means_the_address_as_given() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_current(dir.path()).unwrap(), None);
        assert_eq!(read_next(dir.path()).unwrap(), None);
        assert_eq!(read_current(&dir.path().join("nowhere")).unwrap(), None);
    }

    #[test]
    fn a_damaged_file_is_an_error_that_names_it_and_never_shows_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DATABASE_SECRET_FILE);
        let good = "0123456789abcdef".repeat(4);
        for content in [
            CANARY.to_owned(),
            String::new(),
            format!("{good}\n"),
            good.to_ascii_uppercase(),
            format!("{CANARY}{}", "a".repeat(200)),
        ] {
            fs::write(&path, &content).unwrap();
            let error = read_current(dir.path()).unwrap_err();
            assert!(
                matches!(&error, DatabasePasswordError::Damaged { path: named } if *named == path),
                "{error:?}"
            );
            let text = format!("{error} {error:?}");
            assert!(text.contains(DATABASE_SECRET_FILE), "{text}");
            if !content.is_empty() {
                assert!(!text.contains(content.trim()), "{text}");
                assert!(!text.contains(&content[..8]), "{text}");
            }
        }
    }

    #[test]
    fn the_steps_of_the_files_leave_one_password_under_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let first = a_password();
        let second = DatabasePassword::generate().unwrap();

        // The first step: the new password waits in its own file; the one in use stays.
        write_next(dir.path(), &first).unwrap();
        assert_eq!(read_next(dir.path()).unwrap().as_ref(), Some(&first));
        assert_eq!(read_current(dir.path()).unwrap(), None);
        // The database has it: it goes under its name, and the other file goes away.
        promote(dir.path(), &first).unwrap();
        assert_eq!(read_current(dir.path()).unwrap().as_ref(), Some(&first));
        assert_eq!(read_next(dir.path()).unwrap(), None);
        // A replacement the database never saw is dropped, and the one in use stays.
        write_next(dir.path(), &second).unwrap();
        discard_next(dir.path()).unwrap();
        discard_next(dir.path()).unwrap();
        assert_eq!(read_current(dir.path()).unwrap().as_ref(), Some(&first));
        assert_eq!(read_next(dir.path()).unwrap(), None);
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [DATABASE_SECRET_FILE]);
        assert_eq!(
            fs::read(dir.path().join(DATABASE_SECRET_FILE)).unwrap(),
            first.expose().as_bytes()
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_files_of_the_password_are_readable_by_the_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let password = a_password();
        write_next(dir.path(), &password).unwrap();
        let mode = |name: &str| {
            fs::metadata(dir.path().join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        };
        assert_eq!(mode(DATABASE_SECRET_NEXT_FILE), 0o600);
        promote(dir.path(), &password).unwrap();
        assert_eq!(mode(DATABASE_SECRET_FILE), 0o600);
    }

    // ------------------------------------------------------------------ after a crash

    #[test]
    fn the_password_of_the_next_file_is_tried_only_when_postgresql_refused_the_password() {
        use FirstAttempt::{Connected, Failed, PasswordRefused};
        // The replacement stopped before the database had the new password: the password in
        // use works, and the other file goes away.
        assert_eq!(
            next_step(Connected, true),
            NextStep::Keep { discard_next: true }
        );
        assert_eq!(
            next_step(Connected, false),
            NextStep::Keep {
                discard_next: false
            }
        );
        // The replacement stopped after: the password in use is refused, the new one is tried.
        assert_eq!(next_step(PasswordRefused, true), NextStep::TryNext);
        // Nothing to try.
        assert_eq!(next_step(PasswordRefused, false), NextStep::GiveUp);
        // A network error says nothing about which password the database has.
        assert_eq!(next_step(Failed, true), NextStep::GiveUp);
        assert_eq!(next_step(Failed, false), NextStep::GiveUp);
    }

    #[test]
    fn only_a_refusal_of_the_password_counts_as_one() {
        assert!(is_password_refusal(Some("28P01")));
        assert!(is_password_refusal(Some("28000")));
        for other in [
            None,
            Some("08006"),
            Some("3D000"),
            Some("57P03"),
            Some("28P0"),
        ] {
            assert!(!is_password_refusal(other), "{other:?}");
        }
        assert_eq!(classify(&sqlx::Error::PoolTimedOut), FirstAttempt::Failed);
        assert_eq!(
            classify(&sqlx::Error::Io(io::Error::other("refused"))),
            FirstAttempt::Failed
        );
    }
}
