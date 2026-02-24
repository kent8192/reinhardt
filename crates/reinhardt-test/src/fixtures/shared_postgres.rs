//! Shared PostgreSQL Container with Template Database Pattern
//!
//! This module provides a PostgreSQL database for tests using TestContainers.
//!
//! ## Architecture
//!
//! For nextest's process-per-test model, file-based coordination ensures:
//! - First process starts the container and writes URL to a shared file
//! - Subsequent processes read the URL from the shared file
//! - Template database enables fast test isolation (~10-40ms per clone)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use reinhardt_test::fixtures::get_test_pool;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_with_postgres() {
//!     let pool = get_test_pool().await;
//!     // Each test gets its own isolated database
//! }
//! ```
//!
//! ## Configuration
//!
//! Ryuk sidecar is disabled via `.testcontainers.properties` (`ryuk.disabled=true`)
//! to allow container reuse across test processes.

use fs2::FileExt;
use sqlx::{Executor, PgPool};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use testcontainers::{
	ContainerAsync, GenericImage, ImageExt,
	core::{IntoContainerPort, WaitFor},
	runners::AsyncRunner,
};
use tokio::sync::OnceCell;
use tokio::time::sleep;
use uuid::Uuid;

/// Retry a fallible async operation with exponential backoff
///
/// # Arguments
///
/// * `max_attempts` - Maximum number of retry attempts
/// * `initial_delay` - Initial delay between retries (doubled after each attempt)
/// * `operation` - Async operation to retry
///
/// # Returns
///
/// Result of the operation if successful within max_attempts
async fn retry_with_backoff<F, Fut, T, E>(
	max_attempts: usize,
	initial_delay: Duration,
	mut operation: F,
) -> Result<T, E>
where
	F: FnMut() -> Fut,
	Fut: std::future::Future<Output = Result<T, E>>,
{
	let mut delay = initial_delay;
	let mut last_error = None;

	for attempt in 1..=max_attempts {
		match operation().await {
			Ok(result) => return Ok(result),
			Err(e) => {
				last_error = Some(e);
				if attempt < max_attempts {
					eprintln!(
						"[retry] Attempt {}/{} failed, retrying after {:?}",
						attempt, max_attempts, delay
					);
					sleep(delay).await;
					delay *= 2; // Exponential backoff
				}
			}
		}
	}

	Err(last_error.expect("Should have at least one error"))
}

/// Shared PostgreSQL container with base URL for connections
pub struct SharedPostgres {
	/// Container reference - kept alive to prevent container shutdown
	#[allow(dead_code)] // Container must be kept alive for the duration of tests
	container: Option<ContainerAsync<GenericImage>>,
	/// Base connection URL (without database name)
	pub base_url: String,
}

/// Global singleton for the shared PostgreSQL container (within a process)
static POSTGRES: OnceCell<SharedPostgres> = OnceCell::const_new();

/// Path to the shared URL file for cross-process coordination
fn get_url_file_path() -> PathBuf {
	std::env::temp_dir().join("reinhardt_test_postgres_url")
}

/// Path to the lock file for cross-process coordination
fn get_lock_file_path() -> PathBuf {
	std::env::temp_dir().join("reinhardt_test_postgres.lock")
}

/// Test if a PostgreSQL URL is reachable
async fn test_connection(url: &str) -> bool {
	match sqlx::postgres::PgPoolOptions::new()
		.max_connections(1)
		.acquire_timeout(Duration::from_secs(3))
		.connect(url)
		.await
	{
		Ok(pool) => {
			let result = sqlx::query("SELECT 1").fetch_one(&pool).await;
			result.is_ok()
		}
		Err(_) => false,
	}
}

/// Read the base URL from the shared file
fn read_url_from_file() -> Option<String> {
	let path = get_url_file_path();
	if !path.exists() {
		return None;
	}

	let mut file = std::fs::File::open(&path).ok()?;
	let mut url = String::new();
	file.read_to_string(&mut url).ok()?;

	if url.trim().is_empty() {
		None
	} else {
		Some(url.trim().to_string())
	}
}

