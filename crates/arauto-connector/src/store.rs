//! The files of the folder: the device folders, `current` and `state.json`
//! (`docs/11-conector.md`, section 2).
//!
//! - Every write is durable (`arauto_platform::write_durable`): temporary file, forced to disk,
//!   renamed, folder forced to disk. The switch of device is one rename of `current`.
//! - A file that is damaged or of an unknown format is an error that says so, never a crash
//!   and never a reason to delete anything.
//! - The secrets (`identity.key`, `credential.seal`) are read into buffers that are wiped, and
//!   no error, `Debug` or log line carries their content.
//! - A folder is deleted by renaming it out of the way first: what is left of a deletion that
//!   was interrupted never looks like a device, and the next start removes it.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use arauto_platform::{FileLock, FsError, LockError, create_private_dir, write_durable};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::invite::InviteDigest;
use arauto_protocol::limits::{CREDENTIAL_MAX_LEN, DIGEST_LEN, KEY_LEN, TOOL_LIST_MAX};
use arauto_protocol::message::read_strict;
use arauto_protocol::noise::{PublicKey, StaticKeypair};
use arauto_protocol::secret::{SealedCredential, Secret};
use arauto_protocol::tools::ToolName;
use serde_json::{Map, Value, json};
use zeroize::Zeroizing;

use crate::address::HubAddress;
use crate::home::Home;

/// The identity of a device.
pub(crate) const IDENTITY_FILE: &str = "identity.key";

/// The latest credential the hub sent.
pub(crate) const CREDENTIAL_FILE: &str = "credential.seal";

/// The hub of the device and its local situation.
pub(crate) const HUB_FILE: &str = "hub.json";

/// What the name of a folder that is being deleted starts with.
const DISCARDED_PREFIX: &str = ".discarded-";

/// The format of `hub.json` and `state.json`.
const FORMAT_VERSION: u64 = 1;

/// The largest `hub.json`.
const HUB_FILE_MAX: usize = 4_096;

/// The largest `state.json`.
const STATE_FILE_MAX: usize = 64 * 1_024;

/// The largest `current`.
const CURRENT_FILE_MAX: usize = 64;

/// How often a lock that is held is tried again.
const LOCK_RETRY: Duration = Duration::from_millis(20);

/// What went wrong with the files of the folder. No variant carries a secret.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A file of a device folder is damaged or of an unknown format.
    #[error("the file {file} of the device folder {device} is damaged or has an unknown format")]
    Damaged {
        /// The folder.
        device: String,
        /// Which file.
        file: &'static str,
    },
    /// `current` is damaged, or names no device folder.
    #[error("the file current is damaged or names no device folder")]
    DamagedCurrent,
    /// The device folder does not exist.
    #[error("the device folder {0} does not exist")]
    NoSuchDevice(String),
    /// A call to the system failed.
    #[error("could not {action} {}: {cause}", .path.display())]
    Io {
        /// What was being done.
        action: &'static str,
        /// Where.
        path: PathBuf,
        /// What the system said.
        cause: io::Error,
    },
    /// A write or a folder of the platform layer.
    #[error(transparent)]
    Fs(#[from] FsError),
    /// The generator of the system failed.
    #[error("the random generator of the system failed")]
    Random,
}

impl StoreError {
    fn io(action: &'static str, path: &Path, cause: io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            cause,
        }
    }

    /// Whether the error is a file that is damaged or of an unknown format.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn is_damaged(&self) -> bool {
        matches!(self, Self::Damaged { .. } | Self::DamagedCurrent)
    }
}

// ---------------------------------------------------------------------------------- reading

/// Reads at most `cap` bytes of `path`, into a buffer that is wiped when dropped. `None` when
/// the file does not exist; `Err(true)` when it holds more than `cap` bytes.
fn read_capped(path: &Path, cap: usize) -> Result<Option<Zeroizing<Vec<u8>>>, CapError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(CapError::Io(cause)),
    };
    let mut bytes = Zeroizing::new(Vec::new());
    let limit = u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1);
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(CapError::Io)?;
    if bytes.len() > cap {
        return Err(CapError::TooLong);
    }
    Ok(Some(bytes))
}

