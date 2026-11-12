//! The engine of each organization, as the hub uses it: loaded from the database with its
//! secrets opened by the vault, kept while its configuration does not change, and rebuilt on
//! the first call after a change. Each call reads the version of the configuration (one query
//! by index), so a key or an address the admin changed holds at the next call.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_protocol::limits::{DAY_MS, HOUR_MS, MINUTE_MS, SECOND_MS};
use sqlx::postgres::PgPool;
use tokio::sync::Notify;
use uuid::Uuid;

use super::client::Deadlines;
use super::config::{EngineAddress, EventsSecret, EventsUrl};
use super::error::{Cause, EngineError};
use super::waha::Waha;
use super::{SessionConfig, WhatsappEngine};
use crate::db::ResolvedOrg;
use crate::repo::{self, RepoError};
use crate::vault::Vault;

/// The deadlines and ceilings of the engine and of its inbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSettings {
    /// Of each call to the engine.
    pub deadlines: Deadlines,
    /// Between two checks of the engine (`docs/10-maquinas-de-estado.md`, section 7).
    pub supervise_interval_ms: i64,
    /// Largest body of an event.
    pub event_body_max: usize,
    /// To read the body of an event.
    pub event_read_ms: i64,
    /// How far, before or after the time of the hub, the time of an event may be. Older ones
    /// are refused: a signed event recorded by someone cannot be played again after its row
    /// is purged.
    pub event_window_ms: i64,
    /// Most chats the catch-up looks at, per number.
    pub catch_up_chats: u32,
    /// Most messages the catch-up records, per chat.
    pub catch_up_messages: u32,
    /// How far back the catch-up goes, at most.
    pub catch_up_horizon_ms: i64,
    /// How long a QR or a code of a pairing is shown.
    pub pairing_ms: i64,
    /// How often a pairing asks the engine how the session is.
    pub pairing_poll_ms: i64,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            deadlines: Deadlines::default(),
            supervise_interval_ms: 5 * MINUTE_MS,
            event_body_max: 256 * 1024,
            event_read_ms: 5 * SECOND_MS,
            event_window_ms: 12 * HOUR_MS,
            catch_up_chats: 50,
            catch_up_messages: 100,
            catch_up_horizon_ms: DAY_MS,
            pairing_ms: 3 * MINUTE_MS,
            pairing_poll_ms: SECOND_MS,
        }
    }
}

/// The engine of an organization, ready.
pub struct LoadedEngine {
    /// The row of the engine.
    pub id: Uuid,
    /// The version of its configuration this was built from.
    pub config_version: i64,
    /// Where it reaches the hub.
    pub events_url: EventsUrl,
    /// The secret of its events.
    pub events: EventsSecret,
    /// The client.
    pub client: Arc<dyn WhatsappEngine>,
}

impl std::fmt::Debug for LoadedEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedEngine")
            .field("id", &self.id)
            .field("config_version", &self.config_version)
            .finish_non_exhaustive()
    }
}

impl LoadedEngine {
    /// The configuration of a session of this engine.
    #[must_use]
    pub fn session_config(&self) -> SessionConfig<'_> {
        SessionConfig {
            events_url: self.events_url.inbox(self.id),
            events_secret: &self.events,
        }
    }
}

/// The engines the hub uses.
#[derive(Debug, Default)]
pub struct Engines {
    settings: EngineSettings,
    cache: Mutex<HashMap<Uuid, Arc<LoadedEngine>>>,
    supervise_now: Notify,
}

impl Engines {
    /// No engine loaded yet.
    #[must_use]
    pub fn new(settings: EngineSettings) -> Self {
        Self {
            settings,
            cache: Mutex::new(HashMap::new()),
            supervise_now: Notify::new(),
        }
    }

    /// The settings.
    #[must_use]
    pub fn settings(&self) -> &EngineSettings {
        &self.settings
    }

    /// The engine of `organization`, built again when its configuration changed.
    ///
    /// # Errors
    /// [`EngineError::Unavailable`] with [`Cause::NotConfigured`] when there is no engine, it
    /// has no key yet, or its stored configuration does not open.
    pub async fn of(
        &self,
        pool: &PgPool,
        vault: &Vault,
        organization: ResolvedOrg,
    ) -> Result<Arc<LoadedEngine>, EngineError> {
        let not_configured = EngineError::Unavailable(Cause::NotConfigured);
        let current = repo::engine::current_version(pool, organization)
            .await
            .map_err(|_| EngineError::Unavailable(Cause::Connect))?;
        let Some((id, version)) = current else {
            return Err(not_configured);
        };
        if let Some(loaded) = self.cached(organization)
            && loaded.id == id
            && loaded.config_version == version
        {
            return Ok(loaded);
        }
        let row = repo::engine::get(pool, organization)
            .await
            .map_err(|_| EngineError::Unavailable(Cause::Connect))?
            .ok_or(not_configured.clone())?;
        let loaded = Arc::new(
            self.build(&row, vault)
                .map_err(|_| not_configured.clone())?,
        );
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(organization.id(), Arc::clone(&loaded));
        }
        Ok(loaded)
    }

    fn cached(&self, organization: ResolvedOrg) -> Option<Arc<LoadedEngine>> {
        self.cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&organization.id()).cloned())
    }

    fn build(
        &self,
        row: &repo::engine::EngineRow,
        vault: &Vault,
    ) -> Result<LoadedEngine, RepoError> {
        let secrets = row.secrets(vault)?;
        let key = secrets
            .key
            .ok_or(RepoError::Inconsistent("the engine has no key yet"))?;
        let address = EngineAddress::parse(&row.address)
            .map_err(|_| RepoError::Inconsistent("the address of the engine"))?;
        let events_url = EventsUrl::parse(&row.events_url)
            .map_err(|_| RepoError::Inconsistent("the address of the events"))?;
        Ok(LoadedEngine {
            id: row.id,
            config_version: row.config_version,
            events_url,
            events: secrets.events,
            client: Arc::new(Waha::new(address, Some(key), self.settings.deadlines)),
        })
    }

    /// Asks the supervision to run now, instead of at its next turn.
    pub fn supervise_soon(&self) {
        self.supervise_now.notify_one();
    }

    pub(crate) async fn supervise_asked(&self) {
        self.supervise_now.notified().await;
    }

    pub(crate) fn interval(&self) -> Duration {
        Duration::from_millis(u64::try_from(self.settings.supervise_interval_ms).unwrap_or(0))
    }
}
