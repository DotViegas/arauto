//! A Hermes in session with the hub of a [`World`], with the steps of the admin on the matrix
//! of accesses as functions: the tests of the WhatsApp tools start from here.

use std::time::Duration;

use arauto_hub::core::access::{Grant, Scope, Verbs};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{CallOutcome, Message, ToolList};
use arauto_testkit::KitError;
use arauto_testkit::device::SessionClient;
use serde_json::{Map, Value};

use crate::world::World;

/// Patience for an answer of the hub.
pub const PATIENCE: Duration = Duration::from_secs(20);

pub const fn verbs(read: bool, send: bool, modify: bool, wake: bool) -> Verbs {
    Verbs {
        read,
        send,
        modify,
        wake,
    }
}

pub const READ: Verbs = verbs(true, false, false, false);
pub const READ_SEND: Verbs = verbs(true, true, false, false);
pub const ALL_VERBS: Verbs = verbs(true, true, true, true);
pub const NONE: Verbs = verbs(false, false, false, false);

pub const fn all(verbs: Verbs) -> Grant {
    Grant {
        scope: Scope::All,
        verbs,
        can_start_chats: false,
    }
}

pub const fn list(verbs: Verbs) -> Grant {
    Grant {
        scope: Scope::List,
        verbs,
        can_start_chats: false,
    }
}

/// A Hermes connected to the hub, in session.
pub struct Agent {
    pub name: String,
    pub session: SessionClient,
    /// Every `tools` the hub sent, in order.
    pub tools: Vec<ToolList>,
    next: u64,
}

impl Agent {
    /// A Hermes of a session opened by hand.
    pub fn from_session(name: String, session: SessionClient) -> Self {
        Self {
            name,
            session,
            tools: Vec::new(),
            next: 0,
        }
    }

    /// The names of the tools the session started with.
    pub fn started_with(&self) -> Vec<String> {
        names(&self.session.session.tools)
    }

    /// A call, and its outcome. A `tools` that comes before the answer is kept.
    pub async fn call(&mut self, tool: &str, args: Value) -> CallOutcome {
        self.next += 1;
        let id = 1_000_000 + self.next;
        self.session.send_call(id, tool, args).await.unwrap();
        self.until_result(id).await
    }

    /// Sends a call without waiting for its answer, with the identifier `id`.
    pub async fn send(&mut self, id: u64, tool: &str, args: Value) {
        self.session.send_call(id, tool, args).await.unwrap();
    }

    /// The outcome of the call `id`.
    pub async fn until_result(&mut self, id: u64) -> CallOutcome {
        loop {
            match self.session.pump(PATIENCE).await {
                Ok(Some(Message::Result(result))) if result.id.get() == id => {
                    return result.outcome;
                }
                Ok(Some(Message::Result(_)) | None) => {}
                Ok(Some(Message::Tools(tools))) => self.tools.push(tools.tools),
                Ok(Some(other)) => panic!("the hub sent {:?}", other.kind()),
                Err(error) => panic!("no answer to call {id}: {error:?}"),
            }
        }
    }

    /// The outcomes of the calls `ids`, in that order, whatever order they come in.
    pub async fn results(&mut self, ids: &[u64]) -> Vec<CallOutcome> {
        let mut found: Vec<Option<CallOutcome>> = ids.iter().map(|_| None).collect();
        while found.iter().any(Option::is_none) {
            match self.session.pump(PATIENCE).await {
                Ok(Some(Message::Result(result))) => {
                    if let Some(at) = ids.iter().position(|id| *id == result.id.get()) {
                        found[at] = Some(result.outcome);
                    }
                }
                Ok(None) => {}
                Ok(Some(Message::Tools(tools))) => self.tools.push(tools.tools),
                Ok(Some(other)) => panic!("the hub sent {:?}", other.kind()),
                Err(error) => panic!("no answer: {error:?}"),
            }
        }
        found.into_iter().map(Option::unwrap).collect()
    }

    /// Waits for the next `tools`, and returns its names.
    pub async fn next_tools(&mut self) -> Vec<String> {
        let seen = self.tools.len();
        loop {
            if self.tools.len() > seen {
                return names(self.tools.last().unwrap());
            }
            match self.session.pump(PATIENCE).await {
                Ok(Some(Message::Tools(tools))) => self.tools.push(tools.tools),
                Ok(None) => {}
                Ok(Some(other)) => panic!("the hub sent {:?}", other.kind()),
                Err(KitError::TimedOut) => panic!("no `tools` came"),
                Err(error) => panic!("{error:?}"),
            }
        }
    }

    /// Keeps the session for `time`, and says whether a `tools` came meanwhile.
    pub async fn quiet_for(&mut self, time: Duration) -> bool {
        let seen = self.tools.len();
        let until = tokio::time::Instant::now() + time;
        loop {
            let left = until.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                return self.tools.len() == seen;
            }
            match self.session.pump(left).await {
                Ok(Some(Message::Tools(tools))) => self.tools.push(tools.tools),
                Ok(None) | Err(KitError::TimedOut) => {}
                Ok(Some(other)) => panic!("the hub sent {:?}", other.kind()),
                Err(error) => panic!("{error:?}"),
            }
        }
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

impl World {
    /// A new Hermes, connected and in session.
    pub async fn agent(&self) -> Agent {
        let (name, session) = self.fixture.connected().await;
        Agent {
            name,
            session,
            tools: Vec::new(),
            next: 0,
        }
    }

    /// `admin access grant`.
    pub async fn grant(&self, hermes: &str, number: &str, grant: Grant) {
        self.fixture
            .admin
            .access_grant(hermes, number, grant, self.now())
            .await
            .unwrap();
    }

    /// `admin access remove`.
    pub async fn take_away(&self, hermes: &str, number: &str) {
        self.fixture
            .admin
            .access_remove(hermes, number, self.now())
            .await
            .unwrap();
    }

    /// `admin access chat`.
    pub async fn line(&self, hermes: &str, number: &str, chat: &str, verbs: Verbs) {
        self.fixture
            .admin
            .access_chat(&self.vault, hermes, number, chat, verbs, self.now())
            .await
            .unwrap();
    }
}
