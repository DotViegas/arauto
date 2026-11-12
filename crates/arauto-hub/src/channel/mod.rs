//! The channel: the port of the connectors, from the accept loop to the session
//! (`docs/14-protocolo-do-canal.md`; `docs/15-mvp.md`, sections 2 and 3).
//!
//! ```text
//! TCP ── hyper (5 s for the headers) ── GET /v1/channel ── ceiling (503, 429)
//!   └─ websocket (binary, 65,535 bytes, no compression)
//!        └─ Noise XK (48, 48, 64 bytes; 10 s)            handshake
//!             └─ Link: reader + writer, two queues      link
//!                  └─ hello / hello_ok                  hello
//!                       ├─ enroll                       enrollment
//!                       ├─ channel                      exchange, then session
//!                       └─ leave                        hello
//! ```
//!
//! What `enroll` and `channel` do is behind [`Purposes`]: [`Served`] is the code of the hub,
//! and the tests of the transport bring their own. Four tasks of the whole hub run beside the
//! connections: the sweep, every 2 seconds, the listener of the notices of the admin command,
//! the purge of what passed its retention, and the supervision of the engine. The internal
//! port also takes the events of the engine (`crate::engine`).
//!
//! [`serve`] is what `arauto-hub serve` runs; [`start`] is the same start, handing back the
//! running hub, for the tests and for whoever needs the addresses it bound.

pub mod admission;
mod budget;
mod client_ip;
mod enrollment;
mod exchange;
mod handshake;
mod hello;
pub mod http;
pub mod link;
mod registry;
mod session;
pub mod settings;
mod tasks;

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use arauto_protocol::clock::Clock;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{Hello, HelloOk};
use arauto_protocol::noise::{PublicKey, StaticKeypair};
use dashmap::DashMap;
use sqlx::postgres::PgPool;
use tokio::net::TcpListener;
use tokio::sync::{Notify, watch};
use tokio::task::JoinHandle;
use uuid::Uuid;

pub use http::{BadDownloadName, DOWNLOAD_PREFIX, Downloads, HEALTH_PATH};
pub use link::{Link, LinkClosed, LinkHandle, LinkInfo, Phase, RecvError, SendError};
pub use settings::ChannelSettings;

use self::admission::Admission;
use self::budget::HubBudget;
use self::http::Port;
use self::registry::{Registry, Status};
use self::settings::duration;
use crate::admin::{self, AdminError};
use crate::audit::AuditError;
use crate::config::{ClientIpSource, DatabaseUrl, ServeConfig};
use crate::db::{self, DbError, PoolOptions, ResolvedOrg, SystemTask};
use crate::engine::{self, Engines};
use crate::mark::Mark;
use crate::repo::enroll::RefusedEnrollments;
use crate::repo::{self, RepoError};
use crate::restore::{self, EnterOutcome, Evidence, RestoreError, Standing};
use crate::vault::{OrganizationKey, Vault, VaultError};
use crate::whatsapp::SendCeiling;

/// What the code of each purpose does once `hello_ok` is sent.
///
/// The connection is closed when the returned future ends: what was queued is sent first.
/// The transport keeps every limit of the phase the connection is in, the heartbeat and the
/// stop of the hub; the code of a purpose keeps the order of the messages and their
/// deadlines.
pub trait Purposes: Send + Sync + 'static {
    /// `purpose: enroll`. Called only with a version in common: a connector too old was
    /// already closed with AR-401.
    fn enroll(&self, hub: Arc<Hub>, link: Link, hello: Hello) -> impl Future<Output = ()> + Send;

    /// `purpose: channel`. `answered` says whether the connector is too old
    /// ([`HelloOk::Outdated`]): then only the presentation without exchange may follow.
    fn channel(
        &self,
        hub: Arc<Hub>,
        link: Link,
        hello: Hello,
        answered: HelloOk,
    ) -> impl Future<Output = ()> + Send;
}

