//! Migration recorder

use crate::backends::{DatabaseConnection, DatabaseError, DatabaseErrorKind};
use chrono::{DateTime, Utc};
use reinhardt_core::exception::Error as FrameworkError;

#[cfg(any(feature = "postgres", feature = "mysql"))]
use crate::backends::error::map_sqlx_error;

#[cfg(any(feature = "postgres", feature = "mysql"))]
fn map_sqlx_migration_error(error: sqlx::Error) -> super::MigrationError {
	super::MigrationError::DatabaseError(map_sqlx_error(error))
}

fn map_framework_database_error(error: FrameworkError) -> super::MigrationError {
	let database_error = error
		.database_error()
		.cloned()
		.unwrap_or_else(|| DatabaseError::new(DatabaseErrorKind::Query, error.to_string()));
	super::MigrationError::DatabaseError(database_error)
}

/// Migration record
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRecord {
	/// The app.
	pub app: String,
	/// The name.
	pub name: String,
	/// The applied.
	pub applied: DateTime<Utc>,
}

/// Migration recorder (in-memory only, for backward compatibility)
pub struct MigrationRecorder {
	records: Vec<MigrationRecord>,
}

/// Database-backed migration recorder
pub struct DatabaseMigrationRecorder {
	connection: DatabaseConnection,
}

/// Holds the CockroachDB sentinel-row lock until the guard is dropped.
#[cfg(feature = "postgres")]
pub(crate) struct CockroachdbSchemaLock {
	_tx: sqlx::Transaction<'static, sqlx::Postgres>,
}

impl MigrationRecorder {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self {
			records: Vec::new(),
		}
	}

	/// Performs the record applied operation.
	pub fn record_applied(&mut self, app: &str, name: &str) {
		self.records.push(MigrationRecord {
			app: app.to_string(),
			name: name.to_string(),
			applied: Utc::now(),
		});
	}

	/// Returns the applied migrations.
	pub fn get_applied_migrations(&self) -> &[MigrationRecord] {
		&self.records
	}

	/// Returns the pplied.
	pub fn is_applied(&self, app: &str, name: &str) -> bool {
		self.records.iter().any(|r| r.app == app && r.name == name)
	}

	/// Performs the ensure schema table operation.
	pub fn ensure_schema_table(&self) {
		// Ensure migration schema table exists
	}

	// Async versions for database operations
	/// Performs the ensure schema table async operation.
	pub async fn ensure_schema_table_async<T>(&self, _pool: &T) -> super::Result<()> {
		Ok(())
	}

	/// Returns the pplied async.
	pub async fn is_applied_async<T>(
		&self,
		_pool: &T,
		app: &str,
		name: &str,
	) -> super::Result<bool> {
		Ok(self.is_applied(app, name))
	}

	/// Performs the record applied async operation.
	pub async fn record_applied_async<T>(
		&mut self,
		_pool: &T,
		app: &str,
		name: &str,
	) -> super::Result<()> {
		self.record_applied(app, name);
		Ok(())
	}

	/// Remove a migration record (for rollback)
	pub fn unapply(&mut self, app: &str, name: &str) {
		self.records.retain(|r| !(r.app == app && r.name == name));
	}

	/// Get all applied migrations for a specific app
	pub fn get_applied_for_app(&self, app: &str) -> Vec<MigrationRecord> {
		self.records
			.iter()
			.filter(|r| r.app == app)
			.cloned()
			.collect()
	}

	/// Async version of unapply
	pub async fn unapply_async<T>(
		&mut self,
		_pool: &T,
		app: &str,
		name: &str,
	) -> super::Result<()> {
		self.unapply(app, name);
		Ok(())
	}

	/// Async version of get_applied_for_app
	pub async fn get_applied_for_app_async<T>(
		&self,
		_pool: &T,
		app: &str,
	) -> super::Result<Vec<MigrationRecord>> {
		Ok(self.get_applied_for_app(app))
	}
}

impl Default for MigrationRecorder {
	fn default() -> Self {
		Self::new()
	}
}

impl DatabaseMigrationRecorder {
	fn recorder_table_might_be_absent(&self, error: &super::MigrationError) -> bool {
		use crate::backends::types::DatabaseType;

		let super::MigrationError::DatabaseError(error) = error else {
			return false;
		};

		match self.connection.database_type() {
			DatabaseType::Postgres => error.code() == Some("42P01"),
			DatabaseType::Mysql => matches!(error.code(), Some("42S02" | "1146")),
			DatabaseType::Sqlite => {
				error.code() == Some("1")
					&& error
						.message()
						.eq_ignore_ascii_case("no such table: reinhardt_migrations")
			}
		}
	}

	async fn recorder_relation_exists(&self) -> super::Result<bool> {
		use crate::backends::types::DatabaseType;

		let sql = match self.connection.database_type() {
			DatabaseType::Postgres => {
				"SELECT to_regclass('reinhardt_migrations') IS NOT NULL AS recorder_exists"
			}
			DatabaseType::Mysql => {
				"SELECT EXISTS(
					SELECT 1
					FROM information_schema.tables
					WHERE table_schema = DATABASE()
					  AND table_name = 'reinhardt_migrations'
				) AS recorder_exists"
			}
			DatabaseType::Sqlite => return Ok(false),
		};
		let rows = self
			.connection
			.fetch_all(sql, vec![])
			.await
			.map_err(map_framework_database_error)?;
		let row = rows.first().ok_or_else(|| {
			super::MigrationError::DatabaseError(DatabaseError::new(
				DatabaseErrorKind::Query,
				"Recorder relation existence query returned no rows",
			))
		})?;

