//! One connection after the handshake: two tasks, two queues, the phases and the limits.
//!
//! - **One task reads, another writes** (`docs/15-mvp.md`, section 3). Only the one that
//!   writes encrypts, at the moment it writes, after choosing between the two queues: sealing
//!   when a frame is queued would break the counter the first time a control frame went
//!   ahead (`docs/14-protocolo-do-canal.md`, section 3).
//! - **Two queues**: control (16 frames) and data (64 frames). The writer looks at the control
//!   queue before every frame. Whoever produces data waits for room; a full control queue
//!   closes the connection, and so does a frame that takes 30 seconds to leave. A slow network
//!   never expels anyone before that.
//! - **The limits of each phase** (section 4): before the session only stream 0 and 4,096
//!   bytes, 90 seconds to become a session apart from the wait for approval; while waiting,
//!   only `check_ok`, `ping`, `pong` and `leave`; in session, the sizes of each direction and
//!   8 streams. The frame rules themselves are the reassembler of the protocol, which takes
//!   its bytes from the budget of the whole hub.
//! - **Heartbeat**: `ping` every 30 seconds from `hello_ok` on; any valid frame received counts
//!   as life, and 90 seconds without one close the connection.
//! - **The limits of the session start with `session` itself**: the writer moves the
//!   connection to the session at the moment it takes the frame of `session` from the queue,
//!   before writing it. Whatever the reader got before that moment the connector sent before it
//!   could read `session`, and the code of the session never sees it: the connection closes
//!   with AR-204 (rule 4 of C7: a `call` before `session` is an error). Whatever the reader
//!   gets after that is read with the limits of the session.
//! - **The test of an established connection** (step 8 of the swap) is answered by the reader:
//!   a `check_ok` with the nonce of the [`LinkHandle::probe`] under way never waits for the
//!   code of the purpose, which may be busy with the database.
//!
//! What the messages mean is not here: [`Link`] hands every message to the code of the
//! purpose (`ping`, `pong` and the answer to a probe excepted), and takes what it sends.

use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::frame::{FrameHeader, Reassembler, Side, SplitError, Splitter};
use arauto_protocol::limits::FRAME_CHUNK_MAX;
use arauto_protocol::message::{
    self, Check, CheckNonce, Closing, EncodeError, Message, MessageType, Ping, Pong,
};
use arauto_protocol::noise::{HandshakeHash, Opener, PublicKey, Sealer};
use axum::extract::ws::{self, WebSocket};
use bytes::Bytes;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{Semaphore, mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep_until, timeout};

use super::admission::{Admission, Permit};
use super::budget::HubBudget;
use super::settings::{ChannelSettings, duration};
use arauto_protocol::limits::MAX_OPEN_STREAMS;

/// Messages the reader hands over before it waits for the code of the purpose to take them.
const INBOX_MESSAGES: usize = 8;

/// What a connection is allowed to do right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// From the handshake until `session`: only stream 0, messages of up to 4,096 bytes, and
    /// 90 seconds in all to become a session.
    PreSession,
    /// A pending device waits for the admin: only `check_ok`, `ping`, `pong` and `leave`. It
    /// does not count toward the ceiling of connections without a session, and the 90 seconds
    /// stop while it waits.
    Waiting,
    /// The session of the channel: the limits of each direction and 8 streams.
    Session,
}

/// What is known of a connection once its handshake ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkInfo {
    /// The order number, given at the end of the handshake. Later connections have larger
    /// numbers: the newest connection of a device is the one with the largest.
    pub serial: u64,
    /// The static key of the connector: the identity of the device.
    pub identity: PublicKey,
    /// The Noise value `h` at the end of the handshake. It goes into the verification code.
    pub handshake_hash: HandshakeHash,
    /// The address of the connector, read as `ARAUTO_CLIENT_IP_SOURCE` says.
    pub address: IpAddr,
    /// When the websocket opened, from the clock of the hub.
    pub opened_at_ms: i64,
}

/// Why a connection is over, or is ending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// Running.
    Open,
    /// Send what is queued, then `closing` with this code, then close.
    Closing(ErrorCode),
    /// The code of the purpose is done: send what is queued, then close.
    Done,
    /// Close now, without writing anything else.
    Silent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct State {
    phase: Phase,
    heartbeat: bool,
}

/// A frame waiting in a queue, not yet sealed.
#[derive(Debug)]
struct OutFrame {
    header: FrameHeader,
    chunk: Bytes,
    /// The last frame of `session`: taken from the queue, the connection is in session.
    opens_session: bool,
}

/// A message the reader handed over, with the phase the connection was in when it arrived.
#[derive(Debug)]
struct Inbound {
    message: Message,
    /// Read with the limits of the session: the connector could already have read `session`.
    in_session: bool,
}

