//! One connection to the hub, on the side of the connector (`docs/14-protocolo-do-canal.md`,
//! sections 1 to 4; `docs/15-mvp.md`, section 3).
//!
//! ```text
//! TCP ── TLS (wss://, the `tls` feature) ── websocket (binary, 65,535 bytes, no compression)
//!   └─ Noise XK with the key of the hub of the block (48, 48, 64 bytes)   another key: AR-202
//!        └─ Link: reader + writer, two queues, the frames and their limits
//! ```
//!
//! - **One task reads, another writes.** Only the writer seals, when it writes, after it chose
//!   between the control queue and the data queue.
//! - **Two queues**: control (`ping`, `pong`, `check_ok`), which passes ahead, and data. A full
//!   control queue, or a frame that does not leave in 30 seconds, ends the connection.
//! - **The limits of each phase**: what the hub sends is put together by a reassembler of the
//!   frames of the hub; before `session` only stream 0 and 4,096 bytes, in session 16 MiB and 8
//!   streams. What this side sends is cut for the limit of its phase before it is sent.
//! - **The reader answers by itself**: `ping` with `pong`, and `check` with `check_ok` (the
//!   conference of whoever presented, and the test of an established connection by a newer
//!   one, which must be answered in 3 seconds whatever the session is doing).
//! - **Heartbeat**: from `hello_ok` on, a `ping` every 30 seconds; 90 seconds without any valid
//!   frame end the connection.
//! - **`closing`** is kept: it says why the hub ended the connection.

#[cfg(feature = "testing")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use arauto_protocol::errors::{ConnectFailure, ErrorCode};
use arauto_protocol::frame::{FrameHeader, Reassembler, Side, SplitError, Splitter, Unlimited};
use arauto_protocol::limits::{
    FRAME_CHUNK_MAX, MAX_OPEN_STREAMS, UNKNOWN_MESSAGES_MAX, WS_MESSAGE_MAX,
};
use arauto_protocol::message::{self, CheckOk, Message, MessageType, Ping, Pong};
use arauto_protocol::noise::{
    ConnectorHandshake, HandshakeHash, NoiseError, Opener, PublicKey, Sealer, StaticKeypair,
};
use bytes::Bytes;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{Semaphore, mpsc, watch};
use tokio::time::{Instant, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::address::HubAddress;
use crate::settings::{Settings, duration};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Messages the reader hands over before it waits for the code of the channel to take them.
const INBOX_MESSAGES: usize = 8;

/// Messages above this size are read on a blocking thread, away from the network tasks.
const INLINE_DECODE_MAX_BYTES: usize = 256 * 1024;

/// Why a connection to the hub could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenError {
    /// AR-201, with its cause.
    Unreachable(ConnectFailure),
    /// The hub answered the upgrade with 429 or 503: it is full or stopping.
    Busy,
    /// The server did not complete the handshake with the key of the block: AR-202.
    NotTheHub,
    /// This build cannot do TLS.
    NoTls,
    /// The generator of the system failed.
    Random,
}

/// Why a connection ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndReason {
    /// The hub closed it, after `closing` or not.
    Closed,
    /// Nothing valid came for the silence deadline.
    Silence,
    /// The hub broke the protocol: a frame, a message or an order that is not allowed.
    Protocol,
    /// The network failed, or a frame did not leave in time.
    Network,
    /// This side closed it.
    ByUs,
}

/// How a connection ended: the code of the `closing` of the hub, when it sent one, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct End {
    pub(crate) closing: Option<ErrorCode>,
    pub(crate) reason: EndReason,
}

/// Why a message was not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendError {
    /// It does not fit the limit of this phase, or could not be written: nothing was sent and
    /// the connection goes on. A call that produced it is answered with AR-709.
    TooLarge,
    /// The connection was over before anything of the message was queued.
    NotSent,
    /// The connection ended while the message was being queued: part of it may have left.
    Interrupted,
}

/// Why [`Link::recv_within`] returned without a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecvError {
    /// Nothing came in time. The connection is still open.
    TimedOut,
    /// The connection is over.
    Ended(End),
}

