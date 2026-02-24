#[cfg(feature = "testcontainers")]
use rstest::*;
#[cfg(feature = "testcontainers")]
use std::sync::Arc;

#[cfg(feature = "testcontainers")]
use testcontainers::{
	ImageExt,
	core::{ContainerPort, WaitFor},
	runners::AsyncRunner,
};

// Public re-exports for fixtures.rs
#[cfg(feature = "testcontainers")]
pub use testcontainers::{ContainerAsync, GenericImage};

/// Check if a port is available (not in use by any process)
#[cfg(feature = "testcontainers")]
async fn is_port_available(port: u16) -> bool {
	use tokio::net::TcpListener;
	TcpListener::bind(format!("127.0.0.1:{}", port))
		.await
		.is_ok()
}

/// Check if all 6 consecutive ports starting from base_port are available
#[cfg(feature = "testcontainers")]
async fn is_port_range_available(base_port: u16) -> bool {
	for offset in 0..6 {
		if !is_port_available(base_port + offset).await {
			return false;
		}
	}
	true
}

/// Get database connection pool configuration from environment variables.
///
/// This function reads pool configuration from environment variables,
/// falling back to sensible defaults if not set.
///
/// # Environment Variables
/// - `TEST_MAX_CONNECTIONS`: Maximum number of connections in the pool (default: 20)
/// - `TEST_ACQUIRE_TIMEOUT_SECS`: Timeout in seconds for acquiring a connection (default: 60)
///
/// # Returns
/// A tuple of (max_connections, acquire_timeout_secs)
///
/// # Example
/// ```bash
/// # Use custom pool configuration
/// TEST_MAX_CONNECTIONS=10 TEST_ACQUIRE_TIMEOUT_SECS=120 cargo nextest run
///
/// # Use default configuration (max_connections=5, timeout=60s)
/// cargo nextest run
/// ```
#[cfg(feature = "testcontainers")]
fn get_pool_config() -> (u32, u64) {
	let max_connections = std::env::var("TEST_MAX_CONNECTIONS")
		.ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(5); // Default: 5 (MUST be > 1 to avoid sqlx v0.7+ prepared statement cache bug #2885)

	let acquire_timeout = std::env::var("TEST_ACQUIRE_TIMEOUT_SECS")
		.ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(60); // Default: 60s - shorter timeout exposes real issues faster

	(max_connections, acquire_timeout)
}

/// Create an AnyPool with proper timeout configuration for tests.
///
/// This function uses the same timeout settings as `postgres_container` fixture,
/// ensuring consistent behavior across all test database connections.
///
/// # Arguments
/// * `database_url` - Connection URL (postgres://, mysql://, sqlite://)
///
/// # Example
/// ```ignore
/// use reinhardt_test::fixtures::testcontainers::create_test_any_pool;
///
/// # async fn example() {
/// let database_url = "postgres://localhost:5432/test";
/// let pool = create_test_any_pool(database_url).await.expect("Failed to connect");
/// # }
/// ```
#[cfg(feature = "testcontainers")]
pub async fn create_test_any_pool(database_url: &str) -> Result<sqlx::AnyPool, sqlx::Error> {
	use sqlx::any::AnyPoolOptions;

	let (max_conns, timeout_secs) = get_pool_config();

	AnyPoolOptions::new()
		.max_connections(max_conns)
		.min_connections(1)
		.acquire_timeout(std::time::Duration::from_secs(timeout_secs))
		.idle_timeout(std::time::Duration::from_secs(600))
		.max_lifetime(std::time::Duration::from_secs(1800))
		.connect(database_url)
		.await
}

