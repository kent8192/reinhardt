//! Validator integration test fixtures
//!
//! This module provides rstest fixtures for validator integration tests,
//! including PostgreSQL database setup with proper connection pool configuration
//! and automatic cleanup.

#[cfg(feature = "testcontainers")]
use rstest::*;
#[cfg(feature = "testcontainers")]
use std::sync::Arc;
#[cfg(feature = "testcontainers")]
use testcontainers::{ContainerAsync, GenericImage, ImageExt, core::WaitFor, runners::AsyncRunner};

#[cfg(feature = "testcontainers")]
use crate::resource::{TeardownGuard, TestResource};

/// Fixture providing a PostgreSQL container with optimized connection pool for validator tests
///
/// This fixture provides a PostgreSQL 17 Alpine container with a connection pool
/// configured specifically for validator integration tests:
/// - max_connections: 10 (increased from default 5 to prevent pool exhaustion)
/// - acquire_timeout: 20 seconds (increased from default 10s)
/// - idle_timeout: 30 seconds
/// - max_lifetime: 300 seconds (5 minutes)
///
/// # Returns
/// Tuple of (container, connection_pool, port, database_url)
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::validator::validator_test_db;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_validator_db(
///     #[future] validator_test_db: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String)
/// ) {
///     let (_container, pool, port, url) = validator_test_db.await;
///     // Test code
///     // Automatically cleaned up
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[fixture]
pub async fn validator_test_db() -> (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String) {
	let postgres = GenericImage::new("postgres", "17-alpine")
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_wait_for(WaitFor::seconds(2)) // Additional wait time
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.with_env_var("POSTGRES_USER", "postgres")
		.with_env_var("POSTGRES_DB", "test_db")
		.start()
		.await
		.expect("Failed to start PostgreSQL container for validator tests");

	let port = postgres
		.get_host_port_ipv4(5432)
		.await
		.expect("Failed to get PostgreSQL port");

	let database_url = format!("postgres://postgres@localhost:{}/test_db", port);

	// Connect with retry mechanism
	let mut retry_count = 0;
	let max_retries = 3;
	let pool = loop {
		match sqlx::postgres::PgPoolOptions::new()
			.max_connections(10) // Increased: 5 → 10
			.min_connections(1)
			.acquire_timeout(std::time::Duration::from_secs(20)) // Increased: 10s → 20s
			.idle_timeout(Some(std::time::Duration::from_secs(30)))
			.max_lifetime(Some(std::time::Duration::from_secs(300)))
			.connect(&database_url)
			.await
		{
			Ok(pool) => break pool,
			Err(e) if retry_count < max_retries => {
				retry_count += 1;
				eprintln!(
					"Failed to connect to PostgreSQL (attempt {}/{}): {}",
					retry_count, max_retries, e
				);
				tokio::time::sleep(std::time::Duration::from_secs(1)).await;
			}
			Err(e) => panic!(
				"Failed to connect to PostgreSQL after {} retries: {}",
				max_retries, e
			),
		}
	};

	(postgres, Arc::new(pool), port, database_url)
}

/// Validator database cleanup guard
///
/// This guard ensures that database connections are properly released
/// even if tests panic or fail.
#[cfg(feature = "testcontainers")]
pub struct ValidatorDbGuard;

#[cfg(feature = "testcontainers")]
impl TestResource for ValidatorDbGuard {
	fn setup() -> Self {
		// Add initialization if needed
		Self
	}

	fn teardown(&mut self) {
		// Pool connections are automatically cleaned up,
		// implement additional cleanup here if needed
	}
}

/// Fixture providing validator database cleanup guard
///
/// Use this fixture to ensure proper cleanup of database resources.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_testkit::fixtures::validator::validator_db_guard;
/// use reinhardt_testkit::resource::TeardownGuard;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_cleanup(
///     _validator_db_guard: TeardownGuard<ValidatorDbGuard>,
/// ) {
///     // Test code
///     // Automatically cleaned up even on panic
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[fixture]
pub fn validator_db_guard() -> TeardownGuard<ValidatorDbGuard> {
	TeardownGuard::new()
}