/// Opens a connection to the hub of `address` and `hub_key` with the identity `identity`, up
/// to the end of the handshake.
pub(crate) async fn open(
    address: &HubAddress,
    identity: &StaticKeypair,
    hub_key: &PublicKey,
    settings: &Settings,
) -> Result<Link, OpenError> {
    let mut ws = connect(address, settings).await?;
    let channel = handshake(&mut ws, identity, hub_key, settings.handshake_ms).await?;
    Ok(Link::start(ws, channel, settings))
}

/// TCP, TLS and the upgrade to websocket, within the deadline of the connection.
async fn connect(address: &HubAddress, settings: &Settings) -> Result<Ws, OpenError> {
    if address.is_secure() && !cfg!(feature = "tls") {
        return Err(OpenError::NoTls);
    }
    let attempt = async {
        let resolved = tokio::net::lookup_host((address.host(), address.port()))
            .await
            .map_err(|_| OpenError::Unreachable(ConnectFailure::NameNotResolved))?;
        let mut stream = None;
        for candidate in resolved {
            if let Ok(connected) = TcpStream::connect(candidate).await {
                stream = Some(connected);
                break;
            }
        }
        let stream = stream.ok_or(OpenError::Unreachable(ConnectFailure::Refused))?;
        let _ = stream.set_nodelay(true);
        let request = address
            .url()
            .into_client_request()
            .map_err(|_| OpenError::Unreachable(ConnectFailure::Refused))?;
        // What the hub sends is never more than one message of 65,535 bytes; nothing is
        // compressed.
        let config = WebSocketConfig::default()
            .max_message_size(Some(WS_MESSAGE_MAX))
            .max_frame_size(Some(WS_MESSAGE_MAX));
        let upgraded = upgrade(address.is_secure(), request, stream, config).await;
        match upgraded {
            Ok((ws, _response)) => Ok(ws),
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                match response.status().as_u16() {
                    429 | 503 => Err(OpenError::Busy),
                    _ => Err(OpenError::Unreachable(ConnectFailure::Refused)),
                }
            }
            Err(_) => Err(OpenError::Unreachable(ConnectFailure::Refused)),
        }
    };
    timeout(duration(settings.connect_ms), attempt)
        .await
        .map_err(|_| OpenError::Unreachable(ConnectFailure::TimedOut))?
}

type Upgraded = Result<
    (
        Ws,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    tokio_tungstenite::tungstenite::Error,
>;

/// The upgrade to websocket, over TLS when `secure`.
async fn upgrade(
    secure: bool,
    request: tokio_tungstenite::tungstenite::handshake::client::Request,
    stream: TcpStream,
    config: WebSocketConfig,
) -> Upgraded {
    #[cfg(feature = "tls")]
    if secure {
        return tls::upgrade(request, stream, config).await;
    }
    // Without the feature, `connect` refused every address with TLS before getting here.
    let _ = secure;
    tokio_tungstenite::client_async_with_config(
        request,
        MaybeTlsStream::Plain(stream),
        Some(config),
    )
    .await
}

/// TLS with the public authorities built into the program, through rustls and its `ring`
/// provider. The certificate is always checked: there is no way to turn the check off.
#[cfg(feature = "tls")]
mod tls {
    use std::sync::Arc;

    use tokio::net::TcpStream;
    use tokio_tungstenite::Connector;
    use tokio_tungstenite::tungstenite::handshake::client::Request;
    use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

    use super::Upgraded;

    pub(super) async fn upgrade(
        request: Request,
        stream: TcpStream,
        config: WebSocketConfig,
    ) -> Upgraded {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|error| {
                tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(error))
            })?
            .with_root_certificates(roots)
            .with_no_client_auth();
        tokio_tungstenite::client_async_tls_with_config(
            request,
            stream,
            Some(config),
            Some(Connector::Rustls(Arc::new(tls))),
        )
        .await
    }
}

/// The next binary message of the websocket, skipping the pings of the websocket itself.
/// `None` when the connection ended.
async fn next_binary(ws: &mut Ws) -> Option<Bytes> {
    loop {
        match ws.next().await {
            Some(Ok(WsMessage::Binary(bytes))) => return Some(bytes),
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Frame(_))) => {}
            Some(Ok(WsMessage::Text(_) | WsMessage::Close(_)) | Err(_)) | None => return None,
        }
    }
}

