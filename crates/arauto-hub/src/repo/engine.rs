//! The engine of an organization (`docs/06-modelo-de-dados.md`, section 4): where it is, its
//! key and the secret of its events, sealed by the vault, and what the hub last saw of it.
//!
//! Every change is a security event, written through [`crate::audit::begin_security`]: the
//! address of the engine is where every number is reached, and its two secrets are who proves
//! itself to whom.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::alert::{AlertDetails, AlertKind, NewAlert, raise};
use super::{Admin, RepoError, notify_hub};
use crate::audit::{self, Action, Actor, Details, Event, TargetKind};
use crate::db::ResolvedOrg;
use crate::engine::{ApiKey, EngineAddress, EngineVersion, EventsSecret, EventsUrl};
use crate::mark::Mark;
use crate::vault::{EngineSecretKind, Vault};

/// Who set the row of the engine (`engine.origin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineOrigin {
    /// An admin, with `admin engine set`. The key is typed by the admin.
    Admin,
    /// The hub, from the address the installation gives. The key is drawn by the hub and is
    /// never typed.
    Installation,
}

impl EngineOrigin {
    /// The word stored in the database and written in the details of an audit row or alert.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Installation => "installation",
        }
    }

    /// The origin a stored word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "admin" => Some(Self::Admin),
            "installation" => Some(Self::Installation),
            _ => None,
        }
    }
}

/// The row of the engine.
#[derive(Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct EngineRow {
    /// The engine.
    pub id: Uuid,
    /// Who set it: the stored word of [`EngineOrigin`]. Read it with [`EngineRow::origin`].
    pub origin: String,
    /// When the engine began refusing the key of the hub, until it answers again or its
    /// configuration changes.
    pub key_refused_since: Option<i64>,
    /// Where the hub reaches it.
    pub address: String,
    /// Where it reaches the hub.
    pub events_url: String,
    /// The key of its API, sealed.
    pub api_key_enc: Option<Vec<u8>>,
    /// The secret of its events, sealed.
    pub events_secret_enc: Vec<u8>,
    /// Grows on every change.
    pub config_version: i64,
    /// The version the hub saw last.
    pub version_seen: Option<String>,
    /// Whether that version is one the hub was checked against.
    pub version_tested: Option<bool>,
    /// When the engine last answered.
    pub last_ok_at: Option<i64>,
    /// When it was created.
    pub created_at: i64,
    /// When it last changed.
    pub changed_at: i64,
}

impl std::fmt::Debug for EngineRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineRow")
            .field("id", &self.id)
            .field("origin", &self.origin)
            .field("config_version", &self.config_version)
            .field("has_key", &self.api_key_enc.is_some())
            .finish_non_exhaustive()
    }
}

/// The secrets of the engine, opened.
#[derive(Debug)]
pub struct EngineSecrets {
    /// The key of its API, when it was set.
    pub key: Option<ApiKey>,
    /// The secret of its events.
    pub events: EventsSecret,
}

impl EngineRow {
    /// Who set the row. A word the program does not know counts as an admin: that is the side
    /// that writes no file.
    #[must_use]
    pub fn origin(&self) -> EngineOrigin {
        EngineOrigin::parse(&self.origin).unwrap_or(EngineOrigin::Admin)
    }

    /// Opens the two secrets with the master key.
    ///
    /// # Errors
    /// [`RepoError::Vault`] when a block does not open; [`RepoError::Inconsistent`] when what
    /// opens is not a key or a secret.
    pub fn secrets(&self, vault: &Vault) -> Result<EngineSecrets, RepoError> {
        let key = match &self.api_key_enc {
            Some(block) => {
                let opened = vault.open_engine_secret(self.id, EngineSecretKind::ApiKey, block)?;
                let text = std::str::from_utf8(&opened)
                    .map_err(|_| RepoError::Inconsistent("the key of the engine is not text"))?;
                Some(ApiKey::parse(text).map_err(|_| {
                    RepoError::Inconsistent("the key of the engine is not in its format")
                })?)
            }
            None => None,
        };
        let events = vault.open_engine_secret(
            self.id,
            EngineSecretKind::EventsSecret,
            &self.events_secret_enc,
        )?;
        let events = EventsSecret::from_stored(&events).ok_or(RepoError::Inconsistent(
            "the secret of the events is not 32 bytes",
        ))?;
        Ok(EngineSecrets { key, events })
    }
}

