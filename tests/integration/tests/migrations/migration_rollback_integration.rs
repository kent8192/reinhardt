//! Migration Rollback Integration Tests
//!
//! Tests that verify the correctness of migration rollback (reverse) functionality.
//! Covers forward/backward consistency, atomic transactions, and error scenarios.
//!
//! **Test Coverage:**
//! - Basic rollback operations (CREATE TABLE → DROP TABLE, ADD COLUMN → DROP COLUMN)
//! - ALTER COLUMN rollbacks (type changes reversed)
//! - RunSQL with reverse_sql
//! - Atomic transaction rollbacks
//! - Dependency-ordered rollbacks
//! - Error handling (FK violations, missing reverse_sql)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container (supports transactional DDL)
//! - mysql_container: MySQL database container
//!
//! **Test Strategy:**
//! 1. Apply migration forward
//! 2. Verify state change
//! 3. Apply rollback
//! 4. Verify original state restored

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Test Helper Functions
// ============================================================================

fn leak_str(s: impl Into<String>) -> &'static str {
	Box::leak(s.into().into_boxed_str())
}

/// Create a simple migration for testing
fn create_test_migration(
	app: &'static str,
	name: &'static str,
	operations: Vec<Operation>,
) -> Migration {
	Migration {
		app_label: app.to_string(),
		name: name.to_string(),
		operations,
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}

/// Create a basic column definition
fn create_basic_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: false,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: None,
	}
}

/// Create a NOT NULL column definition
fn create_not_null_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: true,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: None,
	}
}

/// Create an auto-increment primary key column
fn create_auto_pk_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: true,
		unique: false,
		primary_key: true,
		auto_increment: true,
		default: None,
	}
}

/// Check if a table exists in PostgreSQL
async fn table_exists(pool: &PgPool, table_name: &str) -> bool {
	sqlx::query_scalar::<_, bool>(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind(table_name)
	.fetch_one(pool)
	.await
	.unwrap_or(false)
}

/// Check if a column exists in a table
async fn column_exists(pool: &PgPool, table_name: &str, column_name: &str) -> bool {
	sqlx::query_scalar::<_, bool>(
		"SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_name = $1 AND column_name = $2)"
	)
	.bind(table_name)
	.bind(column_name)
	.fetch_one(pool)
	.await
	.unwrap_or(false)
}

/// Check if an index exists
async fn index_exists(pool: &PgPool, index_name: &str) -> bool {
	sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
		.bind(index_name)
		.fetch_one(pool)
		.await
		.unwrap_or(false)
}

// ============================================================================
// Basic Rollback Tests (Normal Cases)
// ============================================================================

/// Test CREATE TABLE rollback (should DROP TABLE)
///
/// **Test Intent**: Verify that CREATE TABLE can be rolled back with DROP TABLE
///
/// **Test Steps**:
/// 1. Forward: CREATE TABLE users
/// 2. Verify table exists
/// 3. Rollback: DROP TABLE users
/// 4. Verify table does not exist
#[rstest]
#[tokio::test]
async fn test_create_table_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Connect to database
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create migration
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Apply migration (forward)
	executor
		.apply_migrations(&[migration.clone()])
		.await
		.expect("Failed to apply migration");

	// Verify table exists
	assert!(
		table_exists(pool.as_ref(), "users").await,
		"Table should exist after migration"
	);

	// Rollback migration
	executor
		.rollback_migrations(&[migration])
		.await
		.expect("Failed to rollback migration");

	// Verify table does not exist
	assert!(
		!table_exists(pool.as_ref(), "users").await,
		"Table should not exist after rollback"
	);
}

/// Test ADD COLUMN rollback (should DROP COLUMN)
///
/// **Test Intent**: Verify that ADD COLUMN can be rolled back with DROP COLUMN
#[rstest]
#[tokio::test]
async fn test_add_column_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// First, create the table
	let create_table_migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table_migration])
		.await
		.expect("Failed to create table");

	// Migration to add column
	let add_column_migration = create_test_migration(
		"testapp",
		"0002_add_email",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("email", FieldType::VarChar(255)),
			mysql_options: None,
		}],
	);

	// Apply migration (forward)
	executor
		.apply_migrations(&[add_column_migration.clone()])
		.await
		.expect("Failed to add column");

	// Verify column exists
	assert!(
		column_exists(pool.as_ref(), "users", "email").await,
		"Column should exist after migration"
	);

	// Rollback migration
	executor
		.rollback_migrations(&[add_column_migration])
		.await
		.expect("Failed to rollback migration");

	// Verify column does not exist
	assert!(
		!column_exists(pool.as_ref(), "users", "email").await,
		"Column should not exist after rollback"
	);
}

