//! TestContainers integration for database testing
//!
//! Provides automatic Docker container management for testing with real databases.
//! Containers are automatically started and cleaned up during tests.
//!
//! # Features
//!
//! - **PostgreSQL**: Full-featured PostgreSQL container with customizable credentials
//! - **MySQL**: MySQL container with customizable credentials
//! - **Redis**: Redis container for cache/session testing
//! - **SQLite**: In-memory and temporary file database URLs
//!
//! # Container Types
//!
//! ## PostgresContainer
//!
//! ```rust,no_run
//! use reinhardt_test::containers::{PostgresContainer, TestDatabase};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let container = PostgresContainer::new().await;
//! let url = container.connection_url();
//! // Use url for database connection
//! # }
//! ```
//!
//! ## MySqlContainer
//!
//! ```rust,no_run
//! use reinhardt_test::containers::{MySqlContainer, TestDatabase};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let container = MySqlContainer::new().await;
//! let url = container.connection_url();
//! # }
//! ```
//!
//! ## RedisContainer
//!
//! ```rust,no_run
//! use reinhardt_test::containers::RedisContainer;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let container = RedisContainer::new().await;
//! let url = container.connection_url();
//! # }
//! ```
//!
//! # Helper Functions
//!
//! ## Quick Start Functions
//!
//! ```rust,no_run
//! use reinhardt_test::containers::{start_postgres, start_redis};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let (pg_container, pg_url) = start_postgres().await;
//! let (redis_container, redis_url) = start_redis().await;
//! # }
//! ```
//!
//! ## Test Wrapper Functions
//!
//! ```rust,no_run
//! use reinhardt_test::containers::with_postgres;
//!
//! #[tokio::test]
//! async fn my_test() {
//!     with_postgres(|db| async move {
//!         let url = db.connection_url();
//!         // Use database...
//!         Ok(())
//!     }).await.unwrap();
//! }
//! ```
//!
//! ## SQLite Helpers
//!
//! ```rust,no_run
//! use reinhardt_test::containers::sqlite;
//!
//! let memory_url = sqlite::memory_url();
//! let temp_url = sqlite::temp_file_url("my_test");
//! ```

use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::mysql::Mysql;
use testcontainers_modules::redis::Redis as RedisImage;

/// Test key used by Memcached container's `wait_ready()` method to verify readiness.
///
/// This key is used to test actual Memcached set/get operations during initialization.
/// Uses a single underscore prefix following Rust conventions for internal test identifiers.
const TEST_WAIT_READY_KEY: &str = "_test_wait_ready";

/// Common interface for database test containers
#[async_trait::async_trait]
pub trait TestDatabase: Send + Sync {
	/// Get the database connection URL
	fn connection_url(&self) -> String;

	/// Get the database type (postgres, mysql, etc.)
	fn database_type(&self) -> &'static str;

	/// Wait for the database to be ready
	async fn wait_ready(&self) -> Result<(), Box<dyn std::error::Error>>;
}

/// PostgreSQL test container
pub struct PostgresContainer {
	#[allow(dead_code)]
	container: ContainerAsync<GenericImage>,
	host: String,
	port: u16,
	database: String,
	username: String,
	password: String,
}

/// Helper function to start a PostgreSQL container with default credentials
///
/// This is provided for compatibility with existing test code.
/// Returns a tuple of (container, connection_url).
///
/// Default credentials:
/// - Username: postgres
/// - Password: postgres
/// - Database: test
pub async fn start_postgres() -> (PostgresContainer, String) {
	let container = PostgresContainer::new().await;
	let url = container.connection_url();
	(container, url)
}

/// Helper function to start a PostgreSQL container with custom credentials
///
/// Returns a tuple of (container, connection_url).
pub async fn start_postgres_with_credentials(
	username: &str,
	password: &str,
	database: &str,
) -> (PostgresContainer, String) {
	let container = PostgresContainer::with_credentials(username, password, database).await;
	let url = container.connection_url();
	(container, url)
}