enum CapError {
    Io(io::Error),
    TooLong,
}

// ---------------------------------------------------------------------------------- devices

/// The name of a device folder: a UUID drawn by the generator of the system, in its one
/// written form. It is not a secret.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(String);

impl DeviceId {
    /// A new name, from the generator of the system.
    pub(crate) fn generate() -> Result<Self, StoreError> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| StoreError::Random)?;
        let uuid = uuid::Builder::from_random_bytes(bytes).into_uuid();
        Ok(Self(uuid.as_hyphenated().to_string()))
    }

    /// Reads a name in its one written form: a UUID, hyphenated, lowercase.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let uuid = uuid::Uuid::parse_str(text).ok()?;
        let written = uuid.as_hyphenated().to_string();
        (written == text).then_some(Self(written))
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the connector knows of a device, kept in `hub.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situation {
    /// Enrolled, without a session yet.
    Pending,
    /// It had a session.
    Active,
    /// The hub said the device ended, with this code: the channel is not used for it.
    Ended(ErrorCode),
}

impl Situation {
    fn name(self) -> &'static str {
        match self {
            Situation::Pending => "pending",
            Situation::Active => "active",
            Situation::Ended(_) => "ended",
        }
    }
}

/// The contents of `hub.json`: not a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HubRecord {
    /// Where the hub is.
    pub(crate) address: HubAddress,
    /// The public key of the hub.
    pub(crate) key: PublicKey,
    /// The digest of the invite of the request that created the device: how the same invite
    /// is recognized when `join` runs again.
    pub(crate) invite: InviteDigest,
    /// When the folder was created: the newest request is the one followed.
    pub(crate) created_at: i64,
    /// What the connector knows of the device.
    pub(crate) situation: Situation,
    /// Whether `join` registered the connector in Hermes for this request.
    pub(crate) registered: bool,
}

impl HubRecord {
    /// Whether the device is of the hub of `address` and `key`. The same address with another
    /// key is another hub.
    pub(crate) fn same_hub(&self, address: &HubAddress, key: &PublicKey) -> bool {
        self.address == *address && self.key == *key
    }

    fn to_json(&self) -> Value {
        let code = match self.situation {
            Situation::Ended(code) => Value::from(code.as_str()),
            Situation::Pending | Situation::Active => Value::Null,
        };
        json!({
            "v": FORMAT_VERSION,
            "hub": self.address.url(),
            "hub_key": self.key.to_text(),
            "invite": data_encoding::BASE64.encode(self.invite.as_bytes()),
            "created_at": self.created_at,
            "situation": self.situation.name(),
            "code": code,
            "registered": self.registered,
        })
    }

    fn from_json(bytes: &[u8]) -> Option<Self> {
        let Value::Object(object) = read_strict(bytes).ok()? else {
            return None;
        };
        if object.get("v")?.as_u64()? != FORMAT_VERSION {
            return None;
        }
        let text = |key: &str| object.get(key).and_then(Value::as_str);
        let address = HubAddress::parse(text("hub")?).ok()?;
        let key = PublicKey::parse_text(text("hub_key")?).ok()?;
        let invite = data_encoding::BASE64
            .decode(text("invite")?.as_bytes())
            .ok()?;
        let invite = InviteDigest::from_bytes(<[u8; DIGEST_LEN]>::try_from(invite).ok()?);
        let created_at = object.get("created_at")?.as_i64()?;
        let situation = match (text("situation")?, object.get("code")) {
            ("pending", Some(Value::Null) | None) => Situation::Pending,
            ("active", Some(Value::Null) | None) => Situation::Active,
            ("ended", Some(Value::String(code))) => Situation::Ended(ErrorCode::parse(code)?),
            _ => return None,
        };
        let registered = object.get("registered")?.as_bool()?;
        Some(Self {
            address,
            key,
            invite,
            created_at,
            situation,
            registered,
        })
    }
}

/// A device folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeviceDir {
    id: DeviceId,
    path: PathBuf,
}

impl DeviceDir {
    pub(crate) fn of(home: &Home, id: &DeviceId) -> Self {
        Self {
            id: id.clone(),
            path: home.devices().join(id.as_str()),
        }
    }

    pub(crate) fn id(&self) -> &DeviceId {
        &self.id
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn damaged(&self, file: &'static str) -> StoreError {
        StoreError::Damaged {
            device: self.id.to_string(),
            file,
        }
    }

    /// Whether the folder still exists.
    pub(crate) fn exists(&self) -> bool {
        self.path.is_dir()
    }

    /// The identity of the device.
    pub(crate) fn identity(&self) -> Result<StaticKeypair, StoreError> {
        let path = self.file(IDENTITY_FILE);
        match read_capped(&path, KEY_LEN) {
            Ok(Some(bytes)) => Secret::<KEY_LEN>::from_slice(&bytes)
                .map(StaticKeypair::from_private)
                .ok_or_else(|| self.damaged(IDENTITY_FILE)),
            Ok(None) | Err(CapError::TooLong) => Err(self.damaged(IDENTITY_FILE)),
            Err(CapError::Io(cause)) => Err(StoreError::io("read", &path, cause)),
        }
    }

    /// The latest credential, if the hub sent one.
    pub(crate) fn credential(&self) -> Result<Option<SealedCredential>, StoreError> {
        let path = self.file(CREDENTIAL_FILE);
        match read_capped(&path, CREDENTIAL_MAX_LEN) {
            Ok(Some(bytes)) => SealedCredential::from_slice(&bytes)
                .map(Some)
                .map_err(|_| self.damaged(CREDENTIAL_FILE)),
            Ok(None) => Ok(None),
            Err(CapError::TooLong) => Err(self.damaged(CREDENTIAL_FILE)),
            Err(CapError::Io(cause)) => Err(StoreError::io("read", &path, cause)),
        }
    }

    /// Whether the folder holds a credential, readable or not.
    pub(crate) fn has_credential(&self) -> bool {
        self.file(CREDENTIAL_FILE).exists()
    }

    /// `hub.json`.
    pub(crate) fn record(&self) -> Result<HubRecord, StoreError> {
        let path = self.file(HUB_FILE);
        match read_capped(&path, HUB_FILE_MAX) {
            Ok(Some(bytes)) => HubRecord::from_json(&bytes).ok_or_else(|| self.damaged(HUB_FILE)),
            Ok(None) | Err(CapError::TooLong) => Err(self.damaged(HUB_FILE)),
            Err(CapError::Io(cause)) => Err(StoreError::io("read", &path, cause)),
        }
    }

    /// Writes the credential, durably. Only once this returns `Ok` may the connector say
    /// "saved" (C7, rule 5).
    pub(crate) fn write_credential(&self, credential: &SealedCredential) -> Result<(), FsError> {
        write_durable(&self.file(CREDENTIAL_FILE), credential.expose())
    }

    /// Writes `hub.json`, durably.
    pub(crate) fn write_record(&self, record: &HubRecord) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec(&record.to_json()).map_err(|_| self.damaged(HUB_FILE))?;
        Ok(write_durable(&self.file(HUB_FILE), &bytes)?)
    }

    /// Changes the local situation of the device in `hub.json`.
    pub(crate) fn set_situation(&self, situation: Situation) -> Result<(), StoreError> {
        let mut record = self.record()?;
        if record.situation == situation {
            return Ok(());
        }
        record.situation = situation;
        self.write_record(&record)
    }
}

/// Creates the folder of a new request: a new identity, drawn from the generator of the
/// system, and `hub.json`. What was half created is removed on failure.
pub(crate) fn create_request(
    home: &Home,
    address: &HubAddress,
    key: &PublicKey,
    invite: InviteDigest,
    now: i64,
) -> Result<DeviceDir, StoreError> {
    let id = DeviceId::generate()?;
    let dir = DeviceDir::of(home, &id);
    create_private_dir(dir.path())?;
    let written = (|| {
        let identity = StaticKeypair::generate().map_err(|_| StoreError::Random)?;
        write_durable(&dir.file(IDENTITY_FILE), identity.private().expose())?;
        dir.write_record(&HubRecord {
            address: address.clone(),
            key: *key,
            invite,
            created_at: now,
            situation: Situation::Pending,
            registered: false,
        })
    })();
    if let Err(error) = written {
        let _ = remove_folder(home, &id);
        return Err(error);
    }
    Ok(dir)
}

/// One device folder, as found.
#[derive(Debug, Clone)]
pub(crate) struct Folder {
    pub(crate) dir: DeviceDir,
    /// `hub.json`, or why it could not be read.
    pub(crate) record: Result<HubRecord, &'static str>,
    /// Whether `credential.seal` exists.
    pub(crate) has_credential: bool,
}

impl Folder {
    pub(crate) fn id(&self) -> &DeviceId {
        self.dir.id()
    }

    pub(crate) fn record(&self) -> Option<&HubRecord> {
        self.record.as_ref().ok()
    }
}

/// What `current` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Current {
    /// No device in use.
    Absent,
    /// This device folder is in use.
    Device(DeviceId),
    /// The file is damaged, or names a folder that does not exist.
    Damaged,
}

/// What the folder holds.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub(crate) current: Current,
    /// Every device folder, the oldest first.
    pub(crate) folders: Vec<Folder>,
}

impl Layout {
    pub(crate) fn folder(&self, id: &DeviceId) -> Option<&Folder> {
        self.folders.iter().find(|folder| folder.id() == id)
    }

    /// The device in use, when `current` names one.
    pub(crate) fn current(&self) -> Option<&Folder> {
        match &self.current {
            Current::Device(id) => self.folder(id),
            Current::Absent | Current::Damaged => None,
        }
    }

    /// The request the leader follows: the newest pending folder that is not in use.
    pub(crate) fn request(&self) -> Option<&Folder> {
        let current = match &self.current {
            Current::Device(id) => Some(id),
            Current::Absent | Current::Damaged => None,
        };
        self.folders
            .iter()
            .filter(|folder| Some(folder.id()) != current)
            .filter(|folder| {
                folder
                    .record()
                    .is_some_and(|record| record.situation == Situation::Pending)
            })
            .max_by_key(|folder| folder.record().map_or(i64::MIN, |record| record.created_at))
    }
}

/// Reads what the folder holds. Damaged files are reported in the layout, not as an error.
pub(crate) fn scan(home: &Home) -> Result<Layout, StoreError> {
    let devices = home.devices();
    let mut folders = Vec::new();
    let entries = match fs::read_dir(&devices) {
        Ok(entries) => Some(entries),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => None,
        Err(cause) => return Err(StoreError::io("list", &devices, cause)),
    };
    for entry in entries.into_iter().flatten() {
        let entry = entry.map_err(|cause| StoreError::io("list", &devices, cause))?;
        let name = entry.file_name();
        // Only device folders: what is being deleted and the leftovers of writes are not.
        let Some(id) = name.to_str().and_then(DeviceId::parse) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let dir = DeviceDir::of(home, &id);
        let record = match dir.record() {
            Ok(record) => match dir.identity() {
                Ok(_) => Ok(record),
                Err(_) => Err(IDENTITY_FILE),
            },
            Err(_) => Err(HUB_FILE),
        };
        let has_credential = dir.has_credential();
        folders.push(Folder {
            dir,
            record,
            has_credential,
        });
    }
    folders.sort_by_key(|folder| {
        (
            folder.record().map_or(i64::MIN, |record| record.created_at),
            folder.id().clone(),
        )
    });
    let current = match read_capped(&home.current_file(), CURRENT_FILE_MAX) {
        Ok(None) => Current::Absent,
        Ok(Some(bytes)) => std::str::from_utf8(&bytes)
            .ok()
            .and_then(|text| DeviceId::parse(text.trim()))
            .filter(|id| folders.iter().any(|folder| folder.id() == id))
            .map_or(Current::Damaged, Current::Device),
        Err(CapError::TooLong) => Current::Damaged,
        Err(CapError::Io(cause)) => {
            return Err(StoreError::io("read", &home.current_file(), cause));
        }
    };
    Ok(Layout { current, folders })
}

/// Makes `id` the device in use: one durable rename of `current`.
pub(crate) fn write_current(home: &Home, id: &DeviceId) -> Result<(), StoreError> {
    Ok(write_durable(&home.current_file(), id.as_str().as_bytes())?)
}

/// Forgets the device in use.
pub(crate) fn remove_current(home: &Home) -> Result<(), StoreError> {
    remove_file(&home.current_file())
}

fn remove_file(path: &Path) -> Result<(), StoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(StoreError::io("remove", path, cause)),
    }
}

/// Removes a device folder: it is renamed out of the way first, so that what an interruption
/// leaves never looks like a device.
pub(crate) fn remove_folder(home: &Home, id: &DeviceId) -> Result<(), StoreError> {
    let from = home.devices().join(id.as_str());
    let mut suffix = [0u8; 8];
    getrandom::fill(&mut suffix).map_err(|_| StoreError::Random)?;
    let away = home.devices().join(format!(
        "{DISCARDED_PREFIX}{id}-{}",
        data_encoding::HEXLOWER.encode(&suffix)
    ));
    match fs::rename(&from, &away) {
        Ok(()) => {}
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(StoreError::io("move away", &from, cause)),
    }
    fs::remove_dir_all(&away).map_err(|cause| StoreError::io("remove", &away, cause))
}

/// Discards the folder of a request that has no credential, the only folder the connector
/// discards because of what the hub said (`docs/11-conector.md`, section 2). Returns `false`,
/// and keeps it, when it holds `credential.seal`. Call it holding the lock of the device
/// folders: then no credential can arrive between the check and the removal.
pub(crate) fn discard_request(home: &Home, id: &DeviceId) -> Result<bool, StoreError> {
    if DeviceDir::of(home, id).has_credential() {
        return Ok(false);
    }
    remove_folder(home, id)?;
    Ok(true)
}

/// Removes what interrupted deletions and writes left: folders being deleted, and temporary
/// files in the folder, in the folder of the devices and in each device folder. Call it holding
/// the lock of the device folders, with no write of this program under way.
pub(crate) fn clean_leftovers(home: &Home) -> Result<(), StoreError> {
    let devices = home.devices();
    arauto_platform::remove_stale_temporaries(home.root())?;
    if !devices.is_dir() {
        return Ok(());
    }
    arauto_platform::remove_stale_temporaries(&devices)?;
    let entries =
        fs::read_dir(&devices).map_err(|cause| StoreError::io("list", &devices, cause))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        if name.starts_with(DISCARDED_PREFIX) {
            let _ = fs::remove_dir_all(entry.path());
        } else if DeviceId::parse(name).is_some() {
            arauto_platform::remove_stale_temporaries(&entry.path())?;
        }
    }
    Ok(())
}

/// Removes every device folder, `current` and `state.json`: what `leave` deletes. The
/// program, the locks and the socket stay.
pub(crate) fn remove_everything(home: &Home) -> Result<(), StoreError> {
    let layout = scan(home)?;
    remove_current(home)?;
    for folder in &layout.folders {
        remove_folder(home, folder.id())?;
    }
    remove_file(&home.state_file())?;
    remove_file(&home.wakes_file())?;
    clean_leftovers(home)
}

// ---------------------------------------------------------------------------------- state

/// What `state.json` keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct State {
    /// The tools of the last session.
    pub(crate) tools: Vec<ToolName>,
}

impl State {
    fn from_json(bytes: &[u8]) -> Option<Self> {
        let Value::Object(object) = read_strict(bytes).ok()? else {
            return None;
        };
        if object.get("v")?.as_u64()? != FORMAT_VERSION {
            return None;
        }
        let names = object.get("tools")?.as_array()?;
        if names.len() > TOOL_LIST_MAX {
            return None;
        }
        let tools = names
            .iter()
            .map(|name| name.as_str().and_then(|name| ToolName::parse(name).ok()))
            .collect::<Option<Vec<_>>>()?;
        Some(Self { tools })
    }

    fn to_json(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|tool| Value::from(tool.as_str()))
            .collect();
        let mut object = Map::new();
        object.insert("v".to_owned(), Value::from(FORMAT_VERSION));
        object.insert("tools".to_owned(), Value::Array(tools));
        Value::Object(object)
    }
}

/// Reads `state.json`. A file that is missing gives the empty state; one that is damaged is an
/// error.
pub(crate) fn read_state(home: &Home) -> Result<State, StoreError> {
    let path = home.state_file();
    match read_capped(&path, STATE_FILE_MAX) {
        Ok(None) => Ok(State::default()),
        Ok(Some(bytes)) => State::from_json(&bytes).ok_or(StoreError::Damaged {
            device: String::new(),
            file: crate::home::STATE_FILE,
        }),
        Err(CapError::TooLong) => Err(StoreError::Damaged {
            device: String::new(),
            file: crate::home::STATE_FILE,
        }),
        Err(CapError::Io(cause)) => Err(StoreError::io("read", &path, cause)),
    }
}

/// Writes `state.json`, durably.
pub(crate) fn write_state(home: &Home, state: &State) -> Result<(), StoreError> {
    let bytes = serde_json::to_vec(&state.to_json()).map_err(|_| StoreError::Damaged {
        device: String::new(),
        file: crate::home::STATE_FILE,
    })?;
    Ok(write_durable(&home.state_file(), &bytes)?)
}

// ---------------------------------------------------------------------------------- wakes

/// The notices already handled, as `wakes.json` keeps them, the oldest first. A file that is
/// missing or damaged gives none: at worst the Hermes is woken once more for a notice.
pub(crate) fn read_handled(home: &Home) -> Vec<uuid::Uuid> {
    let Ok(Some(bytes)) = read_capped(&home.wakes_file(), STATE_FILE_MAX) else {
        return Vec::new();
    };
    let Ok(Value::Object(object)) = read_strict(&bytes) else {
        return Vec::new();
    };
    if object.get("v").and_then(Value::as_u64) != Some(FORMAT_VERSION) {
        return Vec::new();
    }
    let Some(ids) = object.get("handled").and_then(Value::as_array) else {
        return Vec::new();
    };
    ids.iter()
        .take(crate::wake::HANDLED_MAX)
        .filter_map(|id| id.as_str().and_then(|id| uuid::Uuid::try_parse(id).ok()))
        .collect()
}

/// Writes `wakes.json`, durably.
pub(crate) fn write_handled(home: &Home, handled: &[uuid::Uuid]) -> Result<(), StoreError> {
    let ids: Vec<Value> = handled
        .iter()
        .map(|id| Value::from(id.as_hyphenated().to_string()))
        .collect();
    let mut object = Map::new();
    object.insert("v".to_owned(), Value::from(FORMAT_VERSION));
    object.insert("handled".to_owned(), Value::Array(ids));
    let bytes = serde_json::to_vec(&Value::Object(object)).map_err(|_| StoreError::Damaged {
        device: String::new(),
        file: crate::home::WAKES_FILE,
    })?;
    Ok(write_durable(&home.wakes_file(), &bytes)?)
}

// ---------------------------------------------------------------------------------- lock

/// The lock of the device folders, held while they change.
#[derive(Debug)]
pub(crate) struct DevicesLock {
    _lock: FileLock,
}

/// Why the lock of the device folders was not taken.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DevicesLockError {
    /// Someone else held it for the whole patience.
    #[error("the device folders are busy")]
    Busy,
    /// The system refused.
    #[error(transparent)]
    Lock(#[from] LockError),
    /// The task of the lock did not return.
    #[error("the lock could not be tried")]
    Task,
}

/// Takes the lock of the device folders, trying again for up to `patience_ms`.
pub(crate) async fn lock_devices(
    home: &Home,
    patience_ms: i64,
) -> Result<DevicesLock, DevicesLockError> {
    let path = home.devices_lock_file();
    let until = tokio::time::Instant::now() + crate::settings::duration(patience_ms);
    loop {
        let tried = {
            let path = path.clone();
            tokio::task::spawn_blocking(move || FileLock::try_acquire(&path))
                .await
                .map_err(|_| DevicesLockError::Task)?
        };
        match tried {
            Ok(lock) => return Ok(DevicesLock { _lock: lock }),
            Err(LockError::AlreadyHeld { .. }) if tokio::time::Instant::now() < until => {
                tokio::time::sleep(LOCK_RETRY).await;
            }
            Err(LockError::AlreadyHeld { .. }) => return Err(DevicesLockError::Busy),
            Err(other) => return Err(other.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> (tempfile::TempDir, Home) {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(&dir.path().join("arauto")).unwrap();
        home.prepare().unwrap();
        (dir, home)
    }

    fn address() -> HubAddress {
        HubAddress::parse("ws://127.0.0.1:9/v1/channel").unwrap()
    }

    fn key() -> PublicKey {
        *StaticKeypair::generate().unwrap().public()
    }

    #[test]
    fn device_names_have_one_written_form() {
        let id = DeviceId::generate().unwrap();
        assert_eq!(DeviceId::parse(id.as_str()), Some(id.clone()));
        assert_ne!(DeviceId::generate().unwrap(), id);
        for text in [
            "",
            "..",
            "x",
            "3F2B7C1E-58A4-4D6B-9E0F-A1B2C3D4E5F6",
            "3f2b7c1e58a44d6b9e0fa1b2c3d4e5f6",
            "{3f2b7c1e-58a4-4d6b-9e0f-a1b2c3d4e5f6}",
            "3f2b7c1e-58a4-4d6b-9e0f-a1b2c3d4e5f6 ",
            ".discarded-3f2b7c1e-58a4-4d6b-9e0f-a1b2c3d4e5f6",
        ] {
            assert_eq!(DeviceId::parse(text), None, "{text}");
        }
    }

    #[test]
    fn a_request_holds_its_identity_and_its_hub() {
        let (_dir, home) = home();
        let key = key();
        let digest = InviteDigest::from_bytes([7u8; 32]);
        let dir = create_request(&home, &address(), &key, digest, 1_000).unwrap();
        let record = dir.record().unwrap();
        assert!(record.same_hub(&address(), &key));
        assert!(!record.same_hub(&address(), &self::key()));
        assert_eq!(record.invite, digest);
        assert_eq!(record.situation, Situation::Pending);
        assert!(!record.registered);
        assert!(dir.identity().is_ok());
        assert_eq!(dir.credential().unwrap(), None);
        // The identity is read back the same.
        assert_eq!(
            dir.identity().unwrap().public(),
            dir.identity().unwrap().public()
        );
        dir.set_situation(Situation::Ended(ErrorCode::Ar502))
            .unwrap();
        assert_eq!(
            dir.record().unwrap().situation,
            Situation::Ended(ErrorCode::Ar502)
        );
        let credential = SealedCredential::from_slice(&[9u8; 80]).unwrap();
        dir.write_credential(&credential).unwrap();
        assert_eq!(dir.credential().unwrap(), Some(credential));
    }

    /// A damaged file is an error that says so, and nothing is deleted.
    #[test]
    fn damaged_files_are_reported_not_deleted() {
        let (_dir, home) = home();
        let dir = create_request(
            &home,
            &address(),
            &key(),
            InviteDigest::from_bytes([1; 32]),
            1,
        )
        .unwrap();
        fs::write(dir.file(IDENTITY_FILE), [1u8; 31]).unwrap();
        assert!(dir.identity().unwrap_err().is_damaged());
        fs::write(dir.file(CREDENTIAL_FILE), []).unwrap();
        assert!(dir.credential().unwrap_err().is_damaged());
        fs::write(dir.file(CREDENTIAL_FILE), vec![1u8; CREDENTIAL_MAX_LEN + 1]).unwrap();
        assert!(dir.credential().unwrap_err().is_damaged());
        for garbage in [
            &b"{"[..],
            br#"{"v":2}"#,
            br#"{"v":1,"hub":"x"}"#,
            br#"{"v":1,"v":1}"#,
        ] {
            fs::write(dir.file(HUB_FILE), garbage).unwrap();
            assert!(dir.record().unwrap_err().is_damaged());
        }
        let layout = scan(&home).unwrap();
        assert_eq!(layout.folders.len(), 1);
        assert!(layout.folders[0].record.is_err());
        assert!(dir.exists());
        fs::write(home.current_file(), "not a device").unwrap();
        assert_eq!(scan(&home).unwrap().current, Current::Damaged);
        fs::write(home.state_file(), "[").unwrap();
        assert!(read_state(&home).unwrap_err().is_damaged());
    }

    #[test]
    fn the_request_followed_is_the_newest_pending_one_not_in_use() {
        let (_dir, home) = home();
        let make = |at| {
            create_request(
                &home,
                &address(),
                &key(),
                InviteDigest::from_bytes([1; 32]),
                at,
            )
            .unwrap()
        };
        let old = make(1);
        let device = make(2);
        let newest = make(3);
        write_current(&home, device.id()).unwrap();
        device.set_situation(Situation::Active).unwrap();
        let layout = scan(&home).unwrap();
        assert_eq!(layout.current().map(Folder::id), Some(device.id()));
        assert_eq!(layout.request().unwrap().id(), newest.id());
        newest
            .set_situation(Situation::Ended(ErrorCode::Ar301))
            .unwrap();
        assert_eq!(scan(&home).unwrap().request().unwrap().id(), old.id());
    }

    /// The only folder discarded because of the hub is a request without a credential.
    #[test]
    fn only_a_request_without_credential_is_discarded() {
        let (_dir, home) = home();
        let dir = create_request(
            &home,
            &address(),
            &key(),
            InviteDigest::from_bytes([1; 32]),
            1,
        )
        .unwrap();
        dir.write_credential(&SealedCredential::from_slice(&[1u8; 10]).unwrap())
            .unwrap();
        assert!(!discard_request(&home, dir.id()).unwrap());
        assert!(dir.exists());
        let other = create_request(
            &home,
            &address(),
            &key(),
            InviteDigest::from_bytes([2; 32]),
            2,
        )
        .unwrap();
        assert!(discard_request(&home, other.id()).unwrap());
        assert!(!other.exists());
        assert_eq!(scan(&home).unwrap().folders.len(), 1);
    }

    #[test]
    fn leftovers_of_interrupted_work_are_removed() {
        let (_dir, home) = home();
        let dir = create_request(
            &home,
            &address(),
            &key(),
            InviteDigest::from_bytes([1; 32]),
            1,
        )
        .unwrap();
        let away = home.devices().join(format!("{DISCARDED_PREFIX}x"));
        fs::create_dir(&away).unwrap();
        fs::write(away.join(IDENTITY_FILE), [1u8; 32]).unwrap();
        let temporary = dir.path().join(".credential.seal.0123456789abcdef.tmp");
        fs::write(&temporary, b"secret").unwrap();
        clean_leftovers(&home).unwrap();
        assert!(!away.exists());
        assert!(!temporary.exists());
        assert!(dir.identity().is_ok());
    }

    #[test]
    fn the_state_keeps_the_tools() {
        let (_dir, home) = home();
        assert_eq!(read_state(&home).unwrap(), State::default());
        let state = State {
            tools: vec![ToolName::parse("arauto_status").unwrap()],
        };
        write_state(&home, &state).unwrap();
        assert_eq!(read_state(&home).unwrap(), state);
    }

    #[test]
    fn everything_goes_but_the_locks() {
        let (_dir, home) = home();
        let dir = create_request(
            &home,
            &address(),
            &key(),
            InviteDigest::from_bytes([1; 32]),
            1,
        )
        .unwrap();
        write_current(&home, dir.id()).unwrap();
        write_state(&home, &State::default()).unwrap();
        let _lock = FileLock::try_acquire(&home.lock_file()).unwrap();
        remove_everything(&home).unwrap();
        let layout = scan(&home).unwrap();
        assert!(layout.folders.is_empty());
        assert_eq!(layout.current, Current::Absent);
        assert!(!home.state_file().exists());
        assert!(home.lock_file().exists());
    }
}
