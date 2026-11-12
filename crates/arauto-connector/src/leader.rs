//! The leader: the one copy of the connector that holds the lock of the folder
//! (`docs/11-conector.md`, sections 2 and 3; `docs/04-seguranca-conexao.md`, C9 and C10).
//!
//! - It keeps the channel of the **device in use** (the folder `current` names) and follows,
//!   apart, the **pending request** (the newest folder still waiting for its first session).
//!   Each one is a slot: one attempt at a time, and the waits of [`crate::backoff`].
//! - When the request reaches its session, it becomes the device in use: one rename of
//!   `current`. The old device is asked to close, unless the hub already said it was
//!   replaced, and so is every other identity left in the folder. A folder is deleted only
//!   after the hub answered for it; until then the leader asks again at each start and each
//!   time the channel comes back.
//! - The only folder discarded because of what the hub said is a request without a credential
//!   that the hub refused, denied or let expire.
//! - When the hub tells it to stop using the channel (quarantine, suspended, revoked, unknown,
//!   expired, replaced with nothing to go to), the leader stays alive: the calls get the code,
//!   and the local socket still serves `join`, `leave` and `status`.
//! - It serves the local socket to the other copies. When a `join` asks it to end for the new
//!   program, it tells every connection of the socket, so that the copies that forward end
//!   too.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arauto_platform::{FileLock, LocalListener, LocalSocketError, LocalStream};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::CALLS_IN_FLIGHT_MAX;
use arauto_protocol::message::{CallOutcome, LeftStatus, Payload};
use arauto_protocol::tools::{ARAUTO_STATUS, ToolName};
use serde_json::{Value, json};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::{Semaphore, mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use crate::attempt::{self, Ctx, stopped};
use crate::backoff::{self, Backoff, Next};
use crate::catalog::{Local, Said};
use crate::home::Home;
use crate::local::{self, Envelope, Lines, Request};
use crate::session::{LeaveAsk, Live, News, refused};
use crate::settings::{Settings, duration};
use crate::short;
use crate::status::{ChannelStatus, DeviceStatus, Role, Status};
use crate::store::{self, Current, DeviceDir, DeviceId, Situation};
use crate::writes::Saving;

/// How many connections of the local socket are served at once.
const LOCAL_CONNECTIONS_MAX: usize = 64;

/// How many times the leader tries to listen while the socket of a dead leader goes away.
const BIND_ATTEMPTS: u32 = 40;

/// Why the leader did not start.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LeaderError {
    /// The local socket could not be served.
    #[error(transparent)]
    Socket(#[from] LocalSocketError),
}

/// How the leader ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Finish {
    /// It was asked to stop.
    Stopped,
    /// A `join` asked it to end, so that Hermes starts the new version.
    Restart,
}

/// What every task of the leader shares.
pub(crate) struct Shared {
    pub(crate) home: Home,
    pub(crate) settings: Arc<Settings>,
    commands: mpsc::UnboundedSender<Command>,
    /// The session that serves the calls.
    live: watch::Sender<Option<Live>>,
    /// The released tools: of the session, or of the last one.
    pub(crate) tools: watch::Sender<Vec<ToolName>>,
    /// What a call gets without a session.
    unavailable: watch::Sender<ErrorCode>,
    view: watch::Sender<View>,
    finished: watch::Sender<Option<Finish>>,
}

/// What `status` shows of the slots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct View {
    device: Option<DeviceStatus>,
    request: Option<DeviceStatus>,
    leftovers: usize,
}

/// One identity per answer of a `leave`.
#[derive(Debug, Clone)]
pub(crate) struct LeaveReport {
    /// Each folder and what the hub answered, or why it did not.
    pub(crate) left: Vec<(DeviceId, Result<LeftStatus, Said>)>,
    /// Every identity was confirmed and every file deleted.
    pub(crate) done: bool,
}

#[derive(Debug)]
enum Command {
    Rescan,
    Slot { serial: u64, event: SlotEvent },
    Leave(oneshot::Sender<LeaveReport>),
    LeaveDone { done: bool },
    CleanupDone { folders: Option<usize> },
    Finish(Finish),
}

#[derive(Debug)]
enum SlotEvent {
    Connecting,
    News(News),
    Down(Option<Said>),
    /// The channel of this device is no longer used, but the slot goes on: a version that
    /// does not agree, presented again later.
    Paused(ErrorCode),
    Over(Over),
}

/// Why a slot ended.
#[derive(Debug)]
enum Over {
    /// Asked to stop.
    Asked,
    /// The hub answered a request to leave over its channel.
    Left,
    /// A request without a credential refused by the hub: its folder was discarded.
    Discarded,
    /// The device was replaced by another one.
    Replaced,
    /// The channel is no longer used for this device.
    Stopped(Said),
}

