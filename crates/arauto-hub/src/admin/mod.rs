//! The admin command: what a person does on the machine of the hub (`docs/15-mvp.md`,
//! section 1). In the MVP there is no admin address on the network; `arauto-hub admin` is
//! the panel.
//!
//! Whoever runs it already has the database and the master key in hand, so it opens no
//! weaker way in. It is the same program as the hub and writes through the same functions:
//! every change here is a call into [`crate::repo`], which checks the database against the
//! audit mark before writing. A database that came back from an older copy gets **nothing**
//! from this command, and the mark is left where it is ([`AdminError::Restored`]): the proof
//! stays for the hub, which turns the restored mode on when it starts.
//!
//! The actor in the audit is the local admin, with the system user that ran the command as a
//! declared text.
//!
//! What the command prints is in [`render`]. Nothing of it is a secret but the invite, which
//! appears once; every text that came from a device is escaped.

use std::path::{Path, PathBuf};

use arauto_protocol::code::VerificationCode;
use arauto_protocol::message::DeclaredText;
use arauto_protocol::names::{HermesName, NameError};
use arauto_protocol::noise::{PublicKey, StaticKeypair};
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::audit::{self, Action, AuditError, Event, TargetKind};
use crate::config::PublicUrl;
use crate::core::model::Hermes;
use crate::db::{self, BackupConfirmed, DbError, ResolvedOrg, SchemaState, SystemTask};
use crate::mark::{Mark, MarkError, MarkValue};
use crate::repo::alert::{Alert, AlertFilter};
use crate::repo::approval::{Answer, ApproveOutcome, DenyOutcome, Recognized, RequestView};
use crate::repo::audit_log::AuditRow;
use crate::repo::closing::{Closed, RevokeOutcome};
use crate::repo::device::UnknownListed;
use crate::repo::hermes::{CreateOutcome, HermesLine, RemoveOutcome, Removed, WakeOutcome};
use crate::repo::invite::{IssueOutcome, IssuedInvite};
use crate::repo::{self, Admin, RepoError};
use crate::restore::{self, Evidence, Installation, RestoreError, Standing};
use crate::vault::{MASTER_KEY_FILE, Vault, VaultError};

pub mod access;
pub mod link;
pub mod panel;
pub mod qr;
pub mod render;
pub mod whatsapp;

/// The name of the organization `init` creates. The MVP has one.
const FIRST_ORGANIZATION: &str = "default";

/// The name of the admin `init` creates.
const LOCAL_ADMIN: &str = "local";