/// The three messages of Noise XK. A server that ends the connection, or answers what does not
/// complete the handshake with the key of the block, is not the hub of the block: AR-202.
async fn handshake(
    ws: &mut Ws,
    identity: &StaticKeypair,
    hub_key: &PublicKey,
    ms: i64,
) -> Result<arauto_protocol::noise::Channel, OpenError> {
    let (pending, first) = ConnectorHandshake::start(identity, hub_key).map_err(|error| {
        if error == NoiseError::Random {
            OpenError::Random
        } else {
            OpenError::NotTheHub
        }
    })?;
    let exchange = async {
        ws.send(WsMessage::Binary(Bytes::from(first.to_vec())))
            .await
            .map_err(|_| OpenError::Unreachable(ConnectFailure::Refused))?;
        let second = next_binary(ws).await.ok_or(OpenError::NotTheHub)?;
        let (third, channel) = pending.finish(&second).map_err(|_| OpenError::NotTheHub)?;
        ws.send(WsMessage::Binary(Bytes::from(third.to_vec())))
            .await
            .map_err(|_| OpenError::Unreachable(ConnectFailure::Refused))?;
        Ok(channel)
    };
    timeout(duration(ms), exchange)
        .await
        .map_err(|_| OpenError::Unreachable(ConnectFailure::TimedOut))?
}

// ---------------------------------------------------------------------------------------------
// The two tasks
// ---------------------------------------------------------------------------------------------

/// Whether the connection runs, ends after sending what is queued, or ends now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    Open,
    Done,
    Silent,
}

/// A frame waiting in a queue, not yet sealed.
#[derive(Debug)]
struct OutFrame {
    header: FrameHeader,
    chunk: Bytes,
}

struct Shared {
    ending: watch::Sender<Ending>,
    end: Mutex<Option<End>>,
    control: mpsc::Sender<OutFrame>,
    data: mpsc::Sender<OutFrame>,
    /// Held from the cut of a message until its first frame is queued: the streams reach the
    /// hub in the order of their numbers.
    data_splitter: tokio::sync::Mutex<Splitter>,
    control_splitter: Mutex<Splitter>,
    /// A message of more than one frame opens a stream; the hub allows 8 at a time.
    streams: Arc<Semaphore>,
    heartbeat: watch::Sender<bool>,
    /// The writer ended: after a close, what was queued and the close of the websocket left,
    /// or the network failed.
    written: watch::Sender<bool>,
    /// The reader leaves `check` unanswered: a fault of a test.
    #[cfg(feature = "testing")]
    hold_checks: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A splitter and an optional end: a panic elsewhere leaves them usable.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn is_open(&self) -> bool {
        *self.ending.borrow() == Ending::Open
    }

    /// Ends the connection, with the first reason given.
    fn finish(&self, end: End, ending: Ending) {
        {
            let mut kept = lock(&self.end);
            if kept.is_none() {
                *kept = Some(end);
            }
        }
        self.ending.send_if_modified(|current| {
            let replace = matches!(
                (*current, ending),
                (Ending::Open, _) | (Ending::Done, Ending::Silent)
            );
            if replace {
                *current = ending;
            }
            replace
        });
    }

    fn end(&self) -> End {
        lock(&self.end).unwrap_or(End {
            closing: None,
            reason: EndReason::Network,
        })
    }

    async fn ending(&self) {
        let mut ending = self.ending.subscribe();
        let _ = ending.wait_for(|ending| *ending != Ending::Open).await;
    }

    /// Whether the reader answers `check`: always, but under a fault of a test.
    fn answers_checks(&self) -> bool {
        #[cfg(feature = "testing")]
        return !self.hold_checks.load(Ordering::SeqCst);
        #[cfg(not(feature = "testing"))]
        true
    }
}

/// What every task can do with a connection: send, close, wait for its end.
#[derive(Clone)]
pub(crate) struct LinkHandle {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for LinkHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkHandle").finish_non_exhaustive()
    }
}

