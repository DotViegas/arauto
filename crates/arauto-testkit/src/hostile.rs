//! A hostile client: each method does one abuse that `docs/14-protocolo-do-canal.md` tells
//! the hub to refuse, and reports how the hub ended the connection.
//!
//! | Abuse | Section of the 14 | What the hub does |
//! |---|---|---|
//! | text message | 1 | `closing` AR-204 (after the handshake), or closes (before) |
//! | message above 65,535 bytes | 1 | closes; the message is never put together |
//! | headers that never end | 1 | closes after 5 s |
//! | handshake message of the wrong size | 2 | closes without an answer |
//! | key of low order | 2 | closes without an answer |
//! | handshake that never ends | 2 | closes after 10 s from the opening |
//! | reserved flag bits | 3 | `closing` AR-204 |
//! | empty chunk | 3 | `closing` AR-204 |
//! | stream other than 0 before the session | 4 | `closing` AR-204 |
//! | message above 4,096 bytes before the session | 4 | `closing` AR-204 |
//! | stream number of the wrong parity, repeated or lower | 3 | `closing` AR-204 |
//! | repeated or reordered frame | 3 | `closing` AR-204: the counter does not match |
//! | many connections from one origin | 4 | 429 (origin) or 503 (hub), before the websocket |
//!
//! The abuses of the session ([`stream_numbering`]) take a connector the test already took
//! there; the others need only the address and the key of the hub.

use std::net::SocketAddr;
use std::time::Duration;

use arauto_protocol::frame::FLAG_FIN;
use arauto_protocol::limits::{
    FRAME_CHUNK_MAX, HANDSHAKE_FIRST_LEN, PRE_SESSION_MESSAGE_MAX, WS_MESSAGE_MAX,
};
use arauto_protocol::message::{Call, CallId, Message, Payload, Purpose, encode};
use arauto_protocol::noise::{ConnectorHandshake, PublicKey};
use arauto_protocol::tools::ToolName;
use serde_json::{Map, Value};

use crate::connector::{RawSocket, TestConnector, hello, identity};
use crate::http::{self, Upgrade};
use crate::{Ended, KitError};

/// How long a hostile method waits for the hub to react.
const PATIENCE: Duration = Duration::from_secs(10);

/// Which handshake message goes with the wrong size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeStep {
    /// The first one, of 48 bytes.
    First,
    /// The third one, of 64 bytes.
    Third,
}

/// A rule of the stream numbers to break, once the session started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbering {
    /// An even number from the connector.
    WrongParity,
    /// A number already used by a message that ended.
    Repeated,
    /// A number below the last one opened.
    Lower,
}

/// A client that breaks the rules of the channel.
#[derive(Debug, Clone)]
pub struct Hostile {
    /// The port of the connectors.
    pub addr: SocketAddr,
    /// The public key of the hub.
    pub hub_key: PublicKey,
}

impl Hostile {
    /// A hostile client of the hub at `addr`.
    #[must_use]
    pub fn new(addr: SocketAddr, hub_key: PublicKey) -> Self {
        Self { addr, hub_key }
    }

    async fn connected(&self) -> Result<TestConnector, KitError> {
        TestConnector::connect(self.addr, &identity()?, &self.hub_key).await
    }

    /// A text message, after the handshake.
    ///
    /// # Errors
    /// The network; [`KitError::TimedOut`] when the hub does not close.
    pub async fn text_message(&self) -> Result<Ended, KitError> {
        let mut connector = self.connected().await?;
        connector.send_text("{\"t\":\"hello\"}").await?;
        connector.ended(PATIENCE).await
    }

