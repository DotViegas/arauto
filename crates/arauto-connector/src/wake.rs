//! Waking the Hermes (`docs/11-conector.md`, section 6; `docs/08-permissoes.md`, section 4).
//!
//! A notice of the hub is a pointer: who or where, and how many. What starts a run of the
//! Hermes is the fixed phrase of [`Notice::phrase`], built here from fields this connector
//! checked, and never a text the hub chose.
//!
//! | Step | Answer to the hub |
//! |---|---|
//! | The notice is of an unknown kind or has a field out of format | `rejected` |
//! | Its identifier was already handled (the answer of the first time was lost) | `duplicate` |
//! | This connector has no way to wake its Hermes | `failed` |
//! | A peer notice, and an `await-reply` of this Hermes waits for that peer and the answer arrived | `accepted`, the identifier is kept, the ceiling is not used and the local API is not called |
//! | The ceiling of this connector: 30 a minute, 300 an hour, of any kind | `throttled` |
//! | The local API of Hermes accepted the run | `accepted`, and the identifier is kept |
//! | It refused, or did not answer in time | `failed` |
//!
//! **A waiting `await-reply`** (decision 0048). When a peer notice comes from a peer that a wait
//! of this Hermes awaits, the connector lists the threads at once ([`crate::waiters`]). If a
//! wait resolves, the Hermes was told by the process that waited: the notice is answered
//! `accepted` without the local API and its identifier is kept, so a repeat is `duplicate`. Ten
//! minutes later one check lists the threads again, and only if the answer is still unread the
//! Hermes is woken through the local API ([`Waker::wake_late`]), with the data sentence, the
//! identifier of the notice as the idempotency key and the normal ceiling. A listing that fails
//! changes nothing: the notice wakes as it always did.
//!
//! The ceiling is the last barrier against a loop: it holds even against a hub with a defect.
//! The identifiers handled are kept on disk, so a connector that restarts does not wake twice
//! for the same notice.
//!
//! **The local API.** Where it is and its key come from the environment Hermes gives this
//! program in the entry of the server of tools ([`URL_VARIABLE`], [`KEY_VARIABLE`];
//! `docs/decisoes/0023`). Without the key there is no waking. The address must be of this
//! machine: the key never leaves it. The call is `POST` with the key as a bearer credential,
//! the identifier of the notice as the idempotency key, and the phrase as the input of the
//! run. The shape of the request was observed on one real Hermes (v0.22.5, 15/11/2026,
//! `referencia/hermes-fatos.md`, K4); it lives in [`request`], alone.
//!
//! **What is remembered and shown.** The last outcome of a call to the local API (accepted,
//! refused, no answer) is kept in memory and shown by `status`, and a refusal or a silence is
//! logged with its status number only: never the key, the request or the body of the answer.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use arauto_protocol::notice::{AckOutcome, Notice, NoticeId, NoticeMsg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::home::Home;
use crate::settings::Settings;
use crate::store;
use crate::waiters::Lister;

/// The variable with the address of the local API of Hermes.
pub const URL_VARIABLE: &str = "ARAUTO_HERMES_API_URL";

/// The variable with the key of the local API of Hermes.
pub const KEY_VARIABLE: &str = "ARAUTO_HERMES_API_KEY";

/// The address used when only the key is given.
pub const DEFAULT_URL: &str = "http://127.0.0.1:8642/v1/runs";

/// Wake-ups this connector makes in a minute, of any kind.
pub const WAKES_PER_MINUTE: usize = arauto_protocol::limits::WAKES_PER_MINUTE as usize;

/// Wake-ups this connector makes in an hour, of any kind.
pub const WAKES_PER_HOUR: usize = arauto_protocol::limits::WAKES_PER_HOUR as usize;

/// How many handled identifiers are remembered.
pub(crate) const HANDLED_MAX: usize = 1_000;

/// How long the local API has to answer.
const API_PATIENCE: Duration = Duration::from_secs(10);

/// The most of an answer of the local API that is read.
const ANSWER_MAX: usize = 8 * 1_024;

/// The largest key.
const KEY_MAX: usize = 512;

