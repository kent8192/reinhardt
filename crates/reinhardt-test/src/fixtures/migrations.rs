//! Migration Registry Test Fixtures
//! Migration test fixtures and helpers
//!
//! This module provides comprehensive fixtures for testing database migrations
//! and schema operations in Reinhardt applications.
//!
//! ## Fixture Categories
//!
//! ### Unit Testing Fixtures
//!
//! - `` `migration_registry` `` - Isolated `LocalRegistry` for unit testing
//! - `` `test_migration_source` `` - In-memory migration source
//! - `` `in_memory_repository` `` - In-memory migration repository
//!
//! ### Integration Testing Fixtures (requires `testcontainers` feature)
//!
//! - `` `migration_executor` `` - DatabaseMigrationExecutor with PostgreSQL container
//! - `` `postgres_table_creator` `` - PostgreSQL schema management helper
//! - `` `admin_table_creator` `` - Admin panel integration helper (requires `admin` feature)
//!
//! ## Usage Examples
//!
//! ### Unit Testing with LocalRegistry
//!
//! ```rust,no_run
//! # use reinhardt_test::fixtures::*;
//! # use reinhardt_db::migrations::Migration;
//! # use rstest::*;
//! // #[rstest]
//! // fn test_migration_registration(migration_registry: LocalRegistry) {
//! //     let migration = Migration::new("0001_initial", "polls");
//! //
//! //     migration_registry.register(migration).unwrap();
//! //     assert_eq!(migration_registry.all_migrations().len(), 1);
//! // }
//! ```
//!
//! ### Integration Testing with PostgresTableCreator
//!
//! ```rust,no_run
//! # use reinhardt_test::fixtures::*;
//! # use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
//! # use rstest::*;
//! // #[rstest]
//! // #[tokio::test]
//! // async fn test_with_schema(#[future] postgres_table_creator: PostgresTableCreator) {
//! //     let mut creator = postgres_table_creator.await;
//! //
//! //     // Define schema using Operation enum
//! //     let schema = vec![
//! //         Operation::CreateTable {
//! //             name: "users".to_string(),
//! //             columns: vec![
//! //                 ColumnDefinition::new("id", FieldType::Serial).primary_key(),
//! //                 ColumnDefinition::new("name", FieldType::Text),
//! //             ],
//! //             constraints: vec![],
//! //             without_rowid: None,
//! //             interleave_in_parent: None,
//! //             partition: None,
//! //         },
//! //     ];
//! //
//! //     // Apply schema
//! //     creator.apply(schema).await.unwrap();
//! //
//! //     // Use the database
//! //     let pool = creator.pool();
//! //     // ... test code ...
//! // }
//! ```
//!
//! ### Admin Panel Testing with AdminTableCreator
//!
//! ```rust,no_run
//! # use reinhardt_test::fixtures::*;
//! # use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
//! # use rstest::*;
//! // #[rstest]
//! // #[tokio::test]
//! // async fn test_admin_operations(#[future] admin_table_creator: AdminTableCreator) {
//! //     let mut creator = admin_table_creator.await;
//! //
//! //     // Create schema
//! //     let schema = vec![/* ... */];
//! //     creator.apply(schema).await.unwrap();
//! //
//! //     // Use AdminDatabase
//! //     let db = creator.admin_db();
//! //     let results = db.list::<AdminRecord>("users", vec![], 0, 10).await.unwrap();
//! // }
//! ```
//!
//! ## Migration Patterns
//!
//! The new `PostgresTableCreator` and `AdminTableCreator` fixtures promote type-safe
//! schema management using `Operation` enum instead of raw SQL strings. This provides:
//!
//! - **Type safety**: Schema defined using Rust types
//! - **Testability**: Easy to create isolated test databases
//! - **Maintainability**: Schema changes are explicit and reviewable
//! - **Consistency**: Same patterns across all tests

use async_trait::async_trait;
use reinhardt_db::migrations::registry::LocalRegistry;
use reinhardt_db::migrations::{Migration, MigrationRepository, MigrationSource, Result};
use rstest::*;
use std::collections::HashMap;

