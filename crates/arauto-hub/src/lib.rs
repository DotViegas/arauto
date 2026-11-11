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
//! | [`vault`] | The black box: the only module that touches the master key. Seals and opens credentials, keys and message texts. |
//! | [`mark`] | The audit mark, the number kept outside the database that tells a restored database apart. |
//! | [`audit`] | The closed catalog of actions and the two ways of writing an event. |
//! | [`restore`] | The restored mode of C11. |
//! | [`core`] | The row types, the pure rule of the credential swap and its in-memory simulator. |
//! | [`admin`] | The admin command: `init`, `migrate`, `status` and what a person does to Hermes, invites and requests. |
//! | [`repo`] | The repositories: every read and write of business data, with the organization resolved. |
//! | [`channel`] | The port of the connectors: the accept loop, the websocket, the handshake, the frames and their limits, `hello`, `leave`, and the entry point of the other purposes. |
//!
//! Time is always an `i64` of milliseconds given by the caller, who reads it from an
//! [`arauto_protocol::clock::Clock`]. No SQL here reads the clock of the database.
#![forbid(unsafe_code)]

pub mod admin;
pub mod audit;
pub mod channel;
pub mod config;
pub mod core;
pub mod db;
pub mod mark;
pub mod repo;
pub mod restore;
pub mod vault;

mod lock_wait;