/// Fixture: Find and return an available port range for Redis Cluster.
///
/// This fixture automatically searches for 6 consecutive available ports,
/// ensuring tests never fail due to port conflicts.
///
/// Port selection strategy:
/// 1. Check REDIS_CLUSTER_BASE_PORT environment variable (default: 17000)
/// 2. Verify all 6 consecutive ports are available
/// 3. If not available, try candidates: 27000, 37000, 47000
/// 4. If all candidates occupied, search 20000-60000 in steps of 1000
/// 5. Panic if no available range found
///
/// # Returns
/// Base port number where ports [base_port, base_port+5] are all available
///
/// # Example
/// ```rust
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_auto_ports(
///     #[future] redis_cluster_base_port: u16
/// ) {
///     let base = redis_cluster_base_port.await;
///     // Use ports: base, base+1, ..., base+5
/// }
/// ```
#[fixture]
#[cfg(feature = "testcontainers")]
pub async fn redis_cluster_base_port() -> u16 {
	// Generate process-specific port offset to avoid conflicts in parallel test execution
	// Each process gets a unique 10-port range based on its PID
	let pid = std::process::id();
	let pid_offset = ((pid % 10) * 10) as u16;
	let pid_based_port = 17000 + pid_offset;

	// Priority order:
	// 1. Environment variable (explicit override)
	// 2. PID-based port (automatic per-process allocation)
	// 3. Default 17000
	let env_preferred = std::env::var("REDIS_CLUSTER_BASE_PORT")
		.ok()
		.and_then(|s| s.parse().ok());

	// Build candidate list with priorities
	let mut candidates = Vec::new();

	// First priority: Environment variable override
	if let Some(env_port) = env_preferred {
		candidates.push(env_port);
	}

	// Second priority: PID-based port (for parallel execution)
	candidates.push(pid_based_port);

	// Third priority: Default 17000
	if !candidates.contains(&17000) {
		candidates.push(17000);
	}

	// Fourth priority: Standard fallbacks
	candidates.extend_from_slice(&[27000, 37000, 47000]);

	// Try each candidate
	for &candidate in &candidates {
		if is_port_range_available(candidate).await {
			eprintln!(
				"Using Redis Cluster port range: {}-{} (PID: {}, offset: {})",
				candidate,
				candidate + 5,
				pid,
				if candidate == pid_based_port {
					format!("{} [PID-based]", pid_offset)
				} else {
					"N/A".to_string()
				}
			);
			return candidate;
		}
	}

	eprintln!("WARNING: All preferred port ranges are occupied. Searching 20000-60000...");

	// If all predefined candidates are occupied, search for any available range
	// Start from 20000 to avoid well-known ports
	for base in (20000..60000).step_by(1000) {
		if is_port_range_available(base).await {
			eprintln!(
				"Found available port range: {}-{} (searched from 20000)",
				base,
				base + 5
			);
			return base;
		}
	}

	panic!(
		"Failed to find 6 consecutive available ports. Please free up some ports and try again."
	);
}

// File locking support
use fs2::FileExt;

// ============================================================================
// File Lock Guard for Inter-Process Synchronization
// ============================================================================

/// File-based lock guard for inter-process synchronization
///
/// Uses fs2::FileExt for cross-platform file locking. This is essential for
/// tests that require exclusive access to shared resources across process boundaries.
///
/// # Platform Support
///
/// - **Unix**: Uses advisory locking via flock(2)
/// - **Windows**: Uses mandatory locking via LockFileEx
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::FileLockGuard;
///
/// // Acquire lock (blocks until available)
/// let guard = FileLockGuard::new("/tmp/test.lock")?;
///
/// // Perform exclusive operations...
///
/// // Lock automatically released when guard drops
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct FileLockGuard {
	file: std::fs::File,
	path: std::path::PathBuf,
}

impl FileLockGuard {
	/// Create a new file lock guard
	///
	/// This will block the current thread until the lock can be acquired.
	///
	/// # Errors
	///
	/// Returns an error if the lock file cannot be created or locked.
	pub fn new(lock_path: impl Into<std::path::PathBuf>) -> std::io::Result<Self> {
		let path: std::path::PathBuf = lock_path.into();
		let file = std::fs::OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(false)
			.open(&path)?;

		file.lock_exclusive()?;

		Ok(Self { file, path })
	}
}

impl Drop for FileLockGuard {
	fn drop(&mut self) {
		let _ = self.file.unlock();
		let _ = std::fs::remove_file(&self.path);
	}
}

// ============================================================================
// PostgreSQL Container Fixtures
// ============================================================================

/// Fixture providing a PostgreSQL container with connection pool
///
/// Starts a PostgreSQL 17 Alpine container and provides a connection pool
/// for testing database operations.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::postgres_container;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_postgres(
///     #[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String)
/// ) {
///     let (_container, pool, port, url) = postgres_container.await;
///     let result = sqlx::query("SELECT 1").fetch_one(pool.as_ref()).await;
///     assert!(result.is_ok());
/// }
/// ```
#[fixture]
pub async fn postgres_container() -> (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String)
{
	use testcontainers::core::IntoContainerPort;

	let image = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust");

	let postgres = image
		.start()
		.await
		.expect("Failed to start PostgreSQL container");

	// Wait briefly before first port query to ensure container networking is ready
	// Increased from 200ms to 500ms for better reliability under heavy load
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	// Retry getting port with exponential backoff
	let mut port_retry = 0;
	let max_port_retries = 7; // Increased from 5 for better reliability under load
	let port = loop {
		match postgres.get_host_port_ipv4(5432).await {
			Ok(p) => break p,
			Err(e) if port_retry < max_port_retries => {
				port_retry += 1;
				let delay = tokio::time::Duration::from_millis(200 * 2_u64.pow(port_retry));
				eprintln!(
					"PostgreSQL port query attempt {} of {} failed: {:?}",
					port_retry, max_port_retries, e
				);
				tokio::time::sleep(delay).await;
			}
			Err(e) => {
				panic!(
					"Failed to get PostgreSQL port after {} retries: {}",
					max_port_retries, e
				);
			}
		}
	};

	let database_url = format!(
		"postgres://postgres@localhost:{}/postgres?sslmode=disable",
		port
	);

	// Get pool configuration from environment variables
	let (max_conns, timeout_secs) = get_pool_config();

	// Retry connection to PostgreSQL with exponential backoff
	let mut retry_count = 0;
	let max_retries = 7; // Increased from 5 for better reliability in CI environments

	// Wait briefly before first connection to ensure container is fully ready
	tokio::time::sleep(std::time::Duration::from_millis(500)).await;

	let pool = loop {
		match sqlx::postgres::PgPoolOptions::new()
			.max_connections(max_conns)
			.min_connections(1)
			.acquire_timeout(std::time::Duration::from_secs(timeout_secs))
			.idle_timeout(std::time::Duration::from_secs(600)) // Increase from 30s for sqlx v0.7+ compatibility
			.max_lifetime(std::time::Duration::from_secs(1800)) // Increase from 120s for long-running tests
			.test_before_acquire(false) // sqlx v0.7+ bug workaround (issue #2885, #3241)
			.connect(&database_url)
			.await
		{
			Ok(pool) => {
				// Verify wire protocol is working correctly
				match sqlx::query("SELECT 1").fetch_one(&pool).await {
					Ok(_) => break pool,
					Err(e) if retry_count < max_retries => {
						eprintln!(
							"PostgreSQL health check attempt {} of {} failed: {:?}",
							retry_count + 1,
							max_retries,
							e
						);
						retry_count += 1;
						let delay = std::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
						tokio::time::sleep(delay).await;
						continue;
					}
					Err(e) => {
						panic!(
							"PostgreSQL pool created but health check failed after {} retries: {}",
							max_retries, e
						);
					}
				}
			}
			Err(e) if retry_count < max_retries => {
				eprintln!(
					"PostgreSQL connection attempt {} of {} failed: {:?}",
					retry_count + 1,
					max_retries,
					e
				);
				retry_count += 1;
				let delay = std::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
				tokio::time::sleep(delay).await;
			}
			Err(e) => {
				panic!(
					"Failed to connect to PostgreSQL after {} retries: {}",
					max_retries, e
				);
			}
		}
	};

	(postgres, Arc::new(pool), port, database_url)
}

pub async fn cockroachdb_container()
-> (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String) {
	use testcontainers::core::IntoContainerPort;

	let cockroachdb = GenericImage::new("cockroachdb/cockroach", "v23.1.0")
		.with_exposed_port(26257.tcp())
		.with_wait_for(WaitFor::message_on_stderr("initialized new cluster"))
		.with_cmd(vec![
			"start-single-node".to_string(),
			"--insecure".to_string(),
			"--store=type=mem,size=1GiB".to_string(),
		])
		.start()
		.await
		.expect("Failed to start CockroachDB container");

	let port = cockroachdb
		.get_host_port_ipv4(26257)
		.await
		.expect("Failed to get CockroachDB port");

	// Connect to postgres database to create defaultdb if needed
	let postgres_url = format!("postgresql://root@127.0.0.1:{}/postgres", port);

	let postgres_pool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(1)
		.connect(&postgres_url)
		.await
		.expect("Failed to connect to CockroachDB postgres database");

	// Create defaultdb database
	sqlx::query("CREATE DATABASE IF NOT EXISTS defaultdb")
		.execute(&postgres_pool)
		.await
		.expect("Failed to create defaultdb");

	postgres_pool.close().await;

	// Now connect to defaultdb
	let database_url = format!("postgresql://root@127.0.0.1:{}/defaultdb", port);

	// Get pool configuration from environment variables
	let (max_conns, timeout_secs) = get_pool_config();

	let pool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(max_conns)
		.min_connections(1)
		.acquire_timeout(std::time::Duration::from_secs(timeout_secs))
		.idle_timeout(std::time::Duration::from_secs(30))
		.max_lifetime(std::time::Duration::from_secs(120))
		.connect(&database_url)
		.await
		.expect("Failed to connect to CockroachDB defaultdb");

	(cockroachdb, Arc::new(pool), port, database_url)
}

// ============================================================================
// Redis Container Fixtures
// ============================================================================

/// Fixture providing a Redis container
///
/// Starts a Redis 7 Alpine container for testing cache and pub/sub operations.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::redis_container;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_redis(
///     #[future] redis_container: (ContainerAsync<GenericImage>, u16, String)
/// ) {
///     let (_container, port, url) = redis_container.await;
///     let client = redis::Client::open(url.as_str()).unwrap();
///     let mut conn = client.get_multiplexed_async_connection().await.unwrap();
///     redis::cmd("PING").query_async::<String>(&mut conn).await.unwrap();
/// }
/// ```
#[fixture]
pub async fn redis_container() -> (ContainerAsync<GenericImage>, u16, String) {
	const MAX_RETRIES: u32 = 3;
	const RETRY_DELAY_MS: u64 = 2000;

	let mut last_error = None;

	for attempt in 0..MAX_RETRIES {
		match try_start_redis_container().await {
			Ok(result) => return result,
			Err(e) => {
				eprintln!(
					"Redis container start attempt {} of {} failed: {:?}",
					attempt + 1,
					MAX_RETRIES,
					e
				);
				last_error = Some(e);

				if attempt < MAX_RETRIES - 1 {
					tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
				}
			}
		}
	}

	panic!(
		"Failed to start Redis container after {} attempts: {:?}",
		MAX_RETRIES, last_error
	);
}

async fn try_start_redis_container()
-> Result<(ContainerAsync<GenericImage>, u16, String), Box<dyn std::error::Error>> {
	use testcontainers::core::IntoContainerPort;

	let redis = GenericImage::new("redis", "7-alpine")
		.with_exposed_port(6379.tcp())
		.with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
		.start()
		.await?;

	let port = redis.get_host_port_ipv4(6379).await?;

	let url = format!("redis://localhost:{}", port);

	Ok((redis, port, url))
}

// ============================================================================
// Redis Cluster Container Fixtures
// ============================================================================

/// Metadata for Redis Cluster container
///
/// Stores cluster container reference and initial node ports.
/// Used for cleanup and port tracking.
pub struct RedisClusterContainer {
	pub container: ContainerAsync<GenericImage>,
	/// Initial 6 node ports (7000-7005 mapped to host ports)
	pub node_ports: Vec<u16>,
}

impl std::fmt::Debug for RedisClusterContainer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RedisClusterContainer")
			.field("node_ports", &self.node_ports)
			.field("container", &"<ContainerAsync>")
			.finish()
	}
}

