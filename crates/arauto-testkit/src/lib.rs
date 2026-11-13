//! Test tools for Arauto: a scripted connector, a hostile client, a load generator and a fake
//! Hermes.
//!
//! Never shipped: only the tests of the workspace use this crate.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`connector`] | A low-level connector over `ws://`: the websocket, the handshake, frames, messages, deadlines, and raw frames for the tests that break the rules. |
//! | [`device`] | A connector of high level: enrollment, the channel to the session with its swap, the 24-hour swap, calls, and faults on command. |
//! | [`load`] | A generator of load: many devices, each with its identity, coming back at once, the heartbeat, and local reads with their latencies. |
//! | [`hostile`] | A client that does, one at a time, every abuse `docs/14-protocolo-do-canal.md` tells the hub to refuse. |
//! | [`http`] | Plain HTTP over TCP, for what the websocket library would not send: slow headers, a raw upgrade, a path as written. |
//! | [`hub`] | The real hub in the process of the test: test databases from a template, an installation, a hub started with short deadlines, and the admin steps (Hermes, invite, approval). |
//! | [`hermes`] | A fake Hermes: the program that records the registration of the connector, and the client of MCP that starts copies of the connector with a filtered environment, talks to them, kills and closes them. |
//! | [`passkey`] | A passkey authenticator in software: what a browser and a device answer in the two ceremonies of the panel, with knobs for the answers the hub must refuse. |
//! | [`engine`] | A fake WhatsApp engine in the format of the WAHA: sessions, pairing, chats, sending, signed events, and failures on command. |
#![forbid(unsafe_code)]

pub mod connector;
pub mod device;
pub mod engine;
pub mod hermes;
pub mod hostile;
pub mod http;
pub mod hub;
pub mod load;
pub mod passkey;

use std::time::Duration;

use arauto_protocol::errors::ErrorCode;
use arauto_protocol::frame::{FrameError, SplitError};
use arauto_protocol::message::{DecodeError, EncodeError};
use arauto_protocol::noise::NoiseError;
use arauto_protocol::secret::RandomError;

pub use connector::{RawSocket, TestConnector};
pub use hostile::Hostile;

/// How the hub ended a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ended {
    /// The code of the `closing` the hub sent before closing, if it sent one.
    pub closing: Option<ErrorCode>,
    /// The time from the opening of the websocket (or of the TCP connection) to the end.
    pub after: Duration,
}

/// What went wrong in a test tool.
#[derive(Debug, thiserror::Error)]
pub enum KitError {
    /// The network.
    #[error("input or output failed: {0}")]
    Io(#[from] std::io::Error),
    /// The websocket library.
    #[error("websocket: {0}")]
    Ws(#[from] Box<tokio_tungstenite::tungstenite::Error>),
    /// The hub answered the upgrade with this status instead of 101.
    #[error("the hub answered the upgrade with HTTP {0}")]
    Refused(u16),
    /// The Noise layer.
    #[error("noise: {0}")]
    Noise(#[from] NoiseError),
    /// A frame the hub should not have sent.
    #[error("frame: {0}")]
    Frame(#[from] FrameError),
    /// A message that could not be cut in frames.
    #[error("split: {0}")]
    Split(#[from] SplitError),
    /// A message that could not be written.
    #[error("encode: {0}")]
    Encode(#[from] EncodeError),
    /// A message the hub should not have sent.
    #[error("decode: {0}")]
    Decode(#[from] DecodeError),
    /// The generator of the system.
    #[error("random: {0}")]
    Random(#[from] RandomError),
    /// Nothing came in time.
    #[error("nothing came in time")]
    TimedOut,
    /// The hub ended the connection.
    #[error("the hub ended the connection ({0:?})")]
    Ended(Ended),
    /// Something the test did not expect.
    #[error("unexpected: {0}")]
    Unexpected(String),
}

impl From<tokio_tungstenite::tungstenite::Error> for KitError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        KitError::Ws(Box::new(error))
    }
}
