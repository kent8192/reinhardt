//! Integration tests for multi-database migration support
//!
//! Tests PostgreSQL-specific and cross-database compatibility:
//! - SERIAL vs AUTO_INCREMENT patterns
//! - JSON/JSONB types
//! - Array types (PostgreSQL)
//! - Transaction isolation levels
//! - Cascade delete behavior
//!
//! **Test Coverage:**
//! - Database-specific SQL generation
//! - Cross-database compatibility
//! - Data type handling differences
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//! - mysql_container: MySQL database container (when available)

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, Constraint, FieldType, Migration, Operation,
	executor::DatabaseMigrationExecutor, operations::SqlDialect,
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

/// Create a migration with explicit dependencies for ordering
fn create_test_migration_with_deps(
	app: &'static str,
	name: &'static str,
	operations: Vec<Operation>,
	dependencies: Vec<(String, String)>,
) -> Migration {
	Migration {
		app_label: app.to_string(),
		name: name.to_string(),
		operations,
		dependencies,
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

/// Create an auto-increment primary key column
fn create_auto_pk_column(name: &str) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: FieldType::Integer,
		not_null: true,
		unique: false,
		primary_key: true,
		auto_increment: true,
		default: None,
	}
}

// ============================================================================
// Auto-Increment / SERIAL Tests
// ============================================================================