/// Level 1: Acquire file lock for Redis Cluster initialization
///
/// Prevents concurrent cluster initialization across test processes.
/// Lock is held for the entire test duration.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::testcontainers::redis_cluster_lock;
/// use rstest::*;
///
/// #[rstest]
/// fn test_with_cluster_lock(redis_cluster_lock: reinhardt_test::fixtures::FileLockGuard) {
///     // Lock ensures exclusive cluster access
/// }
/// ```
#[fixture]
pub fn redis_cluster_lock() -> FileLockGuard {
	let lock_path = std::env::temp_dir().join("reinhardt_redis_cluster.lock");
	FileLockGuard::new(lock_path).expect("Failed to acquire Redis cluster lock")
}

/// Level 2: Stop and remove any existing Redis Cluster container
///
/// Ensures clean state before starting new cluster.
/// Depends on: redis_cluster_lock
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::testcontainers::{redis_cluster_lock, redis_cluster_cleanup};
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_cleanup(
///     redis_cluster_lock: reinhardt_test::fixtures::FileLockGuard,
///     #[future] redis_cluster_cleanup: ()
/// ) {
///     let _ = redis_cluster_cleanup.await;
///     // Old cluster is now removed
/// }
/// ```
#[fixture]
pub async fn redis_cluster_cleanup(_redis_cluster_lock: FileLockGuard) {
	// DISABLED: This cleanup was stopping containers from other parallel tests
	// TestContainers automatically cleans up containers when they are dropped

	// // Try to find and stop any existing Redis cluster container
	// // Use docker CLI to find container by name pattern
	// let output = tokio::process::Command::new("docker")
	// 	.args([
	// 		"ps",
	// 		"-a",
	// 		"--filter",
	// 		"ancestor=neohq/redis-cluster:latest",
	// 		"--format",
	// 		"{{.ID}}",
	// 	])
	// 	.output()
	// 	.await;
	//
	// if let Ok(output) = output {
	// 	let container_ids = String::from_utf8_lossy(&output.stdout);
	// 	for container_id in container_ids.lines() {
	// 		let container_id = container_id.trim();
	// 		if !container_id.is_empty() {
	// 			eprintln!(
	// 				"Stopping existing Redis cluster container: {}",
	// 				container_id
	// 			);
	// 			let _ = tokio::process::Command::new("docker")
	// 				.args(["stop", container_id])
	// 				.output()
	// 				.await;
	// 			let _ = tokio::process::Command::new("docker")
	// 				.args(["rm", container_id])
	// 				.output()
	// 				.await;
	// 		}
	// 	}
	// }
	//
	// // Small delay to ensure complete cleanup
}

/// Helper function to attempt Redis cluster container start
async fn try_start_redis_cluster(
	base_port: u16,
) -> Result<(ContainerAsync<GenericImage>, Vec<u16>), Box<dyn std::error::Error>> {
	let cluster = GenericImage::new("grokzen/redis-cluster", "7.0.10")
		.with_wait_for(WaitFor::message_on_stdout("Cluster state changed: ok"))
		.with_startup_timeout(std::time::Duration::from_secs(600))
		.with_env_var("IP", "0.0.0.0")
		.with_env_var("INITIAL_PORT", base_port.to_string())
		.with_mapped_port(base_port, ContainerPort::Tcp(base_port))
		.with_mapped_port(base_port + 1, ContainerPort::Tcp(base_port + 1))
		.with_mapped_port(base_port + 2, ContainerPort::Tcp(base_port + 2))
		.with_mapped_port(base_port + 3, ContainerPort::Tcp(base_port + 3))
		.with_mapped_port(base_port + 4, ContainerPort::Tcp(base_port + 4))
		.with_mapped_port(base_port + 5, ContainerPort::Tcp(base_port + 5))
		.start()
		.await?;

	let node_ports = vec![
		base_port,
		base_port + 1,
		base_port + 2,
		base_port + 3,
		base_port + 4,
		base_port + 5,
	];

	// Wait for all Redis services to start listening
	let max_retries = 30;
	for retry in 0..max_retries {
		let mut all_ready = true;
		for &port in &node_ports {
			if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
				.await
				.is_err()
			{
				all_ready = false;
				break;
			}
		}

		if all_ready {
			eprintln!("All Redis cluster ports ready after {} attempts", retry + 1);
			return Ok((cluster, node_ports));
		}
	}

	Err(format!(
		"Redis cluster ports not ready after {} retries. Ports: {:?}",
		max_retries, node_ports
	)
	.into())
}

#[fixture]
pub async fn redis_cluster_ports_ready(
	#[future] redis_cluster_cleanup: (),
	#[future] redis_cluster_base_port: u16,
) -> (ContainerAsync<GenericImage>, Vec<u16>) {
	let _ = redis_cluster_cleanup.await;
	let mut base_port = redis_cluster_base_port.await;

	// IMPORTANT: Use fixed port mapping (host port = container port)
	//
	// Why fixed ports are necessary:
	// 1. grokzen/redis-cluster runs 6 Redis instances in a single container
	// 2. ClusterClient executes CLUSTER SLOTS to discover topology
	// 3. CLUSTER SLOTS returns internal ports that cannot be overridden
	// 4. redis-rs ClusterClient has no configuration to override port mapping
	// 5. Therefore, host ports MUST match container ports for ClusterClient to work
	//
	// Port selection is handled by redis_cluster_base_port fixture:
	// - Automatically finds 6 consecutive available ports
	// - Checks REDIS_CLUSTER_BASE_PORT env var (default: 17000)
	// - Falls back to alternatives (27000, 37000, 47000) if occupied
	// - Searches 20000-60000 range if all predefined candidates are taken
	// - This ensures tests never fail due to port conflicts

	const MAX_PORT_RETRIES: usize = 5;
	const PORT_INCREMENT: u16 = 1000;

	for retry in 0..MAX_PORT_RETRIES {
		if retry > 0 {
			eprintln!(
				"Retrying Redis cluster start with port {} (attempt {}/{})",
				base_port,
				retry + 1,
				MAX_PORT_RETRIES
			);
		} else {
			eprintln!(
				"Using Redis Cluster port range: {}-{}",
				base_port,
				base_port + 5
			);
		}

		// Verify ports are still available just before container start
		if !is_port_range_available(base_port).await {
			eprintln!(
				"Port range {}-{} became unavailable, trying next range",
				base_port,
				base_port + 5
			);
			base_port += PORT_INCREMENT;
			continue;
		}

		match try_start_redis_cluster(base_port).await {
			Ok((container, node_ports)) => {
				eprintln!(
					"Redis cluster started successfully on ports {:?}",
					node_ports
				);
				return (container, node_ports);
			}
			Err(e) => {
				eprintln!("Failed to start Redis cluster on port {}: {}", base_port, e);

				// If port allocation error, try next port range
				if e.to_string().contains("port is already allocated")
					|| e.to_string().contains("address already in use")
				{
					base_port += PORT_INCREMENT;
					continue;
				}

				// For other errors, panic immediately
				panic!("Failed to start Redis cluster (non-port error): {}", e);
			}
		}
	}

	panic!(
		"Failed to start Redis cluster after {} attempts. Last port tried: {}",
		MAX_PORT_RETRIES, base_port
	);
}

/// Level 4: Wait for cluster initialization (CLUSTER INFO shows cluster_state:ok)
///
/// Polls CLUSTER INFO until cluster is fully initialized.
/// Depends on: redis_cluster_ports_ready
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::testcontainers::redis_cluster_container;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_cluster_ready(
///     redis_cluster_lock: reinhardt_test::fixtures::FileLockGuard,
///     #[future] redis_cluster_cleanup: (),
///     #[future] redis_cluster_ports_ready: (ContainerAsync<GenericImage>, Vec<u16>),
///     #[future] redis_cluster_container: reinhardt_test::fixtures::RedisClusterContainer
/// ) {
///     let container = redis_cluster_container.await;
///     assert_eq!(container.node_ports.len(), 6);
/// }
/// ```
#[fixture]
pub async fn redis_cluster_container(
	#[future] redis_cluster_ports_ready: (ContainerAsync<GenericImage>, Vec<u16>),
) -> RedisClusterContainer {
	let (cluster, node_ports) = redis_cluster_ports_ready.await;

	// WaitFor condition already confirmed "Cluster state changed: ok"
	// No retry needed - just return the container
	eprintln!("Redis cluster ready with ports: {:?}", node_ports);

	RedisClusterContainer {
		container: cluster,
		node_ports,
	}
}

/// Level 5: Complete Redis Cluster fixture with connection
///
/// Provides initialized cluster container + working redis::cluster::ClusterClient.
/// Depends on: redis_cluster_container
///
/// This is the top-level fixture you should use in most tests.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::redis_cluster;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_redis_cluster(
///     redis_cluster_lock: reinhardt_test::fixtures::FileLockGuard,
///     #[future] redis_cluster_cleanup: (),
///     #[future] redis_cluster_ports_ready: (ContainerAsync<GenericImage>, Vec<u16>),
///     #[future] redis_cluster_container: reinhardt_test::fixtures::RedisClusterContainer,
///     #[future] redis_cluster: (
///         reinhardt_test::fixtures::RedisClusterContainer,
///         Arc<redis::cluster::ClusterClient>,
///         Vec<String>
///     )
/// ) {
///     let (container, client, nodes) = redis_cluster.await;
///     let mut conn = client.get_async_connection().await.unwrap();
///     redis::cmd("SET").arg("key").arg("value").query_async::<()>(&mut conn).await.unwrap();
/// }
/// ```
#[fixture]
pub async fn redis_cluster(
	#[future] redis_cluster_container: RedisClusterContainer,
) -> (
	RedisClusterContainer,
	Arc<redis::cluster::ClusterClient>,
	Vec<String>,
) {
	let container = redis_cluster_container.await;

	// Build cluster node URLs
	let cluster_nodes: Vec<String> = container
		.node_ports
		.iter()
		.map(|&port| format!("redis://127.0.0.1:{}", port))
		.collect();

	// Create cluster client
	let client = redis::cluster::ClusterClient::new(cluster_nodes.clone())
		.expect("Failed to create cluster client");

	// Verify cluster connection works
	let mut conn = client
		.get_async_connection()
		.await
		.expect("Failed to connect to cluster");

	// Test basic operation
	redis::cmd("PING")
		.query_async::<String>(&mut conn)
		.await
		.expect("Failed to PING cluster");

	eprintln!("Redis cluster connection verified");

	(container, Arc::new(client), cluster_nodes)
}

/// Lightweight Redis Cluster fixture
///
/// Returns cluster client, node URLs, and the container reference.
/// The container must be kept alive for the duration of the test to prevent
/// premature cleanup of the Redis cluster.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::testcontainers::redis_cluster_client;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_redis(
///     #[future] redis_cluster_client: (
///         Arc<redis::cluster::ClusterClient>,
///         Vec<String>,
///         RedisClusterContainer,
///     )
/// ) {
///     let (client, _nodes, _container) = redis_cluster_client.await;
///     let mut conn = client.get_async_connection().await.unwrap();
///     redis::cmd("SET").arg("key").arg("value").query_async::<()>(&mut conn).await.unwrap();
/// }
/// ```
#[fixture]
pub async fn redis_cluster_client(
	#[future] redis_cluster_container: RedisClusterContainer,
) -> (
	Arc<redis::cluster::ClusterClient>,
	Vec<String>,
	RedisClusterContainer,
) {
	let container = redis_cluster_container.await;

	// Build cluster node URLs
	let cluster_nodes: Vec<String> = container
		.node_ports
		.iter()
		.map(|&port| format!("redis://127.0.0.1:{}", port))
		.collect();

	// Create cluster client
	let client = redis::cluster::ClusterClient::new(cluster_nodes.clone())
		.expect("Failed to create cluster client");

	// Verify cluster connection works
	let mut conn = client
		.get_async_connection()
		.await
		.expect("Failed to connect to cluster");

	// Test basic operation
	redis::cmd("PING")
		.query_async::<String>(&mut conn)
		.await
		.expect("Failed to PING cluster");

	eprintln!("Redis cluster client created");

	// Return container to keep it alive during the test
	(Arc::new(client), cluster_nodes, container)
}

/// Ultra-lightweight Redis Cluster URLs fixture
///
/// Returns only cluster node URLs, completely avoiding container types.
/// This is the safest fixture for tests that don't need container lifecycle control.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::testcontainers::redis_cluster_urls;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_redis(#[future] redis_cluster_urls: Vec<String>) {
///     let urls = redis_cluster_urls.await;
///     // Use urls to create cache or client
/// }
/// ```
#[fixture]
pub async fn redis_cluster_urls(
	#[future] redis_cluster_container: RedisClusterContainer,
) -> (Vec<String>, RedisClusterContainer) {
	let container = redis_cluster_container.await;

	// Build cluster node URLs from health-checked container
	// redis_cluster_container already verified CLUSTER INFO shows cluster_state:ok
	let cluster_nodes: Vec<String> = container
		.node_ports
		.iter()
		.map(|&port| format!("redis://127.0.0.1:{}", port))
		.collect();

	// Return both URLs and container to keep container alive during test
	(cluster_nodes, container)
}

/// Alternative Redis Cluster fixture without composable dependencies
///
/// This fixture provides a complete Redis Cluster setup in a single fixture,
/// without requiring explicit declaration of intermediate dependency fixtures.
/// Internally manages file locking and cleanup.
///
/// Use this when you want a simpler test setup without the 5-level composable pattern.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::testcontainers::redis_cluster_fixture;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_simple_cluster(
///     #[future] redis_cluster_fixture: (
///         reinhardt_test::fixtures::RedisClusterContainer,
///         Arc<redis::cluster::ClusterClient>,
///         Vec<String>
///     )
/// ) {
///     let (_container, client, _nodes) = redis_cluster_fixture.await;
///     let mut conn = client.get_async_connection().await.unwrap();
///     redis::cmd("SET").arg("key").arg("value").query_async::<()>(&mut conn).await.unwrap();
/// }
/// ```
#[fixture]
pub async fn redis_cluster_fixture() -> (
	RedisClusterContainer,
	Arc<redis::cluster::ClusterClient>,
	Vec<String>,
) {
	// Level 1: Acquire lock
	let _lock = {
		let lock_path = std::env::temp_dir().join("reinhardt_redis_cluster.lock");
		FileLockGuard::new(lock_path).expect("Failed to acquire Redis cluster lock")
	};

	// Level 2: Cleanup existing containers
	{
		let output = tokio::process::Command::new("docker")
			.args([
				"ps",
				"-a",
				"--filter",
				"ancestor=neohq/redis-cluster:latest",
				"--format",
				"{{.ID}}",
			])
			.output()
			.await;

		if let Ok(output) = output {
			let container_ids = String::from_utf8_lossy(&output.stdout);
			for container_id in container_ids.lines() {
				let container_id = container_id.trim();
				if !container_id.is_empty() {
					eprintln!(
						"Stopping existing Redis cluster container: {}",
						container_id
					);
					let _ = tokio::process::Command::new("docker")
						.args(["stop", container_id])
						.output()
						.await;
					let _ = tokio::process::Command::new("docker")
						.args(["rm", container_id])
						.output()
						.await;
				}
			}
		}
	}

	// Level 3: Start container and wait for ports
	let (cluster, node_ports) = {
		use testcontainers::core::IntoContainerPort;

		let cluster = GenericImage::new("neohq/redis-cluster", "latest")
			.with_exposed_port(7000.tcp())
			.with_exposed_port(7001.tcp())
			.with_exposed_port(7002.tcp())
			.with_exposed_port(7003.tcp())
			.with_exposed_port(7004.tcp())
			.with_exposed_port(7005.tcp())
			.with_wait_for(WaitFor::message_on_stdout("[OK] All 16384 slots covered."))
			.with_startup_timeout(std::time::Duration::from_secs(600))
			.start()
			.await
			.expect("Failed to start Redis cluster container");

		let node_ports = vec![
			cluster
				.get_host_port_ipv4(7000)
				.await
				.expect("Failed to get port for node 7000"),
			cluster
				.get_host_port_ipv4(7001)
				.await
				.expect("Failed to get port for node 7001"),
			cluster
				.get_host_port_ipv4(7002)
				.await
				.expect("Failed to get port for node 7002"),
			cluster
				.get_host_port_ipv4(7003)
				.await
				.expect("Failed to get port for node 7003"),
			cluster
				.get_host_port_ipv4(7004)
				.await
				.expect("Failed to get port for node 7004"),
			cluster
				.get_host_port_ipv4(7005)
				.await
				.expect("Failed to get port for node 7005"),
		];

		// Wait for all ports to be accessible
		let max_retries = 30;
		for retry in 0..max_retries {
			let mut all_ready = true;
			for &port in &node_ports {
				if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
					.await
					.is_err()
				{
					all_ready = false;
					break;
				}
			}

			if all_ready {
				eprintln!("All Redis cluster ports ready after {} attempts", retry + 1);
				break;
			}

			if retry == max_retries - 1 {
				panic!(
					"Redis cluster ports not ready after {} retries. Ports: {:?}",
					max_retries, node_ports
				);
			}
		}

		(cluster, node_ports)
	};

	// Level 4: Wait for cluster initialization
	{
		let max_retries = 60;
		for retry in 0..max_retries {
			let client_result = redis::Client::open(format!("redis://127.0.0.1:{}", node_ports[0]));

			if let Ok(client) = client_result
				&& let Ok(mut conn) = client.get_multiplexed_async_connection().await
				&& let Ok(info) = redis::cmd("CLUSTER")
					.arg("INFO")
					.query_async::<String>(&mut conn)
					.await && info.contains("cluster_state:ok")
			{
				eprintln!(
					"Redis cluster fully initialized after {} attempts",
					retry + 1
				);
				break;
			}

			if retry == max_retries - 1 {
				panic!(
					"Redis cluster not initialized after {} retries. Ports: {:?}",
					max_retries, node_ports
				);
			}
		}
	}

	// Level 5: Create client and verify connection
	let cluster_nodes: Vec<String> = node_ports
		.iter()
		.map(|&port| format!("redis://127.0.0.1:{}", port))
		.collect();

	let client = redis::cluster::ClusterClient::new(cluster_nodes.clone())
		.expect("Failed to create cluster client");

	let mut conn = client
		.get_async_connection()
		.await
		.expect("Failed to connect to cluster");

	redis::cmd("PING")
		.query_async::<String>(&mut conn)
		.await
		.expect("Failed to PING cluster");

	eprintln!("Redis cluster connection verified");

	let container = RedisClusterContainer {
		container: cluster,
		node_ports,
	};

	(container, Arc::new(client), cluster_nodes)
}

// ============================================================================
// MongoDB Container Fixture
// ============================================================================

async fn try_start_mongodb_container()
-> Result<(ContainerAsync<GenericImage>, String, u16), Box<dyn std::error::Error>> {
	use testcontainers::core::IntoContainerPort;

	let mongo = GenericImage::new("mongo", "7.0")
		.with_exposed_port(27017.tcp())
		.with_wait_for(WaitFor::message_on_stdout("Waiting for connections"))
		.with_startup_timeout(std::time::Duration::from_secs(60))
		.start()
		.await?;

	let port = mongo.get_host_port_ipv4(27017).await?;
	let connection_string = format!("mongodb://127.0.0.1:{}", port);

	Ok((mongo, connection_string, port))
}

/// Fixture providing a MongoDB container
///
/// Starts a MongoDB 7.0 container for testing document operations.
#[fixture]
pub async fn mongodb_container() -> (ContainerAsync<GenericImage>, String, u16) {
	const MAX_RETRIES: u32 = 3;
	const RETRY_DELAY_MS: u64 = 2000;

	let mut last_error = None;

	for attempt in 0..MAX_RETRIES {
		match try_start_mongodb_container().await {
			Ok(result) => return result,
			Err(e) => {
				eprintln!(
					"MongoDB container start attempt {} of {} failed: {:?}",
					attempt + 1,
					MAX_RETRIES,
					e
				);
				last_error = Some(e);

				if attempt < MAX_RETRIES - 1 {
					tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
				}
			}
		}
	}

	panic!(
		"Failed to start MongoDB container after {} attempts: {:?}",
		MAX_RETRIES, last_error
	);
}

// ============================================================================
// LocalStack Container Fixture
// ============================================================================

/// Fixture providing LocalStack container for AWS service mocking
///
/// Starts a LocalStack container with S3, DynamoDB, and other AWS services.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::localstack_fixture;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_localstack(
///     #[future] localstack_fixture: (ContainerAsync<GenericImage>, u16, String)
/// ) {
///     let (_container, port, endpoint) = localstack_fixture.await;
///     // Use endpoint for AWS SDK configuration
/// }
/// ```
#[fixture]
pub async fn localstack_fixture() -> (ContainerAsync<GenericImage>, u16, String) {
	use testcontainers::core::IntoContainerPort;

	let localstack = GenericImage::new("localstack/localstack", "latest")
		.with_exposed_port(4566.tcp())
		.with_wait_for(WaitFor::message_on_stdout("Ready."))
		.with_env_var("SERVICES", "s3,dynamodb")
		.start()
		.await
		.expect("Failed to start LocalStack container");

	let port = localstack
		.get_host_port_ipv4(4566)
		.await
		.expect("Failed to get LocalStack port");

	let endpoint = format!("http://localhost:{}", port);

	(localstack, port, endpoint)
}

// ============================================================================
// Migration Application Fixtures
// ============================================================================

/// Fixture: PostgreSQL container with migrations from a MigrationProvider
///
/// This function starts a PostgreSQL container, applies migrations from the
/// specified `MigrationProvider`, and returns a ready-to-use connection.
///
/// Unlike `postgres_with_migrations`, this function uses compile-time migration
/// collection via the `MigrationProvider` trait, which is necessary because Rust
/// cannot dynamically load code at runtime.
///
/// # Type Parameters
/// * `P` - A type implementing `MigrationProvider`
///
/// # Returns
/// * `(ContainerAsync<GenericImage>, Arc<DatabaseConnection>)` - Container and database connection
///
/// # Example
///
/// ```ignore
/// # use reinhardt_test::fixtures::postgres_with_migrations_from;
/// # use reinhardt_db::migrations::MigrationProvider;
/// # #[tokio::main]
/// # async fn main() {
/// // In your app's migrations.rs, use collect_migrations! macro
/// // pub mod _0001_initial;
/// // pub mod _0002_add_field;
///
/// // collect_migrations!(
/// //     app_label = "myapp",
/// //     _0001_initial,
/// //     _0002_add_field,
/// // );
///
/// // Migrations are automatically registered in global registry via linkme
///
/// // #[tokio::test]
/// // async fn test_with_migrations() {
/// //     let (container, db) = postgres_with_migrations_from::<MyappMigrations>().await;
/// //     // Database has all migrations applied from MyappMigrations provider
/// //     let result = db.fetch_all("SELECT * FROM my_table", vec![]).await;
/// //     assert!(result.is_ok());
/// // }
/// # }
/// ```
#[cfg(feature = "testcontainers")]
pub async fn postgres_with_migrations_from<P: reinhardt_db::migrations::MigrationProvider>()
-> Result<
	(
		ContainerAsync<GenericImage>,
		std::sync::Arc<reinhardt_db::DatabaseConnection>,
	),
	Box<dyn std::error::Error>,
> {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use std::sync::Arc;

	// Start PostgreSQL container
	let (container, _pool, _port, url) = postgres_container().await;

	// Connect to database
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.map_err(|e| format!("Failed to connect to PostgreSQL for migrations: {}", e))?;

	// Get migrations from provider
	let migrations = P::migrations();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.map_err(|e| format!("Failed to apply migrations: {}", e))?;
	}

	Ok((container, Arc::new(connection)))
}

/// Fixture: MySQL container (base fixture)
///
/// Starts a MySQL 8.0 container and provides a connection pool.
///
/// # Examples
///
/// ```ignore
/// use reinhardt_test::fixtures::mysql_container;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_mysql(
///     #[future] mysql_container: (ContainerAsync<GenericImage>, Arc<sqlx::MySqlPool>, u16, String)
/// ) {
///     let (_container, pool, _port, url) = mysql_container.await;
///     let result = sqlx::query("SELECT 1").fetch_one(pool.as_ref()).await;
///     assert!(result.is_ok());
/// }
/// ```
#[fixture]
#[cfg(feature = "testcontainers")]
pub async fn mysql_container() -> (
	ContainerAsync<GenericImage>,
	Arc<sqlx::MySqlPool>,
	u16,
	String,
) {
	use testcontainers::core::IntoContainerPort;

	let mysql = GenericImage::new("mysql", "8.0")
		.with_exposed_port(3306.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"port: 3306  MySQL Community Server",
		))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.with_env_var("MYSQL_ROOT_PASSWORD", "test")
		.with_env_var("MYSQL_DATABASE", "test_db")
		.start()
		.await
		.expect("Failed to start MySQL container");

	// Wait briefly before first port query to ensure container networking is ready
	// Increased from 200ms to 500ms for better reliability under heavy load
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	// Retry getting port with exponential backoff
	let mut port_retry = 0;
	let max_port_retries = 7; // Increased from 5 for better reliability under load
	let port = loop {
		match mysql.get_host_port_ipv4(3306).await {
			Ok(p) => break p,
			Err(e) if port_retry < max_port_retries => {
				port_retry += 1;
				let delay = tokio::time::Duration::from_millis(200 * 2_u64.pow(port_retry));
				eprintln!(
					"MySQL port query attempt {} of {} failed: {:?}",
					port_retry, max_port_retries, e
				);
				tokio::time::sleep(delay).await;
			}
			Err(e) => panic!(
				"Failed to get MySQL port after {} retries: {}",
				max_port_retries, e
			),
		}
	};

	let database_url = format!("mysql://root:test@localhost:{}/test_db", port);

	// Get pool configuration from environment variables
	let (max_conns, timeout_secs) = get_pool_config();

	// Retry connection to MySQL with exponential backoff
	let mut retry_count = 0;
	let max_retries = 7; // Increased from 5 for better reliability in CI environments

	// Wait briefly before first connection to ensure container is fully ready
	tokio::time::sleep(std::time::Duration::from_millis(500)).await;

	let pool = loop {
		match sqlx::mysql::MySqlPoolOptions::new()
			.max_connections(max_conns)
			.min_connections(1)
			.acquire_timeout(std::time::Duration::from_secs(timeout_secs))
			.idle_timeout(std::time::Duration::from_secs(600)) // Increase from 30s for sqlx v0.7+ compatibility
			.max_lifetime(std::time::Duration::from_secs(1800)) // Increase from 120s for long-running tests
			.test_before_acquire(false) // sqlx v0.7+ bug workaround (issue #2885, #3241)
			.connect(&database_url)
			.await
		{
			Ok(pool) => {
				// Verify wire protocol is working correctly
				match sqlx::query("SELECT 1").fetch_one(&pool).await {
					Ok(_) => break pool,
					Err(e) if retry_count < max_retries => {
						eprintln!(
							"MySQL health check attempt {} of {} failed: {:?}",
							retry_count + 1,
							max_retries,
							e
						);
						retry_count += 1;
						let delay = std::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
						tokio::time::sleep(delay).await;
						continue;
					}
					Err(e) => panic!(
						"MySQL health check failed after {} retries: {}",
						max_retries, e
					),
				}
			}
			Err(e) if retry_count < max_retries => {
				eprintln!(
					"MySQL connection attempt {} of {} failed: {:?}",
					retry_count + 1,
					max_retries,
					e
				);
				retry_count += 1;
				let delay = std::time::Duration::from_millis(200 * 2_u64.pow(retry_count));
				tokio::time::sleep(delay).await;
			}
			Err(e) => panic!(
				"Failed to connect to MySQL after {} retries: {}",
				max_retries, e
			),
		}
	};

	(mysql, Arc::new(pool), port, database_url)
}