		row.get::<bool>("recorder_exists")
			.or_else(|_| row.get::<i64>("recorder_exists").map(|exists| exists != 0))
			.map_err(super::MigrationError::DatabaseError)
	}

	/// Create a new database-backed migration recorder
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::recorder::DatabaseMigrationRecorder;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let recorder = DatabaseMigrationRecorder::new(connection);
	/// // Verify recorder was created successfully
	/// recorder.ensure_schema_table().await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub fn new(connection: DatabaseConnection) -> Self {
		Self { connection }
	}

	/// Performs the ensure schema table operation.
	pub async fn ensure_schema_table(&self) -> super::Result<()> {
		// `DatabaseType` is only referenced from the mysql-gated match arm below;
		// import it under the same gate so the non-mysql build does not warn.
		#[cfg(feature = "mysql")]
		use crate::backends::types::DatabaseType;

		// CockroachDB is wire-compatible with PostgreSQL but does NOT implement
		// `pg_advisory_lock()` (returns `function undefined`). Route through a
		// CockroachDB-specific path that locks a sentinel row via
		// `SELECT ... FOR UPDATE` instead. See issue #4642.
		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self.ensure_schema_table_cockroachdb().await;
		}

		// MySQL `GET_LOCK()` / `RELEASE_LOCK()` are session-scoped: the lock is owned
		// by the connection that called `GET_LOCK`, and `RELEASE_LOCK` only succeeds
		// when issued on that same session. The pooled `DatabaseConnection::execute`
		// path acquires a new pool connection per call, so acquiring and releasing
		// through it routinely hits two different sessions — the lock is acquired on
		// session A and the bogus release runs on session B, leaving the lock leaked
		// in session A. Subsequent migration calls in the same process then time out
		// waiting for the leaked lock (manifesting as the rollback timeout in
		// issue #4585). To make the lock symmetrical, MySQL must acquire and release
		// on a single dedicated pool connection that we hold for the lock's lifetime.
		match self.connection.database_type() {
			#[cfg(feature = "mysql")]
			DatabaseType::Mysql => self.ensure_schema_table_mysql().await,
			_ => {
				// Acquire advisory lock to prevent concurrent schema modifications
				self.acquire_schema_lock().await?;

				// Execute schema operations
				let result = self.ensure_schema_table_internal().await;

				// Always release lock, even if operations failed
				let _ = self.release_schema_lock().await;

				result
			}
		}
	}

	/// CockroachDB-specific variant of `ensure_schema_table`.
	///
	/// CockroachDB does not implement PostgreSQL's `pg_advisory_lock()`
	/// (returns `unknown function: pg_advisory_lock(): function undefined`),
	/// so the generic Postgres path in `acquire_schema_lock` fails immediately.
	/// To serialise concurrent migrators we use a sentinel-row mutex: a
	/// single-row `_reinhardt_migration_lock` table whose row is locked with
	/// `SELECT ... FOR UPDATE` inside a held transaction. CockroachDB blocks
	/// concurrent `SELECT FOR UPDATE` on the same row until the holding
	/// transaction commits, giving the same "one migrator at a time"
	/// guarantee that `pg_advisory_lock` provides on real PostgreSQL.
	///
	/// The lock transaction is held on a dedicated pool connection for the
	/// caller's scope; migration execution keeps the guard alive across the
	/// full apply/rollback operation. The actual DDL still runs through the
	/// shared `DatabaseConnection`. This mirrors the dedicated-`PoolConnection`
	/// pattern used by `ensure_schema_table_mysql` (which holds `GET_LOCK` for
	/// the same reason — issue #4585).
	///
	/// Bootstrap of the sentinel table itself is intentionally done before the
	/// row lock exists. CockroachDB may briefly reject the idempotent insert if
	/// a concurrent `CREATE TABLE IF NOT EXISTS` has not made the primary key
	/// visible to `ON CONFLICT` yet, so the seed step retries that narrow
	/// schema-propagation race.
	///
	/// Fixes #4642.
	#[cfg(feature = "postgres")]
	async fn ensure_schema_table_cockroachdb(&self) -> super::Result<()> {
		let _lock = self.acquire_cockroachdb_schema_lock().await?;
		self.ensure_schema_table_internal().await
	}

	/// Acquire the CockroachDB sentinel-row lock.
	///
	/// Holding the returned guard keeps a `SELECT ... FOR UPDATE` transaction
	/// open on `_reinhardt_migration_lock(id = 1)`. Dropping the guard rolls the
	/// transaction back through sqlx's RAII transaction cleanup and releases the
	/// row lock.
	#[cfg(feature = "postgres")]
	pub(crate) async fn acquire_cockroachdb_schema_lock(
		&self,
	) -> super::Result<CockroachdbSchemaLock> {
		let pool = self.connection.into_postgres().ok_or_else(|| {
			super::MigrationError::DatabaseError(DatabaseError::new(
				DatabaseErrorKind::Connection,
				"PostgreSQL backend unavailable when acquiring CockroachDB schema lock",
			))
		})?;

		self.bootstrap_cockroachdb_schema_lock(&pool).await?;

		let mut tx = pool.begin().await.map_err(map_sqlx_migration_error)?;

		sqlx::query("SELECT 1 FROM _reinhardt_migration_lock WHERE id = 1 FOR UPDATE")
			.execute(&mut *tx)
			.await
			.map_err(map_sqlx_migration_error)?;

		Ok(CockroachdbSchemaLock { _tx: tx })
	}

	#[cfg(feature = "postgres")]
	async fn bootstrap_cockroachdb_schema_lock(&self, pool: &sqlx::PgPool) -> super::Result<()> {
		const MAX_ATTEMPTS: usize = 5;

		// Bootstrap the sentinel lock table. Both statements are idempotent.
		for attempt in 1..=MAX_ATTEMPTS {
			sqlx::query(
				"CREATE TABLE IF NOT EXISTS _reinhardt_migration_lock (\
				     id INT PRIMARY KEY, locked_at TIMESTAMPTZ DEFAULT now())",
			)
			.execute(pool)
			.await
			.map_err(map_sqlx_migration_error)?;

			let insert_result = sqlx::query(
				"INSERT INTO _reinhardt_migration_lock (id) VALUES (1) \
				 ON CONFLICT (id) DO NOTHING",
			)
			.execute(pool)
			.await;

			match insert_result {
				Ok(_) => return Ok(()),
				Err(e)
					if attempt < MAX_ATTEMPTS
						&& is_retryable_cockroachdb_lock_bootstrap_error(&e) =>
				{
					tokio::time::sleep(std::time::Duration::from_millis(50 * attempt as u64)).await;
				}
				Err(e) => {
					return Err(super::MigrationError::DatabaseError(map_sqlx_error(e)));
				}
			}
		}

		Ok(())
	}
}

