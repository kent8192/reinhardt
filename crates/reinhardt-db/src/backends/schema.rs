/// Database schema editor module
///
/// This module provides the foundation for DDL (Data Definition Language) operations
/// across different database backends, inspired by Django's schema editor architecture.
///
/// # Example
///
/// ```rust
/// # use reinhardt_db::backends::schema::DDLStatement;
/// let create_table = DDLStatement::CreateTable {
///     table: "users".to_string(),
///     columns: vec![
///         ("id".to_string(), "INTEGER PRIMARY KEY".to_string()),
///         ("name".to_string(), "VARCHAR(100)".to_string()),
///     ],
/// };
/// assert_eq!(create_table.table_name(), "users");
/// ```
use std::fmt;

use reinhardt_query::prelude::{
	Alias, AlterTableStatement, ColumnDef, CreateIndexStatement, CreateTableStatement,
	DropIndexStatement, DropTableStatement, MySqlQueryBuilder, PostgresQueryBuilder, Query,
	QueryBuilder, SqliteQueryBuilder,
};

/// DDL reference objects for schema operations
pub mod ddl_references;

/// Schema editor factory for creating database-specific editors
pub mod factory;

/// Represents a DDL statement type
#[derive(Debug, Clone, PartialEq)]
pub enum DDLStatement {
	/// CREATE TABLE statement
	CreateTable {
		table: String,
		columns: Vec<(String, String)>,
	},
	/// ALTER TABLE statement
	AlterTable {
		table: String,
		changes: Vec<AlterTableChange>,
	},
	/// DROP TABLE statement
	DropTable { table: String, cascade: bool },
	/// CREATE INDEX statement
	CreateIndex {
		name: String,
		table: String,
		columns: Vec<String>,
		unique: bool,
		condition: Option<String>,
	},
	/// DROP INDEX statement
	DropIndex { name: String },
	/// CREATE SCHEMA statement
	CreateSchema { name: String, if_not_exists: bool },
	/// DROP SCHEMA statement
	DropSchema {
		name: String,
		cascade: bool,
		if_exists: bool,
	},
	/// Raw SQL statement
	RawSQL(String),
}

impl DDLStatement {
	/// Get the table name associated with this DDL statement
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::schema::DDLStatement;
	/// let stmt = DDLStatement::CreateTable {
	///     table: "users".to_string(),
	///     columns: vec![],
	/// };
	/// assert_eq!(stmt.table_name(), "users");
	/// ```
	pub fn table_name(&self) -> &str {
		match self {
			DDLStatement::CreateTable { table, .. } => table,
			DDLStatement::AlterTable { table, .. } => table,
			DDLStatement::DropTable { table, .. } => table,
			DDLStatement::CreateIndex { table, .. } => table,
			DDLStatement::DropIndex { .. } => "",
			DDLStatement::CreateSchema { .. } => "",
			DDLStatement::DropSchema { .. } => "",
			DDLStatement::RawSQL(_) => "",
		}
	}
}

/// ALTER TABLE change operations
#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableChange {
	/// Add a column
	AddColumn { name: String, definition: String },
	/// Drop a column
	DropColumn { name: String },
	/// Rename a column
	RenameColumn { old_name: String, new_name: String },
	/// Alter column type
	AlterColumnType {
		name: String,
		new_type: String,
		collation: Option<String>,
	},
	/// Set/drop column default
	AlterColumnDefault {
		name: String,
		default: Option<String>,
	},
	/// Set/drop NOT NULL constraint
	AlterColumnNullability { name: String, nullable: bool },
	/// Add constraint
	AddConstraint { name: String, definition: String },
	/// Drop constraint
	DropConstraint { name: String },
}