/// The engine of `organization`, if there is one.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn get(pool: &PgPool, organization: ResolvedOrg) -> Result<Option<EngineRow>, RepoError> {
    Ok(sqlx::query_as(
        "SELECT id, origin, key_refused_since, address, events_url, api_key_enc, events_secret_enc, config_version, \
                version_seen, version_tested, last_ok_at, created_at, changed_at \
         FROM engine WHERE organization_id = $1",
    )
    .bind(organization.id())
    .fetch_optional(pool)
    .await?)
}

/// The identifier and the version of the configuration of the engine of `organization`: what
/// the hub reads on each call to know whether its client is still the right one.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn current_version(
    pool: &PgPool,
    organization: ResolvedOrg,
) -> Result<Option<(Uuid, i64)>, RepoError> {
    Ok(
        sqlx::query_as("SELECT id, config_version FROM engine WHERE organization_id = $1")
            .bind(organization.id())
            .fetch_optional(pool)
            .await?,
    )
}

/// What [`set_address`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOutcome {
    /// The engine was created, with a new secret of its events.
    Created(Uuid),
    /// Its address changed.
    Changed(Uuid),
    /// It already had these addresses: nothing was written.
    Unchanged(Uuid),
    /// The row was the engine brought by the installation and the admin pointed it at another
    /// address: it is now an engine set by an admin, and its drawn key is gone.
    LeftInstallation(Uuid),
}

/// Creates the engine of `organization` with these addresses and a new secret of its events,
/// or changes its addresses.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn set_address(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    vault: &Vault,
    address: &EngineAddress,
    events_url: &EventsUrl,
    now: i64,
) -> Result<SetOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let current: Option<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT id, address, events_url, origin FROM engine \
         WHERE organization_id = $1 FOR UPDATE",
    )
    .bind(organization.id())
    .fetch_optional(write.connection())
    .await?;
    let address_text = address.http().to_text();
    let events_text = events_url.to_text();
    let outcome = match current {
        Some((id, stored, events, _)) if stored == address_text && events == events_text => {
            write.rollback().await?;
            return Ok(SetOutcome::Unchanged(id));
        }
        // Another address on the engine of the installation: the admin points the organization
        // at another engine. The drawn key is erased, so the hub never sends it to an address
        // the installation did not give; the admin types the key of the new engine.
        Some((id, stored, _, origin))
            if stored != address_text && origin == EngineOrigin::Installation.as_str() =>
        {
            sqlx::query(
                "UPDATE engine SET address = $3, events_url = $4, origin = 'admin', \
                        api_key_enc = NULL, key_refused_since = NULL, \
                        config_version = config_version + 1, changed_at = $5 \
                 WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(id)
            .bind(&address_text)
            .bind(&events_text)
            .bind(now)
            .execute(write.connection())
            .await?;
            SetOutcome::LeftInstallation(id)
        }
        Some((id, _, _, _)) => {
            sqlx::query(
                "UPDATE engine SET address = $3, events_url = $4, key_refused_since = NULL, \
                        config_version = config_version + 1, changed_at = $5 \
                 WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(id)
            .bind(&address_text)
            .bind(&events_text)
            .bind(now)
            .execute(write.connection())
            .await?;
            SetOutcome::Changed(id)
        }
        None => {
            let id = Uuid::new_v4();
            let secret = EventsSecret::draw().map_err(|_| RepoError::Random)?;
            let sealed =
                vault.seal_engine_secret(id, EngineSecretKind::EventsSecret, secret.expose())?;
            sqlx::query(
                "INSERT INTO engine (id, organization_id, kind, origin, address, events_url, \
                                     events_secret_enc, config_version, created_at, changed_at) \
                 VALUES ($1, $2, 'waha', 'admin', $3, $4, $5, 1, $6, $6)",
            )
            .bind(id)
            .bind(organization.id())
            .bind(&address_text)
            .bind(&events_text)
            .bind(sealed)
            .bind(now)
            .execute(write.connection())
            .await?;
            SetOutcome::Created(id)
        }
    };
    let (SetOutcome::Created(id)
    | SetOutcome::Changed(id)
    | SetOutcome::Unchanged(id)
    | SetOutcome::LeftInstallation(id)) = outcome;
    let mut details = Details::new().flag("created", matches!(outcome, SetOutcome::Created(_)));
    if matches!(outcome, SetOutcome::LeftInstallation(_)) {
        details = details.flag("left_installation", true);
    }
    write
        .record(
            now,
            &Event::new(Action::EngineConfigured, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Engine, id)
                .with(details),
        )
        .await?;
    notify_hub(write.connection(), organization, "engine", id).await?;
    write.commit().await?;
    Ok(outcome)
}

/// What [`set_key`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetKeyOutcome {
    /// The key was sealed and stored.
    Set,
    /// There is no engine yet: nothing was written.
    NoEngine,
    /// The engine is the one brought by the installation: its key is drawn by the hub and is
    /// never typed. Nothing was written and nothing was audited.
    InstallationEngine,
}

