//! The database (`docs/15-mvp.md`, section 2).
//!
//! - **The pool** has a fixed size, waits a bounded time for a connection, and sets on every
//!   connection the two timeouts that keep a stuck transaction from holding the others.
//! - **Durability** is checked at start: with `fsync` or `full_page_writes` off, a commit the
//!   hub already answered for can vanish, and that reads as a restored database.
//! - **Migrations** are embedded and only go forward. An empty database gets all of them; a
//!   database with data is only migrated by the command that takes the confirmation of the
//!   backup; a database newer than the program is refused.
//! - **The resolved organization** ([`ResolvedOrg`]) is what every repository function of
//!   business data takes. Only the entry functions of this module build one.
//! - **The durable transaction** ([`begin_durable`]) is the one of security events and of the
//!   credential swap.
//!
//! Time never comes from the database: no SQL here or in the migrations calls its clock.

use std::future::Future;
use std::time::Duration;

use arauto_protocol::invite::InviteDigest;
use arauto_protocol::noise::PublicKey;
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Executor, PgExecutor, Postgres, Transaction};
use uuid::Uuid;

use crate::config::DatabaseUrl;

/// The migrations, embedded in the program at build time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// The table where the applied migrations are recorded.
const MIGRATIONS_TABLE: &str = "_sqlx_migrations";

/// What every connection is told when it opens: a statement waits at most 5 seconds for a
/// lock, and a transaction left idle is ended by the server after 30.
const SESSION_SETTINGS: &str =
    "SET lock_timeout = '5s'; SET idle_in_transaction_session_timeout = '30s'";

/// What went wrong with the database.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The database could not be reached.
    #[error("could not connect to the database: {0}")]
    Connect(#[source] sqlx::Error),

    /// A statement failed.
    #[error("the database refused a statement: {0}")]
    Query(#[from] sqlx::Error),

    /// The database does not force its writes to disk.
    #[error(
        "the database does not force its writes to disk (fsync is {}, full_page_writes is {}): \
         a commit could be lost and the hub would take it for a restored database. Turn both \
         on in PostgreSQL; ARAUTO_ALLOW_UNSAFE_DB=1 exists for tests only",
        on_off(*.fsync),
        on_off(*.full_page_writes)
    )]
    UnsafeDurability {
        /// Whether `fsync` is on.
        fsync: bool,
        /// Whether `full_page_writes` is on.
        full_page_writes: bool,
    },

    /// The database is empty: nothing was installed in it yet.
    #[error("the database is empty: run `arauto-hub init`")]
    SchemaEmpty,

    /// The database has data and migrations of this program were not applied to it.
    #[error(
        "the database has {} pending migration(s), the first one {first}: take a backup of \
         the database and run `arauto-hub migrate --i-have-a-backup`",
        .pending.len()
    )]
    MigrationsPending {
        /// The first version that is missing.
        first: i64,
        /// Every version that is missing, in order.
        pending: Vec<i64>,
    },

    /// The database was migrated by a newer program.
    #[error(
        "the database is newer than this program (it has migration {newest}, unknown here): \
         run the arauto-hub version that migrated it"
    )]
    SchemaNewer {
        /// The newest version in the database that this program does not have.
        newest: i64,
    },

    /// A migration recorded in the database is not the one of this program.
    #[error(
        "migration {version} in the database is not the one of this program: the database \
         belongs to another build"
    )]
    MigrationChanged {
        /// The version whose content differs.
        version: i64,
    },

    /// A migration stopped in the middle.
    #[error(
        "migration {version} failed halfway in this database: restore the backup taken before \
         migrating"
    )]
    MigrationFailed {
        /// The version that did not finish.
        version: i64,
    },

    /// The database has tables and none of them says it is of Arauto.
    #[error(
        "the database has tables that are not of arauto: point ARAUTO_DATABASE_URL at an \
         empty database"
    )]
    SchemaForeign,

    /// Applying the migrations failed.
    #[error("could not apply the migrations: {0}")]
    Migrate(#[source] MigrateError),
}

const fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

// ---------------------------------------------------------------------------- the pool

/// The size and the patience of a pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolOptions {
    /// How many connections the pool has at most. It does not grow past this.
    pub size: u32,
    /// How long a caller waits for a free connection.
    pub acquire_timeout: Duration,
}

impl Default for PoolOptions {
    /// Sixteen connections and five seconds, the starting values of the hub.
    fn default() -> Self {
        Self {
            size: 16,
            acquire_timeout: Duration::from_secs(5),
        }
    }
}

