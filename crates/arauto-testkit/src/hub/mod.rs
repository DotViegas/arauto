//! The real hub, in the process of the test, with the test PostgreSQL.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`db`] | The test databases (a template migrated once, one of its own per test that needs it, one shared), the data folder of a test, and rows written by hand. |
//! | [`support`] | One installation shared by the whole process, the settings and the configuration of a hub of the test, and a hub started with the purposes a test brings. |
//! | [`fixture`] | A hub of its own with the purposes of the hub, the admin session on the same database, and the steps of the admin as functions. |
//!
//! Every test that uses this module needs `ARAUTO_TEST_DATABASE_URL`: without it, it fails and
//! says so.

pub mod db;
pub mod fixture;
pub mod support;
