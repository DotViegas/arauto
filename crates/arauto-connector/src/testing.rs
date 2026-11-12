//! What the tests of this crate use to make the connector fail on command and to watch it.
//!
//! Only under the `testing` feature, which the dev-dependency of this crate on itself turns
//! on and which a shipped build never has. Nothing here is reachable from the command line or
//! from the environment: a test builds [`Settings`] and gives it to the library.
//!
//! | Piece | What it does |
//! |---|---|
//! | [`Settings::for_tests`] | Deadlines of milliseconds instead of seconds. |
//! | [`Faults`] | Says, at each [`Point`] of the swap, whether the connector fails there. |
//! | [`Event`] | What the connector did: attempts, waits, sessions, stops, the switch of device. |
//! | [`Settings::with_seed`] | The waits before a new attempt come from a seed, not from the system. |
//! | [`Settings::with_hermes`] | The command of Hermes is answered by the test instead of run. |
//! | [`attempt`] | One attempt of the channel of a device, with the code of the leader, and nothing around it. |

use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::limits::Deadlines;
use tokio::sync::mpsc::UnboundedSender;

use crate::home::Home;
use crate::settings::Settings;

/// A point of the channel where a test may make the connector fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Point {
    /// After `state`, before `present`: the connection is dropped.
    BeforePresent,
    /// After `present`: the `check` is never answered, and the connection is dropped when it
    /// comes.
    BeforeCheckOk,
    /// After `credential`, before it is written.
    BeforeSaving,
    /// The write of the credential fails, as on a full disk that takes its time to say so.
    /// Nothing is written.
    WriteCredential,
    /// After the credential is on disk, before `saved`.
    BeforeSaved,
    /// After `saved`, before `session`.
    BeforeSession,
    /// The 24-hour swap: after `credential`, before it is written.
    RotationBeforeSaving,
    /// The 24-hour swap: the write fails, as on a full disk that takes its time to say so.
    RotationWrite,
    /// The 24-hour swap: after the write, before `saved`.
    RotationBeforeSaved,
    /// The 24-hour swap: after `saved`, before `rotated`.
    RotationBeforeRotated,
}

impl Point {
    /// The points of the swap that opens a session.
    pub const SWAP: [Point; 6] = [
        Point::BeforePresent,
        Point::BeforeCheckOk,
        Point::BeforeSaving,
        Point::WriteCredential,
        Point::BeforeSaved,
        Point::BeforeSession,
    ];

    /// The points of the 24-hour swap.
    pub const ROTATION: [Point; 4] = [
        Point::RotationBeforeSaving,
        Point::RotationWrite,
        Point::RotationBeforeSaved,
        Point::RotationBeforeRotated,
    ];
}

/// Says where the connector fails.
pub trait Faults: Send + Sync {
    /// Whether an attempt of the device of folder `device` fails at `point`. Asked each time
    /// the point is reached.
    fn fails(&self, device: &str, point: Point) -> bool;
}

/// What the connector did, for a test to watch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// This copy became the leader of its folder, or a copy that forwards to it.
    Role {
        /// The leader.
        leader: bool,
    },
    /// An attempt of the channel of a device began.
    AttemptStarted {
        /// The folder of the device.
        device: String,
    },
    /// An attempt ended.
    AttemptEnded {
        /// The folder of the device.
        device: String,
        /// The code of the `closing` of the hub, when it sent one.
        closing: Option<ErrorCode>,
        /// Whether the connection was established: waiting or in session.
        established: bool,
    },
    /// The connector waits before the next attempt of a device.
    Waiting {
        /// The folder of the device.
        device: String,
        /// How long.
        ms: u64,
    },
    /// A session began.
    Session {
        /// The folder of the device.
        device: String,
    },
    /// The session of the device in use serves the calls of the leader.
    Serving {
        /// The folder of the device.
        device: String,
    },
    /// The connector stopped using the channel of a device.
    Stopped {
        /// The folder of the device.
        device: String,
        /// Why.
        code: ErrorCode,
    },
    /// A request became the device in use.
    Takeover {
        /// The folder of the device.
        device: String,
    },
    /// The folder of a device or request was deleted.
    Deleted {
        /// The folder.
        device: String,
    },
    /// A fault of the test happened.
    Faulted {
        /// The folder of the device.
        device: String,
        /// Where.
        point: Point,
    },
    /// A 24-hour swap was confirmed.
    Rotated {
        /// The folder of the device.
        device: String,
    },
}

