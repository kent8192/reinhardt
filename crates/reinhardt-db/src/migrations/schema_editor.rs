//! Schema Editor for migration execution
//!
//! Provides atomic transaction support for DDL operations,
//! similar to Django's schema_editor.
//!
//! # Overview
//!
//! The `SchemaEditor` wraps database connections and optionally manages
//! transactions for atomic migration execution. It follows Django's pattern
//! where migrations can be wrapped in transactions for databases that support
//! transactional DDL.
//!
//! # Database Support
//!
//! | Database | Transactional DDL | Notes |
//! |----------|-------------------|-------|
//! | PostgreSQL | Yes | DDL can be rolled back |
//! | SQLite | Yes | DDL can be rolled back |
//! | MySQL | No | DDL causes implicit commit |
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_db::migrations::schema_editor::SchemaEditor;
//! use reinhardt_db::backends::{DatabaseConnection, DatabaseType};
//!
//! let connection = DatabaseConnection::connect_postgres("postgres://...").await?;
//! let mut editor = SchemaEditor::new(connection.clone(), true, DatabaseType::Postgres).await?;
//!
//! editor.execute("CREATE TABLE users (id SERIAL PRIMARY KEY)").await?;
//! editor.execute("ALTER TABLE users ADD COLUMN name TEXT").await?;
//!
//! // Commit all changes atomically
//! editor.finish().await?;
//! ```

#[cfg(feature = "sqlite")]
use std::{
	future::Future,
	panic::{AssertUnwindSafe, resume_unwind},
	pin::Pin,
};

#[cfg(feature = "sqlite")]
use super::MigrationError;
use super::Result;
#[cfg(feature = "sqlite")]
use crate::backends::dialect::SqliteBackend;
use crate::backends::{
	connection::DatabaseConnection,
	types::{DatabaseType, QueryValue, Row, TransactionExecutor},
};
#[cfg(feature = "sqlite")]
use futures::FutureExt;
#[cfg(feature = "sqlite")]
use sqlx::{Row as SqlxRow, Sqlite, pool::PoolConnection};

/// Owns the physical SQLite connection used by atomic and non-atomic recreation.
///
/// Dropping a dirty session transfers its connection to an independent cleanup
/// task. The task rolls back the open transaction and restores the previous
/// foreign-key state before returning the connection to the pool. If cleanup
/// cannot complete, the connection is closed instead of exposing partial DDL or
/// connection-local state to the next borrower.
#[cfg(feature = "sqlite")]
struct SqliteRecreationSession {
	connection: Option<PoolConnection<Sqlite>>,
	previous_foreign_keys: Option<bool>,
	transaction_started: bool,
}

#[cfg(feature = "sqlite")]
struct SqliteCleanupConnection {
	connection: Option<PoolConnection<Sqlite>>,
}

#[cfg(feature = "sqlite")]
impl SqliteCleanupConnection {
	fn new(connection: PoolConnection<Sqlite>) -> Self {
		Self {
			connection: Some(connection),
		}
	}

	fn connection_mut(&mut self) -> &mut PoolConnection<Sqlite> {
		self.connection
			.as_mut()
			.expect("SQLite cleanup connection must be present")
	}

	fn return_to_pool(mut self) {
		self.connection.take();
	}
}

#[cfg(feature = "sqlite")]
impl Drop for SqliteCleanupConnection {
	fn drop(&mut self) {
		if let Some(connection) = self.connection.as_mut() {
			connection.close_on_drop();
		}
	}
}

#[cfg(feature = "sqlite")]
fn sqlite_rollback_succeeded_or_inactive<T>(result: std::result::Result<T, sqlx::Error>) -> bool {
	match result {
		Ok(_) => true,
		Err(sqlx::Error::Database(error))
			if error.message() == "cannot rollback - no transaction is active" =>
		{
			true
		}
		Err(error) => {
			tracing::error!("failed to roll back SQLite recreation session: {error}");
			false
		}
	}
}

#[cfg(feature = "sqlite")]
impl SqliteRecreationSession {
	fn new(connection: PoolConnection<Sqlite>) -> Self {
		Self {
			connection: Some(connection),
			previous_foreign_keys: None,
			transaction_started: false,
		}
	}

	fn connection_mut(&mut self) -> &mut PoolConnection<Sqlite> {
		self.connection
			.as_mut()
			.expect("SQLite recreation connection must be present")
	}

	fn set_previous_foreign_keys(&mut self, enabled: bool) {
		self.previous_foreign_keys = Some(enabled);
	}

	fn mark_transaction_started(&mut self) {
		self.transaction_started = true;
	}

	fn mark_transaction_finished(&mut self) {
		self.transaction_started = false;
	}

	fn mark_clean(&mut self) {
		self.previous_foreign_keys = None;
		self.transaction_started = false;
	}
}

#[cfg(feature = "sqlite")]
impl Drop for SqliteRecreationSession {
	fn drop(&mut self) {
		let Some(mut connection) = self.connection.take() else {
			return;
		};
		if self.previous_foreign_keys.is_none() && !self.transaction_started {
			drop(connection);
			return;
		}

		let previous_foreign_keys = self.previous_foreign_keys;
		let transaction_started = self.transaction_started;
		let Ok(runtime) = tokio::runtime::Handle::try_current() else {
			connection.close_on_drop();
			drop(connection);
			return;
		};

		runtime.spawn(async move {
			let mut cleanup = SqliteCleanupConnection::new(connection);
			let rollback_succeeded = if transaction_started {
				sqlite_rollback_succeeded_or_inactive(
					sqlx::query("ROLLBACK")
						.execute(&mut **cleanup.connection_mut())
						.await,
				)
			} else {
				true
			};
			let restore_succeeded = match previous_foreign_keys {
				Some(true) => sqlx::query("PRAGMA foreign_keys = ON")
					.execute(&mut **cleanup.connection_mut())
					.await
					.is_ok(),
				Some(false) => sqlx::query("PRAGMA foreign_keys = OFF")
					.execute(&mut **cleanup.connection_mut())
					.await
					.is_ok(),
				None => true,
			};
			if rollback_succeeded && restore_succeeded {
				cleanup.return_to_pool();
			}
		});
	}
}

