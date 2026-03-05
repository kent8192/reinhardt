//! Database connection management

use std::sync::Arc;

use super::{
	backend::DatabaseBackend,
	error::Result,
	query_builder::{DeleteBuilder, InsertBuilder, SelectBuilder, UpdateBuilder},
};

#[cfg(feature = "postgres")]
use super::dialect::PostgresBackend;

#[cfg(feature = "sqlite")]
use super::dialect::SqliteBackend;

#[cfg(feature = "mysql")]
use super::dialect::MySqlBackend;

/// Database connection wrapper
#[derive(Clone)]
pub struct DatabaseConnection {
	backend: Arc<dyn DatabaseBackend>,
}

/// Injectable implementation for DatabaseConnection
///
/// DatabaseConnection must be explicitly registered in the DI context using
/// `InjectionContextBuilder::singleton()`. It cannot be auto-injected because
/// it requires runtime configuration (connection URL, pool settings, etc.).
///
/// # Example
///
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_di::{InjectionContext, SingletonScope};
/// use reinhardt_db::backends::DatabaseConnection;
/// use std::sync::Arc;
///
/// // Create and configure the connection
/// let db = DatabaseConnection::connect_postgres("postgres://localhost/mydb")
///     .await
///     .expect("Failed to connect to database");
///
/// // Register in DI context
/// let singleton_scope = Arc::new(SingletonScope::new());
/// let ctx = InjectionContext::builder(singleton_scope)
///     .singleton(db)
///     .build();
///
/// # }
/// ```
#[cfg(feature = "di")]
#[async_trait::async_trait]
impl reinhardt_di::Injectable for DatabaseConnection {
	async fn inject(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		// Try singleton scope first (primary expected location)
		if let Some(conn) = ctx.get_singleton::<Self>() {
			return Ok(std::sync::Arc::try_unwrap(conn).unwrap_or_else(|arc| (*arc).clone()));
		}

		// Try request scope as fallback
		if let Some(conn) = ctx.get_request::<Self>() {
			return Ok(std::sync::Arc::try_unwrap(conn).unwrap_or_else(|arc| (*arc).clone()));
		}

		// Not registered - provide helpful error
		Err(reinhardt_di::DiError::NotRegistered {
			type_name: std::any::type_name::<Self>().to_string(),
			hint: "Use InjectionContextBuilder::singleton(db_connection) to register a \
			       DatabaseConnection. Create it with DatabaseConnection::connect_postgres(), \
			       connect_sqlite(), or connect_mysql()."
				.to_string(),
		})
	}

	async fn inject_uncached(ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		// For DatabaseConnection, inject_uncached behaves the same as inject
		// because we don't support creating new connections on demand
		Self::inject(ctx).await
	}
}

impl DatabaseConnection {
	pub fn new(backend: Arc<dyn DatabaseBackend>) -> Self {
		Self { backend }
	}

	#[cfg(feature = "postgres")]
	pub async fn connect_postgres(url: &str) -> Result<Self> {
		Self::connect_postgres_with_pool_size(url, None).await
	}

	#[cfg(feature = "postgres")]
	pub async fn connect_postgres_with_pool_size(
		url: &str,
		pool_size: Option<u32>,
	) -> Result<Self> {
		use sqlx::postgres::PgPoolOptions;
		use std::time::Duration;

		// Priority: explicit argument > environment variable > default
		let max_connections = pool_size
			.or_else(|| {
				std::env::var("DATABASE_POOL_MAX_CONNECTIONS")
					.ok()
					.and_then(|v| v.parse::<u32>().ok())
			})
			.unwrap_or(20); // Increased default from 10 to 20 for better concurrency

		let pool = PgPoolOptions::new()
			.max_connections(max_connections)
			.min_connections(1) // Maintain at least 1 connection
			.acquire_timeout(Duration::from_secs(10)) // Increased from 3s to 10s for busy pools
			.idle_timeout(Some(Duration::from_secs(10))) // Close idle connections after 10s
			.max_lifetime(Some(Duration::from_secs(30 * 60))) // Close connections after 30 minutes
			.connect(url)
			.await?;

		Ok(Self {
			backend: Arc::new(PostgresBackend::new(pool)),
		})
	}