type HermesAnswer = Arc<dyn Fn(&[OsString]) -> bool + Send + Sync>;

/// The test parts of the settings.
#[derive(Clone, Default)]
pub(crate) struct Testing {
    pub(crate) faults: Option<Arc<dyn Faults>>,
    pub(crate) events: Option<UnboundedSender<Event>>,
    pub(crate) seed: Option<u64>,
    pub(crate) hermes: Option<HermesAnswer>,
}

impl Settings {
    /// Short deadlines, in milliseconds, for tests that run the whole connector against a
    /// hub in the same process.
    #[must_use]
    pub fn for_tests() -> Self {
        let mut settings = Settings::default();
        let deadlines = &mut settings.deadlines;
        deadlines.reconnect_backoff_min_ms = 50;
        deadlines.reconnect_backoff_max_ms = 400;
        deadlines.hub_stopping_retry_min_ms = 50;
        deadlines.hub_stopping_retry_max_ms = 200;
        deadlines.replaced_retry_min_ms = 50;
        deadlines.replaced_retry_max_ms = 200;
        deadlines.same_identity_retry_gap_ms = 50;
        deadlines.outdated_presentation_interval_ms = 2_000;
        settings.relay_retry_ms = 50;
        settings.local_leave_ms = 60_000;
        settings
    }

    /// The deadlines of the protocol, to change.
    pub fn deadlines_mut(&mut self) -> &mut Deadlines {
        &mut self.deadlines
    }

    /// How long the leader waits for the lock of the device folders before a write.
    #[must_use]
    pub fn with_devices_lock_ms(mut self, ms: i64) -> Self {
        self.devices_lock_ms = ms;
        self
    }

    /// Fails where `faults` says.
    #[must_use]
    pub fn with_faults(mut self, faults: Arc<dyn Faults>) -> Self {
        self.testing.faults = Some(faults);
        self
    }

    /// Sends every [`Event`] to `events`.
    #[must_use]
    pub fn with_events(mut self, events: UnboundedSender<Event>) -> Self {
        self.testing.events = Some(events);
        self
    }

    /// Draws the waits before a new attempt from `seed`.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.testing.seed = Some(seed);
        self
    }

    /// Answers the command of Hermes with `answer`, which receives the arguments and says
    /// whether the command succeeded. The program is not run.
    #[must_use]
    pub fn with_hermes(
        mut self,
        answer: impl Fn(&[OsString]) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.testing.hermes = Some(Arc::new(answer));
        self
    }

    pub(crate) fn emit(&self, event: impl FnOnce() -> Event) {
        if let Some(events) = &self.testing.events {
            let _ = events.send(event());
        }
    }

    pub(crate) fn fails(&self, device: &str, point: Point) -> bool {
        let fails = self
            .testing
            .faults
            .as_ref()
            .is_some_and(|faults| faults.fails(device, point));
        if fails {
            self.emit(|| Event::Faulted {
                device: device.to_owned(),
                point,
            });
        }
        fails
    }
}

/// How long a disk that fails takes to say so, at [`Point::WriteCredential`] and
/// [`Point::RotationWrite`]: long enough for the hub to act on a "saved" sent too early.
const SLOW_DISK: Duration = Duration::from_millis(400);

/// The wait of a disk that fails.
pub(crate) async fn slow_disk() {
    tokio::time::sleep(SLOW_DISK).await;
}

/// How an attempt of [`attempt`] ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempted {
    /// The session began; it was kept for the time asked and then closed by the connector,
    /// unless the hub closed it first, with this code.
    Session(Option<ErrorCode>),
    /// The hub ended the attempt before the session, with this code when it sent one.
    Closed(Option<ErrorCode>),
    /// The attempt failed at a point of the test.
    Faulted(Point),
    /// The attempt did not reach the hub, or broke on the way.
    Failed(String),
}

/// One attempt of the channel of the device of folder `device`, with the code of the leader:
/// the files on disk, the swap, the durable write of the credential and the session, which is
/// kept for `linger`, answering the hub, and then closed. No switch of device, no new attempt.
///
/// # Errors
/// The folder of the device cannot be read.
pub async fn attempt(
    home: &Home,
    device: &str,
    settings: &Settings,
    linger: Duration,
) -> Result<Attempted, crate::store::StoreError> {
    crate::attempt::one_for_tests(home, device, settings, linger).await
}