/// Schema editor for executing DDL statements with optional transaction support
///
/// This struct wraps a database connection and optionally manages a transaction
/// for atomic migration execution. It follows Django's schema_editor pattern.
///
/// When `atomic` is `true` and the database supports transactional DDL,
/// all DDL operations are wrapped in a transaction that can be committed
/// or rolled back as a unit.
pub struct SchemaEditor {
	/// Database connection
	connection: DatabaseConnection,
	/// Transaction executor (if using atomic mode)
	executor: Option<Box<dyn TransactionExecutor>>,
	/// Whether this editor is using atomic transactions
	atomic: bool,
	/// Database type
	db_type: DatabaseType,
	/// Deferred SQL statements to execute at finish
	deferred_sql: Vec<String>,
	/// Dedicated physical connection used by SQLite recreation.
	#[cfg(feature = "sqlite")]
	sqlite_recreation_session: Option<SqliteRecreationSession>,
}

impl SchemaEditor {
	/// Create a new schema editor
	///
	/// If `atomic` is true and the database supports transactional DDL,
	/// a transaction will be started automatically.
	///
	/// # Arguments
	///
	/// * `connection` - Database connection to use
	/// * `atomic` - Whether to wrap operations in a transaction
	/// * `db_type` - Database type for dialect-specific handling
	///
	/// # Returns
	///
	/// A new SchemaEditor instance
	///
	/// # Notes
	///
	/// If `atomic` is `true` but the database doesn't support transactional DDL
	/// (e.g., MySQL), a warning is logged and operations proceed without
	/// transaction wrapping.
	pub async fn new(
		connection: DatabaseConnection,
		atomic: bool,
		db_type: DatabaseType,
	) -> Result<Self> {
		Self::new_for_migration(connection, atomic, db_type, false).await
	}

	pub(crate) async fn new_for_migration(
		connection: DatabaseConnection,
		atomic: bool,
		db_type: DatabaseType,
		requires_sqlite_recreation: bool,
	) -> Result<Self> {
		let effective_atomic = atomic && db_type.supports_transactional_ddl();
		#[cfg(feature = "sqlite")]
		let use_sqlite_recreation_session = effective_atomic
			&& matches!(db_type, DatabaseType::Sqlite)
			&& requires_sqlite_recreation;
		#[cfg(not(feature = "sqlite"))]
		let use_sqlite_recreation_session = false;

		let executor = if effective_atomic && !use_sqlite_recreation_session {
			Some(connection.begin().await?)
		} else {
			if atomic && !db_type.supports_transactional_ddl() {
				tracing::warn!(
					"atomic=true requested but {:?} doesn't support transactional DDL. \
					 Proceeding without transaction wrapper.",
					db_type
				);
			}
			None
		};

		let mut editor = Self {
			connection,
			executor,
			atomic: effective_atomic,
			db_type,
			deferred_sql: Vec::new(),
			#[cfg(feature = "sqlite")]
			sqlite_recreation_session: None,
		};

		#[cfg(feature = "sqlite")]
		if use_sqlite_recreation_session {
			editor.begin_atomic_sqlite_recreation_session().await?;
		}

		Ok(editor)
	}

	/// Execute a DDL statement
	///
	/// If in atomic mode, executes within the transaction.
	/// Otherwise, executes directly on the connection.
	///
	/// # Arguments
	///
	/// * `sql` - SQL statement to execute
	pub async fn execute(&mut self, sql: &str) -> Result<()> {
		#[cfg(feature = "sqlite")]
		if let Some(session) = self.sqlite_recreation_session.as_mut() {
			sqlx::query(sql)
				.execute(&mut **session.connection_mut())
				.await?;
			return Ok(());
		}

		if let Some(ref mut tx) = self.executor {
			tx.execute(sql, vec![]).await?;
			// SQLite requires a schema cache refresh after DDL within a transaction
			// to prevent SQLITE_SCHEMA (code 262) errors on subsequent DDL statements.
			if self.db_type == DatabaseType::Sqlite {
				tx.execute("SELECT 1", vec![]).await?;
			}
		} else {
			self.connection.execute(sql, vec![]).await?;
		}

		Ok(())
	}

	/// Fetch all rows for a read query, routed through the same connection as
	/// in-flight DDL so that in-transaction schema changes are visible.
	///
	/// When the editor is in atomic mode, the query is dispatched on the open
	/// transaction. Without this, a read issued through the pool would
	/// transparently land on a *different* physical connection and would not
	/// observe uncommitted DDL — the failure mode behind reinhardt-web#4447.
	pub async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		#[cfg(feature = "sqlite")]
		if let Some(session) = self.sqlite_recreation_session.as_mut() {
			let mut query = sqlx::query(sql);
			for param in &params {
				query = SqliteBackend::bind_value(query, param)?;
			}
			let rows = query.fetch_all(&mut **session.connection_mut()).await?;
			return rows
				.into_iter()
				.map(SqliteBackend::convert_row)
				.collect::<crate::backends::error::Result<Vec<_>>>()
				.map_err(MigrationError::from);
		}

