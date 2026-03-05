//! Admin panel migration fixtures
//!
//! This module provides AdminTableCreator which combines PostgresTableCreator
//! with AdminDatabase for admin panel integration testing.

use std::sync::Arc;

use reinhardt_db::migrations::{Operation, Result};
use reinhardt_testkit::fixtures::{PostgresTableCreator, postgres_table_creator};
use rstest::fixture;

/// Admin table creator combining schema management with admin panel access
///
/// This fixture wraps `PostgresTableCreator` and adds access to `AdminDatabase`,
/// enabling tests that need both database schema operations and admin panel
/// functionality.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
/// # use rstest::*;
/// #[rstest]
/// #[tokio::test]
/// async fn test_admin_operations(#[future] admin_table_creator: AdminTableCreator) {
///     let mut creator = admin_table_creator.await;
///
///     let schema = vec![
///         Operation::CreateTable {
///             name: "test_models".to_string(),
///             columns: vec![
///                 ColumnDefinition::new("id", FieldType::Serial).primary_key(),
///                 ColumnDefinition::new("name", FieldType::Text),
///             ],
///             constraints: vec![],
///             without_rowid: None,
///             interleave_in_parent: None,
///             partition: None,
///         },
///     ];
///     creator.apply(schema).await.unwrap();
///
///     let db = creator.admin_db();
///     // Perform admin operations...
/// }
/// ```
pub struct AdminTableCreator {
	postgres_creator: PostgresTableCreator,
	admin_db: Arc<reinhardt_admin::core::AdminDatabase>,
}

impl AdminTableCreator {
	/// Create a new AdminTableCreator
	pub fn new(
		postgres_creator: PostgresTableCreator,
		admin_db: Arc<reinhardt_admin::core::AdminDatabase>,
	) -> Self {
		Self {
			postgres_creator,
			admin_db,
		}
	}

	/// Apply schema operations
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub async fn apply(&mut self, schema: Vec<Operation>) -> Result<()> {
		self.postgres_creator.apply(schema).await
	}

	/// Get a reference to the AdminDatabase
	///
	/// This provides access to admin panel operations like list, create, update, delete.
	pub fn admin_db(&self) -> &Arc<reinhardt_admin::core::AdminDatabase> {
		&self.admin_db
	}

	/// Get a reference to the database connection pool
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub fn pool(&self) -> &Arc<sqlx::PgPool> {
		self.postgres_creator.pool()
	}

	/// Get the database URL
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub fn url(&self) -> &str {
		self.postgres_creator.url()
	}

	/// Get the database port
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub fn port(&self) -> u16 {
		self.postgres_creator.port()
	}

	/// Get a reference to the container
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub fn container(
		&self,
	) -> &reinhardt_testkit::testcontainers::ContainerAsync<
		reinhardt_testkit::testcontainers::GenericImage,
	> {
		self.postgres_creator.container()
	}

	/// Insert data into a table using reinhardt-query
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub async fn insert_data(
		&self,
		table: &str,
		columns: Vec<&str>,
		values: Vec<Vec<reinhardt_query::prelude::Value>>,
	) -> Result<()> {
		self.postgres_creator
			.insert_data(table, columns, values)
			.await
	}

	/// Execute custom SQL (fallback for complex cases)
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub async fn execute_sql(&self, sql: &str) -> Result<sqlx::postgres::PgQueryResult> {
		self.postgres_creator.execute_sql(sql).await
	}

	/// Begin a transaction for advanced test scenarios
	///
	/// Delegates to the underlying PostgresTableCreator.
	pub async fn begin_transaction(&self) -> Result<sqlx::Transaction<'_, sqlx::Postgres>> {
		self.postgres_creator.begin_transaction().await
	}
}

/// Creates an AdminTableCreator for applying test schemas in admin panel tests
///
/// This fixture combines PostgresTableCreator with AdminDatabase, providing
/// a convenient helper for tests that need both schema management and admin
/// panel functionality.
#[fixture]
pub async fn admin_table_creator(
	#[future] postgres_table_creator: PostgresTableCreator,
) -> AdminTableCreator {
	use reinhardt_db::DatabaseConnection;
	use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
	use reinhardt_db::backends::dialect::PostgresBackend;
	use std::sync::Arc as StdArc;

	let creator = postgres_table_creator.await;

	// Create backends connection from pool
	let backend = StdArc::new(PostgresBackend::new((**creator.pool()).clone()));
	let backends_conn = BackendsConnection::new(backend);

	// Create ORM connection
	let connection = DatabaseConnection::new(
		reinhardt_db::orm::connection::DatabaseBackend::Postgres,
		backends_conn,
	);

	// Create AdminDatabase
	let admin_db = Arc::new(reinhardt_admin::core::AdminDatabase::new(connection));

	AdminTableCreator::new(creator, admin_db)
}