/// Write the base URL to the shared file
fn write_url_to_file(url: &str) -> std::io::Result<()> {
	let path = get_url_file_path();
	let mut file = std::fs::File::create(&path)?;
	file.write_all(url.as_bytes())?;
	file.sync_all()
}

/// Start a new PostgreSQL container
async fn start_postgres_container() -> (ContainerAsync<GenericImage>, String) {
	// Ryuk is disabled via .testcontainers.properties file (ryuk.disabled=true)
	// to allow container reuse across processes. This avoids the thread-safety
	// violation of calling std::env::set_var in an async context. (Fixes #873)

	let container = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.start()
		.await
		.expect("Failed to start PostgreSQL container");

	let host = container.get_host().await.unwrap();

	// Retry port retrieval with exponential backoff to handle timing issues
	let port = retry_with_backoff(3, Duration::from_millis(100), || async {
		container
			.get_host_port_ipv4(5432.tcp())
			.await
			.map_err(|e| format!("Port retrieval failed: {}", e))
	})
	.await
	.expect("Failed to get container port after retries");

	// Base URL without database name - sslmode will be added at connection time
	let base_url = format!("postgres://postgres@{}:{}", host, port);

	eprintln!(
		"[shared_postgres] Started new PostgreSQL container at {}:{}",
		host, port
	);

	(container, base_url)
}

/// Initialize the template database
async fn init_template_database(base_url: &str) {
	// Pool configuration optimized for parallel test execution
	// See: https://github.com/launchbadge/sqlx/issues/2885 (prepared statement cache bug)
	// See: https://github.com/launchbadge/sqlx/issues/3241 (unexpected Sync message bug)
	let admin_pool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(5)
		.acquire_timeout(Duration::from_secs(60))
		.test_before_acquire(false)
		.idle_timeout(Some(Duration::from_secs(30)))
		.connect(&format!("{}/postgres?sslmode=disable", base_url))
		.await
		.expect("Failed to connect to PostgreSQL for template setup");

	// Create template database (ignore if exists)
	admin_pool
		.execute("CREATE DATABASE test_template")
		.await
		.ok();

	// Mark as template (allows fast cloning)
	admin_pool
		.execute("ALTER DATABASE test_template IS_TEMPLATE true")
		.await
		.ok();
}

/// Gets or initializes the shared PostgreSQL instance
///
/// This function handles cross-process coordination for nextest:
/// 1. Uses file-based locking for container coordination
/// 2. Reuses existing container if available and reachable
/// 3. Starts a new TestContainers PostgreSQL if needed
///
/// # Returns
///
/// A static reference to the `SharedPostgres` instance.
///
/// # Panics
///
/// Panics if the database cannot be connected to.
pub async fn get_shared_postgres() -> &'static SharedPostgres {
	POSTGRES
		.get_or_init(|| async {
			// Acquire file lock for cross-process coordination
			let lock_path = get_lock_file_path();
			let lock_file = std::fs::OpenOptions::new()
				.create(true)
				.write(true)
				.truncate(false)
				.open(&lock_path)
				.expect("Failed to create lock file");

			lock_file.lock_exclusive().expect("Failed to acquire lock");

			// Try to read existing URL and test connection
			if let Some(url) = read_url_from_file() {
				let postgres_url = format!("{}/postgres?sslmode=disable", url);
				if test_connection(&postgres_url).await {
					eprintln!("[shared_postgres] Reusing existing container at {}", url);
					lock_file.unlock().ok();
					return SharedPostgres {
						container: None, // Container owned by another process
						base_url: url,
					};
				} else {
					eprintln!(
						"[shared_postgres] Existing container not reachable, starting new one"
					);
				}
			}

			// Start new container
			let (container, base_url) = start_postgres_container().await;

			// Initialize template database
			init_template_database(&base_url).await;

			// Write URL to shared file
			write_url_to_file(&base_url).expect("Failed to write URL to file");

			lock_file.unlock().ok();

			SharedPostgres {
				container: Some(container),
				base_url,
			}
		})
		.await
}

