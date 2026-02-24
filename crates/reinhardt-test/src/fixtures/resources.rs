//! Suite-wide shared resources using `resource.rs` SuiteResource pattern
//!
//! This module provides TestContainers-based suite resources that are shared
//! across all tests in a suite and automatically cleaned up when the last test
//! completes.

#[cfg(feature = "testcontainers")]
use rstest::*;
#[cfg(feature = "testcontainers")]
use std::sync::{Mutex, OnceLock, Weak};

#[cfg(feature = "testcontainers")]
use crate::resource::{SuiteGuard, SuiteResource, acquire_suite};

#[cfg(feature = "testcontainers")]
use testcontainers::core::WaitFor;

// ============================================================================
// PostgreSQL Suite Resource
// ============================================================================

/// Suite-wide PostgreSQL container resource
///
/// This resource is shared across all tests in the suite and automatically
/// cleaned up when the last test completes. Uses `SuiteResource` pattern
/// from `resource.rs` for safe lifecycle management.
///
/// ## Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_database_query(postgres_suite: SuiteGuard<PostgresSuiteResource>) {
///     let pool = &postgres_suite.pool;
///     let result = sqlx::query("SELECT 1").fetch_one(pool).await;
///     assert!(result.is_ok());
/// }
/// ```
#[cfg(feature = "testcontainers")]
pub struct PostgresSuiteResource {
	// Note: Container must be held to keep it alive during test suite execution
	// TestContainers automatically stops/removes containers when dropped
	#[allow(dead_code)]
	pub container: testcontainers::ContainerAsync<testcontainers::GenericImage>,
	pub pool: sqlx::postgres::PgPool,
	pub port: u16,
	pub database_url: String,
}

#[cfg(feature = "testcontainers")]
impl SuiteResource for PostgresSuiteResource {
	fn init() -> Self {
		// Block on async initialization (SuiteResource::init is sync)
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current().block_on(async { Self::init_async().await })
		})
	}
}

#[cfg(feature = "testcontainers")]
impl PostgresSuiteResource {
	async fn init_async() -> Self {
		use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

		let postgres = GenericImage::new("postgres", "17-alpine")
			.with_wait_for(WaitFor::message_on_stderr(
				"database system is ready to accept connections",
			))
			.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
			.start()
			.await
			.expect("Failed to start PostgreSQL container");

		let port = postgres
			.get_host_port_ipv4(5432)
			.await
			.expect("Failed to get PostgreSQL port");

		let database_url = format!("postgres://postgres@localhost:{}/postgres", port);

		// Retry connection with exponential backoff
		let pool = {
			use sqlx::postgres::PgPoolOptions;
			use std::time::Duration;

			const MAX_RETRIES: u32 = 10;
			let mut pool_result = None;

			for attempt in 0..MAX_RETRIES {
				match PgPoolOptions::new()
					.max_connections(5)
					.acquire_timeout(Duration::from_secs(3))
					.test_before_acquire(false) // sqlx v0.7+ bug workaround (issue #2885, #3241)
					.connect(&database_url)
					.await
				{
					Ok(pool) => {
						pool_result = Some(pool);
						break;
					}
					Err(e) if attempt < MAX_RETRIES - 1 => {
						eprintln!(
							"Connection attempt {} failed: {}. Retrying...",
							attempt + 1,
							e
						);
					}
					Err(e) => panic!(
						"Failed to connect to PostgreSQL after {} retries: {}",
						MAX_RETRIES, e
					),
				}
			}

			pool_result.expect("Pool should be initialized")
		};

		Self {
			container: postgres,
			pool,
			port,
			database_url,
		}
	}
}

#[cfg(feature = "testcontainers")]
static POSTGRES_SUITE: OnceLock<Mutex<Weak<PostgresSuiteResource>>> = OnceLock::new();

/// Acquire shared PostgreSQL suite resource
///
/// This fixture provides a suite-wide PostgreSQL container that is shared
/// across all tests and automatically cleaned up when the last test completes.
///
/// ## Example
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_example(postgres_suite: SuiteGuard<PostgresSuiteResource>) {
///     let pool = &postgres_suite.pool;
///     // Use pool in test
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[fixture]
pub fn postgres_suite() -> SuiteGuard<PostgresSuiteResource> {
	acquire_suite(&POSTGRES_SUITE)
}

// ============================================================================
// MySQL Suite Resource
// ============================================================================

/// Suite-wide MySQL container resource
#[cfg(feature = "testcontainers")]
pub struct MySqlSuiteResource {
	// Note: Container must be held to keep it alive during test suite execution
	// TestContainers automatically stops/removes containers when dropped
	#[allow(dead_code)]
	pub container: testcontainers::ContainerAsync<testcontainers::GenericImage>,
	pub pool: sqlx::mysql::MySqlPool,
	pub port: u16,
	pub database_url: String,
}

#[cfg(feature = "testcontainers")]
impl SuiteResource for MySqlSuiteResource {
	fn init() -> Self {
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current().block_on(async { Self::init_async().await })
		})
	}
}

#[cfg(feature = "testcontainers")]
impl MySqlSuiteResource {
	async fn init_async() -> Self {
		use testcontainers::{
			GenericImage, ImageExt, core::IntoContainerPort, runners::AsyncRunner,
		};

		let mysql = GenericImage::new("mysql", "8.0")
			.with_exposed_port(3306.tcp())
			.with_wait_for(WaitFor::message_on_stderr("ready for connections"))
			.with_env_var("MYSQL_ROOT_PASSWORD", "test")
			.with_env_var("MYSQL_DATABASE", "test")
			.start()
			.await
			.expect("Failed to start MySQL container");

		let port = mysql
			.get_host_port_ipv4(3306)
			.await
			.expect("Failed to get MySQL port");

		let database_url = format!("mysql://root:test@localhost:{}/test", port);

		// Retry connection with exponential backoff
		let pool = {
			use sqlx::mysql::MySqlPoolOptions;
			use std::time::Duration;

			const MAX_RETRIES: u32 = 10;
			let mut pool_result = None;

			for attempt in 0..MAX_RETRIES {
				match MySqlPoolOptions::new()
					.max_connections(5)
					.acquire_timeout(Duration::from_secs(3))
					.test_before_acquire(false) // sqlx v0.7+ bug workaround (issue #2885, #3241)
					.connect(&database_url)
					.await
				{
					Ok(pool) => {
						pool_result = Some(pool);
						break;
					}
					Err(e) if attempt < MAX_RETRIES - 1 => {
						eprintln!(
							"Connection attempt {} failed: {}. Retrying...",
							attempt + 1,
							e
						);
					}
					Err(e) => panic!(
						"Failed to connect to MySQL after {} retries: {}",
						MAX_RETRIES, e
					),
				}
			}

			pool_result.expect("Pool should be initialized")
		};

		Self {
			container: mysql,
			pool,
			port,
			database_url,
		}
	}
}

#[cfg(feature = "testcontainers")]
static MYSQL_SUITE: OnceLock<Mutex<Weak<MySqlSuiteResource>>> = OnceLock::new();

/// Acquire shared MySQL suite resource
#[cfg(feature = "testcontainers")]
#[fixture]
pub fn mysql_suite() -> SuiteGuard<MySqlSuiteResource> {
	acquire_suite(&MYSQL_SUITE)
}
