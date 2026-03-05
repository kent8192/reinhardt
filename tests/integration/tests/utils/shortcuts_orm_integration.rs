//! Integration tests for ORM-integrated shortcut functions
//!
//! These tests verify that get_object_or_404 and get_list_or_404 work correctly
//! with real PostgreSQL database.

use reinhardt_db::prelude::QuerySet;
use reinhardt_shortcuts::{get_list_or_404, get_object_or_404};
use reinhardt_test::resource::{AsyncTeardownGuard, AsyncTestResource};
use rstest::*;
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::sync::Arc;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestUser {
	id: Option<i64>,
	username: String,
	email: String,
}

reinhardt_test::impl_test_model!(TestUser, i64, "test_users");

/// Suite-wide PostgreSQL database resource
struct PostgresSuite {
	_container: Arc<ContainerAsync<Postgres>>,
	url: String,
}

/// Global suite instance (shared across all tests)
static POSTGRES_SUITE: tokio::sync::OnceCell<Arc<PostgresSuite>> =
	tokio::sync::OnceCell::const_new();

/// Async fixture to initialize and get PostgreSQL suite
#[fixture]
async fn postgres_suite() -> Arc<PostgresSuite> {
	POSTGRES_SUITE
		.get_or_init(|| async {
			// Start PostgreSQL container
			let container = Postgres::default()
				.start()
				.await
				.expect("Failed to start PostgreSQL container");

			let port = container
				.get_host_port_ipv4(5432)
				.await
				.expect("Failed to get PostgreSQL port");

			let url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);

			// Set larger connection pool for tests to prevent pool exhaustion
			// SAFETY: Setting environment variable before database initialization
			// This is only called once during test suite setup, before any other threads access the environment
			unsafe {
				std::env::set_var("DATABASE_POOL_MAX_CONNECTIONS", "100");
			}

			// Initialize global database connection
			reinhardt_db::prelude::init_database(&url)
				.await
				.expect("Failed to initialize database");

			// Create test_users table
			let conn = reinhardt_db::prelude::get_connection()
				.await
				.expect("Failed to get connection");
			conn.execute(
				"CREATE TABLE IF NOT EXISTS test_users (
					id SERIAL PRIMARY KEY,
					username VARCHAR(255) NOT NULL,
					email VARCHAR(255) NOT NULL
				)",
				vec![],
			)
			.await
			.expect("Failed to create test_users table");

			Arc::new(PostgresSuite {
				_container: Arc::new(container),
				url: url.clone(),
			})
		})
		.await
		.clone()
}

/// Resource for connection pool cleanup after each test
struct ConnectionPoolCleanup;

#[async_trait::async_trait]
impl AsyncTestResource for ConnectionPoolCleanup {
	async fn setup() -> Self {
		Self
	}

	async fn teardown(self) {
		// Force connection release by getting and immediately dropping a connection
		// This ensures any Arc references are decremented
		if let Ok(_conn) = reinhardt_db::prelude::get_connection().await {
			drop(_conn);
		}

		// Allow time for sqlx pool to process the release
		tokio::time::sleep(std::time::Duration::from_millis(200)).await;
	}
}

/// Fixture for automatic connection pool cleanup
#[fixture]
async fn pool_cleanup() -> AsyncTeardownGuard<ConnectionPoolCleanup> {
	AsyncTeardownGuard::new().await
}

#[rstest]
#[serial(db)]
#[tokio::test(flavor = "multi_thread")]
async fn test_get_object_or_404_not_found(
	#[future] postgres_suite: Arc<PostgresSuite>,
	#[future] _pool_cleanup: AsyncTeardownGuard<ConnectionPoolCleanup>,
) {
	// Initialize suite (awaits the fixture)
	let suite = postgres_suite.await;
	let _cleanup = _pool_cleanup.await;

	// Reinitialize database connection pool for this test
	reinhardt_db::prelude::reinitialize_database(&suite.url)
		.await
		.expect("Failed to reinitialize database");

	// Clean test data before test
	// clean_test_data()
	// 	.await
	// 	.expect("Failed to clean test data");

	// Query for non-existent record
	let result = get_object_or_404::<TestUser>(999).await;
	assert!(result.is_err());

	let response = result.unwrap_err();
	// Debug: print response body if not 404
	if response.status != hyper::StatusCode::NOT_FOUND {
		let body_str = String::from_utf8_lossy(&response.body);
		eprintln!("Expected 404, got {}: {}", response.status, body_str);
	}
	assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);

	// Cleanup automatically called by AsyncTeardownGuard's Drop (via async-dropper)
}

#[rstest]
#[serial(db)]
#[tokio::test(flavor = "multi_thread")]
async fn test_get_list_or_404_empty(
	#[future] postgres_suite: Arc<PostgresSuite>,
	#[future] _pool_cleanup: AsyncTeardownGuard<ConnectionPoolCleanup>,
) {
	// Initialize suite (awaits the fixture)
	let suite = postgres_suite.await;
	let _cleanup = _pool_cleanup.await;

	// Reinitialize database connection pool for this test
	reinhardt_db::prelude::reinitialize_database(&suite.url)
		.await
		.expect("Failed to reinitialize database");

	// Clean test data before test
	// clean_test_data()
	// 	.await
	// 	.expect("Failed to clean test data");

	// Query empty table
	let queryset = QuerySet::<TestUser>::new();
	let result = get_list_or_404(queryset).await;
	assert!(result.is_err());

	let response = result.unwrap_err();
	assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);

	// Cleanup automatically called by AsyncTeardownGuard's Drop (via async-dropper)
}