/// MySQL container with migrations from a MigrationProvider
///
/// This function starts a MySQL container, applies migrations from the
/// specified `MigrationProvider`, and returns a ready-to-use connection.
///
/// # Type Parameters
/// * `P` - A type implementing `MigrationProvider`
///
/// # Returns
/// * `(ContainerAsync<GenericImage>, Arc<DatabaseConnection>)` - Container and database connection
///
/// # Example
///
/// ```ignore
/// # use reinhardt_test::fixtures::mysql_with_migrations_from;
/// # use reinhardt_db::migrations::MigrationProvider;
/// # #[tokio::main]
/// # async fn main() {
/// // In your app's migrations.rs, use collect_migrations! macro
/// // pub mod _0001_initial;
///
/// // collect_migrations!(
/// //     app_label = "myapp",
/// //     _0001_initial,
/// // );
///
/// // Migrations are automatically registered in global registry via linkme
///
/// // #[tokio::test]
/// // async fn test_with_migrations() {
/// //     let (container, db) = mysql_with_migrations_from::<MyappMigrations>().await;
/// //     // Database has all migrations applied from MyappMigrations provider
/// // }
/// # }
/// ```
#[cfg(feature = "testcontainers")]
pub async fn mysql_with_migrations_from<P: reinhardt_db::migrations::MigrationProvider>() -> (
	ContainerAsync<GenericImage>,
	std::sync::Arc<reinhardt_db::DatabaseConnection>,
) {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use std::sync::Arc;

	// Start MySQL container
	let (container, _pool, _port, url) = mysql_container().await;

	// Connect to database
	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL for migrations");

	// Get migrations from provider
	let migrations = P::migrations();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.expect("Failed to apply migrations");
	}

	(container, Arc::new(connection))
}