/// The `check` a newer connection of the same device sent to this one (step 8 of the swap).
#[derive(Debug)]
struct Probe {
    nonce: CheckNonce,
    answered: oneshot::Sender<()>,
}

/// The connection is over: nothing more can be sent or received on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the connection is closed")]
pub struct LinkClosed;

/// Why [`Link::recv_within`] returned without a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RecvError {
    /// The connector did not send anything in time. The connection is still open: the caller
    /// decides whether to close it, and with which code.
    #[error("the connector did not answer in time")]
    TimedOut,
    /// The connection is over.
    #[error("the connection is closed")]
    Closed,
}

/// Why a message was not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// The message does not fit the limit of this phase and direction. Nothing was sent; the
    /// connection goes on. A call that produced it is answered with AR-709.
    #[error("the message does not fit the limit of this phase")]
    TooLarge,
    /// The message could not be written: too many values, too deep, or not writable. Nothing
    /// was sent; the connection goes on. For a result, AR-709.
    #[error("the message could not be written: {0}")]
    Encode(EncodeError),
    /// Not a message the hub sends, or one that goes another way (`session` goes through
    /// [`Link::enter_session`], `check` and the heartbeat through [`LinkHandle::send_control`]).
    #[error("this message is not sent this way")]
    Misuse,
    /// The connection is over.
    #[error("the connection is closed")]
    Closed,
}

/// What the two tasks and every handle share.
struct Shared {
    info: LinkInfo,
    state: watch::Sender<State>,
    ending: watch::Sender<Ending>,
    finished: watch::Sender<bool>,
    control: mpsc::Sender<OutFrame>,
    data: mpsc::Sender<OutFrame>,
    /// Data frames put in the queue so far.
    data_queued: AtomicU64,
    /// Data frames the writer took out of the queue so far: a control frame queued after
    /// that can no longer pass ahead of them.
    data_taken: watch::Sender<u64>,
    /// Held from the cut of a message until its first frame is queued: the streams reach the
    /// connector in the order of their numbers.
    data_splitter: tokio::sync::Mutex<Splitter>,
    /// Control messages and `closing`: always one frame on stream 0, never a stream number.
    control_splitter: Mutex<Splitter>,
    /// A message of more than one frame opens a stream; the connector allows 8 at a time.
    streams: Arc<Semaphore>,
    admission: Arc<Admission>,
    permit: Mutex<Option<Permit>>,
    /// The probe under way, answered by the reader.
    probe: Mutex<Option<Probe>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Nothing here is left half done by a panic: a splitter or an optional permit.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    /// Asks the connection to end. The first request wins, except that closing at once
    /// overrides a graceful ending that is under way.
    fn end(&self, ending: Ending) {
        self.ending.send_if_modified(|current| {
            let replace = match (*current, ending) {
                (Ending::Open, _) => true,
                (Ending::Silent, _) => false,
                (_, Ending::Silent) => true,
                _ => false,
            };
            if replace {
                *current = ending;
            }
            replace
        });
    }

    fn is_open(&self) -> bool {
        *self.ending.borrow() == Ending::Open
    }

    /// Resolves when the connection starts ending.
    async fn ending(&self) {
        let mut ending = self.ending.subscribe();
        let _ = ending.wait_for(|ending| *ending != Ending::Open).await;
    }

    fn release_permit(&self) {
        lock(&self.permit).take();
    }

    /// A `check_ok` received: whether it answered the probe under way. A late answer to a
    /// probe that gave up, or one with another nonce, answers nothing.
    fn answer_probe(&self, nonce: &CheckNonce) -> bool {
        let mut probe = lock(&self.probe);
        if probe.as_ref().is_some_and(|probe| probe.nonce == *nonce)
            && let Some(probe) = probe.take()
        {
            let _ = probe.answered.send(());
            return true;
        }
        false
    }
}

/// What every task of the hub can do with a connection: send, close, wait for its end.
#[derive(Clone)]
pub struct LinkHandle {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for LinkHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkHandle")
            .field("serial", &self.shared.info.serial)
            .field("phase", &self.phase())
            .finish_non_exhaustive()
    }
}

impl LinkHandle {
    /// What is known of the connection.
    #[must_use]
    pub fn info(&self) -> &LinkInfo {
        &self.shared.info
    }

