//! Support for the tests that need a database of the hub, a data folder, or both.
//!
//! How the tests use the database (`docs/15-mvp.md`, section 5):
//!
//! - The address comes from `ARAUTO_TEST_DATABASE_URL`. Without it a test **fails**, saying
//!   what is missing. The user of that address must be allowed to create databases.
//! - The migrations run once, on a **template** database, created the first time a test of
//!   this process needs it.
//! - A test that touches the state of the whole installation (the installation row, the audit
//!   mark, the restored mode) gets a database of its own, copied from the template:
//!   [`TestDb::own`]. It is **not** dropped when the test ends: every `DROP DATABASE` forces
//!   a checkpoint of the whole server, and dozens of them in the middle of a run held the
//!   other tests past the 5 seconds a statement waits for a lock. The next run drops them.
//! - Every other test shares one database and keeps to an organization of its own:
//!   [`TestDb::shared`] and [`organization`].
//! - Every test has its own pool, of 3 connections, and its own temporary folder for the
//!   master key and the mark: [`TestHome`].
//! - The databases of the runs that ended are dropped when the first test starts, before any
//!   other test touches the server. Their names begin with `arauto_t_` (or with `arauto_x_`,
//!   for the one test that needs a database whose name does not say "test").
//!
//! A test binary of the hub names this module `common`:
//! `use arauto_testkit::hub::db as common;`.
// Test support: a failure here is the failure of the test that called it, with the reason.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arauto_hub::config::DatabaseUrl;
use arauto_hub::db::{self, PoolOptions, ResolvedOrg, SystemTask};
use arauto_hub::mark::Mark;
use arauto_hub::vault::Vault;
use sqlx::postgres::{PgConnection, PgPool};
use sqlx::{AssertSqlSafe, Connection, Executor};
use tokio::sync::OnceCell;
use uuid::Uuid;

/// The variable with the address of the test database server.
pub const DATABASE_VARIABLE: &str = "ARAUTO_TEST_DATABASE_URL";

/// What the name of every test database begins with.
const PREFIX: &str = "arauto_t_";

/// What the name of a test database begins with when the test needs one whose name does
/// **not** say "test": the one `dev-reset` must refuse to erase. The rest of the name is the
/// same, so the cleaning of a later run finds it too.
const PLAIN_PREFIX: &str = "arauto_x_";

/// A time for the rows of a test: any fixed moment will do.
pub const NOW: i64 = 1_800_000_000_000;

/// The address of the server, as given: the database in it is only used to create others.
fn server_url() -> String {
    match std::env::var(DATABASE_VARIABLE) {
        Ok(url) if !url.trim().is_empty() => url,
        _ => panic!(
            "{DATABASE_VARIABLE} is not set. The tests of the hub need a PostgreSQL where they \
             may create databases: set it to the address of one, like \
             postgres://USER@127.0.0.1:PORT/postgres"
        ),
    }
}

/// The same address, pointing at `database`.
fn url_of(database: &str) -> String {
    let url = server_url();
    let (scheme, rest) = url
        .split_once("://")
        .unwrap_or_else(|| panic!("{DATABASE_VARIABLE} is not an address"));
    let (location, query) = match rest.split_once('?') {
        Some((location, query)) => (location, format!("?{query}")),
        None => (rest, String::new()),
    };
    let authority = location.split('/').next().unwrap_or(location);
    format!("{scheme}://{authority}/{database}{query}")
}

async fn admin() -> PgConnection {
    PgConnection::connect(&server_url())
        .await
        .unwrap_or_else(|error| {
            panic!("the server of {DATABASE_VARIABLE} does not answer: {error}")
        })
}

/// This run of the tests: when it started, and the process.
struct Run {
    started_s: u64,
    process: u32,
}

impl Run {
    fn name(&self, what: &str) -> String {
        format!("{PREFIX}{}_{}_{what}", self.started_s, self.process)
    }

    /// The advisory lock a run holds for as long as its process lives.
    fn lock_key(started_s: u64, process: u32) -> i64 {
        ((started_s as i64) << 24) | i64::from(process & 0x00ff_ffff)
    }
}