struct Slot {
    serial: u64,
    id: DeviceId,
    is_device: Arc<AtomicBool>,
    stop: watch::Sender<bool>,
    leave: mpsc::Sender<LeaveAsk>,
    task: JoinHandle<()>,
    channel: ChannelStatus,
}

impl Slot {
    fn view(&self) -> DeviceStatus {
        DeviceStatus {
            folder: self.id.to_string(),
            channel: self.channel.clone(),
        }
    }

    fn established(&self) -> bool {
        matches!(
            self.channel,
            ChannelStatus::Waiting | ChannelStatus::Session { .. }
        )
    }

    async fn stop(self, patience: Duration) {
        self.stop.send_replace(true);
        let _ = tokio::time::timeout(patience, self.task).await;
    }
}

/// The leader of a folder.
pub(crate) struct Leader {
    shared: Arc<Shared>,
    tasks: Vec<JoinHandle<()>>,
}

impl Leader {
    /// Starts the leader: listens on the local socket with the lock of the folder, removes
    /// what interrupted work left, and starts the channels.
    pub(crate) async fn start(
        home: Home,
        settings: Arc<Settings>,
        lock: FileLock,
        tools: watch::Sender<Vec<ToolName>>,
    ) -> Result<Self, LeaderError> {
        let lock = Arc::new(lock);
        let mut tries = 0u32;
        let listener = loop {
            match LocalListener::bind(home.root(), Arc::clone(&lock)).await {
                Ok(listener) => break listener,
                // The socket of a leader that died an instant ago.
                Err(LocalSocketError::AlreadyListening { .. }) if tries < BIND_ATTEMPTS => {
                    tries = tries.saturating_add(1);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(error) => return Err(error.into()),
            }
        };
        drop(lock);
        if let Ok(_guard) = store::lock_devices(&home, settings.command_lock_ms).await {
            let cleaned = {
                let home = home.clone();
                tokio::task::spawn_blocking(move || store::clean_leftovers(&home)).await
            };
            if !matches!(cleaned, Ok(Ok(()))) {
                tracing::warn!("what interrupted work left in the folder could not be removed");
            }
        }
        let initial = {
            let home = home.clone();
            tokio::task::spawn_blocking(move || initial_tools(&home))
                .await
                .unwrap_or_else(|_| always())
        };
        tools.send_replace(initial);
        let (commands, received) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            home,
            settings,
            commands,
            live: watch::Sender::new(None),
            tools,
            unavailable: watch::Sender::new(ErrorCode::Ar203),
            view: watch::Sender::new(View::default()),
            finished: watch::Sender::new(None),
        });
        let actor = tokio::spawn(actor(Arc::clone(&shared), received));
        let server = tokio::spawn(serve(Arc::clone(&shared), listener));
        #[cfg(feature = "testing")]
        shared
            .settings
            .emit(|| crate::testing::Event::Role { leader: true });
        tracing::info!("this copy of the connector is the leader of its folder");
        Ok(Self {
            shared,
            tasks: vec![actor, server],
        })
    }

    pub(crate) fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Resolves when the leader ended, with how.
    pub(crate) async fn finished(&self) -> Finish {
        let mut finished = self.shared.finished.subscribe();
        match finished.wait_for(Option::is_some).await {
            Ok(finish) => (*finish).unwrap_or(Finish::Stopped),
            Err(_) => Finish::Stopped,
        }
    }

    /// Stops in order: the channels close, the local socket stops.
    pub(crate) async fn stop(self) {
        let _ = self.shared.commands.send(Command::Finish(Finish::Stopped));
        let patience = duration(self.shared.settings.shutdown_ms);
        for task in self.tasks {
            let _ = tokio::time::timeout(patience, task).await;
        }
    }
}

/// The tools of the last session, or the one that is always there.
fn initial_tools(home: &Home) -> Vec<ToolName> {
    match store::read_state(home) {
        Ok(state) if !state.tools.is_empty() => state.tools,
        Ok(_) => always(),
        Err(error) => {
            tracing::warn!(%error, "state.json could not be read");
            always()
        }
    }
}

fn always() -> Vec<ToolName> {
    ToolName::parse(ARAUTO_STATUS)
        .map(|tool| vec![tool])
        .unwrap_or_default()
}

impl Shared {
    /// A call, through the session of the device in use.
    pub(crate) async fn call(&self, tool: ToolName, args: Payload) -> CallOutcome {
        let live = self.live.borrow().clone();
        match live {
            Some(live) => live.calls.call(tool, args).await,
            None => refused(*self.unavailable.borrow()),
        }
    }