impl PostgresContainer {
	/// Create a new PostgreSQL container with default settings
	pub async fn new() -> Self {
		Self::with_credentials("postgres", "postgres", "test").await
	}
	/// Create a PostgreSQL container with custom credentials
	pub async fn with_credentials(username: &str, password: &str, database: &str) -> Self {
		use testcontainers::core::IntoContainerPort;

		// Use GenericImage to ensure port is properly exposed
		let image = GenericImage::new("postgres", "16-alpine")
			.with_exposed_port(5432.tcp())
			.with_wait_for(WaitFor::message_on_stderr(
				"database system is ready to accept connections",
			))
			.with_env_var("POSTGRES_USER", username)
			.with_env_var("POSTGRES_PASSWORD", password)
			.with_env_var("POSTGRES_DB", database);

		let container = AsyncRunner::start(image)
			.await
			.expect("Failed to start PostgreSQL container");

		// PostgreSQL listens on port 5432 inside container
		// testcontainers automatically maps it to a random host port
		let port = container
			.get_host_port_ipv4(5432)
			.await
			.expect("Failed to get PostgreSQL port");

		Self {
			container,
			host: "localhost".to_string(),
			port,
			database: database.to_string(),
			username: username.to_string(),
			password: password.to_string(),
		}
	}
	/// Get the container port
	pub fn port(&self) -> u16 {
		self.port
	}
}

#[async_trait::async_trait]
impl TestDatabase for PostgresContainer {
	fn connection_url(&self) -> String {
		format!(
			"postgres://{}:{}@{}:{}/{}?sslmode=disable",
			self.username, self.password, self.host, self.port, self.database
		)
	}

	fn database_type(&self) -> &'static str {
		"postgres"
	}

	async fn wait_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
		// Try to connect to ensure database is ready
		let url = self.connection_url();
		let pool = sqlx::postgres::PgPool::connect(&url).await?;
		sqlx::query("SELECT 1").execute(&pool).await?;
		pool.close().await;
		Ok(())
	}
}

/// MySQL test container
pub struct MySqlContainer {
	#[allow(dead_code)]
	container: ContainerAsync<Mysql>,
	host: String,
	port: u16,
	database: String,
	username: String,
	password: String,
}

impl MySqlContainer {
	/// Create a new MySQL container with default settings
	pub async fn new() -> Self {
		Self::with_credentials("root", "test", "test").await
	}
	/// Create a MySQL container with custom credentials
	pub async fn with_credentials(username: &str, password: &str, database: &str) -> Self {
		// Use mysql:8.0 image (MySQL does not provide official Alpine images)
		let image = Mysql::default()
			.with_tag("8.0")
			.with_env_var("MYSQL_ROOT_PASSWORD", password)
			.with_env_var("MYSQL_DATABASE", database);

		let container = AsyncRunner::start(image)
			.await
			.expect("Failed to start MySQL container");
		let port = container
			.get_host_port_ipv4(3306)
			.await
			.expect("MySQL container port should be available after startup");

		Self {
			container,
			host: "localhost".to_string(),
			port,
			database: database.to_string(),
			username: username.to_string(),
			password: password.to_string(),
		}
	}
	/// Get the container port
	pub fn port(&self) -> u16 {
		self.port
	}
}

#[async_trait::async_trait]
impl TestDatabase for MySqlContainer {
	fn connection_url(&self) -> String {
		format!(
			"mysql://{}:{}@{}:{}/{}",
			self.username, self.password, self.host, self.port, self.database
		)
	}

	fn database_type(&self) -> &'static str {
		"mysql"
	}

	async fn wait_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
		// Try to connect to ensure database is ready
		let url = self.connection_url();
		let pool = sqlx::mysql::MySqlPool::connect(&url).await?;
		sqlx::query("SELECT 1").execute(&pool).await?;
		pool.close().await;
		Ok(())
	}
}

/// Redis test container
pub struct RedisContainer {
	#[allow(dead_code)]
	container: ContainerAsync<RedisImage>,
	host: String,
	port: u16,
}

/// Helper function to start a Redis container (alias for RedisContainer::new)
///
/// This is provided for compatibility with existing test code.
/// Returns a tuple of (container, connection_url).
pub async fn start_redis() -> (RedisContainer, String) {
	let container = RedisContainer::new().await;
	let url = container.connection_url();
	(container, url)
}