    /// A text message instead of the first handshake message.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn text_before_the_handshake(&self) -> Result<Ended, KitError> {
        let mut socket = RawSocket::open(self.addr).await?;
        socket.send_text("hello").await?;
        socket.ended(PATIENCE).await
    }

    /// One binary websocket message one byte above 65,535, after the handshake.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn oversized_websocket_message(&self) -> Result<Ended, KitError> {
        let mut connector = self.connected().await?;
        // A failed send is the hub having closed already.
        let _ = connector
            .send_sealed(vec![0x42; WS_MESSAGE_MAX.saturating_add(1)])
            .await;
        connector.ended(PATIENCE).await
    }

    /// A handshake message with `len` bytes in place of the right size.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn handshake_of_the_wrong_size(
        &self,
        step: HandshakeStep,
        len: usize,
    ) -> Result<Ended, KitError> {
        let mut socket = RawSocket::open(self.addr).await?;
        match step {
            HandshakeStep::First => {
                let _ = socket.send_binary(vec![0x11; len]).await;
            }
            HandshakeStep::Third => {
                let (handshake, first) = ConnectorHandshake::start(&identity()?, &self.hub_key)?;
                socket.send_binary(first.to_vec()).await?;
                let second = socket.recv_binary(PATIENCE).await?;
                let (third, _channel) = handshake.finish(&second)?;
                let mut wrong = third.to_vec();
                wrong.resize(len, 0x22);
                let _ = socket.send_binary(wrong).await;
            }
        }
        socket.ended(PATIENCE).await
    }

    /// A first handshake message whose ephemeral key is a point of low order (zero): every
    /// Diffie-Hellman with it gives the same value, whatever the private key.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn low_order_key(&self) -> Result<Ended, KitError> {
        let mut socket = RawSocket::open(self.addr).await?;
        let mut first = vec![0u8; HANDSHAKE_FIRST_LEN];
        // The tag after the key: any 16 bytes.
        if let Some(tag) = first.get_mut(32..) {
            tag.fill(0x5a);
        }
        let _ = socket.send_binary(first).await;
        socket.ended(PATIENCE).await
    }

    /// Opens the websocket and stops: no handshake message at all, or only the first one.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn handshake_that_never_ends(&self, send_first: bool) -> Result<Ended, KitError> {
        let mut socket = RawSocket::open(self.addr).await?;
        if send_first {
            let (_handshake, first) = ConnectorHandshake::start(&identity()?, &self.hub_key)?;
            socket.send_binary(first.to_vec()).await?;
        }
        socket.ended(PATIENCE.saturating_mul(2)).await
    }

    /// Headers that arrive one at a time and never end.
    ///
    /// # Errors
    /// [`KitError::TimedOut`] when the hub keeps the connection.
    pub async fn slow_headers(&self, every: Duration) -> Result<Ended, KitError> {
        http::slow_headers(self.addr, every, PATIENCE.saturating_mul(2)).await
    }

    async fn after_handshake(&self, frame: &[u8]) -> Result<Ended, KitError> {
        let mut connector = self.connected().await?;
        let _ = connector.send_frame(frame).await;
        connector.ended(PATIENCE).await
    }

    /// A frame with a reserved flag bit set.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn reserved_bits(&self) -> Result<Ended, KitError> {
        let body = hello_bytes(Purpose::Leave)?;
        self.after_handshake(&TestConnector::raw_frame(FLAG_FIN | 0b0000_0100, 0, &body))
            .await
    }

    /// A frame with nothing after its header.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn empty_chunk(&self) -> Result<Ended, KitError> {
        self.after_handshake(&TestConnector::raw_frame(FLAG_FIN, 0, &[]))
            .await
    }

    /// A stream other than 0 before the session.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn stream_before_the_session(&self) -> Result<Ended, KitError> {
        let body = hello_bytes(Purpose::Leave)?;
        self.after_handshake(&TestConnector::raw_frame(0, 1, &body))
            .await
    }

    /// A valid `hello` of 4,097 bytes on stream 0, before the session: one byte above the
    /// limit of the phase. The same `hello` one byte shorter is accepted.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn oversized_before_the_session(&self) -> Result<Ended, KitError> {
        let body = padded_hello(PRE_SESSION_MESSAGE_MAX.saturating_add(1))?;
        self.after_handshake(&TestConnector::raw_frame(FLAG_FIN, 0, &body))
            .await
    }

    /// The same sealed frame twice: the first is `hello`, the second does not decrypt.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn repeated_frame(&self) -> Result<Ended, KitError> {
        let mut connector = self.connected().await?;
        let frame = TestConnector::raw_frame(FLAG_FIN, 0, &hello_bytes(Purpose::Leave)?);
        let sealed = connector.seal(&frame)?;
        connector.send_sealed(sealed.clone()).await?;
        let _ = connector.send_sealed(sealed).await;
        connector.ended(PATIENCE).await
    }

    /// Two frames sealed in order and sent in the other order.
    ///
    /// # Errors
    /// As [`Hostile::text_message`].
    pub async fn reordered_frames(&self) -> Result<Ended, KitError> {
        let mut connector = self.connected().await?;
        let frame = TestConnector::raw_frame(FLAG_FIN, 0, &hello_bytes(Purpose::Leave)?);
        let first = connector.seal(&frame)?;
        let second = connector.seal(&frame)?;
        connector.send_sealed(second).await?;
        let _ = connector.send_sealed(first).await;
        connector.ended(PATIENCE).await
    }

    /// Asks for `count` websockets from one origin and keeps them open, without a handshake.
    /// With `forwarded_for`, each request says it comes from that address (the hub believes
    /// it only from a trusted proxy).
    ///
    /// # Errors
    /// The network.
    pub async fn many_connections(
        &self,
        count: usize,
        forwarded_for: Option<&str>,
    ) -> Result<Vec<Upgrade>, KitError> {
        let headers: Vec<(&str, &str)> = forwarded_for
            .map(|address| vec![("X-Forwarded-For", address)])
            .unwrap_or_default();
        let mut upgrades = Vec::with_capacity(count);
        for _ in 0..count {
            upgrades.push(http::upgrade(self.addr, &headers, PATIENCE).await?);
        }
        Ok(upgrades)
    }
}