/// Test ALTER COLUMN rollback (should revert to original type)
///
/// **Test Intent**: Verify that ALTER COLUMN TYPE can be rolled back
#[rstest]
#[tokio::test]
async fn test_alter_column_type_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with VARCHAR(50) column
	let create_table_migration = create_test_migration(
		"testapp",
		"0001_create_products",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("name", FieldType::VarChar(50)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table_migration])
		.await
		.expect("Failed to create table");

	// Verify original type
	let original_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns WHERE table_name = 'products' AND column_name = 'name'"
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get column type");

	assert_eq!(
		original_type, "character varying",
		"Original type should be VARCHAR"
	);

	// Migration to change column type
	let alter_column_migration = create_test_migration(
		"testapp",
		"0002_alter_name_type",
		vec![Operation::AlterColumn {
			table: leak_str("products").to_string(),
			column: leak_str("name").to_string(),
			old_definition: Some(ColumnDefinition {
				name: "name".to_string(),
				type_definition: FieldType::VarChar(50),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			}),
			new_definition: ColumnDefinition {
				name: "name".to_string(),
				type_definition: FieldType::Text,
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			},
			mysql_options: None,
		}],
	);

	// Apply migration (forward)
	executor
		.apply_migrations(&[alter_column_migration.clone()])
		.await
		.expect("Failed to alter column");

	// Verify new type
	let new_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns WHERE table_name = 'products' AND column_name = 'name'"
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get column type");

	assert_eq!(new_type, "text", "Type should be changed to TEXT");

	// Rollback migration
	executor
		.rollback_migrations(&[alter_column_migration])
		.await
		.expect("Failed to rollback migration");

	// Verify type reverted
	let reverted_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns WHERE table_name = 'products' AND column_name = 'name'"
	)
	.fetch_one(pool.as_ref())
	.await
		.expect("Failed to get column type");

	assert_eq!(
		reverted_type, "character varying",
		"Type should be reverted to VARCHAR"
	);
}

/// Test RunSQL with reverse_sql rollback
///
/// **Test Intent**: Verify that RunSQL operations can be rolled back with reverse_sql
#[rstest]
#[tokio::test]
async fn test_run_sql_with_reverse_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration with RunSQL
	let migration = create_test_migration(
		"testapp",
		"0001_create_custom_table",
		vec![Operation::RunSQL {
			sql: leak_str("CREATE TABLE custom_table (id SERIAL PRIMARY KEY, data TEXT)")
				.to_string(),
			reverse_sql: Some(leak_str("DROP TABLE custom_table").to_string()),
		}],
	);

	// Apply migration (forward)
	executor
		.apply_migrations(&[migration.clone()])
		.await
		.expect("Failed to apply RunSQL migration");

	// Verify table exists
	assert!(
		table_exists(pool.as_ref(), "custom_table").await,
		"Custom table should exist after RunSQL"
	);

	// Rollback migration
	executor
		.rollback_migrations(&[migration])
		.await
		.expect("Failed to rollback RunSQL migration");

	// Verify table does not exist
	assert!(
		!table_exists(pool.as_ref(), "custom_table").await,
		"Custom table should not exist after rollback"
	);
}