/// Base trait for database schema editors
///
/// This trait defines the interface that all database-specific schema editors must implement.
/// It provides methods for creating, altering, and dropping database schema objects.
///
/// # Example
///
/// ```rust,no_run
/// # use reinhardt_db::backends::schema::{BaseDatabaseSchemaEditor, SchemaEditorResult};
/// # use reinhardt_db::backends::DatabaseType;
/// # use async_trait::async_trait;
/// struct MySchemaEditor;
///
/// #[async_trait]
/// impl BaseDatabaseSchemaEditor for MySchemaEditor {
///     fn database_type(&self) -> DatabaseType {
///         DatabaseType::Postgres
///     }
///
///     async fn execute(&mut self, sql: &str) -> SchemaEditorResult<()> {
///         println!("Executing: {}", sql);
///         Ok(())
///     }
/// }
///
/// # async fn example() {
/// let mut editor = MySchemaEditor;
/// editor.execute("CREATE TABLE users (id INT)").await.unwrap();
/// # }
/// ```
#[async_trait::async_trait]
pub trait BaseDatabaseSchemaEditor: Send + Sync {
	/// Get the database type for this schema editor
	///
	/// Used to select the appropriate query builder when generating SQL
	fn database_type(&self) -> super::types::DatabaseType;

	/// Execute a SQL statement
	async fn execute(&mut self, sql: &str) -> SchemaEditorResult<()>;

	/// Generate CREATE TABLE statement using reinhardt-query
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::schema::{BaseDatabaseSchemaEditor, SchemaEditorResult};
	/// # use reinhardt_db::backends::DatabaseType;
	/// # use async_trait::async_trait;
	/// # use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};
	/// struct TestEditor;
	///
	/// #[async_trait]
	/// impl BaseDatabaseSchemaEditor for TestEditor {
	///     fn database_type(&self) -> DatabaseType {
	///         DatabaseType::Postgres
	///     }
	///
	///     async fn execute(&mut self, _sql: &str) -> SchemaEditorResult<()> {
	///         Ok(())
	///     }
	/// }
	///
	/// let editor = TestEditor;
	/// let stmt = editor.create_table_statement("users", &[
	///     ("id", "INTEGER PRIMARY KEY"),
	///     ("name", "VARCHAR(100)"),
	/// ]);
	/// let (sql, _) = PostgresQueryBuilder.build_create_table(&stmt);
	/// assert!(sql.contains("CREATE TABLE"));
	/// assert!(sql.contains("\"users\""));
	/// ```
	fn create_table_statement(
		&self,
		table: &str,
		columns: &[(&str, &str)],
	) -> CreateTableStatement {
		let mut binding = Query::create_table();
		let stmt = binding.table(Alias::new(table)).if_not_exists();

		for (name, definition) in columns {
			// Use custom() for raw type definitions since we receive them as strings
			stmt.col(ColumnDef::new(Alias::new(*name)).custom(*definition));
		}

		stmt.to_owned()
	}

	/// Generate DROP TABLE statement using reinhardt-query
	fn drop_table_statement(&self, table: &str, cascade: bool) -> DropTableStatement {
		let mut binding = Query::drop_table();
		let stmt = binding.table(Alias::new(table)).if_exists();

		if cascade {
			stmt.cascade();
		}

		stmt.to_owned()
	}

	/// Generate ALTER TABLE ADD COLUMN statement using reinhardt-query
	fn add_column_statement(
		&self,
		table: &str,
		column: &str,
		definition: &str,
	) -> AlterTableStatement {
		// Use custom() for raw type definitions
		Query::alter_table()
			.table(Alias::new(table))
			.add_column(ColumnDef::new(Alias::new(column)).custom(Alias::new(definition)))
			.to_owned()
	}

	/// Generate ALTER TABLE DROP COLUMN statement using reinhardt-query
	fn drop_column_statement(&self, table: &str, column: &str) -> AlterTableStatement {
		Query::alter_table()
			.table(Alias::new(table))
			.drop_column(Alias::new(column))
			.to_owned()
	}

	/// Generate ALTER TABLE RENAME COLUMN SQL
	///
	/// Always uses double quotes for PostgreSQL identifier safety.
	/// Note: reinhardt-query doesn't support RENAME COLUMN, so we use raw SQL.
	fn rename_column_statement(&self, table: &str, old_name: &str, new_name: &str) -> String {
		format!(
			"ALTER TABLE \"{}\" RENAME COLUMN \"{}\" TO \"{}\"",
			table, old_name, new_name
		)
	}

