//! The repositories and the admin command against a real PostgreSQL, with a manual clock.
//!
//! These tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../common/mod.rs"]
mod common;

mod support;

mod admin;
mod approval;
mod closing;
mod enrollment;
mod hermes;
mod rounds;
mod scan;
mod swap;
mod sweep;
