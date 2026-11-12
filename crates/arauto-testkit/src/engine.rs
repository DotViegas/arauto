//! A fake WhatsApp engine: a local HTTP server that speaks the format of the WAHA the hub
//! uses (`referencia/waha-fatos.md`, the section on the formats), with sessions, pairing,
//! chats, messages, groups, contacts and sending, all in memory.
//!
//! It keeps every request it got, so a test proves that a malicious value never reached it;
//! it sends events to the internal port of the hub, signed with the secret the hub configured
//! (or with any other, to be refused); and it fails on command: slow, a 5xx, malformed JSON,
//! an enormous body, another type of content, an event repeated or out of order.
//!
//! Never shipped, never a real engine, never a real number.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use arauto_protocol::clock::{Clock, SystemClock};
use bytes::Bytes;
use data_encoding::HEXLOWER;
use hkdf::hmac::{Hmac, KeyInit, Mac};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use sha2::Sha512;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::KitError;

/// The version the fake says it is, by default: the one `referencia/waha-fatos.md` checked.
pub const DEFAULT_VERSION: &str = "2026.10.1";

/// A request the fake received, as it came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The method.
    pub method: String,
    /// The path, exactly as written by the client (still encoded).
    pub path: String,
    /// The query, as written.
    pub query: String,
    /// The key the client sent, if any.
    pub api_key: Option<String>,
    /// The body.
    pub body: Vec<u8>,
}

impl Received {
    /// The body as JSON, when it is JSON.
    #[must_use]
    pub fn json(&self) -> Option<Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

/// A failure on command, for the requests whose path contains a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// Answers after this long.
    Slow(Duration),
    /// Answers 500.
    ServerError,
    /// Answers 200 with a body that is not JSON.
    Malformed,
    /// Answers 200 with a JSON-typed body of this many bytes.
    Huge(usize),
    /// Answers 200 with a body of another type of content.
    WrongType,
}

#[derive(Debug, Clone, Default)]
struct FakeChat {
    name: Option<String>,
    messages: Vec<Value>,
}