/// What went wrong in the admin command. Each text says what to do.
#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    /// The database refused, or is not at the version of this program.
    #[error(transparent)]
    Db(#[from] DbError),

    /// A repository function failed.
    #[error(transparent)]
    Repo(RepoError),

    /// The vault refused.
    #[error(transparent)]
    Vault(#[from] VaultError),

    /// The audit mark could not be read or written.
    #[error(transparent)]
    Mark(#[from] MarkError),

    /// The database came back from an older copy. **Nothing was written** and the mark was
    /// not touched.
    #[error(
        "the database came back from an older copy ({0}). Nothing was written. Start \
         `arauto-hub serve`: the hub turns the restored mode on, and then each Hermes is \
         reconnected with a new invite"
    )]
    Restored(Evidence),

    /// `init` has not run on this database.
    #[error("the hub is not installed: run `arauto-hub init`")]
    NotInstalled,

    /// The database has data and the master key is not in the data folder.
    #[error(
        "{}: the database has data and the master key is gone. Put back the key file that \
         belongs to this database; `init` never creates a new key over existing data",
        .path.display()
    )]
    MasterKeyMissing {
        /// Where the key file should be.
        path: PathBuf,
    },

    /// The database is empty and the data folder holds the mark of an installation that had
    /// events.
    #[error(
        "the database is empty and the data folder holds the audit mark of another \
         installation (at {mark}): point ARAUTO_DATA_DIR at the folder of this database or, \
         in development, run `arauto-hub dev-reset`"
    )]
    MarkOfAnotherInstallation {
        /// The mark that was found.
        mark: i64,
    },

    /// This command handles one organization, and the database has another number of them.
    #[error("this command handles exactly one organization, and the database has {found}")]
    Organizations {
        /// How many organizations the database has.
        found: usize,
    },

    /// The name is not in the restricted format, or is reserved.
    #[error("not a valid Hermes name: {0}")]
    InvalidName(NameError),

    /// The name is taken by a Hermes that exists.
    #[error("there already is a Hermes with that name")]
    NameTaken,

    /// The name belonged to a Hermes that was removed.
    #[error("that name belonged to a Hermes that was removed, and a name is never reused")]
    NameRetired,

    /// There is no Hermes with that name.
    #[error("there is no Hermes with that name")]
    NoSuchHermes,

    /// What was typed is not a verification code.
    #[error("a verification code has six digits, like 123-456")]
    InvalidCode,

    /// `migrate` was run without the word about the backup.
    #[error(
        "migrating changes a database that has data: take a backup of it first, and then run \
         `arauto-hub migrate --i-have-a-backup`"
    )]
    BackupNotConfirmed,

    /// `dev-reset` was run without its confirmation.
    #[error(
        "dev-reset erases the whole database and the audit mark: run it with \
         --yes-erase-everything"
    )]
    ResetNotConfirmed,

    /// `dev-reset` only runs on a development or test database.
    #[error("dev-reset only runs on a database whose name ends with _dev or starts with arauto_t_")]
    NotADevelopmentDatabase,

    /// A blocking task died.
    #[error("a task of the command failed")]
    Task,

    /// The engine was not configured yet.
    #[error("there is no engine yet: run `arauto-hub admin engine set --url ... --events-url ...`")]
    NoEngine,

    /// An address of the engine or of its events is not in the format.
    #[error("not a usable address: {0}")]
    InvalidAddress(crate::engine::AddressError),

    /// What was typed as the key of the engine is not one.
    #[error("not a key of the engine: {0}")]
    InvalidKey(crate::engine::config::ApiKeyError),

    /// The alias is not in the restricted format, or is reserved.
    #[error("not a valid alias of a number: {0}")]
    InvalidAlias(NameError),

    /// What was typed is not a telephone with the code of the country.
    #[error(
        "a telephone has the code of the country and from 8 to 15 digits, like +55 11 98765-4321"
    )]
    InvalidPhone,

    /// There is no number with that alias.
    #[error("there is no number with that alias")]
    NoSuchNumber,

    /// The alias is taken, now or by a number that was removed.
    #[error(
        "that alias is taken, now or by a number that was removed, and an alias is never reused"
    )]
    AliasTaken,

    /// The telephone is already a number.
    #[error("that telephone is already a number of this hub")]
    PhoneTaken,

    /// What was asked does not apply to the state the number is in.
    #[error("the number is {0}: that does not apply now")]
    NumberNotNow(&'static str),

    /// The engine did not do what was asked.
    #[error(transparent)]
    Engine(#[from] crate::engine::EngineError),

    /// The access asked breaks a rule of `docs/08-permissoes.md`, section 1.
    #[error("{0}")]
    AccessRule(crate::core::access::AccessRuleError),

    /// The wait before waking is out of its range.
    #[error("the wait before waking goes from 0 to 60 seconds")]
    WakeWaitRange,

    /// A link from a Hermes to itself.
    #[error("a Hermes has no link to itself: name two different Hermes")]
    LinkToItself,

    /// The Hermes has no access to that number.
    #[error(
        "that Hermes has no access to that number: give it one with `arauto-hub admin access grant`"
    )]
    NoAccess,

    /// The chat named is not a chat of that number.
    #[error("that chat is not a chat of that number")]
    NoSuchChat,

    /// The access has no line for that chat.
    #[error("that access has no line for that chat")]
    NoLine,

    /// What was given is not a chat.
    #[error(
        "a chat is a telephone with the code of the country, an identifier of the engine \
         (like 120363012345678901@g.us) or an identifier shown by `arauto-hub admin access list`"
    )]
    InvalidChat,

    /// The engine says the telephone has no WhatsApp.
    #[error("the engine says that telephone has no WhatsApp")]
    NoWhatsapp,

    /// The password of the database role was not replaced, or was replaced and something
    /// after it failed (the text says which).
    #[error(transparent)]
    DatabasePassword(db::RotationError),
}

