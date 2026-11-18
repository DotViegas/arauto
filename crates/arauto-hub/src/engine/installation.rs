//! The engine that the installation brings (`docs/decisoes/0052-a-instalacao-traz-o-motor.md`):
//! the key the hub draws for it, the row that records it, and the digest file the engine reads
//! its key from.
//!
//! | What | Where |
//! |---|---|
//! | The draw and the digest | [`draw_key`], [`digest_contents`], [`is_digest_file_contents`] |
//! | The row | [`crate::repo::engine::seed_installation`], called by [`ensure`] |
//! | The file | [`settle_digest_file`], called by [`ensure`] |
//!
//! **The file is derived only from the committed row.** [`settle_digest_file`] takes the lock of
//! the folder, reads the row of the engine from the database, opens its key with the vault and
//! only then writes or removes the file, all while it holds the lock. It never writes from a key
//! it holds in memory, such as the key it has just drawn: a writer that lost the race to insert
//! the row would otherwise write a key that is in no row. Anything that changes the key of the
//! engine later must commit and then call [`settle_digest_file`]: whoever takes the lock last
//! reads the last committed key, so the file ends up equal to the digest of the row.
//!
//! The file holds the digest of the key, never the key. The key is sealed in the row and sent to
//! the engine in the header; it never reaches a log, an audit row, an alert or a file.

use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use arauto_platform::{FsError, LockError, remove_stale_temporaries, write_durable_readable};
use data_encoding::HEXLOWER;
use sha2::{Digest, Sha512};
use zeroize::Zeroizing;

use super::ApiKey;
use crate::config::InstallationEngineConfig;
use crate::db::{self, DbError, ResolvedOrg, SystemTask};
use crate::lock_wait::acquire_within;
use crate::mark::Mark;
use crate::repo::RepoError;
use crate::repo::engine::{self, EngineOrigin, EngineRow, Seeded};
use crate::vault::Vault;

/// The digest file, in the folder of `ARAUTO_ENGINE_DIGEST_DIR`. The wrapper of the engine reads
/// this name.
pub const DIGEST_FILE: &str = "api-key.sha512";

/// The lock of the folder: every writer of the folder takes it.
pub const DIGEST_LOCK_FILE: &str = "api-key.lock";

/// How long a start waits for the lock of the folder before it gives up.
pub const LOCK_PATIENCE: Duration = Duration::from_secs(10);

/// The most the hub reads of the digest file: one byte more than the file has, so a longer file
/// is told from a right one.
const READ_CEILING: u64 = 136;

/// The prefix of the file: the form the engine accepts a digest in.
const DIGEST_PREFIX: &str = "sha512:";

/// 7 bytes of prefix and 128 hexadecimal digits of SHA-512. No line end.
const DIGEST_FILE_LEN: usize = 135;

/// How many bytes the key is drawn from.
const KEY_BYTES: usize = 32;

/// What the start of the installation engine could not do.
#[derive(Debug, thiserror::Error)]
pub enum InstallationEngineError {
    /// The generator of the system failed.
    #[error("the random generator of the system failed")]
    Random,
    /// The folder stayed locked by another process for the whole patience. The file is as it
    /// was.
    #[error(
        "ARAUTO_ENGINE_DIGEST_DIR ({}): the folder stayed locked by another process; the \
         digest file was not changed",
        .path.display()
    )]
    DigestDirLocked {
        /// The lock file.
        path: PathBuf,
    },
    /// The lock of the folder could not be taken for another reason.
    #[error(
        "ARAUTO_ENGINE_DIGEST_DIR ({}): could not lock the folder ({source}); it must exist \
         and belong to the user the hub runs as",
        .path.display()
    )]
    DigestDir {
        /// The folder.
        path: PathBuf,
        /// What the lock said.
        source: LockError,
    },
    /// The digest file or its folder could not be read, written or removed.
    #[error(
        "ARAUTO_ENGINE_DIGEST_DIR ({}): could not settle the digest file; the folder must \
         belong to the user the hub runs as: {source}",
        .path.display()
    )]
    File {
        /// The folder.
        path: PathBuf,
        /// What the system said.
        source: FsError,
    },
    /// The digest file could not be read.
    #[error("ARAUTO_ENGINE_DIGEST_DIR ({}): could not read the digest file: {cause}", .path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// What the system said.
        cause: std::io::Error,
    },
    /// The database refused.
    #[error(transparent)]
    Db(#[from] DbError),
    /// A repository function failed, or a stored row is not what the program wrote.
    #[error(transparent)]
    Repo(#[from] RepoError),
    /// A blocking task did not return.
    #[error("a task of the start of the engine of the installation did not return")]
    Task,
}

/// What [`settle_digest_file`] did to the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOutcome {
    /// The file was absent or wrong, and is now right.
    Written,
    /// The file was already right: it was left alone.
    Unchanged,
    /// The file was there and should not be: it is gone.
    Removed,
    /// There was no file and there should be none.
    Absent,
}

