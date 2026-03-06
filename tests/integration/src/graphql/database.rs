//! Database integration tests for GraphQL
//!
//! This module contains tests for GraphQL with real database integration
//! using PostgreSQL via TestContainers and reinhardt-query for SQL construction.

// Re-export test modules
pub mod models;
pub mod relationships;
pub mod resolvers;
pub mod transactions;

// Re-export fixtures for convenience
pub use crate::graphql::fixtures::*;

/// Database-specific fixtures module
pub mod fixtures {
	// Database-specific fixtures will be defined here
}
