/// Schema editor factory module
///
/// This module provides a factory for creating database-specific schema editors,
/// enabling uniform database operations across different database backends.
///
/// # Architecture
///
/// ```text
/// reinhardt-orm/reinhardt-migrations
///           ↓
///    SchemaEditorFactory
///           ↓
///    BaseDatabaseSchemaEditor (trait)
///           ↓
///    ┌──────┴──────┬──────────┐
///    ↓             ↓          ↓
/// PostgreSQL    MySQL     SQLite
/// SchemaEditor  SchemaEditor SchemaEditor
/// ```
///
/// # Example
///
/// ```rust,no_run
/// # use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
/// let factory = SchemaEditorFactory::new();
/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
/// // Use the editor for DDL operations via BaseDatabaseSchemaEditor trait methods
/// ```
use super::BaseDatabaseSchemaEditor;

#[cfg(feature = "postgres")]
use crate::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;

#[cfg(feature = "mysql")]
use crate::backends::drivers::mysql::schema::MySQLSchemaEditor;

#[cfg(feature = "sqlite")]
use crate::backends::drivers::sqlite::schema::SQLiteSchemaEditor;

use std::sync::Arc;

#[cfg(feature = "postgres")]
use sqlx::PgPool;

#[cfg(feature = "mysql")]
use sqlx::MySqlPool;

#[cfg(feature = "sqlite")]
use sqlx::SqlitePool;

/// Database type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseType {
	/// PostgreSQL database
	PostgreSQL,
	/// MySQL database
	MySQL,
	/// SQLite database
	SQLite,
}

impl DatabaseType {
	/// Parse database type from connection string
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::schema::factory::DatabaseType;
	/// let db_type = DatabaseType::from_connection_string("postgres://localhost/mydb");
	/// assert_eq!(db_type, Some(DatabaseType::PostgreSQL));
	///
	/// let db_type = DatabaseType::from_connection_string("mysql://localhost/mydb");
	/// assert_eq!(db_type, Some(DatabaseType::MySQL));
	///
	/// let db_type = DatabaseType::from_connection_string("sqlite:///data.db");
	/// assert_eq!(db_type, Some(DatabaseType::SQLite));
	/// ```
	pub fn from_connection_string(conn_str: &str) -> Option<Self> {
		if conn_str.starts_with("postgres://") || conn_str.starts_with("postgresql://") {
			Some(DatabaseType::PostgreSQL)
		} else if conn_str.starts_with("mysql://") {
			Some(DatabaseType::MySQL)
		} else if conn_str.starts_with("sqlite://") {
			Some(DatabaseType::SQLite)
		} else {
			None
		}
	}

	/// Get the database type name as a string
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::schema::factory::DatabaseType;
	/// assert_eq!(DatabaseType::PostgreSQL.as_str(), "postgresql");
	/// assert_eq!(DatabaseType::MySQL.as_str(), "mysql");
	/// assert_eq!(DatabaseType::SQLite.as_str(), "sqlite");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			DatabaseType::PostgreSQL => "postgresql",
			DatabaseType::MySQL => "mysql",
			DatabaseType::SQLite => "sqlite",
		}
	}
}

/// Schema editor factory for creating database-specific schema editors
///
/// This factory provides a unified interface for creating schema editors
/// that work with different database backends.
///
/// # Example
///
/// ```no_run
/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
/// use sqlx::PgPool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
/// let factory = SchemaEditorFactory::new_postgres(pool);
/// // Create a PostgreSQL schema editor
/// let pg_editor = factory.create_for_database(DatabaseType::PostgreSQL);
/// // Use BaseDatabaseSchemaEditor trait methods for DDL operations
/// # Ok(())
/// # }
/// ```
pub struct SchemaEditorFactory {
	#[cfg(feature = "postgres")]
	pg_pool: Option<Arc<PgPool>>,
	#[cfg(feature = "mysql")]
	// Allow dead_code: pool stored for future MySQL schema operations
	#[allow(dead_code)]
	mysql_pool: Option<Arc<MySqlPool>>,
	#[cfg(feature = "sqlite")]
	// Allow dead_code: pool stored for future SQLite schema operations
	#[allow(dead_code)]
	sqlite_pool: Option<Arc<SqlitePool>>,
}