impl LinkHandle {
    /// Sends a data message: waits for room in the data queue, and for one of the 8 streams
    /// when it is larger than one frame.
    pub(crate) async fn send(&self, message: &Message) -> Result<(), SendError> {
        let shared = &self.shared;
        let bytes = Bytes::from(message::encode(message).map_err(|_| SendError::TooLarge)?);
        if !shared.is_open() {
            return Err(SendError::NotSent);
        }
        let _stream = if bytes.len() > FRAME_CHUNK_MAX {
            tokio::select! {
                biased;
                () = shared.ending() => return Err(SendError::NotSent),
                permit = Arc::clone(&shared.streams).acquire_owned() => {
                    Some(permit.map_err(|_| SendError::NotSent)?)
                }
            }
        } else {
            None
        };
        let mut splitter = tokio::select! {
            biased;
            () = shared.ending() => return Err(SendError::NotSent),
            splitter = shared.data_splitter.lock() => splitter,
        };
        let frames = match cut(&mut splitter, &bytes, false) {
            Ok(frames) => frames,
            Err(SplitError::StreamsExhausted) => {
                // Every stream number of this side is used: the connection ends, and the next
                // one starts again from 1.
                shared.finish(
                    End {
                        closing: None,
                        reason: EndReason::ByUs,
                    },
                    Ending::Done,
                );
                return Err(SendError::NotSent);
            }
            Err(_) => return Err(SendError::TooLarge),
        };
        let mut frames = frames.into_iter();
        if let Some(first) = frames.next() {
            self.enqueue_data(first)
                .await
                .map_err(|()| SendError::NotSent)?;
        }
        drop(splitter);
        for frame in frames {
            self.enqueue_data(frame)
                .await
                .map_err(|()| SendError::Interrupted)?;
        }
        Ok(())
    }

    async fn enqueue_data(&self, frame: OutFrame) -> Result<(), ()> {
        let shared = &self.shared;
        tokio::select! {
            biased;
            () = shared.ending() => Err(()),
            sent = shared.data.send(frame) => sent.map_err(|_| ()),
        }
    }

    /// Sends a control message (`ping`, `pong`, `check_ok`) ahead of the data queue. It never
    /// waits: a full control queue means the hub stopped reading, and the connection ends.
    pub(crate) fn send_control(&self, message: &Message) -> Result<(), SendError> {
        let shared = &self.shared;
        if !shared.is_open() {
            return Err(SendError::NotSent);
        }
        let bytes = Bytes::from(message::encode(message).map_err(|_| SendError::TooLarge)?);
        let frames = cut(&mut lock(&shared.control_splitter), &bytes, true)
            .map_err(|_| SendError::TooLarge)?;
        for frame in frames {
            if shared.control.try_send(frame).is_err() {
                shared.finish(
                    End {
                        closing: None,
                        reason: EndReason::Network,
                    },
                    Ending::Silent,
                );
                return Err(SendError::NotSent);
            }
        }
        Ok(())
    }

    /// Closes the connection: what is queued leaves first, then the websocket closes.
    pub(crate) fn close(&self) {
        self.shared.finish(
            End {
                closing: None,
                reason: EndReason::ByUs,
            },
            Ending::Done,
        );
    }

    /// Waits for the connection to end, and says how.
    pub(crate) async fn ended(&self) -> End {
        self.shared.ending().await;
        self.shared.end()
    }

    /// Waits for the writer to end: once the connection was closed, the frames that were
    /// queued and the close of the websocket left, or the network failed. It is the close in
    /// order of a program that is about to end.
    pub(crate) async fn closed(&self) {
        let mut written = self.shared.written.subscribe();
        let _ = written.wait_for(|written| *written).await;
    }

    /// Leaves every `check` unanswered from now on: a fault of a test.
    #[cfg(feature = "testing")]
    pub(crate) fn hold_checks(&self) {
        self.shared.hold_checks.store(true, Ordering::SeqCst);
    }
}

/// Cuts a message in frames that borrow nothing.
fn cut(splitter: &mut Splitter, bytes: &Bytes, control: bool) -> Result<Vec<OutFrame>, SplitError> {
    Ok(splitter
        .split(bytes, control)?
        .map(|frame| OutFrame {
            header: frame.header,
            chunk: bytes.slice_ref(frame.chunk),
        })
        .collect())
}

/// The connection as the code of the channel drives it: it receives every message of the hub
/// but the heartbeat, and it moves the phase.
pub(crate) struct Link {
    inbox: mpsc::Receiver<Message>,
    handle: LinkHandle,
    handshake_hash: HandshakeHash,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link").finish_non_exhaustive()
    }
}

