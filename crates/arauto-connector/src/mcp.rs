//! The server of tools of Hermes: MCP over stdio, written in the project (`docs/15-mvp.md`,
//! section 4; `docs/09-contrato-mcp.md`). JSON-RPC 2.0, one message per line ([`crate::rpc`]).
//!
//! | Method | Answer |
//! |---|---|
//! | `initialize` | The version asked, when this server speaks it, else the newest it speaks ([`PROTOCOL_VERSIONS`]); the capability `tools` with `listChanged`; the name and the version of the program. A second one is an error. |
//! | `notifications/initialized` | Nothing. From then on, a change of the list of tools is told with `notifications/tools/list_changed`. |
//! | `ping` | `{}`, at any moment. |
//! | `tools/list` | The tools of the catalog that are released: by the session, or by the last session (`state.json`), and `arauto_status` always. Names and descriptions come from the catalog compiled here, never from the hub. |
//! | `tools/call` | The content as `structuredContent` and as its text. A refusal of the hub or of the connector is a result with `isError` and the fixed phrase `AR-xxx: ...` of the catalog. A tool that is not in the catalog is the error `-32602`. |
//! | `notifications/cancelled` | The call gets no answer. It is not cut: a call half written would break the channel. |
//! | Any other method | `-32601`. A notification of any method never gets an answer. |
//!
//! - **Before `initialize`** only `ping` is answered: anything else is `-32600`. A client of
//!   the versions without handshake that probes with `server/discover` gets `-32601`, and falls
//!   back to `initialize`.
//! - **The arguments of a call** are checked here against the catalog (AR-711) before anything
//!   leaves; the hub checks them again.
//! - **The end**: when the input ends, a stop signal comes or a `join` asks the program to end,
//!   the calls in progress are dropped, what was queued for the output leaves, and the server
//!   returns. The standard output carries only MCP: the log goes to the standard error.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use arauto_protocol::errors::{ErrorCode, RetryAfter};
use arauto_protocol::limits::CONNECTOR_MESSAGE_MAX;
use arauto_protocol::message::{CallOutcome, Payload};
use arauto_protocol::tools::{self, CATALOG, ToolCall, ToolFamily, ToolName, ToolSpec};
use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use futures_util::stream::FuturesUnordered;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use crate::rpc::{self, Id, Incoming, Lines, Params, Read, RpcError};

/// The versions of MCP this server speaks, the newest first: the revisions with the handshake
/// of `initialize` and without batches. The revisions without handshake (`2026-07-28` on) are
/// not spoken.
pub const PROTOCOL_VERSIONS: [&str; 2] = ["2025-11-25", "2025-06-18"];

/// The name of this server in `initialize`.
pub const SERVER_INFO_NAME: &str = "arauto-connector";

/// The longest line read: a call of the largest size the hub takes, and its envelope.
pub(crate) const LINE_MAX: usize = CONNECTOR_MESSAGE_MAX + 64 * 1024;

/// Calls in progress at most: the next one is refused at once with AR-710.
pub(crate) const CALLS_IN_FLIGHT_MAX: usize = 64;

/// Lines that wait to be written to the output.
const OUTPUT_QUEUE: usize = 64;

/// How long the lines still queued have to leave at the end.
const FLUSH_PATIENCE: Duration = Duration::from_millis(100);

/// The notification of a new list of tools.
const LIST_CHANGED: &str = "notifications/tools/list_changed";

/// What the server needs from the connector.
pub(crate) trait Backend: Sync {
    /// The released tools: of the session, or of the last session.
    fn tools(&self) -> Vec<ToolName>;

    /// The released tools, and each change of the list.
    fn watch_tools(&self) -> watch::Receiver<Vec<ToolName>>;

    /// Calls `tool` with `args`, through the channel of the leader.
    fn call(&self, tool: ToolName, args: Payload) -> impl Future<Output = CallOutcome> + Send;
}

impl Backend for crate::runtime::Connector {
    fn tools(&self) -> Vec<ToolName> {
        crate::runtime::Connector::tools(self)
    }