impl FileOutcome {
    /// A fixed word for the log.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Written => "written",
            Self::Unchanged => "unchanged",
            Self::Removed => "removed",
            Self::Absent => "absent",
        }
    }
}

/// What the file settled to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    /// The origin of the engine row, or `None` when the organization has no engine.
    pub origin: Option<EngineOrigin>,
    /// What happened to the file.
    pub file: FileOutcome,
}

/// What [`ensure`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ensured {
    /// Nothing was touched: the organization is not one (none, or more than one).
    Skipped,
    /// The row and the file were settled.
    Settled {
        /// This start created the row.
        seeded: bool,
        /// What the row and the file ended as.
        settled: Settled,
    },
}

/// Draws the key of the engine: 32 bytes from the generator of the system, written as 64
/// lowercase hexadecimal characters. That string is the key: it is what is sealed in the row,
/// what goes in the header to the engine and what is hashed into the digest.
///
/// # Errors
/// [`InstallationEngineError::Random`] when the generator of the system fails.
pub fn draw_key() -> Result<ApiKey, InstallationEngineError> {
    let mut bytes = Zeroizing::new([0u8; KEY_BYTES]);
    getrandom::fill(bytes.as_mut_slice()).map_err(|_| InstallationEngineError::Random)?;
    let text = Zeroizing::new(HEXLOWER.encode(bytes.as_slice()));
    ApiKey::parse(&text).map_err(|_| InstallationEngineError::Random)
}