		if let Some(ref mut tx) = self.executor {
			Ok(tx.fetch_all(sql, params).await?)
		} else {
			Ok(self.connection.fetch_all(sql, params).await?)
		}
	}

	/// Fetch a single optional row through the in-flight transaction (if any).
	///
	/// Mirrors [`Self::fetch_all`] for callers that expect zero or one row.
	pub async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> Result<Option<Row>> {
		#[cfg(feature = "sqlite")]
		if let Some(session) = self.sqlite_recreation_session.as_mut() {
			let mut query = sqlx::query(sql);
			for param in &params {
				query = SqliteBackend::bind_value(query, param)?;
			}
			let row = query
				.fetch_optional(&mut **session.connection_mut())
				.await?;
			return row
				.map(SqliteBackend::convert_row)
				.transpose()
				.map_err(MigrationError::from);
		}

		if let Some(ref mut tx) = self.executor {
			Ok(tx.fetch_optional(sql, params).await?)
		} else {
			Ok(self.connection.fetch_optional(sql, params).await?)
		}
	}

	/// Check whether a table exists, routed through the editor's open
	/// transaction so that schema changes made earlier in the same atomic
	/// migration are visible to the check.
	///
	/// When the editor is non-atomic this falls back to the pool, identical
	/// in semantics to [`DatabaseConnection::fetch_optional`].
	///
	/// # SQLite-specific rationale (reinhardt-web#4584)
	///
	/// On SQLite, when an atomic migration has already executed a DDL on the
	/// transaction's connection, the schema cookie has been bumped. A read
	/// issued through the pool transparently lands on a *different* physical
	/// connection whose prepared-statement / schema cache is stale. SQLite
	/// then returns SQLITE_SCHEMA (code 262, "database schema is locked")
	/// instead of returning the up-to-date row. Routing the existence check
	/// through the same connection that performed the DDL avoids the stale
	/// cache entirely.
	pub async fn table_exists(&mut self, table_name: &str) -> Result<bool> {
		use reinhardt_query::prelude::{
			Alias, Cond, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		match self.db_type {
			DatabaseType::Postgres => {
				// Build an escaped/quoted literal query using reinhardt-query
				// (values are inlined via `to_string(QueryBuilder)`, not bound
				// as parameters), mirroring the introspection emitted by
				// `DatabaseMigrationExecutor` so the routing change here is
				// purely about which connection runs the query.
				let subquery = Query::select()
					.expr(Expr::asterisk())
					.from((Alias::new("information_schema"), Alias::new("tables")))
					.cond_where(
						Cond::all()
							.add(Expr::col(Alias::new("table_schema")).eq("public"))
							.add(Expr::col(Alias::new("table_name")).eq(table_name)),
					)
					.to_owned();

				// Explicitly alias the EXISTS expression so we can look it up
				// by a stable column key regardless of how a given adapter
				// exposes an unnamed expression.
				let query_str = format!(
					"SELECT EXISTS ({}) AS table_exists",
					subquery.to_string(PostgresQueryBuilder)
				);

				match self.fetch_optional(&query_str, vec![]).await? {
					Some(row) => match row.data.get("table_exists") {
						Some(QueryValue::Bool(b)) => Ok(*b),
						_ => Ok(false),
					},
					None => Ok(false),
				}
			}
			DatabaseType::Sqlite => {
				let query = Query::select()
					.column(Alias::new("name"))
					.from(Alias::new("sqlite_master"))
					.cond_where(
						Cond::all()
							.add(Expr::col(Alias::new("type")).eq("table"))
							.add(Expr::col(Alias::new("name")).eq(table_name)),
					)
					.to_owned();

				let query_str = query.to_string(SqliteQueryBuilder);
				let row = self.fetch_optional(&query_str, vec![]).await?;
				Ok(row.is_some())
			}
			DatabaseType::Mysql => {
				// `information_schema.tables` exposes canonical UPPER_CASE
				// column names (e.g. `TABLE_SCHEMA`, `TABLE_NAME`); use them
				// consistently to avoid surprises if identifier-quoting or
				// casing behaviour changes in MySQL configurations.
				let query = Query::select()
					.column(Alias::new("TABLE_NAME"))
					.from((Alias::new("information_schema"), Alias::new("tables")))
					.cond_where(
						Cond::all()
							.add(Expr::col(Alias::new("TABLE_SCHEMA")).eq(Expr::cust("DATABASE()")))
							.add(Expr::col(Alias::new("TABLE_NAME")).eq(table_name)),
					)
					.to_owned();

				let query_str = query.to_string(MySqlQueryBuilder);
				let row = self.fetch_optional(&query_str, vec![]).await?;
				Ok(row.is_some())
			}
		}
	}

	/// Defer SQL execution until finish()
	///
	/// Some operations need to be executed after all other operations
	/// in the migration (e.g., creating indexes on newly created columns).
	///
	/// # Arguments
	///
	/// * `sql` - SQL statement to defer
	pub fn defer(&mut self, sql: String) {
		self.deferred_sql.push(sql);
	}

	/// Finish the schema editing session
	///
	/// Executes any deferred SQL and commits the transaction if atomic.
	///
	/// # Returns
	///
	/// Ok(()) on success
	pub async fn finish(mut self) -> Result<()> {
		// Execute deferred SQL
		for sql in std::mem::take(&mut self.deferred_sql) {
			self.execute(&sql).await?;
		}

		#[cfg(feature = "sqlite")]
		if self.sqlite_recreation_session.is_some() {
			let violations = self.check_foreign_key_integrity().await?;
			if !violations.is_empty() {
				let violation_error = Err(MigrationError::ForeignKeyViolation(format!(
					"Foreign key violations detected after migration: {}",
					violations.join("; ")
				)));
				let rollback_result = self.rollback_sqlite_recreation_session().await;
				return merge_foreign_key_scope_results(violation_error, rollback_result);
			}
			self.execute("COMMIT").await?;
			self.mark_sqlite_recreation_transaction_finished();
			self.restore_sqlite_recreation_foreign_keys().await?;
			self.release_sqlite_recreation_session();
			return Ok(());
		}

		// Commit if in transaction
		if let Some(tx) = self.executor.take() {
			tx.commit().await?;
		}

		Ok(())
	}

	/// Rollback any changes (only effective for transactional DDL databases)
	///
	/// For databases that don't support transactional DDL (MySQL),
	/// this is a no-op as DDL statements have already been implicitly committed.
	pub async fn rollback(mut self) -> Result<()> {
		#[cfg(feature = "sqlite")]
		if self.sqlite_recreation_session.is_some() {
			return self.rollback_sqlite_recreation_session().await;
		}

		if let Some(tx) = self.executor.take() {
			tx.rollback().await?;
		}
		Ok(())
	}

	/// Check if this editor is using atomic transactions
	pub fn is_atomic(&self) -> bool {
		self.atomic
	}

	/// Get the database type
	pub fn database_type(&self) -> DatabaseType {
		self.db_type
	}

	/// Get a reference to the underlying connection
	///
	/// This can be used for operations that need direct connection access
	/// outside of the transaction (e.g., checking table existence).
	pub fn connection(&self) -> &DatabaseConnection {
		&self.connection
	}

	/// Disable foreign key checks (SQLite only)
	///
	/// This must be called BEFORE any table recreation operations that might
	/// temporarily break foreign key relationships. Remember to re-enable
	/// foreign keys after the operation completes.
	///
	/// # SQLite Foreign Key Handling
	///
	/// SQLite table recreation temporarily drops the original table, which
	/// can cause foreign key violations. This method disables foreign key
	/// enforcement during the recreation process.
	///
	/// # Returns
	///
	/// Ok(()) if successful, or an error if the operation fails.
	/// Returns Ok(()) immediately for non-SQLite databases.
	#[cfg(feature = "sqlite")]
	pub async fn disable_foreign_keys(&mut self) -> Result<()> {
		if !matches!(self.db_type, DatabaseType::Sqlite) {
			return Ok(());
		}

		tracing::debug!("Disabling SQLite foreign key checks");
		self.execute("PRAGMA foreign_keys = OFF").await?;
		Ok(())
	}

	/// Enable foreign key checks (SQLite only)
	///
	/// This should be called AFTER table recreation operations complete
	/// to restore foreign key enforcement.
	///
	/// # Returns
	///
	/// Ok(()) if successful, or an error if the operation fails.
	/// Returns Ok(()) immediately for non-SQLite databases.
	#[cfg(feature = "sqlite")]
	pub async fn enable_foreign_keys(&mut self) -> Result<()> {
		if !matches!(self.db_type, DatabaseType::Sqlite) {
			return Ok(());
		}

		tracing::debug!("Enabling SQLite foreign key checks");
		self.execute("PRAGMA foreign_keys = ON").await?;
		Ok(())
	}

	/// Runs an SQLite schema operation with foreign key enforcement disabled.
	///
	/// The previous enabled state is restored after the operation returns,
	/// including when the operation returns an error. Callers can therefore use
	/// `?` inside the scoped future without bypassing restoration.
	#[cfg(feature = "sqlite")]
	pub(crate) async fn with_foreign_keys_disabled<T: Send>(
		&mut self,
		operation: impl for<'a> FnOnce(
			&'a mut Self,
		) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>,
	) -> Result<T> {
		if self.sqlite_recreation_session.is_some() {
			let operation_result =
				match std::panic::catch_unwind(AssertUnwindSafe(|| operation(self))) {
					Ok(operation) => AssertUnwindSafe(operation).catch_unwind().await,
					Err(payload) => Err(payload),
				};
			return match operation_result {
				Ok(Ok(value)) => Ok(value),
				Ok(Err(operation_error)) => {
					let rollback_result = self.rollback_sqlite_recreation_session().await;
					merge_foreign_key_scope_results(Err(operation_error), rollback_result)
				}
				Err(payload) => {
					if let Err(cleanup_error) = self.rollback_sqlite_recreation_session().await {
						tracing::error!(
							"failed to clean up atomic SQLite recreation session after panic: {cleanup_error}"
						);
					}
					resume_unwind(payload)
				}
			};
		}

		if self.executor.is_none() && matches!(self.db_type, DatabaseType::Sqlite) {
			return self
				.with_non_atomic_sqlite_recreation_session(operation)
				.await;
		}

		let was_enabled = self.foreign_keys_enabled().await?;
		if was_enabled {
			self.disable_foreign_keys().await?;
		}
		let operation_result = match std::panic::catch_unwind(AssertUnwindSafe(|| operation(self)))
		{
			Ok(operation) => AssertUnwindSafe(operation).catch_unwind().await,
			Err(payload) => Err(payload),
		};
		let restore_result = if was_enabled {
			self.enable_foreign_keys().await
		} else {
			Ok(())
		};
		match operation_result {
			Ok(operation_result) => {
				merge_foreign_key_scope_results(operation_result, restore_result)
			}
			Err(payload) => {
				if let Err(restore_error) = restore_result {
					tracing::error!(
						"failed to restore SQLite foreign key enforcement after schema operation panic: {restore_error}"
					);
				}
				resume_unwind(payload)
			}
		}
	}

	#[cfg(feature = "sqlite")]
	async fn begin_atomic_sqlite_recreation_session(&mut self) -> Result<()> {
		let pool = self.connection.into_sqlite().ok_or_else(|| {
			MigrationError::UnsupportedDatabase(
				"SQLite recreation requires an SQLite connection pool".to_string(),
			)
		})?;
		let connection = pool.acquire().await?;
		self.sqlite_recreation_session = Some(SqliteRecreationSession::new(connection));

		let was_enabled = self.foreign_keys_enabled().await?;
		self.set_sqlite_recreation_previous_foreign_keys(was_enabled);
		if was_enabled {
			self.disable_foreign_keys().await?;
		}
		self.mark_sqlite_recreation_transaction_started();
		self.execute("BEGIN").await?;
		Ok(())
	}

	#[cfg(feature = "sqlite")]
	async fn restore_sqlite_recreation_foreign_keys(&mut self) -> Result<()> {
		let was_enabled = self
			.sqlite_recreation_session
			.as_ref()
			.and_then(|session| session.previous_foreign_keys)
			.unwrap_or(false);
		if was_enabled {
			self.enable_foreign_keys().await
		} else {
			self.disable_foreign_keys().await
		}
	}

	#[cfg(feature = "sqlite")]
	async fn rollback_sqlite_recreation_session(&mut self) -> Result<()> {
		self.execute("ROLLBACK").await?;
		self.mark_sqlite_recreation_transaction_finished();
		self.restore_sqlite_recreation_foreign_keys().await?;
		self.release_sqlite_recreation_session();
		Ok(())
	}

	#[cfg(feature = "sqlite")]
	async fn with_non_atomic_sqlite_recreation_session<T: Send>(
		&mut self,
		operation: impl for<'a> FnOnce(
			&'a mut Self,
		) -> Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>,
	) -> Result<T> {
		let pool = self.connection.into_sqlite().ok_or_else(|| {
			MigrationError::UnsupportedDatabase(
				"SQLite recreation requires an SQLite connection pool".to_string(),
			)
		})?;
		let connection = pool.acquire().await?;
		self.sqlite_recreation_session = Some(SqliteRecreationSession::new(connection));
		let mut scope = SqliteRecreationScope::new(self);

		let was_enabled = scope.editor.foreign_keys_enabled().await?;
		scope
			.editor
			.set_sqlite_recreation_previous_foreign_keys(was_enabled);
		if was_enabled {
			scope.editor.disable_foreign_keys().await?;
		}
		scope.editor.mark_sqlite_recreation_transaction_started();
		scope.editor.execute("BEGIN").await?;

		let operation_result =
			match std::panic::catch_unwind(AssertUnwindSafe(|| operation(scope.editor))) {
				Ok(operation) => AssertUnwindSafe(operation).catch_unwind().await,
				Err(payload) => Err(payload),
			};

		match operation_result {
			Ok(operation_result) => {
				let transaction_result = if operation_result.is_ok() {
					scope.editor.execute("COMMIT").await
				} else {
					scope.editor.execute("ROLLBACK").await
				};
				if transaction_result.is_ok() {
					scope.editor.mark_sqlite_recreation_transaction_finished();
				}
				let restore_result = if was_enabled {
					scope.editor.enable_foreign_keys().await
				} else {
					Ok(())
				};
				let cleanup_result =
					merge_foreign_key_scope_results(transaction_result, restore_result);
				if cleanup_result.is_ok() {
					scope.finish();
				}
				merge_foreign_key_scope_results(operation_result, cleanup_result)
			}
			Err(payload) => {
				let rollback_result = scope.editor.execute("ROLLBACK").await;
				if rollback_result.is_ok() {
					scope.editor.mark_sqlite_recreation_transaction_finished();
				}
				let restore_result = if was_enabled {
					scope.editor.enable_foreign_keys().await
				} else {
					Ok(())
				};
				let cleanup_result =
					merge_foreign_key_scope_results(rollback_result, restore_result);
				if cleanup_result.is_ok() {
					scope.finish();
				} else if let Err(cleanup_error) = cleanup_result {
					tracing::error!(
						"failed to clean up SQLite recreation session after panic: {cleanup_error}"
					);
				}
				resume_unwind(payload)
			}
		}
	}

	#[cfg(feature = "sqlite")]
	fn set_sqlite_recreation_previous_foreign_keys(&mut self, enabled: bool) {
		self.sqlite_recreation_session
			.as_mut()
			.expect("SQLite recreation session must be active")
			.set_previous_foreign_keys(enabled);
	}

	#[cfg(feature = "sqlite")]
	fn mark_sqlite_recreation_transaction_started(&mut self) {
		self.sqlite_recreation_session
			.as_mut()
			.expect("SQLite recreation session must be active")
			.mark_transaction_started();
	}

	#[cfg(feature = "sqlite")]
	fn mark_sqlite_recreation_transaction_finished(&mut self) {
		self.sqlite_recreation_session
			.as_mut()
			.expect("SQLite recreation session must be active")
			.mark_transaction_finished();
	}

	#[cfg(feature = "sqlite")]
	fn release_sqlite_recreation_session(&mut self) {
		if let Some(session) = self.sqlite_recreation_session.as_mut() {
			session.mark_clean();
		}
		self.sqlite_recreation_session.take();
	}

	#[cfg(feature = "sqlite")]
	async fn foreign_keys_enabled(&mut self) -> Result<bool> {
		if !matches!(self.db_type, DatabaseType::Sqlite) {
			return Ok(false);
		}

		if let Some(session) = self.sqlite_recreation_session.as_mut() {
			let enabled = sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
				.fetch_one(&mut **session.connection_mut())
				.await?;
			return Ok(enabled != 0);
		}

		let row = if let Some(ref mut tx) = self.executor {
			tx.fetch_optional("PRAGMA foreign_keys", vec![]).await?
		} else {
			self.connection
				.fetch_optional("PRAGMA foreign_keys", vec![])
				.await?
		};
		Ok(row
			.and_then(|row| row.get::<i64>("foreign_keys").ok())
			.is_some_and(|enabled| enabled != 0))
	}

	/// Check foreign key integrity (SQLite only)
	///
	/// This should be called after table recreation to verify that all
	/// foreign key relationships are valid. If violations are found,
	/// they will be returned as a vector of violation descriptions.
	///
	/// # Returns
	///
	/// A vector of foreign key violation descriptions (empty if no violations).
	/// Returns an empty vector immediately for non-SQLite databases.
	#[cfg(feature = "sqlite")]
	pub async fn check_foreign_key_integrity(&mut self) -> Result<Vec<String>> {
		if !matches!(self.db_type, DatabaseType::Sqlite) {
			return Ok(Vec::new());
		}

		tracing::debug!("Checking SQLite foreign key integrity");

		// PRAGMA foreign_key_check returns rows with:
		// table, rowid, parent_table, fkid
		let sql = "PRAGMA foreign_key_check";
		if let Some(session) = self.sqlite_recreation_session.as_mut() {
			let rows = sqlx::query(sql)
				.fetch_all(&mut **session.connection_mut())
				.await?;
			let violations = rows
				.into_iter()
				.map(|row| {
					let table = row.try_get::<String, _>("table").unwrap_or_default();
					let rowid = row.try_get::<i64, _>("rowid").unwrap_or_default();
					let parent = row.try_get::<String, _>("parent").unwrap_or_default();
					format!("FK violation in '{table}' row {rowid} referencing '{parent}'")
				})
				.collect();
			return Ok(violations);
		}

		let rows = if let Some(ref mut tx) = self.executor {
			tx.fetch_all(sql, vec![]).await?
		} else {
			self.connection.fetch_all(sql, vec![]).await?
		};

		let violations: Vec<String> = rows
			.into_iter()
			.map(|row| {
				// PRAGMA foreign_key_check returns: table, rowid, parent, fkid
				let table: String = row.get("table").unwrap_or_default();
				let rowid: i64 = row.get("rowid").unwrap_or_default();
				let parent_table: String = row.get("parent").unwrap_or_default();
				format!(
					"FK violation in '{}' row {} referencing '{}'",
					table, rowid, parent_table
				)
			})
			.collect();

		if !violations.is_empty() {
			tracing::warn!("Foreign key violations found: {:?}", violations);
		}

		Ok(violations)
	}
}