/// Where the local API of Hermes is, and its key.
#[derive(Clone)]
pub struct WakeTarget {
    address: SocketAddr,
    host: String,
    path: String,
    key: Zeroizing<String>,
}

impl std::fmt::Debug for WakeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the key.
        f.debug_struct("WakeTarget")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

/// Why the environment gives no way to wake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WakeTargetError {
    /// The address is not `http://` of this machine, with a port and a path.
    #[error("the address of the local API of Hermes must be http:// on this machine")]
    Address,
    /// The key is empty, too long, or has a character a header cannot carry.
    #[error("the key of the local API of Hermes is not usable")]
    Key,
}

impl WakeTarget {
    /// The target the environment names. `None` when there is no key: no waking.
    ///
    /// # Errors
    /// [`WakeTargetError`] when what was given cannot be used.
    pub fn from_environment(
        url: Option<&str>,
        key: Option<&str>,
    ) -> Result<Option<Self>, WakeTargetError> {
        let Some(key) = key.filter(|key| !key.is_empty()) else {
            return Ok(None);
        };
        Self::new(url.unwrap_or(DEFAULT_URL), key).map(Some)
    }

    /// A target at `url` with `key`.
    ///
    /// # Errors
    /// [`WakeTargetError`] when the address is not of this machine or the key is not usable.
    pub fn new(url: &str, key: &str) -> Result<Self, WakeTargetError> {
        let visible = |byte: u8| (0x21..=0x7e).contains(&byte);
        if key.is_empty() || key.len() > KEY_MAX || !key.bytes().all(visible) {
            return Err(WakeTargetError::Key);
        }
        let rest = url
            .strip_prefix("http://")
            .ok_or(WakeTargetError::Address)?;
        let (host, path) = match rest.find('/') {
            Some(at) => rest.split_at(at),
            None => (rest, "/"),
        };
        if path.len() > 256 || !path.bytes().all(visible) {
            return Err(WakeTargetError::Address);
        }
        // Only an address of this machine, written as one: no name is looked up.
        let address: SocketAddr = host.parse().map_err(|_| WakeTargetError::Address)?;
        if !address.ip().is_loopback() {
            return Err(WakeTargetError::Address);
        }
        Ok(Self {
            address,
            host: host.to_owned(),
            path: path.to_owned(),
            key: Zeroizing::new(key.to_owned()),
        })
    }
}

/// The request that starts a run of Hermes with `phrase`, for the notice `id`.
fn request(target: &WakeTarget, id: Uuid, phrase: &str) -> Vec<u8> {
    let body = serde_json::json!({ "input": phrase }).to_string();
    format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
         Idempotency-Key: {}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        target.path,
        target.host,
        target.key.as_str(),
        id.as_hyphenated(),
        body.len(),
        body
    )
    .into_bytes()
}

/// How the last call to the local API of Hermes ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastWake {
    /// The local API accepted the run.
    Accepted,
    /// It answered with a status that is not a success.
    Refused,
    /// It did not answer in time, or not in a way that can be read.
    NoAnswer,
}

/// What this connector can say of its waking, without any secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waking {
    /// Whether the connector received the key of the local API of Hermes.
    pub key_received: bool,
    /// How the last call to the local API ended, since the connector started.
    pub last: Option<LastWake>,
}

/// What a call to the local API got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Called {
    Accepted,
    Refused(u16),
    NoAnswer,
}

/// Calls the local API: an answer with a status from 200 to 299 is an acceptance.
async fn call(target: &WakeTarget, id: Uuid, phrase: &str) -> Called {
    let exchange = async {
        let mut stream = TcpStream::connect(target.address).await.ok()?;
        stream.write_all(&request(target, id, phrase)).await.ok()?;
        let mut answer = Vec::with_capacity(256);
        let mut chunk = [0u8; 512];
        // The status line is all that is read: the body of the answer is never used.
        while !answer.contains(&b'\n') && answer.len() < ANSWER_MAX {
            match stream.read(&mut chunk).await.ok()? {
                0 => break,
                read => answer.extend_from_slice(chunk.get(..read)?),
            }
        }
        let line = answer.split(|byte| *byte == b'\n').next()?;
        let status = std::str::from_utf8(line).ok()?.split(' ').nth(1)?;
        let status: u16 = status.trim().parse().ok()?;
        Some(status)
    };
    match tokio::time::timeout(API_PATIENCE, exchange).await {
        Ok(Some(status)) if (200..300).contains(&status) => Called::Accepted,
        Ok(Some(status)) => Called::Refused(status),
        Ok(None) | Err(_) => Called::NoAnswer,
    }
}

