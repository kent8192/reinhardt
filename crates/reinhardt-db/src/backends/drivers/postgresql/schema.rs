/// PostgreSQL-specific schema editor
///
/// This module provides PostgreSQL-specific DDL operations, including:
/// - CONCURRENTLY index operations
/// - IDENTITY column support
/// - Sequence operations
/// - LIKE index auto-creation for varchar/text columns
///
/// # Example
///
/// ```no_run
/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
/// use reinhardt_db::backends::schema::BaseDatabaseSchemaEditor;
/// use sqlx::PgPool;
///
/// # async fn example() -> Result<(), sqlx::Error> {
/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
/// let editor = PostgreSQLSchemaEditor::new(pool);
/// let sql = editor.create_index_concurrently_sql("idx_email", "users", &["email"], false, None);
/// assert!(sql.contains("CONCURRENTLY"));
/// # Ok(())
/// # }
/// ```
use crate::backends::schema::{BaseDatabaseSchemaEditor, SchemaEditorError, SchemaEditorResult};
use sqlx::PgPool;
use std::sync::Arc;

/// Quote a PostgreSQL identifier by wrapping it in double quotes
fn quote_identifier(name: &str) -> String {
	format!("\"{}\"", name)
}

/// PostgreSQL-specific schema editor
pub struct PostgreSQLSchemaEditor {
	/// PostgreSQL connection pool
	pool: Arc<PgPool>,
}

impl PostgreSQLSchemaEditor {
	/// Create a new PostgreSQL schema editor from a connection pool
	///
	/// # Example
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(pool: PgPool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	/// Create from an `Arc<PgPool>`
	///
	/// # Example
	///
	/// ```no_run
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = Arc::new(PgPool::connect("postgresql://localhost/mydb").await?);
	/// let editor = PostgreSQLSchemaEditor::from_pool_arc(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn from_pool_arc(pool: Arc<PgPool>) -> Self {
		Self { pool }
	}

	/// Generate CREATE INDEX CONCURRENTLY SQL
	///
	/// This allows creating indexes without blocking writes to the table.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").expect("Failed to create lazy pool");
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// let sql = editor.create_index_concurrently_sql(
	///     "idx_email",
	///     "users",
	///     &["email"],
	///     false,
	///     None
	/// );
	/// assert_eq!(sql, "CREATE INDEX CONCURRENTLY \"idx_email\" ON \"users\" (\"email\")");
	/// ```
	pub fn create_index_concurrently_sql(
		&self,
		name: &str,
		table: &str,
		columns: &[&str],
		unique: bool,
		condition: Option<&str>,
	) -> String {
		let unique_keyword = if unique { "UNIQUE " } else { "" };
		let quoted_columns: Vec<String> = columns
			.iter()
			.map(|c| quote_identifier(c).to_string())
			.collect();

		let mut sql = format!(
			"CREATE {}INDEX CONCURRENTLY {} ON {} ({})",
			unique_keyword,
			quote_identifier(name),
			quote_identifier(table),
			quoted_columns.join(", ")
		);

		if let Some(cond) = condition {
			sql.push_str(&format!(" WHERE {}", cond));
		}

		sql
	}

	/// Generate DROP INDEX CONCURRENTLY SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").expect("Failed to create lazy pool");
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// let sql = editor.drop_index_concurrently_sql("idx_email");
	/// assert_eq!(sql, "DROP INDEX CONCURRENTLY IF EXISTS \"idx_email\"");
	/// ```
	pub fn drop_index_concurrently_sql(&self, name: &str) -> String {
		format!(
			"DROP INDEX CONCURRENTLY IF EXISTS {}",
			quote_identifier(name)
		)
	}

	/// Generate ALTER SEQUENCE SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").expect("Failed to create lazy pool");
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// let sql = editor.alter_sequence_type_sql("users_id_seq", "BIGINT");
	/// assert_eq!(sql, "ALTER SEQUENCE IF EXISTS \"users_id_seq\" AS BIGINT");
	/// ```
	pub fn alter_sequence_type_sql(&self, sequence: &str, seq_type: &str) -> String {
		format!(
			"ALTER SEQUENCE IF EXISTS {} AS {}",
			quote_identifier(sequence),
			seq_type
		)
	}

	/// Generate DROP SEQUENCE SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").expect("Failed to create lazy pool");
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// let sql = editor.drop_sequence_sql("users_id_seq");
	/// assert_eq!(sql, "DROP SEQUENCE IF EXISTS \"users_id_seq\" CASCADE");
	/// ```
	pub fn drop_sequence_sql(&self, sequence: &str) -> String {
		format!(
			"DROP SEQUENCE IF EXISTS {} CASCADE",
			quote_identifier(sequence)
		)
	}

	/// Generate ADD IDENTITY SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").expect("Failed to create lazy pool");
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// let sql = editor.add_identity_sql("users", "id");
	/// assert_eq!(sql, "ALTER TABLE \"users\" ALTER COLUMN \"id\" ADD GENERATED BY DEFAULT AS IDENTITY");
	/// ```
	pub fn add_identity_sql(&self, table: &str, column: &str) -> String {
		format!(
			"ALTER TABLE {} ALTER COLUMN {} ADD GENERATED BY DEFAULT AS IDENTITY",
			quote_identifier(table),
			quote_identifier(column)
		)
	}

	/// Generate DROP IDENTITY SQL
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").expect("Failed to create lazy pool");
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// let sql = editor.drop_identity_sql("users", "id");
	/// assert_eq!(sql, "ALTER TABLE \"users\" ALTER COLUMN \"id\" DROP IDENTITY IF EXISTS");
	/// ```
	pub fn drop_identity_sql(&self, table: &str, column: &str) -> String {
		format!(
			"ALTER TABLE {} ALTER COLUMN {} DROP IDENTITY IF EXISTS",
			quote_identifier(table),
			quote_identifier(column)
		)
	}