impl From<RepoError> for AdminError {
    fn from(error: RepoError) -> Self {
        match error {
            RepoError::Restored(evidence) => AdminError::Restored(evidence),
            RepoError::Database(error) => AdminError::Db(error),
            other => AdminError::Repo(other),
        }
    }
}

impl From<NameError> for AdminError {
    fn from(error: NameError) -> Self {
        AdminError::InvalidName(error)
    }
}

impl From<AuditError> for AdminError {
    fn from(error: AuditError) -> Self {
        RepoError::from(error).into()
    }
}

impl From<RestoreError> for AdminError {
    fn from(error: RestoreError) -> Self {
        RepoError::from(error).into()
    }
}

impl From<sqlx::Error> for AdminError {
    fn from(error: sqlx::Error) -> Self {
        AdminError::Db(DbError::Query(error))
    }
}

// ---------------------------------------------------------------------------- init

/// An installation: what `init` created, or found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installed {
    /// The public key of the hub, which goes in every block.
    pub hub_key: PublicKey,
    /// The organization.
    pub organization: ResolvedOrg,
}

/// The outcome of [`init`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitOutcome {
    /// The installation was created now.
    Created(Installed),
    /// It already existed. Nothing was changed.
    AlreadyInstalled(Installed),
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, AdminError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| AdminError::Task)
}

async fn has_data(pool: &PgPool) -> Result<bool, AdminError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM installation) OR EXISTS (SELECT 1 FROM audit)",
    )
    .fetch_one(pool)
    .await?)
}

/// The active key of the hub: the row, the public part and the encrypted private part.
async fn active_hub_key(pool: &PgPool) -> Result<Option<(Uuid, PublicKey, Vec<u8>)>, AdminError> {
    let row: Option<(Uuid, Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT id, public_key, private_key_enc FROM hub_key WHERE state = 'active'",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(id, public, private)| {
        PublicKey::from_slice(&public).map(|public| (id, public, private))
    }))
}

/// The public key of the hub, as it goes in the block (`arh1...` with
/// [`PublicKey::to_text`]). `None` before `init`.
///
/// # Errors
/// [`AdminError::Db`] when the database refuses.
pub async fn hub_public_key(pool: &PgPool) -> Result<Option<PublicKey>, AdminError> {
    Ok(active_hub_key(pool).await?.map(|(_, public, _)| public))
}

/// The key pair of the hub, opened with the master key: what the hub answers handshakes
/// with.
///
/// # Errors
/// [`AdminError::NotInstalled`] before `init`; [`AdminError::Vault`] when the stored key
/// does not open with this master key.
pub async fn hub_keypair(pool: &PgPool, vault: &Vault) -> Result<StaticKeypair, AdminError> {
    let (id, public, private) = active_hub_key(pool)
        .await?
        .ok_or(AdminError::NotInstalled)?;
    Ok(vault.open_hub_key(id, &public, &private)?)
}

async fn the_organization(pool: &PgPool, task: SystemTask) -> Result<ResolvedOrg, AdminError> {
    let organizations = db::organizations_for_system_task(pool, task).await?;
    match organizations.as_slice() {
        [organization] => Ok(*organization),
        [] => Err(AdminError::NotInstalled),
        more => Err(AdminError::Organizations { found: more.len() }),
    }
}