    /// The current phase.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.shared.state.borrow().phase
    }

    /// Whether the connection is still running (no ending was asked yet).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.shared.is_open()
    }

    /// Sends a data message. Waits for room in the data queue: that is the backpressure, and
    /// a slow connector slows down whoever produces for it, never the others.
    ///
    /// A message larger than one frame takes a stream number, and waits while 8 streams of
    /// the hub are open on this connection.
    ///
    /// # Errors
    /// [`SendError::TooLarge`] and [`SendError::Encode`] leave the connection open;
    /// [`SendError::Misuse`] for a message that does not go this way; [`SendError::Closed`].
    pub async fn send(&self, message: &Message) -> Result<(), SendError> {
        let kind = message.kind().ok_or(SendError::Misuse)?;
        if !kind.sent_by(Side::Hub) || kind.is_control() || kind == MessageType::Session {
            return Err(SendError::Misuse);
        }
        let bytes = Bytes::from(message::encode(message).map_err(SendError::Encode)?);
        self.send_bytes(bytes).await
    }

    async fn send_bytes(&self, bytes: Bytes) -> Result<(), SendError> {
        let shared = &self.shared;
        if !shared.is_open() {
            return Err(SendError::Closed);
        }
        // Only a message that opens a stream waits for one of the 8.
        let _stream = if bytes.len() > FRAME_CHUNK_MAX {
            tokio::select! {
                biased;
                () = shared.ending() => return Err(SendError::Closed),
                permit = Arc::clone(&shared.streams).acquire_owned() => {
                    Some(permit.map_err(|_| SendError::Closed)?)
                }
            }
        } else {
            None
        };
        let mut splitter = tokio::select! {
            biased;
            () = shared.ending() => return Err(SendError::Closed),
            splitter = shared.data_splitter.lock() => splitter,
        };
        let frames = cut(&mut splitter, &bytes, false).inspect_err(|error| {
            if *error == SendError::Closed {
                // Every stream number is used: the hub closes and the connector reconnects.
                shared.end(Ending::Closing(ErrorCode::Ar205));
            }
        })?;
        let mut frames = frames.into_iter();
        if let Some(first) = frames.next() {
            self.enqueue_data(first).await?;
        }
        drop(splitter);
        for frame in frames {
            self.enqueue_data(frame).await?;
        }
        Ok(())
    }

    async fn enqueue_data(&self, frame: OutFrame) -> Result<(), SendError> {
        let shared = &self.shared;
        tokio::select! {
            biased;
            () = shared.ending() => Err(SendError::Closed),
            sent = shared.data.send(frame) => {
                sent.map_err(|_| SendError::Closed)?;
                shared.data_queued.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
    }

    /// Waits until the writer took every data frame queued so far. A control message sent
    /// after this goes out after them: `check`, which passes ahead of the data queue, cannot
    /// reach the connector before the `state` that came first
    /// (`docs/14-protocolo-do-canal.md`, section 5.3).
    ///
    /// # Errors
    /// [`SendError::Closed`] when the connection ends first.
    pub async fn data_taken(&self) -> Result<(), SendError> {
        let shared = &self.shared;
        let target = shared.data_queued.load(Ordering::SeqCst);
        let mut taken = shared.data_taken.subscribe();
        tokio::select! {
            biased;
            () = shared.ending() => Err(SendError::Closed),
            found = taken.wait_for(|taken| *taken >= target) => {
                found.map(|_| ()).map_err(|_| SendError::Closed)
            }
        }
    }

    /// Sends a control message (`check`, `ping`, `pong`) ahead of the data queue. It never
    /// waits: a full control queue means the connector stopped reading, and the connection
    /// is closed.
    ///
    /// # Errors
    /// [`SendError::Misuse`] for a message that is not of control; [`SendError::Closed`],
    /// also when this message found the queue full.
    pub fn send_control(&self, message: &Message) -> Result<(), SendError> {
        let kind = message.kind().ok_or(SendError::Misuse)?;
        if !kind.sent_by(Side::Hub) || !kind.is_control() {
            return Err(SendError::Misuse);
        }
        let shared = &self.shared;
        if !shared.is_open() {
            return Err(SendError::Closed);
        }
        let bytes = Bytes::from(message::encode(message).map_err(SendError::Encode)?);
        let frames = cut(&mut lock(&shared.control_splitter), &bytes, true)?;
        for frame in frames {
            if shared.control.try_send(frame).is_err() {
                shared.end(Ending::Silent);
                return Err(SendError::Closed);
            }
        }
        Ok(())
    }

    /// Sends `check` with `nonce` ahead of the data queue and returns what resolves when the
    /// connector answers it with the same nonce (step 8 of the swap). The reader answers it,
    /// whatever the code of the purpose is doing. `None` when the connection is closed or its
    /// control queue is full; a newer probe replaces an older one.
    #[must_use]
    pub fn probe(&self, nonce: CheckNonce) -> Option<oneshot::Receiver<()>> {
        let (answered, waiting) = oneshot::channel();
        *lock(&self.shared.probe) = Some(Probe { nonce, answered });
        self.send_control(&Message::Check(Check { nonce }))
            .ok()
            .map(|()| waiting)
    }

    /// Closes with `closing` and `code`: what is already queued goes first, nothing queued
    /// after this call goes at all.
    pub fn close(&self, code: ErrorCode) {
        self.shared.end(Ending::Closing(code));
    }

    /// Closes without writing anything else.
    pub fn close_silently(&self) {
        self.shared.end(Ending::Silent);
    }

    /// Resolves when the connection is over and both of its tasks ended.
    pub async fn closed(&self) {
        let mut finished = self.shared.finished.subscribe();
        let _ = finished.wait_for(|finished| *finished).await;
    }
}

/// Cuts a message in frames that borrow nothing.
fn cut(splitter: &mut Splitter, bytes: &Bytes, control: bool) -> Result<Vec<OutFrame>, SendError> {
    match splitter.split(bytes, control) {
        Ok(frames) => Ok(frames
            .map(|frame| OutFrame {
                header: frame.header,
                chunk: bytes.slice_ref(frame.chunk),
                opens_session: false,
            })
            .collect()),
        Err(SplitError::StreamsExhausted) => Err(SendError::Closed),
        Err(SplitError::Empty | SplitError::TooLarge | SplitError::ControlTooLarge) => {
            Err(SendError::TooLarge)
        }
    }
}

/// The connection as the code of a purpose drives it: it receives every message the
/// connector sends (`ping` and `pong` are answered here), and it moves the phases.
///
/// Dropped, or once the code of the purpose returns, what is queued is sent and the
/// connection closes.
pub struct Link {
    inbox: mpsc::Receiver<Inbound>,
    handle: LinkHandle,
    /// `session` was handed to the writer.
    entered_session: bool,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("handle", &self.handle)
            .finish()
    }
}

