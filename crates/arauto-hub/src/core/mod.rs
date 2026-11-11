//! The core of the connection: what a device is and how its credential is swapped.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`model`] | The rows (Hermes, device, invite), their states, and the `AR-` code each state answers with. |
//! | [`judge`] | The rule of the credential swap as pure functions: no database, no network, no clock. |
//! | [`swap_model`] | An in-memory simulator of the whole swap between connectors and the hub, built on [`judge`], for random rounds. |

pub mod judge;
pub mod model;
pub mod swap_model;