    pub(crate) fn status(&self) -> Status {
        let view = self.view.borrow().clone();
        Status {
            role: Role::Leader,
            leader_process: Some(std::process::id()),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            device: view.device,
            request: view.request,
            tools: self.tools.borrow().clone(),
            leftovers: view.leftovers,
        }
    }

    pub(crate) async fn leave(&self) -> Option<LeaveReport> {
        let (asked, answer) = oneshot::channel();
        self.commands.send(Command::Leave(asked)).ok()?;
        answer.await.ok()
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Keeps the tools of the session in `state.json`, without holding anything up.
    fn keep_tools(&self, tools: Vec<ToolName>) {
        if *self.tools.borrow() == tools {
            return;
        }
        self.tools.send_replace(tools.clone());
        let home = self.home.clone();
        let patience = self.settings.devices_lock_ms;
        tokio::spawn(async move {
            let Ok(_guard) = store::lock_devices(&home, patience).await else {
                return;
            };
            let written = tokio::task::spawn_blocking(move || {
                store::write_state(&home, &store::State { tools })
            })
            .await;
            if !matches!(written, Ok(Ok(()))) {
                tracing::warn!("state.json could not be written");
            }
        });
    }
}

// ---------------------------------------------------------------------------------- actor

#[derive(Default)]
struct State {
    device: Option<Slot>,
    request: Option<Slot>,
    /// The device in use whose channel is no longer used, and why.
    device_stopped: Option<(DeviceId, Said)>,
    /// The request no longer followed, and why.
    request_stopped: Option<(DeviceId, Said)>,
    leaving: bool,
    cleaning: bool,
    clean_again: bool,
    next_serial: u64,
    /// The device folders seen the last time the folder was read.
    folders: usize,
}

async fn actor(shared: Arc<Shared>, mut commands: mpsc::UnboundedReceiver<Command>) {
    let mut state = State::default();
    state.reconcile(&shared, true).await;
    while let Some(command) = commands.recv().await {
        match command {
            Command::Rescan => {
                if !state.leaving {
                    state.reconcile(&shared, false).await;
                }
            }
            Command::Slot { serial, event } => state.on_slot(&shared, serial, event).await,
            Command::Leave(asked) => state.leave(&shared, asked),
            Command::LeaveDone { done } => {
                state.leaving = false;
                if done {
                    state.device_stopped = None;
                    state.request_stopped = None;
                    shared.tools.send_replace(always());
                }
                state.reconcile(&shared, false).await;
            }
            Command::CleanupDone { folders } => {
                state.cleaning = false;
                if let Some(folders) = folders {
                    state.folders = folders;
                }
                if state.clean_again {
                    state.clean_again = false;
                    state.cleanup(&shared);
                }
            }
            Command::Finish(finish) => {
                let patience = duration(shared.settings.shutdown_ms);
                shared.live.send_replace(None);
                // Both at once: the whole stop fits the time Hermes gives.
                let stops = [state.device.take(), state.request.take()]
                    .into_iter()
                    .flatten()
                    .map(|slot| slot.stop(patience));
                futures_util::future::join_all(stops).await;
                shared.finished.send_replace(Some(finish));
                return;
            }
        }
        state.publish(&shared);
    }
}

impl State {
    fn serial(&mut self) -> u64 {
        self.next_serial = self.next_serial.saturating_add(1);
        self.next_serial
    }

    fn start_slot(&mut self, shared: &Arc<Shared>, id: DeviceId, is_device: bool) -> Slot {
        let serial = self.serial();
        let (stop, stop_seen) = watch::channel(false);
        let (leave, leave_seen) = mpsc::channel(4);
        let is_device = Arc::new(AtomicBool::new(is_device));
        let task = tokio::spawn(slot(
            Arc::clone(shared),
            serial,
            id.clone(),
            Arc::clone(&is_device),
            stop_seen,
            leave_seen,
        ));
        Slot {
            serial,
            id,
            is_device,
            stop,
            leave,
            task,
            channel: ChannelStatus::Connecting,
        }
    }