/// `arauto-hub init`: creates the master key, the mark, the organization, the local admin
/// and the key of the hub. It does not redo what exists.
///
/// - A database that is not empty and not at the version of this program is refused.
/// - A database with data and no master key is refused: a new key would open nothing in it.
/// - A database with data and no mark is left as it is: that is the proof of a restored
///   database, and `init` does not erase it.
/// - An empty database beside a mark that already counted events is refused: the mark is of
///   another installation.
///
/// # Errors
/// The refusals above, and what the database, the vault or the mark return.
pub async fn init(
    pool: &PgPool,
    data_dir: &Path,
    os_user: &DeclaredText,
    allow_unsafe_db: bool,
    now: i64,
) -> Result<InitOutcome, AdminError> {
    db::check_durability(pool, allow_unsafe_db).await?;
    db::migrate_new_database(pool, &db::MIGRATOR).await?;

    let key_path = data_dir.join(MASTER_KEY_FILE);
    let dir = data_dir.to_path_buf();
    if has_data(pool).await? {
        let vault = match blocking(move || Vault::open(&dir)).await? {
            Ok(vault) => vault,
            Err(VaultError::MasterKeyMissing { .. }) => {
                return Err(AdminError::MasterKeyMissing { path: key_path });
            }
            Err(other) => return Err(other.into()),
        };
        // The key in the folder must be the one of this database.
        let keypair = hub_keypair(pool, &vault).await?;
        let organization = the_organization(pool, SystemTask::Install).await?;
        return Ok(InitOutcome::AlreadyInstalled(Installed {
            hub_key: *keypair.public(),
            organization,
        }));
    }

    let vault = blocking(move || match Vault::open(&dir) {
        Err(VaultError::MasterKeyMissing { .. }) => Vault::create(&dir),
        found => found,
    })
    .await??;
    let mark = Mark::new(data_dir);
    match mark.read().await? {
        // Only here, with no data in the database, is a missing mark not evidence.
        MarkValue::Absent => {
            mark.advance(0).await?;
        }
        MarkValue::At(0) => {}
        MarkValue::At(mark) => return Err(AdminError::MarkOfAnotherInstallation { mark }),
    }

    let organization_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let hub_key_id = Uuid::new_v4();
    let organization_key = vault.generate_organization_key(organization_id)?;
    let hub_key = vault.generate_hub_key(hub_key_id)?;

    let mut write = audit::begin_security(pool, &mark).await?;
    sqlx::query("INSERT INTO installation (created_at) VALUES ($1)")
        .bind(now)
        .execute(write.connection())
        .await?;
    sqlx::query(
        "INSERT INTO organization (id, name, state, key_enc, created_at) \
         VALUES ($1, $2, 'active', $3, $4)",
    )
    .bind(organization_id)
    .bind(FIRST_ORGANIZATION)
    .bind(organization_key)
    .bind(now)
    .execute(write.connection())
    .await?;
    sqlx::query(
        "INSERT INTO admin (id, organization_id, name, kind, created_at) \
         VALUES ($1, $2, $3, 'local', $4)",
    )
    .bind(admin_id)
    .bind(organization_id)
    .bind(LOCAL_ADMIN)
    .bind(now)
    .execute(write.connection())
    .await?;
    sqlx::query(
        "INSERT INTO hub_key (id, public_key, private_key_enc, state, created_at) \
         VALUES ($1, $2, $3, 'active', $4)",
    )
    .bind(hub_key_id)
    .bind(hub_key.public.as_bytes().as_slice())
    .bind(hub_key.private_key_enc)
    .bind(now)
    .execute(write.connection())
    .await?;
    let organization = db::organizations_for_system_task(write.connection(), SystemTask::Install)
        .await?
        .into_iter()
        .find(|organization| organization.id() == organization_id)
        .ok_or(AdminError::NotInstalled)?;
    let actor = Admin {
        id: admin_id,
        os_user: os_user.clone(),
    }
    .actor();
    let events = [
        Event::new(Action::InstallationCreated, actor.clone()),
        Event::new(Action::OrganizationCreated, actor.clone())
            .in_organization(organization)
            .about(TargetKind::Organization, organization_id),
        Event::new(Action::AdminCreated, actor.clone())
            .in_organization(organization)
            .about(TargetKind::Admin, admin_id),
        Event::new(Action::HubKeyCreated, actor).about(TargetKind::HubKey, hub_key_id),
    ];
    for event in &events {
        write.record(now, event).await?;
    }
    write.commit().await?;
    Ok(InitOutcome::Created(Installed {
        hub_key: hub_key.public,
        organization,
    }))
}