impl Link {
    /// A handle for other tasks: send, close, wait for the end.
    #[must_use]
    pub fn handle(&self) -> LinkHandle {
        self.handle.clone()
    }

    /// What is known of the connection.
    #[must_use]
    pub fn info(&self) -> &LinkInfo {
        self.handle.info()
    }

    /// The current phase.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.handle.phase()
    }

    /// The next message, however long it takes. For the wait for approval; every other step
    /// before the session has a deadline ([`Link::recv_within`]).
    ///
    /// Once in session, a message the reader got before `session` left closes the connection
    /// with AR-204 instead: the connector sent it before it could read `session` (C7, rule 4).
    ///
    /// # Errors
    /// [`LinkClosed`] once the connection is ending.
    pub async fn recv(&mut self) -> Result<Message, LinkClosed> {
        let shared = Arc::clone(&self.handle.shared);
        let inbound = tokio::select! {
            biased;
            () = shared.ending() => return Err(LinkClosed),
            inbound = self.inbox.recv() => inbound.ok_or(LinkClosed)?,
        };
        if self.entered_session && !inbound.in_session {
            self.handle.close(ErrorCode::Ar204);
            return Err(LinkClosed);
        }
        Ok(inbound.message)
    }

    /// The next message, if it comes within `ms` milliseconds.
    ///
    /// # Errors
    /// [`RecvError::TimedOut`] leaves the connection open; [`RecvError::Closed`].
    pub async fn recv_within(&mut self, ms: i64) -> Result<Message, RecvError> {
        match timeout(duration(ms), self.recv()).await {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(LinkClosed)) => Err(RecvError::Closed),
            Err(_) => Err(RecvError::TimedOut),
        }
    }

    /// Sends a data message. See [`LinkHandle::send`].
    ///
    /// # Errors
    /// As [`LinkHandle::send`].
    pub async fn send(&self, message: &Message) -> Result<(), SendError> {
        self.handle.send(message).await
    }

    /// Sends a control message. See [`LinkHandle::send_control`].
    ///
    /// # Errors
    /// As [`LinkHandle::send_control`].
    pub fn send_control(&self, message: &Message) -> Result<(), SendError> {
        self.handle.send_control(message)
    }

    /// Sends `closing` with `code` and closes.
    pub fn close(&self, code: ErrorCode) {
        self.handle.close(code);
    }

    /// Closes without writing anything else.
    pub fn close_silently(&self) {
        self.handle.close_silently();
    }

    /// Sends `session` and switches both directions to the limits of the session.
    ///
    /// The message is cut with the limits of before the session, which the connector still
    /// reads it with; the hub switches its own cut right after, before any other data message
    /// is cut. The reader switches when the writer takes `session` from the queue, before
    /// writing it: whatever the connector sends once it read `session` is read with the
    /// limits of the session, and whatever came before is refused ([`Link::recv`]). Returns
    /// once the writer took it. From here on the connection no longer counts toward the
    /// ceiling of connections without a session.
    ///
    /// # Errors
    /// [`SendError::Misuse`] when `session` is not a `session` message or the connection is
    /// already in session; [`SendError::TooLarge`]; [`SendError::Closed`].
    pub async fn enter_session(&mut self, session: &Message) -> Result<(), SendError> {
        if session.kind() != Some(MessageType::Session) || self.entered_session {
            return Err(SendError::Misuse);
        }
        let shared = Arc::clone(&self.handle.shared);
        let bytes = Bytes::from(message::encode(session).map_err(SendError::Encode)?);
        let mut splitter = tokio::select! {
            biased;
            () = shared.ending() => return Err(SendError::Closed),
            splitter = shared.data_splitter.lock() => splitter,
        };
        let mut frames = cut(&mut splitter, &bytes, false)?;
        if let Some(last) = frames.last_mut() {
            last.opens_session = true;
        }
        for frame in frames {
            self.handle.enqueue_data(frame).await?;
        }
        splitter.enter_session();
        lock(&shared.control_splitter).enter_session();
        drop(splitter);
        self.entered_session = true;
        shared.release_permit();
        self.handle.data_taken().await
    }

    /// The device is pending: the connection waits for the admin. It leaves the ceiling of
    /// connections without a session, and the 90 seconds stop.
    pub fn enter_waiting(&self) {
        let shared = &self.handle.shared;
        if self.phase() == Phase::PreSession {
            shared
                .state
                .send_modify(|state| state.phase = Phase::Waiting);
            shared.release_permit();
        }
    }

    /// The wait is over and the exchange follows: the connection is counted again (over the
    /// ceiling, if need be) and the 90 seconds go on from where they stopped.
    pub fn leave_waiting(&self) {
        let shared = &self.handle.shared;
        if self.phase() == Phase::Waiting {
            let origin = super::admission::Origin::of(shared.info.address);
            *lock(&shared.permit) = Some(shared.admission.readmit(origin));
            shared
                .state
                .send_modify(|state| state.phase = Phase::PreSession);
        }
    }

    /// Starts the heartbeat. Called once `hello_ok` is queued.
    pub(crate) fn start_heartbeat(&self) {
        self.handle
            .shared
            .state
            .send_modify(|state| state.heartbeat = true);
    }
}