	/// Connect to PostgreSQL with automatic database creation if it doesn't exist.
	///
	/// This method first attempts to connect to the specified database. If the connection
	/// fails due to the database not existing, it will:
	/// 1. Connect to the default `postgres` database
	/// 2. Create the target database
	/// 3. Reconnect to the newly created database
	///
	/// # Arguments
	///
	/// * `url` - PostgreSQL connection URL (e.g., "postgres://user:pass@localhost/mydb")
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	///
	/// // Will create 'mydb' if it doesn't exist
	/// let conn = DatabaseConnection::connect_postgres_or_create(
	///     "postgres://postgres@localhost/mydb"
	/// ).await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "postgres")]
	pub async fn connect_postgres_or_create(url: &str) -> Result<Self> {
		Self::connect_postgres_or_create_with_pool_size(url, None).await
	}

	/// Connect to PostgreSQL with automatic database creation and custom pool size.
	///
	/// See [`Self::connect_postgres_or_create`] for details on automatic database creation.
	#[cfg(feature = "postgres")]
	pub async fn connect_postgres_or_create_with_pool_size(
		url: &str,
		pool_size: Option<u32>,
	) -> Result<Self> {
		// First try normal connection
		match Self::connect_postgres_with_pool_size(url, pool_size).await {
			Ok(conn) => return Ok(conn),
			Err(e) => {
				// Check if error indicates database doesn't exist
				let error_str = format!("{:?}", e);
				if !error_str.contains("does not exist")
					&& !error_str.contains("database")
					&& !error_str.contains("3D000")
				{
					// Not a "database doesn't exist" error, propagate it
					return Err(e);
				}
				// Database doesn't exist, try to create it
			}
		}

		// Parse the URL to extract database name
		let (admin_url, db_name) = Self::parse_postgres_url_for_creation(url)?;

		// Connect to default postgres database
		use sqlx::postgres::PgPoolOptions;
		use std::time::Duration;

		let admin_pool = PgPoolOptions::new()
			.max_connections(1)
			.acquire_timeout(Duration::from_secs(10))
			.connect(&admin_url)
			.await
			.map_err(|e| {
				super::error::DatabaseError::ConnectionError(format!(
					"Failed to connect to postgres database for auto-creation: {}",
					e
				))
			})?;

		// Create the database (escape double quotes to prevent SQL injection)
		let create_sql = format!("CREATE DATABASE \"{}\"", db_name.replace('"', "\"\""));
		sqlx::query(&create_sql)
			.execute(&admin_pool)
			.await
			.map_err(|e| {
				super::error::DatabaseError::QueryError(format!(
					"Failed to create database '{}': {}",
					db_name, e
				))
			})?;

		// Close admin connection
		admin_pool.close().await;

		// Now connect to the newly created database
		Self::connect_postgres_with_pool_size(url, pool_size).await
	}

	/// Parse a PostgreSQL URL and return an admin URL (pointing to 'postgres' db) and the target database name.
	#[cfg(feature = "postgres")]
	fn parse_postgres_url_for_creation(url: &str) -> Result<(String, String)> {
		// Parse URL like: postgres://user:pass@host:port/dbname?params
		// We need to extract dbname and create a URL pointing to 'postgres' database

		// Handle both postgres:// and postgresql:// schemes
		let url_without_scheme = url
			.strip_prefix("postgres://")
			.or_else(|| url.strip_prefix("postgresql://"))
			.ok_or_else(|| {
				super::error::DatabaseError::ConnectionError(
					"Invalid PostgreSQL URL: must start with postgres:// or postgresql://"
						.to_string(),
				)
			})?;

		// Split at '?' to separate query params
		let (path_part, query_part) = match url_without_scheme.find('?') {
			Some(pos) => (&url_without_scheme[..pos], Some(&url_without_scheme[pos..])),
			None => (url_without_scheme, None),
		};

		// Find the last '/' which separates host:port from database name
		let last_slash_pos = path_part.rfind('/').ok_or_else(|| {
			super::error::DatabaseError::ConnectionError(
				"Invalid PostgreSQL URL: no database name found".to_string(),
			)
		})?;

		let host_part = &path_part[..last_slash_pos];
		let db_name = &path_part[last_slash_pos + 1..];

		if db_name.is_empty() {
			return Err(super::error::DatabaseError::ConnectionError(
				"Invalid PostgreSQL URL: database name is empty".to_string(),
			));
		}

		// Construct admin URL with 'postgres' database
		let admin_url = match query_part {
			Some(params) => format!("postgres://{}/postgres{}", host_part, params),
			None => format!("postgres://{}/postgres", host_part),
		};

		Ok((admin_url, db_name.to_string()))
	}