/// The purposes of the hub: the enrollment (`docs/14-protocolo-do-canal.md`, section 5.2) and
/// the channel, with the swap and the session (sections 5.3 and 6).
#[derive(Debug, Clone, Copy, Default)]
pub struct Served;

impl Purposes for Served {
    async fn enroll(&self, hub: Arc<Hub>, link: Link, hello: Hello) {
        enrollment::run(hub, link, hello).await;
    }

    async fn channel(&self, hub: Arc<Hub>, link: Link, hello: Hello, answered: HelloOk) {
        exchange::run(hub, link, hello, answered).await;
    }
}

/// What the start of the hub could not do.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    /// The database refused, is not durable, or is not at the version of this program.
    #[error(transparent)]
    Db(#[from] DbError),
    /// The audit mark or the restored mode.
    #[error(transparent)]
    Restore(#[from] RestoreError),
    /// A repository function failed.
    #[error(transparent)]
    Repo(#[from] RepoError),
    /// The key of the hub.
    #[error(transparent)]
    Admin(#[from] AdminError),
    /// The master key.
    #[error(transparent)]
    Vault(#[from] VaultError),
    /// `init` has not run on this database.
    #[error("the hub is not installed: run `arauto-hub init`")]
    NotInstalled,
    /// The database still reads as restored after the restored mode was turned on.
    #[error("the database is still behind the audit mark after the restored mode was turned on")]
    StillRestored,
    /// A port could not be opened.
    #[error("could not listen on {address}: {source}")]
    Listen {
        /// The address of the port.
        address: SocketAddr,
        /// What the system said.
        source: std::io::Error,
    },
    /// A blocking task of the start did not return.
    #[error("a task of the start did not return")]
    Task,
}

/// The tasks of the hub that are still running, for the stop to wait for them.
#[derive(Debug, Default)]
pub(crate) struct TaskCount {
    running: AtomicUsize,
    idle: Notify,
}

impl TaskCount {
    pub(crate) fn guard(self: &Arc<Self>) -> TaskGuard {
        self.running.fetch_add(1, Ordering::SeqCst);
        TaskGuard {
            count: Arc::clone(self),
        }
    }

    async fn idle(&self) {
        loop {
            let notified = self.idle.notified();
            if self.running.load(Ordering::SeqCst) == 0 {
                return;
            }
            notified.await;
        }
    }
}

/// One running task. Dropped, the task is over.
#[derive(Debug)]
pub(crate) struct TaskGuard {
    count: Arc<TaskCount>,
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        if self.count.running.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.count.idle.notify_waiters();
        }
    }
}

/// What every connection of the hub shares: the database, the mark, the vault, the clock, the
/// settings, the key of the hub, the live connections and the record of the channel
/// connections of each device.
pub struct Hub {
    pool: PgPool,
    /// The sweep reads through this pool, apart from the one of the connections.
    sweep_pool: PgPool,
    database_url: DatabaseUrl,
    mark: Mark,
    vault: Vault,
    clock: Arc<dyn Clock>,
    settings: ChannelSettings,
    keypair: StaticKeypair,
    client_ip: ClientIpSource,
    downloads: Downloads,
    admission: Arc<Admission>,
    budget: Arc<HubBudget>,
    links: DashMap<u64, LinkHandle>,
    serial: AtomicU64,
    stopping: watch::Sender<bool>,
    tasks: Arc<TaskCount>,
    registry: Arc<Registry>,
    refusals: RefusedEnrollments,
    keys: DashMap<Uuid, Arc<OrganizationKey>>,
    sweep_now: Notify,
    engines: Engines,
    send_ceiling: SendCeiling,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub")
            .field("public_key", self.keypair.public())
            .field("links", &self.links.len())
            .finish_non_exhaustive()
    }
}

impl Hub {
    /// The pool of database connections.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub(crate) fn sweep_pool(&self) -> &PgPool {
        &self.sweep_pool
    }

    /// The audit mark.
    #[must_use]
    pub fn mark(&self) -> &Mark {
        &self.mark
    }

    /// The vault: the organization keys open here.
    #[must_use]
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// The time of the hub, in milliseconds.
    #[must_use]
    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    /// Deadlines, queues and ceilings.
    #[must_use]
    pub fn settings(&self) -> &ChannelSettings {
        &self.settings
    }

    /// The public key of the hub.
    #[must_use]
    pub fn public_key(&self) -> &PublicKey {
        self.keypair.public()
    }

    pub(crate) fn keypair(&self) -> &StaticKeypair {
        &self.keypair
    }

    pub(crate) fn client_ip_source(&self) -> &ClientIpSource {
        &self.client_ip
    }

    pub(crate) fn downloads(&self) -> &Downloads {
        &self.downloads
    }

    pub(crate) fn admission(&self) -> &Arc<Admission> {
        &self.admission
    }

    pub(crate) fn budget(&self) -> &Arc<HubBudget> {
        &self.budget
    }

    pub(crate) fn tasks(&self) -> &Arc<TaskCount> {
        &self.tasks
    }

    pub(crate) fn next_serial(&self) -> u64 {
        self.serial.fetch_add(1, Ordering::SeqCst).saturating_add(1)
    }

    pub(crate) fn register(&self, serial: u64, handle: LinkHandle) {
        self.links.insert(serial, handle);
    }

    pub(crate) fn unregister(&self, serial: u64) {
        self.links.remove(&serial);
    }

    /// Every live connection, in any phase.
    #[must_use]
    pub fn links(&self) -> Vec<LinkHandle> {
        self.links
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Sends `closing` with `code` to every live connection.
    pub fn close_all(&self, code: ErrorCode) {
        for link in self.links() {
            link.close(code);
        }
    }

    /// Whether the hub is stopping.
    #[must_use]
    pub fn is_stopping(&self) -> bool {
        *self.stopping.borrow()
    }

    pub(crate) fn stopping(&self) -> watch::Receiver<bool> {
        self.stopping.subscribe()
    }

    /// Turns the restored mode on with `evidence` (C11), wherever the proof came from: a
    /// presentation, a request to leave, the sweep. Then every session and every waiting
    /// connection is told the state its device is in now (a suspended one gets AR-506), and
    /// every other connection gets AR-205: each connector comes back and is told the state of
    /// its device, which the mode just changed. When the mode cannot be turned on, the
    /// evidence comes back at the next presentation or sweep.
    pub async fn enter_restored_mode(&self, evidence: Evidence) {
        match restore::enter(&self.pool, &self.mark, self.now_ms(), evidence).await {
            Ok(EnterOutcome::Entered(entered)) => {
                tracing::warn!(
                    seq = entered.seq,
                    suspended = entered.suspended,
                    "the database came back from a copy: the restored mode is on"
                );
                self.close_by_standing().await;
            }
            Ok(EnterOutcome::AlreadyHandled) => {}
            Err(error) => tracing::error!(%error, "the restored mode could not be turned on"),
        }
    }

    /// Closes the sessions and the waiting connections with the code of the state of their
    /// device, and every other connection with AR-205.
    async fn close_by_standing(&self) {
        tasks::check_standings(self).await;
        let established: Vec<u64> = self
            .registry
            .all()
            .iter()
            .filter(|entry| entry.status().is_established())
            .map(|entry| entry.serial())
            .collect();
        for link in self.links() {
            if !established.contains(&link.info().serial) {
                link.close(ErrorCode::Ar205);
            }
        }
    }

    /// The engine of each organization.
    #[must_use]
    pub fn engines(&self) -> &Engines {
        &self.engines
    }

    /// The send limit of every Hermes on every number.
    pub(crate) fn send_ceiling(&self) -> &SendCeiling {
        &self.send_ceiling
    }

    pub(crate) fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    pub(crate) fn refusals(&self) -> &RefusedEnrollments {
        &self.refusals
    }

    pub(crate) fn database_url(&self) -> &DatabaseUrl {
        &self.database_url
    }

    /// Asks the sweep to run now, instead of at its next turn.
    pub(crate) fn sweep_soon(&self) {
        self.sweep_now.notify_one();
    }

    pub(crate) async fn sweep_asked(&self) {
        self.sweep_now.notified().await;
    }

    /// The keys of `organization`, opened once with the master key and kept for as long as
    /// the hub runs.
    pub(crate) async fn organization_key(
        &self,
        organization: ResolvedOrg,
    ) -> Result<Arc<OrganizationKey>, RepoError> {
        if let Some(key) = self.keys.get(&organization.id()) {
            return Ok(Arc::clone(key.value()));
        }
        let key =
            Arc::new(repo::device::organization_key(&self.pool, organization, &self.vault).await?);
        self.keys.insert(organization.id(), Arc::clone(&key));
        Ok(key)
    }

    /// A security write failed: the database came back from a copy (the mode is turned on),
    /// the mark did not follow a committed write (the admin is told), or the database did not
    /// answer. The caller closes the connection.
    pub(crate) async fn security_write_failed(
        &self,
        organization: ResolvedOrg,
        device: Option<Uuid>,
        error: RepoError,
    ) {
        match error {
            RepoError::Restored(evidence) => self.enter_restored_mode(evidence).await,
            RepoError::Audit(AuditError::MarkNotAdvanced { seq, .. }) => {
                tracing::error!(
                    seq,
                    "a security event is committed, but the audit mark did not advance"
                );
                let raised = repo::alert::mark_not_advanced(
                    &self.pool,
                    organization,
                    device,
                    seq,
                    self.now_ms(),
                )
                .await;
                if let Err(error) = raised {
                    tracing::error!(%error, "the alert of the audit mark could not be written");
                }
            }
            other => tracing::error!(error = %other, "a write of the channel failed"),
        }
    }

    /// How many channel connections of each device are in session right now.
    #[must_use]
    pub fn sessions_by_device(&self) -> HashMap<Uuid, usize> {
        let mut found = HashMap::new();
        for entry in self.registry.all() {
            if entry.status() == Status::Session
                && let Some(place) = entry.place()
            {
                let count = found.entry(place.device).or_insert(0usize);
                *count = count.saturating_add(1);
            }
        }
        found
    }
}

/// What [`start`] needs besides the environment.
#[derive(Debug, Clone)]
pub struct Options<P> {
    /// Deadlines, queues and ceilings.
    pub settings: ChannelSettings,
    /// The files of `GET /v1/download/{file}`.
    pub downloads: Downloads,
    /// The code of each purpose.
    pub purposes: P,
}

impl Default for Options<Served> {
    fn default() -> Self {
        Self {
            settings: ChannelSettings::default(),
            downloads: Downloads::empty(),
            purposes: Served,
        }
    }
}

/// A hub that accepts connections.
#[derive(Debug)]
pub struct Running {
    hub: Arc<Hub>,
    connectors: SocketAddr,
    internal: SocketAddr,
    accept: Vec<JoinHandle<()>>,
    /// The sweep and the listener: aborted at the stop.
    background: Vec<JoinHandle<()>>,
    /// The purge: it sees the stop between two batches, and the stop waits for it.
    purger: JoinHandle<()>,
}

impl Running {
    /// The address the port of the connectors is bound to.
    #[must_use]
    pub fn connectors_addr(&self) -> SocketAddr {
        self.connectors
    }

    /// The address the internal port is bound to.
    #[must_use]
    pub fn internal_addr(&self) -> SocketAddr {
        self.internal
    }

    /// What the connections share.
    #[must_use]
    pub fn hub(&self) -> &Arc<Hub> {
        &self.hub
    }

    /// The orderly stop (`docs/14-protocolo-do-canal.md`, section 7): stop accepting; send
    /// `closing` with AR-205 to every connection that is not serving a session; let every
    /// session answer its calls in progress and then close itself with AR-205; wait up to the
    /// grace of the settings (8 seconds) for all of them to end. What is still open then is
    /// closed with AR-205, with the time of a linger for the `closing` to leave.
    ///
    /// The purge stops between two batches, within the same grace: a batch it began ends,
    /// and once the stop returns no batch of it runs. One that does not end in the grace (a
    /// database that does not answer) is aborted; the database does a batch whole or not at
    /// all.
    pub async fn stop(self) {
        let Self {
            hub,
            accept,
            background,
            purger,
            ..
        } = self;
        hub.stopping.send_replace(true);
        for accept in accept {
            let _ = accept.await;
        }
        for task in background {
            task.abort();
            let _ = task.await;
        }
        let grace = duration(hub.settings.deadlines.shutdown_grace_ms);
        let sessions = async {
            // A session of the hub sees the stop itself and drains; a connection that becomes
            // a session after this list was read was closed here before it did.
            let draining: Vec<u64> = hub
                .registry
                .all()
                .iter()
                .filter(|entry| entry.status() == Status::Session)
                .map(|entry| entry.serial())
                .collect();
            for link in hub.links() {
                if !draining.contains(&link.info().serial) {
                    link.close(ErrorCode::Ar205);
                }
            }
            if tokio::time::timeout(grace, hub.tasks.idle()).await.is_err() {
                hub.close_all(ErrorCode::Ar205);
                let linger = duration(hub.settings.linger_ms.saturating_mul(2));
                let _ = tokio::time::timeout(linger, hub.tasks.idle()).await;
            }
        };
        tokio::join!(sessions, end_within(purger, grace));
    }
}

/// Waits for `task` to end by itself, up to `within`; past that, aborts it.
async fn end_within(mut task: JoinHandle<()>, within: std::time::Duration) {
    if tokio::time::timeout(within, &mut task).await.is_err() {
        task.abort();
        let _ = task.await;
    }
}

/// Starts the hub: checks the database and the mark, turns the restored mode on when the
/// database came back from a copy, closes what the previous run left open, opens the key of
/// the hub and only then listens on the two ports.
///
/// # Errors
/// [`ServeError`]: the hub does not start.
pub async fn start<P: Purposes>(
    config: &ServeConfig,
    options: Options<P>,
    clock: Arc<dyn Clock>,
) -> Result<Running, ServeError> {
    let settings = options.settings;
    let pool = db::connect(&config.storage.database_url, settings.pool).await?;
    let sweep_pool = db::connect(
        &config.storage.database_url,
        PoolOptions {
            size: settings::SWEEP_POOL_SIZE,
            acquire_timeout: settings.pool.acquire_timeout,
        },
    )
    .await?;
    // A commit the server may lose would later read as a restored database.
    db::check_durability(&pool, config.storage.allow_unsafe_db).await?;
    db::require_current_schema(&pool, &db::MIGRATOR).await?;
    let mark = Mark::new(&config.storage.data_dir);
    recover(&pool, &mark, clock.now_ms()).await?;
    close_left_open(&pool, clock.now_ms()).await?;
    let data_dir = config.storage.data_dir.clone();
    let vault = tokio::task::spawn_blocking(move || Vault::open(&data_dir))
        .await
        .map_err(|_| ServeError::Task)??;
    let keypair = admin::hub_keypair(&pool, &vault).await?;
    let connectors = listen(config.listen).await?;
    let internal = listen(config.listen_internal).await?;
    let local = |listener: &TcpListener, address| {
        listener
            .local_addr()
            .map_err(|source| ServeError::Listen { address, source })
    };
    let connectors_addr = local(&connectors, config.listen)?;
    let internal_addr = local(&internal, config.listen_internal)?;
    let hub = Arc::new(Hub {
        pool,
        sweep_pool,
        database_url: config.storage.database_url.clone(),
        mark,
        vault,
        clock,
        keypair,
        client_ip: config.client_ip.clone(),
        downloads: options.downloads,
        admission: Admission::new(
            settings.without_session_max,
            settings.without_session_per_origin_max,
        ),
        budget: Arc::new(HubBudget::new(settings.reassembly_budget_bytes)),
        links: DashMap::new(),
        serial: AtomicU64::new(0),
        stopping: watch::Sender::new(false),
        tasks: Arc::new(TaskCount::default()),
        registry: Arc::new(Registry::default()),
        refusals: RefusedEnrollments::new(),
        keys: DashMap::new(),
        sweep_now: Notify::new(),
        engines: Engines::new(settings.engine.clone()),
        send_ceiling: SendCeiling::new(
            settings.sends_per_minute,
            arauto_protocol::limits::WHATSAPP_SEND_WINDOW_MS,
        ),
        settings,
    });
    let purposes = Arc::new(options.purposes);
    let accept = vec![
        tokio::spawn(http::accept(
            connectors,
            Port::Connectors,
            Arc::clone(&hub),
            Arc::clone(&purposes),
        )),
        tokio::spawn(http::accept(
            internal,
            Port::Internal,
            Arc::clone(&hub),
            purposes,
        )),
    ];
    let background = vec![
        tokio::spawn(tasks::sweeper(Arc::clone(&hub))),
        tokio::spawn(tasks::listener(Arc::clone(&hub))),
        tokio::spawn(engine::supervisor::run(Arc::clone(&hub))),
    ];
    let purger = tokio::spawn(tasks::purger(Arc::clone(&hub)));
    Ok(Running {
        hub,
        connectors: connectors_addr,
        internal: internal_addr,
        accept,
        background,
        purger,
    })
}

/// `arauto-hub serve`: starts the hub, runs until `shutdown` resolves, and stops in order.
///
/// # Errors
/// [`ServeError`]: the hub did not start.
pub async fn serve(
    config: &ServeConfig,
    settings: ChannelSettings,
    clock: Arc<dyn Clock>,
    shutdown: impl Future<Output = ()>,
) -> Result<(), ServeError> {
    let options = Options {
        settings,
        ..Options::default()
    };
    let running = start(config, options, clock).await?;
    shutdown.await;
    running.stop().await;
    Ok(())
}

async fn listen(address: SocketAddr) -> Result<TcpListener, ServeError> {
    TcpListener::bind(address)
        .await
        .map_err(|source| ServeError::Listen { address, source })
}

/// Turns the restored mode on when the database came back from a copy, before any connection
/// is accepted (C11). A mode turned on for a mark that was absent may only have created the
/// mark again: the standing is read once more until it holds.
async fn recover(pool: &PgPool, mark: &Mark, now: i64) -> Result<(), ServeError> {
    for _ in 0..3 {
        match restore::check(pool, mark).await? {
            Standing::NotInstalled => return Err(ServeError::NotInstalled),
            Standing::Consistent => return Ok(()),
            Standing::Restored(evidence) => {
                if let EnterOutcome::Entered(entered) =
                    restore::enter(pool, mark, now, evidence).await?
                {
                    tracing::warn!(
                        seq = entered.seq,
                        suspended = entered.suspended,
                        "the database came back from a copy: the restored mode is on"
                    );
                }
            }
        }
    }
    Err(ServeError::StillRestored)
}

/// Closes the channel connections a hub that stopped without warning left open, in every
/// organization.
async fn close_left_open(pool: &PgPool, now: i64) -> Result<(), ServeError> {
    for organization in db::organizations_for_system_task(pool, SystemTask::Start).await? {
        repo::connection::close_left_open(pool, organization, now).await?;
    }
    Ok(())
}