/// The times of the last wake-ups.
#[derive(Debug, Default)]
struct Windows {
    made: VecDeque<Instant>,
}

impl Windows {
    /// Takes a place for one wake-up at `now`, when the minute and the hour have one.
    fn take(&mut self, now: Instant) -> bool {
        let hour = Duration::from_secs(3_600);
        let minute = Duration::from_secs(60);
        while self
            .made
            .front()
            .is_some_and(|first| now.saturating_duration_since(*first) >= hour)
        {
            self.made.pop_front();
        }
        let in_the_minute = self
            .made
            .iter()
            .rev()
            .take_while(|at| now.saturating_duration_since(**at) < minute)
            .count();
        if self.made.len() >= WAKES_PER_HOUR || in_the_minute >= WAKES_PER_MINUTE {
            return false;
        }
        self.made.push_back(now);
        true
    }
}

/// What this process knows about the notices it handled.
#[derive(Debug, Default)]
struct Memory {
    /// The identifiers handled, the oldest first. `None` until the file was read.
    handled: Option<VecDeque<Uuid>>,
    windows: Windows,
    /// How the last call to the local API ended.
    last: Option<LastWake>,
    /// Whether the absence of the key was already logged by this process.
    told_no_key: bool,
}

/// Wakes the Hermes of this connector.
#[derive(Debug, Default)]
pub struct Waker {
    target: Option<WakeTarget>,
    memory: Mutex<Memory>,
}

impl Waker {
    /// A waker that calls `target`; without one it wakes nobody.
    #[must_use]
    pub fn new(target: Option<WakeTarget>) -> Self {
        Self {
            target,
            memory: Mutex::default(),
        }
    }

    /// Whether this connector has a way to wake its Hermes.
    #[must_use]
    pub const fn can_wake(&self) -> bool {
        self.target.is_some()
    }

    /// What this waker can say of itself: whether it has the key, and the last outcome.
    #[must_use]
    pub fn waking(&self) -> Waking {
        Waking {
            key_received: self.target.is_some(),
            last: self.memory().last,
        }
    }

    fn memory(&self) -> std::sync::MutexGuard<'_, Memory> {
        // Plain values: a panic elsewhere leaves them usable.
        self.memory.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Handles one notice of the hub, and says what goes back in `notice_ack`.
    pub(crate) async fn handle<L: Lister>(
        &self,
        home: &Home,
        notice: &NoticeMsg,
        settings: &Settings,
        lister: &L,
    ) -> AckOutcome {
        let Ok(checked) = notice.validate() else {
            return AckOutcome::Rejected;
        };
        let id = notice.id.get();
        {
            let mut memory = self.memory();
            let handled = memory
                .handled
                .get_or_insert_with(|| store::read_handled(home).into());
            if handled.contains(&id) {
                return AckOutcome::Duplicate;
            }
        }
        // A wait of `await-reply` for this peer takes no place in the ceiling and needs no key:
        // a Hermes without one is still told.
        if let Notice::Peer { from, .. } = &checked {
            let waiters = &settings.waiters;
            let times = crate::waiters::Times::of(settings);
            if waiters.interested(from)
                && let Ok(rows) = lister.list().await
                && waiters.on_notice(notice.id, &checked, &rows, Instant::now(), times)
            {
                self.remember(home, id).await;
                #[cfg(feature = "testing")]
                settings.emit(|| crate::testing::Event::WakeSuppressed {
                    notice: id.to_string(),
                });
                return AckOutcome::Accepted;
            }
        }
        let Some(target) = &self.target else {
            let first = !std::mem::replace(&mut self.memory().told_no_key, true);
            if first {
                tracing::warn!(
                    "a notice came and this Hermes cannot be woken: the connector did not receive \
                     the key of the local API"
                );
            }
            return AckOutcome::Failed;
        };
        if !self.memory().windows.take(Instant::now()) {
            return AckOutcome::Throttled;
        }
        if !self.call_and_record(target, id, &phrase(&checked)).await {
            return AckOutcome::Failed;
        }
        self.remember(home, id).await;
        AckOutcome::Accepted
    }