	#[cfg(feature = "sqlite")]
	pub async fn connect_sqlite(url: &str) -> Result<Self> {
		use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
		use std::path::Path;
		use std::str::FromStr;

		// Handle in-memory database
		if url == "sqlite::memory:" {
			let pool = SqlitePool::connect(url).await?;
			return Ok(Self {
				backend: Arc::new(SqliteBackend::new(pool)),
			});
		}

		// Extract file path from URL and convert to absolute path
		let file_path = if url.starts_with("sqlite:///") {
			// Absolute path: sqlite:///path/to/db.sqlite3
			url.trim_start_matches("sqlite:///").to_string()
		} else if url.starts_with("sqlite://") {
			// Relative path: sqlite://path/to/db.sqlite3
			// Convert to absolute path
			let rel_path = url.trim_start_matches("sqlite://");
			std::env::current_dir()
				.map_err(|e| {
					super::error::DatabaseError::ConnectionError(format!(
						"Failed to get current directory: {}",
						e
					))
				})?
				.join(rel_path)
				.to_string_lossy()
				.to_string()
		} else if url.starts_with("sqlite:") {
			// sqlite:path/to/db.sqlite3 (relative path format)
			// Convert to absolute path
			let rel_path = url.trim_start_matches("sqlite:");
			std::env::current_dir()
				.map_err(|e| {
					super::error::DatabaseError::ConnectionError(format!(
						"Failed to get current directory: {}",
						e
					))
				})?
				.join(rel_path)
				.to_string_lossy()
				.to_string()
		} else {
			url.to_string()
		};

		// Normalize the path (remove .. and . components)
		let db_path = Path::new(&file_path);
		let normalized_path = if db_path.exists() {
			// If file exists, canonicalize to get absolute path
			db_path.canonicalize().map_err(|e| {
				super::error::DatabaseError::ConnectionError(format!(
					"Failed to canonicalize path {}: {}",
					db_path.display(),
					e
				))
			})?
		} else {
			// If file doesn't exist, use the path as-is but ensure it's absolute
			if db_path.is_absolute() {
				db_path.to_path_buf()
			} else {
				// Convert relative path to absolute
				std::env::current_dir()
					.map_err(|e| {
						super::error::DatabaseError::ConnectionError(format!(
							"Failed to get current directory: {}",
							e
						))
					})?
					.join(db_path)
			}
		};

		// Create parent directory if it doesn't exist
		if let Some(parent) = normalized_path.parent()
			&& !parent.as_os_str().is_empty()
			&& !parent.exists()
		{
			std::fs::create_dir_all(parent).map_err(|e| {
				super::error::DatabaseError::ConnectionError(format!(
					"Failed to create database directory {}: {}",
					parent.display(),
					e
				))
			})?;
		}

		// Use absolute path with sqlite:/// format
		// On Windows, we need to handle the path separator
		let path_str = normalized_path.to_string_lossy().replace('\\', "/");
		let absolute_url = format!("sqlite:///{}", path_str);

		// Use SqliteConnectOptions with create_if_missing enabled
		let options = SqliteConnectOptions::from_str(&absolute_url)
			.map_err(|e| {
				super::error::DatabaseError::ConnectionError(format!(
					"Invalid SQLite URL '{}': {}",
					absolute_url, e
				))
			})?
			.create_if_missing(true);

		let pool = SqlitePool::connect_with(options).await?;

		Ok(Self {
			backend: Arc::new(SqliteBackend::new(pool)),
		})
	}

	#[cfg(feature = "sqlite")]
	pub fn from_sqlite_pool(pool: sqlx::SqlitePool) -> Self {
		Self {
			backend: Arc::new(SqliteBackend::new(pool)),
		}
	}

	#[cfg(feature = "mysql")]
	pub async fn connect_mysql(url: &str) -> Result<Self> {
		use sqlx::MySqlPool;
		let pool = MySqlPool::connect(url).await?;
		Ok(Self {
			backend: Arc::new(MySqlBackend::new(pool)),
		})
	}

	pub fn backend(&self) -> Arc<dyn DatabaseBackend> {
		self.backend.clone()
	}

	/// Get the database type
	pub fn database_type(&self) -> super::types::DatabaseType {
		self.backend.database_type()
	}

	pub fn insert(&self, table: impl Into<String>) -> InsertBuilder {
		InsertBuilder::new(self.backend.clone(), table)
	}

	pub fn update(&self, table: impl Into<String>) -> UpdateBuilder {
		UpdateBuilder::new(self.backend.clone(), table)
	}

	pub fn select(&self) -> SelectBuilder {
		SelectBuilder::new(self.backend.clone())
	}

	pub fn delete(&self, table: impl Into<String>) -> DeleteBuilder {
		DeleteBuilder::new(self.backend.clone(), table)
	}