/// What `serve` does before it starts: on an **empty** database, the same as [`init`], by the
/// same function and with the same refusals. Returns what was installed, or `None` when the
/// database has anything in it: then nothing is installed over it and no migration is applied
/// here, and the checks of the start say what is wrong with it.
///
/// # Errors
/// What [`init`] returns.
pub async fn install_if_new(
    pool: &PgPool,
    data_dir: &Path,
    os_user: &DeclaredText,
    allow_unsafe_db: bool,
    now: i64,
) -> Result<Option<Installed>, AdminError> {
    if db::schema_state(pool, &db::MIGRATOR).await? != db::SchemaState::Empty {
        return Ok(None);
    }
    match init(pool, data_dir, os_user, allow_unsafe_db, now).await? {
        InitOutcome::Created(installed) => Ok(Some(installed)),
        InitOutcome::AlreadyInstalled(_) => Ok(None),
    }
}

// ---------------------------------------------------------------------------- migrate, status, dev-reset

/// `arauto-hub migrate --i-have-a-backup`: applies the pending migrations to a database that
/// has data. Returns the versions it applied.
///
/// # Errors
/// [`AdminError::BackupNotConfirmed`] without the word about the backup: nothing is applied.
/// And the refusals of [`db::migrate_existing_database`].
pub async fn migrate(
    pool: &PgPool,
    i_have_a_backup: bool,
    now: i64,
) -> Result<Vec<i64>, AdminError> {
    if !i_have_a_backup {
        return Err(AdminError::BackupNotConfirmed);
    }
    let applied =
        db::migrate_existing_database(pool, &db::MIGRATOR, BackupConfirmed::i_have_a_backup())
            .await?;
    if !applied.is_empty() {
        let details = crate::audit::Details::new().number(
            "migrations",
            i64::try_from(applied.len()).unwrap_or(i64::MAX),
        );
        audit::record(
            pool,
            now,
            &Event::new(Action::DatabaseMigrated, crate::audit::Actor::System).with(details),
        )
        .await?;
    }
    Ok(applied)
}

/// What `admin status` shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// How the database stands against the migrations of this program.
    pub schema: SchemaState,
    /// The installation, with the restored mode. `None` before `init`, or when the schema is
    /// not the one of this program.
    pub installation: Option<Installation>,
    /// The public key of the hub.
    pub hub_key: Option<PublicKey>,
    /// The audit mark.
    pub mark: MarkValue,
    /// How the database stands against the mark. `None` when the schema is not current.
    pub standing: Option<Standing>,
    /// The Hermes lines, when there is exactly one organization.
    pub hermes: Vec<HermesLine>,
    /// How many alerts nobody marked as seen.
    pub unseen_alerts: usize,
    /// How many devices the database does not know presented a valid seal and were listed.
    pub unknown_devices: usize,
}

/// `arauto-hub admin status`: the situation, the public key of the hub and the restored
/// mode. Reads only; it works on a database that is not installed or that came back from a
/// copy, and says so.
///
/// # Errors
/// What the database or the mark return.
pub async fn status(pool: &PgPool, mark: &Mark) -> Result<Status, AdminError> {
    let schema = db::schema_state(pool, &db::MIGRATOR).await?;
    let marked = mark.read().await?;
    let mut status = Status {
        schema: schema.clone(),
        installation: None,
        hub_key: None,
        mark: marked,
        standing: None,
        hermes: Vec::new(),
        unseen_alerts: 0,
        unknown_devices: 0,
    };
    if schema != SchemaState::Current {
        return Ok(status);
    }
    status.installation = restore::installation(pool).await?;
    status.hub_key = hub_public_key(pool).await?;
    status.standing = Some(restore::check(pool, mark).await?);
    let organization = match the_organization(pool, SystemTask::LocalAdmin).await {
        Ok(organization) => organization,
        // Not installed, or more organizations than this command lists: nothing to count.
        Err(AdminError::NotInstalled | AdminError::Organizations { .. }) => return Ok(status),
        Err(other) => return Err(other),
    };
    status.hermes = repo::hermes::list(pool, organization, false).await?;
    status.unseen_alerts =
        repo::alert::list(pool, organization, AlertFilter::Unseen, UNSEEN_COUNT_MAX)
            .await?
            .len();
    status.unknown_devices = repo::device::unknown_listed(pool, organization, UNSEEN_COUNT_MAX)
        .await?
        .len();
    Ok(status)
}