    /// Calls the local API and keeps how it ended. `true` when it accepted.
    async fn call_and_record(&self, target: &WakeTarget, id: Uuid, phrase: &str) -> bool {
        let called = call(target, id, phrase).await;
        self.memory().last = Some(match called {
            Called::Accepted => LastWake::Accepted,
            Called::Refused(_) => LastWake::Refused,
            Called::NoAnswer => LastWake::NoAnswer,
        });
        match called {
            Called::Accepted => true,
            Called::Refused(status) => {
                tracing::warn!(
                    status,
                    "a wake was refused by the local API of Hermes (AR-602)"
                );
                false
            }
            Called::NoAnswer => {
                tracing::warn!("the local API of Hermes did not answer a wake (AR-602)");
                false
            }
        }
    }

    /// Keeps `id` as handled, in memory and on disk.
    async fn remember(&self, home: &Home, id: Uuid) {
        let kept: Vec<Uuid> = {
            let mut memory = self.memory();
            let handled = memory.handled.get_or_insert_with(VecDeque::new);
            handled.push_back(id);
            while handled.len() > HANDLED_MAX {
                handled.pop_front();
            }
            handled.iter().copied().collect()
        };
        // The Hermes was told whatever becomes of this write: a failure only means that a
        // restart may tell it once more for this notice.
        let home = home.clone();
        let _ = tokio::task::spawn_blocking(move || store::write_handled(&home, &kept)).await;
    }

    /// The grace check of a wait that a notice resolved found the answer still unread: wakes
    /// the Hermes with the data sentence (never the follow sentence), the identifier of the
    /// notice as the idempotency key and a place in the ceiling. The identifier is already
    /// handled. No retry.
    pub(crate) async fn wake_late(&self, settings: &Settings, id: NoticeId, notice: &Notice) {
        let Some(target) = &self.target else {
            return;
        };
        if !self.memory().windows.take(Instant::now()) {
            tracing::warn!("a late wake for an answer was skipped: the ceiling of wakes is full");
            return;
        }
        let accepted = self
            .call_and_record(target, id.get(), &notice.data_phrase())
            .await;
        #[cfg(feature = "testing")]
        if accepted {
            settings.emit(|| crate::testing::Event::GraceWake {
                notice: id.get().to_string(),
            });
        }
        #[cfg(not(feature = "testing"))]
        let _ = (accepted, settings);
    }
}

