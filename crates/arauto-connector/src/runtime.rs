//! The connector running in a process, as the server of tools of a Hermes uses it: the copy
//! is the leader of its folder or forwards to the leader, and that is decided here, by the lock
//! of the folder (`docs/11-conector.md`, section 3).
//!
//! ```text
//!  Connector::start  ── the lock is free ──> leader: the channel and the local socket
//!                    └─ the lock is held ──> relay: the calls go to the leader
//!  the leader dies   ── the system frees the lock ──> a relay takes it and becomes the leader
//!  a restart         ── the leader tells its relays ─> every copy ends, Hermes starts them again
//! ```
//!
//! What the server of tools needs: [`Connector::call`], [`Connector::tools`] (and
//! [`Connector::watch_tools`]), [`Connector::status`], [`Connector::finished`] and
//! [`Connector::shutdown`]. The role changes underneath without the caller seeing it.

use std::sync::Arc;
use std::time::Duration;

use arauto_platform::{FileLock, LockError};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{CallOutcome, Payload};
use arauto_protocol::tools::ToolName;
use serde_json::{Map, Value};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::attempt::stopped;
use crate::home::Home;
use crate::leader::{self, Finish, Leader};
use crate::relay::Relay;
use crate::session::refused;
use crate::settings::{Settings, duration};
use crate::status::{Role, Status};

/// How long [`Connector::start`] waits for the role to be decided before it returns.
const ROLE_PATIENCE: Duration = Duration::from_secs(5);

/// How long a copy that took the lock and could not start as the leader waits to try again.
const LEADER_RETRY: Duration = Duration::from_secs(5);

/// Why the connector could not start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// The folder of the connector could not be prepared.
    #[error(transparent)]
    Folder(#[from] arauto_platform::FsError),
}

#[derive(Clone)]
enum Now {
    Deciding,
    Leader(Arc<leader::Shared>),
    Relay(Arc<Relay>),
}

struct Inner {
    home: Home,
    settings: Arc<Settings>,
    now: watch::Sender<Now>,
    tools: watch::Sender<Vec<ToolName>>,
    stop: watch::Sender<bool>,
    finished: watch::Sender<bool>,
}

/// The connector in this process.
pub struct Connector {
    inner: Arc<Inner>,
    task: JoinHandle<()>,
}

impl std::fmt::Debug for Connector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connector")
            .field("role", &self.role())
            .finish_non_exhaustive()
    }
}

impl Connector {
    /// Starts the connector of the folder `home`: the leader when the lock is free, a copy
    /// that forwards to the leader otherwise. Returns once the role is decided, or after a few
    /// seconds; meanwhile the calls get AR-203.
    ///
    /// # Errors
    /// [`StartError::Folder`] when the folder cannot be prepared: it must belong to this user
    /// and be closed to others.
    pub async fn start(home: Home, settings: Settings) -> Result<Self, StartError> {
        {
            let home = home.clone();
            tokio::task::spawn_blocking(move || home.prepare())
                .await
                .map_err(|_| {
                    StartError::Folder(arauto_platform::FsError::NotAFilePath {
                        path: std::path::PathBuf::new(),
                    })
                })??;
        }
        let inner = Arc::new(Inner {
            home,
            settings: Arc::new(settings),
            now: watch::Sender::new(Now::Deciding),
            tools: watch::Sender::new(Vec::new()),
            stop: watch::Sender::new(false),
            finished: watch::Sender::new(false),
        });
        let task = tokio::spawn(supervise(Arc::clone(&inner)));
        let mut now = inner.now.subscribe();
        let _ = tokio::time::timeout(
            ROLE_PATIENCE,
            now.wait_for(|now| !matches!(now, Now::Deciding)),
        )
        .await;
        Ok(Self { inner, task })
    }

    /// What this copy is right now.
    #[must_use]
    pub fn role(&self) -> Role {
        match &*self.inner.now.borrow() {
            Now::Deciding => Role::Starting,
            Now::Leader(_) => Role::Leader,
            Now::Relay(_) => Role::Relay,
        }
    }

    /// Calls the tool `tool` with `args`. A name out of the format of the catalog is refused
    /// here with AR-711; without a session the answer is AR-203, or the code of why the
    /// channel is no longer used.
    pub async fn call(&self, tool: &str, args: Map<String, Value>) -> CallOutcome {
        let Ok(tool) = ToolName::parse(tool) else {
            return refused(ErrorCode::Ar711);
        };
        let args = Payload::new(args);
        let now = self.inner.now.borrow().clone();
        match now {
            Now::Leader(leader) => leader.call(tool, args).await,
            Now::Relay(relay) => relay.call(tool, args).await,
            Now::Deciding => refused(ErrorCode::Ar203),
        }
    }

