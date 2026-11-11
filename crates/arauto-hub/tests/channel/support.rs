//! What the tests of the channel share: one installed hub database for the whole process, a
//! hub started in the test with short deadlines, and purposes that take a connection where
//! a test needs it (session, waiting, flood), since enrollment and channel come later.

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arauto_hub::admin::{self, InitOutcome};
use arauto_hub::channel::{
    self, ChannelSettings, Downloads, Hub, Link, Options, Purposes, Running,
};
use arauto_hub::config::{ClientIpSource, DatabaseUrl, PublicUrl, ServeConfig, StorageConfig};
use arauto_hub::db::{PoolOptions, ResolvedOrg};
use arauto_protocol::clock::SystemClock;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{
    CallId, CallOutcome, CallResult, DeclaredText, Hello, HelloOk, Message, Payload, Session,
    ToolList, Tools,
};
use arauto_protocol::names::HermesName;
use arauto_protocol::noise::PublicKey;
use serde_json::{Map, Value};
use tokio::sync::OnceCell;

use crate::common::{NOW, TestDb};

/// A database with an installation, and its data folder.
pub struct Installation {
    pub url: DatabaseUrl,
    pub data_dir: PathBuf,
    pub hub_key: PublicKey,
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

/// The configuration of a hub of `installation`, on two free ports of this machine.
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
    }
}

/// A running hub of the shared installation.
pub struct TestHub {
    pub running: Running,
    pub hub_key: PublicKey,
}

impl TestHub {
    pub fn addr(&self) -> SocketAddr {
        self.running.connectors_addr()
    }

    pub fn hub(&self) -> &Arc<Hub> {
        self.running.hub()
    }

    pub fn hostile(&self) -> arauto_testkit::Hostile {
        arauto_testkit::Hostile::new(self.addr(), self.hub_key)
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

// ---------------------------------------------------------------------------- purposes

/// What the purpose `channel` does in a test.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// Stays before the session and reads whatever comes.
    Park,
    /// Enters the session, and answers every message with `tools`.
    Session,
    /// Enters the session and sends `count` results of `size` bytes, then `tools`. Says in
    /// `blocked` whether a send had to wait for room.
    Flood {
        count: usize,
        size: usize,
        blocked: Arc<AtomicBool>,
    },
    /// Waits for approval for `ms` milliseconds, then goes back to before the session.
    Wait { ms: u64 },
}

/// The purposes of a test: `channel` does the [`Behavior`]; `enroll` closes with AR-205.
#[derive(Debug, Clone)]
pub struct TestPurposes(pub Behavior);

pub fn session_message() -> Message {
    Message::Session(Session {
        hermes: HermesName::parse("test").unwrap(),
        tools: ToolList::new(vec![]).unwrap(),
        wake: false,
    })
}

pub fn tools_message() -> Message {
    Message::Tools(Tools {
        tools: ToolList::new(vec![]).unwrap(),
    })
}

/// A `result` with `size` bytes of padding.
pub fn big_result(size: usize) -> Message {
    let mut content = Map::new();
    content.insert("pad".to_owned(), Value::String("b".repeat(size)));
    Message::Result(CallResult {
        id: CallId::MIN,
        outcome: CallOutcome::Ok(Payload::new(content)),
    })
}

async fn park(link: &mut Link) {
    while link.recv().await.is_ok() {}
}

impl Purposes for TestPurposes {
    async fn enroll(&self, _hub: Arc<Hub>, link: Link, _hello: Hello) {
        link.close(ErrorCode::Ar205);
    }

    async fn channel(&self, _hub: Arc<Hub>, mut link: Link, _hello: Hello, _answered: HelloOk) {
        match &self.0 {
            Behavior::Park => park(&mut link).await,
            Behavior::Session => {
                if link.enter_session(&session_message()).await.is_err() {
                    return;
                }
                while link.recv().await.is_ok() {
                    if link.send(&tools_message()).await.is_err() {
                        return;
                    }
                }
            }
            Behavior::Flood {
                count,
                size,
                blocked,
            } => {
                if link.enter_session(&session_message()).await.is_err() {
                    return;
                }
                let message = big_result(*size);
                for _ in 0..*count {
                    // A send is never abandoned half way: it would leave a stream unfinished.
                    let send = link.send(&message);
                    tokio::pin!(send);
                    let sent = tokio::select! {
                        sent = &mut send => sent,
                        () = tokio::time::sleep(Duration::from_millis(200)) => {
                            blocked.store(true, Ordering::SeqCst);
                            send.await
                        }
                    };
                    if sent.is_err() {
                        return;
                    }
                }
                if link.send(&tools_message()).await.is_err() {
                    return;
                }
                park(&mut link).await;
            }
            Behavior::Wait { ms } => {
                link.enter_waiting();
                let _ = tokio::time::timeout(Duration::from_millis(*ms), park(&mut link)).await;
                link.leave_waiting();
                park(&mut link).await;
            }
        }
    }
}
