//! The channel, against the hub running in this process and the test PostgreSQL. The
//! transport: every abuse of the hostile client, the deadlines before the session, the
//! ceilings, the backpressure, the reassembly budget, the stop, the two ports, `leave`, and
//! the start over a restored database. The purposes of the hub: enrollment, the wait, the
//! swap, the session, the races and the copies, the restored mode, the rounds drawn from a
//! seed, and the return in mass. The task of the purge, as the running hub does it.
//!
//! These tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use arauto_testkit::hub::db as common;
use arauto_testkit::hub::fixture;

mod support;

mod abuse;
mod ceilings;
mod copies;
mod deadlines;
mod join_page;
mod leave;
mod mass;
mod ports;
mod pressure;
mod purge;
mod restored;
mod rounds;
mod served;
mod start_stop;