/// What is needed to start the two tasks of a connection.
pub(crate) struct Start {
    pub(crate) info: LinkInfo,
    pub(crate) sealer: Sealer,
    pub(crate) opener: Opener,
    pub(crate) permit: Permit,
    pub(crate) admission: Arc<Admission>,
    pub(crate) budget: Arc<HubBudget>,
    pub(crate) settings: ChannelSettings,
    /// When the websocket opened: the 90 seconds before the session count from here.
    pub(crate) opened: Instant,
}

/// The two tasks of a connection, for the one that started them to end them.
pub(crate) struct Tasks {
    shared: Arc<Shared>,
    writer: JoinHandle<()>,
    reader: JoinHandle<()>,
    settings: ChannelSettings,
}

/// Starts the reader and the writer of a websocket whose handshake ended.
pub(crate) fn start(socket: WebSocket, start: Start) -> (Link, Tasks) {
    let settings = start.settings;
    let (control, control_rx) = mpsc::channel(settings.control_queue_frames.max(1));
    let (data, data_rx) = mpsc::channel(settings.data_queue_frames.max(1));
    let (inbox_tx, inbox) = mpsc::channel(INBOX_MESSAGES);
    let shared = Arc::new(Shared {
        info: start.info,
        state: watch::Sender::new(State {
            phase: Phase::PreSession,
            heartbeat: false,
        }),
        ending: watch::Sender::new(Ending::Open),
        finished: watch::Sender::new(false),
        data_queued: AtomicU64::new(0),
        data_taken: watch::Sender::new(0),
        control,
        data,
        data_splitter: tokio::sync::Mutex::new(Splitter::new(Side::Hub)),
        control_splitter: Mutex::new(Splitter::new(Side::Hub)),
        streams: Arc::new(Semaphore::new(MAX_OPEN_STREAMS)),
        admission: start.admission,
        permit: Mutex::new(Some(start.permit)),
        probe: Mutex::new(None),
    });
    let (sink, stream) = socket.split();
    let writer = Writer {
        sink,
        sealer: start.sealer,
        control: control_rx,
        data: data_rx,
        shared: Arc::clone(&shared),
        stall: duration(settings.write_stall_ms),
    };
    let now = Instant::now();
    let reader = Reader {
        stream,
        opener: start.opener,
        reassembler: Reassembler::new(
            Side::Connector,
            settings.deadlines.stream_idle_ms,
            start.budget,
        ),
        shared: Arc::clone(&shared),
        inbox: inbox_tx,
        base: now,
        state: shared.state.subscribe(),
        ending: shared.ending.subscribe(),
        last_frame: now,
        pre_session_from: Some(start.opened),
        pre_session_spent: Duration::ZERO,
        phase: Phase::PreSession,
        in_session: false,
        heartbeat: false,
        next_ping: None,
        ping_number: 0,
        unknown: 0,
        settings: settings.clone(),
    };
    let tasks = Tasks {
        shared: Arc::clone(&shared),
        writer: tokio::spawn(writer.run()),
        reader: tokio::spawn(reader.run()),
        settings,
    };
    let link = Link {
        inbox,
        handle: LinkHandle { shared },
        entered_session: false,
    };
    (link, tasks)
}