impl SchemaEditorFactory {
	/// Create a new empty schema editor factory
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_db::backends::schema::factory::SchemaEditorFactory;
	/// let factory = SchemaEditorFactory::new();
	/// # drop(factory); // Verify it's creatable
	/// ```
	pub fn new() -> Self {
		Self {
			#[cfg(feature = "postgres")]
			pg_pool: None,
			#[cfg(feature = "mysql")]
			mysql_pool: None,
			#[cfg(feature = "sqlite")]
			sqlite_pool: None,
		}
	}

	/// Create a factory with PostgreSQL pool
	///
	/// # Example
	///
	/// ```no_run
	/// use reinhardt_db::backends::schema::factory::SchemaEditorFactory;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// let factory = SchemaEditorFactory::new_postgres(pool);
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "postgres")]
	pub fn new_postgres(pool: PgPool) -> Self {
		Self {
			pg_pool: Some(Arc::new(pool)),
			#[cfg(feature = "mysql")]
			mysql_pool: None,
			#[cfg(feature = "sqlite")]
			sqlite_pool: None,
		}
	}

	/// Create a factory with MySQL pool
	#[cfg(feature = "mysql")]
	pub fn new_mysql(pool: MySqlPool) -> Self {
		Self {
			#[cfg(feature = "postgres")]
			pg_pool: None,
			mysql_pool: Some(Arc::new(pool)),
			#[cfg(feature = "sqlite")]
			sqlite_pool: None,
		}
	}

	/// Create a factory with SQLite pool
	#[cfg(feature = "sqlite")]
	pub fn new_sqlite(pool: SqlitePool) -> Self {
		Self {
			#[cfg(feature = "postgres")]
			pg_pool: None,
			#[cfg(feature = "mysql")]
			mysql_pool: None,
			sqlite_pool: Some(Arc::new(pool)),
		}
	}

	/// Create a schema editor for the specified database type
	///
	/// # Example
	///
	/// ```no_run
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// let factory = SchemaEditorFactory::new_postgres(pool);
	/// let editor = factory.create_for_database(DatabaseType::PostgreSQL);
	/// # Ok(())
	/// # }
	/// ```
	pub fn create_for_database(&self, db_type: DatabaseType) -> Box<dyn BaseDatabaseSchemaEditor> {
		match db_type {
			#[cfg(feature = "postgres")]
			DatabaseType::PostgreSQL => {
				let pool = self
					.pg_pool
					.as_ref()
					.expect("PostgreSQL pool not set. Use SchemaEditorFactory::new_postgres()");
				Box::new(PostgreSQLSchemaEditor::from_pool_arc(Arc::clone(pool)))
			}

			#[cfg(not(feature = "postgres"))]
			DatabaseType::PostgreSQL => {
				panic!("PostgreSQL support not enabled. Enable 'postgres' feature.")
			}

			#[cfg(feature = "mysql")]
			DatabaseType::MySQL => Box::new(MySQLSchemaEditor::new()),

			#[cfg(not(feature = "mysql"))]
			DatabaseType::MySQL => {
				panic!("MySQL support not enabled. Enable 'mysql' feature.")
			}

			#[cfg(feature = "sqlite")]
			DatabaseType::SQLite => Box::new(SQLiteSchemaEditor::new()),

			#[cfg(not(feature = "sqlite"))]
			DatabaseType::SQLite => {
				panic!("SQLite support not enabled. Enable 'sqlite' feature.")
			}
		}
	}

	/// Create an Arc-wrapped schema editor for shared access
	///
	/// This is useful when you need to share the schema editor across multiple threads.
	///
	/// # Example
	///
	/// ```no_run
	/// use reinhardt_db::backends::schema::factory::{SchemaEditorFactory, DatabaseType};
	/// use std::sync::Arc;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), sqlx::Error> {
	/// let pool = PgPool::connect("postgresql://localhost/mydb").await?;
	/// let factory = SchemaEditorFactory::new_postgres(pool);
	/// let editor = factory.create_shared(DatabaseType::PostgreSQL);
	///
	/// // Can be cloned and shared across threads
	/// let editor_clone = Arc::clone(&editor);
	/// # Ok(())
	/// # }
	/// ```
	pub fn create_shared(
		&self,
		db_type: DatabaseType,
	) -> Arc<dyn BaseDatabaseSchemaEditor + Send + Sync> {
		match db_type {
			#[cfg(feature = "postgres")]
			DatabaseType::PostgreSQL => {
				let pool = self
					.pg_pool
					.as_ref()
					.expect("PostgreSQL pool not set. Use SchemaEditorFactory::new_postgres()");
				Arc::new(PostgreSQLSchemaEditor::from_pool_arc(Arc::clone(pool)))
			}

			#[cfg(not(feature = "postgres"))]
			DatabaseType::PostgreSQL => {
				panic!("PostgreSQL support not enabled. Enable 'postgres' feature.")
			}

			#[cfg(feature = "mysql")]
			DatabaseType::MySQL => Arc::new(MySQLSchemaEditor::new()),

			#[cfg(not(feature = "mysql"))]
			DatabaseType::MySQL => {
				panic!("MySQL support not enabled. Enable 'mysql' feature.")
			}

			#[cfg(feature = "sqlite")]
			DatabaseType::SQLite => Arc::new(SQLiteSchemaEditor::new()),

			#[cfg(not(feature = "sqlite"))]
			DatabaseType::SQLite => {
				panic!("SQLite support not enabled. Enable 'sqlite' feature.")
			}
		}
	}
}

