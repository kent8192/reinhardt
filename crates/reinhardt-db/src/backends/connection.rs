//! Database connection management

use std::sync::Arc;

use super::{
	backend::DatabaseBackend,
	error::Result,
	query_builder::{DeleteBuilder, InsertBuilder, SelectBuilder, UpdateBuilder},
	types::{
		DatabaseType, QueryResult, QueryValue, Row, RowLockCapabilities, RowStream,
		TransactionExecutor,
	},
};

#[cfg(any(feature = "postgres", feature = "sqlite", feature = "mysql"))]
use super::error::map_sqlx_error;
use super::error::{DatabaseError, DatabaseErrorKind};

#[cfg(feature = "postgres")]
use super::dialect::PostgresBackend;

/// SQLSTATE code for "invalid_catalog_name" (database does not exist)
#[cfg(feature = "postgres")]
const SQLSTATE_INVALID_CATALOG_NAME: &str = "3D000";

#[cfg(feature = "postgres")]
fn map_postgres_initial_connect_error(error: sqlx::Error) -> DatabaseError {
	match error {
		sqlx::Error::PoolTimedOut => DatabaseError::new(
			DatabaseErrorKind::Timeout,
			"Initial PostgreSQL connection timed out",
		),
		error => map_sqlx_error(error),
	}
}

fn parse_server_version(version: &str) -> Option<(u16, u16, u16)> {
	let start = version.find(|character: char| character.is_ascii_digit())?;
	let mut parts = version[start..]
		.split(|character: char| !character.is_ascii_digit())
		.filter(|part| !part.is_empty());
	Some((
		parts.next()?.parse().ok()?,
		parts.next()?.parse().ok()?,
		parts.next().and_then(|part| part.parse().ok()).unwrap_or(0),
	))
}

fn postgres_row_lock_capabilities(
	version: Option<&str>,
	is_cockroachdb: bool,
) -> RowLockCapabilities {
	if is_cockroachdb {
		return RowLockCapabilities::cockroachdb();
	}
	version
		.and_then(parse_server_version)
		.map(|(major, minor, _)| RowLockCapabilities::postgres_for_version(major, minor))
		.unwrap_or_else(RowLockCapabilities::postgres)
}

fn mysql_row_lock_capabilities(version: Option<&str>) -> RowLockCapabilities {
	let Some(version) = version else {
		return RowLockCapabilities::mysql();
	};
	let components: Vec<_> = version.split('-').collect();
	let normalized_version = components
		.iter()
		.position(|component| component.to_ascii_lowercase().contains("mariadb"))
		.and_then(|mariadb| mariadb.checked_sub(1))
		.and_then(|version_component| components.get(version_component).copied())
		.unwrap_or(version);
	let Some((major, minor, patch)) = parse_server_version(normalized_version) else {
		return RowLockCapabilities::mysql();
	};
	if version.to_ascii_lowercase().contains("mariadb") {
		RowLockCapabilities::mariadb_for_version(major, minor, patch)
	} else {
		RowLockCapabilities::mysql_for_version(major, minor, patch)
	}
}

#[cfg(feature = "sqlite")]
use super::dialect::SqliteBackend;

#[cfg(feature = "mysql")]
use super::dialect::MySqlBackend;

/// Database connection wrapper
#[derive(Clone)]
pub struct DatabaseConnection {
	backend: Arc<dyn DatabaseBackend>,
	/// True when the underlying server is CockroachDB rather than real PostgreSQL.
	///
	/// CockroachDB is PostgreSQL wire-compatible and shares the `PostgresBackend`,
	/// so `database_type()` returns `DatabaseType::Postgres` for both. A few
	/// migration paths (notably the schema-bootstrap lock — `pg_advisory_lock`
	/// is not implemented on CockroachDB, see issue #4642) need to behave
	/// differently. This flag is set at connection time via a `SELECT version()`
	/// probe and is `false` for any non-Postgres backend.
	is_cockroachdb: bool,
	row_lock_capabilities: Option<RowLockCapabilities>,
}

struct FlavoredTransactionExecutor {
	inner: Box<dyn TransactionExecutor>,
	is_cockroachdb: bool,
	row_lock_capabilities: Option<RowLockCapabilities>,
}