    /// The released tools: of the session, or of the last session, or the one that is always
    /// there.
    #[must_use]
    pub fn tools(&self) -> Vec<ToolName> {
        self.inner.tools.borrow().clone()
    }

    /// The released tools, and each change of the list.
    #[must_use]
    pub fn watch_tools(&self) -> watch::Receiver<Vec<ToolName>> {
        self.inner.tools.subscribe()
    }

    /// The situation, without any secret.
    pub async fn status(&self) -> Status {
        let now = self.inner.now.borrow().clone();
        let found = match now {
            Now::Leader(leader) => Some(leader.status()),
            Now::Relay(relay) => relay.status().await,
            Now::Deciding => None,
        };
        found.unwrap_or_else(|| Status {
            role: Role::Starting,
            leader_process: None,
            version: env!("CARGO_PKG_VERSION").to_owned(),
            device: None,
            request: None,
            tools: self.tools(),
            leftovers: 0,
        })
    }

    /// Resolves when a `join` asked this process to end, so that Hermes starts the new
    /// version of the program: the leader was asked, and each copy that forwards to it ends
    /// with it.
    pub async fn finished(&self) {
        let mut finished = self.inner.finished.subscribe();
        let _ = finished.wait_for(|finished| *finished).await;
    }

    /// Stops in order: the channel closes, the local socket stops, the lock is released.
    pub async fn shutdown(self) {
        self.inner.stop.send_replace(true);
        let patience = duration(self.inner.settings.shutdown_ms.saturating_mul(3));
        if tokio::time::timeout(patience, self.task).await.is_err() {
            tracing::warn!("the connector did not stop in time");
        }
    }
}

async fn supervise(inner: Arc<Inner>) {
    let mut stop = inner.stop.subscribe();
    let retry = duration(inner.settings.relay_retry_ms);
    loop {
        if *stop.borrow() {
            break;
        }
        let path = inner.home.lock_file();
        let tried = tokio::task::spawn_blocking(move || FileLock::try_acquire(&path)).await;
        match tried {
            Ok(Ok(lock)) => {
                let started = Leader::start(
                    inner.home.clone(),
                    Arc::clone(&inner.settings),
                    lock,
                    inner.tools.clone(),
                )
                .await;
                let leader = match started {
                    Ok(leader) => leader,
                    Err(error) => {
                        tracing::warn!(%error, "the leader could not start");
                        // What stopped it does not go away by itself: the lock is let go and
                        // tried again later, so that the log does not fill.
                        tokio::select! {
                            () = tokio::time::sleep(LEADER_RETRY) => continue,
                            () = stopped(&mut stop) => break,
                        }
                    }
                };
                inner
                    .now
                    .send_replace(Now::Leader(Arc::clone(leader.shared())));
                let finish = tokio::select! {
                    finish = leader.finished() => Some(finish),
                    () = stopped(&mut stop) => None,
                };
                inner.now.send_replace(Now::Deciding);
                leader.stop().await;
                if finish == Some(Finish::Restart) {
                    inner.finished.send_replace(true);
                }
                break;
            }
            Ok(Err(LockError::AlreadyHeld { .. })) => {
                match Relay::connect(
                    &inner.home,
                    Arc::clone(&inner.settings),
                    inner.tools.clone(),
                )
                .await
                {
                    Ok(relay) => {
                        let relay = Arc::new(relay);
                        inner.now.send_replace(Now::Relay(Arc::clone(&relay)));
                        let gone = tokio::select! {
                            () = relay.gone() => true,
                            () = stopped(&mut stop) => false,
                        };
                        inner.now.send_replace(Now::Deciding);
                        let restarting = relay.restarting();
                        relay.stop();
                        if !gone {
                            break;
                        }
                        // The leader ended for the new program: this copy ends as well, so
                        // that Hermes starts it again on the new one.
                        if restarting {
                            inner.finished.send_replace(true);
                            break;
                        }
                        // The leader is gone: the lock is tried again at once.
                    }
                    // The leader is starting, or ending: a moment later.
                    Err(_) => tokio::select! {
                        () = tokio::time::sleep(retry) => {}
                        () = stopped(&mut stop) => break,
                    },
                }
            }
            Ok(Err(error)) => {
                tracing::warn!(%error, "the lock of the folder could not be tried");
                tokio::select! {
                    () = tokio::time::sleep(retry) => {}
                    () = stopped(&mut stop) => break,
                }
            }
            Err(_) => tokio::select! {
                () = tokio::time::sleep(retry) => {}
                () = stopped(&mut stop) => break,
            },
        }
    }
    inner.now.send_replace(Now::Deciding);
}