    /// Makes the slots follow the folders: the device `current` names, and the newest
    /// pending request. A slot of a folder that did not change is left as it is.
    async fn reconcile(&mut self, shared: &Arc<Shared>, starting: bool) {
        let layout = match scan(&shared.home).await {
            Ok(layout) => layout,
            Err(error) => {
                tracing::warn!(%error, "the folder of the connector could not be read");
                return;
            }
        };
        self.folders = layout.folders.len();
        let device = layout
            .current()
            .filter(|folder| folder.record().is_some())
            .map(|folder| folder.id().clone());
        if layout.current == Current::Damaged {
            tracing::warn!("the file current is damaged or names no device folder");
        }
        let request = layout.request().map(|folder| folder.id().clone());
        if self.device.as_ref().map(|slot| &slot.id) != device.as_ref() {
            if let Some(old) = self.device.take() {
                old.stop.send_replace(true);
            }
            let already_stopped = self
                .device_stopped
                .as_ref()
                .is_some_and(|(id, _)| Some(id) == device.as_ref());
            if !already_stopped {
                self.device_stopped = None;
                if let Some(id) = device.clone() {
                    self.device = Some(self.start_slot(shared, id, true));
                }
            }
        }
        if self.request.as_ref().map(|slot| &slot.id) != request.as_ref() {
            if let Some(old) = self.request.take() {
                old.stop.send_replace(true);
            }
            let already_stopped = self
                .request_stopped
                .as_ref()
                .is_some_and(|(id, _)| Some(id) == request.as_ref());
            if !already_stopped {
                self.request_stopped = None;
                if let Some(id) = request.clone() {
                    self.request = Some(self.start_slot(shared, id, false));
                }
            }
        }
        if device.is_none() {
            self.device_stopped = None;
        }
        // At each start, the identities left over from a switch of device are asked to close
        // again.
        if starting && device.is_some() {
            self.cleanup(shared);
        }
    }

    fn slot_of(&mut self, serial: u64) -> Option<(&mut Slot, bool)> {
        if let Some(slot) = self.device.as_mut().filter(|slot| slot.serial == serial) {
            return Some((slot, true));
        }
        self.request
            .as_mut()
            .filter(|slot| slot.serial == serial)
            .map(|slot| (slot, false))
    }

    async fn on_slot(&mut self, shared: &Arc<Shared>, serial: u64, event: SlotEvent) {
        let Some((slot, is_device)) = self.slot_of(serial) else {
            return;
        };
        match event {
            SlotEvent::Connecting => slot.channel = ChannelStatus::Connecting,
            SlotEvent::Down(said) => {
                slot.channel = ChannelStatus::down(said);
                if is_device {
                    shared.live.send_replace(None);
                }
            }
            SlotEvent::Paused(code) => {
                slot.channel = ChannelStatus::Stopped(Some(code));
                if is_device {
                    shared.live.send_replace(None);
                    shared.unavailable.send_replace(code);
                }
                #[cfg(feature = "testing")]
                shared.settings.emit(|| crate::testing::Event::Stopped {
                    device: slot.id.to_string(),
                    code,
                });
            }
            SlotEvent::News(News::Waiting) => slot.channel = ChannelStatus::Waiting,
            SlotEvent::News(News::Tools(tools)) => {
                if is_device {
                    shared.keep_tools(tools);
                }
            }
            SlotEvent::News(News::Ended) => {
                slot.channel = ChannelStatus::Connecting;
                if is_device {
                    shared.live.send_replace(None);
                }
            }
            SlotEvent::News(News::Session(live)) => {
                slot.channel = ChannelStatus::Session {
                    hermes: live.session.hermes.clone(),
                    can_be_woken: live.session.wake,
                };
                if is_device {
                    shared.unavailable.send_replace(ErrorCode::Ar203);
                    shared.live.send_replace(Some(live.clone()));
                    shared.keep_tools(live.session.tools.as_slice().to_vec());
                    #[cfg(feature = "testing")]
                    shared.settings.emit(|| crate::testing::Event::Serving {
                        device: slot.id.to_string(),
                    });
                    record_active(shared, &slot.id).await;
                    // Each time the channel comes back, the leftovers are asked again.
                    self.cleanup(shared);
                } else {
                    self.takeover(shared, live).await;
                }
            }
            SlotEvent::Over(over) => self.on_over(shared, serial, is_device, over),
        }
    }

    fn on_over(&mut self, shared: &Arc<Shared>, serial: u64, is_device: bool, over: Over) {
        let slot = if is_device {
            self.device.take()
        } else {
            self.request.take()
        };
        let Some(slot) = slot.filter(|slot| slot.serial == serial) else {
            return;
        };
        let id = slot.id.clone();
        if matches!(over, Over::Discarded) {
            self.folders = self.folders.saturating_sub(1);
        }
        let said = match over {
            Over::Asked | Over::Left | Over::Discarded => None,
            Over::Replaced => Some(Said::code(ErrorCode::Ar503)),
            Over::Stopped(said) => Some(said),
        };
        if let Some(said) = said {
            if is_device {
                shared.live.send_replace(None);
                shared
                    .unavailable
                    .send_replace(crate::status::code_of(said).unwrap_or(ErrorCode::Ar203));
            }
            // Told once the calls already get the code.
            #[cfg(feature = "testing")]
            if let Some(code) = crate::status::code_of(said) {
                shared.settings.emit(|| crate::testing::Event::Stopped {
                    device: id.to_string(),
                    code,
                });
            }
            if is_device {
                self.device_stopped = Some((id, said));
            } else {
                self.request_stopped = Some((id, said));
            }
        } else if is_device {
            shared.live.send_replace(None);
        }
    }