	/// Generate LIKE index SQL for varchar/text pattern matching
	///
	/// PostgreSQL requires special indexes for LIKE queries outside the C locale.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// # use sqlx::PgPool;
	/// let pool = PgPool::connect_lazy("postgresql://localhost/test").expect("Failed to create lazy pool");
	/// let editor = PostgreSQLSchemaEditor::new(pool);
	/// let sql = editor.create_like_index_sql("users", "email", "varchar(255)");
	/// assert!(sql.is_some());
	/// assert!(sql.unwrap().contains("varchar_pattern_ops"));
	/// ```
	pub fn create_like_index_sql(
		&self,
		table: &str,
		column: &str,
		db_type: &str,
	) -> Option<String> {
		// Only create LIKE indexes for varchar and text types
		if db_type.starts_with("varchar") || db_type == "text" {
			// Skip array types
			if db_type.contains('[') {
				return None;
			}

			let pattern_ops = if db_type == "text" {
				"text_pattern_ops"
			} else {
				"varchar_pattern_ops"
			};

			let index_name = format!("{}_{}_like", table, column);

			Some(format!(
				"CREATE INDEX {} ON {} ({} {})",
				quote_identifier(&index_name),
				quote_identifier(table),
				quote_identifier(column),
				pattern_ops
			))
		} else {
			None
		}
	}
}

#[async_trait::async_trait]
impl BaseDatabaseSchemaEditor for PostgreSQLSchemaEditor {
	fn database_type(&self) -> crate::backends::types::DatabaseType {
		crate::backends::types::DatabaseType::Postgres
	}

	async fn execute(&mut self, sql: &str) -> SchemaEditorResult<()> {
		// Validate SQL input
		if sql.is_empty() {
			return Err(SchemaEditorError::InvalidOperation(
				"Cannot execute empty SQL".to_string(),
			));
		}

		// Execute SQL via sqlx connection pool
		sqlx::query(sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(|e| SchemaEditorError::ExecutionError(e.to_string()))?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	// Fixture to create a test pool for SQL generation tests
	// These tests don't actually execute SQL, they just test SQL generation
	#[fixture]
	async fn pg_pool() -> PgPool {
		// Create a dummy pool for testing SQL generation methods
		// The pool is never actually used in these tests
		PgPool::connect_lazy("postgresql://localhost/test_db").expect("Failed to create test pool")
	}

	// Helper function to create a test editor from the pool fixture
	fn create_test_editor(pool: PgPool) -> PostgreSQLSchemaEditor {
		PostgreSQLSchemaEditor::new(pool)
	}

	#[rstest]
	#[tokio::test]
	async fn test_create_index_concurrently(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql =
			editor.create_index_concurrently_sql("idx_email", "users", &["email"], false, None);

		assert_eq!(
			sql,
			"CREATE INDEX CONCURRENTLY \"idx_email\" ON \"users\" (\"email\")"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_create_unique_index_concurrently(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql =
			editor.create_index_concurrently_sql("idx_email", "users", &["email"], true, None);

		assert_eq!(
			sql,
			"CREATE UNIQUE INDEX CONCURRENTLY \"idx_email\" ON \"users\" (\"email\")"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_create_partial_index_concurrently(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql = editor.create_index_concurrently_sql(
			"idx_active_email",
			"users",
			&["email"],
			false,
			Some("active = true"),
		);

		assert_eq!(
			sql,
			"CREATE INDEX CONCURRENTLY \"idx_active_email\" ON \"users\" (\"email\") WHERE active = true"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_drop_index_concurrently(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql = editor.drop_index_concurrently_sql("idx_email");

		assert_eq!(sql, "DROP INDEX CONCURRENTLY IF EXISTS \"idx_email\"");
	}

	#[rstest]
	#[tokio::test]
	async fn test_alter_sequence_type(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql = editor.alter_sequence_type_sql("users_id_seq", "BIGINT");

		assert_eq!(sql, "ALTER SEQUENCE IF EXISTS \"users_id_seq\" AS BIGINT");
	}

	#[rstest]
	#[tokio::test]
	async fn test_drop_sequence(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql = editor.drop_sequence_sql("users_id_seq");

		assert_eq!(sql, "DROP SEQUENCE IF EXISTS \"users_id_seq\" CASCADE");
	}

	#[rstest]
	#[tokio::test]
	async fn test_add_identity(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql = editor.add_identity_sql("users", "id");

		assert_eq!(
			sql,
			"ALTER TABLE \"users\" ALTER COLUMN \"id\" ADD GENERATED BY DEFAULT AS IDENTITY"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_drop_identity(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);
		let sql = editor.drop_identity_sql("users", "id");

		assert_eq!(
			sql,
			"ALTER TABLE \"users\" ALTER COLUMN \"id\" DROP IDENTITY IF EXISTS"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_create_like_index(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let editor = create_test_editor(pool);

		// varchar should create index
		let varchar_sql = editor.create_like_index_sql("users", "email", "varchar(255)");
		let sql = varchar_sql.unwrap();
		assert!(sql.contains("varchar_pattern_ops"));

		// text should create index
		let text_sql = editor.create_like_index_sql("users", "bio", "text");
		let sql = text_sql.unwrap();
		assert!(sql.contains("text_pattern_ops"));

		// integer should not create index
		let int_sql = editor.create_like_index_sql("users", "id", "integer");
		assert!(int_sql.is_none());

		// varchar array should not create index
		let array_sql = editor.create_like_index_sql("users", "tags", "varchar[100]");
		assert!(array_sql.is_none());
	}
}