    fn watch_tools(&self) -> watch::Receiver<Vec<ToolName>> {
        crate::runtime::Connector::watch_tools(self)
    }

    async fn call(&self, tool: ToolName, args: Payload) -> CallOutcome {
        crate::runtime::Connector::call(self, tool.as_str(), args.into_map()).await
    }
}

/// Why the server returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ended {
    /// The input ended: Hermes closed it to stop the server.
    Input,
    /// The output could not be written any more.
    Output,
    /// The stop came from outside: a signal, or a `join` asked the program to end.
    Stopped,
}

/// The tools the client sees, in the order of the catalog: the ones `released` names that this
/// connector knows, and the ones that are always there.
fn listed(released: &[ToolName]) -> Vec<&'static ToolSpec> {
    CATALOG
        .iter()
        .filter(|spec| {
            spec.family == ToolFamily::Always
                || released.iter().any(|tool| tool.as_str() == spec.name)
        })
        .collect()
}

fn names(listed: &[&'static ToolSpec]) -> Vec<&'static str> {
    listed.iter().map(|spec| spec.name).collect()
}

fn tools_result(listed: &[&'static ToolSpec]) -> Value {
    let tools: Vec<Value> = listed
        .iter()
        .map(|spec| {
            json!({
                "name": spec.name,
                "description": spec.description,
                "inputSchema": spec.input_schema(),
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// The version answered to a client that asked for `requested`.
fn negotiate(requested: &str) -> &'static str {
    PROTOCOL_VERSIONS
        .iter()
        .find(|version| **version == requested)
        .copied()
        .unwrap_or(PROTOCOL_VERSIONS[0])
}

fn initialize_result(version: &'static str) -> Value {
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": true } },
        "serverInfo": {
            "name": SERVER_INFO_NAME,
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

/// The result of a call that ran: the content as structured content, and as its text.
fn success(content: Payload) -> Value {
    let structured = Value::Object(content.into_map());
    let text = serde_json::to_string(&structured).unwrap_or_default();
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
        "isError": false,
    })
}

/// The result of a refusal: the fixed phrase of the code, and nothing else.
fn refusal(code: ErrorCode, retry_after: Option<RetryAfter>) -> Value {
    json!({
        "content": [{ "type": "text", "text": code.phrase(retry_after) }],
        "isError": true,
    })
}

/// One call, from the arguments to its result. The arguments are checked against the catalog
/// before anything leaves: what passes is a few KiB at most, far below the limit of a message.
async fn call_tool<B: Backend>(
    backend: &B,
    spec: &'static ToolSpec,
    args: Map<String, Value>,
) -> Value {
    if ToolCall::parse(spec.name, &args).is_err() {
        return refusal(ErrorCode::Ar711, None);
    }
    match backend.call(spec.tool_name(), Payload::new(args)).await {
        CallOutcome::Ok(content) => success(content),
        CallOutcome::Refused { code, retry_after } => refusal(code, retry_after),
    }
}

/// Where the conversation with the client is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before `initialize`.
    Fresh,
    /// `initialize` was answered; the client has not said it is ready.
    Initialized,
    /// The client said it is ready: changes of the list are told.
    Ready,
}

/// What the answer to a line asks of the loop.
enum Action {
    /// Nothing more.
    None,
    /// Write this line.
    Write(Vec<u8>),
    /// Start this call.
    Call {
        id: Id,
        spec: &'static ToolSpec,
        args: Map<String, Value>,
    },
}

/// The state of the conversation.
struct Server<'a, B> {
    backend: &'a B,
    phase: Phase,
    /// The calls in progress, and whether each was cancelled.
    in_flight: HashMap<Id, bool>,
    /// The list of tools the client was last given or told of.
    shown: Vec<&'static str>,
    /// The changes of the list, from `initialize` on.
    changes: Option<watch::Receiver<Vec<ToolName>>>,
}

impl<'a, B: Backend> Server<'a, B> {
    fn new(backend: &'a B) -> Self {
        Self {
            backend,
            phase: Phase::Fresh,
            in_flight: HashMap::new(),
            shown: Vec::new(),
            changes: None,
        }
    }

    fn line(&mut self, bytes: &[u8]) -> Action {
        // A line of blanks carries no message.
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Action::None;
        }
        match rpc::parse(bytes) {
            Incoming::Request { id, method, params } => self.request(id, &method, params),
            Incoming::Notification { method, params } => {
                self.notification(&method, params);
                Action::None
            }
            Incoming::Dropped => Action::None,
            Incoming::Invalid { id, error } => Action::Write(rpc::error(id.as_ref(), error)),
        }
    }

    fn request(&mut self, id: Id, method: &str, params: Params) -> Action {
        let answer = match method {
            "initialize" => self.initialize(params),
            "ping" => Ok(json!({})),
            "tools/list" | "tools/call" if self.phase == Phase::Fresh => {
                Err(RpcError::NOT_INITIALIZED)
            }
            "tools/list" => self.list(params),
            "tools/call" => return self.call(id, params),
            _ => Err(RpcError::METHOD_NOT_FOUND),
        };
        Action::Write(match answer {
            Ok(result) => rpc::result(&id, result),
            Err(error) => rpc::error(Some(&id), error),
        })
    }

    fn initialize(&mut self, params: Params) -> Result<Value, RpcError> {
        if self.phase != Phase::Fresh {
            return Err(RpcError::ALREADY_INITIALIZED);
        }
        let params = params.object().ok_or(RpcError::INVALID_PARAMS)?;
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or(RpcError::INVALID_PARAMS)?;
        let version = negotiate(requested);
        self.phase = Phase::Initialized;
        let mut changes = self.backend.watch_tools();
        self.shown = names(&listed(&changes.borrow_and_update()));
        self.changes = Some(changes);
        tracing::info!(version, "the client of MCP initialized the server of tools");
        Ok(initialize_result(version))
    }

    fn list(&mut self, params: Params) -> Result<Value, RpcError> {
        let params = params.object().ok_or(RpcError::INVALID_PARAMS)?;
        // This server never gives a cursor: the whole list fits one answer.
        if params.get("cursor").is_some_and(|cursor| !cursor.is_null()) {
            return Err(RpcError::INVALID_CURSOR);
        }
        let listed = listed(&self.backend.tools());
        self.shown = names(&listed);
        Ok(tools_result(&listed))
    }

    fn call(&mut self, id: Id, params: Params) -> Action {
        let Some(mut params) = params.object() else {
            return Action::Write(rpc::error(Some(&id), RpcError::INVALID_PARAMS));
        };
        let Some(Value::String(name)) = params.remove("name") else {
            return Action::Write(rpc::error(Some(&id), RpcError::INVALID_PARAMS));
        };
        let args = match params.remove("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(args)) => args,
            Some(_) => return Action::Write(rpc::error(Some(&id), RpcError::INVALID_PARAMS)),
        };
        let Some(spec) = tools::find(&name) else {
            return Action::Write(rpc::error(Some(&id), RpcError::UNKNOWN_TOOL));
        };
        if self.in_flight.contains_key(&id) {
            return Action::Write(rpc::error(Some(&id), RpcError::ID_IN_USE));
        }
        if self.in_flight.len() >= CALLS_IN_FLIGHT_MAX {
            return Action::Write(rpc::result(&id, refusal(ErrorCode::Ar710, None)));
        }
        self.in_flight.insert(id.clone(), false);
        Action::Call { id, spec, args }
    }

    fn notification(&mut self, method: &str, params: Params) {
        match method {
            "notifications/initialized" if self.phase == Phase::Initialized => {
                self.phase = Phase::Ready;
            }
            "notifications/cancelled" => {
                let id = params
                    .object()
                    .and_then(|params| params.get("requestId").and_then(Id::read));
                if let Some(cancelled) = id.and_then(|id| self.in_flight.get_mut(&id)) {
                    *cancelled = true;
                }
            }
            _ => {}
        }
    }

    /// A call ended: its result, unless the client cancelled it.
    fn answered(&mut self, id: &Id, result: Value) -> Action {
        match self.in_flight.remove(id) {
            Some(false) => Action::Write(rpc::result(id, result)),
            Some(true) | None => Action::None,
        }
    }

    /// The list of tools changed: the client is told, when it is ready and the list it sees
    /// is another one.
    fn changed(&mut self) -> Action {
        let Some(changes) = self.changes.as_mut() else {
            return Action::None;
        };
        let now = names(&listed(&changes.borrow_and_update()));
        if now == self.shown {
            return Action::None;
        }
        self.shown = now;
        Action::Write(rpc::notification(LIST_CHANGED))
    }
}

/// Waits for a change of the list, when the client is ready for it.
async fn list_change(changes: &mut Option<watch::Receiver<Vec<ToolName>>>, ready: bool) {
    match changes.as_mut() {
        Some(changes) if ready => {
            if changes.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
        _ => std::future::pending::<()>().await,
    }
}

async fn read_lines<R: AsyncBufRead + Unpin>(input: R, lines: mpsc::Sender<Read>) {
    let mut reader = Lines::new(input, LINE_MAX);
    loop {
        let read = reader.next().await.unwrap_or(Read::End);
        let end = read == Read::End;
        if lines.send(read).await.is_err() || end {
            return;
        }
    }
}

async fn write_lines<W: AsyncWrite + Unpin>(mut output: W, mut lines: mpsc::Receiver<Vec<u8>>) {
    while let Some(line) = lines.recv().await {
        if output.write_all(&line).await.is_err() || output.flush().await.is_err() {
            return;
        }
    }
}

/// Serves MCP to the client of `input` and `output` until the input ends, the output fails or
/// `stop` resolves.
pub(crate) async fn serve<B, R, W>(
    backend: &B,
    input: R,
    output: W,
    stop: impl Future<Output = ()>,
) -> Ended
where
    B: Backend,
    R: AsyncBufRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // One line read ahead at most: a line may be as large as the ceiling.
    let (lines_in, mut lines) = mpsc::channel(1);
    let reader = tokio::spawn(read_lines(input, lines_in));
    let (out, lines_out) = mpsc::channel(OUTPUT_QUEUE);
    let writer = tokio::spawn(write_lines(output, lines_out));
    let mut server = Server::new(backend);
    let mut calls: FuturesUnordered<BoxFuture<'_, (Id, Value)>> = FuturesUnordered::new();
    tokio::pin!(stop);
    let ended = loop {
        let ready = server.phase == Phase::Ready;
        // The calls before the input: a client that floods the input does not hold its own
        // calls back, and there are at most `CALLS_IN_FLIGHT_MAX` of them.
        let action = tokio::select! {
            biased;
            () = &mut stop => break Ended::Stopped,
            Some((id, result)) = calls.next() => server.answered(&id, result),
            () = list_change(&mut server.changes, ready) => server.changed(),
            read = lines.recv() => match read {
                Some(Read::Line(bytes)) => server.line(&bytes),
                Some(Read::TooLong) => Action::Write(rpc::error(None, RpcError::TOO_LARGE)),
                Some(Read::End) | None => break Ended::Input,
            },
        };
        match action {
            Action::None => {}
            Action::Write(line) => {
                // A client that stopped reading does not hold the stop back.
                let sent = tokio::select! {
                    biased;
                    () = &mut stop => break Ended::Stopped,
                    sent = out.send(line) => sent,
                };
                if sent.is_err() {
                    break Ended::Output;
                }
            }
            Action::Call { id, spec, args } => {
                calls.push(Box::pin(async move {
                    let result = call_tool(backend, spec, args).await;
                    (id, result)
                }));
            }
        }
    };
    reader.abort();
    // The calls still in progress are dropped: the client is gone, or the program ends.
    drop(calls);
    drop(out);
    let _ = tokio::time::timeout(FLUSH_PATIENCE, writer).await;
    ended
}

#[cfg(test)]
mod tests;
