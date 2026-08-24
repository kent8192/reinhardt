//! Migration executor
//!
//! Translated from Django's db/migrations/executor.py

// Allow unused_imports: ForeignKeyAction is used in database-specific code
// that may be conditionally compiled based on feature flags
#[allow(unused_imports)]
use super::{
	DatabaseMigrationRecorder, ForeignKeyAction, Migration, MigrationError, MigrationPlan,
	MigrationService, Operation, Result, SchemaEditor, operations::SqlDialect,
};
use crate::backends::{connection::DatabaseConnection, types::DatabaseType};
use indexmap::IndexMap;
#[cfg(feature = "sqlite")]
use std::collections::HashMap;
use std::collections::HashSet;

#[cfg(feature = "sqlite")]
use super::introspection::SQLiteIntrospector;

/// Split SQL string into individual statements while handling:
/// - String literals (single/double quotes)
/// - Comments (line and block)
/// - PostgreSQL dollar-quotes ($$...$$)
fn split_sql_statements(sql: &str) -> Vec<String> {
	let mut statements = Vec::new();
	let mut current = String::new();
	let mut chars = sql.chars().peekable();

	#[derive(Debug, PartialEq)]
	enum State {
		Normal,
		SingleQuote,
		DoubleQuote,
		LineComment,
		BlockComment,
		DollarQuote(String),
	}

	let mut state = State::Normal;

	while let Some(ch) = chars.next() {
		match state {
			State::Normal => {
				if ch == '\'' {
					current.push(ch);
					state = State::SingleQuote;
				} else if ch == '"' {
					current.push(ch);
					state = State::DoubleQuote;
				} else if ch == '-' && chars.peek() == Some(&'-') {
					current.push(ch);
					current.push(chars.next().unwrap());
					state = State::LineComment;
				} else if ch == '/' && chars.peek() == Some(&'*') {
					current.push(ch);
					current.push(chars.next().unwrap());
					state = State::BlockComment;
				} else if ch == '$' {
					// Potential dollar-quote start
					let mut tag = String::from("$");
					current.push(ch);

					// Collect tag until next $
					while let Some(&next_ch) = chars.peek() {
						if next_ch == '$' {
							tag.push(chars.next().unwrap());
							current.push('$');
							state = State::DollarQuote(tag);
							break;
						} else if next_ch.is_alphanumeric() || next_ch == '_' {
							tag.push(chars.next().unwrap());
							current.push(next_ch);
						} else {
							// Not a valid dollar-quote tag
							break;
						}
					}
				} else if ch == ';' {
					// Statement separator - save current statement if non-empty
					let trimmed = current.trim();
					if !trimmed.is_empty() {
						statements.push(trimmed.to_string());
					}
					current.clear();
				} else {
					current.push(ch);
				}
			}
			State::SingleQuote => {
				current.push(ch);
				if ch == '\'' {
					// Check for escaped quote ''
					if chars.peek() == Some(&'\'') {
						current.push(chars.next().unwrap());
					} else {
						state = State::Normal;
					}
				} else if ch == '\\' && chars.peek().is_some() {
					// Escaped character
					current.push(chars.next().unwrap());
				}
			}
			State::DoubleQuote => {
				current.push(ch);
				if ch == '"' {
					state = State::Normal;
				} else if ch == '\\' && chars.peek().is_some() {
					// Escaped character
					current.push(chars.next().unwrap());
				}
			}
			State::LineComment => {
				current.push(ch);
				if ch == '\n' {
					state = State::Normal;
				}
			}
			State::BlockComment => {
				current.push(ch);
				if ch == '*' && chars.peek() == Some(&'/') {
					current.push(chars.next().unwrap());
					state = State::Normal;
				}
			}
			State::DollarQuote(ref tag) => {
				current.push(ch);
				// Check if we're at the closing tag
				if ch == '$' {
					let mut potential_close = String::from("$");
					let mut temp_chars = vec![];

					// Collect potential closing tag
					while let Some(&next_ch) = chars.peek() {
						if next_ch == '$' {
							potential_close.push(chars.next().unwrap());
							temp_chars.push('$');
							break;
						} else if potential_close.len() < tag.len()
							&& (next_ch.is_alphanumeric() || next_ch == '_')
						{
							potential_close.push(chars.next().unwrap());
							temp_chars.push(next_ch);
						} else {
							break;
						}
					}

					// Add collected characters to current
					for temp_ch in &temp_chars {
						current.push(*temp_ch);
					}

					// Check if it matches the opening tag
					if potential_close == *tag {
						state = State::Normal;
					}
				}
			}
		}
	}

	// Add final statement if non-empty
	let trimmed = current.trim();
	if !trimmed.is_empty() {
		statements.push(trimmed.to_string());
	}

	statements
}

#[derive(Debug)]
/// Represents a execution result.
pub struct ExecutionResult {
	/// The applied.
	pub applied: Vec<String>,
	/// The failed.
	pub failed: Option<String>,
}

/// Migration executor using DatabaseConnection (supports multiple database types)
pub struct DatabaseMigrationExecutor {
	connection: DatabaseConnection,
	recorder: DatabaseMigrationRecorder,
	db_type: DatabaseType,
}

impl DatabaseMigrationExecutor {
	/// Create a new migration executor with DatabaseConnection
	///
	/// The database type is automatically detected from the connection.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// // Example: connecting to a PostgreSQL database
	/// let db = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let executor = DatabaseMigrationExecutor::new(db.clone());
	/// // Database type is automatically detected as PostgreSQL
	/// # });
	/// ```
	pub fn new(connection: DatabaseConnection) -> Self {
		let db_type = connection.database_type();
		let recorder = DatabaseMigrationRecorder::new(connection.clone());
		Self {
			connection,
			recorder,
			db_type,
		}
	}

	/// Get a reference to the database connection
	pub fn connection(&self) -> &DatabaseConnection {
		&self.connection
	}

	/// Get the database type
	pub fn database_type(&self) -> DatabaseType {
		self.db_type
	}

	/// Check if a table exists in the database
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// let db = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let executor = DatabaseMigrationExecutor::new(db);
	/// let exists = executor.table_exists("users").await.unwrap();
	/// # }
	/// ```
	async fn table_exists(&self, table_name: &str) -> Result<bool> {
		use reinhardt_query::prelude::{
			Alias, Cond, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query,
			QueryStatementBuilder, SqliteQueryBuilder,
		};

		match self.db_type {
			DatabaseType::Postgres => {
				// Build parameterized query using reinhardt-query
				let subquery = Query::select()
					.expr(Expr::asterisk())
					.from((Alias::new("information_schema"), Alias::new("tables")))
					.cond_where(
						Cond::all()
							.add(Expr::col(Alias::new("table_schema")).eq("public"))
							.add(Expr::col(Alias::new("table_name")).eq(table_name)),
					)
					.to_owned();

				let query_str = format!(
					"SELECT EXISTS ({})",
					subquery.to_string(PostgresQueryBuilder)
				);

				// For PostgreSQL, EXISTS returns a boolean value
				let result = self.connection.fetch_one(&query_str, vec![]).await?;
				match result.data.get("exists") {
					Some(crate::backends::types::QueryValue::Bool(b)) => Ok(*b),
					_ => Ok(false),
				}
			}
			DatabaseType::Sqlite => {
				// Build parameterized query using reinhardt-query
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

				// For SQLite, check if any row is returned
				let result = self.connection.fetch_optional(&query_str, vec![]).await?;
				Ok(result.is_some())
			}
			DatabaseType::Mysql => {
				// Build parameterized query using reinhardt-query
				let query = Query::select()
					.column(Alias::new("TABLE_NAME"))
					.from((Alias::new("information_schema"), Alias::new("tables")))
					.cond_where(
						Cond::all()
							.add(Expr::col(Alias::new("table_schema")).eq(Expr::cust("DATABASE()")))
							.add(Expr::col(Alias::new("table_name")).eq(table_name)),
					)
					.to_owned();

				let query_str = query.to_string(MySqlQueryBuilder);

				// For MySQL, check if any row is returned
				let result = self.connection.fetch_optional(&query_str, vec![]).await?;
				Ok(result.is_some())
			}
		}
	}

