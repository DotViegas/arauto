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
//! - **The password of the role** the hub connects as can be replaced by one the hub draws
//!   and keeps in the data folder ([`rotate_password`], [`crate::db_password`]); the program
//!   opens every pool with [`open`], which uses it.
//!
//! Time never comes from the database: no SQL here or in the migrations calls its clock.

use std::future::Future;
use std::str::FromStr;
use std::time::Duration;

use arauto_platform::LockError;
use arauto_protocol::invite::InviteDigest;
use arauto_protocol::noise::PublicKey;
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::{AssertSqlSafe, ConnectOptions, Executor, PgExecutor, Postgres, Transaction};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::audit::{self, Action, Actor, AuditError, Details, Event};
use crate::config::{DatabaseUrl, PasswordRotation, StorageConfig};
use crate::db_password::{self, DatabasePasswordError, DatabaseTarget, FirstAttempt, NextStep};
use crate::lock_wait::acquire_within;
use crate::mark::Mark;
use crate::panel::secret::PanelDigest;

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

    /// The password of the database role kept in the data folder could not be read or
    /// settled.
    #[error(transparent)]
    Password(#[from] DatabasePasswordError),
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

/// Opens the pool on an address as given. The program opens its pools with [`open`], which
/// uses the password kept in the data folder; this one is for the tests and the tools that
/// hold an address of their own. Every connection of the pool has `lock_timeout` at 5
/// seconds and `idle_in_transaction_session_timeout` at 30.
///
/// The database is reached directly: a pooler in transaction mode would drop these settings.
///
/// # Errors
/// [`DbError::Connect`] when the address is wrong or the database does not answer. The
/// address is never part of the error.
pub async fn connect(url: &DatabaseUrl, options: PoolOptions) -> Result<PgPool, DbError> {
    let target = PgConnectOptions::from_str(url.expose()).map_err(DbError::Connect)?;
    connect_to(&target, options).await
}

/// The pool of `options`, with the settings of every connection.
fn pool_options(options: PoolOptions) -> PgPoolOptions {
    PgPoolOptions::new()
        .max_connections(options.size)
        .acquire_timeout(options.acquire_timeout)
        .after_connect(|connection, _| {
            Box::pin(async move {
                connection.execute(SESSION_SETTINGS).await?;
                Ok(())
            })
        })
}

async fn connect_to(target: &PgConnectOptions, options: PoolOptions) -> Result<PgPool, DbError> {
    pool_options(options)
        .connect_with(target.clone())
        .await
        .map_err(DbError::Connect)
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, DatabasePasswordError> + Send + 'static,
) -> Result<T, DatabasePasswordError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| DatabasePasswordError::Task)?
}

/// Opens the pool of the hub: every place of the program that opens the database comes here.
///
/// The address is the one of `ARAUTO_DATABASE_URL`, with the password of `database.secret`
/// when the data folder has that file ([`StorageConfig::database_target`]). When the file
/// `database.secret.next` is there too, a replacement of the password stopped in the middle
/// ([`rotate_password`]), and the database may or may not have the new password:
///
/// - the password in use works: the database never had the new one, and the file of the new
///   one goes away;
/// - PostgreSQL refuses the password in use (and only then: a network error says nothing about
///   the password): the new one is tried, and when it works it is put under
///   `database.secret`, which finishes the replacement.
///
/// The decision is [`db_password::next_step`].
///
/// # Errors
/// [`DbError::Connect`] when the database does not take the connection, with the error of the
/// password in use; [`DbError::Password`] when a file of the password is damaged or cannot be
/// settled. Neither the address nor a password is ever part of the error.
pub async fn open(storage: &StorageConfig, options: PoolOptions) -> Result<PgPool, DbError> {
    let dir = storage.data_dir.clone();
    let (current, next) = blocking(move || {
        Ok((
            db_password::read_current(&dir)?,
            db_password::read_next(&dir)?,
        ))
    })
    .await?;
    let normal = db_password::target(&storage.database_url, current.as_ref())?;
    let first = connect_to(normal.options(), options).await;
    let attempt = match &first {
        Ok(_) => FirstAttempt::Connected,
        Err(DbError::Connect(error)) => db_password::classify(error),
        Err(_) => FirstAttempt::Failed,
    };
    let dir = storage.data_dir.clone();
    match (db_password::next_step(attempt, next.is_some()), next) {
        (NextStep::Keep { discard_next: true }, _) => {
            // The pool is good: a leftover that stays is tried again at the next opening.
            if let Err(error) = blocking(move || db_password::discard_next(&dir)).await {
                tracing::warn!(%error, "the file of a database password never used stays");
            }
            first
        }
        (NextStep::TryNext, Some(next)) => {
            let target = normal.with_password(&next);
            match connect_to(target.options(), options).await {
                Ok(pool) => {
                    blocking(move || db_password::promote(&dir, &next)).await?;
                    tracing::info!(
                        "a replacement of the database password stopped halfway was finished"
                    );
                    Ok(pool)
                }
                Err(_) => first,
            }
        }
        _ => first,
    }
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
/// [`organization_of_device`], [`organization_of_invite`], [`organization_of_engine_events`],
/// the three of the panel ([`organization_of_panel_entry`], [`organization_of_panel_session`]
/// and [`organization_of_panel_passkey`]) and [`organizations_for_system_task`].
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

/// The organization of the entry code of the panel with this digest, while it is issued.
/// Whether it is still in its minutes is judged by the repository, under the lock.
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organization_of_panel_entry<'e>(
    executor: impl PgExecutor<'e>,
    digest: &PanelDigest,
) -> Result<Option<ResolvedOrg>, DbError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT organization_id FROM panel_entry WHERE digest = $1")
            .bind(digest.as_bytes().as_slice())
            .fetch_optional(executor)
            .await?;
    Ok(id.map(|id| ResolvedOrg { id }))
}