impl RedisContainer {
	/// Create a new Redis container
	pub async fn new() -> Self {
		let image = RedisImage::default();
		let container = AsyncRunner::start(image)
			.await
			.expect("Failed to start Redis container");
		let port = container
			.get_host_port_ipv4(6379)
			.await
			.expect("Redis container port should be available after startup");

		let redis_container = Self {
			container,
			host: "localhost".to_string(),
			port,
		};

		// Wait for Redis to be ready
		redis_container
			.wait_until_ready()
			.await
			.expect("Redis container failed to become ready");

		redis_container
	}

	/// Wait for Redis server to be ready to accept connections
	async fn wait_until_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
		use redis::AsyncCommands;
		use tokio::time::{Duration, sleep};

		let connection_url = self.connection_url();

		// Try to connect to Redis with retries (max 30 attempts, ~15 seconds total)
		for attempt in 1..=30 {
			match redis::Client::open(connection_url.as_str()) {
				Ok(client) => {
					match client.get_multiplexed_async_connection().await {
						Ok(mut conn) => {
							// Try PING command to ensure Redis is fully ready
							match conn.ping::<String>().await {
								Ok(_) => {
									// Connection successful and Redis is ready
									return Ok(());
								}
								Err(e) if attempt < 30 => {
									// PING failed, but we'll retry
									eprintln!("Redis PING attempt {}/30 failed: {}", attempt, e);
									sleep(Duration::from_millis(500)).await;
								}
								Err(e) => {
									// Final attempt failed
									return Err(Box::new(std::io::Error::new(
										std::io::ErrorKind::ConnectionRefused,
										format!(
											"Redis failed to become ready after 30 attempts: {}",
											e
										),
									)));
								}
							}
						}
						Err(e) if attempt < 30 => {
							// Connection failed, but we'll retry
							eprintln!("Redis connection attempt {}/30 failed: {}", attempt, e);
							sleep(Duration::from_millis(500)).await;
						}
						Err(e) => {
							// Final attempt failed
							return Err(Box::new(std::io::Error::new(
								std::io::ErrorKind::ConnectionRefused,
								format!("Redis failed to become ready after 30 attempts: {}", e),
							)));
						}
					}
				}
				Err(e) if attempt < 30 => {
					eprintln!("Redis client creation attempt {}/30 failed: {}", attempt, e);
					sleep(Duration::from_millis(500)).await;
				}
				Err(e) => {
					return Err(Box::new(e));
				}
			}
		}

		Ok(())
	}

	/// Get the connection URL for Redis
	pub fn connection_url(&self) -> String {
		format!("redis://{}:{}", self.host, self.port)
	}
	/// Get the container port
	pub fn port(&self) -> u16 {
		self.port
	}
}

/// Memcached test container
pub struct MemcachedContainer {
	#[allow(dead_code)]
	container: ContainerAsync<GenericImage>,
	host: String,
	port: u16,
}

/// Helper function to start a Memcached container
///
/// Returns a tuple of (container, connection_url).
pub async fn start_memcached() -> (MemcachedContainer, String) {
	let container = MemcachedContainer::new().await;
	let url = container.connection_url();
	(container, url)
}

impl MemcachedContainer {
	/// Create a new Memcached container
	pub async fn new() -> Self {
		use testcontainers::core::IntoContainerPort;

		// Start Memcached container without WaitFor (we'll handle it manually)
		let image = GenericImage::new("memcached", "1.6-alpine").with_exposed_port(11211.tcp());

		let container = AsyncRunner::start(image)
			.await
			.expect("Failed to start Memcached container");
		let port = container
			.get_host_port_ipv4(11211)
			.await
			.expect("Memcached container port should be available after startup");

		let instance = Self {
			container,
			host: "localhost".to_string(),
			port,
		};

		// Wait for Memcached to be fully ready with set/get test
		// This has its own retry logic with exponential backoff
		instance
			.wait_ready()
			.await
			.expect("Failed to wait for Memcached to be ready");

		instance
	}

	/// Get the connection URL for Memcached
	pub fn connection_url(&self) -> String {
		format!("{}:{}", self.host, self.port)
	}

	/// Get the container port
	pub fn port(&self) -> u16 {
		self.port
	}