	/// Generate ALTER TABLE ALTER COLUMN TYPE SQL
	///
	/// Returns database-specific SQL for changing a column's type.
	/// Note: reinhardt-query doesn't support ALTER COLUMN TYPE, so we use raw SQL.
	///
	/// Default implementation uses PostgreSQL syntax:
	/// `ALTER TABLE table ALTER COLUMN column TYPE new_type`
	///
	/// Override this method in database-specific schema editors for:
	/// - MySQL: `ALTER TABLE table MODIFY COLUMN column new_type`
	/// - SQLite: Requires table recreation (complex multi-step process)
	/// - CockroachDB: Same as PostgreSQL
	fn alter_column_statement(&self, table: &str, column: &str, new_type: &str) -> String {
		format!(
			"ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {}",
			table, column, new_type
		)
	}

	/// Generate CREATE INDEX statement using reinhardt-query (or raw SQL for partial indexes)
	///
	/// Note: reinhardt-query doesn't support partial indexes (WHERE clause), so we use raw SQL for those cases
	fn create_index_statement(
		&self,
		name: &str,
		table: &str,
		columns: &[&str],
		unique: bool,
		condition: Option<&str>,
	) -> Result<CreateIndexStatement, String> {
		if condition.is_some() {
			// reinhardt-query doesn't support partial indexes, return error to indicate fallback needed
			// Always use double quotes for PostgreSQL identifier safety
			return Err(format!(
				"Partial indexes not supported by reinhardt-query. Use raw SQL: CREATE {}INDEX \"{}\" ON \"{}\" ({}) WHERE {}",
				if unique { "UNIQUE " } else { "" },
				name,
				table,
				columns
					.iter()
					.map(|c| format!("\"{}\"", c))
					.collect::<Vec<_>>()
					.join(", "),
				condition.unwrap()
			));
		}

		let mut binding = Query::create_index();
		let stmt = binding.name(Alias::new(name)).table(Alias::new(table));

		if unique {
			stmt.unique();
		}

		for col in columns {
			stmt.col(Alias::new(*col));
		}

		Ok(stmt.to_owned())
	}

	/// Generate DROP INDEX statement using reinhardt-query
	fn drop_index_statement(&self, name: &str) -> DropIndexStatement {
		let mut binding = Query::drop_index();
		binding.name(Alias::new(name)).if_exists().to_owned()
	}

	/// Generate CREATE SCHEMA statement
	///
	/// Note: reinhardt-query doesn't support CREATE SCHEMA, so we use raw SQL
	///
	/// # Arguments
	///
	/// * `name` - Schema name
	/// * `if_not_exists` - Whether to add IF NOT EXISTS clause
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::schema::BaseDatabaseSchemaEditor;
	/// # use reinhardt_db::backends::DatabaseType;
	/// # use async_trait::async_trait;
	/// # use reinhardt_db::backends::schema::SchemaEditorResult;
	/// struct TestEditor;
	///
	/// #[async_trait]
	/// impl BaseDatabaseSchemaEditor for TestEditor {
	///     fn database_type(&self) -> DatabaseType {
	///         DatabaseType::Postgres
	///     }
	///
	///     async fn execute(&mut self, _sql: &str) -> SchemaEditorResult<()> {
	///         Ok(())
	///     }
	/// }
	///
	/// let editor = TestEditor;
	/// let sql = editor.create_schema_statement("my_schema", true);
	/// assert_eq!(sql, "CREATE SCHEMA IF NOT EXISTS \"my_schema\"");
	/// ```
	fn create_schema_statement(&self, name: &str, if_not_exists: bool) -> String {
		if if_not_exists {
			format!("CREATE SCHEMA IF NOT EXISTS \"{}\"", name)
		} else {
			format!("CREATE SCHEMA \"{}\"", name)
		}
	}