    /// The request reached its session: it becomes the device in use, with one rename of
    /// `current`, and the old device and every other identity are asked to close.
    async fn takeover(&mut self, shared: &Arc<Shared>, live: Live) {
        let Some(new) = self.request.take() else {
            return;
        };
        if let Err(error) = switch_current(shared, &new.id).await {
            // The session is used anyway; the next start finds the request with its credential
            // and completes the switch.
            tracing::warn!(%error, "the device in use could not be switched");
        }
        new.is_device.store(true, Ordering::SeqCst);
        let old = self.device.take();
        self.device_stopped = None;
        self.request_stopped = None;
        shared.unavailable.send_replace(ErrorCode::Ar203);
        shared.live.send_replace(Some(live.clone()));
        shared.keep_tools(live.session.tools.as_slice().to_vec());
        #[cfg(feature = "testing")]
        {
            shared.settings.emit(|| crate::testing::Event::Serving {
                device: new.id.to_string(),
            });
            shared.settings.emit(|| crate::testing::Event::Takeover {
                device: new.id.to_string(),
            });
        }
        tracing::info!("the new device took over");
        self.device = Some(new);
        if let Some(old) = old {
            old.stop(duration(shared.settings.shutdown_ms)).await;
        }
        self.cleanup(shared);
    }

    /// Asks the hub to close every identity that is neither the device in use nor the
    /// request followed, and deletes the folder of each one the hub answered for.
    fn cleanup(&mut self, shared: &Arc<Shared>) {
        if self.cleaning {
            self.clean_again = true;
            return;
        }
        self.cleaning = true;
        let shared = Arc::clone(shared);
        let keep: Vec<DeviceId> = [self.device.as_ref(), self.request.as_ref()]
            .into_iter()
            .flatten()
            .map(|slot| slot.id.clone())
            .collect();
        tokio::spawn(async move {
            cleanup(&shared, &keep).await;
            let folders = scan(&shared.home)
                .await
                .ok()
                .map(|layout| layout.folders.len());
            shared.send(Command::CleanupDone { folders });
        });
    }

    /// The request to leave: over each open channel, then by short connections, and the files
    /// are deleted only when every identity was confirmed.
    fn leave(&mut self, shared: &Arc<Shared>, asked: oneshot::Sender<LeaveReport>) {
        if self.leaving {
            let _ = asked.send(LeaveReport {
                left: Vec::new(),
                done: false,
            });
            return;
        }
        self.leaving = true;
        shared.live.send_replace(None);
        let slots: Vec<Slot> = [self.device.take(), self.request.take()]
            .into_iter()
            .flatten()
            .collect();
        let shared = Arc::clone(shared);
        tokio::spawn(async move {
            let report = leave_everything(&shared, slots).await;
            let done = report.done;
            let _ = asked.send(report);
            shared.send(Command::LeaveDone { done });
        });
    }

