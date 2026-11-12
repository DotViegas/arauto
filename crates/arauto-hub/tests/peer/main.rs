//! The conversation between Hermes (`docs/13-conversa-entre-hermes.md`), with the real hub in
//! the process of the test, the test PostgreSQL and the test connector in session: the list of
//! peers, the threads and the messages, the ceilings, the queue of notices and the list of
//! tools that follows the links.
//!
//! The tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod world;

mod admin;
mod ceilings;
mod messages;
mod notices;
mod tools;
