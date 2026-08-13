//! Settings compile-time integration tests.
//!
//! This standalone test target keeps trybuild cases on the dedicated UI-test
//! profile instead of the default cross-crate integration-test profile.

#[path = "settings/schema_trybuild.rs"]
mod schema_trybuild;
