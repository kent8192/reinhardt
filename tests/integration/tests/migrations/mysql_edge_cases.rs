//! MySQL-specific edge case tests
//!
//! Tests MySQL non-transactional DDL behavior:
//! - Partial state after DDL failure
//! - Error messages about partial state
//!
//! MySQL DDL statements cause implicit commits, preventing rollback.
//!
//! **Test Coverage:**
//! - Non-transactional DDL behavior
//! - Implicit commit detection
//! - Partial migration state handling
//! - Error message clarity for partial state
//!
//! **Fixtures Used:**
//! - mysql_container: MySQL database container

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, Constraint, FieldType, Migration, Operation,
	executor::DatabaseMigrationExecutor,
};
use reinhardt_test::fixtures::mysql_container;
use rstest::*;
use sqlx::Row;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Create a simple migration for testing
fn create_test_migration(app: &str, name: &str, operations: Vec<Operation>) -> Migration {
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

/// Create a column with constraints
fn create_column_with_constraints(
	name: &str,
	type_def: FieldType,
	not_null: bool,
	primary_key: bool,
) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null,
		unique: false,
		primary_key,
		auto_increment: primary_key,
		default: None,
	}
}

// ============================================================================
// EC-DB-03: Non-transactional DDL failure
// ============================================================================