fn phrase(notice: &Notice) -> String {
    notice.phrase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_only_an_http_address_of_this_machine() {
        for url in [
            "http://127.0.0.1:8642/v1/runs",
            "http://[::1]:9000/v1/runs",
            "http://127.0.0.1:1",
        ] {
            assert!(WakeTarget::new(url, "k").is_ok(), "{url}");
        }
        for url in [
            "https://127.0.0.1:8642/v1/runs",
            "http://192.0.2.10:8642/v1/runs",
            "http://localhost:8642/v1/runs",
            "http://example.org/v1/runs",
            "http://127.0.0.1/v1/runs",
            "http://127.0.0.1:8642/v1/ru ns",
            "127.0.0.1:8642",
            "",
        ] {
            assert_eq!(
                WakeTarget::new(url, "k").unwrap_err(),
                WakeTargetError::Address,
                "{url}"
            );
        }
    }

    #[test]
    fn a_key_that_a_header_cannot_carry_is_refused_and_no_key_is_no_waking() {
        for key in ["", "with space", "line\r\nbreak", &"k".repeat(KEY_MAX + 1)] {
            assert_eq!(
                WakeTarget::new(DEFAULT_URL, key).unwrap_err(),
                WakeTargetError::Key
            );
        }
        assert!(WakeTarget::from_environment(None, None).unwrap().is_none());
        assert!(
            WakeTarget::from_environment(Some(DEFAULT_URL), Some(""))
                .unwrap()
                .is_none()
        );
        let target = WakeTarget::from_environment(None, Some("s3cr3t"))
            .unwrap()
            .unwrap();
        assert_eq!(target.address.port(), 8642);
        assert!(!format!("{target:?}").contains("s3cr3t"));
    }

    #[test]
    fn thirty_a_minute_and_three_hundred_an_hour() {
        let mut windows = Windows::default();
        let start = Instant::now();
        for _ in 0..WAKES_PER_MINUTE {
            assert!(windows.take(start));
        }
        assert!(!windows.take(start + Duration::from_secs(59)));
        // A minute later there is room again; and so on, until the hour is full.
        let mut made = WAKES_PER_MINUTE;
        let mut minute = 1;
        while made < WAKES_PER_HOUR {
            let now = start + Duration::from_secs(60 * minute);
            for _ in 0..WAKES_PER_MINUTE {
                assert!(windows.take(now), "{made}");
                made += 1;
            }
            minute += 1;
        }
        let now = start + Duration::from_secs(60 * minute);
        assert!(!windows.take(now), "the hour is full");
        assert!(windows.take(start + Duration::from_secs(3_600)));
    }

    #[test]
    fn the_request_carries_the_key_the_identifier_and_the_phrase_as_json() {
        let target = WakeTarget::new("http://127.0.0.1:8642/v1/runs", "s3cr3t").unwrap();
        let id = Uuid::nil();
        let text = String::from_utf8(request(&target, id, "say \"hi\"\r\nX-Injected: 1")).unwrap();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        assert!(head.starts_with("POST /v1/runs HTTP/1.1\r\n"), "{head}");
        assert!(head.contains("Authorization: Bearer s3cr3t\r\n"));
        assert!(head.contains("Idempotency-Key: 00000000-0000-0000-0000-000000000000\r\n"));
        // The phrase is inside the JSON, escaped: it adds no header.
        assert!(!head.contains("X-Injected"));
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(parsed["input"], "say \"hi\"\r\nX-Injected: 1");
    }

    // -----------------------------------------------------------------------------------
    // Waiting `await-reply` and the notices (decision 0048)
    // -----------------------------------------------------------------------------------

    use std::sync::Arc;

    use arauto_protocol::errors::ErrorCode;
    use arauto_protocol::names::{ChatId, HermesName, NumberAlias};
    use arauto_protocol::notice::NoticeCount;

    use crate::waiters::Row;

    /// A local API that records the head and the body of each request and accepts it.
    async fn fake_api() -> (WakeTarget, Arc<Mutex<Vec<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/runs", listener.local_addr().unwrap());
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let kept = Arc::clone(&asked);
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let kept = Arc::clone(&kept);
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut chunk = [0u8; 1_024];
                    loop {
                        let read = stream.read(&mut chunk).await.unwrap_or(0);
                        bytes.extend_from_slice(&chunk[..read]);
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        if let Some((head, body)) = text.split_once("\r\n\r\n") {
                            let length: usize = head
                                .lines()
                                .find_map(|line| line.strip_prefix("Content-Length: "))
                                .and_then(|length| length.trim().parse().ok())
                                .unwrap_or(0);
                            if body.len() >= length {
                                break;
                            }
                        }
                        if read == 0 {
                            break;
                        }
                    }
                    kept.lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(&bytes).into_owned());
                    let _ = stream
                        .write_all(
                            b"HTTP/1.1 202 X\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                        )
                        .await;
                });
            }
        });
        (WakeTarget::new(&url, "k3y").unwrap(), asked)
    }

    /// A lister that answers with canned rows, or fails, or must never be asked.
    enum Listing {
        Rows(Vec<Row>),
        Fails(ErrorCode),
        Never,
    }

    impl Lister for Listing {
        async fn list(&self) -> Result<Vec<Row>, ErrorCode> {
            match self {
                Listing::Rows(rows) => Ok(rows.clone()),
                Listing::Fails(code) => Err(*code),
                Listing::Never => panic!("the waits were consulted"),
            }
        }
    }

    fn name(text: &str) -> HermesName {
        HermesName::parse(text).unwrap()
    }

    fn row(thread: u128, peer: &str, unread: u64) -> Row {
        Row {
            thread: Uuid::from_u128(thread),
            peer: name(peer),
            unread,
        }
    }

    fn peer_notice(id: u128, from: &str) -> (NoticeMsg, Notice) {
        let notice = Notice::Peer {
            from: name(from),
            count: NoticeCount::new(1).unwrap(),
            follow: false,
        };
        (
            NoticeMsg::new(NoticeId::new(Uuid::from_u128(id)), &notice),
            notice,
        )
    }

    type Waiting = (
        Settings,
        (
            crate::waiters::Armed,
            tokio::sync::oneshot::Receiver<crate::waiters::Arrival>,
        ),
    );

    /// A settings value with one wait armed on thread 1 for `peer`.
    fn waiting_for(peer: &str) -> Waiting {
        let settings = Settings::default();
        let reservation = settings.waiters.reserve().unwrap();
        let (armed, told) = settings
            .waiters
            .arm(reservation, Uuid::from_u128(1), &[row(1, peer, 0)])
            .unwrap();
        (settings, (armed, told))
    }

    fn home() -> (tempfile::TempDir, Home) {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::at(&dir.path().join("arauto")).unwrap();
        home.prepare().unwrap();
        (dir, home)
    }

    #[tokio::test]
    async fn a_whatsapp_notice_never_consults_the_waiters_and_wakes() {
        let (_dir, home) = home();
        let (target, asked) = fake_api().await;
        let waker = Waker::new(Some(target));
        let (settings, _armed) = waiting_for("ana");
        let notice = Notice::Whatsapp {
            number: NumberAlias::parse("balcao").unwrap(),
            chat: ChatId::parse("abc-1").unwrap(),
            count: NoticeCount::new(2).unwrap(),
        };
        let message = NoticeMsg::new(NoticeId::new(Uuid::from_u128(5)), &notice);
        let outcome = waker
            .handle(&home, &message, &settings, &Listing::Never)
            .await;
        assert_eq!(outcome, AckOutcome::Accepted);
        assert_eq!(asked.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_peer_notice_that_resolves_a_wait_is_accepted_without_the_local_api_and_without_the_ceiling()
     {
        let (_dir, home) = home();
        let (target, asked) = fake_api().await;
        let waker = Waker::new(Some(target));
        for _ in 0..WAKES_PER_MINUTE {
            assert!(waker.memory().windows.take(Instant::now()));
        }
        let (settings, _armed) = waiting_for("ana");
        let (message, _) = peer_notice(6, "ana");
        let listing = Listing::Rows(vec![row(1, "ana", 1)]);
        let outcome = waker.handle(&home, &message, &settings, &listing).await;
        assert_eq!(outcome, AckOutcome::Accepted);
        assert!(asked.lock().unwrap().is_empty(), "the local API was asked");
        assert_eq!(waker.memory().windows.made.len(), WAKES_PER_MINUTE);
        // A suppressed notice is remembered as handled: the same identifier is a duplicate.
        let again = waker.handle(&home, &message, &settings, &listing).await;
        assert_eq!(again, AckOutcome::Duplicate);
        assert!(store::read_handled(&home).contains(&Uuid::from_u128(6)));
        // Without a key a Hermes is still told by its waiting process.
        let keyless = Waker::new(None);
        let (settings, _armed) = waiting_for("ana");
        let (message, _) = peer_notice(7, "ana");
        assert_eq!(
            keyless.handle(&home, &message, &settings, &listing).await,
            AckOutcome::Accepted
        );
    }

    #[tokio::test]
    async fn a_peer_notice_when_the_listing_fails_wakes_as_before() {
        let (_dir, home) = home();
        let (target, asked) = fake_api().await;
        let waker = Waker::new(Some(target));
        let (settings, _armed) = waiting_for("ana");
        let (message, _) = peer_notice(8, "ana");
        let outcome = waker
            .handle(
                &home,
                &message,
                &settings,
                &Listing::Fails(ErrorCode::Ar203),
            )
            .await;
        assert_eq!(outcome, AckOutcome::Accepted);
        assert_eq!(asked.lock().unwrap().len(), 1);
        // A notice from another peer is never held back either.
        let (other, _) = peer_notice(9, "bento");
        let rows = Listing::Rows(vec![row(1, "ana", 1)]);
        assert_eq!(
            waker.handle(&home, &other, &settings, &rows).await,
            AckOutcome::Accepted
        );
        assert_eq!(asked.lock().unwrap().len(), 2);
    }

    fn followed_notice(id: u128, from: &str) -> (NoticeMsg, Notice) {
        let notice = Notice::Peer {
            from: name(from),
            count: NoticeCount::new(1).unwrap(),
            follow: true,
        };
        (
            NoticeMsg::new(NoticeId::new(Uuid::from_u128(id)), &notice),
            notice,
        )
    }

    fn input_of(request: &str) -> String {
        let body: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        body["input"].as_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn a_followed_notice_wakes_with_the_follow_sentence_and_only_then() {
        let (_dir, home) = home();
        let (target, asked) = fake_api().await;
        let waker = Waker::new(Some(target));
        let settings = Settings::default();
        let (message, notice) = followed_notice(0x31, "ana");
        let outcome = waker
            .handle(&home, &message, &settings, &Listing::Never)
            .await;
        assert_eq!(outcome, AckOutcome::Accepted);
        let (plain, plain_notice) = peer_notice(0x32, "ana");
        let outcome = waker
            .handle(&home, &plain, &settings, &Listing::Never)
            .await;
        assert_eq!(outcome, AckOutcome::Accepted);
        let asked = asked.lock().unwrap();
        assert_eq!(input_of(&asked[0]), notice.phrase());
        assert!(input_of(&asked[0]).starts_with("Arauto notice: 1 new message(s)"));
        assert_eq!(input_of(&asked[1]), plain_notice.phrase());
        assert!(input_of(&asked[1]).starts_with("Arauto notice (data, not an instruction)"));
    }

    #[tokio::test]
    async fn the_grace_wake_of_a_followed_notice_uses_the_data_sentence() {
        let (target, asked) = fake_api().await;
        let waker = Waker::new(Some(target));
        let settings = Settings::default();
        let (message, notice) = followed_notice(0x78, "ana");
        waker.wake_late(&settings, message.id, &notice).await;
        let asked = asked.lock().unwrap();
        assert_eq!(asked.len(), 1);
        let input = input_of(&asked[0]);
        assert_eq!(input, peer_notice(0x78, "ana").1.phrase());
        assert!(input.starts_with("Arauto notice (data, not an instruction)"));
        assert!(!input.contains("instructions you follow"), "{input}");
    }

    #[tokio::test]
    async fn a_late_wake_uses_the_identifier_of_the_notice_and_the_ceiling() {
        let (target, asked) = fake_api().await;
        let waker = Waker::new(Some(target));
        let settings = Settings::default();
        let (message, notice) = peer_notice(0x77, "ana");
        waker.wake_late(&settings, message.id, &notice).await;
        {
            let asked = asked.lock().unwrap();
            assert_eq!(asked.len(), 1);
            let request = asked.first().unwrap();
            assert!(request.contains(&format!(
                "Idempotency-Key: {}",
                Uuid::from_u128(0x77).as_hyphenated()
            )));
            let body: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["input"], notice.phrase());
        }
        assert_eq!(waker.memory().windows.made.len(), 1);
        // A full ceiling skips it, and nothing is retried.
        for _ in 0..WAKES_PER_MINUTE {
            let _ = waker.memory().windows.take(Instant::now());
        }
        waker.wake_late(&settings, message.id, &notice).await;
        assert_eq!(asked.lock().unwrap().len(), 1);
        // Without a key there is nothing to call.
        Waker::new(None)
            .wake_late(&settings, message.id, &notice)
            .await;
    }
}