/// Seals and stores the key of the API of the engine typed by an admin. It never changes the
/// key of the engine brought by the installation: the hub drew that one.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn set_key(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    vault: &Vault,
    key: &ApiKey,
    now: i64,
) -> Result<SetKeyOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let found: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, origin FROM engine WHERE organization_id = $1 FOR UPDATE")
            .bind(organization.id())
            .fetch_optional(write.connection())
            .await?;
    let Some((id, origin)) = found else {
        write.rollback().await?;
        return Ok(SetKeyOutcome::NoEngine);
    };
    if origin == EngineOrigin::Installation.as_str() {
        write.rollback().await?;
        return Ok(SetKeyOutcome::InstallationEngine);
    }
    let sealed = vault.seal_engine_secret(id, EngineSecretKind::ApiKey, key.expose().as_bytes())?;
    // The second guard: only an admin row takes a typed key.
    let changed = sqlx::query(
        "UPDATE engine SET api_key_enc = $3, key_refused_since = NULL, \
                config_version = config_version + 1, changed_at = $4 \
         WHERE organization_id = $1 AND id = $2 AND origin = 'admin'",
    )
    .bind(organization.id())
    .bind(id)
    .bind(sealed)
    .bind(now)
    .execute(write.connection())
    .await?
    .rows_affected();
    if changed != 1 {
        write.rollback().await?;
        return Ok(SetKeyOutcome::InstallationEngine);
    }
    write
        .record(
            now,
            &Event::new(Action::EngineKeyChanged, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Engine, id),
        )
        .await?;
    notify_hub(write.connection(), organization, "engine", id).await?;
    write.commit().await?;
    Ok(SetKeyOutcome::Set)
}

/// Draws a new secret of the events of the engine. The hub gives it to every session of the
/// engine at its next check. `false` when there is no engine yet.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
pub async fn new_events_secret(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    vault: &Vault,
    now: i64,
) -> Result<bool, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM engine WHERE organization_id = $1 FOR UPDATE")
            .bind(organization.id())
            .fetch_optional(write.connection())
            .await?;
    let Some(id) = id else {
        write.rollback().await?;
        return Ok(false);
    };
    let secret = EventsSecret::draw().map_err(|_| RepoError::Random)?;
    let sealed = vault.seal_engine_secret(id, EngineSecretKind::EventsSecret, secret.expose())?;
    sqlx::query(
        "UPDATE engine SET events_secret_enc = $3, key_refused_since = NULL, \
                config_version = config_version + 1, changed_at = $4 \
         WHERE organization_id = $1 AND id = $2",
    )
    .bind(organization.id())
    .bind(id)
    .bind(sealed)
    .bind(now)
    .execute(write.connection())
    .await?;
    write
        .record(
            now,
            &Event::new(Action::EngineSecretChanged, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Engine, id),
        )
        .await?;
    notify_hub(write.connection(), organization, "engine", id).await?;
    write.commit().await?;
    Ok(true)
}

/// Records the version the engine said, and that it answered. Returns whether the version is
/// another than the one seen before (or the first): the hub alerts once per version.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn record_version(
    pool: &PgPool,
    organization: ResolvedOrg,
    engine: Uuid,
    version: &EngineVersion,
    now: i64,
) -> Result<bool, RepoError> {
    let seen = format!(
        "{}+{}",
        version.version,
        version.engine.to_ascii_lowercase()
    );
    let before: Option<Option<String>> = sqlx::query_scalar(
        "UPDATE engine e SET version_seen = $3, version_tested = $4, last_ok_at = $5, \
                key_refused_since = NULL \
         FROM (SELECT version_seen FROM engine WHERE organization_id = $1 AND id = $2 FOR UPDATE) old \
         WHERE e.organization_id = $1 AND e.id = $2 \
         RETURNING old.version_seen",
    )
    .bind(organization.id())
    .bind(engine)
    .bind(&seen)
    .bind(version.is_tested())
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(before.is_some_and(|before| before.as_deref() != Some(seen.as_str())))
}