/// Opens the pool. Every connection of it has `lock_timeout` at 5 seconds and
/// `idle_in_transaction_session_timeout` at 30.
///
/// The database is reached directly: a pooler in transaction mode would drop these settings.
///
/// # Errors
/// [`DbError::Connect`] when the address is wrong or the database does not answer. The
/// address is never part of the error.
pub async fn connect(url: &DatabaseUrl, options: PoolOptions) -> Result<PgPool, DbError> {
    PgPoolOptions::new()
        .max_connections(options.size)
        .acquire_timeout(options.acquire_timeout)
        .after_connect(|connection, _| {
            Box::pin(async move {
                connection.execute(SESSION_SETTINGS).await?;
                Ok(())
            })
        })
        .connect(url.expose())
        .await
        .map_err(DbError::Connect)
}

// ---------------------------------------------------------------------------- durability

/// The two settings of PostgreSQL without which a commit is not a promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Durability {
    /// `fsync`: writes are forced to disk.
    pub fsync: bool,
    /// `full_page_writes`: a page torn by a crash can be rebuilt.
    pub full_page_writes: bool,
}

/// Reads the two settings from the server.
///
/// # Errors
/// [`DbError::Query`] when the server does not answer.
pub async fn read_durability(pool: &PgPool) -> Result<Durability, DbError> {
    let (fsync, full_page_writes): (String, String) =
        sqlx::query_as("SELECT current_setting('fsync'), current_setting('full_page_writes')")
            .fetch_one(pool)
            .await?;
    Ok(Durability {
        fsync: fsync == "on",
        full_page_writes: full_page_writes == "on",
    })
}

/// The check of the start: refuses a database that does not force its writes to disk, unless
/// the explicit test option was given.
///
/// # Errors
/// [`DbError::UnsafeDurability`], or what reading the settings returned.
pub async fn check_durability(pool: &PgPool, allow_unsafe: bool) -> Result<(), DbError> {
    check_durability_with(read_durability(pool), allow_unsafe).await
}

/// [`check_durability`] with the reading apart, so that a test can stand in for a server
/// with the settings off.
///
/// # Errors
/// [`DbError::UnsafeDurability`], or what `read` returned.
pub async fn check_durability_with(
    read: impl Future<Output = Result<Durability, DbError>>,
    allow_unsafe: bool,
) -> Result<(), DbError> {
    let found = read.await?;
    if (found.fsync && found.full_page_writes) || allow_unsafe {
        return Ok(());
    }
    Err(DbError::UnsafeDurability {
        fsync: found.fsync,
        full_page_writes: found.full_page_writes,
    })
}

// ---------------------------------------------------------------------------- migrations

/// How the database stands against the migrations of this program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaState {
    /// Nothing in it: a new database.
    Empty,
    /// Every migration of this program is applied, and no other.
    Current,
    /// Migrations of this program are missing in a database that has others.
    Pending {
        /// The versions that are missing, in order. Never empty.
        pending: Vec<i64>,
    },
    /// The database has a migration this program does not know.
    Newer {
        /// The newest such version.
        newest: i64,
    },
    /// A recorded migration has another content than the one of this program.
    Changed {
        /// The first such version.
        version: i64,
    },
    /// A migration is recorded as not finished.
    Failed {
        /// The first such version.
        version: i64,
    },
    /// There are tables, and no record of migrations.
    Foreign,
}

/// Looks at the database without changing it, against `migrator` (the program passes
/// [`MIGRATOR`]).
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn schema_state(pool: &PgPool, migrator: &Migrator) -> Result<SchemaState, DbError> {
    let has_record: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(MIGRATIONS_TABLE)
        .fetch_one(pool)
        .await?;
    let other_objects: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_catalog.pg_class c \
         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = current_schema() \
           AND c.relkind IN ('r', 'p', 'v', 'm', 'S', 'f') \
           AND c.relname <> $1",
    )
    .bind(MIGRATIONS_TABLE)
    .fetch_one(pool)
    .await?;
    let applied: Vec<(i64, Vec<u8>, bool)> = if has_record {
        sqlx::query_as("SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await?
    } else {
        Vec::new()
    };
    let embedded: Vec<(i64, &[u8])> = migrator
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .map(|migration| (migration.version, migration.checksum.as_ref()))
        .collect();
    Ok(compare(&embedded, &applied, other_objects > 0))
}

