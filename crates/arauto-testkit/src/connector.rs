//! A low-level connector for tests: it speaks the protocol of the channel over `ws://`, and
//! lets a test break any rule of it on purpose.
//!
//! ```text
//! RawSocket::open ── the websocket, nothing else
//!   └─ handshake ──> TestConnector: send / recv messages, send raw frames, read the end
//! ```
//!
//! [`TestConnector::recv`] answers the `ping`s of the hub (unless told not to), skips the
//! `pong`s, switches to the limits of the session when it reads `session`, and turns the end
//! of the connection into [`KitError::Ended`], with the code of the `closing` it read on the
//! way.

use std::net::SocketAddr;
use std::time::Duration;

use arauto_protocol::clock::{Clock, SystemClock};
use arauto_protocol::errors::ErrorCode;
use arauto_protocol::frame::{Reassembler, Side, Splitter, Unlimited};
use arauto_protocol::limits::STREAM_IDLE_TIMEOUT_MS;
use arauto_protocol::message::{
    self, Closing, DeclaredText, Hello, HelloOk, Message, Ping, Pong, ProtoRange, Purpose,
};
use arauto_protocol::noise::{
    ConnectorHandshake, HandshakeHash, Opener, PublicKey, Sealer, StaticKeypair,
};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::{Instant, timeout_at};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::{Ended, KitError};

/// The path of the channel.
pub const CHANNEL_PATH: &str = "/v1/channel";

/// What the test connector says it is in `hello`.
pub const CONNECTOR_VERSION: &str = "0.1.0-test";

/// A websocket to the channel, before the handshake.
pub struct RawSocket {
    ws: WebSocketStream<TcpStream>,
    opened: Instant,
}

impl std::fmt::Debug for RawSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawSocket").finish_non_exhaustive()
    }
}

impl RawSocket {
    /// Opens the websocket of the channel at `addr`.
    ///
    /// # Errors
    /// [`KitError::Refused`] with the status when the hub does not upgrade; the network.
    pub async fn open(addr: SocketAddr) -> Result<Self, KitError> {
        Self::open_with(addr, &[]).await
    }

    /// Opens the websocket with extra request headers, such as `X-Forwarded-For`.
    ///
    /// # Errors
    /// As [`RawSocket::open`].
    pub async fn open_with(addr: SocketAddr, headers: &[(&str, &str)]) -> Result<Self, KitError> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        let mut request = format!("ws://{addr}{CHANNEL_PATH}").into_client_request()?;
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| KitError::Unexpected(format!("bad header name {name}")))?;
            let value = HeaderValue::from_str(value)
                .map_err(|_| KitError::Unexpected(format!("bad header value {value}")))?;
            request.headers_mut().append(name, value);
        }
        // No ceiling on what this side sends: the tests send oversized messages on purpose.
        let config = WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None);
        let (ws, _response) = match tokio_tungstenite::client_async_with_config(
            request,
            stream,
            Some(config),
        )
        .await
        {
            Ok(done) => done,
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                return Err(KitError::Refused(response.status().as_u16()));
            }
            Err(other) => return Err(other.into()),
        };
        Ok(Self {
            ws,
            opened: Instant::now(),
        })
    }

    /// When the websocket opened.
    #[must_use]
    pub fn opened(&self) -> Instant {
        self.opened
    }

    /// Sends a binary websocket message as it is.
    ///
    /// # Errors
    /// The websocket.
    pub async fn send_binary(&mut self, bytes: Vec<u8>) -> Result<(), KitError> {
        Ok(self.ws.send(WsMessage::Binary(Bytes::from(bytes))).await?)
    }

    /// Sends a text websocket message.
    ///
    /// # Errors
    /// The websocket.
    pub async fn send_text(&mut self, text: &str) -> Result<(), KitError> {
        Ok(self.ws.send(WsMessage::text(text)).await?)
    }

    /// The next binary websocket message.
    ///
    /// # Errors
    /// [`KitError::TimedOut`]; [`KitError::Ended`] when the hub closes.
    pub async fn recv_binary(&mut self, within: Duration) -> Result<Bytes, KitError> {
        let deadline = Instant::now() + within;
        loop {
            let item = timeout_at(deadline, self.ws.next())
                .await
                .map_err(|_| KitError::TimedOut)?;
            match item {
                Some(Ok(WsMessage::Binary(bytes))) => return Ok(bytes),
                Some(Ok(WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Frame(_))) => {}
                Some(Ok(WsMessage::Text(_))) => {
                    return Err(KitError::Unexpected("a text message from the hub".into()));
                }
                Some(Ok(WsMessage::Close(_)) | Err(_)) | None => {
                    return Err(KitError::Ended(self.ended_now(None)));
                }
            }
        }
    }

    /// Waits for the hub to close, reading and dropping whatever comes before.
    ///
    /// # Errors
    /// [`KitError::TimedOut`] when the hub keeps the connection open.
    pub async fn ended(&mut self, within: Duration) -> Result<Ended, KitError> {
        loop {
            match self.recv_binary(within).await {
                Ok(_) => {}
                Err(KitError::Ended(ended)) => return Ok(ended),
                Err(other) => return Err(other),
            }
        }
    }

    fn ended_now(&self, closing: Option<ErrorCode>) -> Ended {
        Ended {
            closing,
            after: self.opened.elapsed(),
        }
    }

    /// The Noise handshake, as the connector does it.
    ///
    /// # Errors
    /// [`KitError::Noise`] when the hub is not the one of `hub_key`; the websocket.
    pub async fn handshake(
        mut self,
        identity: &StaticKeypair,
        hub_key: &PublicKey,
        within: Duration,
    ) -> Result<TestConnector, KitError> {
        let (handshake, first) = ConnectorHandshake::start(identity, hub_key)?;
        self.send_binary(first.to_vec()).await?;
        let second = self.recv_binary(within).await?;
        let (third, channel) = handshake.finish(&second)?;
        self.send_binary(third.to_vec()).await?;
        Ok(TestConnector {
            raw: self,
            sealer: channel.sealer,
            opener: channel.opener,
            handshake_hash: channel.handshake_hash,
            splitter: Splitter::new(Side::Connector),
            reassembler: Reassembler::new(Side::Hub, STREAM_IDLE_TIMEOUT_MS, Unlimited),
            closing: None,
            answer_pings: true,
            pings: 0,
        })
    }
}

