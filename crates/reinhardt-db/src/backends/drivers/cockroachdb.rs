//! CockroachDB Backend Module
//!
//! This module provides CockroachDB-specific database backend functionality,
//! extending PostgreSQL compatibility with distributed database features.
//!
//! CockroachDB is a distributed SQL database built on PostgreSQL wire protocol,
//! offering ACID transactions, strong consistency, and horizontal scalability.
//!
//! # Features
//!
//! - **Schema Operations**: Multi-region table creation, partitioning, and region-aware indexes
//! - **Distributed Transactions**: Automatic retry logic for serialization conflicts
//! - **AS OF SYSTEM TIME**: Historical query support for consistent snapshots
//! - **Connection Management**: PostgreSQL-compatible connection pooling with CockroachDB optimizations
//!
//! # Example
//!
//! ```rust,no_run
//! use reinhardt_db::backends::drivers::cockroachdb::{
//!     CockroachDBBackend,
//!     schema::CockroachDBSchemaEditor,
//! };
//! use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
//! use sqlx::PgPool;
//!
//! // Create a CockroachDB backend
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
//! let pg_editor = PostgreSQLSchemaEditor::new(pool);
//! let backend = CockroachDBBackend::new(pg_editor);
//!
//! // Create a multi-region table
//! let editor = backend.schema_editor();
//! let sql = editor.create_table_with_locality_sql(
//!     "users",
//!     &[("id", "UUID PRIMARY KEY"), ("name", "VARCHAR(100)")],
//!     "REGIONAL BY ROW"
//! );
//! assert!(sql.contains("LOCALITY REGIONAL BY ROW"));
//! # Ok(())
//! # }
//! ```

/// CockroachDB connection management.
pub mod connection;
/// CockroachDB distributed transaction support.
pub mod distributed_tx;
/// CockroachDB schema editor implementation.
pub mod schema;

pub use connection::{CockroachDBConnection, CockroachDBConnectionConfig};
pub use distributed_tx::{ClusterInfo, CockroachDBTransactionManager};
pub use schema::CockroachDBSchemaEditor;

use super::postgresql::schema::PostgreSQLSchemaEditor;

/// CockroachDB Backend
///
/// Main backend structure that combines PostgreSQL compatibility
/// with CockroachDB-specific features.
///
/// # Examples
///
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_db::backends::drivers::cockroachdb::CockroachDBBackend;
/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
/// use sqlx::PgPool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
/// let backend = CockroachDBBackend::new(pg_editor);
/// # Ok(())
/// # }
/// # Ok(())
/// # }
/// ```
pub struct CockroachDBBackend {
	schema_editor: CockroachDBSchemaEditor,
}

impl CockroachDBBackend {
	/// Create a new CockroachDB backend
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::backends::drivers::cockroachdb::CockroachDBBackend;
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let backend = CockroachDBBackend::new(pg_editor);
	/// # Ok(())
	/// # }
	/// ```
	pub fn new(pg_editor: PostgreSQLSchemaEditor) -> Self {
		Self {
			schema_editor: CockroachDBSchemaEditor::new(pg_editor),
		}
	}

	/// Get a reference to the schema editor
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::backends::drivers::cockroachdb::CockroachDBBackend;
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let backend = CockroachDBBackend::new(pg_editor);
	/// let editor = backend.schema_editor();
	///
	/// let sql = editor.show_regions_sql();
	/// assert_eq!(sql, "SHOW REGIONS");
	/// # Ok(())
	/// # }
	/// ```
	pub fn schema_editor(&self) -> &CockroachDBSchemaEditor {
		&self.schema_editor
	}

	/// Get a mutable reference to the schema editor
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::backends::drivers::cockroachdb::CockroachDBBackend;
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let mut backend = CockroachDBBackend::new(pg_editor);
	/// let editor = backend.schema_editor_mut();
	/// # Ok(())
	/// # }
	/// ```
	pub fn schema_editor_mut(&mut self) -> &mut CockroachDBSchemaEditor {
		&mut self.schema_editor
	}

	/// Get the database name identifier
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::backends::drivers::cockroachdb::CockroachDBBackend;
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let backend = CockroachDBBackend::new(pg_editor);
	/// assert_eq!(backend.database_name(), "cockroachdb");
	/// # Ok(())
	/// # }
	/// ```
	pub fn database_name(&self) -> &str {
		"cockroachdb"
	}

	/// Check if a feature is supported
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::backends::drivers::cockroachdb::CockroachDBBackend;
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let backend = CockroachDBBackend::new(pg_editor);
	///
	/// assert!(backend.supports_feature("multi_region"));
	/// assert!(backend.supports_feature("distributed_transactions"));
	/// assert!(backend.supports_feature("as_of_system_time"));
	/// assert!(!backend.supports_feature("unknown_feature"));
	/// # Ok(())
	/// # }
	/// ```
	pub fn supports_feature(&self, feature: &str) -> bool {
		matches!(
			feature,
			"multi_region"
				| "distributed_transactions"
				| "as_of_system_time"
				| "table_partitioning"
				| "regional_by_row"
				| "regional_by_table"
				| "global_tables"
				| "storing_indexes"
				| "transaction_priorities"
		)
	}

	/// Get a list of all supported features
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::backends::drivers::cockroachdb::CockroachDBBackend;
	/// use reinhardt_db::backends::drivers::postgresql::schema::PostgreSQLSchemaEditor;
	/// use sqlx::PgPool;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = PgPool::connect("postgresql://localhost:26257/mydb").await?;
	/// let pg_editor = PostgreSQLSchemaEditor::new(pool);
	/// let backend = CockroachDBBackend::new(pg_editor);
	/// let features = backend.supported_features();
	///
	/// assert!(features.contains(&"multi_region"));
	/// assert!(features.contains(&"distributed_transactions"));
	/// # Ok(())
	/// # }
	/// ```
	pub fn supported_features(&self) -> Vec<&str> {
		vec![
			"multi_region",
			"distributed_transactions",
			"as_of_system_time",
			"table_partitioning",
			"regional_by_row",
			"regional_by_table",
			"global_tables",
			"storing_indexes",
			"transaction_priorities",
		]
	}
}
