//! GraphQL integration tests for Reinhardt framework
//!
//! This module provides comprehensive GraphQL testing covering:
//! - Basic query/mutation execution
//! - Database integration with real PostgreSQL
//! - Dependency injection (DI) integration
//! - Real-time subscriptions
//! - GraphQL over gRPC
//! - Advanced testing techniques (fuzz, property-based, combinatorial)

// Always public modules
pub mod advanced;
pub mod basic;
pub mod database;

// Conditionally compiled modules based on feature flags
#[cfg(feature = "di")]
pub mod di;

#[cfg(feature = "subscription")]
pub mod subscription;

#[cfg(feature = "graphql-grpc")]
pub mod grpc;

// Re-export commonly used fixtures and helpers
pub use fixtures::*;

/// Test utilities module
pub mod utils {
	// Common test utilities will be placed here
}

/// Fixtures module - contains specialized fixtures for GraphQL testing
pub mod fixtures {
	// Re-export all fixtures for easy access
	pub use super::basic::fixtures::*;
	pub use super::database::fixtures::*;

	#[cfg(feature = "di")]
	pub use super::di::fixtures::*;

	#[cfg(feature = "subscription")]
	pub use super::subscription::fixtures::*;

	#[cfg(feature = "graphql-grpc")]
	pub use super::grpc::fixtures::*;
}

/// Prelude for GraphQL testing
pub mod prelude {
	pub use reinhardt_graphql::*;
	pub use reinhardt_test::fixtures::*;
	pub use rstest::*;
	pub use serde_json::json;
	pub use sqlx::PgPool;
	pub use testcontainers::{ContainerAsync, GenericImage};

	// Re-export our fixtures
	pub use super::fixtures::*;

	/// Common test assertions for GraphQL responses
	pub mod assertions {
		use async_graphql::{Response, ServerResult};
		use serde_json::Value;

		/// Assert that a GraphQL response is successful
		pub fn assert_graphql_success(response: &Response) {
			assert!(
				response.is_ok(),
				"GraphQL response should be successful: {:?}",
				response
			);
		}

		/// Assert that a GraphQL response contains expected data field
		pub fn assert_graphql_data_contains(response: &Response, path: &str, expected: &Value) {
			let data = response.data.clone().into_json().unwrap();
			let actual = data.pointer(path).unwrap_or(&Value::Null);
			assert_eq!(actual, expected, "GraphQL data at path '{}' mismatch", path);
		}

		/// Extract field from GraphQL response
		pub fn extract_field(response: &Response, path: &str) -> Value {
			let data = response.data.clone().into_json().unwrap();
			data.pointer(path).unwrap_or(&Value::Null).clone()
		}
	}
}