/// Creates an isolated test database cloned from template
///
/// This function:
/// 1. Ensures the shared database is available
/// 2. Creates a new database from the template (~10-40ms)
/// 3. Returns a connection pool to the new database
///
/// # Returns
///
/// A new `PgPool` connected to an isolated test database.
///
/// # Pool Configuration
///
/// The pool is configured to avoid known sqlx v0.7+ bugs:
/// - `max_connections = 5`: Avoids prepared statement cache bug (#2885)
/// - `test_before_acquire = false`: Avoids "unexpected Sync message" bug (#3241)
///
/// # Panics
///
/// Panics if the test database cannot be created or connected to.
pub async fn get_test_pool() -> PgPool {
	let pg = get_shared_postgres().await;
	let db_name = format!("test_{}", Uuid::new_v4().simple());

	// Connect to postgres database to create test database
	let admin_pool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(1)
		.acquire_timeout(Duration::from_secs(10))
		.connect(&format!("{}/postgres?sslmode=disable", pg.base_url))
		.await
		.expect("Failed to connect to postgres for test database creation");

	// Clone from template (fast operation)
	let create_sql = format!("CREATE DATABASE {} TEMPLATE test_template", db_name);
	sqlx::query(&create_sql)
		.execute(&admin_pool)
		.await
		.expect("Failed to create test database from template");

	// Connect to the new test database with optimized settings
	// See: https://github.com/launchbadge/sqlx/issues/2885
	// See: https://github.com/launchbadge/sqlx/issues/3241
	sqlx::postgres::PgPoolOptions::new()
		// MUST be > 1 to avoid prepared statement cache bug
		.max_connections(5)
		// Reasonable timeout for test operations
		.acquire_timeout(Duration::from_secs(10))
		// CRITICAL: Disable to avoid "unexpected Sync message" timeout
		.test_before_acquire(false)
		// Prevent idle connection issues
		.idle_timeout(Some(Duration::from_secs(30)))
		.connect(&format!("{}/{}?sslmode=disable", pg.base_url, db_name))
		.await
		.expect("Failed to connect to test database")
}

/// Creates an isolated test database with table creation
///
/// This is a convenience function that:
/// 1. Creates an isolated test database
/// 2. Executes the provided table creation SQL
///
/// # Arguments
///
/// * `table_sql` - SQL statement(s) to create tables
///
/// # Returns
///
/// A `PgPool` connected to the new database with tables created.
pub async fn get_test_pool_with_table(table_sql: &str) -> PgPool {
	let pool = get_test_pool().await;

	sqlx::query(table_sql)
		.execute(&pool)
		.await
		.expect("Failed to create table in test database");

	pool
}

/// Creates an isolated test database AND initializes global ORM manager
///
/// This is useful for tests that use `reinhardt-views` or other components
/// that rely on `manager::get_connection()` for database access.
///
/// Unlike `get_test_pool()`, this function also initializes the global ORM
/// database connection, which is required by components like:
/// - `View::dispatch()` and generic API views (`ListAPIView`, `CreateAPIView`, etc.)
/// - `QuerySet::all()`, `QuerySet::filter()`, and other QuerySet methods
/// - Any code that calls `manager::get_connection()`
///
/// # Returns
///
/// A tuple of `(PgPool, database_url)` for the new isolated test database.
///
/// # Pool Configuration
///
/// The pool is configured identically to `get_test_pool()`:
/// - `max_connections = 5`: Avoids prepared statement cache bug (#2885)
/// - `test_before_acquire = false`: Avoids "unexpected Sync message" bug (#3241)
///
/// # Panics
///
/// Panics if the test database cannot be created, connected to, or if ORM
/// initialization fails.
pub async fn get_test_pool_with_orm() -> (PgPool, String) {
	let pg = get_shared_postgres().await;
	let db_name = format!("test_{}", Uuid::new_v4().simple());
	let db_url = format!("{}/{}?sslmode=disable", pg.base_url, db_name);

	// Connect to postgres database to create test database
	let admin_pool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(1)
		.acquire_timeout(Duration::from_secs(10))
		.connect(&format!("{}/postgres?sslmode=disable", pg.base_url))
		.await
		.expect("Failed to connect to postgres for test database creation");

	// Clone from template (fast operation)
	let create_sql = format!("CREATE DATABASE {} TEMPLATE test_template", db_name);
	sqlx::query(&create_sql)
		.execute(&admin_pool)
		.await
		.expect("Failed to create test database from template");

	// Connect to the new test database with optimized settings
	// See: https://github.com/launchbadge/sqlx/issues/2885
	// See: https://github.com/launchbadge/sqlx/issues/3241
	let pool = sqlx::postgres::PgPoolOptions::new()
		// MUST be > 1 to avoid prepared statement cache bug
		.max_connections(5)
		// Reasonable timeout for test operations
		.acquire_timeout(Duration::from_secs(10))
		// CRITICAL: Disable to avoid "unexpected Sync message" timeout
		.test_before_acquire(false)
		// Prevent idle connection issues
		.idle_timeout(Some(Duration::from_secs(30)))
		.connect(&db_url)
		.await
		.expect("Failed to connect to test database");

	// Initialize global ORM manager with this database
	reinhardt_db::orm::reinitialize_database(&db_url)
		.await
		.expect("Failed to reinitialize ORM database");

	(pool, db_url)
}

