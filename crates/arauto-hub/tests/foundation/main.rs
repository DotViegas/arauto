//! The foundation of the hub against a real PostgreSQL: the pool, the migrations, the audit,
//! the mark and the restored mode.
//!
//! These tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use arauto_testkit::hub::db as common;

mod audit;
mod database;
mod restore;
mod rows;