	/// Performs the apply migrations operation.
	pub async fn apply_migrations(&mut self, migrations: &[Migration]) -> Result<ExecutionResult> {
		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self
				.apply_migrations_with_cockroachdb_schema_lock(migrations)
				.await;
		}

		// Ensure the migration recorder table exists
		self.recorder.ensure_schema_table().await?;
		self.apply_migrations_after_schema_table(migrations).await
	}

	#[cfg(feature = "postgres")]
	async fn apply_migrations_with_cockroachdb_schema_lock(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.apply_migrations_after_schema_table(migrations).await
	}

	async fn apply_migrations_after_schema_table(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let mut applied = Vec::new();

		// Build MigrationGraph for dependency resolution
		let mut graph = super::graph::MigrationGraph::new();

		for migration in migrations {
			let key = super::graph::MigrationKey::new(
				migration.app_label.clone(),
				migration.name.clone(),
			);
			let deps: Vec<super::graph::MigrationKey> = migration
				.dependencies
				.iter()
				.map(|(app, name)| super::graph::MigrationKey::new(app.clone(), name.clone()))
				.collect();

			graph.add_migration(key, deps);
		}

		// Perform topological sort (automatically detects circular dependencies)
		let sorted_keys = graph.topological_sort()?;

		// Apply migrations in dependency-resolved order
		for key in sorted_keys {
			// Find the migration corresponding to this key
			let migration = migrations
				.iter()
				.find(|m| m.app_label == key.app_label && m.name == key.name)
				.ok_or_else(|| {
					MigrationError::DependencyError(format!("Migration not found: {}", key.id()))
				})?;

			// Check if already applied
			if self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?
			{
				continue;
			}

			// Apply migration operations
			self.apply_migration(migration).await?;

			// Record migration as applied
			self.recorder
				.record_applied(&migration.app_label, &migration.name)
				.await?;

			applied.push(migration.id());
		}

		Ok(ExecutionResult {
			applied,
			failed: None,
		})
	}

	/// Rollback (unapply) a list of migrations
	///
	/// Migrations are rolled back in reverse order (newest first).
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::{Migration, executor::DatabaseMigrationExecutor};
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # async fn example() {
	/// let connection = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let mut executor = DatabaseMigrationExecutor::new(connection);
	///
	/// let migrations = vec![Migration::new("0001_initial", "myapp")];
	/// let result = executor.rollback_migrations(&migrations).await.unwrap();
	/// # }
	/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
	/// ```
	pub async fn rollback_migrations(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self
				.rollback_migrations_with_cockroachdb_schema_lock(migrations)
				.await;
		}

		// Ensure the migration recorder table exists
		self.recorder.ensure_schema_table().await?;
		self.rollback_migrations_after_schema_table(migrations)
			.await
	}

	#[cfg(feature = "postgres")]
	async fn rollback_migrations_with_cockroachdb_schema_lock(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.rollback_migrations_after_schema_table(migrations)
			.await
	}

	async fn rollback_migrations_after_schema_table(
		&mut self,
		migrations: &[Migration],
	) -> Result<ExecutionResult> {
		let mut rolledback = Vec::new();

		// Process migrations in reverse order (newest first)
		for migration in migrations.iter().rev() {
			// Check if migration is actually applied
			let is_applied = self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?;

			if !is_applied {
				continue;
			}

			// Rollback the migration
			self.rollback_migration(migration).await?;

			// Remove from recorder
			self.recorder
				.unapply(&migration.app_label, &migration.name)
				.await?;

			rolledback.push(migration.id());
		}

		Ok(ExecutionResult {
			applied: rolledback,
			failed: None,
		})
	}

	/// Rollback a single migration
	async fn rollback_migration(&mut self, migration: &Migration) -> Result<()> {
		// Skip database operations if state_only flag is set
		if migration.state_only {
			tracing::debug!(
				"Skipping database operations for migration '{}' (state_only=true)",
				migration.id()
			);
			return Ok(());
		}

		// Determine SQL dialect
		let dialect = match self.connection.database_type() {
			crate::backends::types::DatabaseType::Postgres => SqlDialect::Postgres,
			crate::backends::types::DatabaseType::Mysql => SqlDialect::Mysql,
			crate::backends::types::DatabaseType::Sqlite => SqlDialect::Sqlite,
		};

		// Create SchemaEditor for atomic operations
		let mut editor = SchemaEditor::new(
			self.connection.clone(),
			migration.atomic,
			self.connection.database_type(),
		)
		.await?;

		// Process operations in reverse order
		let project_state = super::ProjectState::default();

		for operation in migration.operations.iter().rev() {
			// Check if SQLite and reverse operation requires recreation
			#[cfg(feature = "sqlite")]
			if matches!(dialect, SqlDialect::Sqlite)
				&& operation.reverse_requires_sqlite_recreation()
			{
				// Get the reverse operation and use table recreation
				if let Some(reverse_op) = operation.to_reverse_operation(&project_state)? {
					tracing::debug!("=== SQLite Recreation for reverse of {:?} ===", operation);
					self.handle_sqlite_recreation(&reverse_op, &mut editor)
						.await?;
					tracing::debug!("✅ SQLite recreation for reverse operation completed");
					continue;
				} else {
					tracing::warn!(
						"Cannot generate reverse operation for SQLite recreation: {:?}",
						operation
					);
					// Fall through to standard SQL execution
				}
			}

			// Standard reverse SQL execution
			let reverse_sql = operation.to_reverse_sql(&dialect, &project_state)?;

			if let Some(statements) = reverse_sql {
				tracing::debug!(
					"=== Reverse SQL for {:?} ({} statement(s)) ===",
					operation,
					statements.len()
				);
				// Dispatch each statement separately through SchemaEditor.
				// SchemaEditor::execute() is backed by sqlx Extended Query, which
				// accepts only one statement per payload — see operations.rs
				// `Operation::to_reverse_sql` for why some DDL reversals produce
				// multiple statements (e.g. AlterColumn on PostgreSQL/CockroachDB
				// splits type reversion and NOT NULL restoration).
				for sql in &statements {
					tracing::debug!("{}", sql);
					editor.execute(sql).await?;
				}

				tracing::debug!("✅ Reverse operation executed successfully");
			} else {
				tracing::warn!(
					"No reverse SQL available for operation in migration '{}': {:?}",
					migration.id(),
					operation
				);
			}
		}

		// Commit SchemaEditor changes
		editor.finish().await?;

		Ok(())
	}

	/// Apply a single migration with atomic transaction support
	///
	/// If the migration's `atomic` flag is true and the database supports
	/// transactional DDL (PostgreSQL, SQLite), all operations are wrapped
	/// in a transaction that can be rolled back on failure.
	///
	/// For databases that don't support transactional DDL (MySQL), operations
	/// are executed directly without transaction wrapping, and a warning is logged.
	async fn apply_migration(&self, migration: &Migration) -> Result<()> {
		// Skip database operations if state_only flag is set
		// (Django's SeparateDatabaseAndState equivalent with state_operations only)
		if migration.state_only {
			tracing::debug!(
				"Skipping database operations for migration '{}' (state_only=true)",
				migration.id()
			);
			return Ok(());
		}

		let dialect = match self.db_type {
			DatabaseType::Postgres => SqlDialect::Postgres,
			DatabaseType::Sqlite => SqlDialect::Sqlite,
			DatabaseType::Mysql => SqlDialect::Mysql,
		};

		// Create schema editor with atomic support based on migration's atomic flag
		let mut editor =
			SchemaEditor::new(self.connection.clone(), migration.atomic, self.db_type).await?;

		// Log if database_only flag is set
		// Note: ProjectState tracking during migration execution is a planned enhancement.
		// Currently, state is not tracked during apply_migration. For rollback operations,
		// use to_reverse_sql with a pre-operation ProjectState snapshot.
		if migration.database_only {
			tracing::debug!(
				"Skipping ProjectState updates for migration '{}' (database_only=true)",
				migration.id()
			);
		}

		tracing::debug!(
			"Applying migration '{}' (atomic={}, effective_atomic={})",
			migration.id(),
			migration.atomic,
			editor.is_atomic()
		);

		// Execute operations through schema editor
		for operation in &migration.operations {
			// Handle SQLite table recreation for incompatible operations
			#[cfg(feature = "sqlite")]
			if matches!(dialect, SqlDialect::Sqlite) && operation.requires_sqlite_recreation() {
				self.handle_sqlite_recreation(operation, &mut editor)
					.await?;
				continue;
			}

			// Check if this is a CreateTable operation and if the table already
			// exists. The check MUST run through the schema editor (and thus
			// through the editor's open transaction, if any) — otherwise the
			// pool may pick a different physical connection whose schema cache
			// is stale w.r.t. DDL committed earlier in this same migration.
			//
			// On SQLite, an introspection read on a separate pool connection
			// after a DDL on the transaction's connection raises SQLITE_SCHEMA
			// (code 262, "database schema is locked") because the schema
			// cookie has been bumped on the writer but the reader's prepared
			// statement cache is stale. See reinhardt-web#4584.
			if let Operation::CreateTable { name, .. } = operation {
				let table_exists = editor.table_exists(name).await?;
				if table_exists {
					tracing::info!(
						"Table '{}' already exists, skipping CREATE TABLE operation",
						name
					);
					continue;
				}
			}

			let sql = operation.to_sql(&dialect);

			tracing::debug!(
				"Executing migration SQL (length={}, semicolons={})",
				sql.len(),
				sql.matches(';').count()
			);

			// Split SQL into individual statements to handle PostgreSQL's
			// prepared statement limitation (cannot execute multiple commands)
			let statements = split_sql_statements(&sql);

			tracing::debug!("Split into {} statements", statements.len());

			for (i, statement) in statements.iter().enumerate() {
				if !statement.trim().is_empty() {
					tracing::debug!(
						"Statement {} (length: {} chars): {}",
						i + 1,
						statement.len(),
						&statement[..statement.len().min(100)]
					);

					editor.execute(statement).await.map_err(|e| {
						tracing::error!(
							"Migration operation failed: {}. SQL: {}",
							e,
							&statement[..statement.len().min(200)]
						);
						e
					})?;

					tracing::debug!("Statement {} executed successfully", i + 1);
				}
			}
		}

		// Finish (commits if atomic)
		editor.finish().await?;

		tracing::debug!("Migration '{}' applied successfully", migration.id());

		Ok(())
	}

	/// Apply migrations from a MigrationPlan
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::migrations::{MigrationPlan, executor::DatabaseMigrationExecutor};
	/// use reinhardt_db::backends::DatabaseConnection;
	///
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// // Example: connecting to a PostgreSQL database
	/// let db = DatabaseConnection::connect_postgres("postgres://localhost/mydb").await.unwrap();
	/// let mut executor = DatabaseMigrationExecutor::new(db);
	///
	/// let plan = MigrationPlan::new();
	/// let result = executor.apply(&plan).await.unwrap();
	/// # });
	/// ```
	pub async fn apply(&mut self, plan: &MigrationPlan) -> Result<ExecutionResult> {
		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self.apply_with_cockroachdb_schema_lock(plan).await;
		}

		// Ensure the migration recorder table exists
		self.recorder.ensure_schema_table().await?;
		self.apply_after_schema_table(plan).await
	}

	#[cfg(feature = "postgres")]
	async fn apply_with_cockroachdb_schema_lock(
		&mut self,
		plan: &MigrationPlan,
	) -> Result<ExecutionResult> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.apply_after_schema_table(plan).await
	}

	async fn apply_after_schema_table(&mut self, plan: &MigrationPlan) -> Result<ExecutionResult> {
		let mut applied = Vec::new();

		let dialect = match self.db_type {
			DatabaseType::Postgres => SqlDialect::Postgres,
			DatabaseType::Sqlite => SqlDialect::Sqlite,
			DatabaseType::Mysql => SqlDialect::Mysql,
		};

		for migration in &plan.migrations {
			// Check if already applied
			if self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?
			{
				continue;
			}

			// Apply migration
			for operation in &migration.operations {
				// Check if this is a CreateTable operation and if the table already exists
				if let Operation::CreateTable { name, .. } = operation {
					let table_exists = self.table_exists(name).await?;
					if table_exists {
						eprintln!(
							"⏭️  Table '{}' already exists, skipping CREATE TABLE operation",
							name
						);
						continue;
					}
				}

				let sql = operation.to_sql(&dialect);

				// Split SQL into individual statements to handle PostgreSQL's
				// prepared statement limitation (cannot execute multiple commands)
				for statement in split_sql_statements(&sql) {
					if !statement.trim().is_empty() {
						self.connection.execute(&statement, vec![]).await?;
					}
				}
			}

			// Record migration as applied
			self.recorder
				.record_applied(&migration.app_label, &migration.name)
				.await?;

			applied.push(migration.id());
		}

		Ok(ExecutionResult {
			applied,
			failed: None,
		})
	}

	/// Build migration plan - returns list of migrations to apply
	///
	/// Returns (app_label, migration_name) tuples in dependency order
	// Allow dead_code: public API for migration CLI tooling to preview pending migrations
	#[allow(dead_code)]
	pub async fn build_plan(&self, service: &MigrationService) -> Result<Vec<(String, String)>> {
		let graph = service.build_dependency_graph().await?;
		let mut plan = Vec::new();

		for migration in graph {
			let is_applied = self
				.recorder
				.is_applied(&migration.app_label, &migration.name)
				.await?;

			if !is_applied {
				plan.push((migration.app_label.to_string(), migration.name.to_string()));
			}
		}

		Ok(plan)
	}

	/// Read columns + constraints for a SQLite table using the SchemaEditor's
	/// currently-open transaction (if any), falling back to the live pool when
	/// the editor is non-atomic.
	///
	/// This exists because `SQLiteIntrospector` issues every query through the
	/// underlying `sqlx::SqlitePool`, which transparently picks a *different*
	/// physical connection than the one holding the editor's open transaction.
	/// That second connection cannot see uncommitted DDL — so a recreation
	/// triggered later in the same migration would rebuild the table from a
	/// stale column list and silently discard the just-`ALTER`'d column.
	/// See reinhardt-web#4447.
	#[cfg(feature = "sqlite")]
	async fn read_sqlite_table_via_editor(
		editor: &mut SchemaEditor,
		table_name: &str,
	) -> Result<(
		Vec<super::ColumnDefinition>,
		Vec<super::Constraint>,
		HashMap<String, String>,
	)> {
		// 1. PRAGMA table_info(<table>) → columns. Identifier interpolation
		//    via the shared `sqlite_pragma` helper. See issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let table_info_sql = format!(
			"PRAGMA table_info({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let info_rows = editor.fetch_all(&table_info_sql, vec![]).await?;

		// Collect rows into typed records first so we can detect AUTOINCREMENT.
		struct ColRow {
			name: String,
			type_str: String,
			notnull: i64,
			default: Option<String>,
			pk: i64,
		}
		let mut col_rows: Vec<ColRow> = Vec::with_capacity(info_rows.len());
		for row in &info_rows {
			let name: String = row
				.get("name")
				.map_err(|e| MigrationError::IntrospectionError(format!("table_info name: {e}")))?;
			let type_str: String = row.get("type").unwrap_or_default();
			let notnull: i64 = row.get("notnull").unwrap_or(0);
			let default: Option<String> = row.get("dflt_value").ok();
			let pk: i64 = row.get("pk").unwrap_or(0);
			col_rows.push(ColRow {
				name,
				type_str,
				notnull,
				default,
				pk,
			});
		}

		// 2. CREATE TABLE SQL → detect AUTOINCREMENT and parse named
		//    constraint metadata (FK names + CHECK constraints).
		let create_sql_row = editor
			.fetch_optional(
				"SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
				vec![table_name.into()],
			)
			.await?;
		let create_sql: Option<String> = create_sql_row.and_then(|r| r.get("sql").ok());
		let has_autoincrement = create_sql
			.as_ref()
			.map(|sql| sql.to_uppercase().contains("AUTOINCREMENT"))
			.unwrap_or(false);

		// 3. Build ColumnDefinition list, mirroring the introspector's
		//    semantics (PK columns are implicitly NOT NULL; AUTOINCREMENT is
		//    only meaningful on PK columns).
		let mut columns: Vec<super::ColumnDefinition> = col_rows
			.iter()
			.map(|c| {
				let is_pk = c.pk > 0;
				let is_auto = is_pk && has_autoincrement;
				let nullable = if is_pk { false } else { c.notnull == 0 };
				// Preserve `dflt_value` verbatim as the raw SQL fragment
				// (e.g. `'pending'` including surrounding quotes). The
				// downstream `format!("DEFAULT {}", default)` paths in
				// `operations.rs` then emit valid DDL (`DEFAULT 'pending'`,
				// not the previously broken `DEFAULT pending`). See
				// `super::sqlite_pragma` and issue #4454.
				let default = c
					.default
					.as_ref()
					.map(|v| super::sqlite_pragma::normalize_default_value(v));
				super::ColumnDefinition {
					name: c.name.clone(),
					type_definition: SQLiteIntrospector::parse_sqlite_type(&c.type_str),
					not_null: !nullable,
					unique: false,
					primary_key: is_pk,
					auto_increment: is_auto,
					default,
				}
			})
			.collect();

		// Preserve the introspector's ordering: PK first, then by name. This
		// matters because `SqliteTableRecreation` uses column order to emit
		// the new CREATE TABLE and the INSERT SELECT.
		columns.sort_by(|a, b| {
			if a.primary_key && !b.primary_key {
				std::cmp::Ordering::Less
			} else if !a.primary_key && b.primary_key {
				std::cmp::Ordering::Greater
			} else {
				a.name.cmp(&b.name)
			}
		});

		// 4. Foreign keys via PRAGMA foreign_key_list(<table>), grouped by
		//    id. Identifier interpolation via the shared `sqlite_pragma`
		//    helper. See issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let fk_sql = format!(
			"PRAGMA foreign_key_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let fk_rows = editor.fetch_all(&fk_sql, vec![]).await?;

		struct FkRow {
			id: i64,
			seq: i64,
			table: String,
			from: String,
			to: String,
			on_update: String,
			on_delete: String,
		}
		let parsed_fks: Vec<FkRow> = fk_rows
			.iter()
			.map(|row| FkRow {
				id: row.get("id").unwrap_or(0),
				seq: row.get("seq").unwrap_or(0),
				table: row.get("table").unwrap_or_default(),
				from: row.get("from").unwrap_or_default(),
				to: row.get("to").unwrap_or_default(),
				on_update: row.get("on_update").unwrap_or_default(),
				on_delete: row.get("on_delete").unwrap_or_default(),
			})
			.collect();

		let named_fks = create_sql
			.as_ref()
			.map(|sql| SQLiteIntrospector::parse_fk_constraint_names(sql))
			.unwrap_or_default();
		let named_unique_constraints = create_sql
			.as_ref()
			.map(|sql| SQLiteIntrospector::parse_unique_constraint_names(sql))
			.unwrap_or_default();

		let mut fk_groups: std::collections::HashMap<i64, Vec<FkRow>> =
			std::collections::HashMap::new();
		for r in parsed_fks {
			fk_groups.entry(r.id).or_default().push(r);
		}

		fn fk_action(s: &str) -> super::ForeignKeyAction {
			match s {
				"CASCADE" => super::ForeignKeyAction::Cascade,
				"SET NULL" => super::ForeignKeyAction::SetNull,
				"SET DEFAULT" => super::ForeignKeyAction::SetDefault,
				"NO ACTION" => super::ForeignKeyAction::NoAction,
				_ => super::ForeignKeyAction::Restrict,
			}
		}

		let mut constraints: Vec<super::Constraint> = Vec::new();
		let mut sqlite_constraint_aliases = HashMap::new();
		for (fk_id, mut group) in fk_groups {
			group.sort_by_key(|r| r.seq);
			let referenced_table = group[0].table.clone();
			let columns_from: Vec<String> = group.iter().map(|r| r.from.clone()).collect();
			let columns_to: Vec<String> = group.iter().map(|r| r.to.clone()).collect();
			let signature = (columns_from.clone(), referenced_table.clone());
			let name = named_fks
				.get(&signature)
				.cloned()
				.unwrap_or_else(|| format!("fk_{}_{}", table_name, fk_id));
			constraints.push(super::Constraint::ForeignKey {
				name,
				columns: columns_from,
				referenced_table,
				referenced_columns: columns_to,
				on_delete: fk_action(&group[0].on_delete),
				on_update: fk_action(&group[0].on_update),
				deferrable: None,
			});
		}

		// 5. Unique constraints via PRAGMA index_list / index_info where
		//    origin = 'u' (i.e. declared with the UNIQUE keyword or as a
		//    named CONSTRAINT … UNIQUE). Identifier interpolation via the
		//    shared `sqlite_pragma` helper. See issue #4454.
		// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
		let idx_list_sql = format!(
			"PRAGMA index_list({})",
			super::sqlite_pragma::quote_pragma_identifier(table_name)
		);
		let idx_rows = editor.fetch_all(&idx_list_sql, vec![]).await?;
		for row in &idx_rows {
			let origin: String = row.get("origin").unwrap_or_default();
			let unique: i64 = row.get("unique").unwrap_or(0);
			if origin != "u" || unique != 1 {
				continue;
			}
			let idx_name: String = row.get("name").unwrap_or_default();
			// nosemgrep: rust.actix.sql.sqlx-taint.sqlx-taint
			let info_sql = format!(
				"PRAGMA index_info({})",
				super::sqlite_pragma::quote_pragma_identifier(&idx_name)
			);
			let info_rows = editor.fetch_all(&info_sql, vec![]).await?;
			let cols: Vec<String> = info_rows
				.iter()
				.filter_map(|r| r.get::<String>("name").ok())
				.collect();
			let declared_name = named_unique_constraints.get(&cols).cloned();
			if let Some(declared_name) = &declared_name
				&& declared_name != &idx_name
			{
				sqlite_constraint_aliases.insert(idx_name.clone(), declared_name.clone());
			}
			constraints.push(super::Constraint::Unique {
				name: declared_name.unwrap_or(idx_name),
				columns: cols,
			});
		}

		// 6. CHECK constraints parsed from CREATE TABLE SQL (SQLite has no
		//    PRAGMA for these).
		if let Some(ref sql) = create_sql {
			for (idx, check) in SQLiteIntrospector::parse_check_constraints(sql)?
				.into_iter()
				.enumerate()
			{
				constraints.push(super::Constraint::Check {
					name: check.name.unwrap_or_else(|| format!("check_{}", idx)),
					expression: check.expression,
				});
			}
		}

		Ok((columns, constraints, sqlite_constraint_aliases))
	}

	#[cfg(feature = "sqlite")]
	fn resolve_sqlite_constraint_name<'a>(
		aliases: &'a HashMap<String, String>,
		requested_name: &'a str,
	) -> &'a str {
		aliases
			.get(requested_name)
			.map(String::as_str)
			.unwrap_or(requested_name)
	}

	/// Handle SQLite table recreation for operations that require it
	///
	/// SQLite has limited ALTER TABLE support. Operations like DropColumn and AlterColumn
	/// require table recreation (CREATE new table → COPY data → DROP old → RENAME).
	///
	/// This method handles foreign key constraints by:
	/// 1. Disabling FK checks before recreation
	/// 2. Executing the table recreation
	/// 3. Re-enabling FK checks
	/// 4. Checking for FK integrity violations
	#[cfg(feature = "sqlite")]
	async fn handle_sqlite_recreation(
		&self,
		operation: &Operation,
		editor: &mut SchemaEditor,
	) -> Result<()> {
		use super::operations::SqliteTableRecreation;

		// Disable foreign key checks before table recreation
		// This prevents FK violations during the temporary DROP TABLE phase
		editor.disable_foreign_keys().await?;

		// Build the recreation plan based on operation type.
		//
		// Critical: introspection must run via the editor's open transaction so
		// that DDL applied earlier in the same migration (e.g. a preceding
		// `AddColumn`) is visible. Reading via the pool would land on a
		// different connection that cannot see the uncommitted schema and
		// would silently rebuild the table from a stale column set — the
		// root cause of reinhardt-web#4447.
		let recreation = match operation {
			Operation::DropColumn { table, column } => {
				tracing::debug!(
					"Handling SQLite table recreation for DropColumn: table={}, column={}",
					table,
					column
				);
				let (columns, constraints, _) =
					Self::read_sqlite_table_via_editor(editor, table).await?;
				SqliteTableRecreation::for_drop_column(table, columns, column, constraints)
			}
			Operation::AlterColumn {
				table,
				column,
				new_definition,
				..
			} => {
				tracing::debug!(
					"Handling SQLite table recreation for AlterColumn: table={}, column={}",
					table,
					column
				);
				let (columns, constraints, _) =
					Self::read_sqlite_table_via_editor(editor, table).await?;
				SqliteTableRecreation::for_alter_column(
					table,
					columns,
					column,
					new_definition.clone(),
					constraints,
				)
			}
			Operation::AddConstraint {
				table,
				constraint_sql,
			} => {
				tracing::debug!(
					"Handling SQLite table recreation for AddConstraint: table={}",
					table
				);
				let (columns, constraints, _) =
					Self::read_sqlite_table_via_editor(editor, table).await?;
				SqliteTableRecreation::for_add_constraint(
					table,
					columns,
					constraints,
					constraint_sql.clone(),
				)
			}
			Operation::DropConstraint {
				table,
				constraint_name,
			} => {
				tracing::debug!(
					"Handling SQLite table recreation for DropConstraint: table={}, constraint={}",
					table,
					constraint_name
				);
				let (columns, constraints, aliases) =
					Self::read_sqlite_table_via_editor(editor, table).await?;
				let resolved_name = Self::resolve_sqlite_constraint_name(&aliases, constraint_name);
				SqliteTableRecreation::for_drop_constraint(
					table,
					columns,
					constraints,
					resolved_name,
				)
			}
			_ => {
				// This branch should not be reached if requires_sqlite_recreation() is correct
				tracing::warn!(
					"Operation {:?} was passed to handle_sqlite_recreation but is not handled. \
					Attempting to execute as-is, which may fail.",
					std::mem::discriminant(operation)
				);
				// Re-enable FK checks and fall back to normal SQL execution
				editor.enable_foreign_keys().await?;
				let sql = operation.to_sql(&super::operations::SqlDialect::Sqlite);
				editor.execute(&sql).await?;
				return Ok(());
			}
		};

		// Execute recreation steps
		for stmt in recreation.to_sql_statements() {
			tracing::debug!("Executing recreation SQL: {}", &stmt[..stmt.len().min(100)]);
			editor.execute(&stmt).await?;
		}

		// Re-enable foreign key checks
		editor.enable_foreign_keys().await?;

		// Check for FK integrity violations (logs warning if any found)
		let violations = editor.check_foreign_key_integrity().await?;
		if !violations.is_empty() {
			return Err(MigrationError::ForeignKeyViolation(format!(
				"Foreign key violations detected after table recreation: {}",
				violations.join("; ")
			)));
		}

		tracing::debug!(
			"SQLite table recreation completed for {:?}",
			std::mem::discriminant(operation)
		);

		Ok(())
	}

	/// Record a migration as applied without actually running it
	pub async fn record_migration(&mut self, app_label: &str, migration_name: &str) -> Result<()> {
		self.recorder
			.record_applied(app_label, migration_name)
			.await?;
		Ok(())
	}

	/// Execute a migration by loading it from the service
	// Allow dead_code: public API for programmatic single-migration execution
	#[allow(dead_code)]
	pub async fn execute_migration(
		&mut self,
		app_label: &str,
		migration_name: &str,
		service: &MigrationService,
	) -> Result<()> {
		let migration = service.load_migration(app_label, migration_name).await?;

		#[cfg(feature = "postgres")]
		if self.connection.is_cockroachdb() {
			return self
				.execute_migration_with_cockroachdb_schema_lock(&migration)
				.await;
		}

		self.recorder.ensure_schema_table().await?;
		self.execute_migration_after_schema_table(&migration).await
	}

	#[cfg(feature = "postgres")]
	async fn execute_migration_with_cockroachdb_schema_lock(
		&mut self,
		migration: &Migration,
	) -> Result<()> {
		let _lock = self.recorder.acquire_cockroachdb_schema_lock().await?;
		self.recorder.ensure_schema_table_internal().await?;
		self.execute_migration_after_schema_table(migration).await
	}

	async fn execute_migration_after_schema_table(&mut self, migration: &Migration) -> Result<()> {
		// Apply operations
		self.apply_migration(migration).await?;

		// Record as applied
		self.recorder
			.record_applied(&migration.app_label, &migration.name)
			.await?;

		Ok(())
	}
}