/// A connector after the handshake.
pub struct TestConnector {
    raw: RawSocket,
    sealer: Sealer,
    opener: Opener,
    handshake_hash: HandshakeHash,
    splitter: Splitter,
    reassembler: Reassembler<Unlimited>,
    closing: Option<ErrorCode>,
    answer_pings: bool,
    pings: u64,
}

impl std::fmt::Debug for TestConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestConnector")
            .field("pings", &self.pings)
            .finish_non_exhaustive()
    }
}

/// A new identity, from the generator of the system.
///
/// # Errors
/// [`KitError::Random`].
pub fn identity() -> Result<StaticKeypair, KitError> {
    Ok(StaticKeypair::generate()?)
}

/// The `hello` of the test connector.
///
/// # Errors
/// [`KitError::Unexpected`] for a range the protocol does not allow.
pub fn hello(proto: (u32, u32), purpose: Purpose) -> Result<Message, KitError> {
    let proto = ProtoRange::new(proto.0, proto.1)
        .ok_or_else(|| KitError::Unexpected("a protocol range from 1, lowest first".into()))?;
    let connector = DeclaredText::parse(CONNECTOR_VERSION)
        .map_err(|_| KitError::Unexpected("the version of the test connector".into()))?;
    Ok(Message::Hello(Hello {
        proto,
        connector,
        purpose,
    }))
}

impl TestConnector {
    /// Opens the websocket and does the handshake.
    ///
    /// # Errors
    /// As [`RawSocket::open`] and [`RawSocket::handshake`].
    pub async fn connect(
        addr: SocketAddr,
        identity: &StaticKeypair,
        hub_key: &PublicKey,
    ) -> Result<Self, KitError> {
        RawSocket::open(addr)
            .await?
            .handshake(identity, hub_key, Duration::from_secs(10))
            .await
    }

    /// The handshake hash, as this side read it.
    #[must_use]
    pub fn handshake_hash(&self) -> &HandshakeHash {
        &self.handshake_hash
    }

    /// When the websocket opened.
    #[must_use]
    pub fn opened(&self) -> Instant {
        self.raw.opened
    }

    /// Whether `recv` answers the `ping`s of the hub (it does by default).
    pub fn answer_pings(&mut self, answer: bool) {
        self.answer_pings = answer;
    }

    /// How many `ping`s of the hub were read.
    #[must_use]
    pub fn pings(&self) -> u64 {
        self.pings
    }

    /// Sends `hello` with the versions of this build and reads `hello_ok`.
    ///
    /// # Errors
    /// [`KitError::Ended`] when the hub closes instead; [`KitError::Unexpected`] for another
    /// message.
    pub async fn hello(&mut self, purpose: Purpose) -> Result<HelloOk, KitError> {
        let range = ProtoRange::CURRENT;
        self.send(&hello((range.min(), range.max()), purpose)?)
            .await?;
        match self.recv(Duration::from_secs(10)).await? {
            Message::HelloOk(answer) => Ok(answer),
            other => Err(KitError::Unexpected(format!("{:?}", other.kind()))),
        }
    }

