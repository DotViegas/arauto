//! The core: what a device is and how its credential is swapped, and the states of a number.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`model`] | The rows (Hermes, device, invite), their states, and the `AR-` code each state answers with. |
//! | [`judge`] | The rule of the credential swap as pure functions: no database, no network, no clock. |
//! | [`swap_model`] | An in-memory simulator of the whole swap between connectors and the hub, built on [`judge`], for random rounds. |
//! | [`number`] | The states of a WhatsApp number and what moves it from one to another, as a pure function. |
//! | [`org`] | The org chart of the Hermes, stage A: levels, places, roles, the draft and its version. Read by no part of the channel. |
//! | [`reconnect`] | Why an invite is issued, and the standing of the connector of a device, as pure functions. |
//! | [`access`] | The rule of the matrix of accesses: what an access and the line of a chat allow, and the code of each refusal. |

pub mod access;
pub mod judge;
pub mod model;
pub mod number;
pub mod org;
pub mod reconnect;
pub mod swap_model;
