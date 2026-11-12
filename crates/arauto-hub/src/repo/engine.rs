//! The engine of an organization (`docs/06-modelo-de-dados.md`, section 4): where it is, its
//! key and the secret of its events, sealed by the vault, and what the hub last saw of it.
//!
//! Every change is a security event, written through [`crate::audit::begin_security`]: the
//! address of the engine is where every number is reached, and its two secrets are who proves
//! itself to whom.

use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::{Admin, RepoError, notify_hub};
use crate::audit::{self, Action, Details, Event, TargetKind};
use crate::db::ResolvedOrg;
use crate::engine::{ApiKey, EngineAddress, EngineVersion, EventsSecret, EventsUrl};
use crate::mark::Mark;
use crate::vault::{EngineSecretKind, Vault};

/// The row of the engine.
#[derive(Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct EngineRow {
    /// The engine.
    pub id: Uuid,
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
        "SELECT id, address, events_url, api_key_enc, events_secret_enc, config_version, \
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
    let current: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, address, events_url FROM engine WHERE organization_id = $1 FOR UPDATE",
    )
    .bind(organization.id())
    .fetch_optional(write.connection())
    .await?;
    let address_text = address.http().to_text();
    let events_text = events_url.to_text();
    let outcome = match current {
        Some((id, stored, events)) if stored == address_text && events == events_text => {
            write.rollback().await?;
            return Ok(SetOutcome::Unchanged(id));
        }
        Some((id, _, _)) => {
            sqlx::query(
                "UPDATE engine SET address = $3, events_url = $4, \
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
                "INSERT INTO engine (id, organization_id, kind, address, events_url, \
                                     events_secret_enc, config_version, created_at, changed_at) \
                 VALUES ($1, $2, 'waha', $3, $4, $5, 1, $6, $6)",
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
    let (SetOutcome::Created(id) | SetOutcome::Changed(id) | SetOutcome::Unchanged(id)) = outcome;
    write
        .record(
            now,
            &Event::new(Action::EngineConfigured, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Engine, id)
                .with(Details::new().flag("created", matches!(outcome, SetOutcome::Created(_)))),
        )
        .await?;
    notify_hub(write.connection(), organization, "engine", id).await?;
    write.commit().await?;
    Ok(outcome)
}

/// Seals and stores the key of the API of the engine. `false` when there is no engine yet.
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
    let sealed = vault.seal_engine_secret(id, EngineSecretKind::ApiKey, key.expose().as_bytes())?;
    sqlx::query(
        "UPDATE engine SET api_key_enc = $3, config_version = config_version + 1, changed_at = $4 \
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
            &Event::new(Action::EngineKeyChanged, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Engine, id),
        )
        .await?;
    notify_hub(write.connection(), organization, "engine", id).await?;
    write.commit().await?;
    Ok(true)
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
        "UPDATE engine SET events_secret_enc = $3, config_version = config_version + 1, \
                changed_at = $4 \
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
        "UPDATE engine e SET version_seen = $3, version_tested = $4, last_ok_at = $5 \
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