/// SQLite in-memory database with migrations from a MigrationProvider
///
/// This function creates an SQLite in-memory database, applies migrations from the
/// specified `MigrationProvider`, and returns a ready-to-use connection.
///
/// # Type Parameters
/// * `P` - A type implementing `MigrationProvider`
///
/// # Returns
/// * `Arc<DatabaseConnection>` - Database connection (no container needed for SQLite)
///
/// # Example
///
/// ```ignore
/// # use reinhardt_test::fixtures::sqlite_with_migrations_from;
/// # use reinhardt_db::migrations::MigrationProvider;
/// # #[tokio::main]
/// # async fn main() {
/// // In your app's migrations.rs, use collect_migrations! macro
/// // pub mod _0001_initial;
///
/// // collect_migrations!(
/// //     app_label = "myapp",
/// //     _0001_initial,
/// // );
///
/// // Migrations are automatically registered in global registry via linkme
///
/// // #[tokio::test]
/// // async fn test_with_migrations() {
/// //     let db = sqlite_with_migrations_from::<MyappMigrations>().await;
/// //     // Database has all migrations applied from MyappMigrations provider
/// // }
/// # }
/// ```
#[cfg(feature = "testcontainers")]
pub async fn sqlite_with_migrations_from<P: reinhardt_db::migrations::MigrationProvider>()
-> std::sync::Arc<reinhardt_db::DatabaseConnection> {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use std::sync::Arc;

	let database_url = "sqlite::memory:";

	// Connect to database
	let connection = DatabaseConnection::connect_sqlite(database_url)
		.await
		.expect("Failed to connect to SQLite for migrations");

	// Get migrations from provider
	let migrations = P::migrations();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.expect("Failed to apply migrations");
	}

	Arc::new(connection)
}

