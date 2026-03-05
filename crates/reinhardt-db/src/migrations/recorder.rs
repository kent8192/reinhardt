//! Migration recorder

use crate::backends::DatabaseConnection;
use chrono::{DateTime, Utc};

/// Migration record
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRecord {
	pub app: String,
	pub name: String,
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

impl MigrationRecorder {
	pub fn new() -> Self {
		Self {
			records: Vec::new(),
		}
	}

	pub fn record_applied(&mut self, app: &str, name: &str) {
		self.records.push(MigrationRecord {
			app: app.to_string(),
			name: name.to_string(),
			applied: Utc::now(),
		});
	}

	pub fn get_applied_migrations(&self) -> &[MigrationRecord] {
		&self.records
	}

	pub fn is_applied(&self, app: &str, name: &str) -> bool {
		self.records.iter().any(|r| r.app == app && r.name == name)
	}

	pub fn ensure_schema_table(&self) {
		// Ensure migration schema table exists
	}

	// Async versions for database operations
	pub async fn ensure_schema_table_async<T>(&self, _pool: &T) -> super::Result<()> {
		Ok(())
	}

	pub async fn is_applied_async<T>(
		&self,
		_pool: &T,
		app: &str,
		name: &str,
	) -> super::Result<bool> {
		Ok(self.is_applied(app, name))
	}

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

	pub async fn ensure_schema_table(&self) -> super::Result<()> {
		// Acquire advisory lock to prevent concurrent schema modifications
		self.acquire_schema_lock().await?;

		// Execute schema operations
		let result = self.ensure_schema_table_internal().await;

		// Always release lock, even if operations failed
		let _ = self.release_schema_lock().await;

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
			.map_err(super::MigrationError::DatabaseError)?;

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
					.map_err(super::MigrationError::DatabaseError)?;
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
					.map_err(super::MigrationError::DatabaseError)?;

				// Try to get the lock status as i64 or bool
				let locked = if let Ok(val) = result.get::<i64>("locked") {
					val == 1
				} else {
					result.get::<bool>("locked").unwrap_or_default()
				};

				if !locked {
					return Err(super::MigrationError::DatabaseError(
						crate::backends::DatabaseError::QueryError(
							"Failed to acquire migration lock (timeout)".to_string(),
						),
					));
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
					.map_err(super::MigrationError::DatabaseError)?;
			}
			DatabaseType::Mysql => {
				self.connection
					.execute("SELECT RELEASE_LOCK('reinhardt_migrations')", vec![])
					.await
					.map_err(super::MigrationError::DatabaseError)?;
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
	async fn ensure_schema_table_internal(&self) -> super::Result<()> {
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
			.map_err(super::MigrationError::DatabaseError)?;

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
					.map_err(super::MigrationError::DatabaseError)?;
			}
		} else {
			// PostgreSQL, SQLite handle IF NOT EXISTS correctly
			self.connection
				.execute(&create_index_sql, vec![])
				.await
				.map_err(super::MigrationError::DatabaseError)?;
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
			.map_err(super::MigrationError::DatabaseError)?;

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
		use crate::backends::types::DatabaseType;
		use reinhardt_query::prelude::{
			Alias, MySqlQueryBuilder, PostgresQueryBuilder, Query, QueryStatementBuilder,
			SqliteQueryBuilder,
		};

		// Build INSERT query using reinhardt-query
		let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
		let stmt = Query::insert()
			.into_table(Alias::new("reinhardt_migrations"))
			.columns([Alias::new("app"), Alias::new("name"), Alias::new("applied")])
			.values_panic([app.to_string(), name.to_string(), now])
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

		self.connection
			.execute(&sql, vec![])
			.await
			.map_err(super::MigrationError::DatabaseError)?;

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
			.map_err(super::MigrationError::DatabaseError)?;

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
							super::MigrationError::DatabaseError(
								crate::backends::DatabaseError::TypeError(format!(
									"Failed to parse SQLite timestamp '{}': {}",
									applied_str, e
								)),
							)
						})?
				}
				_ => row
					.get("applied")
					.map_err(super::MigrationError::DatabaseError)?,
			};

			records.push(MigrationRecord { app, name, applied });
		}

		Ok(records)
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
			.map_err(super::MigrationError::DatabaseError)?;

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
			.map_err(super::MigrationError::DatabaseError)?;

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
							super::MigrationError::DatabaseError(
								crate::backends::DatabaseError::TypeError(format!(
									"Failed to parse SQLite timestamp '{}': {}",
									applied_str, e
								)),
							)
						})?
				}
				_ => row
					.get("applied")
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