impl Default for SchemaEditorFactory {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[test]
	fn test_database_type_from_connection_string() {
		assert_eq!(
			DatabaseType::from_connection_string("postgres://localhost/mydb"),
			Some(DatabaseType::PostgreSQL)
		);
		assert_eq!(
			DatabaseType::from_connection_string("postgresql://localhost/mydb"),
			Some(DatabaseType::PostgreSQL)
		);
		assert_eq!(
			DatabaseType::from_connection_string("mysql://localhost/mydb"),
			Some(DatabaseType::MySQL)
		);
		assert_eq!(
			DatabaseType::from_connection_string("sqlite:///data.db"),
			Some(DatabaseType::SQLite)
		);
		assert_eq!(
			DatabaseType::from_connection_string("unknown://localhost/mydb"),
			None
		);
	}

	#[test]
	fn test_database_type_as_str() {
		assert_eq!(DatabaseType::PostgreSQL.as_str(), "postgresql");
		assert_eq!(DatabaseType::MySQL.as_str(), "mysql");
		assert_eq!(DatabaseType::SQLite.as_str(), "sqlite");
	}

	#[test]
	fn test_factory_creation() {
		let factory = SchemaEditorFactory::new();
		let _factory2 = SchemaEditorFactory::default();
		// Factory should be creatable
		drop(factory);
	}

	#[cfg(feature = "postgres")]
	#[fixture]
	async fn pg_pool() -> PgPool {
		PgPool::connect_lazy("postgresql://localhost/test_db").expect("Failed to create test pool")
	}

	#[cfg(feature = "postgres")]
	#[rstest]
	#[tokio::test]
	async fn test_create_postgresql_editor(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let factory = SchemaEditorFactory::new_postgres(pool);
		let _editor = factory.create_for_database(DatabaseType::PostgreSQL);
		// Editor created successfully
	}

	#[cfg(feature = "postgres")]
	#[rstest]
	#[tokio::test]
	async fn test_create_shared_editor(#[future] pg_pool: PgPool) {
		let pool = pg_pool.await;
		let factory = SchemaEditorFactory::new_postgres(pool);
		let editor = factory.create_shared(DatabaseType::PostgreSQL);

		let _editor_clone = Arc::clone(&editor);
		assert_eq!(Arc::strong_count(&editor), 2);
	}
}