	/// Get database URL from environment variable or settings files
	///
	/// This function first checks the `DATABASE_URL` environment variable.
	/// If not found, it attempts to load database configuration from settings files
	/// in the `settings/` directory.
	///
	/// # Arguments
	///
	/// * `base_dir` - Base directory for the project (defaults to current directory if None)
	///
	/// # Returns
	///
	/// Returns the database URL string, or an error if neither environment variable
	/// nor settings configuration is found.
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	///
	/// let url = DatabaseConnection::get_database_url_from_env_or_settings(None)?;
	/// let conn = DatabaseConnection::connect_sqlite(&url).await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "settings")]
	pub fn get_database_url_from_env_or_settings(
		base_dir: Option<std::path::PathBuf>,
	) -> Result<String> {
		use std::env;

		// First, try to get from environment variable
		if let Ok(url) = env::var("DATABASE_URL") {
			return Ok(url);
		}

		// If not found, try to load from settings files
		let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
		let profile = reinhardt_conf::settings::profile::Profile::parse(&profile_str);

		let base_dir = base_dir.unwrap_or_else(|| {
			env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
		});
		let settings_dir = base_dir.join("settings");

		// Try to load settings
		let merged = reinhardt_conf::settings::builder::SettingsBuilder::new()
			.profile(profile)
			.add_source(
				reinhardt_conf::settings::sources::DefaultSource::new()
					.with_value("debug", serde_json::Value::Bool(false))
					.with_value(
						"language_code",
						serde_json::Value::String("en-us".to_string()),
					)
					.with_value("time_zone", serde_json::Value::String("UTC".to_string())),
			)
			.add_source(
				reinhardt_conf::settings::sources::LowPriorityEnvSource::new()
					.with_prefix("REINHARDT_"),
			)
			.add_source(reinhardt_conf::settings::sources::TomlFileSource::new(
				settings_dir.join("base.toml"),
			))
			.add_source(reinhardt_conf::settings::sources::TomlFileSource::new(
				settings_dir.join(format!("{}.toml", profile_str)),
			))
			.build()
			.map_err(|e| {
				super::error::DatabaseError::ConnectionError(format!(
					"Failed to load settings: {}. Please ensure settings files exist in the settings/ directory.",
					e
				))
			})?;

		// Try to get database configuration directly from merged settings
		// TOML [database] section maps to "database" key as an object
		let db_config: reinhardt_conf::settings::DatabaseConfig = {
			// First, check if "database" key exists as raw value
			if let Some(db_val) = merged.get_raw("database") {
				// Try to deserialize as DatabaseConfig
				serde_json::from_value(db_val.clone())
					.ok()
					.or_else(|| {
						// If direct deserialization fails, try to extract from object
						if let serde_json::Value::Object(db_map) = db_val {
							// Try to construct DatabaseConfig from the object fields
							let engine = db_map
								.get("engine")
								.and_then(|v| v.as_str())
								.unwrap_or("sqlite")
								.to_string();
							let name = db_map
								.get("name")
								.and_then(|v| v.as_str())
								.map(|s| s.to_string())
								.unwrap_or_else(|| "db.sqlite3".to_string());

							let mut config =
								reinhardt_conf::settings::DatabaseConfig::new(engine, name);
							if let Some(user) = db_map
								.get("user")
								.and_then(|v| v.as_str())
							{
								config = config.with_user(user);
							}
							if let Some(password) = db_map
								.get("password")
								.and_then(|v| v.as_str())
							{
								config = config.with_password(password);
							}
							if let Some(host) = db_map
								.get("host")
								.and_then(|v| v.as_str())
							{
								config = config.with_host(host);
							}
							if let Some(port) = db_map
								.get("port")
								.and_then(|v| v.as_u64())
							{
								config = config.with_port(port as u16);
							}
							Some(config)
						} else {
							None
						}
					})
			} else {
				// Try to get from "databases.default" or "databases.database"
				merged
					.get_optional::<serde_json::Value>("databases")
					.and_then(|dbs| {
						if let serde_json::Value::Object(dbs_map) = dbs {
							// Try "default" first, then "database"
							dbs_map
								.get("default")
								.or_else(|| dbs_map.get("database"))
								.and_then(|db_val| serde_json::from_value(db_val.clone()).ok())
						} else {
							None
						}
					})
			}
		}
		.ok_or_else(|| {
			super::error::DatabaseError::ConnectionError(
				"Database configuration not found in settings. Please configure [database] in your settings file or set DATABASE_URL environment variable.".to_string(),
			)
		})?;

		Ok(db_config.to_url())
	}