impl Tasks {
    /// Ends the connection once the code of its purpose returned: what is queued is sent, a
    /// connection that does not take it within the write deadline is dropped.
    pub(crate) async fn finish(mut self) {
        self.shared.end(Ending::Done);
        let stall = duration(self.settings.write_stall_ms);
        let linger = duration(self.settings.linger_ms);
        // The queued frames, then `closing` and the close: each write has the deadline.
        if timeout(
            stall.saturating_mul(3).saturating_add(linger),
            &mut self.writer,
        )
        .await
        .is_err()
        {
            self.shared.end(Ending::Silent);
            self.writer.abort();
        }
        if timeout(linger.saturating_add(linger), &mut self.reader)
            .await
            .is_err()
        {
            self.shared.end(Ending::Silent);
            self.reader.abort();
        }
        self.shared.release_permit();
        self.shared.finished.send_replace(true);
    }
}

// ---------------------------------------------------------------------------------------------
// The writer
// ---------------------------------------------------------------------------------------------

struct Writer {
    sink: SplitSink<WebSocket, ws::Message>,
    sealer: Sealer,
    control: mpsc::Receiver<OutFrame>,
    data: mpsc::Receiver<OutFrame>,
    shared: Arc<Shared>,
    stall: Duration,
}

impl Writer {
    async fn run(mut self) {
        let mut ending = self.shared.ending.subscribe();
        loop {
            let now = *ending.borrow_and_update();
            match now {
                Ending::Open => {}
                Ending::Silent => return,
                Ending::Closing(code) => return self.finish(Some(code)).await,
                Ending::Done => return self.finish(None).await,
            }
            let frame = tokio::select! {
                biased;
                changed = ending.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    continue;
                }
                Some(frame) = self.control.recv() => frame,
                Some(frame) = self.data.recv() => {
                    if frame.opens_session {
                        // Before `session` leaves: whatever the reader gets from now on may
                        // come from a connector that read it.
                        self.shared
                            .state
                            .send_modify(|state| state.phase = Phase::Session);
                    }
                    self.shared.data_taken.send_modify(|taken| *taken = taken.saturating_add(1));
                    frame
                }
            };
            if self.write(&frame).await.is_err() {
                self.shared.end(Ending::Silent);
                return;
            }
        }
    }

    /// Seals and writes one frame. A frame that does not leave within the write deadline, or
    /// a connection that is closed meanwhile, ends the writing.
    async fn write(&mut self, frame: &OutFrame) -> Result<(), ()> {
        let mut plaintext = Vec::with_capacity(frame.chunk.len().saturating_add(5));
        plaintext.extend_from_slice(&frame.header.encode());
        plaintext.extend_from_slice(&frame.chunk);
        let sealed = self.sealer.seal(&plaintext).map_err(|_| ())?;
        self.write_raw(ws::Message::Binary(Bytes::from(sealed)))
            .await
    }

    async fn write_raw(&mut self, message: ws::Message) -> Result<(), ()> {
        let mut ending = self.shared.ending.subscribe();
        tokio::select! {
            biased;
            _ = ending.wait_for(|ending| *ending == Ending::Silent) => Err(()),
            sent = timeout(self.stall, self.sink.send(message)) => match sent {
                Ok(Ok(())) => Ok(()),
                _ => Err(()),
            },
        }
    }

    /// Ends the writing: what is already queued goes first, in order, then `closing` when
    /// there is a code, then the close of the websocket. The queues take nothing new, and the
    /// whole of it has the write deadline: a connector that does not read gets no more time.
    async fn finish(mut self, code: Option<ErrorCode>) {
        self.control.close();
        self.data.close();
        let until = Instant::now() + self.stall;
        while Instant::now() < until {
            let frame = match self.control.try_recv() {
                Ok(frame) => frame,
                Err(_) => match self.data.try_recv() {
                    Ok(frame) => frame,
                    Err(_) => break,
                },
            };
            if self.write(&frame).await.is_err() {
                self.shared.end(Ending::Silent);
                return;
            }
        }
        if let Some(code) = code {
            let Ok(bytes) = message::encode(&Message::Closing(Closing { code })) else {
                return;
            };
            let bytes = Bytes::from(bytes);
            let Ok(frames) = cut(&mut lock(&self.shared.control_splitter), &bytes, false) else {
                return;
            };
            for frame in &frames {
                if self.write(frame).await.is_err() {
                    return;
                }
            }
        }
        let _ = self.write_raw(ws::Message::Close(None)).await;
    }
}