/// Operation optimizer for migration execution
///
/// Reorders and optimizes operations for better performance and safety.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::executor::OperationOptimizer;
/// use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
///
/// let ops = vec![
///     Operation::AddColumn {
///         table: "users".to_string(),
///         column: ColumnDefinition::new("name", FieldType::VarChar(100)),
///         mysql_options: None,
///     },
///     Operation::CreateTable {
///         name: "users".to_string(),
///         columns: vec![],
///         constraints: vec![],
///         without_rowid: None,
///         interleave_in_parent: None,
///         partition: None,
///     },
/// ];
///
/// let optimizer = OperationOptimizer::new();
/// let optimized = optimizer.optimize(ops);
/// // CreateTable should come before AddColumn
/// ```
pub struct OperationOptimizer {
	_private: (),
}

impl OperationOptimizer {
	/// Create a new operation optimizer
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::executor::OperationOptimizer;
	///
	/// let optimizer = OperationOptimizer::new();
	/// ```
	pub fn new() -> Self {
		Self { _private: () }
	}

	/// Optimize and reorder operations
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::executor::OperationOptimizer;
	/// use reinhardt_db::migrations::{Operation, ColumnDefinition};
	///
	/// let ops = vec![
	///     Operation::CreateTable {
	///         name: "users".to_string(),
	///         columns: vec![],
	///         constraints: vec![],
	///         without_rowid: None,
	///         interleave_in_parent: None,
	///         partition: None,
	///     },
	/// ];
	///
	/// let optimizer = OperationOptimizer::new();
	/// let optimized = optimizer.optimize(ops);
	/// assert_eq!(optimized.len(), 1);
	/// ```
	pub fn optimize(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut optimized = operations;

		// Step 1: Reorder operations by dependency
		optimized = self.reorder_by_dependency(optimized);

		// Step 2: Group similar operations
		optimized = self.group_similar_operations(optimized);

		// Step 3: Remove redundant operations
		optimized = self.remove_redundant_operations(optimized);

		optimized
	}