/// The decision of [`schema_state`], apart from the reading.
fn compare(
    embedded: &[(i64, &[u8])],
    applied: &[(i64, Vec<u8>, bool)],
    has_other_objects: bool,
) -> SchemaState {
    if applied.is_empty() {
        return if has_other_objects {
            SchemaState::Foreign
        } else {
            SchemaState::Empty
        };
    }
    if let Some((version, _, _)) = applied.iter().find(|(_, _, success)| !success) {
        return SchemaState::Failed { version: *version };
    }
    let known = |version: i64| embedded.iter().find(|(known, _)| *known == version);
    if let Some(newest) = applied
        .iter()
        .map(|(version, _, _)| *version)
        .filter(|version| known(*version).is_none())
        .max()
    {
        return SchemaState::Newer { newest };
    }
    for (version, checksum, _) in applied {
        if known(*version).is_some_and(|(_, expected)| *expected != checksum.as_slice()) {
            return SchemaState::Changed { version: *version };
        }
    }
    let pending: Vec<i64> = embedded
        .iter()
        .map(|(version, _)| *version)
        .filter(|version| !applied.iter().any(|(applied, _, _)| applied == version))
        .collect();
    if pending.is_empty() {
        SchemaState::Current
    } else {
        SchemaState::Pending { pending }
    }
}

impl SchemaState {
    /// `Ok` for a database exactly at the version of this program; otherwise the refusal,
    /// which says what to do.
    ///
    /// # Errors
    /// One [`DbError`] for each other state.
    pub fn require_current(self) -> Result<(), DbError> {
        Err(match self {
            SchemaState::Current => return Ok(()),
            SchemaState::Empty => DbError::SchemaEmpty,
            SchemaState::Pending { pending } => DbError::MigrationsPending {
                first: pending.first().copied().unwrap_or_default(),
                pending,
            },
            SchemaState::Newer { newest } => DbError::SchemaNewer { newest },
            SchemaState::Changed { version } => DbError::MigrationChanged { version },
            SchemaState::Failed { version } => DbError::MigrationFailed { version },
            SchemaState::Foreign => DbError::SchemaForeign,
        })
    }
}

async fn apply_all(pool: &PgPool, migrator: &Migrator) -> Result<(), DbError> {
    migrator.run(pool).await.map_err(DbError::Migrate)
}

/// What `init` does: applies every migration of `migrator` to an **empty** database. A database that is
/// already current is left as it is.
///
/// # Errors
/// [`DbError::MigrationsPending`] when the database has data and is behind (that is for
/// [`migrate_existing_database`]), and the refusal of every other state.
pub async fn migrate_new_database(pool: &PgPool, migrator: &Migrator) -> Result<(), DbError> {
    match schema_state(pool, migrator).await? {
        SchemaState::Empty => apply_all(pool, migrator).await,
        other => other.require_current(),
    }
}

/// The word of whoever runs `migrate --i-have-a-backup`. Only that command builds one.
#[derive(Debug)]
pub struct BackupConfirmed(());

impl BackupConfirmed {
    /// The person said, on the command line, that a backup of the database was taken.
    #[must_use]
    pub const fn i_have_a_backup() -> Self {
        Self(())
    }
}

/// What `migrate --i-have-a-backup` does: applies the pending migrations to a database that
/// has data. Returns the versions it applied.
///
/// # Errors
/// [`DbError::SchemaNewer`] when the database is ahead of the program, and the refusal of a
/// changed, failed or foreign database. Nothing is applied in those cases.
pub async fn migrate_existing_database(
    pool: &PgPool,
    migrator: &Migrator,
    _confirmed: BackupConfirmed,
) -> Result<Vec<i64>, DbError> {
    match schema_state(pool, migrator).await? {
        SchemaState::Pending { pending } => {
            apply_all(pool, migrator).await?;
            Ok(pending)
        }
        SchemaState::Empty => {
            apply_all(pool, migrator).await?;
            Ok(migrator.iter().map(|migration| migration.version).collect())
        }
        other => other.require_current().map(|()| Vec::new()),
    }
}

/// What `serve` and `admin` do before anything else: refuse a database that is not exactly
/// at the version of this program.
///
/// # Errors
/// [`DbError::SchemaEmpty`], [`DbError::MigrationsPending`], [`DbError::SchemaNewer`] and
/// the refusal of a changed, failed or foreign database.
pub async fn require_current_schema(pool: &PgPool, migrator: &Migrator) -> Result<(), DbError> {
    schema_state(pool, migrator).await?.require_current()
}