#[derive(Debug, Clone)]
struct FakeGroup {
    subject: String,
    participants: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct FakeSession {
    status: String,
    config: Value,
    /// The account connected, as `digits@c.us`.
    me: Option<String>,
    /// The telephone that connects when the QR or the code is used.
    pairs_as: Option<String>,
    /// The QR or the code was used: the next reading of the session finds it connected.
    pair_pending: bool,
    qr_served: u32,
    configured: u32,
    chats: BTreeMap<String, FakeChat>,
    groups: BTreeMap<String, FakeGroup>,
}

#[derive(Debug, Default)]
struct State {
    api_key: String,
    version: String,
    mechanism: String,
    sessions: BTreeMap<String, FakeSession>,
    received: Vec<Received>,
    faults: Vec<(String, Fault)>,
    contacts: BTreeMap<String, (String, String, String)>,
    registered: BTreeMap<String, String>,
    files: BTreeMap<String, (String, Vec<u8>)>,
    sent: Vec<(String, Value)>,
    edited: Vec<(String, String, Value)>,
    deleted: Vec<(String, String)>,
    counter: u64,
}

/// The fake engine, running on 127.0.0.1.
#[derive(Debug)]
pub struct FakeEngine {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}

impl Drop for FakeEngine {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl FakeEngine {
    /// Starts a fake engine that takes `api_key`.
    ///
    /// # Errors
    /// The network.
    pub async fn start(api_key: &str) -> Result<Self, KitError> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(State {
            api_key: api_key.to_owned(),
            version: DEFAULT_VERSION.to_owned(),
            mechanism: "GOWS".to_owned(),
            ..State::default()
        }));
        let shared = Arc::clone(&state);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let state = Arc::clone(&shared);
                tokio::spawn(async move {
                    let service = service_fn(move |request: Request<Incoming>| {
                        let state = Arc::clone(&state);
                        async move { Ok::<_, Infallible>(handle(&state, addr, request).await) }
                    });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        Ok(Self { addr, state, task })
    }

    /// `http://127.0.0.1:port`: what the hub is given as the address of the engine.
    #[must_use]
    pub fn address(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The socket address.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Every request received so far.
    #[must_use]
    pub fn received(&self) -> Vec<Received> {
        lock(&self.state).received.clone()
    }

    /// Forgets the requests received so far.
    pub fn forget_received(&self) {
        lock(&self.state).received.clear();
    }

    /// Fails every request whose path contains `needle` (`""` for all of them).
    pub fn fail(&self, needle: &str, fault: Fault) {
        lock(&self.state).faults.push((needle.to_owned(), fault));
    }

    /// No failure any more.
    pub fn heal(&self) {
        lock(&self.state).faults.clear();
    }

    /// Says it is another version.
    pub fn set_version(&self, version: &str, mechanism: &str) {
        let mut state = lock(&self.state);
        state.version = version.to_owned();
        state.mechanism = mechanism.to_owned();
    }

    /// Changes the key it takes.
    pub fn set_api_key(&self, key: &str) {
        lock(&self.state).api_key = key.to_owned();
    }

    /// The names of the sessions it has.
    #[must_use]
    pub fn session_names(&self) -> Vec<String> {
        lock(&self.state).sessions.keys().cloned().collect()
    }

    /// The configuration of a session, as the hub gave it.
    #[must_use]
    pub fn config(&self, session: &str) -> Option<Value> {
        lock(&self.state)
            .sessions
            .get(session)
            .map(|session| session.config.clone())
    }

    /// How many times a configuration was given to a session (creation included).
    #[must_use]
    pub fn configured(&self, session: &str) -> u32 {
        lock(&self.state)
            .sessions
            .get(session)
            .map_or(0, |session| session.configured)
    }

    /// Changes the configuration of a session from outside, as someone who reached the engine
    /// would.
    pub fn tamper_config(&self, session: &str, config: Value) {
        if let Some(found) = lock(&self.state).sessions.get_mut(session) {
            found.config = config;
        }
    }

    /// The status of a session.
    #[must_use]
    pub fn status(&self, session: &str) -> Option<String> {
        lock(&self.state)
            .sessions
            .get(session)
            .map(|session| session.status.clone())
    }

    /// Sets the status of a session.
    pub fn set_status(&self, session: &str, status: &str) {
        if let Some(found) = lock(&self.state).sessions.get_mut(session) {
            found.status = status.to_owned();
        }
    }

    /// The telephone that connects when the QR or the code of `session` is used. The next
    /// reading of the session after the QR or the code was served finds it working.
    pub fn pairs_as(&self, session: &str, phone: &str) {
        if let Some(found) = lock(&self.state).sessions.get_mut(session) {
            found.pairs_as = Some(phone.to_owned());
        }
    }

    /// Connects a session with the account of `phone` at once.
    pub fn connect(&self, session: &str, phone: &str) {
        if let Some(found) = lock(&self.state).sessions.get_mut(session) {
            found.me = Some(format!("{phone}@c.us"));
            found.status = "WORKING".to_owned();
        }
    }

    /// Adds a chat to a session.
    pub fn add_chat(&self, session: &str, chat: &str, name: &str) {
        if let Some(found) = lock(&self.state).sessions.get_mut(session) {
            found.chats.entry(chat.to_owned()).or_default().name = Some(name.to_owned());
        }
    }

    /// Adds a message to a chat of a session, as the engine keeps it.
    pub fn add_message(&self, session: &str, chat: &str, message: Value) {
        if let Some(found) = lock(&self.state).sessions.get_mut(session) {
            found
                .chats
                .entry(chat.to_owned())
                .or_default()
                .messages
                .push(message);
        }
    }

    /// Adds a group to a session.
    pub fn add_group(
        &self,
        session: &str,
        group: &str,
        subject: &str,
        participants: &[(&str, &str)],
    ) {
        if let Some(found) = lock(&self.state).sessions.get_mut(session) {
            found.groups.insert(
                group.to_owned(),
                FakeGroup {
                    subject: subject.to_owned(),
                    participants: participants
                        .iter()
                        .map(|(id, role)| ((*id).to_owned(), (*role).to_owned()))
                        .collect(),
                },
            );
        }
    }

    /// Adds a contact: its name, its own name and its profile phrase.
    pub fn add_contact(&self, contact: &str, name: &str, pushname: &str, about: &str) {
        lock(&self.state).contacts.insert(
            contact.to_owned(),
            (name.to_owned(), pushname.to_owned(), about.to_owned()),
        );
    }

    /// Says that `phone` (digits) has WhatsApp, with the chat `chat`.
    pub fn register(&self, phone: &str, chat: &str) {
        lock(&self.state)
            .registered
            .insert(phone.to_owned(), chat.to_owned());
    }

    /// A file the engine serves at `/api/files/{name}`.
    pub fn add_file(&self, name: &str, mimetype: &str, data: Vec<u8>) {
        lock(&self.state)
            .files
            .insert(name.to_owned(), (mimetype.to_owned(), data));
    }

    /// What was sent: the path of each sending and its body.
    #[must_use]
    pub fn sent(&self) -> Vec<(String, Value)> {
        lock(&self.state).sent.clone()
    }

    /// What was edited: the chat, the message and the body.
    #[must_use]
    pub fn edited(&self) -> Vec<(String, String, Value)> {
        lock(&self.state).edited.clone()
    }

    /// What was deleted: the chat and the message.
    #[must_use]
    pub fn deleted(&self) -> Vec<(String, String)> {
        lock(&self.state).deleted.clone()
    }

    /// Where a session sends its events, and the key that signs them, as configured.
    #[must_use]
    pub fn webhook(&self, session: &str) -> Option<(String, String)> {
        let state = lock(&self.state);
        let config = &state.sessions.get(session)?.config;
        let hook = config.get("webhooks")?.get(0)?;
        Some((
            hook.get("url")?.as_str()?.to_owned(),
            hook.get("hmac")?.get("key")?.as_str()?.to_owned(),
        ))
    }

    /// Sends an event of `session` to the address it is configured with, signed with the key
    /// it is configured with, as the engine does. Returns the status the hub answered.
    ///
    /// # Errors
    /// [`KitError::Unexpected`] when the session has no destination for its events; the
    /// network.
    pub async fn emit(&self, session: &str, event: &str, payload: Value) -> Result<u16, KitError> {
        let envelope = self.envelope(session, event, payload, None)?;
        self.deliver(session, &envelope).await
    }

    /// The body of an event, as the engine writes it. `id` is drawn when not given; the time
    /// is now.
    ///
    /// # Errors
    /// [`KitError::Unexpected`] when there is no such session.
    pub fn envelope(
        &self,
        session: &str,
        event: &str,
        payload: Value,
        id: Option<&str>,
    ) -> Result<Vec<u8>, KitError> {
        self.envelope_at(session, event, payload, id, SystemClock.now_ms())
    }

    /// [`FakeEngine::envelope`] at a given time, in milliseconds.
    ///
    /// # Errors
    /// [`KitError::Unexpected`] when there is no such session.
    pub fn envelope_at(
        &self,
        session: &str,
        event: &str,
        payload: Value,
        id: Option<&str>,
        timestamp: i64,
    ) -> Result<Vec<u8>, KitError> {
        let mut state = lock(&self.state);
        state.counter = state.counter.saturating_add(1);
        let drawn = format!("evt_{:026}", state.counter);
        let found = state
            .sessions
            .get(session)
            .ok_or_else(|| KitError::Unexpected("no such session".into()))?;
        let me = found
            .me
            .as_ref()
            .map_or(Value::Null, |me| json!({"id": me, "pushName": "~"}));
        Ok(json!({
            "id": id.unwrap_or(&drawn),
            "timestamp": timestamp,
            "event": event,
            "session": session,
            "metadata": {},
            "me": me,
            "payload": payload,
            "engine": "GOWS",
            "environment": {"tier": "CORE", "version": state.version},
        })
        .to_string()
        .into_bytes())
    }

    /// Delivers `body` as an event of `session`, signed with the key the session has.
    ///
    /// # Errors
    /// [`KitError::Unexpected`] when the session has no destination; the network.
    pub async fn deliver(&self, session: &str, body: &[u8]) -> Result<u16, KitError> {
        let (url, key) = self
            .webhook(session)
            .ok_or_else(|| KitError::Unexpected("the session has no destination".into()))?;
        let signature = sign(key.as_bytes(), body);
        post(
            &url,
            &[
                ("Content-Type", "application/json"),
                ("X-Webhook-Hmac", &signature),
                ("X-Webhook-Hmac-Algorithm", "sha512"),
            ],
            body,
        )
        .await
    }
}

/// HMAC-SHA-512 of `body` keyed by `key`, in hexadecimal: the signature of the engine.
#[must_use]
pub fn sign(key: &[u8], body: &[u8]) -> String {
    #[allow(clippy::expect_used)]
    let mut mac = <Hmac<Sha512> as KeyInit>::new_from_slice(key).expect("any key length");
    mac.update(body);
    HEXLOWER.encode(&mac.finalize().into_bytes())
}

/// Posts `body` to `url` (an `http://` address) and returns the status of the answer.
///
/// # Errors
/// [`KitError`] when the connection fails or no status comes back in time.
pub async fn post(url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<u16, KitError> {
    let framing = format!("Content-Length: {}\r\n", body.len());
    exchange(url, &framing, headers, body).await
}

/// Sends only the head of a request that declares a body of `declared` bytes, and returns the
/// status of the answer: what a server that refuses by the declared length answers, without a
/// body left unread on the connection (which some systems turn into a reset that loses the
/// answer).
///
/// # Errors
/// [`KitError`] when the connection fails or no status comes back in time.
pub async fn post_declaring(
    url: &str,
    headers: &[(&str, &str)],
    declared: u64,
) -> Result<u16, KitError> {
    let framing = format!("Content-Length: {declared}\r\n");
    exchange(url, &framing, headers, &[]).await
}

/// Sends `body` as one chunk of a chunked body that never ends, and returns the status of the
/// answer: a body whose length is not declared, for a server that must count what it reads.
///
/// # Errors
/// [`KitError`] when the connection fails or no status comes back in time.
pub async fn post_chunked(
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<u16, KitError> {
    let mut chunk = format!("{:x}\r\n", body.len()).into_bytes();
    chunk.extend_from_slice(body);
    exchange(url, "Transfer-Encoding: chunked\r\n", headers, &chunk).await
}

async fn exchange(
    url: &str,
    framing: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<u16, KitError> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| KitError::Unexpected("not an http address".into()))?;
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let path = if path.is_empty() { "/" } else { path };
    let work = async {
        let stream = TcpStream::connect(authority).await?;
        let (mut reader, mut writer) = stream.into_split();
        let mut head =
            format!("POST {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n{framing}");
        for (name, value) in headers {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        writer.write_all(head.as_bytes()).await?;
        // The body is written while the answer is read: a server that refuses early answers
        // and closes before the whole body is written, and the answer must be read before the
        // write fails.
        let write = async {
            for piece in body.chunks(16 * 1024) {
                if writer.write_all(piece).await.is_err() {
                    break;
                }
            }
        };
        let read = async {
            let mut answer = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = reader.read(&mut buffer).await?;
                if read == 0 {
                    break;
                }
                answer.extend_from_slice(buffer.get(..read).unwrap_or_default());
                if answer.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Ok::<_, std::io::Error>(answer)
        };
        let ((), answer) = tokio::join!(write, read);
        let answer = answer?;
        let text = String::from_utf8_lossy(&answer);
        text.split(' ')
            .nth(1)
            .and_then(|code| code.parse().ok())
            .ok_or_else(|| KitError::Unexpected("an answer without a status".into()))
    };
    tokio::time::timeout(Duration::from_secs(15), work)
        .await
        .map_err(|_| KitError::TimedOut)?
}

fn respond(status: StatusCode, body: Value) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(body.to_string())));
    *response.status_mut() = status;
    response.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
}

fn ok(body: Value) -> Response<Full<Bytes>> {
    respond(StatusCode::OK, body)
}

fn not_found() -> Response<Full<Bytes>> {
    respond(StatusCode::NOT_FOUND, json!({"message": "not found"}))
}

fn decode(segment: &str) -> String {
    segment.replace("%40", "@")
}

fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then(|| decode(value))
    })
}

