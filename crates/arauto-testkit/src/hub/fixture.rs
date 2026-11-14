//! A hub of its own, with the purposes of the hub, an admin session on the same database, and
//! the steps of the admin as functions: create a Hermes, issue the invite, enroll, approve
//! with the typed code. What the tests of the channel start from, on the side of the hub and on
//! the side of the connector.
// Test support: a failure here is the failure of the test that called it, with the reason.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use arauto_hub::admin::Session;
use arauto_hub::channel::{self, ChannelSettings, Downloads, Hub, Options, Running, Served};
use arauto_hub::config::{ClientIpSource, PublicUrl};
use arauto_hub::db::{self, PoolOptions};
use arauto_hub::join_page::PageHandle;
use arauto_hub::mark::Mark;
use arauto_hub::repo::approval::{ApproveOutcome, Recognized};
use arauto_protocol::clock::{Clock, SystemClock};
use arauto_protocol::invite::Invite;
use arauto_protocol::message::{CheckNonce, DeclaredText, Message, MessageType, Present, Purpose};
use arauto_protocol::noise::PublicKey;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use super::support::{self, Installation};
use crate::connector::TestConnector;
use crate::device::{self, Device, Enrolled, Faults, SessionClient};
use crate::{Ended, KitError};

/// Patience for anything the hub does at once.
pub const PATIENCE: Duration = Duration::from_secs(15);

static NAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A name of Hermes nobody else in this process uses.
pub fn name() -> String {
    format!(
        "s{}",
        NAMES.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    )
}

/// A hub of its own: its database, its data folder, its clock.
pub struct Fixture {
    /// The database and the data folder.
    pub installation: Installation,
    /// A pool for the assertions of the test.
    pub pool: PgPool,
    /// The session of the local admin.
    pub admin: Session,
    /// The hub, while it runs.
    pub running: Option<Running>,
    /// The settings the hub starts with.
    pub settings: ChannelSettings,
    /// The clock of the hub.
    pub clock: Arc<dyn Clock>,
    /// The files the hub serves for download, read when it starts.
    pub downloads: Downloads,
    /// How the hub reads the address of a client, when it starts.
    pub client_ip: ClientIpSource,
}

/// The settings of the served tests: the documented values, a sweep every 200 ms, and the
/// deadlines no test measures stretched.
#[must_use]
pub fn settings() -> ChannelSettings {
    let mut settings = support::settings();
    settings.sweep_interval_ms = 200;
    settings.pool = PoolOptions {
        size: 8,
        acquire_timeout: Duration::from_secs(10),
    };
    settings
}

async fn pool_of(installation: &Installation) -> PgPool {
    db::connect(
        &installation.url,
        PoolOptions {
            size: 3,
            acquire_timeout: Duration::from_secs(10),
        },
    )
    .await
    .unwrap()
}

impl Fixture {
    /// A hub with these settings and the clock of the system.
    pub async fn new(settings: ChannelSettings) -> Self {
        Self::with_clock(settings, Arc::new(SystemClock)).await
    }

    /// A hub with these settings and this clock.
    pub async fn with_clock(settings: ChannelSettings, clock: Arc<dyn Clock>) -> Self {
        let installation = support::install().await;
        let mut fixture = Self::stopped(installation, settings, clock).await;
        fixture.start().await;
        fixture
    }

    /// The installation, the pool and the admin, without a hub running.
    pub async fn stopped(
        installation: Installation,
        settings: ChannelSettings,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let pool = pool_of(&installation).await;
        let admin = Session::open(
            pool_of(&installation).await,
            Mark::new(&installation.data_dir),
            DeclaredText::lossy("tester"),
        )
        .await
        .unwrap();
        Self {
            installation,
            pool,
            admin,
            running: None,
            settings,
            clock,
            downloads: Downloads::empty(),
            client_ip: ClientIpSource::Socket,
        }
    }

    /// Starts the hub.
    pub async fn start(&mut self) {
        let running = channel::start(
            &support::config(&self.installation, self.client_ip.clone()),
            Options {
                settings: self.settings.clone(),
                downloads: self.downloads.clone(),
                purposes: Served,
            },
            Arc::clone(&self.clock),
        )
        .await
        .unwrap();
        self.running = Some(running);
    }

