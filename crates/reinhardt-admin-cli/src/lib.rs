//! Library entry point for `reinhardt-admin-cli`.
//!
//! Exposes internal modules so they can be invoked from integration tests
//! and (eventually) from other tooling. The actual command-line entry
//! point lives in `src/main.rs`.

#![warn(missing_docs)]

pub mod migrate_v2;