	/// Generate DROP SCHEMA statement
	///
	/// Note: reinhardt-query doesn't support DROP SCHEMA, so we use raw SQL
	///
	/// # Arguments
	///
	/// * `name` - Schema name
	/// * `cascade` - Whether to add CASCADE clause
	/// * `if_exists` - Whether to add IF EXISTS clause
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::schema::BaseDatabaseSchemaEditor;
	/// # use reinhardt_db::backends::DatabaseType;
	/// # use async_trait::async_trait;
	/// # use reinhardt_db::backends::schema::SchemaEditorResult;
	/// struct TestEditor;
	///
	/// #[async_trait]
	/// impl BaseDatabaseSchemaEditor for TestEditor {
	///     fn database_type(&self) -> DatabaseType {
	///         DatabaseType::Postgres
	///     }
	///
	///     async fn execute(&mut self, _sql: &str) -> SchemaEditorResult<()> {
	///         Ok(())
	///     }
	/// }
	///
	/// let editor = TestEditor;
	/// let sql = editor.drop_schema_statement("my_schema", true, true);
	/// assert_eq!(sql, "DROP SCHEMA IF EXISTS \"my_schema\" CASCADE");
	/// ```
	fn drop_schema_statement(&self, name: &str, cascade: bool, if_exists: bool) -> String {
		let if_exists_clause = if if_exists { " IF EXISTS" } else { "" };
		let cascade_clause = if cascade { " CASCADE" } else { "" };

		format!(
			"DROP SCHEMA{} \"{}\"{}",
			if_exists_clause, name, cascade_clause
		)
	}

	/// Build SQL string from `CreateTableStatement` using appropriate QueryBuilder
	fn build_create_table_sql(&self, stmt: &CreateTableStatement) -> String {
		use super::types::DatabaseType;

		let (sql, _values) = match self.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_create_table(stmt),
			DatabaseType::Mysql => MySqlQueryBuilder.build_create_table(stmt),
			DatabaseType::Sqlite => SqliteQueryBuilder.build_create_table(stmt),
		};
		sql
	}

	/// Build SQL string from `DropTableStatement` using appropriate QueryBuilder
	fn build_drop_table_sql(&self, stmt: &DropTableStatement) -> String {
		use super::types::DatabaseType;

		let (sql, _values) = match self.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_drop_table(stmt),
			DatabaseType::Mysql => MySqlQueryBuilder.build_drop_table(stmt),
			DatabaseType::Sqlite => SqliteQueryBuilder.build_drop_table(stmt),
		};
		sql
	}

	/// Build SQL string from `AlterTableStatement` using appropriate QueryBuilder
	fn build_alter_table_sql(&self, stmt: &AlterTableStatement) -> String {
		use super::types::DatabaseType;

		let (sql, _values) = match self.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_alter_table(stmt),
			DatabaseType::Mysql => MySqlQueryBuilder.build_alter_table(stmt),
			DatabaseType::Sqlite => SqliteQueryBuilder.build_alter_table(stmt),
		};
		sql
	}

	/// Build SQL string from `CreateIndexStatement` using appropriate QueryBuilder
	fn build_create_index_sql(&self, stmt: &CreateIndexStatement) -> String {
		use super::types::DatabaseType;

		let (sql, _values) = match self.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_create_index(stmt),
			DatabaseType::Mysql => MySqlQueryBuilder.build_create_index(stmt),
			DatabaseType::Sqlite => SqliteQueryBuilder.build_create_index(stmt),
		};
		sql
	}

	/// Build SQL string from `DropIndexStatement` using appropriate QueryBuilder
	fn build_drop_index_sql(&self, stmt: &DropIndexStatement) -> String {
		use super::types::DatabaseType;

		let (sql, _values) = match self.database_type() {
			DatabaseType::Postgres => PostgresQueryBuilder.build_drop_index(stmt),
			DatabaseType::Mysql => MySqlQueryBuilder.build_drop_index(stmt),
			DatabaseType::Sqlite => SqliteQueryBuilder.build_drop_index(stmt),
		};
		sql
	}
}

/// Result type for schema editor operations
pub type SchemaEditorResult<T> = Result<T, SchemaEditorError>;

/// Errors that can occur during schema editing operations
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SchemaEditorError {
	/// SQL execution error
	ExecutionError(String),
	/// Invalid operation
	InvalidOperation(String),
	/// Database error
	DatabaseError(String),
}

impl fmt::Display for SchemaEditorError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SchemaEditorError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
			SchemaEditorError::InvalidOperation(msg) => {
				write!(f, "Invalid operation: {}", msg)
			}
			SchemaEditorError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
		}
	}
}

impl std::error::Error for SchemaEditorError {}

#[cfg(test)]
mod tests {
	use super::*;

	struct TestSchemaEditor;

	#[async_trait::async_trait]
	impl BaseDatabaseSchemaEditor for TestSchemaEditor {
		async fn execute(&mut self, _sql: &str) -> SchemaEditorResult<()> {
			Ok(())
		}

