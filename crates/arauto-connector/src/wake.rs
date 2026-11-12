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
//! | The ceiling of this connector: 30 a minute, 300 an hour, of any kind | `throttled` |
//! | The local API of Hermes accepted the run | `accepted`, and the identifier is kept |
//! | It refused, or did not answer in time | `failed` |
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
//! run. The exact shape of the body is still to be proven against a real Hermes
//! (`referencia/hermes-fatos.md`): it lives in [`request`], alone.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use arauto_protocol::notice::{AckOutcome, Notice, NoticeMsg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::home::Home;
use crate::store;

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

/// Whether the local API accepted the run: an answer with a status from 200 to 299.
async fn call(target: &WakeTarget, id: Uuid, phrase: &str) -> bool {
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
        Some((200..300).contains(&status))
    };
    matches!(
        tokio::time::timeout(API_PATIENCE, exchange).await,
        Ok(Some(true))
    )
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

    fn memory(&self) -> std::sync::MutexGuard<'_, Memory> {
        // Plain values: a panic elsewhere leaves them usable.
        self.memory.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Handles one notice of the hub, and says what goes back in `notice_ack`.
    pub(crate) async fn handle(&self, home: &Home, notice: &NoticeMsg) -> AckOutcome {
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
        let Some(target) = &self.target else {
            return AckOutcome::Failed;
        };
        if !self.memory().windows.take(Instant::now()) {
            return AckOutcome::Throttled;
        }
        if !call(target, id, &phrase(&checked)).await {
            return AckOutcome::Failed;
        }
        let kept: Vec<Uuid> = {
            let mut memory = self.memory();
            let handled = memory.handled.get_or_insert_with(VecDeque::new);
            handled.push_back(id);
            while handled.len() > HANDLED_MAX {
                handled.pop_front();
            }
            handled.iter().copied().collect()
        };
        // The Hermes was woken whatever becomes of this write: a failure only means that a
        // restart may wake once more for this notice.
        let home = home.clone();
        let _ = tokio::task::spawn_blocking(move || store::write_handled(&home, &kept)).await;
        AckOutcome::Accepted
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
}
