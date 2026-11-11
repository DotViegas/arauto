//! Shared protocol of the Arauto hub and connector.
//!
//! This crate is what the two programs have in common: frames, messages, the Noise handshake,
//! the invite, the verification code, name formats, limits and deadlines, and the catalogs of
//! errors and tools. It follows `docs/14-protocolo-do-canal.md`.
//!
//! It is sans-IO: no async, no network and no disk. There are only types and pure state
//! machines, which receive bytes and the time (an `i64` of milliseconds) as arguments.
//!
//! Everything that arrives from the other side is hostile until checked. No function here
//! panics on input, allocates by a size the other side announced, or repeats a refused value
//! in an error text.
//!
//! # The path of a message
//!
//! ```text
//! write   Message -> message::encode -> frame::Splitter -> noise::Sealer -> websocket
//! read    websocket -> noise::Opener -> frame::Reassembler -> message::decode -> Message
//! ```
//!
//! # Modules
//!
//! | Module | What it holds |
//! |---|---|
//! | [`limits`] | Sizes, counts and deadlines; [`limits::Deadlines`] for tests to shrink. |
//! | [`clock`] | [`clock::Clock`], the system clock, and test helpers behind `testing`. |
//! | [`secret`] | Types that hold a secret: wiped on drop, never printed. |
//! | [`names`] | The narrow formats: Hermes name, number alias, chat identifier, text. |
//! | [`errors`] | The `AR-` codes and the fixed phrase of each one. |
//! | [`frame`] | The frame header, the splitter, and the reassembler with every rule. |
//! | [`message`] | Every message of the channel, with strict reading and writing. |
//! | [`noise`] | The Noise XK handshake and the two halves of the transport. |
//! | [`invite`] | The invite, its strict text form and its digest. |
//! | [`code`] | The verification code. |
//! | [`digest`] | The digest of a sealed credential. |
//! | [`notice`] | The check of a notice and the fixed phrase that wakes the Hermes. |
//! | [`tools`] | The catalog of tools and the check of each call. |
#![forbid(unsafe_code)]

#[cfg(test)]
#[macro_use]
mod test_util;

#[macro_use]
mod base64;
#[macro_use]
mod serde_util;

pub mod clock;
pub mod code;
pub mod digest;
pub mod errors;
pub mod frame;
pub mod invite;
pub mod limits;
pub mod message;
pub mod names;
pub mod noise;
pub mod notice;
pub mod secret;
pub mod tools;

#[cfg(test)]
mod canary;
#[cfg(test)]
mod pipeline;