/// The organization of the session of the panel with this digest, while it is open. Whether
/// it is still in its hours is judged by whoever reads the session.
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organization_of_panel_session<'e>(
    executor: impl PgExecutor<'e>,
    digest: &PanelDigest,
) -> Result<Option<ResolvedOrg>, DbError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT organization_id FROM panel_session WHERE digest = $1")
            .bind(digest.as_bytes().as_slice())
            .fetch_optional(executor)
            .await?;
    Ok(id.map(|id| ResolvedOrg { id }))
}

/// The organization of the passkey with this credential, while the passkey is active.
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organization_of_panel_passkey<'e>(
    executor: impl PgExecutor<'e>,
    credential_id: &[u8],
) -> Result<Option<ResolvedOrg>, DbError> {
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT organization_id FROM panel_passkey WHERE credential_id = $1 AND state = 'active'",
    )
    .bind(credential_id)
    .fetch_optional(executor)
    .await?;
    Ok(id.map(|id| ResolvedOrg { id }))
}

/// An event of an engine that is not yet proven: the organization comes out of it only when
/// the signature of the body checks with the secret of that engine
/// ([`EngineEventsGate::verify`]).
pub struct EngineEventsGate {
    organization: Uuid,
    engine: Uuid,
    secret_enc: Vec<u8>,
}

impl std::fmt::Debug for EngineEventsGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EngineEventsGate([redacted])")
    }
}

impl EngineEventsGate {
    /// The organization of the engine, when `signature` is the signature of `body` with the
    /// secret of the events of that engine, compared in constant time. `None` for anything
    /// else, a secret that does not open included.
    #[must_use]
    pub fn verify(
        self,
        vault: &crate::vault::Vault,
        body: &[u8],
        signature: &str,
    ) -> Option<(ResolvedOrg, Uuid)> {
        let secret = vault
            .open_engine_secret(
                self.engine,
                crate::vault::EngineSecretKind::EventsSecret,
                &self.secret_enc,
            )
            .ok()?;
        let secret = crate::engine::EventsSecret::from_stored(&secret)?;
        secret.verifies(body, signature).then_some((
            ResolvedOrg {
                id: self.organization,
            },
            self.engine,
        ))
    }
}

/// The engine `engine`, for an event that says it comes from it, with the sealed secret its
/// signature is checked with. Nothing of the organization is usable before
/// [`EngineEventsGate::verify`].
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organization_of_engine_events<'e>(
    executor: impl PgExecutor<'e>,
    engine: Uuid,
) -> Result<Option<EngineEventsGate>, DbError> {
    let row: Option<(Uuid, Vec<u8>)> =
        sqlx::query_as("SELECT organization_id, events_secret_enc FROM engine WHERE id = $1")
            .bind(engine)
            .fetch_optional(executor)
            .await?;
    Ok(row.map(|(organization, secret_enc)| EngineEventsGate {
        organization,
        engine,
        secret_enc,
    }))
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
    /// The supervision of the engine, at the start and every 5 minutes: its version, the
    /// configuration of each session, the pairings that expired and the catch-up.
    Engine,
}