    /// Switches this side to the limits of the session. [`TestConnector::recv`] does it by
    /// itself when it reads `session`.
    pub fn enter_session(&mut self) {
        self.splitter.enter_session();
        self.reassembler.enter_session();
    }

    /// Sends a message, cut in frames and sealed.
    ///
    /// # Errors
    /// [`KitError::Encode`], [`KitError::Split`], the websocket.
    pub async fn send(&mut self, message: &Message) -> Result<(), KitError> {
        let bytes = message::encode(message)?;
        let frames: Vec<Vec<u8>> = self
            .splitter
            .split(&bytes, message.is_control())?
            .map(|frame| frame.to_bytes())
            .collect();
        for frame in frames {
            self.send_frame(&frame).await?;
        }
        Ok(())
    }

    /// The frames of a message, not sealed, cut by this side's splitter: a test sends them in
    /// any order, or only some of them.
    ///
    /// # Errors
    /// [`KitError::Encode`], [`KitError::Split`].
    pub fn frames_of(&mut self, message: &Message) -> Result<Vec<Vec<u8>>, KitError> {
        let bytes = message::encode(message)?;
        Ok(self
            .splitter
            .split(&bytes, message.is_control())?
            .map(|frame| frame.to_bytes())
            .collect())
    }

    /// A frame put together by hand: flags, stream number, chunk.
    #[must_use]
    pub fn raw_frame(flags: u8, stream: u32, chunk: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(chunk.len().saturating_add(5));
        frame.push(flags);
        frame.extend_from_slice(&stream.to_be_bytes());
        frame.extend_from_slice(chunk);
        frame
    }

    /// Seals a frame with the next counter of this side, without sending it.
    ///
    /// # Errors
    /// [`KitError::Noise`].
    pub fn seal(&mut self, frame: &[u8]) -> Result<Vec<u8>, KitError> {
        Ok(self.sealer.seal(frame)?)
    }

    /// Seals a frame and sends it.
    ///
    /// # Errors
    /// [`KitError::Noise`], the websocket.
    pub async fn send_frame(&mut self, frame: &[u8]) -> Result<(), KitError> {
        let sealed = self.seal(frame)?;
        self.raw.send_binary(sealed).await
    }

    /// Sends bytes as one binary websocket message, sealed or not.
    ///
    /// # Errors
    /// The websocket.
    pub async fn send_sealed(&mut self, sealed: Vec<u8>) -> Result<(), KitError> {
        self.raw.send_binary(sealed).await
    }

    /// Sends a text websocket message.
    ///
    /// # Errors
    /// The websocket.
    pub async fn send_text(&mut self, text: &str) -> Result<(), KitError> {
        self.raw.send_text(text).await
    }

    /// The next message of the hub, whatever it is (`ping`, `pong` and `closing` included).
    ///
    /// # Errors
    /// [`KitError::TimedOut`]; [`KitError::Ended`] when the connection is over; what the hub
    /// should not have sent.
    pub async fn recv_any(&mut self, within: Duration) -> Result<Message, KitError> {
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let sealed = match self.raw.recv_binary(left).await {
                Ok(sealed) => sealed,
                Err(KitError::Ended(_)) => {
                    return Err(KitError::Ended(self.raw.ended_now(self.closing)));
                }
                Err(other) => return Err(other),
            };
            let frame = self.opener.open(&sealed)?;
            let Some(delivered) = self.reassembler.push(&frame, SystemClock.now_ms())? else {
                continue;
            };
            let message = message::decode(&delivered.payload, delivered.control)?;
            if matches!(message, Message::Session(_)) {
                self.enter_session();
            }
            if let Message::Closing(Closing { code }) = message {
                self.closing = Some(code);
            }
            return Ok(message);
        }
    }

    /// The next message of the hub that is not of the heartbeat nor `closing`. A `ping` is
    /// answered (unless told not to); after `closing` the connection is read to its end.
    ///
    /// # Errors
    /// As [`TestConnector::recv_any`].
    pub async fn recv(&mut self, within: Duration) -> Result<Message, KitError> {
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.recv_any(left).await? {
                Message::Ping(Ping { n }) => {
                    self.pings = self.pings.saturating_add(1);
                    if self.answer_pings {
                        self.send(&Message::Pong(Pong { n })).await?;
                    }
                }
                Message::Pong(_) | Message::Closing(_) => {}
                message => return Ok(message),
            }
        }
    }

    /// Reads until the hub ends the connection.
    ///
    /// # Errors
    /// [`KitError::TimedOut`] when the connection is still open after `within`.
    pub async fn ended(&mut self, within: Duration) -> Result<Ended, KitError> {
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.recv(left).await {
                Ok(_) => {}
                Err(KitError::Ended(ended)) => return Ok(ended),
                Err(other) => return Err(other),
            }
        }
    }
}