/// What [`seed_installation`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seeded {
    /// The row was created, with the key and the secret of the events the caller drew.
    Created(Uuid),
    /// The organization already had an engine, or another process created the engine of the
    /// installation first: nothing was written. The key and the secret drawn were dropped.
    AlreadyThere,
}

/// Creates the engine of `organization` as the one brought by the installation, with the
/// addresses the installation gives, the `key` the hub drew and a new secret of its events.
/// One write, safe against a second process doing the same at once: the loser writes nothing.
/// The key is sealed with the id of the row that is inserted, and on a conflict the sealed
/// block is discarded with the key.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn seed_installation(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    vault: &Vault,
    address: &EngineAddress,
    events_url: &EventsUrl,
    key: &ApiKey,
    now: i64,
) -> Result<Seeded, RepoError> {
    let id = Uuid::new_v4();
    let secret = EventsSecret::draw().map_err(|_| RepoError::Random)?;
    let sealed_events =
        vault.seal_engine_secret(id, EngineSecretKind::EventsSecret, secret.expose())?;
    let sealed_key =
        vault.seal_engine_secret(id, EngineSecretKind::ApiKey, key.expose().as_bytes())?;
    let mut write = audit::begin_security(pool, mark).await?;
    // No conflict target: it covers both the one engine of an organization and the one engine
    // of the installation. A second process waits for the first transaction and then does
    // nothing.
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO engine (id, organization_id, kind, origin, address, events_url, \
                             api_key_enc, events_secret_enc, config_version, created_at, \
                             changed_at) \
         VALUES ($1, $2, 'waha', 'installation', $3, $4, $5, $6, 1, $7, $7) \
         ON CONFLICT DO NOTHING \
         RETURNING id",
    )
    .bind(id)
    .bind(organization.id())
    .bind(address.http().to_text())
    .bind(events_url.to_text())
    .bind(sealed_key)
    .bind(sealed_events)
    .bind(now)
    .fetch_optional(write.connection())
    .await?;
    let Some(id) = inserted else {
        write.rollback().await?;
        return Ok(Seeded::AlreadyThere);
    };
    write
        .record(
            now,
            &Event::new(Action::EngineConfigured, Actor::System)
                .in_organization(organization)
                .about(TargetKind::Engine, id)
                .with(
                    Details::new()
                        .flag("created", true)
                        .word("origin", EngineOrigin::Installation.as_str()),
                ),
        )
        .await?;
    write
        .record(
            now,
            &Event::new(Action::EngineKeyChanged, Actor::System)
                .in_organization(organization)
                .about(TargetKind::Engine, id)
                .with(
                    Details::new()
                        .flag("drawn", true)
                        .word("origin", EngineOrigin::Installation.as_str()),
                ),
        )
        .await?;
    notify_hub(write.connection(), organization, "engine", id).await?;
    write.commit().await?;
    Ok(Seeded::Created(id))
}

/// Notes that the engine began refusing the key of the hub, once per episode, and raises the
/// alert in the same transaction. Returns the origin of the row when this call opened the
/// episode, and `None` when one was already open or the row is gone. The episode ends at the
/// first answer that works ([`record_version`]) or at a change of the configuration. The
/// alert says only which engine and where it came from: never a key, a digest or an address.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn key_refused(
    pool: &PgPool,
    organization: ResolvedOrg,
    engine: Uuid,
    now: i64,
) -> Result<Option<EngineOrigin>, RepoError> {
    let mut transaction = pool.begin().await?;
    let origin: Option<String> = sqlx::query_scalar(
        "UPDATE engine SET key_refused_since = $3 \
         WHERE organization_id = $1 AND id = $2 AND key_refused_since IS NULL \
         RETURNING origin",
    )
    .bind(organization.id())
    .bind(engine)
    .bind(now)
    .fetch_optional(&mut *transaction)
    .await?;
    let origin = origin.map(|word| EngineOrigin::parse(&word).unwrap_or(EngineOrigin::Admin));
    if let Some(origin) = origin {
        raise(
            &mut transaction,
            organization,
            now,
            &NewAlert {
                kind: AlertKind::EngineKeyRefused,
                hermes: None,
                device: None,
                details: AlertDetails::new()
                    .id("engine", engine)
                    .word("origin", origin.as_str()),
            },
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(origin)
}