	pub async fn execute(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<super::types::QueryResult> {
		self.backend.execute(sql, params).await
	}

	pub async fn fetch_one(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<super::types::Row> {
		self.backend.fetch_one(sql, params).await
	}

	pub async fn fetch_all(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<Vec<super::types::Row>> {
		self.backend.fetch_all(sql, params).await
	}

	pub async fn fetch_optional(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<Option<super::types::Row>> {
		self.backend.fetch_optional(sql, params).await
	}

	/// Begin a database transaction and return a dedicated executor
	///
	/// This method acquires a dedicated database connection and begins a
	/// transaction on it. All queries executed through the returned
	/// `TransactionExecutor` are guaranteed to run on the same physical
	/// connection, ensuring proper transaction isolation.
	///
	/// # Returns
	///
	/// A boxed `TransactionExecutor` that holds the dedicated connection
	/// and provides methods for executing queries within the transaction.
	///
	/// # Example
	///
	/// ```no_run
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	///
	/// let conn = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await?;
	/// let mut tx = conn.begin().await?;
	///
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Alice".into()]).await?;
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin(&self) -> Result<Box<dyn super::types::TransactionExecutor>> {
		self.backend.begin().await
	}

	/// Begin a transaction with a specific isolation level
	///
	/// # Examples
	///
	/// ```no_run
	/// # async fn example() -> reinhardt_db::backends::error::Result<()> {
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	/// use reinhardt_db::backends::types::IsolationLevel;
	///
	/// let conn = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await?;
	/// let mut tx = conn.begin_with_isolation(IsolationLevel::Serializable).await?;
	///
	/// tx.execute("INSERT INTO users (name) VALUES ($1)", vec!["Alice".into()]).await?;
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin_with_isolation(
		&self,
		level: super::types::IsolationLevel,
	) -> Result<Box<dyn super::types::TransactionExecutor>> {
		self.backend.begin_with_isolation(level).await
	}

	#[cfg(feature = "postgres")]
	pub fn into_postgres(&self) -> Option<sqlx::PgPool> {
		self.backend
			.as_any()
			.downcast_ref::<super::dialect::PostgresBackend>()
			.map(|backend| backend.pool().clone())
	}

	#[cfg(feature = "sqlite")]
	pub fn into_sqlite(&self) -> Option<sqlx::SqlitePool> {
		self.backend
			.as_any()
			.downcast_ref::<super::dialect::SqliteBackend>()
			.map(|backend| backend.pool().clone())
	}

	#[cfg(feature = "mysql")]
	pub fn into_mysql(&self) -> Option<sqlx::MySqlPool> {
		self.backend
			.as_any()
			.downcast_ref::<super::dialect::MySqlBackend>()
			.map(|backend| backend.pool().clone())
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	/// Helper to build a CREATE DATABASE SQL statement with proper identifier escaping.
	/// Mirrors the escaping logic used in `connect_postgres_or_create_with_pool_size`.
	fn build_create_database_sql(db_name: &str) -> String {
		format!("CREATE DATABASE \"{}\"", db_name.replace('"', "\"\""))
	}

	#[rstest]
	fn test_create_database_sql_normal_name() {
		// Arrange
		let db_name = "my_database";

		// Act
		let sql = build_create_database_sql(db_name);

		// Assert
		assert_eq!(sql, "CREATE DATABASE \"my_database\"");
	}

	#[rstest]
	fn test_create_database_sql_injection_with_double_quotes() {
		// Arrange: attacker tries to break out with double quotes
		let db_name = "test\"; DROP TABLE users; --";

		// Act
		let sql = build_create_database_sql(db_name);

		// Assert: double quotes are escaped by doubling
		assert_eq!(sql, "CREATE DATABASE \"test\"\"; DROP TABLE users; --\"");
		// The escaped SQL treats the entire string as a single identifier,
		// preventing the attacker from injecting additional SQL statements
	}

	#[rstest]
	fn test_create_database_sql_injection_with_multiple_quotes() {
		// Arrange: attacker uses multiple double-quote escape attempts
		let db_name = "db\"\"injection";

		// Act
		let sql = build_create_database_sql(db_name);

		// Assert: each quote is doubled
		assert_eq!(sql, "CREATE DATABASE \"db\"\"\"\"injection\"");
	}

	#[cfg(feature = "postgres")]
	#[rstest]
	fn test_parse_postgres_url_extracts_db_name() {
		// Arrange
		let url = "postgres://user:pass@localhost:5432/testdb";

		// Act
		let (admin_url, db_name) =
			super::DatabaseConnection::parse_postgres_url_for_creation(url).unwrap();

		// Assert
		assert_eq!(db_name, "testdb");
		assert_eq!(admin_url, "postgres://user:pass@localhost:5432/postgres");
	}
}