/// How many unseen alerts, and how many unknown devices, `status` counts before saying "or
/// more".
pub const UNSEEN_COUNT_MAX: u32 = 1_000;

/// `arauto-hub dev-reset`: erases the database **and** the audit mark, together. Erasing one
/// of the two would turn the restored mode on, which is the right behavior everywhere else.
///
/// It only runs with the explicit confirmation, and on a database whose name ends with
/// `_dev` or starts with `arauto_t_`. The master key stays: the next `init` uses it.
///
/// # Errors
/// [`AdminError::ResetNotConfirmed`] and [`AdminError::NotADevelopmentDatabase`]: nothing is
/// erased.
pub async fn dev_reset(pool: &PgPool, mark: &Mark, confirmed: bool) -> Result<(), AdminError> {
    if !confirmed {
        return Err(AdminError::ResetNotConfirmed);
    }
    let (database, schema): (String, Option<String>) =
        sqlx::query_as("SELECT current_database()::text, current_schema()::text")
            .fetch_one(pool)
            .await?;
    if !is_disposable(&database) || schema.as_deref() != Some("public") {
        return Err(AdminError::NotADevelopmentDatabase);
    }
    // The database first: stopping between the two leaves a mark ahead of an empty database,
    // which `init` refuses and a second `dev-reset` fixes.
    let mut transaction = pool.begin().await?;
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("CREATE SCHEMA public")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    mark.erase().await?;
    Ok(())
}

/// Whether `dev-reset` may erase a database with this name: only one of development (`..._dev`)
/// or of the tests (`arauto_t_...`).
fn is_disposable(database: &str) -> bool {
    database.ends_with("_dev") || database.starts_with("arauto_t_")
}

// ---------------------------------------------------------------------------- the session of the command

/// One run of `arauto-hub admin`: the database, the mark, the organization and who is
/// running it.
#[derive(Debug, Clone)]
pub struct Session {
    pool: PgPool,
    mark: Mark,
    organization: ResolvedOrg,
    admin: Admin,
}

/// The block the admin pastes into the command channel of the Hermes: where the hub is, how
/// to know it, and the invite.
#[derive(Debug)]
pub struct Block {
    /// What was issued. It holds the invite: show it once.
    pub issued: Box<IssuedInvite>,
    /// The address of the channel of the hub.
    pub hub_url: String,
    /// The public key of the hub.
    pub hub_key: PublicKey,
}

impl Session {
    /// Opens the session of the local admin on an installed hub.
    ///
    /// # Errors
    /// [`AdminError::Db`] when the database is not at the version of this program;
    /// [`AdminError::NotInstalled`] before `init`; [`AdminError::Organizations`] when the
    /// database has more than one organization.
    pub async fn open(pool: PgPool, mark: Mark, os_user: DeclaredText) -> Result<Self, AdminError> {
        db::require_current_schema(&pool, &db::MIGRATOR).await?;
        let organization = the_organization(&pool, SystemTask::LocalAdmin).await?;
        Self::open_for(pool, mark, organization, os_user).await
    }

    /// Opens the session of the local admin of `organization`.
    ///
    /// # Errors
    /// [`AdminError::NotInstalled`] when the organization has no local admin.
    pub async fn open_for(
        pool: PgPool,
        mark: Mark,
        organization: ResolvedOrg,
        os_user: DeclaredText,
    ) -> Result<Self, AdminError> {
        let admin = repo::local_admin(&pool, organization, os_user)
            .await?
            .ok_or(AdminError::NotInstalled)?;
        Ok(Self {
            pool,
            mark,
            organization,
            admin,
        })
    }

    /// The organization of this session.
    #[must_use]
    pub const fn organization(&self) -> ResolvedOrg {
        self.organization
    }