// ============================================================================
// Non-Generic Fixtures with Global Registry
// ============================================================================
// These fixtures use the global migration registry populated by `collect_migrations!`
// macro calls. They automatically collect and apply all registered migrations.

/// PostgreSQL container with ALL registered migrations applied
///
/// This fixture collects migrations from the global registry (populated by
/// `collect_migrations!` macro calls) and applies them to a fresh PostgreSQL
/// container.
///
/// # Example
///
/// ```ignore
/// # use reinhardt_test::fixtures::*;
/// # use rstest::*;
/// # use std::sync::Arc;
/// # use reinhardt_db::DatabaseConnection;
/// # use ::testcontainers::{ContainerAsync, GenericImage};
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_all_migrations(
///     #[future] postgres_with_all_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
/// ) {
///     let (_container, db) = postgres_with_all_migrations.await;
///     // All migrations from all apps are applied
///     let result = db.fetch_all("SELECT * FROM django_migrations", vec![]).await;
///     assert!(result.is_ok());
/// }
/// ```
///
/// # Prerequisites
///
/// Your app must register migrations using `collect_migrations!`:
///
/// ```rust,ignore
/// // In your app's migrations.rs
/// reinhardt::collect_migrations!(
///     app_label = "polls",
///     _0001_initial,
///     _0002_add_fields,
/// );
/// ```
#[cfg(feature = "testcontainers")]
#[rstest::fixture]
pub async fn postgres_with_all_migrations() -> Result<
	(
		ContainerAsync<GenericImage>,
		std::sync::Arc<reinhardt_db::DatabaseConnection>,
	),
	Box<dyn std::error::Error>,
> {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt_db::migrations::registry::{MigrationRegistry, global_registry};
	use std::sync::Arc;

	// Start PostgreSQL container
	let (container, _pool, _port, url) = postgres_container().await;

	// Connect to database
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.map_err(|e| format!("Failed to connect to PostgreSQL for migrations: {}", e))?;

	// Get migrations from global registry
	let migrations = global_registry().all_migrations();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.map_err(|e| format!("Failed to apply migrations: {}", e))?;
	}

	Ok((container, Arc::new(connection)))
}

