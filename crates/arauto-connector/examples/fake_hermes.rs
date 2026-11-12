//! The fake command of Hermes that `ARAUTO_HERMES_BIN` points to in the tests of the connector:
//! it records the registration (`arauto_testkit::hermes::registry`).
//!
//! It is an example so that `cargo test` builds it beside the tests of this package. It is
//! never part of the program that is shipped: the program has no code of the tests, and does
//! not know this one exists.
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    arauto_testkit::hermes::registry::main()
}
