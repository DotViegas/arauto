//! The engine and the numbers, against the fake engine of the kit and, where a hub is needed,
//! the test PostgreSQL: each operation of the adapter of the WAHA; hostile answers of the
//! engine; malicious identifiers refused before any request reaches the engine; the inbox of
//! events on the internal port; the states of a number; the supervision; the admin commands.
//!
//! Never a real engine and never a real number. The tests that need a hub need
//! `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[allow(dead_code, unused_imports)]
#[path = "../channel/support.rs"]
mod support;

use arauto_testkit::hub::fixture;

mod world;

mod agent;

mod access;
mod adapter;
mod admin;
mod ceiling;
mod chats;
mod hostile;
mod inbox;
mod isolation;
mod numbers;
mod supervision;
mod tools;