fn session_view(name: &str, session: &FakeSession) -> Value {
    let me = session
        .me
        .as_ref()
        .map_or(Value::Null, |me| json!({"id": me, "pushName": "~"}));
    json!({
        "name": name,
        "status": session.status,
        "config": session.config,
        "me": me,
        "engine": {"engine": "GOWS"},
    })
}

async fn handle(
    state: &Mutex<State>,
    addr: SocketAddr,
    request: Request<Incoming>,
) -> Response<Full<Bytes>> {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().unwrap_or_default().to_owned();
    let api_key = request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = match request.into_body().collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(_) => Vec::new(),
    };
    let fault = {
        let mut guard = lock(state);
        guard.received.push(Received {
            method: method.to_string(),
            path: path.clone(),
            query: query.clone(),
            api_key: api_key.clone(),
            body: body.clone(),
        });
        guard
            .faults
            .iter()
            .find(|(needle, _)| path.contains(needle.as_str()))
            .map(|(_, fault)| fault.clone())
    };
    match fault {
        Some(Fault::Slow(wait)) => tokio::time::sleep(wait).await,
        Some(Fault::ServerError) => {
            return respond(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"message": "boom"}),
            );
        }
        Some(Fault::Malformed) => {
            let mut response = Response::new(Full::new(Bytes::from_static(b"{\"status\": \"WORK")));
            response.headers_mut().insert(
                hyper::header::CONTENT_TYPE,
                hyper::header::HeaderValue::from_static("application/json"),
            );
            return response;
        }
        Some(Fault::Huge(size)) => {
            let mut response = Response::new(Full::new(Bytes::from(vec![b' '; size])));
            response.headers_mut().insert(
                hyper::header::CONTENT_TYPE,
                hyper::header::HeaderValue::from_static("application/json"),
            );
            return response;
        }
        Some(Fault::WrongType) => {
            let mut response = Response::new(Full::new(Bytes::from_static(b"<html>hi</html>")));
            response.headers_mut().insert(
                hyper::header::CONTENT_TYPE,
                hyper::header::HeaderValue::from_static("text/html"),
            );
            return response;
        }
        None => {}
    }
    if path == "/ping" {
        return ok(json!({"message": "pong"}));
    }
    {
        let guard = lock(state);
        if api_key.as_deref() != Some(guard.api_key.as_str()) {
            return respond(StatusCode::UNAUTHORIZED, json!({"message": "unauthorized"}));
        }
    }
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let segments: Vec<String> = path
        .trim_start_matches('/')
        .split('/')
        .map(decode)
        .collect();
    let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
    let mut guard = lock(state);
    route(&mut guard, addr, &method, &segments, &query, body)
}

