//! Basic GraphQL functionality tests
//!
//! This module contains tests for basic GraphQL operations:
//! - Query execution (happy path, error path)
//! - Mutation execution
//! - Schema validation
//! - Variables and aliases

// Re-export test modules
pub mod mutation;
pub mod query;
pub mod schema_validation;
pub mod variables_aliases;

// Re-export fixtures for convenience
pub use crate::graphql::fixtures::*;