/// PostgreSQL container with migrations from specific apps
///
/// This function allows selective application of migrations from specific apps.
///
/// # Arguments
///
/// * `app_labels` - List of app labels to include (e.g., `["polls", "users"]`)
///
/// # Example
///
/// ```ignore
/// # use reinhardt_test::fixtures::postgres_with_apps_migrations;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // #[tokio::test]
/// // async fn test_polls_only() {
/// let (_container, db) = postgres_with_apps_migrations(&["polls"]).await?;
/// // Only polls app migrations are applied
/// // }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "testcontainers")]
pub async fn postgres_with_apps_migrations(
	app_labels: &[&str],
) -> Result<
	(
		ContainerAsync<GenericImage>,
		std::sync::Arc<reinhardt_db::DatabaseConnection>,
	),
	Box<dyn std::error::Error>,
> {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt_db::migrations::registry::{MigrationRegistry, global_registry};
	use std::sync::Arc;

	// Start PostgreSQL container
	let (container, _pool, _port, url) = postgres_container().await;

	// Connect to database
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.map_err(|e| format!("Failed to connect to PostgreSQL for migrations: {}", e))?;

	// Get migrations from global registry, filtered by app labels
	let migrations: Vec<_> = global_registry()
		.all_migrations()
		.into_iter()
		.filter(|m| app_labels.contains(&m.app_label.as_str()))
		.collect();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.map_err(|e| format!("Failed to apply migrations: {}", e))?;
	}

	Ok((container, Arc::new(connection)))
}