impl Link {
    fn start(ws: Ws, channel: arauto_protocol::noise::Channel, settings: &Settings) -> Self {
        let (control, control_rx) = mpsc::channel(settings.control_queue_frames.max(1));
        let (data, data_rx) = mpsc::channel(settings.data_queue_frames.max(1));
        let (inbox_tx, inbox) = mpsc::channel(INBOX_MESSAGES);
        let shared = Arc::new(Shared {
            ending: watch::Sender::new(Ending::Open),
            end: Mutex::new(None),
            control,
            data,
            data_splitter: tokio::sync::Mutex::new(Splitter::new(Side::Connector)),
            control_splitter: Mutex::new(Splitter::new(Side::Connector)),
            streams: Arc::new(Semaphore::new(MAX_OPEN_STREAMS)),
            heartbeat: watch::Sender::new(false),
            written: watch::Sender::new(false),
            #[cfg(feature = "testing")]
            hold_checks: AtomicBool::new(false),
        });
        let (sink, stream) = ws.split();
        let writer = Writer {
            sink,
            sealer: channel.sealer,
            control: control_rx,
            data: data_rx,
            shared: Arc::clone(&shared),
            stall: duration(settings.write_stall_ms),
        };
        let now = Instant::now();
        let reader = Reader {
            stream,
            opener: channel.opener,
            reassembler: Reassembler::new(Side::Hub, settings.deadlines.stream_idle_ms, Unlimited),
            shared: Arc::clone(&shared),
            inbox: inbox_tx,
            base: now,
            last_frame: now,
            silence: duration(settings.deadlines.silence_ms),
            ping_every: duration(settings.deadlines.ping_interval_ms),
            next_ping: None,
            ping_number: 0,
            in_session: false,
            unknown: 0,
        };
        tokio::spawn(writer.run());
        tokio::spawn(reader.run());
        Self {
            inbox,
            handle: LinkHandle { shared },
            handshake_hash: channel.handshake_hash,
        }
    }

    /// A handle for other tasks.
    pub(crate) fn handle(&self) -> LinkHandle {
        self.handle.clone()
    }

    /// The Noise value `h` at the end of the handshake: it goes into the verification code.
    pub(crate) fn handshake_hash(&self) -> &HandshakeHash {
        &self.handshake_hash
    }

    /// The next message of the hub, however long it takes: for the wait for approval and the
    /// session. Every other step has a deadline ([`Link::recv_within`]).
    pub(crate) async fn recv(&mut self) -> Result<Message, End> {
        let shared = Arc::clone(&self.handle.shared);
        tokio::select! {
            biased;
            message = self.inbox.recv() => match message {
                Some(message) => Ok(message),
                None => Err(shared.end()),
            },
            () = shared.ending() => {
                // What arrived before the end is still read: a `closing` comes last.
                match self.inbox.try_recv() {
                    Ok(message) => Ok(message),
                    Err(_) => Err(shared.end()),
                }
            }
        }
    }

    /// The next message, if it comes within `ms` milliseconds.
    pub(crate) async fn recv_within(&mut self, ms: i64) -> Result<Message, RecvError> {
        match timeout(duration(ms), self.recv()).await {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(end)) => Err(RecvError::Ended(end)),
            Err(_) => Err(RecvError::TimedOut),
        }
    }

    /// Sends a data message. See [`LinkHandle::send`].
    pub(crate) async fn send(&self, message: &Message) -> Result<(), SendError> {
        self.handle.send(message).await
    }

    /// Starts the heartbeat. Called once `hello_ok` was read.
    pub(crate) fn start_heartbeat(&self) {
        self.handle.shared.heartbeat.send_replace(true);
    }

    /// Switches what this side sends to the limits of the session. The reader switched what
    /// it receives as soon as it read `session`.
    pub(crate) async fn enter_session(&self) {
        let shared = &self.handle.shared;
        shared.data_splitter.lock().await.enter_session();
        lock(&shared.control_splitter).enter_session();
    }

    /// Closes the connection. See [`LinkHandle::close`].
    pub(crate) fn close(&self) {
        self.handle.close();
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // Nobody reads it any more: the connection is over.
        self.handle.close();
    }
}

struct Writer {
    sink: SplitSink<Ws, WsMessage>,
    sealer: Sealer,
    control: mpsc::Receiver<OutFrame>,
    data: mpsc::Receiver<OutFrame>,
    shared: Arc<Shared>,
    stall: Duration,
}

impl Writer {
    async fn run(self) {
        let shared = Arc::clone(&self.shared);
        self.work().await;
        shared.written.send_replace(true);
    }

