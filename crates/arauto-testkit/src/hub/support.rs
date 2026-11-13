//! What the tests of a channel share: one installed hub database for the whole process, and a
//! hub started in the test with short deadlines and the purposes the test brings.
// Test support: a failure here is the failure of the test that called it, with the reason.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use arauto_hub::admin::{self, InitOutcome};
use arauto_hub::channel::{self, ChannelSettings, Downloads, Hub, Options, Purposes, Running};
use arauto_hub::config::{
    ClientIpSource, DatabaseUrl, PanelConfig, PublicUrl, ServeConfig, StorageConfig,
};
use arauto_hub::db::{PoolOptions, ResolvedOrg};
use arauto_protocol::clock::SystemClock;
use arauto_protocol::message::DeclaredText;
use arauto_protocol::noise::PublicKey;
use tokio::sync::OnceCell;

use super::db::{NOW, TestDb};

/// A database with an installation, and its data folder.
pub struct Installation {
    /// The address of the database.
    pub url: DatabaseUrl,
    /// The data folder: master key and mark.
    pub data_dir: PathBuf,
    /// The public key of the hub.
    pub hub_key: PublicKey,
    /// The organization `init` created.
    pub organization: ResolvedOrg,
}

static SHARED: OnceCell<Installation> = OnceCell::const_new();

/// Installs a hub in a database of its own, the way `arauto-hub init` does.
pub async fn install() -> Installation {
    let db = TestDb::own().await;
    // The folder outlives the test: the hubs of the whole process use it.
    let data_dir = tempfile::tempdir().unwrap().keep().join("data");
    let outcome = admin::init(
        &db.pool,
        &data_dir,
        &DeclaredText::lossy("test"),
        false,
        NOW,
    )
    .await
    .unwrap();
    let InitOutcome::Created(installed) = outcome else {
        panic!("a new database was already installed");
    };
    Installation {
        url: db.url(),
        data_dir,
        hub_key: installed.hub_key,
        organization: installed.organization,
    }
}

/// The installation most tests share: the transport never writes to it, and the tests that
/// do keep to rows of their own.
pub async fn shared() -> &'static Installation {
    SHARED.get_or_init(install).await
}

/// Documented values, a pool of two connections, and the deadlines a test does not measure
/// stretched, so that a slow machine does not end a connection by accident.
#[must_use]
pub fn settings() -> ChannelSettings {
    let mut settings = ChannelSettings {
        pool: PoolOptions {
            size: 2,
            acquire_timeout: Duration::from_secs(10),
        },
        ..ChannelSettings::default()
    };
    settings.deadlines.become_session_ms = 120_000;
    settings.deadlines.silence_ms = 120_000;
    settings
}

/// The address a test names the panel by: what goes in `Host` and, with `http://` before it,
/// in `Origin`. The panel listens on a free port; it answers to this name.
pub const PANEL_HOST: &str = "localhost:8445";

/// The origin of the panel of a test hub.
pub const PANEL_ORIGIN: &str = "http://localhost:8445";

/// The configuration of a hub of `installation`, on free ports of this machine: the two
/// every hub has, and the one of the panel.
#[must_use]
pub fn config(installation: &Installation, client_ip: ClientIpSource) -> ServeConfig {
    let local: SocketAddr = "127.0.0.1:0".parse().unwrap();
    ServeConfig {
        storage: StorageConfig {
            database_url: installation.url.clone(),
            data_dir: installation.data_dir.clone(),
            allow_unsafe_db: false,
        },
        listen: local,
        listen_internal: local,
        public_url: PublicUrl::parse("http://127.0.0.1:1").unwrap(),
        client_ip,
        panel: Some(PanelConfig {
            listen: local,
            url: PublicUrl::parse(PANEL_ORIGIN).unwrap(),
        }),
    }
}

/// A running hub of the shared installation.
pub struct TestHub {
    /// The hub.
    pub running: Running,
    /// Its public key.
    pub hub_key: PublicKey,
}

impl TestHub {
    /// The address of the port of the connectors.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.running.connectors_addr()
    }

    /// What the connections share.
    #[must_use]
    pub fn hub(&self) -> &Arc<Hub> {
        self.running.hub()
    }

    /// A hostile client of this hub.
    #[must_use]
    pub fn hostile(&self) -> crate::Hostile {
        crate::Hostile::new(self.addr(), self.hub_key)
    }

    /// Waits until the hub has `count` live connections.
    pub async fn links_become(&self, count: usize, within: Duration) -> bool {
        eventually(within, || self.hub().links().len() == count).await
    }
}

/// Starts a hub of the shared installation.
pub async fn start<P: Purposes>(settings: ChannelSettings, purposes: P) -> TestHub {
    start_with(
        settings,
        purposes,
        ClientIpSource::Socket,
        Downloads::empty(),
    )
    .await
}

/// Starts a hub of the shared installation, with every option.
pub async fn start_with<P: Purposes>(
    settings: ChannelSettings,
    purposes: P,
    client_ip: ClientIpSource,
    downloads: Downloads,
) -> TestHub {
    let installation = shared().await;
    let options = Options {
        settings,
        downloads,
        purposes,
    };
    let running = channel::start(
        &config(installation, client_ip),
        options,
        Arc::new(SystemClock),
    )
    .await
    .unwrap();
    TestHub {
        running,
        hub_key: installation.hub_key,
    }
}

/// Polls `condition` until it holds or `within` runs out.
pub async fn eventually(within: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let until = tokio::time::Instant::now() + within;
    loop {
        if condition() {
            return true;
        }
        if tokio::time::Instant::now() >= until {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Runs `work` and says how long it took.
pub async fn timed<T>(work: impl Future<Output = T>) -> (T, Duration) {
    let started = tokio::time::Instant::now();
    let value = work.await;
    (value, started.elapsed())
}