	/// Wait for Memcached to be ready by performing actual set/get operations
	///
	/// This method implements exponential backoff retry logic and tests
	/// actual Memcached operations (set/get) instead of just connection checks.
	/// This ensures Memcached is fully initialized and ready to handle requests.
	pub async fn wait_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
		use memcache_async::ascii::Protocol;
		use std::time::Duration;
		use tokio::net::TcpStream;
		use tokio::time::sleep;
		use tokio_util::compat::TokioAsyncReadCompatExt;

		let max_attempts = 10;
		let mut attempt = 0;
		let base_delay = Duration::from_millis(100);
		let test_key = TEST_WAIT_READY_KEY.to_string();
		let test_value = b"ready";

		while attempt < max_attempts {
			match TcpStream::connect(format!("{}:{}", self.host, self.port)).await {
				Ok(stream) => {
					let compat_stream = stream.compat();
					let mut proto = Protocol::new(compat_stream);

					// Test actual set operation
					if let Ok(()) = proto.set(&test_key, test_value, 10).await {
						// Test actual get operation
						if let Ok(retrieved) = proto.get(&test_key).await
							&& retrieved == test_value
						{
							// Success! Clean up test key
							let _ = proto.delete(&test_key).await;
							// Small delay to ensure cleanup completes
							sleep(Duration::from_millis(50)).await;
							return Ok(());
						}
					}

					// Operations failed, retry with backoff
					attempt += 1;
					let delay = base_delay * 2_u32.pow(attempt.min(5));
					sleep(delay).await;
				}
				Err(e) => {
					// Connection failed
					attempt += 1;
					if attempt >= max_attempts {
						return Err(format!(
							"Memcached not ready after {} attempts: {}",
							max_attempts, e
						)
						.into());
					}

					// Exponential backoff: 100ms, 200ms, 400ms, 800ms, 1600ms, 3200ms...
					let delay = base_delay * 2_u32.pow(attempt.min(5));
					sleep(delay).await;
				}
			}
		}

		Err("Memcached not ready: set/get test failed after maximum retry attempts".into())
	}
}

/// Helper function to run a test with a Memcached container
pub async fn with_memcached<F, Fut>(f: F) -> Result<(), Box<dyn std::error::Error>>
where
	F: FnOnce(MemcachedContainer) -> Fut,
	Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
	let container = MemcachedContainer::new().await;
	f(container).await
}

/// Helper function to run a test with a database container
///
/// # Example
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_test::containers::{with_postgres, PostgresContainer};
///
/// #[tokio::test]
/// async fn test_with_database() {
///     with_postgres(|db| async move {
///         let url = db.connection_url();
///         // Use database...
///         Ok(())
///     }).await.unwrap();
/// }
/// # }
/// ```
pub async fn with_postgres<F, Fut>(f: F) -> Result<(), Box<dyn std::error::Error>>
where
	F: FnOnce(PostgresContainer) -> Fut,
	Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
	let container = PostgresContainer::new().await;
	container.wait_ready().await?;
	f(container).await
}
/// Helper function to run a test with a MySQL container
pub async fn with_mysql<F, Fut>(f: F) -> Result<(), Box<dyn std::error::Error>>
where
	F: FnOnce(MySqlContainer) -> Fut,
	Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
	let container = MySqlContainer::new().await;
	container.wait_ready().await?;
	f(container).await
}
/// Helper function to run a test with a Redis container
pub async fn with_redis<F, Fut>(f: F) -> Result<(), Box<dyn std::error::Error>>
where
	F: FnOnce(RedisContainer) -> Fut,
	Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
	let container = RedisContainer::new().await;
	f(container).await
}

/// RabbitMQ test container
pub struct RabbitMQContainer {
	#[allow(dead_code)]
	container: ContainerAsync<GenericImage>,
	host: String,
	port: u16,
	management_port: u16,
	username: String,
	password: String,
}

/// Helper function to start a RabbitMQ container
///
/// Returns a tuple of (container, connection_url, management_url).
pub async fn start_rabbitmq() -> (RabbitMQContainer, String, String) {
	let container = RabbitMQContainer::new().await;
	let url = container.connection_url();
	let mgmt_url = container.management_url();
	(container, url, mgmt_url)
}