/// What the tests of this process share: the template and the shared database.
struct Shared {
    template: String,
    shared: String,
}

static SHARED: OnceCell<Shared> = OnceCell::const_new();
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Created by the first test that needs a database; the others wait for it.
async fn shared() -> &'static Shared {
    SHARED
        .get_or_init(|| async {
            let run = Run {
                started_s: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("the clock is after 1970")
                    .as_secs(),
                process: std::process::id(),
            };
            // A connection that stays open, holding the lock of this run, until the process
            // ends: that is how the next run knows whether these databases are still in use.
            let mut guard = admin().await;
            sqlx::query("SELECT pg_advisory_lock($1)")
                .bind(Run::lock_key(run.started_s, run.process))
                .execute(&mut guard)
                .await
                .expect("the lock of this run is free");
            drop_databases_of_dead_runs(&mut guard).await;

            let template = run.name("template");
            create_database(&mut guard, &template, None).await;
            let pool = pool_of(&template).await;
            db::migrate_new_database(&pool, &db::MIGRATOR)
                .await
                .expect("the migrations apply to an empty database");
            pool.close().await;
            let shared = run.name("shared");
            create_database(&mut guard, &shared, Some(&template)).await;

            std::mem::forget(guard);
            Shared { template, shared }
        })
        .await
}

/// How many connections, at most, drop the databases of the runs that ended.
const DROPPERS: usize = 48;

/// Drops the test databases whose run no longer holds its lock.
///
/// A run leaves every database of its own behind. The first drop waits for a checkpoint that
/// forces to disk every file of every database still there, the ones about to be dropped
/// among them: half a minute of it, measured. So they all go at the same moment, each on a
/// connection of its own: a database that is dropped takes its files out of that checkpoint.
/// This runs before any test of this process has started.
async fn drop_databases_of_dead_runs(admin: &mut PgConnection) {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT datname FROM pg_database \
         WHERE datname LIKE 'arauto\\_t\\_%' OR datname LIKE 'arauto\\_x\\_%'",
    )
    .fetch_all(&mut *admin)
    .await
    .expect("the databases can be listed");
    // The lock of each dead run is taken once and held until its databases are gone.
    let mut held: Vec<i64> = Vec::new();
    let mut alive: Vec<i64> = Vec::new();
    let mut dead_databases = Vec::new();
    for name in names {
        let mut parts = name
            .trim_start_matches(PREFIX)
            .trim_start_matches(PLAIN_PREFIX)
            .split('_');
        let (Some(Ok(started_s)), Some(Ok(process))) = (
            parts.next().map(str::parse::<u64>),
            parts.next().map(str::parse::<u32>),
        ) else {
            continue;
        };
        let key = Run::lock_key(started_s, process);
        if alive.contains(&key) {
            continue;
        }
        if !held.contains(&key) {
            let dead: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
                .bind(key)
                .fetch_one(&mut *admin)
                .await
                .expect("the lock of a run can be tried");
            if !dead {
                alive.push(key);
                continue;
            }
            held.push(key);
        }
        dead_databases.push(name);
    }

    let wanted = DROPPERS.min(dead_databases.len());
    let queue = Arc::new(Mutex::new(dead_databases));
    let mut droppers = Vec::new();
    for _ in 0..wanted {
        let queue = Arc::clone(&queue);
        droppers.push(tokio::spawn(async move {
            let Ok(mut connection) = PgConnection::connect(&server_url()).await else {
                return;
            };
            loop {
                let next = queue.lock().expect("the queue of databases").pop();
                let Some(name) = next else {
                    break;
                };
                // Another run that started at the same moment may be dropping it too.
                let _ = connection
                    .execute(AssertSqlSafe(format!(
                        "DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"
                    )))
                    .await;
            }
        }));
    }
    for dropper in droppers {
        let _ = dropper.await;
    }
    for key in held {
        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(key)
            .execute(&mut *admin)
            .await
            .expect("the lock of a dead run can be released");
    }
}