	/// Reorder operations to respect dependencies
	fn reorder_by_dependency(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut ordered = Vec::new();
		let mut remaining = operations;
		let mut created_tables = HashSet::new();

		// Priority order:
		// 1. CreateTable (sorted by foreign key dependencies)
		// 2. AddColumn
		// 3. AlterColumn
		// 4. CreateIndex
		// 5. AddConstraint
		// 6. RunSQL
		// 7. RenameColumn
		// 8. DropColumn
		// 9. DropTable

		// First pass: Create tables (respecting foreign key dependencies)
		// Extract all CreateTable operations
		let mut create_table_ops = Vec::new();
		let mut i = 0;
		while i < remaining.len() {
			if matches!(&remaining[i], Operation::CreateTable { .. }) {
				create_table_ops.push(remaining.remove(i));
			} else {
				i += 1;
			}
		}

		// Sort CreateTable operations by dependencies using topological sort
		while !create_table_ops.is_empty() {
			let mut found_independent = false;

			for i in 0..create_table_ops.len() {
				if let Operation::CreateTable {
					name, constraints, ..
				} = &create_table_ops[i]
				{
					// Extract foreign key references from constraints
					let mut depends_on_uncreated = false;
					for constraint in constraints {
						if let Some(referenced_table) =
							self.extract_foreign_key_reference(constraint)
						{
							// Check if the referenced table has been created
							if !created_tables.contains(&referenced_table)
								&& referenced_table != *name
							{
								depends_on_uncreated = true;
								break;
							}
						}
					}

					// If this table doesn't depend on any uncreated table, we can create it now
					if !depends_on_uncreated {
						// Clone the name before removing the operation
						let name_copy = name.clone();
						let op = create_table_ops.remove(i);
						created_tables.insert(name_copy);
						ordered.push(op);
						found_independent = true;
						break;
					}
				}
			}

			// If we couldn't find any independent table, just add the remaining tables
			// (this handles circular dependencies or malformed constraints)
			if !found_independent {
				for op in create_table_ops.drain(..) {
					if let Operation::CreateTable { ref name, .. } = op {
						created_tables.insert(name.clone());
					}
					ordered.push(op);
				}
				break;
			}
		}

		// Second pass: Add columns (for all tables)
		i = 0;
		while i < remaining.len() {
			if let Operation::AddColumn { .. } = &remaining[i] {
				ordered.push(remaining.remove(i));
			} else {
				i += 1;
			}
		}

		// Third pass: Other operations
		ordered.extend(remaining);

		ordered
	}