impl RabbitMQContainer {
	/// Create a new RabbitMQ container
	pub async fn new() -> Self {
		Self::with_credentials("guest", "guest").await
	}

	/// Create a RabbitMQ container with custom credentials
	pub async fn with_credentials(username: &str, password: &str) -> Self {
		use testcontainers::core::IntoContainerPort;

		let image = GenericImage::new("rabbitmq", "3.12-management-alpine")
			.with_exposed_port(5672.tcp())      // AMQP port
			.with_exposed_port(15672.tcp())     // Management UI port
			.with_wait_for(WaitFor::message_on_stdout("Server startup complete"))
			.with_env_var("RABBITMQ_DEFAULT_USER", username)
			.with_env_var("RABBITMQ_DEFAULT_PASS", password);

		let container = AsyncRunner::start(image)
			.await
			.expect("Failed to start RabbitMQ container");

		// RabbitMQ AMQP port (5672) and Management UI port (15672)
		let port = container
			.get_host_port_ipv4(5672)
			.await
			.expect("RabbitMQ AMQP container port should be available after startup");
		let management_port = container
			.get_host_port_ipv4(15672)
			.await
			.expect("RabbitMQ management container port should be available after startup");

		let rabbitmq_container = Self {
			container,
			host: "localhost".to_string(),
			port,
			management_port,
			username: username.to_string(),
			password: password.to_string(),
		};

		// Wait for RabbitMQ to be ready
		rabbitmq_container
			.wait_until_ready()
			.await
			.expect("RabbitMQ container failed to become ready");

		rabbitmq_container
	}

	/// Wait for RabbitMQ server to be ready to accept connections
	async fn wait_until_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
		use tokio::time::{Duration, sleep};

		let connection_url = self.connection_url();

		// Try to connect to RabbitMQ with retries (max 30 attempts, ~15 seconds total)
		for attempt in 1..=30 {
			match lapin::Connection::connect(
				&connection_url,
				lapin::ConnectionProperties::default(),
			)
			.await
			{
				Ok(conn) => {
					// Successfully connected, close and return
					let _ = conn.close(200, "OK").await;
					return Ok(());
				}
				Err(e) if attempt < 30 => {
					eprintln!("RabbitMQ connection attempt {}/30 failed: {}", attempt, e);
					sleep(Duration::from_millis(500)).await;
				}
				Err(e) => {
					return Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::ConnectionRefused,
						format!("RabbitMQ failed to become ready after 30 attempts: {}", e),
					)));
				}
			}
		}

		Ok(())
	}

	/// Get the AMQP connection URL for RabbitMQ
	pub fn connection_url(&self) -> String {
		format!(
			"amqp://{}:{}@{}:{}",
			self.username, self.password, self.host, self.port
		)
	}

	/// Get the Management UI URL for RabbitMQ
	pub fn management_url(&self) -> String {
		format!("http://{}:{}", self.host, self.management_port)
	}

	/// Get the AMQP port
	pub fn port(&self) -> u16 {
		self.port
	}

	/// Get the Management UI port
	pub fn management_port(&self) -> u16 {
		self.management_port
	}
}

/// Helper function to run a test with a RabbitMQ container
pub async fn with_rabbitmq<F, Fut>(f: F) -> Result<(), Box<dyn std::error::Error>>
where
	F: FnOnce(RabbitMQContainer) -> Fut,
	Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
	let container = RabbitMQContainer::new().await;
	f(container).await
}

/// Mailpit test container for SMTP testing
pub struct MailpitContainer {
	#[allow(dead_code)]
	container: ContainerAsync<GenericImage>,
	host: String,
	smtp_port: u16,
	http_port: u16,
}

/// Helper function to start a Mailpit container
///
/// Returns a tuple of (container, smtp_url, http_url).
pub async fn start_mailpit() -> (MailpitContainer, String, String) {
	let container = MailpitContainer::new().await;
	let smtp_url = container.smtp_url();
	let http_url = container.http_url();
	(container, smtp_url, http_url)
}