    fn publish(&self, shared: &Shared) {
        let device = self.device.as_ref().map(Slot::view).or_else(|| {
            self.device_stopped.as_ref().map(|(id, said)| DeviceStatus {
                folder: id.to_string(),
                channel: ChannelStatus::stopped(*said),
            })
        });
        let request = self.request.as_ref().map(Slot::view).or_else(|| {
            self.request_stopped
                .as_ref()
                .map(|(id, said)| DeviceStatus {
                    folder: id.to_string(),
                    channel: ChannelStatus::stopped(*said),
                })
        });
        let keep = [device.as_ref(), request.as_ref()]
            .into_iter()
            .flatten()
            .count();
        let leftovers = self.folders.saturating_sub(keep);
        shared.view.send_replace(View {
            device,
            request,
            leftovers,
        });
    }
}

/// Reads the folder, away from the tasks of the network.
async fn scan(home: &Home) -> Result<store::Layout, store::StoreError> {
    let home = home.clone();
    tokio::task::spawn_blocking(move || store::scan(&home))
        .await
        .map_err(|_| store::StoreError::DamagedCurrent)?
}

async fn record_active(shared: &Shared, id: &DeviceId) {
    let dir = DeviceDir::of(&shared.home, id);
    let Ok(_guard) = store::lock_devices(&shared.home, shared.settings.devices_lock_ms).await
    else {
        return;
    };
    let _ = tokio::task::spawn_blocking(move || dir.set_situation(Situation::Active)).await;
}

/// One rename of `current`, under the lock of the device folders.
async fn switch_current(shared: &Shared, id: &DeviceId) -> Result<(), store::StoreError> {
    let _guard = store::lock_devices(&shared.home, shared.settings.devices_lock_ms)
        .await
        .map_err(|_| store::StoreError::DamagedCurrent)?;
    let home = shared.home.clone();
    let id = id.clone();
    tokio::task::spawn_blocking(move || {
        store::write_current(&home, &id)?;
        DeviceDir::of(&home, &id).set_situation(Situation::Active)
    })
    .await
    .map_err(|_| store::StoreError::DamagedCurrent)?
}

async fn delete(shared: &Shared, id: &DeviceId) -> Result<(), store::StoreError> {
    let _guard = store::lock_devices(&shared.home, shared.settings.devices_lock_ms)
        .await
        .map_err(|_| store::StoreError::DamagedCurrent)?;
    let home = shared.home.clone();
    let removed = id.clone();
    tokio::task::spawn_blocking(move || store::remove_folder(&home, &removed))
        .await
        .map_err(|_| store::StoreError::DamagedCurrent)??;
    #[cfg(feature = "testing")]
    shared.settings.emit(|| crate::testing::Event::Deleted {
        device: id.to_string(),
    });
    Ok(())
}

/// Asks the hub to close one identity by a short connection.
async fn leave_one(shared: &Shared, folder: &store::Folder) -> Result<LeftStatus, Said> {
    let record = folder.record().ok_or(Said::Local(Local::Damaged))?;
    let identity = folder
        .dir
        .identity()
        .map_err(|_| Said::Local(Local::Damaged))?;
    short::leave(&record.address, &record.key, &identity, &shared.settings).await
}

async fn cleanup(shared: &Arc<Shared>, keep: &[DeviceId]) {
    let Ok(layout) = scan(&shared.home).await else {
        return;
    };
    // Only once a device is in use: until then nothing took the place of anyone.
    let Current::Device(current) = &layout.current else {
        return;
    };
    for folder in &layout.folders {
        if folder.id() == current || keep.contains(folder.id()) {
            continue;
        }
        let replaced = folder
            .record()
            .is_some_and(|record| record.situation == Situation::Ended(ErrorCode::Ar503));
        // The hub already said it was replaced; otherwise it is asked, and "closed", "already
        // closed" and "unknown" are all an answer.
        let answered = replaced || leave_one(shared, folder).await.is_ok();
        if answered && let Err(error) = delete(shared, folder.id()).await {
            tracing::warn!(%error, "a folder left over could not be deleted");
        }
    }
}

async fn leave_everything(shared: &Arc<Shared>, slots: Vec<Slot>) -> LeaveReport {
    let patience = duration(shared.settings.shutdown_ms);
    let wait_left = duration(
        shared
            .settings
            .deadlines
            .wait_left_ms
            .saturating_add(shared.settings.shutdown_ms),
    );
    let mut left: Vec<(DeviceId, Result<LeftStatus, Said>)> = Vec::new();
    for slot in &slots {
        if !slot.established() {
            continue;
        }
        let (asker, answer) = oneshot::channel();
        if slot.leave.send(asker).await.is_err() {
            continue;
        }
        if let Ok(Ok(Some(status))) = tokio::time::timeout(wait_left, answer).await {
            left.push((slot.id.clone(), Ok(status)));
        }
    }
    for slot in slots {
        slot.stop(patience).await;
    }
    let layout = match scan(&shared.home).await {
        Ok(layout) => layout,
        Err(_) => {
            return LeaveReport { left, done: false };
        }
    };
    for folder in &layout.folders {
        if left.iter().any(|(id, _)| id == folder.id()) {
            continue;
        }
        let answer = leave_one(shared, folder).await;
        left.push((folder.id().clone(), answer));
    }
    let confirmed = left.iter().all(|(_, answer)| answer.is_ok());
    let done = confirmed && remove_everything(shared).await;
    LeaveReport { left, done }
}

async fn remove_everything(shared: &Shared) -> bool {
    let Ok(_guard) = store::lock_devices(&shared.home, shared.settings.devices_lock_ms).await
    else {
        return false;
    };
    let home = shared.home.clone();
    matches!(
        tokio::task::spawn_blocking(move || store::remove_everything(&home)).await,
        Ok(Ok(()))
    )
}

// ---------------------------------------------------------------------------------- slots

async fn slot(
    shared: Arc<Shared>,
    serial: u64,
    id: DeviceId,
    is_device: Arc<AtomicBool>,
    mut stop: watch::Receiver<bool>,
    mut leave: mpsc::Receiver<LeaveAsk>,
) {
    let tell = |event| shared.send(Command::Slot { serial, event });
    let over = run_slot(&shared, &id, &is_device, &mut stop, &mut leave, &tell).await;
    tell(SlotEvent::Over(over));
}

async fn run_slot(
    shared: &Arc<Shared>,
    id: &DeviceId,
    is_device: &AtomicBool,
    stop: &mut watch::Receiver<bool>,
    leave: &mut mpsc::Receiver<LeaveAsk>,
    tell: &impl Fn(SlotEvent),
) -> Over {
    let saving = Saving {
        home: shared.home.clone(),
        dir: DeviceDir::of(&shared.home, id),
        settings: Arc::clone(&shared.settings),
    };
    let mut backoff = Backoff::new(&shared.settings);
    loop {
        if *stop.borrow() {
            return Over::Asked;
        }
        tell(SlotEvent::Connecting);
        let (news, mut heard) = mpsc::unbounded_channel();
        let outcome = {
            let running = attempt::run(Ctx {
                saving: saving.clone(),
                news: &news,
                leave,
                stop,
            });
            tokio::pin!(running);
            loop {
                tokio::select! {
                    outcome = &mut running => break outcome,
                    Some(news) = heard.recv() => tell(SlotEvent::News(news)),
                }
            }
        };
        drop(news);
        while let Some(news) = heard.recv().await {
            tell(SlotEvent::News(news));
        }
        // Requests to leave that came while no channel was established: none was sent.
        while let Ok(asker) = leave.try_recv() {
            let _ = asker.send(None);
        }
        if outcome.stopped {
            return Over::Asked;
        }
        if outcome.left.is_some() {
            return Over::Left;
        }
        if outcome.problem == Some(Said::Local(Local::Damaged)) {
            return Over::Stopped(Said::Local(Local::Damaged));
        }
        match backoff::next(&outcome.ended) {
            Next::Retry(wait) => {
                tell(SlotEvent::Down(
                    outcome.problem.or(outcome.ended.closing.map(Said::code)),
                ));
                let wait = backoff.wait(&outcome.ended, wait);
                #[cfg(feature = "testing")]
                shared.settings.emit(|| crate::testing::Event::Waiting {
                    device: id.to_string(),
                    ms: u64::try_from(wait.as_millis()).unwrap_or(u64::MAX),
                });
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    () = stopped(stop) => return Over::Asked,
                }
            }
            Next::Stop(code @ (ErrorCode::Ar401 | ErrorCode::Ar402)) => {
                // The version does not agree: presented again later (decision 0030).
                tell(SlotEvent::Paused(code));
                tokio::select! {
                    () = tokio::time::sleep(backoff::version_retry(&shared.settings)) => {}
                    () = stopped(stop) => return Over::Asked,
                }
            }
            Next::Stop(code) => return stop_slot(shared, id, is_device, code).await,
        }
    }
}

