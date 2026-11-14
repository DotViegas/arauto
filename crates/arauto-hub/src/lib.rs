//! The Arauto hub.
//!
//! The foundation, the repositories, the admin command and the transport of the channel. It
//! follows `docs/04-seguranca-conexao.md` (C7, C9 and C11), `docs/14-protocolo-do-canal.md`
//! and `docs/15-mvp.md` (sections 2 and 3).
//!
//! | Module | What it holds |
//! |---|---|
//! | [`config`] | The environment variables, read and checked; each command asks only for what it uses. |
//! | [`db`] | The connection pool, the durability check, the migrations, the resolved organization and the durable transaction. |
//! | [`db_password`] | The password of the database role, drawn by the hub and kept in the data folder. |
//! | [`vault`] | The black box: the only module that touches the master key. Seals and opens credentials, keys and message texts. |
//! | [`mark`] | The audit mark, the number kept outside the database that tells a restored database apart. |
//! | [`audit`] | The closed catalog of actions and the two ways of writing an event. |
//! | [`restore`] | The restored mode of C11. |
//! | [`core`] | The row types, the pure rule of the credential swap and its in-memory simulator. |
//! | [`admin`] | The admin command: `init`, `migrate`, `status` and what a person does to Hermes, invites and requests. |
//! | [`repo`] | The repositories: every read and write of business data, with the organization resolved. |
//! | [`engine`] | The WhatsApp engine: its interface, the adapter of the WAHA, the inbox of its events and its supervision. |
//! | [`whatsapp`] | The WhatsApp tools answered in a session: the check of the matrix at every call, the signed identifiers of messages and the send limit. |
//! | [`channel`] | The port of the connectors: the accept loop, the websocket, the handshake, the frames and their limits, `hello`, `leave`, and the entry point of the other purposes. |
//! | [`first_access`] | The block with the start code that the hub writes in its own log until the first passkey is registered. |
//! | [`logging`] | How the lines of the log are written: colors only on a terminal. |
//! | [`panel`] | The panel of the admin, on a port of its own and off by default: the entry by a code of one use, the session, and the pages that show what the admin command shows. |
//!
//! Time is always an `i64` of milliseconds given by the caller, who reads it from an
//! [`arauto_protocol::clock::Clock`]. No SQL here reads the clock of the database.
#![forbid(unsafe_code)]

pub mod admin;
pub mod audit;
pub mod channel;
pub mod check;
pub mod config;
pub mod core;
pub mod db;
pub mod db_password;
pub mod download;
pub mod engine;
pub mod first_access;
pub mod join_page;
pub mod logging;
pub mod mark;
pub mod panel;
mod peer;
pub mod repo;
pub mod restore;
pub mod vault;
pub mod whatsapp;

mod lock_wait;