// TestContainers-related imports (conditional on feature)
#[cfg(feature = "testcontainers")]
use crate::fixtures::testcontainers::postgres_container;
#[cfg(feature = "testcontainers")]
use reinhardt_db::migrations::executor::DatabaseMigrationExecutor;
#[cfg(feature = "testcontainers")]
use reinhardt_db::migrations::{DatabaseConnection, MigrationError, Operation};
#[cfg(feature = "testcontainers")]
use std::sync::Arc;
#[cfg(feature = "testcontainers")]
use testcontainers::{ContainerAsync, GenericImage};

/// Creates a new isolated migration registry for testing
///
/// Each test gets its own empty LocalRegistry instance, ensuring complete
/// isolation between test cases. This avoids the "duplicate distributed_slice"
/// errors that occur with linkme's global registry in test environments.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::Migration;
/// # use rstest::*;
/// // #[rstest]
/// // fn test_migration_operations(migration_registry: LocalRegistry) {
/// //     // Registry starts empty
/// //     assert!(migration_registry.all_migrations().is_empty());
/// //
/// //     // Register a migration
/// //     migration_registry.register(Migration::new("0001_initial", "polls")).unwrap();
/// //
/// //     // Verify registration
/// //     assert_eq!(migration_registry.all_migrations().len(), 1);
/// // }
/// ```
#[fixture]
pub fn migration_registry() -> LocalRegistry {
	LocalRegistry::new()
}

/// In-memory migration source for testing
///
/// Provides a simple implementation of `MigrationSource` that stores migrations
/// in memory. Useful for testing migration-related functionality without
/// filesystem or database dependencies.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::TestMigrationSource;
/// # use reinhardt_db::migrations::{Migration, MigrationSource};
/// # #[tokio::main]
/// # async fn main() {
/// // #[tokio::test]
/// // async fn test_source() {
/// let mut source = TestMigrationSource::new();
/// source.add_migration(Migration::new("0001_initial", "polls"));
///
/// let migrations = source.all_migrations().await.unwrap();
/// assert_eq!(migrations.len(), 1);
/// // }
/// # }
/// ```
pub struct TestMigrationSource {
	migrations: Vec<Migration>,
}

impl TestMigrationSource {
	/// Create a new empty TestMigrationSource
	pub fn new() -> Self {
		Self {
			migrations: Vec::new(),
		}
	}

	/// Create a TestMigrationSource with initial migrations
	pub fn with_migrations(migrations: Vec<Migration>) -> Self {
		Self { migrations }
	}

	/// Add a migration to the source
	pub fn add_migration(&mut self, migration: Migration) {
		self.migrations.push(migration);
	}

	/// Clear all migrations from the source
	pub fn clear(&mut self) {
		self.migrations.clear();
	}

	/// Get the number of migrations
	pub fn len(&self) -> usize {
		self.migrations.len()
	}

	/// Check if the source is empty
	pub fn is_empty(&self) -> bool {
		self.migrations.is_empty()
	}
}

impl Default for TestMigrationSource {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl MigrationSource for TestMigrationSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

/// In-memory migration repository for testing
///
/// Provides a simple implementation of `MigrationRepository` that stores migrations
/// in memory using a HashMap. Useful for testing migration persistence without
/// actual file I/O.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::InMemoryRepository;
/// # use reinhardt_db::migrations::{Migration, MigrationRepository};
/// # #[tokio::main]
/// # async fn main() {
/// // #[tokio::test]
/// // async fn test_repository() {
/// let mut repo = InMemoryRepository::new();
///
/// let migration = Migration::new("0001_initial", "polls");
///
/// repo.save(&migration).await.unwrap();
/// let retrieved = repo.get("polls", "0001_initial").await.unwrap();
/// assert_eq!(retrieved.name, "0001_initial");
/// // }
/// # }
/// ```
pub struct InMemoryRepository {
	migrations: HashMap<(String, String), Migration>,
}

impl InMemoryRepository {
	/// Create a new empty InMemoryRepository
	pub fn new() -> Self {
		Self {
			migrations: HashMap::new(),
		}
	}

	/// Create an InMemoryRepository with initial migrations
	pub fn with_migrations(migrations: Vec<Migration>) -> Self {
		let mut repo = Self::new();
		for migration in migrations {
			let key = (migration.app_label.to_string(), migration.name.to_string());
			repo.migrations.insert(key, migration);
		}
		repo
	}