/// The hub told the device to stop: what is kept, what is discarded, what is said.
async fn stop_slot(
    shared: &Shared,
    id: &DeviceId,
    is_device: &AtomicBool,
    code: ErrorCode,
) -> Over {
    tracing::warn!(code = code.as_str(), "the hub ended this device");
    let Ok(_guard) = store::lock_devices(&shared.home, shared.settings.devices_lock_ms).await
    else {
        return Over::Stopped(Said::code(code));
    };
    let home = shared.home.clone();
    let device = id.clone();
    let request = !is_device.load(Ordering::SeqCst);
    let kept = tokio::task::spawn_blocking(move || {
        // A request refused, denied or expired is discarded, but only while it has no
        // credential (docs/11-conector.md, section 2).
        if request
            && matches!(code, ErrorCode::Ar301 | ErrorCode::Ar302 | ErrorCode::Ar303)
            && store::discard_request(&home, &device)?
        {
            return Ok::<bool, store::StoreError>(false);
        }
        DeviceDir::of(&home, &device).set_situation(Situation::Ended(code))?;
        Ok(true)
    })
    .await;
    match kept {
        Ok(Ok(false)) => {
            #[cfg(feature = "testing")]
            shared.settings.emit(|| crate::testing::Event::Deleted {
                device: id.to_string(),
            });
            Over::Discarded
        }
        _ if code == ErrorCode::Ar503 => Over::Replaced,
        _ => Over::Stopped(Said::code(code)),
    }
}

// ---------------------------------------------------------------------------------- socket