	/// Extract the referenced table name from a FOREIGN KEY constraint
	/// Returns the referenced table name if the constraint is a ForeignKey
	fn extract_foreign_key_reference(&self, constraint: &super::Constraint) -> Option<String> {
		match constraint {
			super::Constraint::ForeignKey {
				referenced_table, ..
			} => Some(referenced_table.clone()),
			_ => None,
		}
	}

	/// Extract constraint name from SQL definition
	fn extract_constraint_name(&self, constraint_sql: &str) -> Option<String> {
		let trimmed = constraint_sql.trim();

		// Check if starts with "CONSTRAINT"
		if !trimmed.starts_with("CONSTRAINT") {
			return None;
		}

		// Skip "CONSTRAINT" and whitespace
		let after_keyword = trimmed["CONSTRAINT".len()..].trim_start();

		// Extract identifier (alphanumeric + underscore)
		let name: String = after_keyword
			.chars()
			.take_while(|c| c.is_alphanumeric() || *c == '_')
			.collect();

		if name.is_empty() { None } else { Some(name) }
	}

	/// Group similar operations together
	fn group_similar_operations(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut by_table: IndexMap<String, Vec<Operation>> = IndexMap::new();
		let mut create_ops = Vec::new();
		let mut other_ops = Vec::new();

		for op in operations {
			match &op {
				Operation::CreateTable { .. } => {
					// CreateTable operations go first
					create_ops.push(op);
				}
				Operation::AddColumn { table, .. }
				| Operation::DropColumn { table, .. }
				| Operation::AlterColumn { table, .. } => {
					by_table.entry(table.to_string()).or_default().push(op);
				}
				_ => {
					other_ops.push(op);
				}
			}
		}

		let mut grouped = Vec::new();

		// Add create table operations first
		grouped.extend(create_ops);

		// Add table-specific operations grouped by table
		for (_, ops) in by_table {
			grouped.extend(ops);
		}

		// Add other operations
		grouped.extend(other_ops);

		grouped
	}