	/// Clear all migrations from the repository
	pub fn clear(&mut self) {
		self.migrations.clear();
	}

	/// Get the number of migrations in the repository
	pub fn len(&self) -> usize {
		self.migrations.len()
	}

	/// Check if the repository is empty
	pub fn is_empty(&self) -> bool {
		self.migrations.is_empty()
	}
}

impl Default for InMemoryRepository {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl MigrationRepository for InMemoryRepository {
	async fn save(&mut self, migration: &Migration) -> Result<()> {
		let key = (migration.app_label.to_string(), migration.name.to_string());
		self.migrations.insert(key, migration.clone());
		Ok(())
	}

	async fn get(&self, app_label: &str, name: &str) -> Result<Migration> {
		let key = (app_label.to_string(), name.to_string());
		self.migrations.get(&key).cloned().ok_or_else(|| {
			reinhardt_db::migrations::MigrationError::NotFound(format!("{}.{}", app_label, name))
		})
	}

	async fn list(&self, app_label: &str) -> Result<Vec<Migration>> {
		Ok(self
			.migrations
			.values()
			.filter(|m| m.app_label == app_label)
			.cloned()
			.collect())
	}

	async fn delete(&mut self, app_label: &str, name: &str) -> Result<()> {
		let key = (app_label.to_string(), name.to_string());
		self.migrations.remove(&key).ok_or_else(|| {
			reinhardt_db::migrations::MigrationError::NotFound(format!("{}.{}", app_label, name))
		})?;
		Ok(())
	}
}

/// Creates a new TestMigrationSource for testing
///
/// Provides an empty migration source that can be populated with test migrations.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::{Migration, MigrationSource};
/// # use rstest::*;
/// # #[tokio::main]
/// # async fn main() {
/// // #[rstest]
/// // #[tokio::test]
/// // async fn test_with_source(mut test_migration_source: TestMigrationSource) {
/// let mut test_migration_source = TestMigrationSource::new();
/// test_migration_source.add_migration(Migration::new("0001_initial", "polls"));
///
/// let migrations = test_migration_source.all_migrations().await.unwrap();
/// assert_eq!(migrations.len(), 1);
/// // }
/// # }
/// ```
#[fixture]
pub fn test_migration_source() -> TestMigrationSource {
	TestMigrationSource::new()
}

/// Creates a new InMemoryRepository for testing
///
/// Provides an empty migration repository that stores migrations in memory.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::{Migration, MigrationRepository};
/// # use rstest::*;
/// # #[tokio::main]
/// # async fn main() {
/// // #[rstest]
/// // #[tokio::test]
/// // async fn test_with_repository(mut in_memory_repository: InMemoryRepository) {
/// let mut in_memory_repository = InMemoryRepository::new();
/// let migration = Migration::new("0001_initial", "polls");
///
/// in_memory_repository.save(&migration).await.unwrap();
/// let retrieved = in_memory_repository.get("polls", "0001_initial").await.unwrap();
/// assert_eq!(retrieved.name, "0001_initial");
/// }
/// ```
#[fixture]
pub fn in_memory_repository() -> InMemoryRepository {
	InMemoryRepository::new()
}

// ============================================================================
// TestContainers-based Migration Executor Fixtures
// ============================================================================

/// Type alias for migration_executor fixture return value
///
/// Contains all elements from postgres_container plus the migration executor:
/// - `DatabaseMigrationExecutor`: Migration executor instance
/// - `ContainerAsync<GenericImage>`: PostgreSQL container
/// - `Arc<PgPool>`: Database connection pool
/// - `u16`: PostgreSQL port
/// - `String`: Database URL
#[cfg(feature = "testcontainers")]
pub type MigrationExecutorFixture = (
	DatabaseMigrationExecutor,
	ContainerAsync<GenericImage>,
	Arc<sqlx::PgPool>,
	u16,
	String,
);

/// Helper for applying database schema migrations in tests with PostgreSQL
///
/// Provides a convenient interface for creating database tables and applying
/// schema operations during test setup. Holds a DatabaseMigrationExecutor
/// and connection pool internally.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
/// # use rstest::*;
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_schema(#[future] postgres_table_creator: PostgresTableCreator) {
///     let mut creator = postgres_table_creator.await;
///
///     let schema = vec![
///         Operation::CreateTable {
///             name: "users".to_string(),
///             columns: vec![
///                 ColumnDefinition::new("id", FieldType::Integer),
///                 ColumnDefinition::new("name", FieldType::Text),
///             ],
///             constraints: vec![],
///             without_rowid: None,
///             interleave_in_parent: None,
///             partition: None,
///         },
///     ];
///
///     creator.apply(schema).await.unwrap();
///
///     let pool = creator.pool();
///     // Run tests using pool...
/// }
/// ```
#[cfg(feature = "testcontainers")]
pub struct PostgresTableCreator {
	executor: DatabaseMigrationExecutor,
	pool: Arc<sqlx::PgPool>,
	container: ContainerAsync<GenericImage>,
	port: u16,
	url: String,
}

#[cfg(feature = "testcontainers")]
impl PostgresTableCreator {
	/// Create a new TableCreator
	pub fn new(
		executor: DatabaseMigrationExecutor,
		container: ContainerAsync<GenericImage>,
		pool: Arc<sqlx::PgPool>,
		port: u16,
		url: String,
	) -> Self {
		Self {
			executor,
			pool,
			container,
			port,
			url,
		}
	}

	/// Apply schema operations by creating and executing a migration
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_test::fixtures::*;
	/// # use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
	/// # async fn example(mut creator: PostgresTableCreator) {
	/// let schema = vec![
	///     Operation::CreateTable {
	///         name: "products".to_string(),
	///         columns: vec![
	///             ColumnDefinition::new("id", FieldType::Integer),
	///             ColumnDefinition::new("name", FieldType::Text),
	///         ],
	///         constraints: vec![],
	///         without_rowid: None,
	///         interleave_in_parent: None,
	///         partition: None,
	///     },
	/// ];
	///
	/// creator.apply(schema).await.unwrap();
	/// # }
	/// ```
	pub async fn apply(&mut self, schema: Vec<Operation>) -> Result<()> {
		let mut migration = Migration::new("0001_test_schema", "testapp");

		for operation in schema {
			migration = migration.add_operation(operation);
		}

		self.executor
			.apply_migrations(&[migration])
			.await
			.expect("Failed to apply test schema migrations");

		Ok(())
	}

	/// Get a reference to the database connection pool
	pub fn pool(&self) -> &Arc<sqlx::PgPool> {
		&self.pool
	}

	/// Get the database URL
	pub fn url(&self) -> &str {
		&self.url
	}

	/// Get the database port
	pub fn port(&self) -> u16 {
		self.port
	}

	/// Get a reference to the container
	///
	/// This is useful for advanced test scenarios that need direct container access.
	pub fn container(&self) -> &ContainerAsync<GenericImage> {
		&self.container
	}

	/// Insert data into a table using reinhardt-query
	///
	/// This method provides a convenient way to insert test data using type-safe
	/// reinhardt-query builders instead of raw SQL strings.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_test::fixtures::*;
	/// # use reinhardt_query::prelude::Value;
	/// # async fn example(creator: &PostgresTableCreator) {
	/// creator.insert_data(
	///     "users",
	///     vec!["id", "name", "email"],
	///     vec![
	///         vec![
	///             Value::Int(Some(1)),
	///             Value::String(Some(Box::new("Alice".to_string()))),
	///             Value::String(Some(Box::new("alice@example.com".to_string()))),
	///         ],
	///     ],
	/// ).await.unwrap();
	/// # }
	/// ```
	pub async fn insert_data(
		&self,
		table: &str,
		columns: Vec<&str>,
		values: Vec<Vec<reinhardt_query::prelude::Value>>,
	) -> Result<()> {
		use reinhardt_query::prelude::{Alias, PostgresQueryBuilder, Query, QueryStatementBuilder};

		for row_values in values {
			let mut query = Query::insert();
			query
				.into_table(Alias::new(table))
				.columns(columns.iter().map(|&c| Alias::new(c)));

			query.values_panic(row_values);

			// Build SQL string
			let sql = query.to_string(PostgresQueryBuilder::new());

			sqlx::query(&sql)
				.execute(self.pool.as_ref())
				.await
				.map_err(MigrationError::SqlError)?;
		}
		Ok(())
	}

	/// Execute custom SQL (fallback for complex cases)
	///
	/// This method allows executing arbitrary SQL statements when reinhardt-query
	/// is insufficient for complex schema operations or PostgreSQL-specific features.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_test::fixtures::*;
	/// # async fn example(creator: &PostgresTableCreator) {
	/// // PostgreSQL-specific feature
	/// creator.execute_sql(
	///     "CREATE TABLE test (id SERIAL PRIMARY KEY) WITH (autovacuum_enabled = false)"
	/// ).await.unwrap();
	/// # }
	/// ```
	pub async fn execute_sql(&self, sql: &str) -> Result<sqlx::postgres::PgQueryResult> {
		sqlx::query(sql)
			.execute(self.pool.as_ref())
			.await
			.map_err(MigrationError::SqlError)
	}

	/// Begin a transaction for advanced test scenarios
	///
	/// This is useful for testing two-phase commit (2PC) or other transaction-based
	/// operations.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// # use reinhardt_test::fixtures::*;
	/// # async fn example(creator: &PostgresTableCreator) {
	/// let mut tx = creator.begin_transaction().await.unwrap();
	/// // Perform transactional operations...
	/// tx.commit().await.unwrap();
	/// # }
	/// ```
	pub async fn begin_transaction(&self) -> Result<sqlx::Transaction<'_, sqlx::Postgres>> {
		self.pool.begin().await.map_err(MigrationError::SqlError)
	}
}

// ============================================================================
// AdminTableCreator - Integration with AdminDatabase
// ============================================================================

/// Helper for applying database schema migrations in AdminDatabase tests
///
/// This structure combines PostgresTableCreator with AdminDatabase, providing
/// a convenient interface for tests that need both schema management and
/// admin panel functionality.
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
/// # use rstest::*;
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_admin(#[future] admin_table_creator: AdminTableCreator) {
///     let mut creator = admin_table_creator.await;
///
///     // Create schema
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
///     // Use admin database
///     let db = creator.admin_db();
///     // Perform admin operations...
/// }
/// ```
#[cfg(all(feature = "admin", feature = "testcontainers"))]
pub struct AdminTableCreator {
	postgres_creator: PostgresTableCreator,
	admin_db: Arc<reinhardt_admin::core::AdminDatabase>,
}

#[cfg(all(feature = "admin", feature = "testcontainers"))]
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
	pub fn container(&self) -> &ContainerAsync<GenericImage> {
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

// ============================================================================
// Migration Executor and Table Creator Fixtures
// ============================================================================

/// Creates a DatabaseMigrationExecutor connected to a test PostgreSQL container
///
/// This fixture combines postgres_container with migration executor creation,
/// providing a ready-to-use migration executor for tests.
///
/// # Type Signature
///
/// Returns `MigrationExecutorFixture`:
/// - `DatabaseMigrationExecutor`: Migration executor instance
/// - `ContainerAsync<GenericImage>`: PostgreSQL container
/// - `Arc<PgPool>`: Database connection pool
/// - `u16`: PostgreSQL port
/// - `String`: Database URL
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::Migration;
/// # use rstest::*;
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_executor(
///     #[future] migration_executor: MigrationExecutorFixture
/// ) {
///     let (mut executor, _container, _pool, _port, _url) = migration_executor.await;
///
///     let migration = Migration::new("0001_test", "testapp");
///     executor.apply_migrations(&[migration]).await.unwrap();
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[fixture]
pub async fn migration_executor(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<sqlx::PgPool>, u16, String),
) -> MigrationExecutorFixture {
	let (container, pool, port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to test database");

	let executor = DatabaseMigrationExecutor::new(connection);

	(executor, container, pool, port, url)
}

/// Creates a PostgresTableCreator for applying test schemas
///
/// This fixture combines migration_executor with a convenient helper structure
/// for applying database schema operations in PostgreSQL tests.
///
/// # Type Signature
///
/// Returns `` `PostgresTableCreator` ``:
/// - Helper with methods to apply schema operations
/// - Provides access to connection pool, URL, and port
/// - Includes methods for data insertion and custom SQL execution
///
/// # Examples
///
/// ```rust,no_run
/// # use reinhardt_test::fixtures::*;
/// # use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
/// # use rstest::*;
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_creator(#[future] postgres_table_creator: PostgresTableCreator) {
///     let mut creator = postgres_table_creator.await;
///
///     let schema = vec![
///         Operation::CreateTable {
///             name: "test_table".to_string(),
///             columns: vec![
///                 ColumnDefinition::new("id", FieldType::Integer),
///             ],
///             constraints: vec![],
///             without_rowid: None,
///             interleave_in_parent: None,
///             partition: None,
///         },
///     ];
///
///     creator.apply(schema).await.unwrap();
///     let pool = creator.pool();
///     // Use pool for testing...
/// }
/// ```
#[cfg(feature = "testcontainers")]
#[fixture]
pub async fn postgres_table_creator(
	#[future] migration_executor: MigrationExecutorFixture,
) -> PostgresTableCreator {
	let (executor, container, pool, port, url) = migration_executor.await;
	PostgresTableCreator::new(executor, container, pool, port, url)
}

/// Creates an AdminTableCreator for applying test schemas in admin panel tests
///
/// This fixture combines PostgresTableCreator with AdminDatabase, providing
/// a convenient helper for tests that need both schema management and admin
/// panel functionality.
///
/// # Type Signature
///
/// Returns `` `AdminTableCreator` ``:
/// - Helper with methods to apply schema operations
/// - Provides access to AdminDatabase for admin panel testing
/// - Includes all PostgresTableCreator methods (insert_data, execute_sql, etc.)
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
///     // Create schema
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
///     // Use admin database
///     let db = creator.admin_db();
///     let results = db.list::<AdminRecord>("test_models", vec![], 0, 10).await.unwrap();
/// }
/// ```
#[cfg(all(feature = "admin", feature = "testcontainers"))]
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

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_db::migrations::Migration;
	use reinhardt_db::migrations::registry::MigrationRegistry;

	#[rstest]
	fn test_migration_registry_fixture(migration_registry: LocalRegistry) {
		assert!(migration_registry.all_migrations().is_empty());
	}

	#[rstest]
	fn test_registry_isolation_between_tests(migration_registry: LocalRegistry) {
		// This test runs independently - registry should be empty
		assert_eq!(migration_registry.all_migrations().len(), 0);

		migration_registry
			.register(Migration::new("0001_initial", "test_app"))
			.unwrap();

		assert_eq!(migration_registry.all_migrations().len(), 1);
	}

	#[rstest]
	fn test_another_isolated_test(migration_registry: LocalRegistry) {
		// Even though previous test registered a migration,
		// this new fixture instance should be empty
		assert_eq!(migration_registry.all_migrations().len(), 0);
	}

	#[cfg(feature = "testcontainers")]
	mod testcontainer_fixtures {
		use super::*;
		use reinhardt_db::migrations::{ColumnDefinition, DatabaseType, FieldType, Operation};

		#[rstest]
		#[tokio::test]
		async fn test_migration_executor_fixture(
			#[future] migration_executor: MigrationExecutorFixture,
		) {
			let (executor, _container, _pool, _port, _url) = migration_executor.await;

			// Verify executor is connected to PostgreSQL
			assert_eq!(executor.database_type(), DatabaseType::Postgres);
		}

		#[rstest]
		#[tokio::test]
		async fn test_postgres_table_creator_fixture(
			#[future] postgres_table_creator: PostgresTableCreator,
		) {
			let mut creator = postgres_table_creator.await;

			// Define schema directly in test
			let schema = vec![Operation::CreateTable {
				name: "fixture_test_table".to_string(),
				columns: vec![
					ColumnDefinition::new("id", FieldType::Integer),
					ColumnDefinition::new("value", FieldType::Text),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			}];

			// Apply schema
			creator.apply(schema).await.unwrap();

			// Verify table was created
			let pool = creator.pool();
			let result = sqlx::query("SELECT * FROM fixture_test_table")
				.fetch_all(pool.as_ref())
				.await
				.unwrap();

			assert!(result.is_empty());
		}
	}
}