#[cfg(feature = "postgres")]
fn is_retryable_cockroachdb_lock_bootstrap_error(error: &sqlx::Error) -> bool {
	is_cockroachdb_constraint_visibility_error(&error.to_string())
}

fn is_retryable_cockroachdb_record_applied_error(error: &FrameworkError) -> bool {
	error.database_error().is_some_and(|database_error| {
		database_error.kind() == DatabaseErrorKind::Query
			&& is_cockroachdb_constraint_visibility_error(database_error.message())
	})
}

fn is_cockroachdb_constraint_visibility_error(message: &str) -> bool {
	message.contains(
		"there is no unique or exclusion constraint matching the ON CONFLICT specification",
	)
}

impl DatabaseMigrationRecorder {
	/// MySQL-specific variant of `ensure_schema_table` that holds the advisory
	/// lock on a dedicated pool connection for the lifetime of the lock.
	///
	/// `GET_LOCK()` and `RELEASE_LOCK()` are session-scoped in MySQL: the lock
	/// belongs to the connection (session) that successfully acquired it, and
	/// only that same session can release it. Running these two statements on
	/// different pool connections (as happens when each call goes through the
	/// generic `DatabaseConnection::execute` path) leaks the lock into the
	/// acquiring session, which is then returned to the pool while still
	/// holding the named lock. Subsequent calls block waiting on that leaked
	/// lock and surface as the `Failed to acquire migration lock (timeout)`
	/// error reported in issue #4585.
	///
	/// This routine acquires a single `PoolConnection`, runs `GET_LOCK` on it,
	/// performs the DDL through the shared connection (the lock still
	/// serialises concurrent migrators because they all queue on the same
	/// named lock), and finally runs `RELEASE_LOCK` on the same held
	/// connection before returning it to the pool. The result: no leaked
	/// session-bound lock between consecutive `apply_migrations` /
	/// `rollback_migrations` calls.
	#[cfg(feature = "mysql")]
	async fn ensure_schema_table_mysql(&self) -> super::Result<()> {
		let pool = self.connection.into_mysql().ok_or_else(|| {
			super::MigrationError::DatabaseError(DatabaseError::new(
				DatabaseErrorKind::Connection,
				"MySQL backend unavailable when acquiring schema lock",
			))
		})?;

		let mut conn = pool.acquire().await.map_err(map_sqlx_migration_error)?;

		// Acquire the named advisory lock on this specific session, with a
		// 10 second timeout (matches the previous behaviour).
		//
		// `GET_LOCK` can return NULL on internal errors (e.g. interrupted by
		// `KILL`), so we model the column as `Option<i64>` and treat NULL as
		// a failed acquisition.
		let locked: Option<i64> = sqlx::query_scalar("SELECT GET_LOCK('reinhardt_migrations', 10)")
			.fetch_one(&mut *conn)
			.await
			.map_err(map_sqlx_migration_error)?;

		if locked != Some(1) {
			return Err(super::MigrationError::DatabaseError(DatabaseError::new(
				DatabaseErrorKind::Timeout,
				"Failed to acquire migration lock (timeout)",
			)));
		}

		// Execute schema operations while the lock is held. The DDL runs on
		// the pooled `DatabaseConnection`, but the named lock — owned by
		// `conn` here — still serialises any concurrent migrator process
		// because `GET_LOCK` is a global named lock, not a row/table lock.
		let result = self.ensure_schema_table_internal().await;

		// Always release the lock on the same session that acquired it,
		// regardless of whether the DDL succeeded.
		//
		// `RELEASE_LOCK` returns a column rather than signalling failure via
		// `Err`: `Some(1)` = released, `Some(0)` = not held by this session,
		// `None` = lock did not exist. Anything other than `Some(1)` indicates
		// the release silently no-op'd on the wrong session and would
		// reintroduce a lock leak — surface that as a warning. Mirrors the
		// `GET_LOCK` handling above.
		let release_result: Result<Option<i64>, _> =
			sqlx::query_scalar("SELECT RELEASE_LOCK('reinhardt_migrations')")
				.fetch_one(&mut *conn)
				.await;
		match release_result {
			Ok(Some(1)) => {}
			Ok(other) => {
				tracing::warn!(
					result = ?other,
					"RELEASE_LOCK did not release the MySQL migration advisory lock; \
					 the session will release it on connection close"
				);
			}
			Err(e) => {
				tracing::warn!(
					error = %e,
					"Failed to call RELEASE_LOCK for the MySQL migration advisory lock; \
					 the session will release it on connection close"
				);
			}
		}

		result
	}