// ---------------------------------------------------------------------------------------------
// The reader
// ---------------------------------------------------------------------------------------------

struct Reader {
    stream: SplitStream<WebSocket>,
    opener: Opener,
    reassembler: Reassembler<Arc<HubBudget>>,
    shared: Arc<Shared>,
    inbox: mpsc::Sender<Inbound>,
    /// The start of the clock of the network deadlines of this connection. A stream that
    /// stays idle is measured on it, never on the clock of the hub: a jump of the time of the
    /// system neither cuts a transfer nor holds the budget longer.
    base: Instant,
    state: watch::Receiver<State>,
    ending: watch::Receiver<Ending>,
    /// The last valid frame received: the heartbeat counts from here.
    last_frame: Instant,
    /// While before the session, when the current stretch began. `None` while waiting and in
    /// session.
    pre_session_from: Option<Instant>,
    /// Time already spent before the session, apart from the stretch under way.
    pre_session_spent: Duration,
    /// The phase as this task last read it: what it receives is judged by it.
    phase: Phase,
    in_session: bool,
    heartbeat: bool,
    next_ping: Option<Instant>,
    ping_number: u64,
    unknown: u32,
    settings: ChannelSettings,
}

/// What the reader does after one step.
enum Step {
    Go,
    End(Ending),
}