impl MailpitContainer {
	/// Create a new Mailpit container
	pub async fn new() -> Self {
		use testcontainers::core::IntoContainerPort;

		// Enable --smtp-auth-accept-any and --smtp-auth-allow-insecure for testing
		// These options allow any authentication credentials and permit auth over plain text
		let image = GenericImage::new("axllent/mailpit", "latest")
			.with_exposed_port(1025.tcp()) // SMTP port
			.with_exposed_port(8025.tcp()) // HTTP API/UI port
			.with_cmd(["--smtp-auth-accept-any", "--smtp-auth-allow-insecure"]);

		let container = AsyncRunner::start(image)
			.await
			.expect("Failed to start Mailpit container");

		// Mailpit SMTP port (1025) and HTTP API/UI port (8025)
		let smtp_port = container
			.get_host_port_ipv4(1025)
			.await
			.expect("Mailpit SMTP container port should be available after startup");
		let http_port = container
			.get_host_port_ipv4(8025)
			.await
			.expect("Mailpit HTTP container port should be available after startup");

		let mailpit_container = Self {
			container,
			host: "localhost".to_string(),
			smtp_port,
			http_port,
		};

		// Wait for Mailpit to be ready
		mailpit_container
			.wait_until_ready()
			.await
			.expect("Mailpit container failed to become ready");

		mailpit_container
	}

	/// Wait for Mailpit server to be ready
	async fn wait_until_ready(&self) -> Result<(), Box<dyn std::error::Error>> {
		use tokio::time::{Duration, sleep};

		let http_url = format!("{}/api/v1/messages", self.http_url());

		// Try to access Mailpit HTTP API with retries (max 30 attempts, ~15 seconds total)
		for attempt in 1..=30 {
			match reqwest::get(&http_url).await {
				Ok(response) if response.status().is_success() => {
					return Ok(());
				}
				Ok(response) if attempt < 30 => {
					eprintln!(
						"Mailpit HTTP check attempt {}/30 failed with status: {}",
						attempt,
						response.status()
					);
					sleep(Duration::from_millis(500)).await;
				}
				Ok(response) => {
					return Err(format!(
						"Mailpit HTTP API not ready after 30 attempts, last status: {}",
						response.status()
					)
					.into());
				}
				Err(e) if attempt < 30 => {
					eprintln!("Mailpit HTTP check attempt {}/30 failed: {}", attempt, e);
					sleep(Duration::from_millis(500)).await;
				}
				Err(e) => {
					return Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::ConnectionRefused,
						format!("Mailpit failed to become ready after 30 attempts: {}", e),
					)));
				}
			}
		}

		Ok(())
	}

	/// Get the SMTP URL for Mailpit
	pub fn smtp_url(&self) -> String {
		format!("smtp://{}:{}", self.host, self.smtp_port)
	}

	/// Get the HTTP API/UI URL for Mailpit
	pub fn http_url(&self) -> String {
		format!("http://{}:{}", self.host, self.http_port)
	}

	/// Get the SMTP port
	pub fn smtp_port(&self) -> u16 {
		self.smtp_port
	}

	/// Get the HTTP API/UI port
	pub fn http_port(&self) -> u16 {
		self.http_port
	}
}

/// Helper function to run a test with a Mailpit container
pub async fn with_mailpit<F, Fut>(f: F) -> Result<(), Box<dyn std::error::Error>>
where
	F: FnOnce(MailpitContainer) -> Fut,
	Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
	let container = MailpitContainer::new().await;
	f(container).await
}