	/// Check if an index exists in MySQL
	///
	/// This is a MySQL-specific helper to check if an index already exists.
	/// PostgreSQL and SQLite handle `IF NOT EXISTS` correctly, but MySQL
	/// returns an error even with `IF NOT EXISTS` if the index already exists.
	async fn check_index_exists(&self, table: &str, index: &str) -> super::Result<bool> {
		// Use EXISTS pattern similar to is_applied() method for reliable type handling
		let query = "SELECT EXISTS(
		                 SELECT 1 FROM information_schema.statistics
		                 WHERE table_schema = DATABASE()
		                 AND table_name = ?
		                 AND index_name = ?
		             ) as exists_flag";

		let result = self
			.connection
			.fetch_one(query, vec![table.into(), index.into()])
			.await
			.map_err(map_framework_database_error)?;

		// Try to get as bool first, then as i64 for databases that return int
		// This pattern matches the is_applied() implementation
		if let Ok(exists) = result.get::<bool>("exists_flag") {
			Ok(exists)
		} else if let Ok(exists_int) = result.get::<i64>("exists_flag") {
			Ok(exists_int > 0)
		} else {
			Ok(false)
		}
	}

	/// Acquire a database-level advisory lock for schema operations
	///
	/// This prevents concurrent schema modifications that could cause conflicts.
	/// Different databases use different locking mechanisms:
	/// - PostgreSQL: pg_advisory_lock() with hash of string
	/// - MySQL: GET_LOCK() with timeout
	/// - SQLite: No additional lock needed (handled by transaction isolation)
	async fn acquire_schema_lock(&self) -> super::Result<()> {
		use crate::backends::types::DatabaseType;

		match self.connection.database_type() {
			DatabaseType::Postgres => {
				// PostgreSQL advisory lock using string hash
				self.connection
					.execute(
						"SELECT pg_advisory_lock(hashtext('reinhardt_migrations'))",
						vec![],
					)
					.await
					.map_err(map_framework_database_error)?;
			}
			DatabaseType::Mysql => {
				// MySQL GET_LOCK with 10 second timeout
				let result = self
					.connection
					.fetch_one(
						"SELECT GET_LOCK('reinhardt_migrations', 10) as locked",
						vec![],
					)
					.await
					.map_err(map_framework_database_error)?;

				// Try to get the lock status as i64 or bool
				let locked = if let Ok(val) = result.get::<i64>("locked") {
					val == 1
				} else {
					result.get::<bool>("locked").unwrap_or_default()
				};

				if !locked {
					return Err(super::MigrationError::DatabaseError(DatabaseError::new(
						DatabaseErrorKind::Timeout,
						"Failed to acquire migration lock (timeout)",
					)));
				}
			}
			DatabaseType::Sqlite => {
				// SQLite uses transaction isolation, no additional lock needed
			}
		}

		Ok(())
	}

	/// Release the database-level advisory lock
	///
	/// Should be called after schema operations complete, even if they fail.
	async fn release_schema_lock(&self) -> super::Result<()> {
		use crate::backends::types::DatabaseType;

		match self.connection.database_type() {
			DatabaseType::Postgres => {
				self.connection
					.execute(
						"SELECT pg_advisory_unlock(hashtext('reinhardt_migrations'))",
						vec![],
					)
					.await
					.map_err(map_framework_database_error)?;
			}
			DatabaseType::Mysql => {
				self.connection
					.execute("SELECT RELEASE_LOCK('reinhardt_migrations')", vec![])
					.await
					.map_err(map_framework_database_error)?;
			}
			DatabaseType::Sqlite => {
				// SQLite uses transaction isolation, no explicit unlock needed
			}
		}

		Ok(())
	}

	/// Internal implementation of ensure_schema_table without locking
	///
	/// This is called by ensure_schema_table() after acquiring the lock.
	pub(crate) async fn ensure_schema_table_internal(&self) -> super::Result<()> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, ColumnDef, Expr, MySqlQueryBuilder, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		// Build SQL using appropriate query builder based on database type
		// Scope stmt to ensure it's dropped before await
		let (create_table_sql, create_index_sql) = {
			let create_table_stmt = Query::create_table()
				.table(Alias::new("reinhardt_migrations"))
				.if_not_exists()
				.col(
					ColumnDef::new("id")
						.integer()
						.not_null(true)
						.auto_increment(true)
						.primary_key(true),
				)
				.col(ColumnDef::new("app").string_len(255).not_null(true))
				.col(ColumnDef::new("name").string_len(255).not_null(true))
				.col(
					ColumnDef::new("applied")
						.timestamp()
						.not_null(true)
						.default(Expr::current_timestamp().into_simple_expr()),
				)
				.to_owned();

			let create_index_stmt = Query::create_index()
				.if_not_exists()
				.name("reinhardt_migrations_app_name_unique")
				.table(Alias::new("reinhardt_migrations"))
				.col(Alias::new("app"))
				.col(Alias::new("name"))
				.unique()
				.to_owned();

			match self.connection.database_type() {
				DatabaseType::Postgres => (
					create_table_stmt.to_string(PostgresQueryBuilder),
					create_index_stmt.to_string(PostgresQueryBuilder),
				),
				DatabaseType::Mysql => (
					create_table_stmt.to_string(MySqlQueryBuilder),
					create_index_stmt.to_string(MySqlQueryBuilder),
				),
				DatabaseType::Sqlite => (
					create_table_stmt.to_string(SqliteQueryBuilder),
					create_index_stmt.to_string(SqliteQueryBuilder),
				),
			}
		}; // stmts are dropped here, before await

		// Create table
		self.connection
			.execute(&create_table_sql, vec![])
			.await
			.map_err(map_framework_database_error)?;

		// Create unique index on (app, name)
		// MySQL requires explicit check because IF NOT EXISTS doesn't work for indexes
		if self.connection.database_type() == DatabaseType::Mysql {
			let index_exists = self
				.check_index_exists(
					"reinhardt_migrations",
					"reinhardt_migrations_app_name_unique",
				)
				.await?;

			if !index_exists {
				self.connection
					.execute(&create_index_sql, vec![])
					.await
					.map_err(map_framework_database_error)?;
			}
		} else {
			// PostgreSQL, SQLite handle IF NOT EXISTS correctly
			self.connection
				.execute(&create_index_sql, vec![])
				.await
				.map_err(map_framework_database_error)?;
		}

		Ok(())
	}

	/// Check if a migration has been applied
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::recorder::DatabaseMigrationRecorder;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let recorder = DatabaseMigrationRecorder::new(connection);
	/// recorder.ensure_schema_table().await.unwrap();
	///
	/// let is_applied = recorder.is_applied("myapp", "0001_initial").await.unwrap();
	/// assert!(!is_applied); // Initially not applied
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn is_applied(&self, app: &str, name: &str) -> super::Result<bool> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		// Build SELECT EXISTS query using reinhardt-query
		let subquery = Query::select()
			.expr(Expr::value(1))
			.from(Alias::new("reinhardt_migrations"))
			.and_where(Expr::col(Alias::new("app")).eq(app))
			.and_where(Expr::col(Alias::new("name")).eq(name))
			.to_owned();

		let stmt = Query::select()
			.expr_as(Expr::exists(subquery), Alias::new("exists_flag"))
			.to_owned();

		let sql = match self.connection.database_type() {
			DatabaseType::Postgres => stmt.to_string(PostgresQueryBuilder),
			DatabaseType::Mysql => stmt.to_string(MySqlQueryBuilder),
			DatabaseType::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};

		let rows = self
			.connection
			.fetch_all(&sql, vec![])
			.await
			.map_err(map_framework_database_error)?;

		if rows.is_empty() {
			return Ok(false);
		}

		let row = &rows[0];

		// Try to get as bool first, then as i64 for databases that return int
		if let Ok(exists) = row.get::<bool>("exists_flag") {
			Ok(exists)
		} else if let Ok(exists_int) = row.get::<i64>("exists_flag") {
			Ok(exists_int > 0)
		} else {
			Ok(false)
		}
	}

	/// Record that a migration has been applied
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::recorder::DatabaseMigrationRecorder;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let recorder = DatabaseMigrationRecorder::new(connection);
	/// recorder.ensure_schema_table().await.unwrap();
	///
	/// recorder.record_applied("myapp", "0001_initial").await.unwrap();
	/// // Verify migration was recorded
	/// let is_applied = recorder.is_applied("myapp", "0001_initial").await.unwrap();
	/// assert!(is_applied);
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn record_applied(&self, app: &str, name: &str) -> super::Result<()> {
		self.record_applied_at(app, name, Utc::now()).await
	}

	pub(crate) async fn record_applied_at(
		&self,
		app: &str,
		name: &str,
		applied_at: DateTime<Utc>,
	) -> super::Result<()> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, MySqlQueryBuilder, PostgresQueryBuilder, Query, QueryStatementBuilder,
			SqliteQueryBuilder,
		};

		// Build INSERT query using reinhardt-query
		let applied_at = applied_at.format("%Y-%m-%d %H:%M:%S").to_string();
		let stmt = Query::insert()
			.into_table(Alias::new("reinhardt_migrations"))
			.columns([Alias::new("app"), Alias::new("name"), Alias::new("applied")])
			.values_panic([app.to_string(), name.to_string(), applied_at])
			.to_owned();

		// Add conflict resolution for concurrent execution.
		// Use to_string() to inline values directly into SQL, avoiding parameter
		// binding since execute() is called with empty params.
		let sql = match self.connection.database_type() {
			DatabaseType::Postgres => {
				// PostgreSQL: ON CONFLICT DO NOTHING
				let base_sql = stmt.to_string(PostgresQueryBuilder::new());
				format!("{} ON CONFLICT (app, name) DO NOTHING", base_sql)
			}
			DatabaseType::Mysql => {
				// MySQL: INSERT IGNORE
				let base_sql = stmt.to_string(MySqlQueryBuilder::new());
				base_sql.replacen("INSERT", "INSERT IGNORE", 1)
			}
			DatabaseType::Sqlite => {
				// SQLite: INSERT OR IGNORE
				let base_sql = stmt.to_string(SqliteQueryBuilder::new());
				base_sql.replacen("INSERT", "INSERT OR IGNORE", 1)
			}
		};

		let max_attempts = if self.connection.is_cockroachdb() {
			5
		} else {
			1
		};

		for attempt in 1..=max_attempts {
			match self.connection.execute(&sql, vec![]).await {
				Ok(_) => return Ok(()),
				Err(e)
					if self.connection.is_cockroachdb()
						&& attempt < max_attempts
						&& is_retryable_cockroachdb_record_applied_error(&e) =>
				{
					tokio::time::sleep(std::time::Duration::from_millis(50 * attempt as u64)).await;
				}
				Err(e) => return Err(map_framework_database_error(e)),
			}
		}

		Ok(())
	}

	/// Atomically adopt a complete replacement migration set.
	///
	/// The replacement record and every replaced record are changed in one
	/// transaction so an interrupted adoption cannot leave the recorder in a
	/// mixed state.
	pub async fn adopt_replacement(
		&self,
		replacement_app: &str,
		replacement_name: &str,
		replaces: &[(String, String)],
	) -> super::Result<()> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
		let record = Query::insert()
			.into_table(Alias::new("reinhardt_migrations"))
			.columns([Alias::new("app"), Alias::new("name"), Alias::new("applied")])
			.values_panic([
				replacement_app.to_string(),
				replacement_name.to_string(),
				now,
			])
			.to_owned();
		let record_sql = match self.connection.database_type() {
			DatabaseType::Postgres => format!(
				"{} ON CONFLICT (app, name) DO NOTHING",
				record.to_string(PostgresQueryBuilder::new())
			),
			DatabaseType::Mysql => {
				record
					.to_string(MySqlQueryBuilder::new())
					.replacen("INSERT", "INSERT IGNORE", 1)
			}
			DatabaseType::Sqlite => record.to_string(SqliteQueryBuilder::new()).replacen(
				"INSERT",
				"INSERT OR IGNORE",
				1,
			),
		};

		let mut transaction = self.connection.begin().await?;
		transaction.execute(&record_sql, vec![]).await?;
		for (app, name) in replaces {
			let delete = Query::delete()
				.from_table(Alias::new("reinhardt_migrations"))
				.and_where(Expr::col(Alias::new("app")).eq(app.as_str()))
				.and_where(Expr::col(Alias::new("name")).eq(name.as_str()))
				.to_owned();
			let delete_sql = match self.connection.database_type() {
				DatabaseType::Postgres => delete.to_string(PostgresQueryBuilder::new()),
				DatabaseType::Mysql => delete.to_string(MySqlQueryBuilder::new()),
				DatabaseType::Sqlite => delete.to_string(SqliteQueryBuilder::new()),
			};
			transaction.execute(&delete_sql, vec![]).await?;
		}
		transaction.commit().await?;

		Ok(())
	}

	/// Get all applied migrations
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::recorder::DatabaseMigrationRecorder;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// // For doctest purposes, using mock connection (URL is ignored in current implementation)
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let recorder = DatabaseMigrationRecorder::new(connection);
	/// recorder.ensure_schema_table().await.unwrap();
	///
	/// let migrations = recorder.get_applied_migrations().await.unwrap();
	/// assert!(migrations.is_empty()); // Initially no migrations applied
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn get_applied_migrations(&self) -> super::Result<Vec<MigrationRecord>> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query, QueryStatementBuilder,
			SqliteQueryBuilder,
		};

		// Build SELECT query using reinhardt-query
		let stmt = Query::select()
			.columns([Alias::new("app"), Alias::new("name"), Alias::new("applied")])
			.from(Alias::new("reinhardt_migrations"))
			.order_by(Alias::new("applied"), Order::Asc)
			.order_by(Alias::new("id"), Order::Asc)
			.to_owned();

		let sql = match self.connection.database_type() {
			DatabaseType::Postgres => stmt.to_string(PostgresQueryBuilder),
			DatabaseType::Mysql => stmt.to_string(MySqlQueryBuilder),
			DatabaseType::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};

		let rows = self
			.connection
			.fetch_all(&sql, vec![])
			.await
			.map_err(map_framework_database_error)?;

		let db_type = self.connection.database_type();
		let mut records = Vec::new();
		for row in rows {
			let app: String = row
				.get("app")
				.map_err(super::MigrationError::DatabaseError)?;
			let name: String = row
				.get("name")
				.map_err(super::MigrationError::DatabaseError)?;

			// Parse timestamp from database
			// SQLite stores CURRENT_TIMESTAMP as string "YYYY-MM-DD HH:MM:SS"
			// PostgreSQL and MySQL return proper DateTime types
			let applied: DateTime<Utc> = match db_type {
				DatabaseType::Sqlite => {
					let applied_str: String = row
						.get("applied")
						.map_err(super::MigrationError::DatabaseError)?;
					// Parse SQLite's CURRENT_TIMESTAMP format (no timezone info, assume UTC)
					chrono::NaiveDateTime::parse_from_str(&applied_str, "%Y-%m-%d %H:%M:%S")
						.map(|naive| naive.and_utc())
						.map_err(|e| {
							super::MigrationError::DatabaseError(DatabaseError::new(
								DatabaseErrorKind::Type,
								format!(
									"Failed to parse SQLite timestamp '{}': {}",
									applied_str, e
								),
							))
						})?
				}
				DatabaseType::Postgres => row
					.get("applied")
					.or_else(|_| {
						row.get::<chrono::NaiveDateTime>("applied")
							.map(|naive| naive.and_utc())
					})
					.map_err(super::MigrationError::DatabaseError)?,
				DatabaseType::Mysql => row
					.get("applied")
					.or_else(|_| {
						row.get::<chrono::NaiveDateTime>("applied")
							.map(|naive| naive.and_utc())
					})
					.map_err(super::MigrationError::DatabaseError)?,
			};

			records.push(MigrationRecord { app, name, applied });
		}

		Ok(records)
	}

	/// Return all applied migrations without creating the recorder table.
	///
	/// A missing recorder table means that no migrations have been applied.
	/// Other database failures, including malformed schemas, permissions, and
	/// timestamp decoding errors, are returned to the caller.
	pub async fn get_applied_migrations_if_present(&self) -> super::Result<Vec<MigrationRecord>> {
		match self.get_applied_migrations().await {
			Err(error) if self.recorder_table_might_be_absent(&error) => {
				if self.connection.database_type() == crate::backends::types::DatabaseType::Sqlite {
					return Ok(Vec::new());
				}
				match self.recorder_relation_exists().await {
					Ok(false) => Ok(Vec::new()),
					Ok(true) | Err(_) => Err(error),
				}
			}
			result => result,
		}
	}

	/// Rename an applied migration while retaining its original recorder row.
	pub async fn rename_applied(
		&self,
		old_app: &str,
		old_name: &str,
		new_app: &str,
		new_name: &str,
	) -> super::Result<()> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		let stmt = Query::update()
			.table(Alias::new("reinhardt_migrations"))
			.value(Alias::new("app"), new_app.to_string())
			.value(Alias::new("name"), new_name.to_string())
			.and_where(Expr::col(Alias::new("app")).eq(old_app))
			.and_where(Expr::col(Alias::new("name")).eq(old_name))
			.to_owned();
		let sql = match self.connection.database_type() {
			DatabaseType::Postgres => stmt.to_string(PostgresQueryBuilder),
			DatabaseType::Mysql => stmt.to_string(MySqlQueryBuilder),
			DatabaseType::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};
		self.connection
			.execute(&sql, vec![])
			.await
			.map_err(map_framework_database_error)?;
		Ok(())
	}

	/// Unapply a migration (remove from records)
	///
	/// Used when rolling back migrations.
	pub async fn unapply(&self, app: &str, name: &str) -> super::Result<()> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		// Build DELETE query using reinhardt-query
		let stmt = Query::delete()
			.from_table(Alias::new("reinhardt_migrations"))
			.and_where(Expr::col(Alias::new("app")).eq(app))
			.and_where(Expr::col(Alias::new("name")).eq(name))
			.to_owned();

		let sql = match self.connection.database_type() {
			DatabaseType::Postgres => stmt.to_string(PostgresQueryBuilder),
			DatabaseType::Mysql => stmt.to_string(MySqlQueryBuilder),
			DatabaseType::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};

		self.connection
			.execute(&sql, vec![])
			.await
			.map_err(map_framework_database_error)?;

		Ok(())
	}

	/// Get all applied migrations for a specific app
	///
	/// Returns migrations sorted by applied timestamp in ascending order.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::recorder::DatabaseMigrationRecorder;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let recorder = DatabaseMigrationRecorder::new(connection);
	/// recorder.ensure_schema_table().await.unwrap();
	///
	/// let migrations = recorder.get_applied_for_app("myapp").await.unwrap();
	/// assert!(migrations.is_empty()); // Initially no migrations applied
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn get_applied_for_app(&self, app: &str) -> super::Result<Vec<MigrationRecord>> {
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, Expr, ExprTrait, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		// Build SELECT query using reinhardt-query with app filter
		let stmt = Query::select()
			.columns([Alias::new("app"), Alias::new("name"), Alias::new("applied")])
			.from(Alias::new("reinhardt_migrations"))
			.and_where(Expr::col(Alias::new("app")).eq(app))
			.order_by(Alias::new("applied"), Order::Asc)
			.order_by(Alias::new("id"), Order::Asc)
			.to_owned();

		let sql = match self.connection.database_type() {
			DatabaseType::Postgres => stmt.to_string(PostgresQueryBuilder),
			DatabaseType::Mysql => stmt.to_string(MySqlQueryBuilder),
			DatabaseType::Sqlite => stmt.to_string(SqliteQueryBuilder),
		};

		let rows = self
			.connection
			.fetch_all(&sql, vec![])
			.await
			.map_err(map_framework_database_error)?;

		let db_type = self.connection.database_type();
		let mut records = Vec::new();
		for row in rows {
			let app_val: String = row
				.get("app")
				.map_err(super::MigrationError::DatabaseError)?;
			let name: String = row
				.get("name")
				.map_err(super::MigrationError::DatabaseError)?;

			// Parse timestamp from database
			let applied: DateTime<Utc> = match db_type {
				DatabaseType::Sqlite => {
					let applied_str: String = row
						.get("applied")
						.map_err(super::MigrationError::DatabaseError)?;
					chrono::NaiveDateTime::parse_from_str(&applied_str, "%Y-%m-%d %H:%M:%S")
						.map(|naive| naive.and_utc())
						.map_err(|e| {
							super::MigrationError::DatabaseError(DatabaseError::new(
								DatabaseErrorKind::Type,
								format!(
									"Failed to parse SQLite timestamp '{}': {}",
									applied_str, e
								),
							))
						})?
				}
				DatabaseType::Postgres => row
					.get("applied")
					.or_else(|_| {
						row.get::<chrono::NaiveDateTime>("applied")
							.map(|naive| naive.and_utc())
					})
					.map_err(super::MigrationError::DatabaseError)?,
				DatabaseType::Mysql => row
					.get("applied")
					.or_else(|_| {
						row.get::<chrono::NaiveDateTime>("applied")
							.map(|naive| naive.and_utc())
					})
					.map_err(super::MigrationError::DatabaseError)?,
			};

			records.push(MigrationRecord {
				app: app_val,
				name,
				applied,
			});
		}

		Ok(records)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;

	#[test]
	fn test_migration_recorder_creation() {
		let recorder = MigrationRecorder::new();
		assert_eq!(recorder.get_applied_migrations().len(), 0);
	}

	#[test]
	fn test_record_applied() {
		let mut recorder = MigrationRecorder::new();
		recorder.record_applied("auth", "0001_initial");

		assert_eq!(recorder.get_applied_migrations().len(), 1);
		assert!(recorder.is_applied("auth", "0001_initial"));
	}

	#[test]
	fn test_is_applied() {
		let mut recorder = MigrationRecorder::new();

		assert!(!recorder.is_applied("auth", "0001_initial"));

		recorder.record_applied("auth", "0001_initial");

		assert!(recorder.is_applied("auth", "0001_initial"));
		assert!(!recorder.is_applied("auth", "0002_add_field"));
	}

	#[test]
	fn test_get_applied_migrations() {
		let mut recorder = MigrationRecorder::new();

		recorder.record_applied("auth", "0001_initial");
		recorder.record_applied("users", "0001_initial");
		recorder.record_applied("auth", "0002_add_field");

		let migrations = recorder.get_applied_migrations();
		assert_eq!(migrations.len(), 3);

		// Verify all migrations were recorded
		assert!(
			migrations
				.iter()
				.any(|m| m.app == "auth" && m.name == "0001_initial")
		);
		assert!(
			migrations
				.iter()
				.any(|m| m.app == "users" && m.name == "0001_initial")
		);
		assert!(
			migrations
				.iter()
				.any(|m| m.app == "auth" && m.name == "0002_add_field")
		);
	}

	#[test]
	fn test_migration_record_contains_timestamp() {
		let mut recorder = MigrationRecorder::new();
		let before = Utc::now();

		recorder.record_applied("auth", "0001_initial");

		let after = Utc::now();
		let migrations = recorder.get_applied_migrations();

		assert_eq!(migrations.len(), 1);
		let record = &migrations[0];

		// Check timestamp is within expected range
		assert!(record.applied >= before);
		assert!(record.applied <= after);
	}

	#[test]
	fn test_multiple_apps_migrations() {
		let mut recorder = MigrationRecorder::new();

		recorder.record_applied("auth", "0001_initial");
		recorder.record_applied("auth", "0002_add_field");
		recorder.record_applied("users", "0001_initial");
		recorder.record_applied("posts", "0001_initial");

		assert!(recorder.is_applied("auth", "0001_initial"));
		assert!(recorder.is_applied("auth", "0002_add_field"));
		assert!(recorder.is_applied("users", "0001_initial"));
		assert!(recorder.is_applied("posts", "0001_initial"));

		assert!(!recorder.is_applied("comments", "0001_initial"));
	}

	#[tokio::test]
	async fn test_async_record_applied() {
		let mut recorder = MigrationRecorder::new();

		recorder
			.record_applied_async(&(), "auth", "0001_initial")
			.await
			.unwrap();

		assert!(recorder.is_applied("auth", "0001_initial"));
	}

	#[tokio::test]
	async fn test_async_is_applied() {
		let mut recorder = MigrationRecorder::new();

		recorder.record_applied("auth", "0001_initial");

		let result = recorder
			.is_applied_async(&(), "auth", "0001_initial")
			.await
			.unwrap();

		assert!(result);

		let result_not_applied = recorder
			.is_applied_async(&(), "auth", "0002_add_field")
			.await
			.unwrap();

		assert!(!result_not_applied);
	}

	#[tokio::test]
	async fn test_ensure_schema_table_async() {
		let recorder = MigrationRecorder::new();
		let result = recorder.ensure_schema_table_async(&()).await;
		assert!(result.is_ok());
	}
}