/// Every organization, for a named task of the hub itself, oldest first. The supervision of
/// the engine only goes through the organizations that have one: a hub without an engine
/// spends one query on it.
///
/// # Errors
/// [`DbError::Query`] when the database does not answer.
pub async fn organizations_for_system_task<'e>(
    executor: impl PgExecutor<'e>,
    task: SystemTask,
) -> Result<Vec<ResolvedOrg>, DbError> {
    let query = match task {
        SystemTask::Engine => {
            "SELECT o.id FROM organization o \
             WHERE EXISTS (SELECT 1 FROM engine e WHERE e.organization_id = o.id) \
             ORDER BY o.created_at, o.id"
        }
        _ => "SELECT id FROM organization ORDER BY created_at, id",
    };
    let ids: Vec<Uuid> = sqlx::query_scalar(query).fetch_all(executor).await?;
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

// ---------------------------------------------------------------------------- the password of the role

/// How long a replacement of the password waits for another one that is running.
const PASSWORD_LOCK_PATIENCE: Duration = Duration::from_secs(5);

/// When [`rotate_password`] is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// A person asked for it: `arauto-hub admin database rotate-password`. It always replaces.
    Asked,
    /// The first passkey of the installation was just registered. It replaces only when
    /// `ARAUTO_DATABASE_PASSWORD` asks for it and the data folder keeps no password yet.
    FirstAccess,
}

impl When {
    const fn word(self) -> &'static str {
        match self {
            When::Asked => "asked",
            When::FirstAccess => "first_access",
        }
    }
}

/// What [`rotate_password`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotated {
    /// The password was replaced and is kept in the data folder.
    Replaced,
    /// [`When::FirstAccess`], and the configuration does not ask for it. Nothing was done.
    NotAsked,
    /// [`When::FirstAccess`], and the data folder already keeps a password: the hub never
    /// replaces it twice by itself. Nothing was done.
    AlreadyReplaced,
}

/// What went wrong while replacing the password of the role. No variant carries a password.
#[derive(Debug, thiserror::Error)]
pub enum RotationError {
    /// Another replacement is running right now. Nothing was done.
    #[error("the database password is being replaced by another command: {0}")]
    Busy(LockError),

    /// A file of the password could not be read or written. When this comes before the
    /// database was told, nothing was done.
    #[error(transparent)]
    Password(#[from] DatabasePasswordError),

    /// The database refused. Nothing was done.
    #[error(transparent)]
    Db(#[from] DbError),

    /// The security write refused (nothing was done), or it is committed and the audit mark
    /// did not follow ([`AuditError::MarkNotAdvanced`]): then the password **is** replaced,
    /// kept in the data folder and in use by this process.
    #[error(transparent)]
    Audit(#[from] AuditError),

    /// The database has the new password and this process uses it, and it could not be put
    /// under `database.secret`: it waits in `database.secret.next`, which the next opening of
    /// the database puts in place.
    #[error(
        "the database password was replaced and is in use, and could not be kept under its \
         name ({0}); it waits in database.secret.next, which the next start of the hub puts \
         in place"
    )]
    NotKept(DatabasePasswordError),

    /// The commit of the replacement failed on the way, and the database may have either
    /// password. The new one waits in `database.secret.next`: the next opening of the
    /// database finds out which one works and settles the files.
    #[error(
        "the database did not confirm the replacement of its password ({0}); restart the hub: \
         it finds out which password the database has"
    )]
    Unknown(AuditError),

    /// A replacement stopped halfway and was not settled yet: `database.secret.next` is
    /// there, and the database may have its password. Nothing was done.
    #[error(
        "a replacement of the database password stopped halfway and database.secret.next is \
         still there: restart the hub, which settles it, and then try again"
    )]
    Unsettled,
}