	/// Remove redundant operations by detecting cancellations and merging similar operations
	fn remove_redundant_operations(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut optimized = Vec::new();
		let mut removed_indices = HashSet::new();

		// Pass 1: Detect and remove operation cancellations
		for i in 0..operations.len() {
			if removed_indices.contains(&i) {
				continue;
			}

			let op = &operations[i];
			let mut found_cancellation = false;

			// Search forward for cancelling operations
			for (j, next_op) in operations.iter().enumerate().skip(i + 1) {
				if removed_indices.contains(&j) {
					continue;
				}

				// Check for cancellation patterns
				let cancels = match (op, next_op) {
					// CreateTable + DropTable
					(
						Operation::CreateTable { name: n1, .. },
						Operation::DropTable { name: n2 },
					) if n1 == n2 => true,
					// AddColumn + DropColumn
					(
						Operation::AddColumn {
							table: t1,
							column: col1,
							..
						},
						Operation::DropColumn {
							table: t2,
							column: col2,
						},
					) if t1 == t2 && col1.name == *col2 => true,
					// CreateIndex + DropIndex
					(
						Operation::CreateIndex {
							table: t1,
							columns: c1,
							..
						},
						Operation::DropIndex {
							table: t2,
							columns: c2,
						},
					) if t1 == t2 && c1 == c2 => true,
					// AddConstraint + DropConstraint
					(
						Operation::AddConstraint {
							table: t1,
							constraint_sql,
						},
						Operation::DropConstraint {
							table: t2,
							constraint_name,
						},
					) if t1 == t2 => {
						// Try to extract constraint name from SQL for exact matching
						if let Some(extracted_name) = self.extract_constraint_name(constraint_sql) {
							// Perfect match: compare extracted name with drop target
							extracted_name == *constraint_name
						} else {
							// Fallback: approximate match by table only
							true
						}
					}
					_ => false,
				};

				if cancels {
					removed_indices.insert(i);
					removed_indices.insert(j);
					found_cancellation = true;
					break;
				}
			}

			if !found_cancellation {
				optimized.push(op.clone());
			}
		}

		// Pass 1.5: Remove duplicate CreateTable operations (keep last occurrence)
		let mut deduped = Vec::new();
		let mut create_table_map: IndexMap<String, Operation> = IndexMap::new();

		for operation in optimized {
			match &operation {
				Operation::CreateTable { name, .. } => {
					// Last CreateTable for same table wins
					create_table_map.insert(name.to_string(), operation.clone());
				}
				_ => {
					// Flush accumulated CreateTable operations before non-CreateTable operation
					for (_, create_op) in create_table_map.drain(..) {
						deduped.push(create_op);
					}
					deduped.push(operation);
				}
			}
		}

		// Flush remaining CreateTable operations
		for (_, create_op) in create_table_map {
			deduped.push(create_op);
		}

		// Pass 2: Merge consecutive AlterColumn operations on same column
		let mut merged = Vec::new();
		let mut alter_column_map: IndexMap<(String, String), Operation> = IndexMap::new();

		for operation in deduped {
			match &operation {
				Operation::AlterColumn {
					table,
					column,
					new_definition: _,
					..
				} => {
					let key = (table.to_string(), column.to_string());
					// Last AlterColumn wins (overwrites previous)
					alter_column_map.insert(key, operation.clone());
				}
				_ => {
					// Flush accumulated AlterColumn operations before non-AlterColumn operation
					for (_, alter_op) in alter_column_map.drain(..) {
						merged.push(alter_op);
					}
					merged.push(operation);
				}
			}
		}

		// Flush remaining AlterColumn operations
		for (_, alter_op) in alter_column_map {
			merged.push(alter_op);
		}

		// Pass 3: Chain consecutive RenameTable operations
		let mut chained = Vec::new();
		let mut rename_chain: IndexMap<String, String> = IndexMap::new(); // original_name -> current_name

		for operation in merged {
			match &operation {
				Operation::RenameTable { old_name, new_name } => {
					// Find if any existing chain ends with this old_name
					let mut found_chain = None;
					for (original, current) in &rename_chain {
						if current == old_name {
							found_chain = Some(original.clone());
							break;
						}
					}

					if let Some(original) = found_chain {
						// Extend existing chain: original -> new_name
						rename_chain.insert(original, new_name.clone());
					} else {
						// Start new chain: old_name -> new_name
						rename_chain.insert(old_name.clone(), new_name.clone());
					}
				}
				_ => {
					// Flush accumulated RenameTable chains before non-RenameTable operation
					for (original_name, final_name) in rename_chain.drain(..) {
						chained.push(Operation::RenameTable {
							old_name: original_name,
							new_name: final_name,
						});
					}
					chained.push(operation);
				}
			}
		}

		// Flush remaining RenameTable chains
		for (original_name, final_name) in rename_chain {
			chained.push(Operation::RenameTable {
				old_name: original_name,
				new_name: final_name,
			});
		}

		chained
	}
}

impl Default for OperationOptimizer {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod optimizer_tests {
	use super::*;
	use crate::migrations::{ColumnDefinition, FieldType};

	#[test]
	fn test_optimizer_creation() {
		let optimizer = OperationOptimizer::new();
		let ops = vec![];
		let optimized = optimizer.optimize(ops);
		assert_eq!(optimized.len(), 0);
	}

	#[test]
	fn test_reorder_create_before_add() {
		let optimizer = OperationOptimizer::new();

		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("name", FieldType::VarChar(100)),
				mysql_options: None,
			},
			Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
		];

		let optimized = optimizer.optimize(ops);