#[allow(clippy::too_many_lines)]
fn route(
    state: &mut State,
    addr: SocketAddr,
    method: &Method,
    segments: &[&str],
    query: &str,
    body: Value,
) -> Response<Full<Bytes>> {
    let get = method == Method::GET;
    let post = method == Method::POST;
    let put = method == Method::PUT;
    let delete = method == Method::DELETE;
    match segments {
        ["health"] if get => ok(json!({"status": "ok", "info": {}, "error": {}, "details": {}})),
        ["api", "server", "version"] if get => ok(json!({
            "version": state.version, "engine": state.mechanism, "tier": "CORE",
        })),
        ["api", "sessions"] if post => {
            let Some(name) = body.get("name").and_then(Value::as_str) else {
                return respond(StatusCode::UNPROCESSABLE_ENTITY, json!({}));
            };
            if state.sessions.contains_key(name) {
                return respond(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    json!({"message": "exists"}),
                );
            }
            let start = body.get("start").and_then(Value::as_bool).unwrap_or(true);
            let session = FakeSession {
                status: if start { "SCAN_QR_CODE" } else { "STOPPED" }.to_owned(),
                config: body.get("config").cloned().unwrap_or(Value::Null),
                me: None,
                pairs_as: None,
                pair_pending: false,
                qr_served: 0,
                configured: 1,
                chats: BTreeMap::new(),
                groups: BTreeMap::new(),
            };
            let view = session_view(name, &session);
            state.sessions.insert(name.to_owned(), session);
            respond(StatusCode::CREATED, view)
        }
        ["api", "sessions", name] => {
            let name = (*name).to_owned();
            if get {
                let Some(session) = state.sessions.get_mut(&name) else {
                    return not_found();
                };
                if session.pair_pending {
                    session.pair_pending = false;
                    session.me = session
                        .pairs_as
                        .as_ref()
                        .map(|phone| format!("{phone}@c.us"));
                    session.status = "WORKING".to_owned();
                }
                ok(session_view(&name, session))
            } else if put {
                let Some(session) = state.sessions.get_mut(&name) else {
                    return not_found();
                };
                session.config = body.get("config").cloned().unwrap_or(Value::Null);
                session.configured = session.configured.saturating_add(1);
                if session.status != "STOPPED" {
                    session.status = if session.me.is_some() {
                        "WORKING"
                    } else {
                        "SCAN_QR_CODE"
                    }
                    .to_owned();
                }
                ok(session_view(&name, session))
            } else if delete {
                match state.sessions.remove(&name) {
                    Some(_) => ok(json!({})),
                    None => not_found(),
                }
            } else {
                not_found()
            }
        }
        ["api", "sessions", name, action] if post => {
            let Some(session) = state.sessions.get_mut(*name) else {
                return not_found();
            };
            match *action {
                "start" | "restart" => {
                    session.status = if session.me.is_some() {
                        "WORKING"
                    } else {
                        "SCAN_QR_CODE"
                    }
                    .to_owned();
                }
                "stop" => session.status = "STOPPED".to_owned(),
                "logout" => {
                    session.me = None;
                    session.chats.clear();
                    session.status = "STOPPED".to_owned();
                }
                _ => return not_found(),
            }
            ok(session_view(name, session))
        }
        ["api", session, "auth", "qr"] if get => {
            let Some(found) = state.sessions.get_mut(*session) else {
                return not_found();
            };
            if found.status != "SCAN_QR_CODE" || !query.contains("format=raw") {
                return respond(StatusCode::UNPROCESSABLE_ENTITY, json!({}));
            }
            found.qr_served = found.qr_served.saturating_add(1);
            if found.pairs_as.is_some() {
                found.pair_pending = true;
            }
            ok(json!({"value": format!("2@fake-qr-value-{},{}", found.qr_served, session)}))
        }
        ["api", session, "auth", "request-code"] if post => {
            let Some(found) = state.sessions.get_mut(*session) else {
                return not_found();
            };
            if found.status != "SCAN_QR_CODE" || body.get("phoneNumber").is_none() {
                return respond(StatusCode::UNPROCESSABLE_ENTITY, json!({}));
            }
            if found.pairs_as.is_some() {
                found.pair_pending = true;
            }
            ok(json!({"code": "ABCD-EFGH"}))
        }
        ["api", session, "chats", "overview"] if get => {
            let Some(found) = state.sessions.get(*session) else {
                return not_found();
            };
            let chats: Vec<Value> = found
                .chats
                .iter()
                .map(|(id, chat)| {
                    let last = chat
                        .messages
                        .iter()
                        .max_by_key(|m| m.get("timestamp").and_then(Value::as_i64).unwrap_or(0))
                        .cloned()
                        .unwrap_or(Value::Null);
                    json!({"id": id, "name": chat.name, "picture": null, "lastMessage": last})
                })
                .collect();
            ok(Value::Array(chats))
        }
        ["api", session, "chats", chat, "messages"] if get => {
            let Some(found) = state.sessions.get(*session) else {
                return not_found();
            };
            let Some(chat) = found.chats.get(*chat) else {
                return ok(json!([]));
            };
            let gte =
                query_value(query, "filter.timestamp.gte").and_then(|v| v.parse::<i64>().ok());
            let lte =
                query_value(query, "filter.timestamp.lte").and_then(|v| v.parse::<i64>().ok());
            let limit = query_value(query, "limit")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(100);
            let mut messages: Vec<Value> = chat
                .messages
                .iter()
                .filter(|m| {
                    let at = m.get("timestamp").and_then(Value::as_i64).unwrap_or(0);
                    gte.is_none_or(|gte| at >= gte) && lte.is_none_or(|lte| at <= lte)
                })
                .cloned()
                .collect();
            messages.sort_by_key(|m| -m.get("timestamp").and_then(Value::as_i64).unwrap_or(0));
            messages.truncate(limit);
            ok(Value::Array(messages))
        }
        ["api", session, "chats", chat, "messages", message] => {
            let key = ((*chat).to_owned(), (*message).to_owned());
            if put {
                state.edited.push((key.0, key.1, body));
                return ok(json!({}));
            }
            if delete {
                state.deleted.push(key);
                return ok(json!({}));
            }
            if !get {
                return not_found();
            }
            let Some(found) = state.sessions.get(*session) else {
                return not_found();
            };
            let found = found.chats.get(*chat).and_then(|chat| {
                chat.messages
                    .iter()
                    .find(|m| m.get("id").and_then(Value::as_str) == Some(*message))
            });
            match found {
                Some(message) => {
                    let mut message = message.clone();
                    if let Some(file) = message
                        .get("media")
                        .and_then(|media| media.get("file"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                    {
                        message["media"]["url"] =
                            Value::String(format!("http://{addr}/api/files/{file}"));
                    }
                    ok(message)
                }
                None => not_found(),
            }
        }
        ["api", "files", rest @ ..] if get => {
            let name = rest.join("/");
            match state.files.get(&name) {
                Some((mimetype, data)) => {
                    let mut response = Response::new(Full::new(Bytes::from(data.clone())));
                    if let Ok(value) = hyper::header::HeaderValue::from_str(mimetype) {
                        response
                            .headers_mut()
                            .insert(hyper::header::CONTENT_TYPE, value);
                    }
                    response
                }
                None => not_found(),
            }
        }
        ["api", "contacts"] if get => {
            let id = query_value(query, "contactId").unwrap_or_default();
            match state.contacts.get(&id) {
                Some((name, pushname, _)) => ok(json!({
                    "id": id, "number": id.split('@').next(), "name": name,
                    "pushname": pushname, "isMe": false, "isGroup": false,
                })),
                None => not_found(),
            }
        }
        ["api", "contacts", "about"] if get => {
            let id = query_value(query, "contactId").unwrap_or_default();
            match state.contacts.get(&id) {
                Some((_, _, about)) => ok(json!({"about": about})),
                None => not_found(),
            }
        }
        ["api", "contacts", "profile-picture"] if get => {
            ok(json!({"profilePictureURL": "https://pps.example.com/picture.jpg"}))
        }
        ["api", "contacts", "check-exists"] if get => {
            let phone = query_value(query, "phone").unwrap_or_default();
            match state.registered.get(&phone) {
                Some(chat) => ok(json!({"numberExists": true, "chatId": chat})),
                None => ok(json!({"numberExists": false, "chatId": null})),
            }
        }
        ["api", session, "groups"] if get => {
            let Some(found) = state.sessions.get(*session) else {
                return not_found();
            };
            let groups: Vec<Value> = found
                .groups
                .iter()
                .map(|(id, group)| {
                    let participants: Vec<Value> = group
                        .participants
                        .iter()
                        .map(|(id, role)| json!({"id": id, "role": role}))
                        .collect();
                    // The count of the engine reads zero, as the real one does.
                    json!({"id": id, "subject": group.subject, "participantsCount": 0,
                           "participants": participants})
                })
                .collect();
            ok(Value::Array(groups))
        }
        ["api", session, "groups", group] if get => {
            let found = state
                .sessions
                .get(*session)
                .and_then(|found| found.groups.get(*group));
            match found {
                Some(found) => ok(json!({"id": group, "subject": found.subject,
                                         "description": "", "participantsCount": 0})),
                None => not_found(),
            }
        }
        ["api", session, "groups", group, "participants", "v2"] if get => {
            let found = state
                .sessions
                .get(*session)
                .and_then(|found| found.groups.get(*group));
            match found {
                Some(found) => ok(Value::Array(
                    found
                        .participants
                        .iter()
                        .map(|(id, role)| json!({"id": id, "role": role}))
                        .collect(),
                )),
                None => not_found(),
            }
        }
        [
            "api",
            action @ ("sendText" | "sendImage" | "sendFile" | "sendVoice" | "forwardMessage"),
        ] if post => {
            let session = body
                .get("session")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let chat = body
                .get("chatId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let working = state
                .sessions
                .get(session)
                .is_some_and(|found| found.status == "WORKING");
            if !working {
                return respond(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    json!({"message": "not working"}),
                );
            }
            state.counter = state.counter.saturating_add(1);
            let id = format!("true_{chat}_3EB0{:016X}", state.counter);
            state.sent.push(((*action).to_owned(), body));
            ok(json!({"id": id, "fromMe": true}))
        }
        ["api", action @ ("sendSeen" | "startTyping" | "stopTyping")] if post => {
            state.sent.push(((*action).to_owned(), body));
            ok(json!({}))
        }
        ["api", "reaction"] if put => {
            state.sent.push(("reaction".to_owned(), body));
            ok(json!({}))
        }
        _ => not_found(),
    }
}