/// Replaces the password of the database role of the hub with one it draws, which nobody ever
/// sees, and keeps it in the data folder (`docs/04-seguranca-conexao.md`: a long-lived secret
/// never passes through a person).
///
/// The steps, in an order that leaves a hub that can still connect wherever it stops:
///
/// 1. the new password is drawn and written to `database.secret.next`, durably;
/// 2. `ALTER ROLE ... PASSWORD`, on the role the hub connects as, in a security write with
///    its audit event (`database.password_replaced`): both or neither;
/// 3. the file goes under `database.secret`, durably;
/// 4. every pool of `pools` opens its next connections with the new password, so the process
///    that replaced it never needs a restart. The connections already open stay as they are.
///
/// The statement that carries the password runs on a connection of its own, whose statements
/// are never logged. A process that did not replace the password (a hub that runs while the
/// command does) keeps its open connections, and its new ones are refused until it restarts.
///
/// # Errors
/// [`RotationError`]: nothing was done, except with [`RotationError::NotKept`] and
/// [`AuditError::MarkNotAdvanced`], which say the password is replaced.
pub async fn rotate_password(
    storage: &StorageConfig,
    pools: &[&PgPool],
    mark: &Mark,
    actor: Actor,
    when: When,
    now: i64,
) -> Result<Rotated, RotationError> {
    if when == When::FirstAccess && storage.password_rotation != PasswordRotation::AtFirstAccess {
        return Ok(Rotated::NotAsked);
    }
    let dir = storage.data_dir.clone();
    let _replacing = tokio::task::spawn_blocking(move || {
        acquire_within(
            &dir.join(db_password::DATABASE_SECRET_LOCK_FILE),
            PASSWORD_LOCK_PATIENCE,
        )
    })
    .await
    .map_err(|_| DatabasePasswordError::Task)?
    .map_err(RotationError::Busy)?;

    // Read under the lock: the password in use is the one of the file, whoever wrote it.
    let dir = storage.data_dir.clone();
    let (current, unsettled) = blocking(move || {
        Ok((
            db_password::read_current(&dir)?,
            dir.join(db_password::DATABASE_SECRET_NEXT_FILE).exists(),
        ))
    })
    .await?;
    if when == When::FirstAccess && current.is_some() {
        return Ok(Rotated::AlreadyReplaced);
    }
    // The database may have the password of that file: it is never written over.
    if unsettled {
        return Err(RotationError::Unsettled);
    }
    let old = db_password::target(&storage.database_url, current.as_ref())?;
    let new = db_password::DatabasePassword::generate()?;

    let (dir, next) = (storage.data_dir.clone(), new.clone());
    blocking(move || db_password::write_next(&dir, &next)).await?;

    let committed = match alter_role(&old, &new, mark, actor, when, now).await {
        // The database has the new password.
        Altered::Committed(committed) => committed,
        Altered::NotCommitted(error) => {
            // The database kept the password in use: the new one goes away.
            let dir = storage.data_dir.clone();
            if let Err(leftover) = blocking(move || db_password::discard_next(&dir)).await {
                tracing::warn!(error = %leftover, "the file of a database password never used stays");
            }
            return Err(RotationError::Audit(error));
        }
        // The new password stays in its file: the next opening finds out which one the
        // database has.
        Altered::Unknown(error) => return Err(RotationError::Unknown(error)),
    };

    // The database has the new password from here on.
    let (dir, kept) = (storage.data_dir.clone(), new.clone());
    let promoted = blocking(move || db_password::promote(&dir, &kept)).await;
    let replaced = old.with_password(&new);
    for pool in pools {
        pool.set_connect_options(replaced.options().clone());
    }
    match (promoted, committed) {
        (Err(error), _) => Err(RotationError::NotKept(error)),
        (Ok(()), Err(error)) => Err(RotationError::Audit(error)),
        (Ok(()), Ok(())) => Ok(Rotated::Replaced),
    }
}

/// How step 2 of [`rotate_password`] ended.
enum Altered {
    /// The database has the new password. An error here is
    /// [`AuditError::MarkNotAdvanced`].
    Committed(Result<(), AuditError>),
    /// The database kept the password in use.
    NotCommitted(AuditError),
    /// The commit failed on the way: the database may have either password.
    Unknown(AuditError),
}

/// Step 2 of [`rotate_password`]: the statement and its event, in one security write, on a
/// connection of its own that logs no statement and is closed at the end.
async fn alter_role(
    old: &DatabaseTarget,
    new: &db_password::DatabasePassword,
    mark: &Mark,
    actor: Actor,
    when: When,
    now: i64,
) -> Altered {
    let quiet = match pool_options(PoolOptions {
        size: 1,
        acquire_timeout: PASSWORD_LOCK_PATIENCE,
    })
    .connect_with(old.options().clone().disable_statement_logging())
    .await
    {
        Ok(quiet) => quiet,
        Err(error) => return Altered::NotCommitted(DbError::Connect(error).into()),
    };
    let written = async {
        let mut write = audit::begin_security(&quiet, mark).await?;
        let role: String = sqlx::query_scalar("SELECT current_user::text")
            .fetch_one(write.connection())
            .await?;
        // The role is quoted as an identifier; the password is hexadecimal by construction,
        // so it never closes its quotes.
        let statement = Zeroizing::new(format!(
            "ALTER ROLE \"{}\" PASSWORD '{}'",
            role.replace('"', "\"\""),
            new.expose()
        ));
        sqlx::raw_sql(AssertSqlSafe(statement.as_str()))
            .execute(write.connection())
            .await?;
        let event = Event::new(Action::DatabasePasswordReplaced, actor)
            .with(Details::new().word("when", when.word()));
        write.record(now, &event).await?;
        Ok::<_, AuditError>(write)
    }
    .await;
    let altered = match written {
        Err(error) => Altered::NotCommitted(error),
        Ok(write) => match write.commit().await {
            Ok(_) => Altered::Committed(Ok(())),
            Err(error @ AuditError::MarkNotAdvanced { .. }) => Altered::Committed(Err(error)),
            Err(error) => Altered::Unknown(error),
        },
    };
    quiet.close().await;
    altered
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