async fn create_database(admin: &mut PgConnection, name: &str, template: Option<&str>) {
    let sql = match template {
        Some(template) => format!("CREATE DATABASE \"{name}\" TEMPLATE \"{template}\""),
        None => format!("CREATE DATABASE \"{name}\""),
    };
    // Two copies of the same template at the same moment can refuse each other.
    let mut last = None;
    for _ in 0..40 {
        match admin.execute(AssertSqlSafe(sql.clone())).await {
            Ok(_) => return,
            Err(error) => {
                last = Some(error);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
    panic!("could not create the test database {name}: {last:?}");
}

async fn pool_of(database: &str) -> PgPool {
    let options = PoolOptions {
        size: 3,
        ..PoolOptions::default()
    };
    // A database just created can take seconds to take its first connection while the
    // server forces the files of other new databases to disk: try again, as the creation
    // does.
    let url = DatabaseUrl::new(url_of(database));
    let mut last = None;
    for _ in 0..6 {
        match db::connect(&url, options).await {
            Ok(pool) => return pool,
            Err(error) => {
                last = Some(error);
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    }
    panic!("could not open the test database {database}: {last:?}");
}

/// A database for one test.
pub struct TestDb {
    /// The pool of this test: 3 connections.
    pub pool: PgPool,
    name: String,
}

impl TestDb {
    /// The database every test of this process shares, already migrated. Keep to an
    /// organization of your own ([`organization`]) and never touch what is of the whole
    /// installation.
    pub async fn shared() -> Self {
        let name = shared().await.shared.clone();
        Self {
            pool: pool_of(&name).await,
            name,
        }
    }

    /// A database only for this test, already migrated. The next run of the tests drops it.
    pub async fn own() -> Self {
        let template = shared().await.template.clone();
        Self::create(Some(&template), PREFIX).await
    }

    /// A database only for this test with nothing in it, not even the migrations.
    pub async fn empty() -> Self {
        shared().await;
        Self::create(None, PREFIX).await
    }

    /// As [`TestDb::empty`], with a name that does not begin with `arauto_t_`: a database
    /// that does not look like one of the tests.
    pub async fn empty_with_a_plain_name() -> Self {
        shared().await;
        Self::create(None, PLAIN_PREFIX).await
    }

    /// The name of this database.
    pub fn name(&self) -> &str {
        &self.name
    }

    async fn create(template: Option<&str>, prefix: &str) -> Self {
        let number = COUNTER.fetch_add(1, Ordering::SeqCst);
        // The same run and process as the template, so that the cleaning of a later run
        // finds it.
        let name = shared()
            .await
            .template
            .replace("template", &format!("own_{number}"))
            .replacen(PREFIX, prefix, 1);
        let mut admin = admin().await;
        create_database(&mut admin, &name, template).await;
        Self {
            pool: pool_of(&name).await,
            name,
        }
    }

    /// The address of this database, for a test that opens its own connections.
    pub fn url(&self) -> DatabaseUrl {
        DatabaseUrl::new(url_of(&self.name))
    }

    /// Another pool on the same database, as another process would have.
    pub async fn second_pool(&self) -> PgPool {
        pool_of(&self.name).await
    }
}

/// The data folder of one test: a master key and the place of the mark.
pub struct TestHome {
    dir: tempfile::TempDir,
    /// The vault of this folder.
    pub vault: Vault,
    /// The mark of this folder. Absent until a test advances it.
    pub mark: Mark,
}

impl TestHome {
    /// A new folder with a new master key and no mark.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let data = dir.path().join("data");
        let vault = Vault::create(&data).expect("a master key");
        let mark = Mark::new(&data);
        Self { dir, vault, mark }
    }

    /// The data folder.
    pub fn data_dir(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    /// Another handle on the same mark, with its own memory, as another process would have.
    pub fn second_mark(&self) -> Mark {
        Mark::new(&self.data_dir())
    }
}

impl Default for TestHome {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------- rows

static NAMES: AtomicU64 = AtomicU64::new(0);

/// Creates an organization, with its key encrypted by the vault of `home`, and resolves it
/// the way the program does.
pub async fn organization(pool: &PgPool, home: &TestHome) -> ResolvedOrg {
    let id = Uuid::new_v4();
    let key_enc = home
        .vault
        .generate_organization_key(id)
        .expect("an organization key");
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, 'test', 'active', $2, $3)",
    )
    .bind(id)
    .bind(key_enc)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("the organization is created");
    db::organizations_for_system_task(pool, SystemTask::Install)
        .await
        .expect("the organizations are listed")
        .into_iter()
        .find(|organization| organization.id() == id)
        .expect("the new organization is listed")
}

/// Creates the installation row, as `init` does.
pub async fn install(pool: &PgPool) {
    sqlx::query("INSERT INTO installation (created_at) VALUES ($1)")
        .bind(NOW)
        .execute(pool)
        .await
        .expect("the installation is created");
}

/// Creates a Hermes line with a name of its own.
pub async fn hermes(pool: &PgPool, organization: ResolvedOrg) -> Uuid {
    let id = Uuid::new_v4();
    let name = format!("h{}", NAMES.fetch_add(1, Ordering::SeqCst));
    sqlx::query(
        "INSERT INTO hermes (id, organization_id, name, created_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(name)
    .bind(NOW)
    .execute(pool)
    .await
    .expect("the Hermes is created");
    id
}

/// Whether a device already confirmed a credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmed {
    /// A request in progress, or one that ended before the first credential.
    Never,
    /// A device in use, at generation 3, or one that ended after being in use.
    Yes,
}

/// Creates a device of `hermes` in `state`, with the columns that state asks for, and
/// returns its row and its identity.
pub async fn device(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
    state: &str,
    confirmed: Confirmed,
) -> (Uuid, [u8; 32]) {
    let id = Uuid::new_v4();
    let mut identity = [0u8; 32];
    identity[..16].copy_from_slice(id.as_bytes());
    identity[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    let is_final = matches!(state, "denied" | "expired" | "replaced" | "revoked");
    let slot = match (is_final, confirmed) {
        (true, _) => None,
        (false, Confirmed::Never) => Some("request"),
        (false, Confirmed::Yes) => Some("in_use"),
    };
    let (first_confirmed_at, generation, digest) = match confirmed {
        Confirmed::Never => (None, 0i64, None),
        Confirmed::Yes => (Some(NOW), 3, Some(vec![0x33u8; 32])),
    };
    sqlx::query(
        "INSERT INTO device (id, organization_id, hermes_id, public_key, state, state_since, \
                             slot, first_confirmed_at, current_generation, current_digest, \
                             max_generation, verification_code, confirm_until, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $9, $11, $12, $6)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(hermes)
    .bind(identity.as_slice())
    .bind(state)
    .bind(NOW)
    .bind(slot)
    .bind(first_confirmed_at)
    .bind(generation)
    .bind(digest)
    .bind((state == "pending").then_some("123456"))
    .bind((state == "pending").then_some(NOW + 600_000))
    .execute(pool)
    .await
    .unwrap_or_else(|error| panic!("a {state} device is created: {error}"));
    (id, identity)
}

/// Creates an invite of `hermes` in `state`. A used invite needs the device it created.
pub async fn invite(
    pool: &PgPool,
    organization: ResolvedOrg,
    hermes: Uuid,
    state: &str,
    device: Option<Uuid>,
) -> (Uuid, [u8; 32]) {
    let id = Uuid::new_v4();
    let mut digest = [0u8; 32];
    digest[..16].copy_from_slice(id.as_bytes());
    digest[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    sqlx::query(
        "INSERT INTO invite (id, organization_id, hermes_id, digest, state, created_at, \
                             expires_at, used_at, device_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(organization.id())
    .bind(hermes)
    .bind((state != "expired").then_some(digest.as_slice()))
    .bind(state)
    .bind(NOW)
    .bind(NOW + 900_000)
    .bind(device.map(|_| NOW))
    .bind(device)
    .execute(pool)
    .await
    .unwrap_or_else(|error| panic!("a {state} invite is created: {error}"));
    (id, digest)
}

/// One text column of one row, for the assertions.
pub async fn text_of(
    pool: &PgPool,
    table: &'static str,
    column: &'static str,
    id: Uuid,
) -> Option<String> {
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT {column}::text FROM {table} WHERE id = $1"
    )))
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|error| panic!("{table}.{column} is read: {error}"))
}