#[async_trait::async_trait]
impl TransactionExecutor for FlavoredTransactionExecutor {
	fn backend(&self) -> DatabaseType {
		self.inner.backend()
	}

	fn is_cockroachdb(&self) -> bool {
		self.is_cockroachdb
	}

	fn row_lock_capabilities(&self) -> super::types::RowLockCapabilities {
		self.row_lock_capabilities
			.unwrap_or_else(|| self.inner.row_lock_capabilities())
	}

	fn supports_pgvector_error_hints(&self) -> bool {
		self.inner.supports_pgvector_error_hints()
	}

	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		self.inner.execute(sql, params).await
	}

	async fn execute_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<QueryResult> {
		self.inner.execute_with_context(sql, params, context).await
	}

	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		self.inner.fetch_one(sql, params).await
	}

	async fn fetch_one_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Row> {
		self.inner
			.fetch_one_with_context(sql, params, context)
			.await
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		self.inner.fetch_all(sql, params).await
	}

	async fn fetch_all_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Vec<Row>> {
		self.inner
			.fetch_all_with_context(sql, params, context)
			.await
	}

	fn fetch_stream<'a>(
		&'a mut self,
		sql: String,
		params: Vec<QueryValue>,
		chunk_size: usize,
	) -> Result<RowStream<'a>> {
		self.inner.fetch_stream(sql, params, chunk_size)
	}

	fn fetch_stream_with_context<'a>(
		&'a mut self,
		sql: String,
		params: Vec<QueryValue>,
		chunk_size: usize,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<RowStream<'a>> {
		self.inner
			.fetch_stream_with_context(sql, params, chunk_size, context)
	}

	async fn fetch_optional(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		self.inner.fetch_optional(sql, params).await
	}

	async fn fetch_optional_with_context(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Option<Row>> {
		self.inner
			.fetch_optional_with_context(sql, params, context)
			.await
	}

	async fn commit(self: Box<Self>) -> Result<()> {
		self.inner.commit().await
	}

	async fn rollback(self: Box<Self>) -> Result<()> {
		self.inner.rollback().await
	}

	async fn savepoint(&mut self, name: &str) -> Result<()> {
		self.inner.savepoint(name).await
	}

	async fn release_savepoint(&mut self, name: &str) -> Result<()> {
		self.inner.release_savepoint(name).await
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
		self.inner.rollback_to_savepoint(name).await
	}
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
	/// Connects to the backend selected by the URL scheme.
	pub async fn connect(url: &str) -> Result<Self> {
		let postgres = url.starts_with("postgres://") || url.starts_with("postgresql://");
		let mysql = url.starts_with("mysql://");
		let sqlite = url.starts_with("sqlite://") || url.starts_with("sqlite:");

		#[cfg(feature = "postgres")]
		if postgres {
			return Self::connect_postgres(url).await;
		}

		#[cfg(feature = "mysql")]
		if mysql {
			return Self::connect_mysql(url).await;
		}

		#[cfg(feature = "sqlite")]
		if sqlite {
			return Self::connect_sqlite(url).await;
		}

		let missing_feature = if postgres {
			Some("postgres")
		} else if mysql {
			Some("mysql")
		} else if sqlite {
			Some("sqlite")
		} else {
			None
		};
		if let Some(feature) = missing_feature {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				format!("Database backend not compiled in. Enable the '{feature}' feature."),
			)
			.into());
		}
		Err(DatabaseError::new(
			DatabaseErrorKind::Configuration,
			format!("Unsupported database URL scheme: {url}"),
		)
		.into())
	}

	/// Creates a new instance.
	///
	/// Defaults to `is_cockroachdb = false`, which is the correct choice when
	/// the caller has not (or cannot) probe the server. If the supplied backend
	/// is known to be CockroachDB, use [`Self::new_with_flavor`] instead so the
	/// migration recorder routes through the sentinel-row lock path rather than
	/// `pg_advisory_lock` (issue #4642).
	pub fn new(backend: Arc<dyn DatabaseBackend>) -> Self {
		Self::new_with_flavor(backend, false)
	}

	/// Creates a new instance with an explicit CockroachDB flavor flag.
	///
	/// Use this when wrapping an externally constructed Postgres backend whose
	/// flavor is already known — e.g. tests that mount a CockroachDB pool, or
	/// adapters that pre-probe `SELECT version()` themselves.
	pub fn new_with_flavor(backend: Arc<dyn DatabaseBackend>, is_cockroachdb: bool) -> Self {
		let row_lock_capabilities = is_cockroachdb.then_some(RowLockCapabilities::cockroachdb());
		Self {
			backend,
			is_cockroachdb,
			row_lock_capabilities,
		}
	}

	/// Creates a new instance with an explicit row-lock capability profile.
	///
	/// This is useful for custom backends whose server version is known by the
	/// caller. Without an explicit profile, transaction executors retain their
	/// own capability reporting.
	pub fn new_with_flavor_and_row_lock_capabilities(
		backend: Arc<dyn DatabaseBackend>,
		is_cockroachdb: bool,
		row_lock_capabilities: RowLockCapabilities,
	) -> Self {
		Self {
			backend,
			is_cockroachdb,
			row_lock_capabilities: Some(row_lock_capabilities),
		}
	}

	#[cfg(feature = "postgres")]
	/// Connects to postgres.
	pub async fn connect_postgres(url: &str) -> Result<Self> {
		Self::connect_postgres_with_pool_size(url, None).await
	}

	#[cfg(feature = "postgres")]
	/// Connects to postgres with pool size.
	pub async fn connect_postgres_with_pool_size(
		url: &str,
		pool_size: Option<u32>,
	) -> Result<Self> {
		let pool = Self::build_postgres_pool(url, pool_size)
			.await
			.map_err(map_postgres_initial_connect_error)?;
		let version = Self::probe_postgres_version(&pool).await;
		let is_cockroachdb = version
			.as_deref()
			.is_some_and(|value| value.starts_with("CockroachDB"));
		let row_lock_capabilities =
			postgres_row_lock_capabilities(version.as_deref(), is_cockroachdb);

		Ok(Self {
			backend: Arc::new(PostgresBackend::new(pool)),
			is_cockroachdb,
			row_lock_capabilities: Some(row_lock_capabilities),
		})
	}

	/// Probe whether the connected PostgreSQL-protocol server is CockroachDB.
	///
	/// CockroachDB's `SELECT version()` response always begins with the literal
	/// string `CockroachDB` (e.g. `CockroachDB CCL v23.1.0 ...`), while
	/// PostgreSQL's begins with `PostgreSQL`. The probe is best-effort: any
	/// failure (network blip, RBAC denying `version()`) is treated as
	/// "not CockroachDB" so the regular PostgreSQL path stays the default.
	///
	/// The comparison is pushed into SQL (`LIKE 'CockroachDB%'`) so the server
	/// returns a single `bool` instead of streaming the full version string —
	/// no allocation and no client-side sensitivity to whitespace or casing.
	///
	/// Used to drive the migration-lock dispatch in `MigrationRecorder`
	/// (issue #4642: CockroachDB does not implement `pg_advisory_lock`).
	#[cfg(feature = "postgres")]
	async fn probe_postgres_version(pool: &sqlx::PgPool) -> Option<String> {
		sqlx::query_scalar::<_, String>("SELECT version()")
			.fetch_one(pool)
			.await
			.ok()
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

	/// Build a PostgreSQL pool with the given URL and pool size.
	///
	/// Returns the raw `sqlx::Error` on failure so callers can inspect
	/// SQLSTATE codes before converting to `DatabaseError`.
	#[cfg(feature = "postgres")]
	async fn build_postgres_pool(
		url: &str,
		pool_size: Option<u32>,
	) -> std::result::Result<sqlx::PgPool, sqlx::Error> {
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

		PgPoolOptions::new()
			.max_connections(max_connections)
			.min_connections(1) // Maintain at least 1 connection
			.acquire_timeout(Duration::from_secs(10)) // Increased from 3s to 10s for busy pools
			.idle_timeout(Some(Duration::from_secs(10))) // Close idle connections after 10s
			.max_lifetime(Some(Duration::from_secs(30 * 60))) // Close connections after 30 minutes
			.connect(url)
			.await
	}

	/// Connect to PostgreSQL with automatic database creation and custom pool size.
	///
	/// See [`Self::connect_postgres_or_create`] for details on automatic database creation.
	#[cfg(feature = "postgres")]
	pub async fn connect_postgres_or_create_with_pool_size(
		url: &str,
		pool_size: Option<u32>,
	) -> Result<Self> {
		// First try normal connection, keeping the raw sqlx::Error
		// so we can check the SQLSTATE code
		match Self::build_postgres_pool(url, pool_size).await {
			Ok(pool) => {
				let version = Self::probe_postgres_version(&pool).await;
				let is_cockroachdb = version
					.as_deref()
					.is_some_and(|value| value.starts_with("CockroachDB"));
				return Ok(Self {
					backend: Arc::new(PostgresBackend::new(pool)),
					is_cockroachdb,
					row_lock_capabilities: Some(postgres_row_lock_capabilities(
						version.as_deref(),
						is_cockroachdb,
					)),
				});
			}
			Err(e) => {
				// Check if the error is SQLSTATE 3D000 (invalid_catalog_name),
				// which indicates the database does not exist
				let is_db_not_found = matches!(
					&e,
					sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some(SQLSTATE_INVALID_CATALOG_NAME)
				);
				if !is_db_not_found {
					return Err(map_postgres_initial_connect_error(e).into());
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
			.map_err(map_postgres_initial_connect_error)?;

		// Create the database (escape double quotes to prevent SQL injection)
		let create_sql = format!("CREATE DATABASE \"{}\"", db_name.replace('"', "\"\""));
		sqlx::query(&create_sql)
			.execute(&admin_pool)
			.await
			.map_err(map_sqlx_error)?;

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
				DatabaseError::new(
					DatabaseErrorKind::Configuration,
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
			DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"Invalid PostgreSQL URL: no database name found".to_string(),
			)
		})?;

		let host_part = &path_part[..last_slash_pos];
		let db_name = &path_part[last_slash_pos + 1..];

		if db_name.is_empty() {
			return Err(DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"Invalid PostgreSQL URL: database name is empty",
			)
			.into());
		}

		// Construct admin URL with 'postgres' database
		let admin_url = match query_part {
			Some(params) => format!("postgres://{}/postgres{}", host_part, params),
			None => format!("postgres://{}/postgres", host_part),
		};

		Ok((admin_url, db_name.to_string()))
	}

	/// Connects to a SQLite database at the given URL.
	#[cfg(feature = "sqlite")]
	pub async fn connect_sqlite(url: &str) -> Result<Self> {
		use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
		use std::path::Path;
		use std::str::FromStr;

		// Handle in-memory database
		if url == "sqlite::memory:" {
			let pool = SqlitePoolOptions::new()
				.max_connections(1)
				.min_connections(1)
				.idle_timeout(None)
				.max_lifetime(None)
				.connect(url)
				.await
				.map_err(map_sqlx_error)?;
			return Ok(Self {
				backend: Arc::new(SqliteBackend::new(pool)),
				is_cockroachdb: false,
				row_lock_capabilities: Some(RowLockCapabilities::unsupported()),
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
					DatabaseError::new(
						DatabaseErrorKind::Connection,
						format!("Failed to get current directory: {}", e),
					)
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
					DatabaseError::new(
						DatabaseErrorKind::Connection,
						format!("Failed to get current directory: {}", e),
					)
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
				DatabaseError::new(
					DatabaseErrorKind::Connection,
					format!("Failed to canonicalize path {}: {}", db_path.display(), e),
				)
			})?
		} else {
			// If file doesn't exist, use the path as-is but ensure it's absolute
			if db_path.is_absolute() {
				db_path.to_path_buf()
			} else {
				// Convert relative path to absolute
				std::env::current_dir()
					.map_err(|e| {
						DatabaseError::new(
							DatabaseErrorKind::Connection,
							format!("Failed to get current directory: {}", e),
						)
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
				DatabaseError::new(
					DatabaseErrorKind::Connection,
					format!(
						"Failed to create database directory {}: {}",
						parent.display(),
						e
					),
				)
			})?;
		}

		// Use absolute path with sqlite:/// format
		// On Windows, we need to handle the path separator
		let path_str = normalized_path.to_string_lossy().replace('\\', "/");
		let absolute_url = format!("sqlite:///{}", path_str);

		// Use SqliteConnectOptions with create_if_missing enabled
		let options = SqliteConnectOptions::from_str(&absolute_url)
			.map_err(map_sqlx_error)?
			.create_if_missing(true);

		let pool = SqlitePool::connect_with(options)
			.await
			.map_err(map_sqlx_error)?;

		Ok(Self {
			backend: Arc::new(SqliteBackend::new(pool)),
			is_cockroachdb: false,
			row_lock_capabilities: Some(RowLockCapabilities::unsupported()),
		})
	}

	/// Creates a connection from an existing SQLite pool.
	#[cfg(feature = "sqlite")]
	pub fn from_sqlite_pool(pool: sqlx::SqlitePool) -> Self {
		Self {
			backend: Arc::new(SqliteBackend::new(pool)),
			is_cockroachdb: false,
			row_lock_capabilities: Some(RowLockCapabilities::unsupported()),
		}
	}

	/// Connects to a MySQL database at the given URL.
	#[cfg(feature = "mysql")]
	pub async fn connect_mysql(url: &str) -> Result<Self> {
		use sqlx::MySqlPool;
		let pool = MySqlPool::connect(url).await.map_err(map_sqlx_error)?;
		let version = sqlx::query_scalar::<_, String>("SELECT VERSION()")
			.fetch_one(&pool)
			.await
			.ok();
		Ok(Self {
			backend: Arc::new(MySqlBackend::new(pool)),
			is_cockroachdb: false,
			row_lock_capabilities: Some(mysql_row_lock_capabilities(version.as_deref())),
		})
	}

	/// Performs the backend operation.
	pub fn backend(&self) -> Arc<dyn DatabaseBackend> {
		self.backend.clone()
	}

	/// Get the database type
	pub fn database_type(&self) -> super::types::DatabaseType {
		self.backend.database_type()
	}

	/// Returns whether the inner backend supports contextual pgvector hints.
	pub fn supports_pgvector_error_hints(&self) -> bool {
		self.backend.supports_pgvector_error_hints()
	}

	/// Returns true when the underlying server is CockroachDB.
	///
	/// CockroachDB is wire-compatible with PostgreSQL and uses the same
	/// `PostgresBackend`, so `database_type()` returns `DatabaseType::Postgres`
	/// for both. Callers that must dispatch on the *server flavour* (e.g. the
	/// migration-lock path, which cannot use `pg_advisory_lock` on CockroachDB —
	/// see issue #4642) should check this flag first.
	///
	/// The flag is determined at connection time via a single `SELECT version()`
	/// probe and is `false` for any non-Postgres backend.
	pub fn is_cockroachdb(&self) -> bool {
		self.is_cockroachdb
	}

	/// Performs the insert operation.
	pub fn insert(&self, table: impl Into<String>) -> InsertBuilder {
		InsertBuilder::new(self.backend.clone(), table)
	}

	/// Performs the update operation.
	pub fn update(&self, table: impl Into<String>) -> UpdateBuilder {
		UpdateBuilder::new(self.backend.clone(), table)
	}

	/// Performs the select operation.
	pub fn select(&self) -> SelectBuilder {
		SelectBuilder::new(self.backend.clone())
	}

	/// Performs the delete operation.
	pub fn delete(&self, table: impl Into<String>) -> DeleteBuilder {
		DeleteBuilder::new(self.backend.clone(), table)
	}

	/// Resolve a database URL from an already-built composed settings value.
	///
	/// This is the preferred entry point for callers that already hold a
	/// `ProjectSettings` (or any type that implements
	/// [`reinhardt_conf::HasCoreSettings`]). It reads the `default` entry of
	/// `CoreSettings::databases` and converts it to a URL via
	/// [`DatabaseConfig::to_url`](reinhardt_conf::DatabaseConfig::to_url).
	///
	/// The optional `env_override` argument is honored first: if it is
	/// `Some(url)`, that URL is returned verbatim. Pass `None` to skip the
	/// override entirely. To opt into the env-var short circuit, bind the
	/// result of `std::env::var` first so the temporary `String` outlives
	/// the borrow:
	///
	/// ```ignore
	/// let database_url_env = std::env::var("DATABASE_URL").ok();
	/// let url = DatabaseConnection::database_url_from(
	///     settings,
	///     database_url_env.as_deref(),
	/// )?;
	/// ```
	///
	/// # Errors
	///
	/// Returns a `ConnectionError` if the `core.databases.default` entry is
	/// missing from the composed settings. `DatabaseConfig::to_url` itself
	/// is infallible, so a successfully resolved `default` entry always
	/// yields `Ok(_)`.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_db::backends::connection::DatabaseConnection;
	/// # fn doc<S: reinhardt_conf::HasCoreSettings>(settings: &S) {
	/// let url = DatabaseConnection::database_url_from(settings, None)
	///     .expect("database url");
	/// # let _ = url;
	/// # }
	/// ```
	#[cfg(feature = "settings")]
	pub fn database_url_from<S>(settings: &S, env_override: Option<&str>) -> Result<String>
	where
		S: reinhardt_conf::HasCoreSettings + ?Sized,
	{
		if let Some(url) = env_override {
			return Ok(url.to_string());
		}

		let core = settings.core();
		let db_config = core.databases.get("default").ok_or_else(|| {
			DatabaseError::new(
				DatabaseErrorKind::Configuration,
				"Database configuration `core.databases.default` not found in settings."
					.to_string(),
			)
		})?;

		Ok(db_config.to_url())
	}

	/// Executes the operation.
	pub async fn execute(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<super::types::QueryResult> {
		self.backend.execute(sql, params).await
	}

	pub(crate) async fn execute_with_context(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<super::types::QueryResult> {
		self.backend
			.execute_with_context(sql, params, context)
			.await
	}

	/// Fetches one.
	pub async fn fetch_one(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<super::types::Row> {
		self.backend.fetch_one(sql, params).await
	}

	pub(crate) async fn fetch_one_with_context(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<super::types::Row> {
		self.backend
			.fetch_one_with_context(sql, params, context)
			.await
	}

	/// Fetches all.
	pub async fn fetch_all(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<Vec<super::types::Row>> {
		self.backend.fetch_all(sql, params).await
	}

	/// Streams rows without eagerly materializing the result set.
	pub fn fetch_stream(
		&self,
		sql: String,
		params: Vec<super::types::QueryValue>,
		chunk_size: usize,
	) -> Result<RowStream<'_>> {
		self.backend.fetch_stream(sql, params, chunk_size)
	}

	pub(crate) fn fetch_stream_with_context(
		&self,
		sql: String,
		params: Vec<super::types::QueryValue>,
		chunk_size: usize,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<RowStream<'_>> {
		self.backend
			.fetch_stream_with_context(sql, params, chunk_size, context)
	}

	pub(crate) fn supports_row_streaming(&self) -> bool {
		self.backend.supports_row_streaming()
	}

	pub(crate) async fn fetch_all_with_context(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Vec<super::types::Row>> {
		self.backend
			.fetch_all_with_context(sql, params, context)
			.await
	}

	/// Fetches optional.
	pub async fn fetch_optional(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
	) -> Result<Option<super::types::Row>> {
		self.backend.fetch_optional(sql, params).await
	}

	pub(crate) async fn fetch_optional_with_context(
		&self,
		sql: &str,
		params: Vec<super::types::QueryValue>,
		context: Option<super::error::PgvectorOperationKind>,
	) -> Result<Option<super::types::Row>> {
		self.backend
			.fetch_optional_with_context(sql, params, context)
			.await
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
		let inner = self.backend.begin().await?;
		Ok(Box::new(FlavoredTransactionExecutor {
			inner,
			is_cockroachdb: self.is_cockroachdb,
			row_lock_capabilities: self.row_lock_capabilities,
		}))
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
		let inner = self.backend.begin_with_isolation(level).await?;
		Ok(Box::new(FlavoredTransactionExecutor {
			inner,
			is_cockroachdb: self.is_cockroachdb,
			row_lock_capabilities: self.row_lock_capabilities,
		}))
	}

	#[cfg(feature = "postgres")]
	/// Converts into postgres.
	pub fn into_postgres(&self) -> Option<sqlx::PgPool> {
		self.backend
			.as_any()
			.downcast_ref::<super::dialect::PostgresBackend>()
			.map(|backend| backend.pool().clone())
	}

	/// Converts into the underlying SQLite pool, if the backend is SQLite.
	#[cfg(feature = "sqlite")]
	pub fn into_sqlite(&self) -> Option<sqlx::SqlitePool> {
		self.backend
			.as_any()
			.downcast_ref::<super::dialect::SqliteBackend>()
			.map(|backend| backend.pool().clone())
	}

	/// Converts into the underlying MySQL pool, if the backend is MySQL.
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

	#[cfg(feature = "pgvector")]
	struct WrappedPostgresBackend {
		context:
			std::sync::Arc<std::sync::Mutex<Option<super::super::error::PgvectorOperationKind>>>,
	}

	#[cfg(feature = "pgvector")]
	#[async_trait::async_trait]
	impl super::super::backend::DatabaseBackend for WrappedPostgresBackend {
		fn database_type(&self) -> super::super::types::DatabaseType {
			super::super::types::DatabaseType::Postgres
		}

		fn supports_pgvector_error_hints(&self) -> bool {
			true
		}

		fn placeholder(&self, index: usize) -> String {
			format!("${index}")
		}

		fn supports_returning(&self) -> bool {
			true
		}

		fn supports_on_conflict(&self) -> bool {
			true
		}

		async fn execute(
			&self,
			_sql: &str,
			_params: Vec<super::super::types::QueryValue>,
		) -> super::super::error::Result<super::super::types::QueryResult> {
			panic!("contextual connection execution must use the backend context seam")
		}

		async fn execute_with_context(
			&self,
			_sql: &str,
			_params: Vec<super::super::types::QueryValue>,
			context: Option<super::super::error::PgvectorOperationKind>,
		) -> super::super::error::Result<super::super::types::QueryResult> {
			*self
				.context
				.lock()
				.expect("context mutex should not be poisoned") = context;
			Ok(super::super::types::QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}

		async fn fetch_one(
			&self,
			_sql: &str,
			_params: Vec<super::super::types::QueryValue>,
		) -> super::super::error::Result<super::super::types::Row> {
			panic!("wrapped test backend does not fetch rows")
		}

		async fn fetch_all(
			&self,
			_sql: &str,
			_params: Vec<super::super::types::QueryValue>,
		) -> super::super::error::Result<Vec<super::super::types::Row>> {
			panic!("wrapped test backend does not fetch rows")
		}

		async fn fetch_optional(
			&self,
			_sql: &str,
			_params: Vec<super::super::types::QueryValue>,
		) -> super::super::error::Result<Option<super::super::types::Row>> {
			panic!("wrapped test backend does not fetch rows")
		}

		async fn begin(
			&self,
		) -> super::super::error::Result<Box<dyn super::super::types::TransactionExecutor>> {
			panic!("wrapped test backend does not begin transactions")
		}

		fn as_any(&self) -> &dyn std::any::Any {
			self
		}
	}

	#[cfg(feature = "pgvector")]
	#[tokio::test]
	async fn contextual_execution_uses_wrapped_backend_trait_seam() {
		let context = std::sync::Arc::new(std::sync::Mutex::new(None));
		let connection =
			super::DatabaseConnection::new(std::sync::Arc::new(WrappedPostgresBackend {
				context: context.clone(),
			}));

		assert_eq!(
			connection.database_type(),
			super::super::types::DatabaseType::Postgres
		);
		assert!(connection.supports_pgvector_error_hints());

		let result = connection
			.execute_with_context(
				"ALTER TABLE source ADD COLUMN embedding vector(3)",
				Vec::new(),
				Some(super::super::error::PgvectorOperationKind::ColumnType),
			)
			.await
			.expect("wrapped backend should execute contextually");

		assert_eq!(result.rows_affected, 1);
		assert_eq!(
			*context
				.lock()
				.expect("context mutex should not be poisoned"),
			Some(super::super::error::PgvectorOperationKind::ColumnType)
		);
	}

	#[cfg(feature = "postgres")]
	#[test]
	fn postgres_initial_pool_timeout_is_classified_as_timeout() {
		let error = super::map_postgres_initial_connect_error(sqlx::Error::PoolTimedOut);

		assert_eq!(error.kind(), super::DatabaseErrorKind::Timeout);
	}

	#[rstest]
	#[case("PostgreSQL 9.4.26", Some((9, 4, 26)))]
	#[case("8.0.0-rc1", Some((8, 0, 0)))]
	#[case("not a version", None)]
	fn server_version_parser_extracts_numeric_components(
		#[case] version: &str,
		#[case] expected: Option<(u16, u16, u16)>,
	) {
		assert_eq!(super::parse_server_version(version), expected);
	}

	#[test]
	fn maria_db_compatibility_prefix_uses_the_maria_db_version() {
		let capabilities = super::mysql_row_lock_capabilities(Some("5.5.5-10.11.6-MariaDB"));

		assert!(capabilities.nowait);
		assert!(capabilities.skip_locked);
		assert!(!capabilities.targets);
	}

	#[test]
	fn row_lock_capabilities_follow_probed_server_versions() {
		let postgres_94 = super::postgres_row_lock_capabilities(Some("PostgreSQL 9.4.26"), false);
		assert!(postgres_94.update);
		assert!(postgres_94.no_key_update);
		assert!(postgres_94.nowait);
		assert!(!postgres_94.skip_locked);

		let mysql_800 = super::mysql_row_lock_capabilities(Some("8.0.0"));
		assert!(mysql_800.update);
		assert!(!mysql_800.nowait);
		assert!(!mysql_800.skip_locked);
		assert!(!mysql_800.targets);

		let mysql_801 = super::mysql_row_lock_capabilities(Some("8.0.1"));
		assert!(mysql_801.nowait);
		assert!(mysql_801.skip_locked);
		assert!(mysql_801.targets);

		let mariadb_105 = super::mysql_row_lock_capabilities(Some("10.5.23-MariaDB"));
		assert!(mariadb_105.nowait);
		assert!(!mariadb_105.skip_locked);
		assert!(!mariadb_105.targets);

		let mariadb_106 = super::mysql_row_lock_capabilities(Some("10.6.18-MariaDB"));
		assert!(mariadb_106.skip_locked);
		assert!(!mariadb_106.targets);
	}

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

	#[cfg(feature = "postgres")]
	#[rstest]
	#[case("http://localhost/testdb")]
	#[case("postgres://localhost")]
	#[case("postgres://localhost/")]
	fn test_parse_postgres_url_rejects_invalid_configuration(#[case] url: &str) {
		// Act
		let error = super::DatabaseConnection::parse_postgres_url_for_creation(url).unwrap_err();

		// Assert
		assert_eq!(
			error.database_kind(),
			Some(super::DatabaseErrorKind::Configuration)
		);
	}

	#[cfg(feature = "sqlite")]
	#[rstest]
	#[tokio::test]
	async fn connect_selects_sqlite_from_url_scheme() {
		// Act
		let connection = super::DatabaseConnection::connect("sqlite::memory:")
			.await
			.unwrap();

		// Assert
		assert!(connection.into_sqlite().is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn connect_rejects_unknown_url_scheme() {
		// Act
		let Err(error) = super::DatabaseConnection::connect("unknown://localhost/database").await
		else {
			panic!("unknown URL schemes must be rejected");
		};

		// Assert
		assert_eq!(
			error.database_kind(),
			Some(super::DatabaseErrorKind::Configuration)
		);
	}

	#[cfg(feature = "sqlite")]
	#[rstest]
	#[tokio::test]
	async fn sqlite_memory_connection_uses_single_pool_connection() {
		// Arrange
		let connection = super::DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let pool = connection.into_sqlite().unwrap();

		// Act
		let first = pool.acquire().await.unwrap();
		let second = pool.try_acquire();
		drop(first);

		// Assert
		assert!(
			second.is_none(),
			"sqlite::memory: must stay single-connection so migrated schema remains visible"
		);
	}
}
