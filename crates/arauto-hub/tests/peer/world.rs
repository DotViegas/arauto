//! A hub on a clock the test moves, and Hermes in session with it.

use std::sync::Arc;
use std::time::Duration;

use arauto_protocol::clock::ManualClock;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{CallOutcome, Message, NoticeAck, ToolList};
use arauto_protocol::notice::{AckOutcome, Notice, NoticeMsg};
use arauto_testkit::KitError;
use arauto_testkit::device::SessionClient;
use arauto_testkit::hub::fixture::{self, Fixture};
use serde_json::{Map, Value, json};

/// Patience for an answer of the hub.
pub const PATIENCE: Duration = Duration::from_secs(20);

/// How long a test waits to say that nothing came. The hub looks at its queues every 200 ms.
pub const QUIET: Duration = Duration::from_millis(900);

pub const SECOND: i64 = 1_000;
pub const MINUTE: i64 = 60 * SECOND;
pub const HOUR: i64 = 60 * MINUTE;

/// The time the clock of the hub starts at.
const START_MS: i64 = 1_800_000_000_000;

pub struct World {
    pub fixture: Fixture,
    pub clock: Arc<ManualClock>,
}

impl World {
    pub async fn new() -> Self {
        let clock = Arc::new(ManualClock::new(START_MS));
        let mut settings = fixture::settings();
        // The test tells the sessions by the sweep, every 200 ms.
        settings.listen_admin = false;
        // Many of these hubs run at the same time, on one test database server.
        settings.pool.size = 4;
        let fixture = Fixture::with_clock(settings, clock.clone()).await;
        Self { fixture, clock }
    }

    pub fn now(&self) -> i64 {
        self.fixture.now()
    }

    pub fn advance(&self, ms: i64) {
        self.clock.advance(ms);
    }

    /// A new Hermes, connected and in session.
    pub async fn agent(&self) -> Agent {
        let (name, session) = self.fixture.connected().await;
        Agent {
            name,
            session,
            tools: Vec::new(),
            notices: Vec::new(),
            next: 0,
        }
    }

    /// `admin link add FROM TO [--wake]`.
    pub async fn link(&self, from: &Agent, to: &Agent, wake: bool) {
        self.fixture
            .admin
            .link_add(&from.name, &to.name, wake, false, self.now())
            .await
            .unwrap();
    }

    /// `admin link remove FROM TO`.
    pub async fn unlink(&self, from: &Agent, to: &Agent) {
        self.fixture
            .admin
            .link_remove(&from.name, &to.name, false, self.now())
            .await
            .unwrap();
    }

    /// `admin wake NAME on|off`.
    pub async fn may_be_woken(&self, agent: &Agent, on: bool) {
        self.fixture
            .admin
            .wake(&agent.name, on, self.now())
            .await
            .unwrap();
    }

    /// The states of the notices of the messages of `from` to `to`, the oldest first.
    pub async fn notice_states(&self, from: &Agent, to: &Agent) -> Vec<(String, i32)> {
        sqlx::query_as(
            "SELECT n.state, n.count FROM notice n \
             JOIN hermes f ON f.id = n.from_hermes_id \
             JOIN hermes t ON t.id = n.hermes_id \
             WHERE f.name = $1 AND t.name = $2 ORDER BY n.created_at, n.id",
        )
        .bind(&from.name)
        .bind(&to.name)
        .fetch_all(&self.fixture.pool)
        .await
        .unwrap()
    }
}

/// A Hermes connected to the hub, in session.
pub struct Agent {
    pub name: String,
    pub session: SessionClient,
    /// Every `tools` the hub sent, in order.
    pub tools: Vec<ToolList>,
    /// Every `notice` the hub sent, in order.
    pub notices: Vec<NoticeMsg>,
    next: u64,
}

impl Agent {
    /// The names of the tools the session started with.
    pub fn started_with(&self) -> Vec<String> {
        names(&self.session.session.tools)
    }

    fn keep(&mut self, message: Message) {
        match message {
            Message::Tools(tools) => self.tools.push(tools.tools),
            Message::Notice(notice) => self.notices.push(notice),
            Message::Result(_) => {}
            other => panic!("the hub sent {:?}", other.kind()),
        }
    }

    /// A call, and its outcome. What else comes before the answer is kept.
    pub async fn call(&mut self, tool: &str, args: Value) -> CallOutcome {
        self.next += 1;
        let id = 1_000_000 + self.next;
        self.session.send_call(id, tool, args).await.unwrap();
        loop {
            match self.session.pump(PATIENCE).await {
                Ok(Some(Message::Result(result))) if result.id.get() == id => {
                    return result.outcome;
                }
                Ok(Some(message)) => self.keep(message),
                Ok(None) => {}
                Err(error) => panic!("no answer to {tool}: {error:?}"),
            }
        }
    }