// ---------------------------------------------------------------------------- the organization

/// An organization that was found by one of the entry functions of this module.
///
/// Every repository function of business data takes one, so a query cannot be written
/// without saying whose data it reads. There is no public constructor: the only ways in are
/// [`organization_of_device`], [`organization_of_invite`] and
/// [`organizations_for_system_task`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolvedOrg {
    id: Uuid,
}

impl ResolvedOrg {
    /// The identifier, to bind in a query.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.id
    }
}

/// The organization of the device with this identity, in any state.
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organization_of_device<'e>(
    executor: impl PgExecutor<'e>,
    identity: &PublicKey,
) -> Result<Option<ResolvedOrg>, DbError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT organization_id FROM device WHERE public_key = $1")
            .bind(identity.as_bytes().as_slice())
            .fetch_optional(executor)
            .await?;
    Ok(id.map(|id| ResolvedOrg { id }))
}

/// The organization of the invite with this digest, in any state.
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organization_of_invite<'e>(
    executor: impl PgExecutor<'e>,
    digest: &InviteDigest,
) -> Result<Option<ResolvedOrg>, DbError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT organization_id FROM invite WHERE digest = $1")
            .bind(digest.as_bytes().as_slice())
            .fetch_optional(executor)
            .await?;
    Ok(id.map(|id| ResolvedOrg { id }))
}

/// The work of the hub itself that goes through every organization. Each one is named, so a
/// search for the name shows who reads across organizations and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemTask {
    /// `init`: the organization that was just created.
    Install,
    /// The admin command, run by a person on the machine of the hub.
    LocalAdmin,
    /// The worker of the notice queue.
    Queues,
    /// The sweep of deadlines: invites, requests, inactivity.
    Deadlines,
    /// The purge of what has a retention time.
    Purge,
    /// The audit mark and the restored mode.
    Restore,
    /// A device nobody knows presented a seal: every organization key is tried on it.
    UnknownSeal,
    /// The start of `serve`: the channel connections a hub that stopped without warning left
    /// open are closed in every organization.
    Start,
}

/// Every organization, for a named task of the hub itself, oldest first.
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organizations_for_system_task<'e>(
    executor: impl PgExecutor<'e>,
    _task: SystemTask,
) -> Result<Vec<ResolvedOrg>, DbError> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM organization ORDER BY created_at, id")
        .fetch_all(executor)
        .await?;
    Ok(ids.into_iter().map(|id| ResolvedOrg { id }).collect())
}

// ---------------------------------------------------------------------------- the durable transaction