    async fn work(mut self) {
        let mut ending = self.shared.ending.subscribe();
        loop {
            let now = *ending.borrow_and_update();
            match now {
                Ending::Open => {}
                Ending::Silent => return,
                Ending::Done => return self.finish().await,
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
                Some(frame) = self.data.recv() => frame,
            };
            if self.write(&frame).await.is_err() {
                self.network_failed();
                return;
            }
        }
    }

    fn network_failed(&self) {
        self.shared.finish(
            End {
                closing: None,
                reason: EndReason::Network,
            },
            Ending::Silent,
        );
    }

    async fn write(&mut self, frame: &OutFrame) -> Result<(), ()> {
        let mut plaintext = Vec::with_capacity(frame.chunk.len().saturating_add(5));
        plaintext.extend_from_slice(&frame.header.encode());
        plaintext.extend_from_slice(&frame.chunk);
        let sealed = self.sealer.seal(&plaintext).map_err(|_| ())?;
        match timeout(
            self.stall,
            self.sink.send(WsMessage::Binary(Bytes::from(sealed))),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            _ => Err(()),
        }
    }

    /// What is queued leaves, within the write deadline, and the websocket closes.
    async fn finish(mut self) {
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
                return;
            }
        }
        let _ = timeout(self.stall, self.sink.send(WsMessage::Close(None))).await;
        let _ = timeout(self.stall, self.sink.close()).await;
    }
}

struct Reader {
    stream: SplitStream<Ws>,
    opener: Opener,
    reassembler: Reassembler<Unlimited>,
    shared: Arc<Shared>,
    inbox: mpsc::Sender<Message>,
    /// The start of the clock of the deadlines of the frames: a jump of the time of the system
    /// does not cut a message that is arriving.
    base: Instant,
    last_frame: Instant,
    silence: Duration,
    ping_every: Duration,
    next_ping: Option<Instant>,
    ping_number: u64,
    in_session: bool,
    unknown: u32,
}

/// What the reader does after one step.
enum Step {
    Go,
    End(End, Ending),
}

fn protocol() -> Step {
    Step::End(
        End {
            closing: None,
            reason: EndReason::Protocol,
        },
        Ending::Done,
    )
}

