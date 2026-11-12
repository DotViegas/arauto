//! The Arauto connector: the program beside a Hermes. It keeps one channel to the hub and, as
//! the server of tools that Hermes starts, answers its calls (`docs/11-conector.md`).
//!
//! | Piece | What it holds |
//! |---|---|
//! | [`Connector`] | The connector of a process, leader of its folder or a copy that forwards to the leader: calls, tools, status. |
//! | [`join()`], [`leave()`], [`report()`] | The commands `join`, `leave` and `status`, as functions. |
//! | [`cli`] | The command line, its fixed texts and its exit codes; `mcp` serves MCP over stdio, in the versions of [`PROTOCOL_VERSIONS`]. |
//! | [`catalog`] | What the connector says: the codes of the catalog and the local problems. |
//! | [`Home`], [`HubAddress`], [`Hermes`], [`Settings`] | The folder, the address of the hub, the command of Hermes, the waits. |
//! | `testing` (feature) | Faults on command, events and short deadlines, for the tests of this crate. |
//!
//! Inside: `link` (the connection, the handshake, the frames and their limits), `attempt` (one
//! attempt of the channel: the swap, the wait for approval, the presentation without swap),
//! `session` (the calls and the 24-hour swap), `backoff` (what follows an attempt), `leader`
//! (the slots of the device in use and of the request, the switch of device, the cleaning, the
//! local socket), `relay` (the copy that forwards), `local` (the messages of the local
//! socket), `store` (the files), `short` (the enrollment and the request to leave), `mcp` (the
//! server of tools Hermes talks to) and `rpc` (JSON-RPC 2.0 over lines, for it).
#![forbid(unsafe_code)]

mod address;
mod ask;
mod attempt;
mod backoff;
pub mod catalog;
pub mod cli;
mod hermes;
mod home;
mod join;
mod leader;
mod leave;
mod link;
mod local;
mod mcp;
mod relay;
mod report;
mod rpc;
mod runtime;
mod session;
mod settings;
mod short;
mod status;
mod store;
#[cfg(feature = "testing")]
pub mod testing;
mod writes;

pub use address::{AddressError, CHANNEL_PATH, HubAddress};
pub use hermes::{DEFAULT_PROGRAM, Hermes, SERVER_NAME};
pub use home::{Home, HomeError, PROGRAM_NAME};
pub use join::{JoinArgs, Joined, join};
pub use leave::{Left, leave};
pub use mcp::{PROTOCOL_VERSIONS, SERVER_INFO_NAME};
pub use report::{FolderReport, Report, render, report};
pub use runtime::{Connector, StartError};
pub use settings::Settings;
pub use status::{ChannelStatus, DeviceStatus, Role, Status};
pub use store::{DeviceId, Situation, StoreError};