/// MySQL container with ALL registered migrations applied
///
/// This fixture collects migrations from the global registry and applies them
/// to a fresh MySQL container.
///
/// # Example
///
/// ```ignore
/// # use reinhardt_test::fixtures::*;
/// # use rstest::*;
/// # use std::sync::Arc;
/// # use reinhardt_db::DatabaseConnection;
/// # use ::testcontainers::{ContainerAsync, GenericImage};
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_all_migrations(
///     #[future] mysql_with_all_migrations: (ContainerAsync<GenericImage>, Arc<DatabaseConnection>)
/// ) {
///     let (_container, db) = mysql_with_all_migrations.await;
///     // All migrations from all apps are applied
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[rstest::fixture]
pub async fn mysql_with_all_migrations() -> (
	ContainerAsync<GenericImage>,
	std::sync::Arc<reinhardt_db::DatabaseConnection>,
) {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt_db::migrations::registry::{MigrationRegistry, global_registry};
	use std::sync::Arc;

	// Start MySQL container
	let (container, _pool, _port, url) = mysql_container().await;

	// Connect to database
	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL for migrations");

	// Get migrations from global registry
	let migrations = global_registry().all_migrations();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.expect("Failed to apply migrations");
	}

	(container, Arc::new(connection))
}

/// MySQL container with migrations from specific apps
///
/// # Arguments
///
/// * `app_labels` - List of app labels to include
#[cfg(feature = "testcontainers")]
pub async fn mysql_with_apps_migrations(
	app_labels: &[&str],
) -> (
	ContainerAsync<GenericImage>,
	std::sync::Arc<reinhardt_db::DatabaseConnection>,
) {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt_db::migrations::registry::{MigrationRegistry, global_registry};
	use std::sync::Arc;

	// Start MySQL container
	let (container, _pool, _port, url) = mysql_container().await;

	// Connect to database
	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to MySQL for migrations");

	// Get migrations from global registry, filtered by app labels
	let migrations: Vec<_> = global_registry()
		.all_migrations()
		.into_iter()
		.filter(|m| app_labels.contains(&m.app_label.as_str()))
		.collect();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.expect("Failed to apply migrations");
	}

	(container, Arc::new(connection))
}

/// SQLite in-memory database with ALL registered migrations applied
///
/// This fixture collects migrations from the global registry and applies them
/// to a fresh SQLite in-memory database.
///
/// # Example
///
/// ```ignore
/// # use reinhardt_test::fixtures::*;
/// # use rstest::*;
/// # use std::sync::Arc;
/// # use reinhardt_db::DatabaseConnection;
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_all_migrations(
///     #[future] sqlite_with_all_migrations: Arc<DatabaseConnection>
/// ) {
///     let db = sqlite_with_all_migrations.await;
///     // All migrations from all apps are applied
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[rstest::fixture]
pub async fn sqlite_with_all_migrations() -> std::sync::Arc<reinhardt_db::DatabaseConnection> {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt_db::migrations::registry::{MigrationRegistry, global_registry};
	use std::sync::Arc;

	let database_url = "sqlite::memory:";

	// Connect to database
	let connection = DatabaseConnection::connect_sqlite(database_url)
		.await
		.expect("Failed to connect to SQLite for migrations");

	// Get migrations from global registry
	let migrations = global_registry().all_migrations();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.expect("Failed to apply migrations");
	}

	Arc::new(connection)
}

/// SQLite in-memory database with migrations from specific apps
///
/// # Arguments
///
/// * `app_labels` - List of app labels to include
#[cfg(feature = "testcontainers")]
pub async fn sqlite_with_apps_migrations(
	app_labels: &[&str],
) -> std::sync::Arc<reinhardt_db::DatabaseConnection> {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	use reinhardt_db::migrations::registry::{MigrationRegistry, global_registry};
	use std::sync::Arc;

	let database_url = "sqlite::memory:";

	// Connect to database
	let connection = DatabaseConnection::connect_sqlite(database_url)
		.await
		.expect("Failed to connect to SQLite for migrations");

	// Get migrations from global registry, filtered by app labels
	let migrations: Vec<_> = global_registry()
		.all_migrations()
		.into_iter()
		.filter(|m| app_labels.contains(&m.app_label.as_str()))
		.collect();

	if !migrations.is_empty() {
		let mut executor = DatabaseMigrationExecutor::new(connection.inner().clone());
		executor
			.apply_migrations(&migrations)
			.await
			.expect("Failed to apply migrations");
	}

	Arc::new(connection)
}

// ============================================================================
// RabbitMQ Container Fixtures
// ============================================================================

/// RabbitMQ container fixture for testing message queue operations
///
/// Returns a tuple of (container, port, url) where:
/// - container: The running RabbitMQ container instance
/// - port: The host port mapped to RabbitMQ's AMQP port (5672)
/// - url: AMQP connection URL (e.g., "amqp://localhost:55001/%2f")
///
/// # Example
///
/// ```rust
/// use reinhardt_test::fixtures::rabbitmq_container;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_rabbitmq(
///     #[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String)
/// ) {
///     let (_container, port, url) = rabbitmq_container.await;
///     // Use RabbitMQ connection
/// }
/// ```
#[fixture]
pub async fn rabbitmq_container() -> (ContainerAsync<GenericImage>, u16, String) {
	const MAX_RETRIES: u32 = 3;
	const RETRY_DELAY_MS: u64 = 2000;

	let mut last_error = None;

	for attempt in 0..MAX_RETRIES {
		match try_start_rabbitmq_container().await {
			Ok(result) => return result,
			Err(e) => {
				eprintln!(
					"RabbitMQ container start attempt {} of {} failed: {:?}",
					attempt + 1,
					MAX_RETRIES,
					e
				);
				last_error = Some(e);

				if attempt < MAX_RETRIES - 1 {
					tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
				}
			}
		}
	}

	panic!(
		"Failed to start RabbitMQ container after {} attempts: {:?}",
		MAX_RETRIES, last_error
	);
}

async fn try_start_rabbitmq_container()
-> Result<(ContainerAsync<GenericImage>, u16, String), Box<dyn std::error::Error>> {
	use testcontainers::core::IntoContainerPort;

	let rabbitmq = GenericImage::new("rabbitmq", "3-management-alpine")
		.with_exposed_port(5672.tcp()) // AMQP port
		.with_exposed_port(15672.tcp()) // Management UI port
		.with_wait_for(WaitFor::message_on_stdout("Server startup complete"))
		.with_startup_timeout(std::time::Duration::from_secs(120))
		.start()
		.await?;

	// Retry getting port with exponential backoff
	let mut port_retry = 0;
	let max_port_retries = 5;
	let port = loop {
		match rabbitmq.get_host_port_ipv4(5672).await {
			Ok(p) => break p,
			Err(_) if port_retry < max_port_retries => {
				port_retry += 1;
				let delay = std::time::Duration::from_millis(100 * 2_u64.pow(port_retry));
				tokio::time::sleep(delay).await;
			}
			Err(e) => {
				return Err(Box::new(std::io::Error::other(format!(
					"Failed to get RabbitMQ port after {} retries: {}",
					max_port_retries, e
				))));
			}
		}
	};

	// RabbitMQ default vhost is "/" which needs to be URL-encoded as "%2f"
	let url = format!("amqp://localhost:{}/%2f", port);

	Ok((rabbitmq, port, url))
}