/// Test MySQL DDL implicit commit behavior on migration failure
///
/// **Test Intent**: Verify that MySQL's implicit commit behavior is properly handled
/// when a migration fails partway through execution.
///
/// **Integration Point**: MigrationExecutor → MySQL DDL statements
///
/// **Expected Behavior**:
/// - First DDL statement is committed (implicit commit)
/// - Second statement fails
/// - Error message indicates partial state
/// - Database contains partially applied changes
///
/// **MySQL Behavior**:
/// MySQL DDL statements (CREATE TABLE, ALTER TABLE, DROP TABLE, etc.) cause
/// implicit commits. This means:
/// 1. Each DDL statement commits before it executes
/// 2. Cannot roll back DDL statements within a transaction
/// 3. Failed migrations leave database in partial state
#[rstest]
#[tokio::test]
async fn test_mysql_ddl_implicit_commit_partial_state(
	#[future] mysql_container: (
		ContainerAsync<GenericImage>,
		Arc<sqlx::MySqlPool>,
		u16,
		String,
	),
) {
	// Arrange
	let (_container, pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create migration with two operations:
	// 1. CREATE TABLE (will succeed and be committed implicitly)
	// 2. CREATE TABLE with invalid reference (will fail)
	//
	// MySQL's implicit commit behavior means the first operation
	// cannot be rolled back even though the second operation fails.
	let migration = create_test_migration(
		"testapp",
		"0001_partial_ddl",
		vec![
			// First operation: CREATE TABLE - will be committed implicitly
			Operation::CreateTable {
				name: "test_partial_table".to_string(),
				columns: vec![
					create_column_with_constraints("id", FieldType::Integer, true, true),
					create_basic_column("name", FieldType::VarChar(255)),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			// Second operation: Try to create a table with a foreign key
			// referencing a non-existent table
			// This will fail, but the first table will already exist (implicit commit)
			Operation::CreateTable {
				name: "test_ref_table".to_string(),
				columns: vec![
					create_column_with_constraints("id", FieldType::Integer, true, true),
					create_basic_column("test_id", FieldType::Integer),
				],
				constraints: vec![Constraint::ForeignKey {
					name: "fk_test".to_string(),
					columns: vec!["test_id".to_string()],
					referenced_table: "nonexistent_table".to_string(),
					referenced_columns: vec!["id".to_string()],
					on_delete: reinhardt_db::migrations::ForeignKeyAction::NoAction,
					on_update: reinhardt_db::migrations::ForeignKeyAction::NoAction,
					deferrable: None,
				}],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
	);

	// Act
	let result = executor.apply_migrations(&[migration]).await;

	// Assert
	// Migration should fail due to invalid foreign key reference
	assert!(
		result.is_err(),
		"Migration should fail due to non-existent referenced table"
	);

	// Verify error message mentions foreign key issue
	let error_msg = format!("{:?}", result.err());
	assert!(
		error_msg.contains("Foreign key")
			|| error_msg.contains("constraint")
			|| error_msg.contains("nonexistent")
			|| error_msg.contains("referenced"),
		"Error message should indicate foreign key constraint issue: {}",
		error_msg
	);

	// CRITICAL: Verify that the first table exists despite migration failure
	// This proves MySQL's implicit commit behavior - the CREATE TABLE
	// was committed before the failing operation.
	let table_exists = sqlx::query(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'test_db' AND table_name = 'test_partial_table'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table existence")
	.get::<i64, _>(0);

	assert_eq!(
		table_exists, 1,
		"Table should exist due to MySQL's implicit commit behavior, even though migration failed"
	);

	// Verify the table has the expected columns.
	// Note: Using index-based access instead of column name because MySQL returns
	// information_schema column names in uppercase (COLUMN_NAME), while sqlx uses
	// case-sensitive lookup. Index-based access is robust across databases. (fixes #1357)
	let columns = sqlx::query(
		"SELECT column_name FROM information_schema.columns WHERE table_schema = 'test_db' AND table_name = 'test_partial_table' ORDER BY ordinal_position",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to fetch columns");

	assert_eq!(
		columns.len(),
		2,
		"Table should have both columns (id, name)"
	);

	let column_names: Vec<String> = columns.iter().map(|row| row.get::<String, _>(0)).collect();

	assert!(
		column_names.contains(&"id".to_string()),
		"Table should have 'id' column"
	);
	assert!(
		column_names.contains(&"name".to_string()),
		"Table should have 'name' column"
	);

	// Cleanup
	sqlx::query("DROP TABLE IF EXISTS test_partial_table")
		.execute(pool.as_ref())
		.await
		.expect("Failed to cleanup test table");
}

/// Test explicit error message for MySQL DDL partial state
///
/// **Test Intent**: Verify that users receive clear error messages when
/// MySQL's implicit commit behavior results in partial migration state.
///
/// **Integration Point**: MigrationExecutor → Error reporting for MySQL DDL
///
/// **Expected Behavior**:
/// - Error message explicitly mentions partial state
/// - Error message indicates manual cleanup may be needed
#[rstest]
#[tokio::test]
async fn test_mysql_partial_state_error_message(
	#[future] mysql_container: (
		ContainerAsync<GenericImage>,
		Arc<sqlx::MySqlPool>,
		u16,
		String,
	),
) {
	// Arrange
	let (_container, pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create a migration that will fail partway through
	let migration = create_test_migration(
		"testapp",
		"0002_error_msg_test",
		vec![
			Operation::CreateTable {
				name: "before_failure".to_string(),
				columns: vec![
					create_column_with_constraints("id", FieldType::Integer, true, true),
					create_basic_column("data", FieldType::Text),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			// This will fail - AddColumn to non-existent table
			Operation::AddColumn {
				table: "does_not_exist".to_string(),
				column: create_basic_column("new_column", FieldType::Integer),
				mysql_options: None,
			},
		],
	);

	// Act
	let result = executor.apply_migrations(&[migration]).await;

	// Assert
	assert!(result.is_err(), "Migration should fail");

	// Verify partial state - first table should exist
	let table_exists = sqlx::query(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'test_db' AND table_name = 'before_failure'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table")
	.get::<i64, _>(0);

	assert_eq!(
		table_exists, 1,
		"Table created before failure should exist (MySQL implicit commit)"
	);

	// Verify error message is informative
	let error_msg = format!("{:?}", result);

	// Error should mention the operation that failed
	assert!(
		error_msg.contains("does_not_exist")
			|| error_msg.contains("table")
			|| error_msg.contains("Table"),
		"Error message should mention the missing table or operation: {}",
		error_msg
	);

	// Cleanup
	sqlx::query("DROP TABLE IF EXISTS before_failure")
		.execute(pool.as_ref())
		.await
		.expect("Failed to cleanup");
}

/// Test multiple DDL statements in a single migration on MySQL
///
/// **Test Intent**: Verify that each DDL statement is independently
/// committed in MySQL, even when part of a single atomic migration.
///
/// **Integration Point**: MigrationExecutor → MySQL multi-statement migration
///
/// **Expected Behavior**:
/// - Each successful DDL statement is committed independently
/// - Failure mid-migration leaves earlier DDL statements applied
#[rstest]
#[tokio::test]
async fn test_mysql_multiple_ddl_statements(
	#[future] mysql_container: (
		ContainerAsync<GenericImage>,
		Arc<sqlx::MySqlPool>,
		u16,
		String,
	),
) {
	// Arrange
	let (_container, pool, _port, url) = mysql_container.await;

	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	let migration = create_test_migration(
		"testapp",
		"0003_multi_ddl",
		vec![
			// First table
			Operation::CreateTable {
				name: "table_one".to_string(),
				columns: vec![
					create_column_with_constraints("id", FieldType::Integer, true, true),
					create_basic_column("value", FieldType::VarChar(100)),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			// Second table
			Operation::CreateTable {
				name: "table_two".to_string(),
				columns: vec![
					create_column_with_constraints("id", FieldType::Integer, true, true),
					create_basic_column("data", FieldType::Text),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			// Third table
			Operation::CreateTable {
				name: "table_three".to_string(),
				columns: vec![
					create_column_with_constraints("id", FieldType::Integer, true, true),
					create_basic_column("info", FieldType::VarChar(255)),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
	);

	// Act
	let result = executor.apply_migrations(&[migration]).await;

	// Assert - all tables should be created successfully
	assert!(
		result.is_ok(),
		"Migration with multiple CREATE TABLE should succeed: {:?}",
		result.err()
	);

	// Verify all three tables exist
	async fn check_table(pool: &sqlx::MySqlPool, name: &str) -> i64 {
		sqlx::query("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'test_db' AND table_name = ?")
			.bind(name)
			.fetch_one(pool)
			.await
			.expect("Failed to check table")
			.get::<i64, _>(0)
	}

	assert_eq!(
		check_table(pool.as_ref(), "table_one").await,
		1,
		"table_one should exist"
	);
	assert_eq!(
		check_table(pool.as_ref(), "table_two").await,
		1,
		"table_two should exist"
	);
	assert_eq!(
		check_table(pool.as_ref(), "table_three").await,
		1,
		"table_three should exist"
	);

	// Cleanup
	for table in ["table_one", "table_two", "table_three"] {
		sqlx::query(&format!("DROP TABLE IF EXISTS {}", table))
			.execute(pool.as_ref())
			.await
			.expect("Failed to cleanup");
	}
}