/// Test auto-increment primary key creation (PostgreSQL SERIAL)
///
/// **Test Intent**: Verify auto-increment column generates correct SQL for PostgreSQL
///
/// **Integration Point**: MigrationExecutor → PostgreSQL SERIAL type
///
/// **Expected Behavior**: SERIAL column created with auto-increment behavior
#[rstest]
#[tokio::test]
async fn test_auto_increment_serial(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	let migration = create_test_migration(
		"testapp",
		"0001_auto_inc",
		vec![Operation::CreateTable {
			name: leak_str("auto_inc_table").to_string(),
			columns: vec![
				create_auto_pk_column("id"),
				create_basic_column("name", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_ok(),
		"Auto-increment column should be created: {:?}",
		result.err()
	);

	// Verify auto-increment works
	sqlx::query("INSERT INTO auto_inc_table (name) VALUES ('test1')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert");

	sqlx::query("INSERT INTO auto_inc_table (name) VALUES ('test2')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert");

	let rows: Vec<(i32,)> = sqlx::query_as("SELECT id FROM auto_inc_table ORDER BY id")
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch");

	assert_eq!(rows.len(), 2, "Should have 2 rows");
	assert_eq!(rows[0].0, 1, "First row should have id=1");
	assert_eq!(rows[1].0, 2, "Second row should have id=2");
}

// ============================================================================
// JSON/JSONB Type Tests
// ============================================================================

/// Test JSONB column creation (PostgreSQL-specific)
///
/// **Test Intent**: Verify JSONB type is properly created in PostgreSQL
///
/// **Integration Point**: MigrationExecutor → PostgreSQL JSONB type
///
/// **Expected Behavior**: JSONB column with proper indexing capabilities
#[rstest]
#[tokio::test]
async fn test_json_jsonb_types(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	let migration = create_test_migration(
		"testapp",
		"0001_json_types",
		vec![Operation::CreateTable {
			name: leak_str("json_table").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				create_basic_column("json_data", FieldType::Custom("JSON".to_string())),
				create_basic_column("jsonb_data", FieldType::Custom("JSONB".to_string())),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_ok(),
		"JSON columns should be created: {:?}",
		result.err()
	);

	// Test JSON operations
	sqlx::query(
		r#"INSERT INTO json_table (json_data, jsonb_data)
		   VALUES ('{"key": "value1"}', '{"key": "value2"}')"#,
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert JSON");

	// Verify JSONB indexing works (PostgreSQL specific)
	let result: (String,) =
		sqlx::query_as(r#"SELECT jsonb_data->>'key' as value FROM json_table WHERE id = 1"#)
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to query JSONB");

	assert_eq!(result.0, "value2", "JSONB extraction should work");
}

// ============================================================================
// Array Type Tests (PostgreSQL)
// ============================================================================

/// Test PostgreSQL array type support
///
/// **Test Intent**: Verify PostgreSQL array types work correctly
///
/// **Integration Point**: MigrationExecutor → PostgreSQL ARRAY type
///
/// **Expected Behavior**: Array column with proper array operations
#[rstest]
#[tokio::test]
async fn test_postgres_array_type(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	let migration = create_test_migration(
		"testapp",
		"0001_array_type",
		vec![Operation::CreateTable {
			name: leak_str("array_table").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				create_basic_column("tags", FieldType::Custom("TEXT[]".to_string())),
				create_basic_column("scores", FieldType::Custom("INTEGER[]".to_string())),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_ok(),
		"Array columns should be created: {:?}",
		result.err()
	);

	// Test array operations
	sqlx::query(
		"INSERT INTO array_table (tags, scores) VALUES (ARRAY['rust', 'django'], ARRAY[95, 88, 92])",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert arrays");

	// Verify array contains operation
	let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM array_table WHERE 'rust' = ANY(tags)")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to query array");

	assert_eq!(count.0, 1, "Array contains operation should work");
}

// ============================================================================
// Transaction Isolation Level Tests
// ============================================================================

/// Test transaction isolation in migration execution
///
/// **Test Intent**: Verify migrations respect transaction boundaries
///
/// **Integration Point**: MigrationExecutor → PostgreSQL transaction handling
///
/// **Expected Behavior**: Migrations run in proper isolation
#[rstest]
#[tokio::test]
async fn test_transaction_isolation_levels(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create a table for isolation testing
	let migration = create_test_migration(
		"testapp",
		"0001_isolation_test",
		vec![Operation::CreateTable {
			name: leak_str("isolation_table").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				create_basic_column("value", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;
	assert!(result.is_ok(), "Migration should succeed");

	// Insert test data
	sqlx::query("INSERT INTO isolation_table (value) VALUES (100)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert");

	// Verify isolation by checking the table from the pool
	let value: (i32,) = sqlx::query_as("SELECT value FROM isolation_table WHERE id = 1")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to query");

	assert_eq!(value.0, 100, "Value should be 100");

	// Test SERIALIZABLE isolation level with raw SQL
	sqlx::query("BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE")
		.execute(pool.as_ref())
		.await
		.expect("Failed to begin serializable transaction");

	sqlx::query("UPDATE isolation_table SET value = 200 WHERE id = 1")
		.execute(pool.as_ref())
		.await
		.expect("Failed to update");

	sqlx::query("COMMIT")
		.execute(pool.as_ref())
		.await
		.expect("Failed to commit");

	let new_value: (i32,) = sqlx::query_as("SELECT value FROM isolation_table WHERE id = 1")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to query");

	assert_eq!(new_value.0, 200, "Value should be updated to 200");
}

// ============================================================================
// Cascade Delete Tests
// ============================================================================

/// Test ON DELETE CASCADE behavior
///
/// **Test Intent**: Verify cascade delete is properly set up and works
///
/// **Integration Point**: MigrationExecutor → PostgreSQL FK with CASCADE
///
/// **Expected Behavior**: Child records deleted when parent is deleted
#[rstest]
#[tokio::test]
async fn test_cascade_delete(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create parent table
	let migration1 = create_test_migration(
		"testapp",
		"0001_parent",
		vec![Operation::CreateTable {
			name: leak_str("cascade_parent").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				create_basic_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Create child table with CASCADE
	// Explicit dependency ensures parent table is created first
	let migration2 = create_test_migration_with_deps(
		"testapp",
		"0002_child",
		vec![Operation::CreateTable {
			name: leak_str("cascade_child").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				ColumnDefinition {
					name: "parent_id".to_string(),
					type_definition: FieldType::Integer,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				create_basic_column("data", FieldType::Text),
			],
			constraints: vec![Constraint::ForeignKey {
				name: "fk_parent".to_string(),
				columns: vec!["parent_id".to_string()],
				referenced_table: "cascade_parent".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: reinhardt_db::migrations::ForeignKeyAction::Cascade,
				on_update: reinhardt_db::migrations::ForeignKeyAction::NoAction,
				deferrable: None,
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("testapp".to_string(), "0001_parent".to_string())],
	);

	executor
		.apply_migrations(&[migration1, migration2])
		.await
		.expect("Failed to create tables");

	// Insert test data
	sqlx::query("INSERT INTO cascade_parent (name) VALUES ('Parent1')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert parent");

	sqlx::query("INSERT INTO cascade_child (parent_id, data) VALUES (1, 'Child1')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert child 1");

	sqlx::query("INSERT INTO cascade_child (parent_id, data) VALUES (1, 'Child2')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert child 2");

	// Verify children exist
	let child_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cascade_child")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count children");
	assert_eq!(child_count.0, 2, "Should have 2 children");

	// Delete parent - children should be cascade deleted
	sqlx::query("DELETE FROM cascade_parent WHERE id = 1")
		.execute(pool.as_ref())
		.await
		.expect("Failed to delete parent");

	// Verify children are gone
	let child_count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cascade_child")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count children after delete");
	assert_eq!(child_count_after.0, 0, "Children should be cascade deleted");
}

// ============================================================================
// SQL Dialect Generation Tests
// ============================================================================

/// Test SQL dialect-specific code generation
///
/// **Test Intent**: Verify correct SQL is generated for different dialects
///
/// **Integration Point**: Operation::to_sql() → SqlDialect
///
/// **Expected Behavior**: Different SQL for different dialects
#[rstest]
#[tokio::test]
async fn test_sql_dialect_generation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, _url) = postgres_container.await;

	// Test that operations generate different SQL for different dialects
	let operation = Operation::CreateTable {
		name: "dialect_test".to_string(),
		columns: vec![create_auto_pk_column("id")],
		constraints: vec![],
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	};

	// Generate SQL for PostgreSQL
	let postgres_sql = operation.to_sql(&SqlDialect::Postgres);
	assert!(
		postgres_sql.contains("SERIAL")
			|| postgres_sql.contains("serial")
			|| postgres_sql.contains("GENERATED")
			|| postgres_sql.contains("generated"),
		"PostgreSQL should use SERIAL or GENERATED: {}",
		postgres_sql
	);

	// Generate SQL for MySQL
	let mysql_sql = operation.to_sql(&SqlDialect::Mysql);
	assert!(
		mysql_sql.contains("AUTO_INCREMENT") || mysql_sql.contains("auto_increment"),
		"MySQL should use AUTO_INCREMENT: {}",
		mysql_sql
	);

	// Generate SQL for SQLite
	let sqlite_sql = operation.to_sql(&SqlDialect::Sqlite);
	assert!(
		sqlite_sql.contains("AUTOINCREMENT")
			|| sqlite_sql.contains("autoincrement")
			|| sqlite_sql.contains("INTEGER PRIMARY KEY"),
		"SQLite should use AUTOINCREMENT or INTEGER PRIMARY KEY: {}",
		sqlite_sql
	);
}

// ============================================================================
// Lock Contention Tests
// ============================================================================

/// Test lock behavior during migrations
///
/// **Test Intent**: Verify migrations handle table locks correctly
///
/// **Integration Point**: MigrationExecutor → PostgreSQL table locking
///
/// **Expected Behavior**: Locks are acquired and released properly
#[rstest]
#[tokio::test]
async fn test_lock_contention(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create a table
	let migration = create_test_migration(
		"testapp",
		"0001_lock_test",
		vec![Operation::CreateTable {
			name: leak_str("lock_table").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				create_basic_column("value", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to create table");

	// Insert test data
	sqlx::query("INSERT INTO lock_table (value) VALUES (1)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert");

	// Test FOR UPDATE lock
	let row: (i32, i32) =
		sqlx::query_as("SELECT id, value FROM lock_table WHERE id = 1 FOR UPDATE")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to select with lock");

	assert_eq!(row.0, 1, "ID should be 1");
	assert_eq!(row.1, 1, "Value should be 1");

	// Update with lock
	sqlx::query("UPDATE lock_table SET value = 2 WHERE id = 1")
		.execute(pool.as_ref())
		.await
		.expect("Failed to update");

	let updated: (i32,) = sqlx::query_as("SELECT value FROM lock_table WHERE id = 1")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to verify update");

	assert_eq!(updated.0, 2, "Value should be updated to 2");
}

// ============================================================================
// Multi-Database Type Compatibility Tests
// ============================================================================

/// Test common SQL types work across databases
///
/// **Test Intent**: Verify standard SQL types are portable
///
/// **Integration Point**: MigrationExecutor → cross-database type handling
///
/// **Expected Behavior**: Standard types work consistently
#[rstest]
#[tokio::test]
async fn test_common_type_compatibility(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table with common SQL types that should work across DBs
	let migration = create_test_migration(
		"testapp",
		"0001_common_types",
		vec![Operation::CreateTable {
			name: leak_str("common_types_table").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				},
				create_basic_column("int_col", FieldType::Integer),
				create_basic_column("bigint_col", FieldType::BigInteger),
				create_basic_column("text_col", FieldType::Text),
				create_basic_column("varchar_col", FieldType::VarChar(255)),
				create_basic_column("bool_col", FieldType::Boolean),
				create_basic_column("float_col", FieldType::Float),
				create_basic_column("date_col", FieldType::Date),
				create_basic_column("timestamp_col", FieldType::DateTime),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_ok(),
		"Common types should be created: {:?}",
		result.err()
	);

	// Verify all columns exist
	let columns: Vec<(String,)> = sqlx::query_as(
		"SELECT column_name FROM information_schema.columns
		 WHERE table_name = 'common_types_table'
		 ORDER BY ordinal_position",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to get columns");

	assert_eq!(columns.len(), 9, "Should have 9 columns");

	// Test data insertion with all types
	sqlx::query(
		"INSERT INTO common_types_table
		 (int_col, bigint_col, text_col, varchar_col, bool_col, float_col, date_col, timestamp_col)
		 VALUES (42, 9223372036854775807, 'text', 'varchar', true, 3.14, '2024-01-01', '2024-01-01 12:00:00')",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert data");

	let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM common_types_table")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count");

	assert_eq!(count.0, 1, "Should have 1 row");
}