/// Cleanup helper: Remove the shared URL file
///
/// Call this after all tests are done to ensure clean state.
/// Usually not needed as containers are managed by Docker.
pub fn cleanup_shared_postgres() {
	let url_path = get_url_file_path();
	let lock_path = get_lock_file_path();

	if url_path.exists() {
		std::fs::remove_file(url_path).ok();
	}
	if lock_path.exists() {
		std::fs::remove_file(lock_path).ok();
	}
}

// ========================================================================
// rstest Fixtures for test integration
// ========================================================================

/// rstest fixture that provides a shared PostgreSQL pool with ORM initialized
///
/// This fixture wraps `get_test_pool_with_orm()` for easy use in rstest-based tests.
/// Each test gets an isolated database cloned from a template, with the global
/// ORM manager initialized.
///
/// # Usage
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::shared_db_pool;
/// use rstest::*;
/// use sqlx::PgPool;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_shared_pool(#[future] shared_db_pool: (PgPool, String)) {
///     let (pool, _url) = shared_db_pool.await;
///     // Use pool for database operations
/// }
/// ```
///
/// # Fixture-in-Fixture Pattern
///
/// This fixture can be used as a base for other fixtures:
///
/// ```rust,no_run
/// use reinhardt_test::fixtures::shared_db_pool;
/// use rstest::*;
/// use sqlx::PgPool;
/// use std::sync::Arc;
///
/// #[fixture]
/// async fn db_pool(#[future] shared_db_pool: (PgPool, String)) -> Arc<PgPool> {
///     let (pool, _url) = shared_db_pool.await;
///     Arc::new(pool)
/// }
///
/// #[fixture]
/// async fn articles_table(#[future] db_pool: Arc<PgPool>) -> Arc<PgPool> {
///     let pool = db_pool.await;
///     sqlx::query("CREATE TABLE articles (id SERIAL PRIMARY KEY, title TEXT)")
///         .execute(pool.as_ref())
///         .await
///         .unwrap();
///     pool
/// }
/// ```
#[rstest::fixture]
pub async fn shared_db_pool() -> (PgPool, String) {
	get_test_pool_with_orm().await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_shared_postgres_initialization() {
		let pg = get_shared_postgres().await;
		assert!(!pg.base_url.is_empty());
	}

	#[tokio::test]
	async fn test_isolated_databases() {
		let pool1 = get_test_pool().await;
		let pool2 = get_test_pool().await;

		// Create table in pool1
		sqlx::query("CREATE TABLE test_table (id SERIAL PRIMARY KEY)")
			.execute(&pool1)
			.await
			.expect("Failed to create table");

		// Table should not exist in pool2 (isolated)
		let result = sqlx::query("SELECT 1 FROM test_table")
			.fetch_optional(&pool2)
			.await;

		assert!(
			result.is_err(),
			"Databases should be isolated - table should not exist in pool2"
		);
	}
}
