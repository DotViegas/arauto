//! What the tests of the connector share: the folder of a connector, its files read back, the
//! settings of a test with its events and its answers of Hermes, and the steps that bring a
//! connector to the session against the hub of a fixture.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arauto_connector::catalog::Said;
use arauto_connector::testing::{self, Attempted, Event, Faults, Point};
use arauto_connector::{Connector, Hermes, Home, JoinArgs, Joined, Settings};
use arauto_hub::repo::approval::ApproveOutcome;
use arauto_protocol::clock::{Clock, SystemClock};
use arauto_protocol::invite::Invite;
use arauto_protocol::noise::{PublicKey, StaticKeypair};
use arauto_protocol::secret::Secret;
use arauto_testkit::hub::fixture::Fixture;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use zeroize::Zeroizing;

/// Patience for what the connector does at once.
pub const PATIENCE: Duration = Duration::from_secs(20);

/// The folder of one connector.
pub struct Folder {
    _dir: tempfile::TempDir,
    pub home: Home,
}

impl Folder {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(&dir.path().join("arauto")).unwrap();
        Self { _dir: dir, home }
    }

    pub fn root(&self) -> PathBuf {
        self.home.root().to_path_buf()
    }

    /// The device folders, by name.
    pub fn devices(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.root().join("devices"))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| !name.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// The only device folder.
    pub fn only_device(&self) -> String {
        let devices = self.devices();
        assert_eq!(devices.len(), 1, "{devices:?}");
        devices[0].clone()
    }

    /// What `current` names.
    pub fn current(&self) -> Option<String> {
        std::fs::read_to_string(self.root().join("current"))
            .ok()
            .map(|text| text.trim().to_owned())
    }

    pub fn device_path(&self, device: &str) -> PathBuf {
        self.root().join("devices").join(device)
    }

    /// The identity of a device folder.
    pub fn identity(&self, device: &str) -> StaticKeypair {
        let bytes = std::fs::read(self.device_path(device).join("identity.key")).unwrap();
        StaticKeypair::from_private(Secret::from_slice(&bytes).unwrap())
    }

    /// The public identity of a device folder.
    pub fn public(&self, device: &str) -> PublicKey {
        *self.identity(device).public()
    }

    /// The credential of a device folder, as it is on disk.
    pub fn credential(&self, device: &str) -> Option<Vec<u8>> {
        std::fs::read(self.device_path(device).join("credential.seal")).ok()
    }

    pub fn hub_json(&self, device: &str) -> Value {
        let bytes = std::fs::read(self.device_path(device).join("hub.json")).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// A copy of every file, as another machine would have it.
    pub fn copy(&self) -> Folder {
        let copy = Folder::new();
        copy.home.prepare().unwrap();
        for device in self.devices() {
            let to = copy.device_path(&device);
            std::fs::create_dir_all(&to).unwrap();
            for entry in std::fs::read_dir(self.device_path(&device))
                .unwrap()
                .flatten()
            {
                std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
        for file in ["current", "state.json"] {
            let from = self.root().join(file);
            if from.exists() {
                std::fs::copy(from, copy.root().join(file)).unwrap();
            }
        }
        copy
    }
}

/// The events of a connector, kept as they come.
pub struct Watch {
    events: UnboundedReceiver<Event>,
    pub seen: Vec<Event>,
}

impl Watch {
    /// Waits for an event that `wanted` accepts, and returns it.
    pub async fn until(
        &mut self,
        within: Duration,
        mut wanted: impl FnMut(&Event) -> bool,
    ) -> Event {
        if let Some(found) = self.seen.iter().find(|event| wanted(event)) {
            return found.clone();
        }
        let deadline = tokio::time::Instant::now() + within;
        loop {
            let event = tokio::time::timeout_at(deadline, self.events.recv())
                .await
                .unwrap_or_else(|_| panic!("the event did not come; seen: {:#?}", self.seen))
                .unwrap();
            self.seen.push(event.clone());
            if wanted(&event) {
                return event;
            }
        }
    }

    /// Forgets what was seen: the next `until` looks only at what comes from now on, and at
    /// what came but was not read yet.
    pub fn forget(&mut self) {
        self.seen.clear();
    }

    /// Takes what came so far without waiting.
    pub fn drain(&mut self) -> &[Event] {
        while let Ok(event) = self.events.try_recv() {
            self.seen.push(event);
        }
        &self.seen
    }
}

/// What a test answers when the connector runs the command of Hermes, and what it ran.
#[derive(Clone, Default)]
pub struct Registrations {
    pub calls: Arc<Mutex<Vec<Vec<String>>>>,
    pub refuse: Arc<std::sync::atomic::AtomicBool>,
}

impl Registrations {
    pub fn all(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    pub fn refusing(&self, refuse: bool) {
        self.refuse
            .store(refuse, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The settings of a test: short waits, the events, and the answers of Hermes.
pub fn settings() -> (Settings, Watch, Registrations) {
    let (sender, events) = unbounded_channel();
    let registrations = Registrations::default();
    let recorded = registrations.clone();
    let settings =
        Settings::for_tests()
            .with_events(sender)
            .with_hermes(move |arguments: &[OsString]| {
                recorded.calls.lock().unwrap().push(
                    arguments
                        .iter()
                        .map(|argument| argument.to_string_lossy().into_owned())
                        .collect(),
                );
                !recorded.refuse.load(std::sync::atomic::Ordering::SeqCst)
            });
    (
        settings,
        Watch {
            events,
            seen: Vec::new(),
        },
        registrations,
    )
}

/// A small file that stands for the program the connector copies to its fixed place.
pub fn program() -> PathBuf {
    let dir = tempfile::tempdir().unwrap().keep();
    let path = dir.join("arauto-connector-download");
    std::fs::write(&path, b"a program for the tests").unwrap();
    path
}

pub fn now() -> i64 {
    SystemClock.now_ms()
}

/// `join` with the block of the hub of `fixture` and `invite`.
pub async fn join(
    fixture: &Fixture,
    folder: &Folder,
    invite: &Invite,
    settings: &Settings,
) -> Result<Joined, Said> {
    join_with(
        folder,
        &fixture.channel_url(),
        &fixture.hub_key_text(),
        &invite.reveal(),
        settings,
    )
    .await
}

/// `join` with any block.
pub async fn join_with(
    folder: &Folder,
    hub: &str,
    key: &str,
    invite: &str,
    settings: &Settings,
) -> Result<Joined, Said> {
    let args = JoinArgs {
        hub: hub.to_owned(),
        key: key.to_owned(),
        invite: Zeroizing::new(invite.to_owned()),
        register: true,
        program: program(),
    };
    arauto_connector::join(&folder.home, args, &Hermes::default(), settings, now()).await
}

/// The code of a `join` that worked.
pub fn code_of(joined: Result<Joined, Said>) -> String {
    match joined {
        Ok(Joined::Enrolled { code, .. }) => code.to_string(),
        other => panic!("the join did not enroll: {other:?}"),
    }
}

/// `admin approve`, which must approve.
pub async fn approve(fixture: &Fixture, name: &str, code: &str) {
    let outcome = fixture.approve(name, code).await;
    assert!(
        matches!(outcome, ApproveOutcome::Approved(_)),
        "{outcome:?}"
    );
}

/// A Hermes whose connector joined and was approved, at generation zero, and its folder.
pub async fn approved(fixture: &Fixture, settings: &Settings) -> (String, Folder) {
    let name = fixture.hermes().await;
    let folder = Folder::new();
    let invite = fixture.invite(&name).await;
    let code = code_of(join(fixture, &folder, &invite, settings).await);
    approve(fixture, &name, &code).await;
    (name, folder)
}

/// A Hermes connected end to end: joined, approved, and its leader in session.
pub async fn connected(
    fixture: &Fixture,
    settings: Settings,
    watch: &mut Watch,
) -> (String, Folder, Connector) {
    let (name, folder) = approved(fixture, &settings).await;
    let connector = Connector::start(folder.home.clone(), settings)
        .await
        .unwrap();
    watch
        .until(PATIENCE, |event| matches!(event, Event::Takeover { .. }))
        .await;
    (name, folder, connector)
}

/// Polls `condition` until it holds, or fails the test.
pub async fn eventually<F, Fut>(what: &str, within: Duration, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let until = tokio::time::Instant::now() + within;
    loop {
        if condition().await {
            return;
        }
        assert!(tokio::time::Instant::now() < until, "never: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Whether a call of `arauto_status` works and says connected.
pub async fn status_works(connector: &Connector) -> bool {
    match connector
        .call("arauto_status", serde_json::Map::new())
        .await
    {
        arauto_protocol::message::CallOutcome::Ok(content) => {
            content.as_map().get("connected") == Some(&Value::Bool(true))
        }
        arauto_protocol::message::CallOutcome::Refused { .. } => false,
    }
}

/// The code a call of `arauto_status` gets, when refused.
pub async fn refusal(connector: &Connector) -> Option<arauto_protocol::errors::ErrorCode> {
    match connector
        .call("arauto_status", serde_json::Map::new())
        .await
    {
        arauto_protocol::message::CallOutcome::Ok(_) => None,
        arauto_protocol::message::CallOutcome::Refused { code, .. } => Some(code),
    }
}

/// Fails once, at one point.
pub struct Once {
    point: Point,
    done: std::sync::atomic::AtomicBool,
}

impl Once {
    pub fn at(point: Point) -> Arc<Self> {
        Arc::new(Self {
            point,
            done: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

impl Faults for Once {
    fn fails(&self, _device: &str, point: Point) -> bool {
        point == self.point && !self.done.swap(true, std::sync::atomic::Ordering::SeqCst)
    }
}

/// Whether the connector tries again after this end, without anything being wrong.
pub fn retried(attempted: &Attempted) -> bool {
    use arauto_protocol::errors::ErrorCode;
    match attempted {
        Attempted::Closed(closing) | Attempted::Session(closing) => matches!(
            closing,
            None | Some(ErrorCode::Ar204 | ErrorCode::Ar205 | ErrorCode::Ar206 | ErrorCode::Ar208)
        ),
        Attempted::Faulted(_) | Attempted::Failed(_) => true,
    }
}

/// Clean attempts of the device of `folder`, tried again while the hub answers with what the
/// connector retries, until one reaches the session. `Err` says how it was locked out.
pub async fn clean(
    folder: &Folder,
    device: &str,
    settings: &Settings,
) -> Result<Attempted, String> {
    let mut seen = Vec::new();
    for _ in 0..20 {
        let attempted = testing::attempt(&folder.home, device, settings, Duration::from_millis(30))
            .await
            .map_err(|error| error.to_string())?;
        if matches!(attempted, Attempted::Session(_)) {
            return Ok(attempted);
        }
        if !retried(&attempted) {
            return Ok(attempted);
        }
        seen.push(attempted);
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    Err(format!("no session after 20 clean attempts: {seen:?}"))
}