		// CreateTable should come before AddColumn
		assert!(matches!(optimized[0], Operation::CreateTable { .. }));
		assert!(matches!(optimized[1], Operation::AddColumn { .. }));
	}

	#[test]
	fn test_remove_duplicate_create_table() {
		let optimizer = OperationOptimizer::new();

		let ops = vec![
			Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
			Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
		];

		let optimized = optimizer.optimize(ops);
		assert_eq!(optimized.len(), 1);
	}

	#[test]
	fn test_group_operations_by_table() {
		let optimizer = OperationOptimizer::new();

		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("name", FieldType::VarChar(100)),
				mysql_options: None,
			},
			Operation::CreateTable {
				name: "posts".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("email", FieldType::VarChar(255)),
				mysql_options: None,
			},
		];

		let optimized = optimizer.optimize(ops);
		assert_eq!(optimized.len(), 3);
	}

	#[cfg(feature = "sqlite")]
	#[test]
	fn sqlite_autoindex_alias_resolves_to_declared_constraint_name() {
		// Arrange
		let aliases = std::collections::HashMap::from([(
			"sqlite_autoindex_users_1".to_string(),
			"users_email_uniq".to_string(),
		)]);

		// Act
		let resolved_alias = DatabaseMigrationExecutor::resolve_sqlite_constraint_name(
			&aliases,
			"sqlite_autoindex_users_1",
		);
		let resolved_declared =
			DatabaseMigrationExecutor::resolve_sqlite_constraint_name(&aliases, "users_email_uniq");

		// Assert
		assert_eq!(resolved_alias, "users_email_uniq");
		assert_eq!(resolved_declared, "users_email_uniq");
	}

	#[cfg(test)]
	mod split_sql_tests {
		use crate::migrations::executor::split_sql_statements;

		#[test]
		fn test_split_simple_statements() {
			let sql = "CREATE TABLE t1 (id INT); CREATE TABLE t2 (id INT);";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
			assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
		}

		#[test]
		fn test_split_with_string_literals() {
			let sql = r#"INSERT INTO t VALUES ('a;b'); INSERT INTO t VALUES ('c;d');"#;
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "INSERT INTO t VALUES ('a;b')");
			assert_eq!(result[1], "INSERT INTO t VALUES ('c;d')");
		}

		#[test]
		fn test_split_with_line_comments() {
			// Line comment after semicolon becomes part of next statement
			let sql =
				"CREATE TABLE t1 (id INT); -- comment; with semicolon\nCREATE TABLE t2 (id INT);";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
			assert!(result[1].contains("-- comment"));
			assert!(result[1].contains("CREATE TABLE t2"));
		}

		#[test]
		fn test_split_with_block_comments() {
			let sql =
				"CREATE TABLE t1 (id INT); /* comment; with semicolon */ CREATE TABLE t2 (id INT);";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert!(result[0].contains("CREATE TABLE t1"));
			assert!(result[1].contains("CREATE TABLE t2"));
		}

		#[test]
		fn test_split_with_dollar_quotes() {
			let sql = r#"CREATE FUNCTION f() RETURNS text AS $$SELECT 'value; with semicolon';$$ LANGUAGE sql; CREATE TABLE t1 (id INT);"#;
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert!(result[0].contains("CREATE FUNCTION"));
			assert!(result[0].contains("value; with semicolon"));
			assert!(result[1].contains("CREATE TABLE t1"));
		}

		#[test]
		fn test_split_with_escaped_quotes() {
			let sql = r#"INSERT INTO t VALUES ('it''s a test; value'); INSERT INTO t VALUES ('another');"#;
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert!(result[0].contains("it''s a test; value"));
			assert!(result[1].contains("another"));
		}

		#[test]
		fn test_split_empty_statements() {
			let sql = ";;; CREATE TABLE t1 (id INT); ;";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 1);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
		}

		#[test]
		fn test_split_no_semicolon() {
			let sql = "CREATE TABLE t1 (id INT)";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 1);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
		}

		#[test]
		fn test_split_whitespace_handling() {
			let sql = "  CREATE TABLE t1 (id INT)  ;  \n\n  CREATE TABLE t2 (id INT)  ;  ";
			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);
			assert_eq!(result[0], "CREATE TABLE t1 (id INT)");
			assert_eq!(result[1], "CREATE TABLE t2 (id INT)");
		}

		#[test]
		fn test_split_reinhardt_query_migration_sql() {
			// Actual SQL generated by reinhardt-query for polls migration (from diagnostic test)
			let sql = r###"CREATE TABLE "questions_table" ( "id" bigint GENERATED BY DEFAULT AS IDENTITY NOT NULL PRIMARY KEY, "question_text" text NOT NULL, "pub_date" timestamp with time zone NOT NULL );

CREATE TABLE "choices_table" ( "id" bigint GENERATED BY DEFAULT AS IDENTITY NOT NULL PRIMARY KEY, "question_id" bigint NOT NULL, "choice_text" text NOT NULL, "votes" integer NOT NULL DEFAULT 0, FOREIGN KEY ("question_id") REFERENCES "questions_table" ("id") ON DELETE CASCADE );"###;
			let result = split_sql_statements(sql);

			// Should split into exactly 2 statements (not 3 with empty string)
			assert_eq!(
				result.len(),
				2,
				"Expected 2 statements, got {}",
				result.len()
			);

			// First statement should be CREATE TABLE questions_table
			assert!(
				result[0].contains("questions_table"),
				"First statement should contain 'questions_table'"
			);
			assert!(
				result[0].contains("question_text"),
				"First statement should contain 'question_text'"
			);
			assert!(
				!result[0].contains("choices_table"),
				"First statement should not contain 'choices_table'"
			);

			// Second statement should be CREATE TABLE choices_table
			assert!(
				result[1].contains("choices_table"),
				"Second statement should contain 'choices_table'"
			);
			assert!(
				result[1].contains("choice_text"),
				"Second statement should contain 'choice_text'"
			);
			// Verify reference to questions_table, as FOREIGN KEY constraint contains referenced table name
			assert!(
				result[1].contains("FOREIGN KEY"),
				"Second statement should contain FOREIGN KEY constraint"
			);
			assert!(
				result[1].contains("REFERENCES \"questions_table\""),
				"FOREIGN KEY should reference questions_table"
			);
		}

		#[test]
		fn test_split_multiple_foreign_keys() {
			// Case where table has multiple FOREIGN KEY constraints
			let sql = r###"CREATE TABLE "posts" ("id" bigint PRIMARY KEY);
CREATE TABLE "users" ("id" bigint PRIMARY KEY);
CREATE TABLE "comments" (
	"id" bigint PRIMARY KEY,
	"post_id" bigint,
	"user_id" bigint,
	FOREIGN KEY ("post_id") REFERENCES "posts" ("id"),
	FOREIGN KEY ("user_id") REFERENCES "users" ("id")
);"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 3, "Expected 3 statements");

			// Third statement contains 2 FOREIGN KEY constraints
			assert_eq!(
				result[2].matches("FOREIGN KEY").count(),
				2,
				"Third statement should contain 2 FOREIGN KEY constraints"
			);
			assert!(
				result[2].contains("REFERENCES \"posts\""),
				"Should reference posts table"
			);
			assert!(
				result[2].contains("REFERENCES \"users\""),
				"Should reference users table"
			);
		}

		#[test]
		fn test_split_mixed_constraints() {
			// Case with mixed CHECK constraint and FOREIGN KEY
			let sql = r###"CREATE TABLE "tasks" ("id" bigint PRIMARY KEY);
CREATE TABLE "task_status" (
	"id" bigint PRIMARY KEY,
	"task_id" bigint,
	"status" text CHECK (status IN ('pending', 'completed')),
	FOREIGN KEY ("task_id") REFERENCES "tasks" ("id")
);"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 2);

			// Second statement contains both CHECK constraint and FOREIGN KEY constraint
			assert!(
				result[1].contains("CHECK"),
				"Second statement should contain CHECK constraint"
			);
			assert!(
				result[1].contains("FOREIGN KEY"),
				"Second statement should contain FOREIGN KEY constraint"
			);
		}

		#[test]
		fn test_split_self_referencing_foreign_key() {
			// Case with self-referencing FOREIGN KEY
			let sql = r###"CREATE TABLE "categories" (
	"id" bigint PRIMARY KEY,
	"parent_id" bigint,
	FOREIGN KEY ("parent_id") REFERENCES "categories" ("id")
);"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 1);

			// FOREIGN KEY referencing the same table
			assert!(
				result[0].contains("REFERENCES \"categories\""),
				"Should self-reference categories table"
			);
		}

		#[test]
		fn test_split_create_index_statements() {
			// Splitting CREATE INDEX statements
			let sql = r###"CREATE TABLE "products" ("id" bigint PRIMARY KEY, "name" text);
CREATE INDEX "idx_products_name" ON "products" ("name");
CREATE UNIQUE INDEX "idx_products_id" ON "products" ("id");"###;

			let result = split_sql_statements(sql);
			assert_eq!(result.len(), 3);

			assert!(
				result[0].contains("CREATE TABLE"),
				"First statement should be CREATE TABLE"
			);
			assert!(
				result[1].contains("CREATE INDEX"),
				"Second statement should be CREATE INDEX"
			);
			assert!(
				result[2].contains("CREATE UNIQUE INDEX"),
				"Third statement should be CREATE UNIQUE INDEX"
			);
		}
	}
}