/// Begins the transaction of a security event or of a credential swap: its commit only
/// returns after the write is on disk, whatever the server default is
/// (`SET LOCAL synchronous_commit = on`).
///
/// # Errors
/// [`DbError::Query`] when no connection came in time or the server refused.
pub async fn begin_durable(pool: &PgPool) -> Result<Transaction<'static, Postgres>, DbError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("SET LOCAL synchronous_commit = on")
        .execute(&mut *transaction)
        .await?;
    Ok(transaction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ------------------------------------------------------------------ durability

    async fn settings(fsync: bool, full_page_writes: bool) -> Result<Durability, DbError> {
        Ok(Durability {
            fsync,
            full_page_writes,
        })
    }

    #[tokio::test]
    async fn a_database_that_does_not_force_writes_is_refused() {
        assert!(
            check_durability_with(settings(true, true), false)
                .await
                .is_ok()
        );
        for (fsync, full_page_writes) in [(false, true), (true, false), (false, false)] {
            let refused = check_durability_with(settings(fsync, full_page_writes), false).await;
            let Err(DbError::UnsafeDurability {
                fsync: said_fsync,
                full_page_writes: said_pages,
            }) = refused
            else {
                panic!("fsync {fsync}, full_page_writes {full_page_writes} was accepted");
            };
            assert_eq!((said_fsync, said_pages), (fsync, full_page_writes));
        }
    }

    #[tokio::test]
    async fn the_test_option_accepts_it() {
        for (fsync, full_page_writes) in [(false, true), (true, false), (false, false)] {
            assert!(
                check_durability_with(settings(fsync, full_page_writes), true)
                    .await
                    .is_ok()
            );
        }
    }

    #[tokio::test]
    async fn a_failed_reading_is_not_taken_for_a_safe_database() {
        let failed = async { Err(DbError::SchemaForeign) };
        assert!(check_durability_with(failed, false).await.is_err());
        let failed = async { Err(DbError::SchemaForeign) };
        assert!(check_durability_with(failed, true).await.is_err());
    }

    #[test]
    fn the_refusal_says_which_setting_and_what_to_do() {
        let text = DbError::UnsafeDurability {
            fsync: false,
            full_page_writes: true,
        }
        .to_string();
        assert!(text.contains("fsync is off"), "{text}");
        assert!(text.contains("full_page_writes is on"), "{text}");
        assert!(text.contains("ARAUTO_ALLOW_UNSAFE_DB"), "{text}");
    }

    // ------------------------------------------------------------------ the state of the schema

    fn applied(rows: &[(i64, &[u8], bool)]) -> Vec<(i64, Vec<u8>, bool)> {
        rows.iter()
            .map(|(version, checksum, success)| (*version, checksum.to_vec(), *success))
            .collect()
    }

    const PROGRAM: [(i64, &[u8]); 3] = [(1, b"one"), (2, b"two"), (3, b"three")];

    #[test]
    fn an_empty_database_gets_everything() {
        assert_eq!(compare(&PROGRAM, &[], false), SchemaState::Empty);
    }

    #[test]
    fn tables_without_a_record_are_not_ours() {
        assert_eq!(compare(&PROGRAM, &[], true), SchemaState::Foreign);
    }

    #[test]
    fn a_database_at_the_version_of_the_program_is_current() {
        let rows = applied(&[(1, b"one", true), (2, b"two", true), (3, b"three", true)]);
        assert_eq!(compare(&PROGRAM, &rows, true), SchemaState::Current);
    }

    #[test]
    fn a_database_with_data_and_a_pending_migration_is_pending() {
        let rows = applied(&[(1, b"one", true)]);
        assert_eq!(
            compare(&PROGRAM, &rows, true),
            SchemaState::Pending {
                pending: vec![2, 3]
            }
        );
        // A hole in the middle is pending too.
        let rows = applied(&[(1, b"one", true), (3, b"three", true)]);
        assert_eq!(
            compare(&PROGRAM, &rows, true),
            SchemaState::Pending { pending: vec![2] }
        );
    }

    #[test]
    fn a_database_newer_than_the_program_is_refused() {
        let rows = applied(&[
            (1, b"one", true),
            (2, b"two", true),
            (3, b"three", true),
            (4, b"four", true),
            (7, b"seven", true),
        ]);
        assert_eq!(
            compare(&PROGRAM, &rows, true),
            SchemaState::Newer { newest: 7 }
        );
        // Newer wins over pending: the program must not add its own on top.
        let rows = applied(&[(1, b"one", true), (9, b"nine", true)]);
        assert_eq!(
            compare(&PROGRAM, &rows, true),
            SchemaState::Newer { newest: 9 }
        );
    }

    #[test]
    fn a_changed_or_failed_migration_is_refused() {
        let rows = applied(&[(1, b"one", true), (2, b"TWO", true), (3, b"three", true)]);
        assert_eq!(
            compare(&PROGRAM, &rows, true),
            SchemaState::Changed { version: 2 }
        );
        let rows = applied(&[(1, b"one", true), (2, b"two", false)]);
        assert_eq!(
            compare(&PROGRAM, &rows, true),
            SchemaState::Failed { version: 2 }
        );
    }

    #[test]
    fn each_refusal_says_what_to_do() {
        let refusal = |state: SchemaState| state.require_current().unwrap_err();
        assert!(SchemaState::Current.require_current().is_ok());
        let pending = refusal(SchemaState::Pending {
            pending: vec![2, 3],
        })
        .to_string();
        assert!(pending.contains("migrate --i-have-a-backup"), "{pending}");
        assert!(pending.contains("backup"), "{pending}");
        assert!(pending.contains("2 pending"), "{pending}");
        let newer = refusal(SchemaState::Newer { newest: 9 }).to_string();
        assert!(newer.contains("newer than this program"), "{newer}");
        let empty = refusal(SchemaState::Empty).to_string();
        assert!(empty.contains("arauto-hub init"), "{empty}");
    }

    // ------------------------------------------------------------------ the migration files

    /// Why a migration breaks the rule "time is of the hub", if it does.
    fn clock_use(sql: &str) -> Option<String> {
        /// Functions and keywords that read the clock of the database.
        const CLOCK_WORDS: [&str; 11] = [
            "now",
            "current_timestamp",
            "current_time",
            "current_date",
            "localtimestamp",
            "localtime",
            "clock_timestamp",
            "statement_timestamp",
            "transaction_timestamp",
            "timeofday",
            "pg_postmaster_start_time",
        ];
        /// Column types that hold a time of the database instead of milliseconds of the hub.
        const TIME_TYPES: [&str; 6] = [
            "timestamp",
            "timestamptz",
            "date",
            "time",
            "timetz",
            "interval",
        ];
        for (number, line) in sql.lines().enumerate() {
            let code = line.split("--").next().unwrap_or(line).to_ascii_lowercase();
            let words: Vec<&str> = code
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .filter(|word| !word.is_empty())
                .collect();
            let line_number = number + 1;
            if let Some(word) = words.iter().find(|word| CLOCK_WORDS.contains(word)) {
                return Some(format!(
                    "line {line_number}: `{word}` reads the database clock"
                ));
            }
            if let Some(word) = words.iter().find(|word| TIME_TYPES.contains(word)) {
                return Some(format!("line {line_number}: `{word}` is a time type"));
            }
            let is_time_column = |name: &str| {
                name == "at"
                    || ["_at", "_until", "_since", "_before", "_time", "_on"]
                        .iter()
                        .any(|ending| name.ends_with(ending))
            };
            if words.contains(&"default")
                && let Some(column) = words.first()
                && is_time_column(column)
            {
                return Some(format!(
                    "line {line_number}: the time column `{column}` has a default"
                ));
            }
        }
        None
    }

    #[test]
    fn the_scan_catches_every_way_of_using_the_database_clock() {
        for bad in [
            "created_at BIGINT NOT NULL DEFAULT 0,",
            "    expires_at BIGINT DEFAULT 1700000000000,",
            "at BIGINT NOT NULL DEFAULT 0",
            "confirm_until BIGINT DEFAULT NULL",
            "seen TIMESTAMPTZ NOT NULL,",
            "seen timestamp with time zone,",
            "x BIGINT DEFAULT (extract(epoch from now()) * 1000),",
            "SELECT NOW ();",
            "UPDATE t SET at = CURRENT_TIMESTAMP;",
            "SELECT clock_timestamp();",
            "SELECT LOCALTIMESTAMP;",
            "born DATE,",
            "lasted INTERVAL,",
        ] {
            assert!(clock_use(bad).is_some(), "not caught: {bad}");
        }
        for good in [
            "created_at BIGINT NOT NULL,",
            "restored_mode BOOLEAN NOT NULL DEFAULT FALSE,",
            "-- time is BIGINT; never now() and no time defaults",
            "count INTEGER NOT NULL DEFAULT 0, -- updated at each message",
            "CREATE INDEX audit_org_at ON audit (organization_id, at);",
            "CREATE INDEX knowledge ON t (known, snow);",
        ] {
            assert_eq!(clock_use(good), None, "{good}");
        }
    }

    #[test]
    fn no_migration_uses_the_database_clock() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
            .collect();
        files.sort();
        assert!(!files.is_empty(), "no migration in {}", dir.display());
        for file in &files {
            let sql = std::fs::read_to_string(file).unwrap();
            if let Some(found) = clock_use(&sql) {
                panic!("{}: {found}", file.display());
            }
        }
        // Every file is embedded, and the embedded ones are what the scan read.
        assert_eq!(MIGRATOR.iter().count(), files.len());
        for migration in MIGRATOR.iter() {
            assert_eq!(clock_use(migration.sql.as_str()), None);
        }
    }

    #[test]
    fn the_migrations_only_go_forward_and_are_numbered_in_order() {
        let versions: Vec<i64> = MIGRATOR.iter().map(|migration| migration.version).collect();
        assert_eq!(versions.first(), Some(&1));
        assert!(versions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            MIGRATOR
                .iter()
                .all(|migration| !migration.migration_type.is_down_migration())
        );
    }

    #[test]
    fn the_pool_starts_at_sixteen_connections_and_five_seconds() {
        let options = PoolOptions::default();
        assert_eq!(options.size, 16);
        assert_eq!(options.acquire_timeout, Duration::from_secs(5));
        assert!(SESSION_SETTINGS.contains("lock_timeout = '5s'"));
        assert!(SESSION_SETTINGS.contains("idle_in_transaction_session_timeout = '30s'"));
    }
}