    /// `admin hermes add NAME`.
    ///
    /// # Errors
    /// [`AdminError::InvalidName`] for a name outside the format or reserved;
    /// [`AdminError::NameTaken`] and [`AdminError::NameRetired`].
    pub async fn hermes_add(&self, name: &str, now: i64) -> Result<Hermes, AdminError> {
        let name = HermesName::parse(name)?;
        let created = repo::hermes::create(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            now,
        )
        .await?;
        match created {
            CreateOutcome::Created(hermes) => Ok(hermes),
            CreateOutcome::NameTaken => Err(AdminError::NameTaken),
            CreateOutcome::NameRetired => Err(AdminError::NameRetired),
        }
    }

    /// `admin hermes list`.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn hermes_list(&self, include_removed: bool) -> Result<Vec<HermesLine>, AdminError> {
        Ok(repo::hermes::list(&self.pool, self.organization, include_removed).await?)
    }

    /// `admin hermes remove NAME`.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::Restored`].
    pub async fn hermes_remove(&self, name: &str, now: i64) -> Result<Removed, AdminError> {
        let name = known_name(name)?;
        let removed = repo::hermes::remove(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            now,
        )
        .await?;
        match removed {
            RemoveOutcome::Removed(removed) => Ok(removed),
            RemoveOutcome::NotFound => Err(AdminError::NoSuchHermes),
        }
    }

    /// `admin wake NAME on|off`.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::Restored`].
    pub async fn wake(&self, name: &str, on: bool, now: i64) -> Result<WakeOutcome, AdminError> {
        let name = known_name(name)?;
        let outcome = repo::hermes::set_wake(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            on,
            now,
        )
        .await?;
        match outcome {
            WakeOutcome::NotFound => Err(AdminError::NoSuchHermes),
            other => Ok(other),
        }
    }

    /// `admin invite NAME`: issues the invite and returns the block. This is the only moment
    /// the invite appears.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::Restored`]; [`AdminError::NotInstalled`]
    /// when the hub has no key.
    pub async fn invite(
        &self,
        name: &str,
        public_url: &PublicUrl,
        now: i64,
    ) -> Result<Block, AdminError> {
        let name = known_name(name)?;
        let hub_key = hub_public_key(&self.pool)
            .await?
            .ok_or(AdminError::NotInstalled)?;
        let issued = repo::invite::issue(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            now,
        )
        .await?;
        match issued {
            IssueOutcome::Issued(issued) => Ok(Block {
                issued,
                hub_url: public_url.channel_url(),
                hub_key,
            }),
            IssueOutcome::NotFound => Err(AdminError::NoSuchHermes),
        }
    }

    /// The pending or approved request of a Hermes, with the last use of the device it would
    /// replace: what `admin approve` shows before asking for the code.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn request(&self, name: &str) -> Result<Option<RequestView>, AdminError> {
        let name = known_name(name)?;
        Ok(repo::approval::request_of(&self.pool, self.organization, &name).await?)
    }

    /// `admin approve NAME CODE [--recognized | --not-recognized]`. The code is typed by the
    /// admin; the hub never shows it. On a reconnect, `recognition` carries the answer and
    /// the last use the command showed ([`Session::request`]): an answer about a last use
    /// that is no longer the one of the device approves nothing.
    ///
    /// # Errors
    /// [`AdminError::InvalidCode`] when what was typed is not six digits: nothing is
    /// counted. [`AdminError::NoSuchHermes`]; [`AdminError::Restored`].
    pub async fn approve(
        &self,
        name: &str,
        typed: &str,
        recognition: Option<Recognized>,
        now: i64,
    ) -> Result<ApproveOutcome, AdminError> {
        let name = known_name(name)?;
        let code = VerificationCode::parse(typed.trim()).ok_or(AdminError::InvalidCode)?;
        let outcome = repo::approval::approve(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            Answer { code, recognition },
            now,
        )
        .await?;
        match outcome {
            ApproveOutcome::NotFound => Err(AdminError::NoSuchHermes),
            other => Ok(other),
        }
    }

    /// `admin deny NAME`.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::Restored`].
    pub async fn deny(&self, name: &str, now: i64) -> Result<DenyOutcome, AdminError> {
        let name = known_name(name)?;
        let outcome = repo::approval::deny(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            now,
        )
        .await?;
        match outcome {
            DenyOutcome::NotFound => Err(AdminError::NoSuchHermes),
            other => Ok(other),
        }
    }

    /// `admin revoke NAME`.
    ///
    /// # Errors
    /// [`AdminError::NoSuchHermes`]; [`AdminError::Restored`].
    pub async fn revoke(&self, name: &str, now: i64) -> Result<Closed, AdminError> {
        let name = known_name(name)?;
        let outcome = repo::closing::revoke(
            &self.pool,
            &self.mark,
            self.organization,
            &self.admin,
            &name,
            now,
        )
        .await?;
        match outcome {
            RevokeOutcome::Revoked(closed) => Ok(closed),
            RevokeOutcome::NotFound => Err(AdminError::NoSuchHermes),
        }
    }

    /// `admin alerts`.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn alerts(&self, filter: AlertFilter, limit: u32) -> Result<Vec<Alert>, AdminError> {
        Ok(repo::alert::list(&self.pool, self.organization, filter, limit).await?)
    }

    /// The devices the database does not know and that presented a valid seal: what the
    /// panel would list beside the alerts (C11).
    ///
    /// # Errors
    /// What the database returns.
    pub async fn unknown_devices(&self, limit: u32) -> Result<Vec<UnknownListed>, AdminError> {
        Ok(repo::device::unknown_listed(&self.pool, self.organization, limit).await?)
    }

    /// The installation, with the restored mode: every command shows it when it is on.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn installation(&self) -> Result<Option<Installation>, AdminError> {
        Ok(restore::installation(&self.pool).await?)
    }

    /// `admin alerts --ack ID`. Returns whether there was an unseen alert with that
    /// identifier.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn acknowledge(&self, alert: Uuid, now: i64) -> Result<bool, AdminError> {
        Ok(repo::alert::acknowledge(&self.pool, self.organization, alert, now).await?)
    }

    /// `admin audit --tail N`.
    ///
    /// # Errors
    /// What the database returns.
    pub async fn audit(&self, tail: u32) -> Result<Vec<AuditRow>, AdminError> {
        Ok(repo::audit_log::tail(&self.pool, self.organization, tail).await?)
    }
}