    /// `hermes_send_message` to a peer, in a new thread.
    pub async fn send_to(&mut self, to: &str, text: &str) -> CallOutcome {
        self.call("hermes_send_message", json!({"to": to, "text": text}))
            .await
    }

    /// `hermes_send_message` in a thread.
    pub async fn send_in(&mut self, thread: &str, text: &str) -> CallOutcome {
        self.call(
            "hermes_send_message",
            json!({"thread": thread, "text": text}),
        )
        .await
    }

    /// Sends a message to a peer and returns its thread.
    pub async fn thread_with(&mut self, to: &str, text: &str) -> String {
        let sent = ok(self.send_to(to, text).await);
        sent["thread"].as_str().unwrap().to_owned()
    }

    /// `hermes_read_thread`, and the texts it gave.
    pub async fn read(&mut self, thread: &str) -> Vec<String> {
        let read = ok(self
            .call("hermes_read_thread", json!({"thread": thread}))
            .await);
        texts(&read)
    }

    /// Waits for the next `tools`, and returns its names.
    pub async fn next_tools(&mut self) -> Vec<String> {
        let seen = self.tools.len();
        loop {
            if self.tools.len() > seen {
                return names(self.tools.last().unwrap());
            }
            match self.session.pump(PATIENCE).await {
                Ok(Some(message)) => self.keep(message),
                Ok(None) => {}
                Err(KitError::TimedOut) => panic!("no `tools` came"),
                Err(error) => panic!("{error:?}"),
            }
        }
    }

    /// Waits for the next `notice`.
    pub async fn next_notice(&mut self) -> NoticeMsg {
        let seen = self.notices.len();
        loop {
            if self.notices.len() > seen {
                return self.notices.last().unwrap().clone();
            }
            match self.session.pump(PATIENCE).await {
                Ok(Some(message)) => self.keep(message),
                Ok(None) => {}
                Err(KitError::TimedOut) => panic!("no `notice` came"),
                Err(error) => panic!("{error:?}"),
            }
        }
    }

    /// Keeps the session for [`QUIET`], and says how many notices came meanwhile.
    pub async fn notices_while_quiet(&mut self) -> usize {
        let seen = self.notices.len();
        let until = tokio::time::Instant::now() + QUIET;
        loop {
            let left = until.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                return self.notices.len() - seen;
            }
            match self.session.pump(left).await {
                Ok(Some(message)) => self.keep(message),
                Ok(None) | Err(KitError::TimedOut) => {}
                Err(error) => panic!("{error:?}"),
            }
        }
    }

    /// Answers a notice.
    pub async fn acknowledge(&mut self, notice: &NoticeMsg, outcome: AckOutcome) {
        let ack = Message::NoticeAck(NoticeAck {
            id: notice.id,
            outcome,
        });
        self.session.connector().send(&ack).await.unwrap();
        // The answer of a call comes after the hub handled the `notice_ack` before it: the
        // test moves the clock only after that.
        ok(self.call("arauto_status", json!({})).await);
    }
}

pub fn names(list: &ToolList) -> Vec<String> {
    list.as_slice()
        .iter()
        .map(|name| name.as_str().to_owned())
        .collect()
}

/// The content of an answer that went through.
pub fn ok(outcome: CallOutcome) -> Map<String, Value> {
    match outcome {
        CallOutcome::Ok(content) => content.into_map(),
        CallOutcome::Refused { code, .. } => panic!("refused with {code}"),
    }
}

/// The code of a refusal; `None` when the call went through.
pub fn refused(outcome: &CallOutcome) -> Option<ErrorCode> {
    match outcome {
        CallOutcome::Ok(_) => None,
        CallOutcome::Refused { code, .. } => Some(*code),
    }
}

/// The wait of a refusal, in seconds, when it says one.
pub fn retry_after(outcome: &CallOutcome) -> Option<u32> {
    match outcome {
        CallOutcome::Refused { retry_after, .. } => retry_after.map(|wait| wait.get()),
        CallOutcome::Ok(_) => None,
    }
}

/// The texts of the messages of a read, in order.
pub fn texts(read: &Map<String, Value>) -> Vec<String> {
    read["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["text"].as_str().unwrap().to_owned())
        .collect()
}

/// The sender and the count of a notice of messages.
pub fn peer_notice(notice: &NoticeMsg) -> (String, u16) {
    match notice.validate().unwrap() {
        Notice::Peer { from, count } => (from.as_str().to_owned(), count.get()),
        Notice::Whatsapp { .. } => panic!("a notice of WhatsApp"),
    }
}