/// SQLite test helpers
pub mod sqlite {
	/// Get a SQLite in-memory database URL for testing
	///
	/// This returns a connection URL for an in-memory SQLite database,
	/// which is useful for fast tests that don't require a real database container.
	///
	/// # Example
	/// ```ignore
	/// use reinhardt_test::containers::sqlite::memory_url;
	///
	/// let url = memory_url();
	/// assert_eq!(url, "sqlite::memory:");
	/// ```
	pub fn memory_url() -> &'static str {
		"sqlite::memory:"
	}

	/// Get a SQLite temporary file database URL for testing
	///
	/// Creates a temporary file-based SQLite database. The file is automatically
	/// cleaned up when the test completes (if using proper cleanup).
	///
	/// # Example
	/// ```ignore
	/// use reinhardt_test::containers::sqlite::temp_file_url;
	///
	/// let url = temp_file_url("test_db");
	/// // Use the database...
	/// ```
	pub fn temp_file_url(name: &str) -> String {
		// Validate name to prevent path traversal attacks
		assert!(!name.is_empty(), "temp_file_url: name must not be empty");
		assert!(
			!name.contains(".."),
			"temp_file_url: name must not contain '..' (path traversal)"
		);
		assert!(
			!name.contains('/') && !name.contains('\\'),
			"temp_file_url: name must not contain path separators ('/' or '\\')"
		);
		assert!(
			!name.contains('\0'),
			"temp_file_url: name must not contain null bytes"
		);
		assert!(
			name.chars()
				.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'),
			"temp_file_url: name must contain only alphanumeric characters, hyphens, underscores, or dots"
		);

		format!("sqlite:/tmp/{}.db", name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_rabbitmq_connection_url_uses_default_credentials() {
		// Arrange
		let container = RabbitMQContainer::new().await;

		// Act
		let url = container.connection_url();

		// Assert
		assert!(url.starts_with("amqp://guest:guest@"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_rabbitmq_connection_url_uses_custom_credentials() {
		// Arrange
		let container = RabbitMQContainer::with_credentials("admin", "secret_pass").await;

		// Act
		let url = container.connection_url();

		// Assert
		assert!(url.starts_with("amqp://admin:secret_pass@"));
	}

	#[tokio::test]
	async fn test_postgres_container() {
		with_postgres(|db| async move {
			let url = db.connection_url();
			assert!(url.starts_with("postgres://"));
			assert_eq!(db.database_type(), "postgres");
			Ok(())
		})
		.await
		.unwrap();
	}

	#[tokio::test]
	async fn test_mysql_container() {
		with_mysql(|db| async move {
			let url = db.connection_url();
			assert!(url.starts_with("mysql://"));
			assert_eq!(db.database_type(), "mysql");
			Ok(())
		})
		.await
		.unwrap();
	}

	#[tokio::test]
	async fn test_redis_container() {
		with_redis(|redis| async move {
			let url = redis.connection_url();
			assert!(url.starts_with("redis://"));
			Ok(())
		})
		.await
		.unwrap();
	}

	#[rstest]
	fn test_temp_file_url_accepts_valid_name() {
		// Arrange
		let name = "test_db";

		// Act
		let url = sqlite::temp_file_url(name);

		// Assert
		assert_eq!(url, "sqlite:/tmp/test_db.db");
	}

	#[rstest]
	fn test_temp_file_url_accepts_name_with_dots_and_hyphens() {
		// Arrange
		let name = "my-test.db-v2";

		// Act
		let url = sqlite::temp_file_url(name);

		// Assert
		assert_eq!(url, "sqlite:/tmp/my-test.db-v2.db");
	}

	#[rstest]
	#[should_panic(expected = "must not be empty")]
	fn test_temp_file_url_rejects_empty_name() {
		// Arrange
		let name = "";

		// Act
		sqlite::temp_file_url(name);
	}

	#[rstest]
	#[should_panic(expected = "path traversal")]
	fn test_temp_file_url_rejects_path_traversal() {
		// Arrange
		let name = "../../etc/passwd";

		// Act
		sqlite::temp_file_url(name);
	}

	#[rstest]
	#[should_panic(expected = "path separators")]
	fn test_temp_file_url_rejects_forward_slash() {
		// Arrange
		let name = "foo/bar";

		// Act
		sqlite::temp_file_url(name);
	}

	#[rstest]
	#[should_panic(expected = "path separators")]
	fn test_temp_file_url_rejects_backslash() {
		// Arrange
		let name = "foo\\bar";

		// Act
		sqlite::temp_file_url(name);
	}

	#[rstest]
	#[should_panic(expected = "null bytes")]
	fn test_temp_file_url_rejects_null_bytes() {
		// Arrange
		let name = "test\0db";

		// Act
		sqlite::temp_file_url(name);
	}

	#[rstest]
	#[should_panic(expected = "alphanumeric")]
	fn test_temp_file_url_rejects_special_characters() {
		// Arrange
		let name = "test db!@#";

		// Act
		sqlite::temp_file_url(name);
	}
}
