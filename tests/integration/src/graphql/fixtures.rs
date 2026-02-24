//! GraphQL test fixtures
//!
//! This module provides specialized fixtures for GraphQL testing that wrap
//! generic `reinhardt-test` fixtures with GraphQL-specific functionality.

// Re-export all fixture modules
pub mod database;
pub mod models;
pub mod schema;
pub mod server;

// Re-export individual fixtures for easy access
pub use database::*;
pub use models::*;
pub use schema::*;
pub use server::*;

/// Common imports for fixture implementations
mod prelude {
	pub use reinhardt_graphql::*;
	pub use reinhardt_test::fixtures::*;
	pub use rstest::{fixture, rstest};
	pub use sqlx::PgPool;
	pub use std::sync::Arc;
	pub use testcontainers::{ContainerAsync, GenericImage};
}