#[cfg(all(test, feature = "sqlite"))]
mod rollback_orchestration_tests {
	//! In-crate tests for [`DatabaseMigrationExecutor::rollback_migrations`] and
	//! the private [`DatabaseMigrationExecutor::rollback_migration`].
	//!
	//! These exercise the orchestration layer — reverse iteration of the input
	//! slice, recorder synchronisation, the `state_only` short-circuit, and the
	//! warn-and-continue behaviour for `RunSQL` operations that lack a
	//! `reverse_sql`. Per-`Operation` SQL generation is already covered by the
	//! `to_reverse_sql` tests in `operations.rs`; this module focuses on what
	//! sits above those primitives in `executor.rs`.
	//!
	//! A real SQLite `:memory:` connection is used rather than test doubles
	//! because [`DatabaseConnection`], [`DatabaseMigrationRecorder`], and
	//! [`SchemaEditor`] are concrete types with no trait abstraction today.
	//! `:memory:` is the lightest available substitute — sub-second per case,
	//! no external services, and gated behind the `sqlite` feature that the
	//! rollback path itself already references.
	//!
	//! Not covered here (require failure injection, which needs a trait
	//! refactor to land first): non-atomic partial-rollback bookkeeping when
	//! an operation fails mid-run, atomic-mode rollback-on-failure, and
	//! foreign-key violation behaviour during rollback. Those paths remain
	//! exercised by the container-backed suite in
	//! `tests/integration/tests/migrations/migration_rollback_integration.rs`.

	use super::*;
	use crate::backends::DatabaseConnection;
	use crate::migrations::recorder::DatabaseMigrationRecorder;
	use crate::migrations::{ColumnDefinition, FieldType, Migration};
	use rstest::*;

	/// Open a fresh SQLite `:memory:` database and wrap it in a
	/// [`DatabaseMigrationExecutor`]. Each call returns an isolated database.
	async fn make_executor() -> DatabaseMigrationExecutor {
		let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("failed to open sqlite :memory: connection");
		DatabaseMigrationExecutor::new(connection)
	}

	/// Build a single-operation `CreateTable` migration with one integer
	/// primary key column.
	fn make_create_table_migration(name: &str, table: &str) -> Migration {
		let mut migration = Migration::new(name, "rolltest");
		migration.operations.push(Operation::CreateTable {
			name: table.to_string(),
			columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
		migration
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_with_empty_input_returns_empty_result() {
		// Arrange
		let mut executor = make_executor().await;

		// Act
		let result = executor
			.rollback_migrations(&[])
			.await
			.expect("rollback of empty input should not fail");

		// Assert
		assert!(
			result.applied.is_empty(),
			"no migrations should be reported as rolled back"
		);
		assert!(result.failed.is_none(), "no failure should be reported");
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_iterates_input_slice_in_reverse_order() {
		// Arrange - apply three independent migrations.
		let m1 = make_create_table_migration("0001_a", "rolltest_a");
		let m2 = make_create_table_migration("0002_b", "rolltest_b");
		let m3 = make_create_table_migration("0003_c", "rolltest_c");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(&[m1.clone(), m2.clone(), m3.clone()])
			.await
			.expect("apply m1..m3");

		// Act - pass the slice in declaration order; rollback must consume in reverse.
		let result = executor
			.rollback_migrations(&[m1.clone(), m2.clone(), m3.clone()])
			.await
			.expect("rollback m1..m3");

		// Assert - newest-first order, independent of how apply_migrations
		// ordered them via the topological sort.
		assert_eq!(
			result.applied,
			vec![m3.id(), m2.id(), m1.id()],
			"rollback_migrations must iterate input in reverse"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_skips_migrations_that_were_never_applied() {
		// Arrange - apply only m1, leave m2 unrecorded.
		let m1 = make_create_table_migration("0001_first", "rolltest_first");
		let m2 = make_create_table_migration("0002_second", "rolltest_second");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&m1))
			.await
			.expect("apply m1");

		// Act - ask to roll both back.
		let result = executor
			.rollback_migrations(&[m1.clone(), m2.clone()])
			.await
			.expect("rollback should succeed even with an unapplied entry");

		// Assert - only the actually-applied migration is reported as rolled back.
		assert_eq!(
			result.applied,
			vec![m1.id()],
			"unapplied migrations must be silently skipped, not rolled back"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_clears_recorder_state_for_rolled_back_migration() {
		// Arrange
		let migration = make_create_table_migration("0001_only", "rolltest_only");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply migration");

		let recorder_before = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert!(
			recorder_before
				.is_applied(&migration.app_label, &migration.name)
				.await
				.expect("query recorder before"),
			"sanity: migration should be applied before rollback"
		);

		// Act
		executor
			.rollback_migrations(std::slice::from_ref(&migration))
			.await
			.expect("rollback");

		// Assert - recorder no longer marks the migration as applied.
		let recorder_after = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert!(
			!recorder_after
				.is_applied(&migration.app_label, &migration.name)
				.await
				.expect("query recorder after"),
			"recorder must report unapplied after successful rollback"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_with_state_only_flag_skips_schema_changes() {
		// Arrange - apply a normal migration so the table really exists.
		let mut migration = make_create_table_migration("0001_state", "rolltest_state");
		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply migration to create the table");

		// Re-mark the migration as state_only for the rollback request,
		// which must hit the `state_only` short-circuit in
		// `rollback_migration` and skip schema operations entirely.
		migration.state_only = true;

		// Act
		let result = executor
			.rollback_migrations(std::slice::from_ref(&migration))
			.await
			.expect("state_only rollback should succeed without DB ops");

		// Assert - rollback is recorded, but the table is intentionally still
		// present because no DROP TABLE was issued.
		assert_eq!(result.applied, vec![migration.id()]);

		let table_still_present = executor
			.connection()
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec!["rolltest_state".into()],
			)
			.await
			.expect("introspect sqlite_master")
			.is_some();
		assert!(
			table_still_present,
			"state_only rollback must not execute schema operations"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn rollback_run_sql_without_reverse_sql_completes_without_error() {
		// Pins the current contract of the `Operation::RunSQL` arm in
		// `rollback_migration` when `reverse_sql` is `None`: the rollback
		// logs a warning and continues. It does NOT return an error. If a
		// future change promotes that to a typed error, this test will fail
		// and force the change to be explicit rather than silent.
		let mut migration = Migration::new("0001_run_sql_noop", "rolltest");
		migration.operations.push(Operation::RunSQL {
			sql: "CREATE TABLE rolltest_runsql (id INTEGER PRIMARY KEY)".to_string(),
			reverse_sql: None,
		});

		let mut executor = make_executor().await;
		executor
			.apply_migrations(std::slice::from_ref(&migration))
			.await
			.expect("apply RunSQL migration");

		// Act
		let result = executor
			.rollback_migrations(std::slice::from_ref(&migration))
			.await
			.expect("rollback should succeed even when reverse_sql is missing");

		// Assert - the rollback is reported in `result.applied` and the
		// recorder is cleared even though no reverse SQL ran (the
		// warn-and-skip contract still updates bookkeeping).
		assert_eq!(result.applied, vec![migration.id()]);

		let recorder = DatabaseMigrationRecorder::new(executor.connection().clone());
		assert!(
			!recorder
				.is_applied(&migration.app_label, &migration.name)
				.await
				.expect("query recorder after"),
			"recorder must reflect unapplied state after warn-and-skip rollback"
		);
	}
}
