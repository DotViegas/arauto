//! A copy of the connector that is not the leader: it sends its calls to the leader through the
//! local socket (`docs/11-conector.md`, section 3; `docs/14-protocolo-do-canal.md`, section 11).
//!
//! Two connections to the leader: one for the requests, whose answers come by number, and one
//! that watches the list of tools. When either ends, the leader is gone: a call that was
//! written to it and got no answer is AR-207, one that was not written is AR-203, and the copy
//! tries to become the leader; unless the leader said it ends for a restart, and then this copy
//! ends too.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arauto_platform::LocalStream;
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::message::{CallOutcome, Payload};
use arauto_protocol::tools::ToolName;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::{Notify, oneshot, watch};
use tokio::task::JoinHandle;

use crate::home::Home;
use crate::local::{self, Lines};
use crate::session::refused;
use crate::settings::{Settings, duration};
use crate::status::{Role, Status};

/// The answers the leader owes, by number.
#[derive(Default)]
struct Book {
    pending: HashMap<u64, oneshot::Sender<Map<String, Value>>>,
    closed: bool,
}

struct Shared {
    book: Mutex<Book>,
    next: AtomicU64,
    writer: tokio::sync::Mutex<WriteHalf<LocalStream>>,
    gone: Notify,
    tools: watch::Sender<Vec<ToolName>>,
    /// The leader said it ends for a restart: this copy ends too.
    restart: AtomicBool,
}

impl Shared {
    fn book(&self) -> MutexGuard<'_, Book> {
        // A map of senders: a panic elsewhere leaves it usable.
        self.book.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The leader is gone: every answer still owed is dropped, and the copy learns of it.
    fn close(&self) {
        let mut book = self.book();
        book.closed = true;
        book.pending.clear();
        drop(book);
        self.gone.notify_waiters();
    }
}

/// How an answer of the leader went.
enum Asked {
    Answered(Map<String, Value>),
    /// The request was never written: the leader was gone before.
    NotSent,
    /// The request was written and no answer came.
    Lost,
}

/// The connection of a copy to its leader.
pub(crate) struct Relay {
    shared: Arc<Shared>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    settings: Arc<Settings>,
}

impl Relay {
    /// Connects to the leader of `home`.
    pub(crate) async fn connect(
        home: &Home,
        settings: Arc<Settings>,
        tools: watch::Sender<Vec<ToolName>>,
    ) -> Result<Self, arauto_platform::LocalSocketError> {
        let requests = LocalStream::connect(home.root()).await?;
        let watching = LocalStream::connect(home.root()).await?;
        let (reader, writer) = tokio::io::split(requests);
        let mut first_list = tools.subscribe();
        let shared = Arc::new(Shared {
            book: Mutex::new(Book::default()),
            next: AtomicU64::new(0),
            writer: tokio::sync::Mutex::new(writer),
            gone: Notify::new(),
            tools,
            restart: AtomicBool::new(false),
        });
        let answers = tokio::spawn(read_answers(Arc::clone(&shared), reader));
        let watch = tokio::spawn(watch_tools(Arc::clone(&shared), watching));
        // The list of the leader, before this copy serves: it lists what the leader lists.
        let _ =
            tokio::time::timeout(duration(settings.local_answer_ms), first_list.changed()).await;
        #[cfg(feature = "testing")]
        settings.emit(|| crate::testing::Event::Role { leader: false });
        tracing::info!("this copy of the connector forwards to the leader of its folder");
        Ok(Self {
            shared,
            tasks: Mutex::new(vec![answers, watch]),
            settings,
        })
    }

    /// Resolves when the leader is gone.
    pub(crate) async fn gone(&self) {
        loop {
            let notified = self.shared.gone.notified();
            if self.shared.book().closed {
                return;
            }
            notified.await;
        }
    }

    /// Whether the leader went away for a restart: then this copy ends too, instead of taking
    /// its place.
    pub(crate) fn restarting(&self) -> bool {
        self.shared.restart.load(Ordering::SeqCst)
    }