impl Reader {
    fn link_handle(&self) -> LinkHandle {
        LinkHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    async fn run(mut self) {
        loop {
            self.sync_state();
            if *self.ending.borrow_and_update() != Ending::Open {
                return self.linger().await;
            }
            let deadline = self.next_deadline();
            let step = tokio::select! {
                biased;
                changed = self.ending.changed() => {
                    if changed.is_err() { Step::End(Ending::Silent) } else { Step::Go }
                }
                changed = self.state.changed() => {
                    if changed.is_err() { Step::End(Ending::Silent) } else { Step::Go }
                }
                item = self.stream.next() => self.receive(item).await,
                () = sleep_until(deadline) => self.on_time(),
            };
            if let Step::End(ending) = step {
                self.shared.end(ending);
            }
        }
    }

    /// After `closing`, keeps reading and discarding what the connector still sends, until
    /// it closes or the linger runs out.
    async fn linger(mut self) {
        if *self.ending.borrow() == Ending::Silent {
            return;
        }
        let until = Instant::now() + duration(self.settings.linger_ms);
        loop {
            tokio::select! {
                biased;
                _ = self.ending.wait_for(|ending| *ending == Ending::Silent) => return,
                () = sleep_until(until) => return,
                item = self.stream.next() => match item {
                    Some(Ok(ws::Message::Close(_)) | Err(_)) | None => return,
                    Some(Ok(_)) => {}
                },
            }
        }
    }

    fn sync_state(&mut self) {
        let state = *self.state.borrow_and_update();
        self.phase = state.phase;
        match state.phase {
            Phase::Session => {
                if !self.in_session {
                    self.reassembler.enter_session();
                    self.in_session = true;
                }
                self.stop_pre_session_clock();
            }
            Phase::Waiting => self.stop_pre_session_clock(),
            Phase::PreSession => {
                if self.pre_session_from.is_none() {
                    self.pre_session_from = Some(Instant::now());
                }
            }
        }
        if state.heartbeat && !self.heartbeat {
            self.heartbeat = true;
            self.next_ping =
                Some(Instant::now() + duration(self.settings.deadlines.ping_interval_ms));
        }
    }

    fn stop_pre_session_clock(&mut self) {
        if let Some(from) = self.pre_session_from.take() {
            self.pre_session_spent = self.pre_session_spent.saturating_add(from.elapsed());
        }
    }

    fn silence_deadline(&self) -> Instant {
        self.last_frame + duration(self.settings.deadlines.silence_ms)
    }

    fn pre_session_deadline(&self) -> Option<Instant> {
        let left = duration(self.settings.deadlines.become_session_ms)
            .saturating_sub(self.pre_session_spent);
        self.pre_session_from.map(|from| from + left)
    }

    /// Milliseconds since this connection started, on the monotonic clock: what the
    /// reassembler measures the idle streams with.
    fn elapsed_ms(&self) -> i64 {
        i64::try_from(self.base.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    fn idle_deadline(&self) -> Option<Instant> {
        let at = self.reassembler.next_deadline_ms()?;
        let left = at.saturating_sub(self.elapsed_ms());
        Some(Instant::now() + duration(left))
    }

    fn next_deadline(&self) -> Instant {
        [
            Some(self.silence_deadline()),
            self.pre_session_deadline(),
            self.idle_deadline(),
            self.next_ping,
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or_else(|| self.silence_deadline())
    }

    fn on_time(&mut self) -> Step {
        let now = Instant::now();
        if now >= self.silence_deadline() {
            return Step::End(Ending::Closing(ErrorCode::Ar204));
        }
        if self.pre_session_deadline().is_some_and(|at| now >= at) {
            return Step::End(Ending::Closing(ErrorCode::Ar204));
        }
        if let Err(error) = self.reassembler.check_idle(self.elapsed_ms()) {
            return Step::End(Ending::Closing(error.closing_code()));
        }
        if let Some(at) = self.next_ping
            && now >= at
        {
            self.ping_number = self.ping_number.saturating_add(1);
            self.next_ping = Some(now + duration(self.settings.deadlines.ping_interval_ms));
            let ping = Message::Ping(Ping {
                n: self.ping_number,
            });
            if self.link_handle().send_control(&ping).is_err() {
                return Step::End(Ending::Silent);
            }
        }
        Step::Go
    }

    async fn receive(&mut self, item: Option<Result<ws::Message, axum::Error>>) -> Step {
        let sealed = match item {
            Some(Ok(ws::Message::Binary(sealed))) => sealed,
            // Only binary messages are part of the protocol.
            Some(Ok(ws::Message::Text(_))) => {
                return Step::End(Ending::Closing(ErrorCode::Ar204));
            }
            // The library answers websocket pings itself; they are not frames of the channel.
            Some(Ok(ws::Message::Ping(_) | ws::Message::Pong(_))) => return Step::Go,
            // Gone, or a websocket the library refused (a message above 65,535 bytes is never
            // put together: the library stops at its header).
            Some(Ok(ws::Message::Close(_)) | Err(_)) | None => {
                return Step::End(Ending::Silent);
            }
        };
        // A repeated, missing or reordered frame does not decrypt: the counter is our own.
        let Ok(frame) = self.opener.open(&sealed) else {
            return Step::End(Ending::Closing(ErrorCode::Ar204));
        };
        let delivered = match self.reassembler.push(&frame, self.elapsed_ms()) {
            Ok(delivered) => delivered,
            Err(error) => return Step::End(Ending::Closing(error.closing_code())),
        };
        self.last_frame = Instant::now();
        let Some(delivered) = delivered else {
            return Step::Go;
        };
        let control = delivered.control;
        let payload = delivered.payload.into_owned();
        let decoded = if payload.len() > self.settings.inline_decode_max_bytes {
            match tokio::task::spawn_blocking(move || message::decode(&payload, control)).await {
                Ok(decoded) => decoded,
                Err(_) => return Step::End(Ending::Closing(ErrorCode::Ar205)),
            }
        } else {
            message::decode(&payload, control)
        };
        match decoded {
            Ok(message) => self.handle(message).await,
            Err(error) => Step::End(Ending::Closing(error.closing_code())),
        }
    }

    async fn handle(&mut self, message: Message) -> Step {
        let phase = self.phase;
        let Some(kind) = message.kind() else {
            // Unknown type: before the session it closes; in session the hundredth closes.
            if phase != Phase::Session {
                return Step::End(Ending::Closing(ErrorCode::Ar204));
            }
            self.unknown = self.unknown.saturating_add(1);
            if self.unknown >= arauto_protocol::limits::UNKNOWN_MESSAGES_MAX {
                return Step::End(Ending::Closing(ErrorCode::Ar204));
            }
            return Step::Go;
        };
        if !kind.sent_by(Side::Connector)
            || (phase == Phase::Waiting && !kind.allowed_while_waiting())
        {
            return Step::End(Ending::Closing(ErrorCode::Ar204));
        }
        match message {
            // The heartbeat starts with `hello_ok`; before it, a ping is out of order.
            Message::Ping(_) | Message::Pong(_) if !self.heartbeat => {
                Step::End(Ending::Closing(ErrorCode::Ar204))
            }
            Message::Ping(Ping { n }) => {
                let pong = Message::Pong(Pong { n });
                match self.link_handle().send_control(&pong) {
                    Ok(()) => Step::Go,
                    Err(_) => Step::End(Ending::Silent),
                }
            }
            Message::Pong(_) => Step::Go,
            // The answer to the test of a newer connection: it never waits for the code of the
            // purpose.
            Message::CheckOk(answer) if self.shared.answer_probe(&answer.nonce) => Step::Go,
            message => {
                let inbound = Inbound {
                    message,
                    in_session: self.in_session,
                };
                tokio::select! {
                    biased;
                    _ = self.ending.wait_for(|ending| *ending != Ending::Open) => Step::Go,
                    // The code of the purpose stopped listening: it is ending the connection.
                    _ = self.inbox.send(inbound) => Step::Go,
                }
            }
        }
    }
}