#[cfg(feature = "sqlite")]
struct SqliteRecreationScope<'a> {
	editor: &'a mut SchemaEditor,
	finished: bool,
}

#[cfg(feature = "sqlite")]
impl<'a> SqliteRecreationScope<'a> {
	fn new(editor: &'a mut SchemaEditor) -> Self {
		Self {
			editor,
			finished: false,
		}
	}

	fn finish(&mut self) {
		self.editor.release_sqlite_recreation_session();
		self.finished = true;
	}
}

#[cfg(feature = "sqlite")]
impl Drop for SqliteRecreationScope<'_> {
	fn drop(&mut self) {
		if !self.finished {
			self.editor.sqlite_recreation_session.take();
		}
	}
}

#[cfg(feature = "sqlite")]
fn merge_foreign_key_scope_results<T>(
	operation_result: Result<T>,
	restore_result: Result<()>,
) -> Result<T> {
	match (operation_result, restore_result) {
		(Ok(value), Ok(())) => Ok(value),
		(Err(operation_error), Ok(())) => Err(operation_error),
		(Ok(_), Err(restore_error)) => Err(restore_error),
		(Err(operation_error), Err(restore_error)) => {
			Err(MigrationError::InvalidMigration(format!(
				"SQLite schema operation failed: {operation_error}; additionally failed to restore foreign key enforcement: {restore_error}"
			)))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_database_type_transactional_ddl() {
		assert!(DatabaseType::Postgres.supports_transactional_ddl());
		assert!(DatabaseType::Sqlite.supports_transactional_ddl());
		assert!(!DatabaseType::Mysql.supports_transactional_ddl());
	}

	#[cfg(feature = "sqlite")]
	#[test]
	fn foreign_key_scope_preserves_operation_error_when_restore_succeeds() {
		let result = merge_foreign_key_scope_results::<()>(
			Err(MigrationError::InvalidMigration(
				"operation failed".to_string(),
			)),
			Ok(()),
		);

		assert!(matches!(
			result,
			Err(MigrationError::InvalidMigration(message)) if message == "operation failed"
		));
	}

	#[cfg(feature = "sqlite")]
	#[test]
	fn foreign_key_scope_reports_operation_and_restore_errors() {
		let result = merge_foreign_key_scope_results::<()>(
			Err(MigrationError::InvalidMigration(
				"operation failed".to_string(),
			)),
			Err(MigrationError::InvalidMigration(
				"restore failed".to_string(),
			)),
		);

		let Err(MigrationError::InvalidMigration(message)) = result else {
			panic!("combined scope failure should be an invalid migration error");
		};
		assert!(message.contains("operation failed"), "{message}");
		assert!(message.contains("restore failed"), "{message}");
	}

	#[cfg(feature = "sqlite")]
	#[test]
	fn foreign_key_scope_returns_restore_error_after_successful_operation() {
		let result = merge_foreign_key_scope_results(
			Ok(()),
			Err(MigrationError::InvalidMigration(
				"restore failed".to_string(),
			)),
		);

		assert!(matches!(
			result,
			Err(MigrationError::InvalidMigration(message)) if message == "restore failed"
		));
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn foreign_key_scope_restores_state_before_resuming_panic() {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("connect to in-memory SQLite");
		connection
			.execute("PRAGMA foreign_keys = ON", vec![])
			.await
			.expect("enable foreign key enforcement");
		let mut editor = SchemaEditor::new(connection, false, DatabaseType::Sqlite)
			.await
			.expect("create non-atomic schema editor");

		let panic_result = AssertUnwindSafe(editor.with_foreign_keys_disabled::<()>(|_| {
			Box::pin(async { panic!("schema operation panic") })
		}))
		.catch_unwind()
		.await;

		assert!(panic_result.is_err(), "operation panic must be resumed");
		assert!(
			editor
				.foreign_keys_enabled()
				.await
				.expect("read restored foreign key state"),
			"foreign key enforcement must be restored before resuming the panic"
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn recreation_cleanup_returns_connection_when_transaction_is_already_inactive() {
		use sqlx::sqlite::SqlitePoolOptions;

		// Arrange
		let pool = SqlitePoolOptions::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.expect("connect to in-memory SQLite");
		sqlx::query("PRAGMA foreign_keys = ON")
			.execute(&pool)
			.await
			.expect("enable foreign key enforcement");
		sqlx::query("CREATE TABLE cleanup_boundary_marker (id INTEGER PRIMARY KEY)")
			.execute(&pool)
			.await
			.expect("create marker table");
		let connection = pool.acquire().await.expect("acquire sole connection");
		let mut session = SqliteRecreationSession::new(connection);
		session.set_previous_foreign_keys(true);
		sqlx::query("PRAGMA foreign_keys = OFF")
			.execute(&mut **session.connection_mut())
			.await
			.expect("disable foreign key enforcement");
		session.mark_transaction_started();

		// Act
		drop(session);
		let mut returned_connection = pool
			.acquire()
			.await
			.expect("cleanup should return the sole connection");

		// Assert
		let foreign_keys = sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
			.fetch_one(&mut *returned_connection)
			.await
			.expect("read restored foreign key state");
		assert_eq!(foreign_keys, 1, "foreign key enforcement must be restored");
		let marker_exists = sqlx::query_scalar::<_, i64>(
			"SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'cleanup_boundary_marker'",
		)
		.fetch_one(&mut *returned_connection)
		.await
		.expect("check marker table");
		assert_eq!(
			marker_exists, 1,
			"cleanup must preserve the original in-memory connection"
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn atomic_sqlite_recreation_abort_restores_connection_and_schema() {
		use sqlx::sqlite::SqlitePoolOptions;
		use tokio::sync::oneshot;

		// Arrange
		let pool = SqlitePoolOptions::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.expect("connect to in-memory SQLite");
		let connection = DatabaseConnection::from_sqlite_pool(pool);
		connection
			.execute("PRAGMA foreign_keys = ON", vec![])
			.await
			.expect("enable foreign key enforcement");
		connection
			.execute(
				"CREATE TABLE atomic_cleanup_parent (id INTEGER PRIMARY KEY)",
				vec![],
			)
			.await
			.expect("create parent table");
		connection
			.execute("INSERT INTO atomic_cleanup_parent (id) VALUES (1)", vec![])
			.await
			.expect("insert parent row");
		let assertion_connection = connection.clone();
		let (recreation_started_tx, recreation_started_rx) = oneshot::channel();

		// Act
		let recreation = tokio::spawn(async move {
			let mut editor =
				SchemaEditor::new_for_migration(connection, true, DatabaseType::Sqlite, true)
					.await
					.expect("create atomic SQLite recreation editor");
			editor
				.execute("CREATE TABLE atomic_cleanup_temporary (id INTEGER)")
				.await
				.expect("create temporary table inside migration transaction");
			let _ = recreation_started_tx.send(());
			std::future::pending::<()>().await;
		});
		recreation_started_rx
			.await
			.expect("recreation should reach the cancellation point");
		recreation.abort();
		let cancellation = recreation.await;

		// Assert
		assert!(cancellation.is_err(), "recreation task must be cancelled");
		let foreign_keys: i64 = assertion_connection
			.fetch_one("PRAGMA foreign_keys", vec![])
			.await
			.expect("read restored foreign key state")
			.get("foreign_keys")
			.expect("foreign_keys should be an integer");
		assert_eq!(foreign_keys, 1, "foreign key enforcement must be restored");
		let original_rows: i64 = assertion_connection
			.fetch_one(
				"SELECT COUNT(*) AS count FROM atomic_cleanup_parent",
				vec![],
			)
			.await
			.expect("read original table")
			.get("count")
			.expect("count should be an integer");
		assert_eq!(original_rows, 1, "original data must remain intact");
		let temporary_table = assertion_connection
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'atomic_cleanup_temporary'",
				vec![],
			)
			.await
			.expect("check temporary table cleanup");
		assert!(
			temporary_table.is_none(),
			"uncommitted temporary table must be rolled back"
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn non_atomic_sqlite_recreation_abort_restores_connection_and_schema() {
		use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
		use std::str::FromStr;
		use tokio::sync::oneshot;

		// Arrange
		let temp_dir = tempfile::tempdir().expect("create temporary database directory");
		let database_path = temp_dir.path().join("recreation.sqlite3");
		let options =
			SqliteConnectOptions::from_str(&format!("sqlite://{}", database_path.display()))
				.expect("build SQLite connection options")
				.create_if_missing(true);
		let pool = SqlitePoolOptions::new()
			.max_connections(1)
			.min_connections(1)
			.connect_with(options)
			.await
			.expect("connect to file SQLite database");
		let connection = DatabaseConnection::from_sqlite_pool(pool);
		connection
			.execute(
				"CREATE TABLE cleanup_parent (id INTEGER PRIMARY KEY)",
				vec![],
			)
			.await
			.expect("create parent table");
		connection
			.execute(
				"CREATE TABLE cleanup_child (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES cleanup_parent(id), obsolete TEXT)",
				vec![],
			)
			.await
			.expect("create child table");
		connection
			.execute("INSERT INTO cleanup_parent (id) VALUES (1)", vec![])
			.await
			.expect("insert parent row");
		connection
			.execute(
				"INSERT INTO cleanup_child (id, parent_id, obsolete) VALUES (1, 1, 'keep')",
				vec![],
			)
			.await
			.expect("insert child row");
		let assertion_connection = connection.clone();
		let (recreation_started_tx, recreation_started_rx) = oneshot::channel();

		// Act
		let recreation = tokio::spawn(async move {
			let mut editor = SchemaEditor::new(connection, false, DatabaseType::Sqlite)
				.await
				.expect("create non-atomic schema editor");
			editor
				.with_foreign_keys_disabled(move |editor| {
					Box::pin(async move {
						editor
							.execute(
								"CREATE TABLE cleanup_child_new (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES cleanup_parent(id))",
							)
							.await?;
						let _ = recreation_started_tx.send(());
						std::future::pending::<Result<()>>().await
					})
				})
				.await
		});
		recreation_started_rx
			.await
			.expect("recreation should reach the cancellation point");
		recreation.abort();
		let cancellation = recreation.await;

		// Assert
		assert!(cancellation.is_err(), "recreation task must be cancelled");
		let foreign_keys: i64 = assertion_connection
			.fetch_one("PRAGMA foreign_keys", vec![])
			.await
			.expect("read foreign key state after cancellation")
			.get("foreign_keys")
			.expect("foreign_keys should be an integer");
		assert_eq!(foreign_keys, 1, "foreign key enforcement must be restored");
		let invalid_write = assertion_connection
			.execute(
				"INSERT INTO cleanup_child (id, parent_id) VALUES (2, 999)",
				vec![],
			)
			.await;
		assert!(
			invalid_write.is_err(),
			"restored foreign key enforcement must reject invalid child rows"
		);
		let original_rows: i64 = assertion_connection
			.fetch_one("SELECT COUNT(*) AS count FROM cleanup_child", vec![])
			.await
			.expect("read original child table")
			.get("count")
			.expect("count should be an integer");
		assert_eq!(original_rows, 1, "original data must remain intact");
		let replacement = assertion_connection
			.fetch_optional(
				"SELECT name FROM sqlite_schema WHERE type = 'table' AND name = 'cleanup_child_new'",
				vec![],
			)
			.await
			.expect("inspect replacement table");
		assert!(
			replacement.is_none(),
			"replacement table must be rolled back"
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn non_atomic_sqlite_recreation_abort_preserves_in_memory_database() {
		use tokio::sync::oneshot;

		// Arrange
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("connect to in-memory SQLite");
		connection
			.execute("CREATE TABLE kept (id INTEGER PRIMARY KEY)", vec![])
			.await
			.expect("create original table");
		connection
			.execute("INSERT INTO kept (id) VALUES (1)", vec![])
			.await
			.expect("insert original row");
		let assertion_connection = connection.clone();
		let (recreation_started_tx, recreation_started_rx) = oneshot::channel();

		// Act
		let recreation = tokio::spawn(async move {
			let mut editor = SchemaEditor::new(connection, false, DatabaseType::Sqlite)
				.await
				.expect("create non-atomic schema editor");
			editor
				.with_foreign_keys_disabled(move |editor| {
					Box::pin(async move {
						editor
							.execute("CREATE TABLE kept_new (id INTEGER PRIMARY KEY)")
							.await?;
						let _ = recreation_started_tx.send(());
						std::future::pending::<Result<()>>().await
					})
				})
				.await
		});
		recreation_started_rx
			.await
			.expect("recreation should reach the cancellation point");
		recreation.abort();
		assert!(
			recreation.await.is_err(),
			"recreation task must be cancelled"
		);

		// Assert
		let foreign_keys: i64 = assertion_connection
			.fetch_one("PRAGMA foreign_keys", vec![])
			.await
			.expect("read foreign key state after cancellation")
			.get("foreign_keys")
			.expect("foreign_keys should be an integer");
		assert_eq!(foreign_keys, 1, "foreign key enforcement must be restored");
		let original_rows: i64 = assertion_connection
			.fetch_one("SELECT COUNT(*) AS count FROM kept", vec![])
			.await
			.expect("read original in-memory table")
			.get("count")
			.expect("count should be an integer");
		assert_eq!(original_rows, 1, "in-memory database must remain intact");
		let replacement = assertion_connection
			.fetch_optional(
				"SELECT name FROM sqlite_schema WHERE type = 'table' AND name = 'kept_new'",
				vec![],
			)
			.await
			.expect("inspect replacement table");
		assert!(
			replacement.is_none(),
			"replacement table must be rolled back"
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn foreign_key_scope_restores_state_after_future_factory_panic() {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("connect to in-memory SQLite");
		let mut editor = SchemaEditor::new(connection, false, DatabaseType::Sqlite)
			.await
			.expect("create non-atomic schema editor");

		let panic_result = AssertUnwindSafe(
			editor.with_foreign_keys_disabled::<()>(|_| panic!("future factory panic")),
		)
		.catch_unwind()
		.await;

		assert!(
			panic_result.is_err(),
			"future factory panic must be resumed"
		);
		assert!(
			editor
				.foreign_keys_enabled()
				.await
				.expect("read restored foreign key state"),
			"foreign key enforcement must be restored after a future factory panic"
		);
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn non_atomic_sqlite_recreation_reads_use_dedicated_connection() {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("connect to in-memory SQLite");
		let mut editor = SchemaEditor::new(connection, false, DatabaseType::Sqlite)
			.await
			.expect("create non-atomic schema editor");

		let observed_foreign_keys = tokio::time::timeout(
			std::time::Duration::from_secs(1),
			editor.with_foreign_keys_disabled(|editor| {
				Box::pin(async move {
					let row = editor
						.fetch_optional("PRAGMA foreign_keys", vec![])
						.await?
						.expect("foreign_keys pragma should return one row");
					row.get::<i64>("foreign_keys").map_err(|error| {
						MigrationError::InvalidMigration(format!(
							"failed to read foreign_keys: {error}"
						))
					})
				})
			}),
		)
		.await
		.expect("session read must not wait for its own pooled connection")
		.expect("session read should succeed");

		assert_eq!(
			observed_foreign_keys, 0,
			"callback reads must observe the dedicated connection state"
		);
	}
}