/// Whether `text` is a key as [`draw_key`] writes it: 64 lowercase hexadecimal characters.
fn is_drawn_key(text: &str) -> bool {
    text.len() == KEY_BYTES * 2 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The contents of the digest file for `key`: `sha512:` and the 128 lowercase hexadecimal
/// digits of the SHA-512 of the bytes of the key. 135 bytes, no line end. It is what the engine
/// takes as its key in place of the key itself.
#[must_use]
pub fn digest_contents(key: &ApiKey) -> String {
    format!(
        "{DIGEST_PREFIX}{}",
        HEXLOWER.encode(&Sha512::digest(key.expose().as_bytes()))
    )
}

/// Whether `bytes` are exactly the contents of a digest file: 135 bytes, `sha512:` and 128
/// lowercase hexadecimal digits.
#[must_use]
pub fn is_digest_file_contents(bytes: &[u8]) -> bool {
    bytes.len() == DIGEST_FILE_LEN
        && bytes.starts_with(DIGEST_PREFIX.as_bytes())
        && bytes[DIGEST_PREFIX.len()..]
            .iter()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The key of an engine row of the installation, opened with the vault. It must be a key the
/// hub drew; anything else is a row the program did not write.
fn drawn_key_of(row: &EngineRow, vault: &Vault) -> Result<ApiKey, InstallationEngineError> {
    let secrets = row.secrets(vault)?;
    let key = secrets.key.ok_or(RepoError::Inconsistent(
        "the engine of the installation has no key",
    ))?;
    if !is_drawn_key(key.expose()) {
        return Err(RepoError::Inconsistent(
            "the key of the engine of the installation is not one the hub drew",
        )
        .into());
    }
    Ok(key)
}

/// Settles the engine of the installation at the start of `serve`: creates its row when the
/// organization has none, and then settles the digest file from the committed row
/// ([`settle_digest_file`]).
///
/// - No engine: the hub draws a key and writes the row in one write; a second hub that starts at
///   the same moment writes nothing, and both end with the same row and the same file.
/// - An engine of the installation: its address stays as it is, with a warning if it differs
///   from the configuration, and the file is written if it is absent or wrong.
/// - An engine an admin set: nothing of it changes, with a warning, and the file is removed.
/// - More than one organization: nothing is touched, with a warning. The panel serves an
///   organization only when there is exactly one, and so does this.
///
/// # Errors
/// [`InstallationEngineError`]; the hub does not start, so the engine, which waits for the hub
/// to be healthy, does not start either.
pub async fn ensure(
    pool: &sqlx::postgres::PgPool,
    mark: &Mark,
    vault: &Vault,
    config: &InstallationEngineConfig,
    now: i64,
) -> Result<Ensured, InstallationEngineError> {
    ensure_with(
        pool,
        mark,
        vault,
        config,
        now,
        LOCK_PATIENCE,
        std::future::ready(()),
    )
    .await
}

/// [`ensure`] with the patience for the lock, and a future that is awaited after the row is read
/// and before the file is written, while the lock is held. For the tests that put a change of
/// the key at that moment.
///
/// # Errors
/// As [`ensure`].
#[doc(hidden)]
pub async fn ensure_with(
    pool: &sqlx::postgres::PgPool,
    mark: &Mark,
    vault: &Vault,
    config: &InstallationEngineConfig,
    now: i64,
    patience: Duration,
    after_read: impl Future<Output = ()> + Send,
) -> Result<Ensured, InstallationEngineError> {
    let organizations =
        db::organizations_for_system_task(pool, SystemTask::InstallationEngine).await?;
    let [organization] = organizations.as_slice() else {
        tracing::warn!(
            "the engine of the installation needs exactly one organization; nothing was touched"
        );
        return Ok(Ensured::Skipped);
    };
    let organization = *organization;
    let mut seeded = false;
    match engine::get(pool, organization).await? {
        None => {
            let key = draw_key()?;
            let address = &config.address;
            let events_url = &config.events_url;
            seeded = matches!(
                engine::seed_installation(
                    pool,
                    mark,
                    organization,
                    vault,
                    address,
                    events_url,
                    &key,
                    now
                )
                .await?,
                Seeded::Created(_)
            );
        }
        Some(row) if row.origin() == EngineOrigin::Installation => {
            if row.address != config.address.http().to_text()
                || row.events_url != config.events_url.to_text()
            {
                tracing::warn!(
                    "the engine row of the installation differs from ARAUTO_ENGINE_URL or \
                     ARAUTO_ENGINE_EVENTS_URL; the row is kept"
                );
            }
        }
        Some(_) => {
            tracing::warn!(
                "an engine set by an admin is in use; the engine brought by the installation \
                 stays off"
            );
        }
    }
    let settled = settle_digest_file_with(
        pool,
        vault,
        organization,
        &config.digest_dir,
        patience,
        after_read,
    )
    .await?;
    tracing::info!(
        row = if seeded { "seeded" } else { "kept" },
        origin = settled.origin.map_or("none", EngineOrigin::as_str),
        file = settled.file.word(),
        "the engine of the installation was checked"
    );
    Ok(Ensured::Settled { seeded, settled })
}

/// Settles the digest file in `dir` with what the engine row of `organization` says now.
///
/// It takes the lock of the folder, reads the committed row, opens its key and writes, repairs
/// or removes the file, and releases the lock. The file exists only for a row of origin
/// `installation`, and then it holds the digest of the key of that row; for any other row, or
/// no row, it is removed. Every writer of the folder calls this and no other function writes
/// there.
///
/// # Errors
/// [`InstallationEngineError::DigestDirLocked`] when the lock was not free within `patience`;
/// the other variants for the folder, the file, the database and the vault. The file is not
/// changed on an error before the write.
pub async fn settle_digest_file(
    pool: &sqlx::postgres::PgPool,
    vault: &Vault,
    organization: ResolvedOrg,
    dir: &Path,
    patience: Duration,
) -> Result<Settled, InstallationEngineError> {
    settle_digest_file_with(
        pool,
        vault,
        organization,
        dir,
        patience,
        std::future::ready(()),
    )
    .await
}

/// [`settle_digest_file`] with a future that is awaited after the row is read and before the
/// file is written, while the lock is held.
///
/// # Errors
/// As [`settle_digest_file`].
#[doc(hidden)]
pub async fn settle_digest_file_with(
    pool: &sqlx::postgres::PgPool,
    vault: &Vault,
    organization: ResolvedOrg,
    dir: &Path,
    patience: Duration,
    after_read: impl Future<Output = ()> + Send,
) -> Result<Settled, InstallationEngineError> {
    let lock_path = dir.join(DIGEST_LOCK_FILE);
    let lock = tokio::task::spawn_blocking({
        let lock_path = lock_path.clone();
        move || acquire_within(&lock_path, patience)
    })
    .await
    .map_err(|_| InstallationEngineError::Task)?
    .map_err(|error| match error {
        LockError::AlreadyHeld { path } => InstallationEngineError::DigestDirLocked { path },
        source => InstallationEngineError::DigestDir {
            path: dir.to_owned(),
            source,
        },
    })?;
    // Under the lock: what is committed now is what the file is made from.
    let row = engine::get(pool, organization).await?;
    let origin = row.as_ref().map(EngineRow::origin);
    let wanted = match &row {
        Some(row) if row.origin() == EngineOrigin::Installation => {
            Some(digest_contents(&drawn_key_of(row, vault)?))
        }
        _ => None,
    };
    drop(row);
    after_read.await;
    let folder = dir.to_owned();
    let file = tokio::task::spawn_blocking(move || settle_files(&folder, wanted.as_deref()))
        .await
        .map_err(|_| InstallationEngineError::Task)??;
    drop(lock);
    Ok(Settled { origin, file })
}

/// The files, under the lock: the leftovers of an interrupted write go, then the file is
/// compared and written or removed. Blocking.
fn settle_files(dir: &Path, wanted: Option<&str>) -> Result<FileOutcome, InstallationEngineError> {
    let at_folder = |source| InstallationEngineError::File {
        path: dir.to_owned(),
        source,
    };
    remove_stale_temporaries(dir).map_err(at_folder)?;
    let path = dir.join(DIGEST_FILE);
    let current = read_bounded(&path)?;
    match wanted {
        Some(wanted) => {
            if current.as_deref() == Some(wanted.as_bytes()) {
                return Ok(FileOutcome::Unchanged);
            }
            write_durable_readable(&path, wanted.as_bytes()).map_err(at_folder)?;
            Ok(FileOutcome::Written)
        }
        None => {
            if current.is_none() {
                return Ok(FileOutcome::Absent);
            }
            match std::fs::remove_file(&path) {
                Ok(()) => Ok(FileOutcome::Removed),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(FileOutcome::Removed)
                }
                Err(cause) => Err(InstallationEngineError::File {
                    path: dir.to_owned(),
                    source: FsError::Io {
                        action: "remove the digest file",
                        path,
                        cause,
                    },
                }),
            }
        }
    }
}

/// The file, read up to [`READ_CEILING`] bytes. `None` when it is not there.
fn read_bounded(path: &Path) -> Result<Option<Vec<u8>>, InstallationEngineError> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => {
            return Err(InstallationEngineError::Read {
                path: path.to_owned(),
                cause,
            });
        }
    };
    let mut bytes = Vec::new();
    file.take(READ_CEILING)
        .read_to_end(&mut bytes)
        .map_err(|cause| InstallationEngineError::Read {
            path: path.to_owned(),
            cause,
        })?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTOR_KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const VECTOR_FILE: &str = "sha512:7f9d357a1a2cfb5d7367e7393a4b0594ae88bf2eefa37ced94e8cd4ed64b1e2e87428f37d98e8d5c9297688a88541f319d409a99412549e39fdefcebc57956d3";

    #[test]
    fn the_digest_of_the_known_key_is_the_known_file() {
        let key = ApiKey::parse(VECTOR_KEY).unwrap();
        let contents = digest_contents(&key);
        assert_eq!(contents, VECTOR_FILE);
        assert_eq!(contents.len(), 135);
        assert!(!contents.ends_with('\n'));
        assert!(is_digest_file_contents(contents.as_bytes()));
    }

    #[test]
    fn the_digest_is_of_the_key_text_and_not_of_its_bytes_or_of_a_line() {
        // The WAHA hashes the text it is given: `echo -n "$key" | sha512sum`.
        let with_line = ApiKey::parse(&format!("{VECTOR_KEY}\n")).unwrap();
        assert_eq!(digest_contents(&with_line), VECTOR_FILE);
        let other = ApiKey::parse(&VECTOR_KEY.to_uppercase()).unwrap();
        assert_ne!(digest_contents(&other), VECTOR_FILE);
    }

    #[test]
    fn a_digest_file_is_exactly_135_bytes_of_the_right_shape() {
        let good = VECTOR_FILE.as_bytes();
        assert!(is_digest_file_contents(good));
        let mut line = good.to_vec();
        line.push(b'\n');
        assert!(!is_digest_file_contents(&line));
        assert!(!is_digest_file_contents(&good[..134]));
        assert!(!is_digest_file_contents(b""));
        assert!(!is_digest_file_contents(
            VECTOR_FILE.replace("sha512:", "sha256:").as_bytes()
        ));
        assert!(!is_digest_file_contents(
            VECTOR_FILE
                .to_uppercase()
                .replace("SHA512:", "sha512:")
                .as_bytes()
        ));
        let mut bad = good.to_vec();
        bad[10] = b'g';
        assert!(!is_digest_file_contents(&bad));
    }

    #[test]
    fn a_drawn_key_is_64_lowercase_hexadecimal_and_never_the_same() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..64 {
            let key = draw_key().unwrap();
            assert_eq!(key.expose().len(), 64);
            assert!(is_drawn_key(key.expose()), "the key is not lowercase hex");
            assert!(seen.insert(key.expose().to_owned()), "a key came twice");
        }
    }

    #[test]
    fn only_a_key_as_the_draw_writes_it_counts_as_drawn() {
        assert!(is_drawn_key(VECTOR_KEY));
        assert!(!is_drawn_key(&VECTOR_KEY.to_uppercase()));
        assert!(!is_drawn_key(&VECTOR_KEY[1..]));
        assert!(!is_drawn_key("s3cr3t-engine-key"));
    }

    #[test]
    fn the_errors_name_the_folder_and_never_hold_a_key() {
        let key = draw_key().unwrap();
        let errors = [
            InstallationEngineError::DigestDirLocked {
                path: PathBuf::from("/run/arauto-engine/api-key.lock"),
            },
            InstallationEngineError::Random,
            InstallationEngineError::Task,
        ];
        for error in errors {
            let shown = format!("{error} {error:?}");
            assert!(!shown.contains(key.expose()));
        }
        let locked = InstallationEngineError::DigestDirLocked {
            path: PathBuf::from("/run/arauto-engine/api-key.lock"),
        };
        assert!(locked.to_string().contains("ARAUTO_ENGINE_DIGEST_DIR"));
        assert!(locked.to_string().contains("api-key.lock"));
    }

    #[test]
    fn settling_the_files_writes_repairs_and_removes_under_the_names_of_the_contract() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DIGEST_FILE);
        assert_eq!(DIGEST_FILE, "api-key.sha512");
        assert_eq!(DIGEST_LOCK_FILE, "api-key.lock");
        // Absent, then wanted: written, then left alone.
        assert_eq!(
            settle_files(dir.path(), Some(VECTOR_FILE)).unwrap(),
            FileOutcome::Written
        );
        assert_eq!(std::fs::read(&path).unwrap(), VECTOR_FILE.as_bytes());
        assert_eq!(
            settle_files(dir.path(), Some(VECTOR_FILE)).unwrap(),
            FileOutcome::Unchanged
        );
        // Damaged, too long and with a leftover: repaired, and the leftover goes.
        std::fs::write(&path, vec![b'x'; 5000]).unwrap();
        let leftover = dir.path().join(".api-key.sha512.0123456789abcdef.tmp");
        std::fs::write(&leftover, b"half").unwrap();
        assert_eq!(
            settle_files(dir.path(), Some(VECTOR_FILE)).unwrap(),
            FileOutcome::Written
        );
        assert_eq!(std::fs::read(&path).unwrap(), VECTOR_FILE.as_bytes());
        assert!(!leftover.exists());
        // Not wanted: removed, then absent.
        assert_eq!(
            settle_files(dir.path(), None).unwrap(),
            FileOutcome::Removed
        );
        assert!(!path.exists());
        assert_eq!(settle_files(dir.path(), None).unwrap(), FileOutcome::Absent);
    }
}