impl Reader {
    fn handle(&self) -> LinkHandle {
        LinkHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    async fn run(mut self) {
        let mut ending = self.shared.ending.subscribe();
        let mut heartbeat = self.shared.heartbeat.subscribe();
        loop {
            if *ending.borrow_and_update() != Ending::Open {
                return;
            }
            if *heartbeat.borrow_and_update() && self.next_ping.is_none() {
                self.next_ping = Some(Instant::now() + self.ping_every);
            }
            let deadline = self.next_deadline();
            let step = tokio::select! {
                biased;
                changed = ending.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    Step::Go
                }
                changed = heartbeat.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    Step::Go
                }
                item = self.stream.next() => self.receive(item).await,
                () = sleep_until(deadline) => self.on_time(),
            };
            if let Step::End(end, how) = step {
                self.shared.finish(end, how);
                return;
            }
        }
    }

    fn elapsed_ms(&self) -> i64 {
        i64::try_from(self.base.elapsed().as_millis()).unwrap_or(i64::MAX)
    }

    fn next_deadline(&self) -> Instant {
        let silence = self.last_frame + self.silence;
        let idle = self
            .reassembler
            .next_deadline_ms()
            .map(|at| Instant::now() + duration(at.saturating_sub(self.elapsed_ms())));
        [Some(silence), idle, self.next_ping]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(silence)
    }

    fn on_time(&mut self) -> Step {
        let now = Instant::now();
        if now >= self.last_frame + self.silence {
            return Step::End(
                End {
                    closing: None,
                    reason: EndReason::Silence,
                },
                Ending::Done,
            );
        }
        if self.reassembler.check_idle(self.elapsed_ms()).is_err() {
            return protocol();
        }
        if let Some(at) = self.next_ping
            && now >= at
        {
            self.ping_number = self.ping_number.saturating_add(1);
            self.next_ping = Some(now + self.ping_every);
            let ping = Message::Ping(Ping {
                n: self.ping_number,
            });
            if self.handle().send_control(&ping).is_err() {
                return Step::End(
                    End {
                        closing: None,
                        reason: EndReason::Network,
                    },
                    Ending::Silent,
                );
            }
        }
        Step::Go
    }

    async fn receive(
        &mut self,
        item: Option<Result<WsMessage, tokio_tungstenite::tungstenite::Error>>,
    ) -> Step {
        let sealed = match item {
            Some(Ok(WsMessage::Binary(sealed))) => sealed,
            // Only binary messages are part of the protocol.
            Some(Ok(WsMessage::Text(_))) => return protocol(),
            // The library answers the pings of the websocket itself.
            Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Frame(_))) => {
                return Step::Go;
            }
            Some(Ok(WsMessage::Close(_))) | None => {
                return Step::End(
                    End {
                        closing: None,
                        reason: EndReason::Closed,
                    },
                    Ending::Silent,
                );
            }
            Some(Err(_)) => {
                return Step::End(
                    End {
                        closing: None,
                        reason: EndReason::Network,
                    },
                    Ending::Silent,
                );
            }
        };
        // A repeated, missing or reordered frame does not decrypt: the counter is our own.
        let Ok(frame) = self.opener.open(&sealed) else {
            return protocol();
        };
        let delivered = match self.reassembler.push(&frame, self.elapsed_ms()) {
            Ok(delivered) => delivered,
            Err(_) => return protocol(),
        };
        self.last_frame = Instant::now();
        let Some(delivered) = delivered else {
            return Step::Go;
        };
        let control = delivered.control;
        let payload = delivered.payload.into_owned();
        let decoded = if payload.len() > INLINE_DECODE_MAX_BYTES {
            match tokio::task::spawn_blocking(move || message::decode(&payload, control)).await {
                Ok(decoded) => decoded,
                Err(_) => return protocol(),
            }
        } else {
            message::decode(&payload, control)
        };
        match decoded {
            Ok(message) => self.handle_message(message).await,
            Err(_) => protocol(),
        }
    }

    async fn handle_message(&mut self, message: Message) -> Step {
        let Some(kind) = message.kind() else {
            // Unknown type: before the session it ends the connection; in session the
            // hundredth does.
            if !self.in_session {
                return protocol();
            }
            self.unknown = self.unknown.saturating_add(1);
            if self.unknown >= UNKNOWN_MESSAGES_MAX {
                return protocol();
            }
            return Step::Go;
        };
        // A message only the connector sends, coming from the hub.
        if !kind.sent_by(Side::Hub) {
            return protocol();
        }
        let heartbeat = *self.shared.heartbeat.borrow();
        match message {
            // The heartbeat starts with `hello_ok`.
            Message::Ping(_) | Message::Pong(_) if !heartbeat => protocol(),
            Message::Ping(Ping { n }) => {
                match self.handle().send_control(&Message::Pong(Pong { n })) {
                    Ok(()) => Step::Go,
                    Err(_) => Step::End(
                        End {
                            closing: None,
                            reason: EndReason::Network,
                        },
                        Ending::Silent,
                    ),
                }
            }
            Message::Pong(_) => Step::Go,
            Message::Closing(closing) => Step::End(
                End {
                    closing: Some(closing.code),
                    reason: EndReason::Closed,
                },
                Ending::Done,
            ),
            message => {
                if let Message::Check(check) = &message
                    && self.shared.answers_checks()
                {
                    // Answered here, whatever the code of the channel is doing.
                    let answer = Message::CheckOk(CheckOk { nonce: check.nonce });
                    if self.handle().send_control(&answer).is_err() {
                        return Step::End(
                            End {
                                closing: None,
                                reason: EndReason::Network,
                            },
                            Ending::Silent,
                        );
                    }
                }
                if kind == MessageType::Session {
                    // Whatever the hub sends from here on comes with the limits of the session.
                    self.reassembler.enter_session();
                    self.in_session = true;
                }
                let mut ending = self.shared.ending.subscribe();
                tokio::select! {
                    biased;
                    _ = ending.wait_for(|ending| *ending != Ending::Open) => Step::Go,
                    sent = self.inbox.send(message) => match sent {
                        Ok(()) => Step::Go,
                        // Nobody reads the connection any more.
                        Err(_) => Step::End(
                            End {
                                closing: None,
                                reason: EndReason::ByUs,
                            },
                            Ending::Done,
                        ),
                    },
                }
            }
        }
    }
}
