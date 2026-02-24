//! DCL Integration Test Module
//!
//! Integration tests for DCL statements using testcontainers-rs.
//!
//! ## Test Organization
//!
//! - **PostgreSQL Tests**: Full DCL support testing
//! - **MySQL Tests**: MySQL-specific feature testing
//! - **CockroachDB Tests**: PostgreSQL compatibility testing

mod postgres_tests;
mod mysql_tests;
mod cockroachdb_tests;

// Re-exports for convenience
pub use postgres_tests::*;
pub use mysql_tests::*;
pub use cockroachdb_tests::*;