async fn serve(shared: Arc<Shared>, mut listener: LocalListener) {
    let connections = Arc::new(Semaphore::new(LOCAL_CONNECTIONS_MAX));
    let mut finished = shared.finished.subscribe();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    // Above the ceiling, the connection is dropped at once.
                    if let Ok(permit) = Arc::clone(&connections).try_acquire_owned() {
                        let shared = Arc::clone(&shared);
                        tokio::spawn(async move {
                            client(shared, stream).await;
                            drop(permit);
                        });
                    }
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            },
            changed = finished.changed() => {
                if changed.is_err() || finished.borrow().is_some() {
                    return;
                }
            }
        }
    }
}

/// One connection of the local socket: lines in, lines out.
async fn client(shared: Arc<Shared>, stream: LocalStream) {
    let (reader, mut writer) = tokio::io::split(stream);
    let (out, mut lines_out) = mpsc::channel::<Vec<u8>>(64);
    let writing = tokio::spawn(async move {
        while let Some(line) = lines_out.recv().await {
            if writer.write_all(&line).await.is_err() || writer.flush().await.is_err() {
                return;
            }
        }
    });
    let mut lines = Lines::new(BufReader::new(reader), local::REQUEST_LINE_MAX);
    let calls = Arc::new(Semaphore::new(CALLS_IN_FLIGHT_MAX));
    let mut finished = shared.finished.subscribe();
    let mut watching: Option<JoinHandle<()>> = None;
    loop {
        let line = tokio::select! {
            line = lines.next() => line,
            _ = finished.wait_for(Option::is_some) => break,
        };
        let Ok(Some(line)) = line else {
            break;
        };
        let send = |value: Value| {
            let out = out.clone();
            async move {
                let _ = out.send(local::line(&value)).await;
            }
        };
        match local::parse_request(&line) {
            Err((id, refusal)) => send(local::refusal(id, refusal)).await,
            Ok(Envelope { id, request }) => match request {
                Request::Call { tool, args } => {
                    let Some(id) = id else {
                        continue;
                    };
                    match Arc::clone(&calls).try_acquire_owned() {
                        Ok(permit) => {
                            let shared = Arc::clone(&shared);
                            let out = out.clone();
                            tokio::spawn(async move {
                                let outcome = shared.call(tool, args).await;
                                let _ = out
                                    .send(local::line(&local::call_answer(id, &outcome)))
                                    .await;
                                drop(permit);
                            });
                        }
                        Err(_) => {
                            send(local::call_answer(id, &refused(ErrorCode::Ar710))).await;
                        }
                    }
                }
                Request::Tools => {
                    let tools = shared.tools.borrow().clone();
                    send(local::tools_answer(id, &tools, true)).await;
                }
                Request::Watch => {
                    if watching.is_none() {
                        let mut tools = shared.tools.subscribe();
                        let out = out.clone();
                        watching = Some(tokio::spawn(async move {
                            let first = tools.borrow_and_update().clone();
                            if out
                                .send(local::line(&local::tools_answer(id, &first, true)))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            while tools.changed().await.is_ok() {
                                let now = tools.borrow_and_update().clone();
                                if out
                                    .send(local::line(&local::tools_answer(None, &now, false)))
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }));
                    }
                }
                Request::Status => {
                    send(local::answer(id, "status", shared.status().to_json())).await
                }
                Request::FolderChanged => {
                    shared.send(Command::Rescan);
                    send(local::done(id)).await;
                }
                Request::Leave => {
                    let report = shared.leave().await;
                    send(leave_answer(id, report.as_ref())).await;
                }
                Request::Restart => {
                    send(local::done(id)).await;
                    shared.send(Command::Finish(Finish::Restart));
                }
            },
        }
    }
    if let Some(watching) = watching {
        watching.abort();
        let _ = watching.await;
    }
    // A restart ends every copy that forwards here too: Hermes starts each one again, on the
    // new program, and none of the old ones takes the place of the leader.
    if *shared.finished.borrow() == Some(Finish::Restart) {
        let _ = out.send(local::line(&local::restarting())).await;
    }
    drop(out);
    let _ = writing.await;
}

fn leave_answer(id: Option<u64>, report: Option<&LeaveReport>) -> Value {
    let Some(report) = report else {
        return local::answer(id, "left", json!([]));
    };
    let left: Vec<Value> = report
        .left
        .iter()
        .map(|(device, answer)| {
            let outcome = match answer {
                Ok(LeftStatus::Closed) => "closed",
                Ok(LeftStatus::AlreadyClosed) => "already_closed",
                Ok(LeftStatus::Unknown) => "unknown",
                Err(_) => "pending",
            };
            json!({"device": device.as_str(), "outcome": outcome})
        })
        .collect();
    let mut answer = local::answer(id, "left", Value::Array(left));
    if let Value::Object(object) = &mut answer {
        object.insert("done".to_owned(), Value::Bool(report.done));
    }
    answer
}
