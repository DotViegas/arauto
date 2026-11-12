//! The connector against the hub running in this process and the test PostgreSQL: the join and
//! each of its cases, the leave and each of its cases, the channel (reconnection, quarantine,
//! revocation, the 24-hour swap, the drops at each point of the swap, the copies), the leader and
//! the copies that forward to it, the local socket, and the rounds drawn from a seed.
//!
//! These tests need `ARAUTO_TEST_DATABASE_URL`; without it they fail and say so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

mod channel;
mod enrollment;
mod leaving;
mod local_socket;
mod rounds;
mod swap;
mod waking;