/// The bytes of a `hello` of this build.
fn hello_bytes(purpose: Purpose) -> Result<Vec<u8>, KitError> {
    Ok(encode(&hello((1, 1), purpose)?)?)
}

/// A `hello` with purpose `leave` of exactly `len` bytes, padded with a field the hub ignores.
///
/// # Errors
/// [`KitError::Unexpected`] when `len` is too short for a `hello`.
pub fn padded_hello(len: usize) -> Result<Vec<u8>, KitError> {
    let mut bytes = hello_bytes(Purpose::Leave)?;
    // `{...}` becomes `{...,"pad":"aaaa"}`.
    let envelope = ",\"pad\":\"\"".len();
    let pad = len
        .checked_sub(bytes.len().saturating_add(envelope))
        .ok_or_else(|| KitError::Unexpected("too short for a hello".into()))?;
    bytes.pop();
    bytes.extend_from_slice(b",\"pad\":\"");
    bytes.extend(std::iter::repeat_n(b'a', pad));
    bytes.extend_from_slice(b"\"}");
    Ok(bytes)
}

/// The bytes of a message of a type this version does not know.
#[must_use]
pub fn unknown_message() -> Vec<u8> {
    b"{\"t\":\"mystery\"}".to_vec()
}

/// Breaks a rule of the stream numbers on a connector that is in session. Every frame is
/// valid on its own; only the number is wrong.
///
/// # Errors
/// As [`Hostile::text_message`].
pub async fn stream_numbering(
    connector: &mut TestConnector,
    abuse: Numbering,
) -> Result<Ended, KitError> {
    let chunk = vec![b' '; FRAME_CHUNK_MAX];
    let open = |stream: u32| TestConnector::raw_frame(0, stream, &chunk);
    let mut frames = Vec::new();
    match abuse {
        Numbering::WrongParity => frames.push(open(2)),
        Numbering::Repeated => {
            // A whole message on the first stream of the connector, then that stream again.
            let call = big_call(FRAME_CHUNK_MAX.saturating_add(1))?;
            let whole = connector.frames_of(&call)?;
            let used = whole
                .first()
                .and_then(|frame| frame.get(1..5))
                .and_then(|number| <[u8; 4]>::try_from(number).ok())
                .map(u32::from_be_bytes)
                .ok_or_else(|| KitError::Unexpected("a message of one frame".into()))?;
            frames.extend(whole);
            frames.push(open(used));
        }
        Numbering::Lower => {
            frames.push(open(5));
            frames.push(open(3));
        }
    }
    for frame in frames {
        if connector.send_frame(&frame).await.is_err() {
            break;
        }
    }
    connector.ended(PATIENCE).await
}

/// A `call` of `arauto_status` whose arguments carry `pad` bytes of padding: a valid message
/// of the size a test needs.
///
/// # Errors
/// [`KitError::Unexpected`] when the name of the tool is refused.
pub fn big_call(pad: usize) -> Result<Message, KitError> {
    let mut args = Map::new();
    args.insert("pad".to_owned(), Value::String("a".repeat(pad)));
    Ok(Message::Call(Call {
        id: CallId::MIN,
        tool: ToolName::parse("arauto_status")
            .map_err(|_| KitError::Unexpected("the name of a tool".into()))?,
        args: Payload::new(args),
    }))
}