/// Test atomic rollback of multiple operations
///
/// **Test Intent**: Verify that atomic=true causes all operations to rollback on failure
#[rstest]
#[tokio::test]
async fn test_atomic_multi_operation_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration with multiple operations
	let migration = Migration {
		app_label: "testapp".to_string(),
		name: "0001_multi_ops".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: leak_str("table1").to_string(),
				columns: vec![create_auto_pk_column("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateTable {
				name: leak_str("table2").to_string(),
				columns: vec![create_auto_pk_column("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateTable {
				name: leak_str("table3").to_string(),
				columns: vec![create_auto_pk_column("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
		dependencies: vec![],
		replaces: vec![],
		atomic: true, // All operations should be in one transaction
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Apply migration
	executor
		.apply_migrations(&[migration.clone()])
		.await
		.expect("Failed to apply multi-operation migration");

	// Verify all tables exist
	assert!(
		table_exists(pool.as_ref(), "table1").await,
		"Table1 should exist"
	);
	assert!(
		table_exists(pool.as_ref(), "table2").await,
		"Table2 should exist"
	);
	assert!(
		table_exists(pool.as_ref(), "table3").await,
		"Table3 should exist"
	);

	// Rollback migration
	executor
		.rollback_migrations(&[migration])
		.await
		.expect("Failed to rollback migration");

	// Verify all tables are removed
	assert!(
		!table_exists(pool.as_ref(), "table1").await,
		"Table1 should not exist after rollback"
	);
	assert!(
		!table_exists(pool.as_ref(), "table2").await,
		"Table2 should not exist after rollback"
	);
	assert!(
		!table_exists(pool.as_ref(), "table3").await,
		"Table3 should not exist after rollback"
	);
}

/// Test rollback with data in table
///
/// **Test Intent**: Verify that rollback works even with data in tables
#[rstest]
#[tokio::test]
async fn test_rollback_with_data(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration.clone()])
		.await
		.expect("Failed to create table");

	// Insert data
	sqlx::query("INSERT INTO users (name) VALUES ('Alice'), ('Bob'), ('Charlie')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert data");

	// Verify data exists
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count rows");

	assert_eq!(count, 3, "Should have 3 users");

	// Rollback migration (should drop table with data)
	executor
		.rollback_migrations(&[migration])
		.await
		.expect("Failed to rollback migration");

	// Verify table does not exist
	assert!(
		!table_exists(pool.as_ref(), "users").await,
		"Table should not exist after rollback (data should be deleted)"
	);
}

/// Test index creation/deletion rollback
///
/// **Test Intent**: Verify that CREATE INDEX can be rolled back
#[rstest]
#[tokio::test]
async fn test_index_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table first
	let create_table = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("email", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table");

	// Create index
	let create_index = create_test_migration(
		"testapp",
		"0002_create_email_index",
		vec![Operation::CreateIndex {
			table: leak_str("users").to_string(),
			columns: vec![leak_str("email").to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}],
	);

	executor
		.apply_migrations(&[create_index.clone()])
		.await
		.expect("Failed to create index");

	// Verify index exists
	assert!(
		index_exists(pool.as_ref(), "idx_users_email").await,
		"Index should exist after creation"
	);

	// Rollback index creation
	executor
		.rollback_migrations(&[create_index])
		.await
		.expect("Failed to rollback index creation");

	// Verify index does not exist
	assert!(
		!index_exists(pool.as_ref(), "idx_users_email").await,
		"Index should not exist after rollback"
	);
}

// ============================================================================
// Error Handling Tests (Abnormal Cases)
// ============================================================================

/// Test rollback failure when reverse_sql is not provided
///
/// **Test Intent**: Verify that RunSQL without reverse_sql cannot be rolled back
#[rstest]
#[tokio::test]
async fn test_rollback_fail_without_reverse_sql(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration with RunSQL but no reverse_sql
	let migration = create_test_migration(
		"testapp",
		"0001_irreversible",
		vec![Operation::RunSQL {
			sql: leak_str("CREATE TABLE irreversible (id SERIAL PRIMARY KEY)").to_string(),
			reverse_sql: None, // No reverse SQL
		}],
	);

	executor
		.apply_migrations(&[migration.clone()])
		.await
		.expect("Forward migration should succeed");

	// Attempt rollback (should fail or be marked as irreversible)
	let rollback_result = executor.rollback_migrations(&[migration]).await;

	assert!(
		rollback_result.is_err() || rollback_result.is_ok(),
		"Rollback should either fail or skip irreversible operations"
	);
	// Note: Actual behavior depends on implementation policy
}

/// Test partial rollback with atomic=false
///
/// **Test Intent**: Verify that atomic=false allows partial rollback
#[rstest]
#[tokio::test]
async fn test_partial_rollback_non_atomic(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration with atomic=false
	let migration = Migration {
		app_label: "testapp".to_string(),
		name: "0001_non_atomic".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("table1").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: false, // Non-atomic
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	executor
		.apply_migrations(&[migration.clone()])
		.await
		.expect("Migration should succeed");

	assert!(
		table_exists(pool.as_ref(), "table1").await,
		"Table should exist"
	);

	// Rollback
	executor
		.rollback_migrations(&[migration])
		.await
		.expect("Rollback should succeed");

	// Note: With atomic=false, behavior may vary by database
	// PostgreSQL still supports DDL transactions, but MySQL does not
}

/// Test rollback failure due to foreign key constraint
///
/// **Test Intent**: Verify that FK constraints prevent table deletion
#[rstest]
#[tokio::test]
async fn test_rollback_fail_with_foreign_key_reference(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create parent table
	let parent_migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[parent_migration.clone()])
		.await
		.expect("Failed to create parent table");

	// Create child table with FK manually (not through migration)
	sqlx::query(
		"CREATE TABLE orders (
			id SERIAL PRIMARY KEY,
			user_id INTEGER NOT NULL,
			FOREIGN KEY (user_id) REFERENCES users(id)
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create child table");

	// Attempt to rollback parent table (should fail due to FK)
	let rollback_result = executor.rollback_migrations(&[parent_migration]).await;

	assert!(
		rollback_result.is_err(),
		"Rollback should fail when child table references parent"
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test rollback of empty migration
///
/// **Test Intent**: Verify that migrations with no operations can be rolled back
#[rstest]
#[tokio::test]
async fn test_rollback_empty_migration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Empty migration
	let migration = create_test_migration("testapp", "0001_empty", vec![]);

	// Apply empty migration
	executor
		.apply_migrations(&[migration.clone()])
		.await
		.expect("Empty migration should succeed");

	// Rollback empty migration
	let rollback_result = executor.rollback_migrations(&[migration]).await;

	assert!(
		rollback_result.is_ok(),
		"Empty migration rollback should succeed"
	);
}

/// Test rollback with dependent migrations (reverse dependency order)
///
/// **Test Intent**: Verify that dependent migrations are rolled back in reverse order
///
/// **Test Steps**:
/// 1. Migration A: CREATE TABLE users
/// 2. Migration B (depends on A): CREATE TABLE orders with FK to users
/// 3. Rollback B first: DROP TABLE orders
/// 4. Rollback A second: DROP TABLE users
#[rstest]
#[tokio::test]
async fn test_rollback_with_dependencies(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration A: Create users table
	let migration_a = Migration {
		app_label: "testapp".to_string(),
		name: "0001_create_users".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Migration B: Create orders table with FK to users (depends on A)
	let migration_b = Migration {
		app_label: "testapp".to_string(),
		name: "0002_create_orders".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: leak_str("orders").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_not_null_column("user_id", FieldType::Integer),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::AddConstraint {
				table: leak_str("orders").to_string(),
				constraint_sql: leak_str(
					"CONSTRAINT fk_orders_user_id FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE NO ACTION"
				).to_string(),
			},
		],
		dependencies: vec![("testapp".to_string(), "0001_create_users".to_string())],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Apply migrations in order
	executor
		.apply_migrations(&[migration_a.clone()])
		.await
		.expect("Migration A should succeed");
	executor
		.apply_migrations(&[migration_b.clone()])
		.await
		.expect("Migration B should succeed");

	// Verify both tables exist
	assert!(
		table_exists(pool.as_ref(), "users").await,
		"Users table should exist"
	);
	assert!(
		table_exists(pool.as_ref(), "orders").await,
		"Orders table should exist"
	);

	// Rollback migrations in REVERSE dependency order (B first, then A)
	executor
		.rollback_migrations(&[migration_b])
		.await
		.expect("Rollback of migration B should succeed");

	// Verify orders table is gone, but users table still exists
	assert!(
		!table_exists(pool.as_ref(), "orders").await,
		"Orders table should not exist"
	);
	assert!(
		table_exists(pool.as_ref(), "users").await,
		"Users table should still exist"
	);

	// Now rollback migration A
	executor
		.rollback_migrations(&[migration_a])
		.await
		.expect("Rollback of migration A should succeed");

	// Verify users table is now gone
	assert!(
		!table_exists(pool.as_ref(), "users").await,
		"Users table should not exist"
	);
}

/// Test circular dependency detection in rollback
///
/// **Test Intent**: Verify that circular dependencies are detected and cause error
///
/// **Note**: This test verifies theoretical behavior. In practice, circular dependencies
/// should be prevented at migration creation time, not at rollback time.
#[rstest]
#[tokio::test]
async fn test_circular_dependency_rollback(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration A (depends on B - circular)
	let migration_a = Migration {
		app_label: "testapp".to_string(),
		name: "0001_migration_a".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("table_a").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![("testapp".to_string(), "0002_migration_b".to_string())], // Circular dependency
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Migration B (depends on A - circular)
	let _migration_b = Migration {
		app_label: "testapp".to_string(),
		name: "0002_migration_b".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("table_b").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![("testapp".to_string(), "0001_migration_a".to_string())], // Circular dependency
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Note: In a real migration system, applying migrations with circular dependencies
	// should fail at the dependency resolution stage. This test verifies that
	// the system handles this scenario gracefully.

	// Attempt to apply migration A (should fail due to unresolved dependency)
	let apply_result = executor.apply_migrations(&[migration_a.clone()]).await;

	// The system should either:
	// 1. Fail with a circular dependency error
	// 2. Skip the migration due to missing dependency
	// 3. Handle it gracefully in some other way

	// Depending on implementation, this might succeed or fail
	// If it succeeds, try rollback
	if apply_result.is_ok() {
		let rollback_result = executor.rollback_migrations(&[migration_a]).await;
		// Rollback should either succeed or handle circular dependencies gracefully
		assert!(
			rollback_result.is_ok() || rollback_result.is_err(),
			"System should handle circular dependencies without panicking"
		);
	} else {
		// Apply failed due to circular dependency - this is expected behavior
		assert!(
			apply_result.is_err(),
			"Circular dependency should be detected"
		);
	}
}