		fn database_type(&self) -> crate::backends::types::DatabaseType {
			crate::backends::types::DatabaseType::Postgres
		}
	}

	#[test]
	fn test_create_table_statement() {
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};

		let editor = TestSchemaEditor;
		let stmt = editor.create_table_statement(
			"users",
			&[("id", "INTEGER PRIMARY KEY"), ("name", "VARCHAR(100)")],
		);
		let (sql, _) = PostgresQueryBuilder.build_create_table(&stmt);

		assert!(sql.contains("CREATE TABLE"));
		assert!(sql.contains("\"users\""));
		assert!(sql.contains("\"id\""));
		assert!(sql.contains("\"name\""));
	}

	#[test]
	fn test_drop_table_statement() {
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};

		let editor = TestSchemaEditor;

		let stmt_no_cascade = editor.drop_table_statement("users", false);
		let (sql_no_cascade, _) = PostgresQueryBuilder.build_drop_table(&stmt_no_cascade);
		assert!(sql_no_cascade.contains("DROP TABLE"));
		assert!(sql_no_cascade.contains("\"users\""));

		let stmt_cascade = editor.drop_table_statement("users", true);
		let (sql_cascade, _) = PostgresQueryBuilder.build_drop_table(&stmt_cascade);
		assert!(sql_cascade.contains("DROP TABLE"));
		assert!(sql_cascade.contains("\"users\""));
		assert!(sql_cascade.contains("CASCADE"));
	}

	#[test]
	fn test_add_column_statement() {
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};

		let editor = TestSchemaEditor;
		let stmt = editor.add_column_statement("users", "email", "VARCHAR(255)");
		let (sql, _) = PostgresQueryBuilder.build_alter_table(&stmt);

		assert!(sql.contains("ALTER TABLE"));
		assert!(sql.contains("\"users\""));
		assert!(sql.contains("ADD COLUMN"));
		assert!(sql.contains("\"email\""));
		assert!(sql.contains("VARCHAR(255)"));
	}

	#[test]
	fn test_create_index_statement() {
		use reinhardt_query::prelude::{PostgresQueryBuilder, QueryBuilder};

		let editor = TestSchemaEditor;

		// Simple index
		let stmt = editor.create_index_statement("idx_email", "users", &["email"], false, None);
		let (sql, _) = PostgresQueryBuilder.build_create_index(&stmt.unwrap());
		assert!(sql.contains("CREATE INDEX"));
		assert!(sql.contains("idx_email"));
		assert!(sql.contains("\"users\""));

		// Unique index
		let unique_stmt =
			editor.create_index_statement("idx_email_uniq", "users", &["email"], true, None);
		let (unique_sql, _) = PostgresQueryBuilder.build_create_index(&unique_stmt.unwrap());
		assert!(unique_sql.contains("CREATE UNIQUE INDEX"));

		// Partial index (not supported by reinhardt-query, returns error with fallback SQL)
		let partial_result = editor.create_index_statement(
			"idx_active",
			"users",
			&["email"],
			false,
			Some("active = true"),
		);
		assert!(partial_result.is_err());
		let error_msg = partial_result.unwrap_err();
		assert!(error_msg.contains("Partial indexes not supported"));
		assert!(error_msg.contains("WHERE active = true"));
	}

	#[test]
	fn test_alter_column_statement() {
		let editor = TestSchemaEditor;

		// Test default PostgreSQL syntax
		let sql = editor.alter_column_statement("users", "email", "TEXT");
		assert_eq!(
			sql,
			"ALTER TABLE \"users\" ALTER COLUMN \"email\" TYPE TEXT"
		);

		// Verify identifier quoting
		assert!(sql.contains("\"users\""));
		assert!(sql.contains("\"email\""));
		assert!(sql.contains("TYPE TEXT"));
	}

	#[test]
	fn test_ddl_statement_table_name() {
		let stmt = DDLStatement::CreateTable {
			table: "users".to_string(),
			columns: vec![],
		};
		assert_eq!(stmt.table_name(), "users");

		let alter_stmt = DDLStatement::AlterTable {
			table: "posts".to_string(),
			changes: vec![],
		};
		assert_eq!(alter_stmt.table_name(), "posts");
	}
}

#[cfg(test)]
pub mod test_utils;