    /// Stops the hub in order.
    pub async fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop().await;
        }
    }

    /// Starts the hub with the port of the connectors at `connectors`: after a stop, the
    /// connectors find it where it was. The port may take a moment to be free again.
    pub async fn start_at(&mut self, connectors: SocketAddr) {
        let mut config = support::config(&self.installation, self.client_ip.clone());
        config.listen = connectors;
        let mut tries = 0u32;
        loop {
            let started = channel::start(
                &config,
                Options {
                    settings: self.settings.clone(),
                    downloads: self.downloads.clone(),
                    purposes: Served,
                },
                Arc::clone(&self.clock),
            )
            .await;
            match started {
                Ok(running) => {
                    self.running = Some(running);
                    return;
                }
                Err(error) if tries < 100 => {
                    tries += 1;
                    tracing::debug!(%error, "the hub could not start yet");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => panic!("the hub did not start again at {connectors}: {error}"),
            }
        }
    }

    /// The running hub.
    pub fn running(&self) -> &Running {
        self.running.as_ref().expect("the hub runs")
    }

    /// The address of the port of the connectors.
    pub fn addr(&self) -> SocketAddr {
        self.running().connectors_addr()
    }

    /// The address a connector is given in the block: the websocket of the channel.
    pub fn channel_url(&self) -> String {
        format!("ws://{}/v1/channel", self.addr())
    }

    /// The public key of the hub, as the block writes it.
    pub fn hub_key_text(&self) -> String {
        self.installation.hub_key.to_text()
    }

    /// What the connections share.
    pub fn hub(&self) -> &Arc<Hub> {
        self.running().hub()
    }

    /// The time of the hub.
    pub fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    /// A new device of this hub.
    pub fn device(&self) -> Device {
        Device::new(self.addr(), self.installation.hub_key).unwrap()
    }

    /// A new Hermes line.
    pub async fn hermes(&self) -> String {
        let name = name();
        self.admin.hermes_add(&name, self.now()).await.unwrap();
        name
    }

    /// `admin invite NAME`.
    pub async fn invite(&self, name: &str) -> Invite {
        self.invite_with_page(name).await.0
    }

    /// `admin invite NAME`, with the handle of the instruction page that came with the invite.
    pub async fn invite_with_page(&self, name: &str) -> (Invite, PageHandle) {
        let public = PublicUrl::parse("http://127.0.0.1:1").unwrap();
        let block = self.admin.invite(name, &public, self.now()).await.unwrap();
        let issued = *block.issued;
        (issued.invite, issued.page)
    }

    /// The device enrolls with a new invite of `name`, and returns the code it would show.
    pub async fn enroll(&self, name: &str, device: &Device) -> String {
        let invite = self.invite(name).await;
        match device.enroll(&invite, device::declared()).await.unwrap() {
            Enrolled::Code(code) => code.to_string(),
            Enrolled::Refused(code) => panic!("the enrollment was refused with {code}"),
        }
    }

    /// `admin approve NAME CODE`, answering the question of the last use when it comes.
    pub async fn approve(&self, name: &str, code: &str) -> ApproveOutcome {
        let mut outcome = self
            .admin
            .approve(name, code, None, self.now())
            .await
            .unwrap();
        // A device that is running keeps using its connection: the last use shown may no
        // longer be the last when the answer comes, and the hub shows the new one. A person
        // would answer again; so does this.
        for _ in 0..10 {
            let ApproveOutcome::NeedsRecognition(shown) = outcome else {
                break;
            };
            outcome = self
                .admin
                .approve(
                    name,
                    code,
                    Some(Recognized {
                        answer: arauto_hub::repo::approval::Recognition::Recognized,
                        shown,
                    }),
                    self.now(),
                )
                .await
                .unwrap();
        }
        outcome
    }

    /// A new Hermes whose device enrolled and was approved, at generation zero.
    pub async fn approved(&self) -> (String, Device) {
        let name = self.hermes().await;
        let device = self.device();
        let code = self.enroll(&name, &device).await;
        assert!(matches!(
            self.approve(&name, &code).await,
            ApproveOutcome::Approved(_)
        ));
        (name, device)
    }

    /// A new Hermes connected end to end: enrolled, approved, and in session.
    pub async fn connected(&self) -> (String, SessionClient) {
        let (name, device) = self.approved().await;
        let session = device
            .channel(Faults::default())
            .await
            .unwrap()
            .session()
            .unwrap();
        (name, session)
    }

    /// The row of a device: state and reason.
    pub async fn state_of(&self, device: &Device) -> (String, Option<String>) {
        self.state_of_key(device.public()).await
    }

    /// The row of the device of this identity: state and reason.
    pub async fn state_of_key(&self, identity: &PublicKey) -> (String, Option<String>) {
        sqlx::query_as("SELECT state, state_reason FROM device WHERE public_key = $1")
            .bind(identity.as_bytes().as_slice())
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// Whether the hub has a row for this identity.
    pub async fn knows(&self, identity: &PublicKey) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM device WHERE public_key = $1")
            .bind(identity.as_bytes().as_slice())
            .fetch_one(&self.pool)
            .await
            .unwrap()
            > 0
    }

    /// The identifier of a device.
    pub async fn id_of(&self, device: &Device) -> Uuid {
        self.id_of_key(device.public()).await
    }

    /// The identifier of the device of this identity.
    pub async fn id_of_key(&self, identity: &PublicKey) -> Uuid {
        sqlx::query_scalar("SELECT id FROM device WHERE public_key = $1")
            .bind(identity.as_bytes().as_slice())
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// How many alerts of `kind` were raised about this device.
    pub async fn alerts(&self, kind: &str, device: &Device) -> i64 {
        self.alerts_of_key(kind, device.public()).await
    }

    /// How many alerts of `kind` were raised about the device of this identity.
    pub async fn alerts_of_key(&self, kind: &str, identity: &PublicKey) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM alert a JOIN device d ON d.id = a.device_id \
             WHERE a.kind = $1 AND d.public_key = $2",
        )
        .bind(kind)
        .bind(identity.as_bytes().as_slice())
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// How many connections of the channel of this identity the hub holds right now.
    pub fn links_of(&self, identity: &PublicKey) -> usize {
        self.hub()
            .links()
            .iter()
            .filter(|link| link.info().identity == *identity)
            .count()
    }

    /// Whether the restored mode is on.
    pub async fn restored_mode(&self) -> bool {
        arauto_hub::restore::installation(&self.pool)
            .await
            .unwrap()
            .unwrap()
            .restored_mode
    }

    /// A connection of `device` taken by hand up to the `check` of the hub: `hello`, `state`,
    /// `present` with what is on its disk.
    pub async fn up_to_the_check(&self, device: &Device) -> (TestConnector, CheckNonce) {
        let mut connector =
            TestConnector::connect(self.addr(), device.identity(), &self.installation.hub_key)
                .await
                .unwrap();
        connector.hello(Purpose::Channel).await.unwrap();
        let Message::State(_) = connector.recv(PATIENCE).await.unwrap() else {
            panic!("no state");
        };
        connector
            .send(&Message::Present(Present {
                credential: device.disk().credential(),
            }))
            .await
            .unwrap();
        let Message::Check(check) = connector.recv(PATIENCE).await.unwrap() else {
            panic!("no check");
        };
        (connector, check.nonce)
    }

    /// The largest generation the hub issued to `device`.
    pub async fn max_generation(&self, device: &Device) -> i64 {
        self.max_generation_of_key(device.public()).await
    }

    /// The largest generation the hub issued to the device of this identity.
    pub async fn max_generation_of_key(&self, identity: &PublicKey) -> i64 {
        sqlx::query_scalar("SELECT max_generation FROM device WHERE public_key = $1")
            .bind(identity.as_bytes().as_slice())
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// Waits until a query of this database waits for a lock: the hub reached the write the
    /// test holds back.
    pub async fn until_a_query_waits_for_a_lock(&self) {
        for _ in 0..1_500 {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity                  WHERE datname = current_database() AND wait_event_type = 'Lock'",
            )
            .fetch_one(&self.pool)
            .await
            .unwrap();
            if waiting > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("no query of the hub came to wait for the lock");
    }

    /// Moves the mark past the last security event, as a restored copy of the database looks.
    pub async fn restore_an_older_copy(&self) {
        let last: i64 = sqlx::query_scalar("SELECT max(seq) FROM audit WHERE security")
            .fetch_one(&self.pool)
            .await
            .unwrap();
        Mark::new(&self.installation.data_dir)
            .advance(last + 10)
            .await
            .unwrap();
    }
}

/// Reads a connection to its end: the kind of every message of the hub but the heartbeat and
/// `closing`, and how it ended.
pub async fn until_the_end(connector: &mut TestConnector) -> (Vec<MessageType>, Ended) {
    let mut seen = Vec::new();
    loop {
        match connector.recv(PATIENCE).await {
            Ok(message) => seen.extend(message.kind()),
            Err(KitError::Ended(ended)) => return (seen, ended),
            Err(other) => panic!("{other:?} after {seen:?}"),
        }
    }
}