/// A name given to a command that looks for an existing Hermes. A text outside the format is
/// the name of no Hermes.
fn known_name(name: &str) -> Result<HermesName, AdminError> {
    HermesName::parse(name).map_err(|_| AdminError::NoSuchHermes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_development_and_test_databases_are_disposable() {
        for name in ["arauto_dev", "x_dev", "arauto_t_1_2_own_3", "arauto_t_"] {
            assert!(is_disposable(name), "{name}");
        }
        for name in [
            "arauto",
            "postgres",
            "arauto_prod",
            "dev",
            "arauto_dev_copy",
            "my_arauto_t_1",
            "ARAUTO_DEV",
            "arauto_t",
            "",
        ] {
            assert!(!is_disposable(name), "{name}");
        }
    }

    #[test]
    fn a_name_that_is_no_hermes_name_names_no_hermes() {
        assert!(matches!(
            known_name("Vendas"),
            Err(AdminError::NoSuchHermes)
        ));
        assert!(matches!(known_name("admin"), Err(AdminError::NoSuchHermes)));
        assert!(known_name("vendas-2").is_ok());
    }

    #[test]
    fn no_refusal_repeats_what_was_typed() {
        // The texts of the errors are fixed: a name or a code that was typed, which may be
        // anything, is never echoed to the terminal.
        let typed = "\u{1b}[31m";
        let refusals = [
            AdminError::NoSuchHermes.to_string(),
            AdminError::InvalidCode.to_string(),
            AdminError::NameTaken.to_string(),
            AdminError::NameRetired.to_string(),
            HermesName::parse(typed)
                .map_err(AdminError::from)
                .unwrap_err()
                .to_string(),
        ];
        for text in refusals {
            assert!(text.is_ascii() && !text.contains(typed), "{text}");
        }
    }
}