    async fn ask(&self, mut request: Map<String, Value>) -> Asked {
        let id = self
            .shared
            .next
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        request.insert("v".to_owned(), Value::from(local::VERSION));
        request.insert("id".to_owned(), Value::from(id));
        let (answer, answered) = oneshot::channel();
        {
            let mut book = self.shared.book();
            if book.closed {
                return Asked::NotSent;
            }
            book.pending.insert(id, answer);
        }
        let line = local::line(&Value::Object(request));
        let written = {
            let mut writer = self.shared.writer.lock().await;
            writer.write_all(&line).await.is_ok() && writer.flush().await.is_ok()
        };
        if !written {
            self.shared.book().pending.remove(&id);
            self.shared.close();
            return Asked::NotSent;
        }
        // Written: without an answer, the result is unknown.
        match answered.await {
            Ok(answer) => Asked::Answered(answer),
            Err(_) => Asked::Lost,
        }
    }

    /// A call, through the leader.
    pub(crate) async fn call(&self, tool: ToolName, args: Payload) -> CallOutcome {
        let mut request = Map::new();
        request.insert("op".to_owned(), Value::from("call"));
        request.insert("tool".to_owned(), Value::from(tool.as_str()));
        request.insert("args".to_owned(), Value::Object(args.into_map()));
        match self.ask(request).await {
            Asked::Answered(answer) => local::read_call_answer(&answer),
            Asked::NotSent => refused(ErrorCode::Ar203),
            Asked::Lost => refused(ErrorCode::Ar207),
        }
    }

    /// The situation, as the leader sees it.
    pub(crate) async fn status(&self) -> Option<Status> {
        let request = json!({"op": "status"});
        let Value::Object(request) = request else {
            return None;
        };
        let asked =
            tokio::time::timeout(duration(self.settings.local_answer_ms), self.ask(request)).await;
        let Ok(Asked::Answered(answer)) = asked else {
            return None;
        };
        let mut status = Status::from_json(answer.get("status")?)?;
        status.role = Role::Relay;
        Some(status)
    }

    /// Closes the connections.
    pub(crate) fn stop(&self) {
        let tasks = std::mem::take(&mut *self.tasks.lock().unwrap_or_else(PoisonError::into_inner));
        for task in tasks {
            task.abort();
        }
        self.shared.close();
    }
}

async fn read_answers(shared: Arc<Shared>, reader: ReadHalf<LocalStream>) {
    let mut lines = Lines::new(BufReader::new(reader), local::ANSWER_LINE_MAX);
    while let Ok(Some(line)) = lines.next().await {
        let Ok(Value::Object(answer)) = arauto_protocol::message::read_strict(&line) else {
            continue;
        };
        if local::is_restart(&answer) {
            shared.restart.store(true, Ordering::SeqCst);
            continue;
        }
        let Some(id) = answer.get("id").and_then(Value::as_u64) else {
            continue;
        };
        let pending = shared.book().pending.remove(&id);
        if let Some(pending) = pending {
            let _ = pending.send(answer);
        }
    }
    shared.close();
}

async fn watch_tools(shared: Arc<Shared>, stream: LocalStream) {
    let (reader, mut writer) = tokio::io::split(stream);
    let line = local::line(&json!({"v": local::VERSION, "op": "watch"}));
    if writer.write_all(&line).await.is_err() || writer.flush().await.is_err() {
        shared.close();
        return;
    }
    let mut lines = Lines::new(BufReader::new(reader), local::ANSWER_LINE_MAX);
    while let Ok(Some(line)) = lines.next().await {
        let Ok(Value::Object(update)) = arauto_protocol::message::read_strict(&line) else {
            continue;
        };
        if local::is_restart(&update) {
            shared.restart.store(true, Ordering::SeqCst);
            continue;
        }
        if let Some(tools) = local::read_tools(&update) {
            shared.tools.send_replace(tools);
        }
    }
    drop(writer);
    shared.close();
}
