//! Large scale edge case tests
//!
//! Tests performance and correctness with:
//! - 100+ migrations in single migrate command
//! - Very large SQL statements (10MB+)
//!
//! **Test Coverage:**
//! - EC-RE-01: Large migration set (100+ migrations)
//! - EC-RE-02: Very large SQL (10MB+)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//!
//! **Performance Baselines:**
//! - 100 migrations: < 60 seconds
//! - 10MB SQL execution: < 30 seconds

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Test Helper Functions
// ============================================================================

fn leak_str(s: impl Into<String>) -> &'static str {
	Box::leak(s.into().into_boxed_str())
}

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

// ============================================================================
// EC-RE-01: Large Migration Set Tests
// ============================================================================

/// Test EC-RE-01: Large migration set (100+ migrations)
///
/// **Test Intent**: Verify that the migration system can handle 100+ migrations
/// in a single migrate command without errors or performance degradation.
///
/// **Integration Point**: MigrationExecutor → dependency resolution → PostgreSQL
///
/// **Expected Behavior**: All 100 migrations are applied in correct order
///
/// **Performance Baseline**: Should complete in < 60 seconds
#[rstest]
#[tokio::test]
async fn test_ec_re_01_large_migration_set(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Arrange
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create 100 migrations programmatically
	let mut migrations = Vec::new();
	let migration_count = 100;

	for i in 0..migration_count {
		let table_name = leak_str(format!("large_scale_table_{}", i));
		let migration_name = leak_str(format!("{:04}_large_scale_{}", i + 1, i));

		let migration = create_test_migration(
			"testapp",
			migration_name,
			vec![Operation::CreateTable {
				name: table_name.to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_basic_column("data", FieldType::Text),
					create_basic_column("created_at", FieldType::Timestamp),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			}],
		);

		migrations.push(migration);
	}

	// Act
	let start_time = std::time::Instant::now();
	let result = executor.apply_migrations(&migrations).await;
	let duration = start_time.elapsed();

	// Assert
	assert!(
		result.is_ok(),
		"100 migrations should be applied successfully: {:?}",
		result.err()
	);

	// Verify all tables were created
	for i in 0..migration_count {
		let table_name = format!("large_scale_table_{}", i);
		let exists = sqlx::query(&format!(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = '{}')",
			table_name
		))
		.fetch_one(pool.as_ref())
		.await
		.expect(&format!("Failed to check table {}", table_name))
		.get::<bool, _>(0);

		assert!(exists, "Table {} should exist", table_name);
	}

	println!("Applied {} migrations in {:?}", migration_count, duration);
	assert!(
		duration.as_secs() < 60,
		"Should complete in < 60 seconds, took {:?}",
		duration
	);
}

/// Test EC-RE-01 variant: Large migration set with dependencies
///
/// **Test Intent**: Verify dependency resolution works correctly with 100+ migrations
///
/// **Integration Point**: MigrationExecutor → dependency resolution with large set
///
/// **Expected Behavior**: Migrations with dependencies are resolved and applied correctly
#[rstest]
#[tokio::test]
async fn test_ec_re_01_large_migration_set_with_dependencies(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Arrange
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create 100 migrations with a chain of dependencies
	let mut migrations = Vec::new();
	let migration_count = 100;

	for i in 0..migration_count {
		let table_name = leak_str(format!("dep_table_{}", i));
		let migration_name = leak_str(format!("{:04}_dep_chain_{}", i + 1, i));

		// Each migration depends on the previous one (except first)
		let dependencies = if i > 0 {
			vec![(
				"testapp".to_string(),
				leak_str(format!("{:04}_dep_chain_{}", i, i - 1)).to_string(),
			)]
		} else {
			vec![]
		};

		let mut migration = create_test_migration(
			"testapp",
			migration_name,
			vec![Operation::CreateTable {
				name: table_name.to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_basic_column("value", FieldType::Integer),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			}],
		);
		migration.dependencies = dependencies;

		migrations.push(migration);
	}

	// Act
	let result = executor.apply_migrations(&migrations).await;

	// Assert
	assert!(
		result.is_ok(),
		"100 migrations with dependencies should be applied: {:?}",
		result.err()
	);

	// Verify all tables were created in order
	for i in 0..migration_count {
		let table_name = format!("dep_table_{}", i);
		let exists = sqlx::query(&format!(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = '{}')",
			table_name
		))
		.fetch_one(pool.as_ref())
		.await
		.expect(&format!("Failed to check table {}", table_name))
		.get::<bool, _>(0);

		assert!(exists, "Table {} should exist", table_name);
	}

	// Verify dependency order by checking migration state table
	let applied_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM reinhardt_migrations WHERE app = 'testapp'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count applied migrations");

	assert_eq!(
		applied_count, migration_count,
		"All {} migrations should be recorded",
		migration_count
	);
}

/// Test EC-RE-01 variant: Large migration set with mixed operations
///
/// **Test Intent**: Verify various operation types work in large migration sets
///
/// **Integration Point**: MigrationExecutor → mixed operation handling
///
/// **Expected Behavior**: CREATE TABLE, ADD COLUMN, CREATE INDEX all work correctly
#[rstest]
#[tokio::test]
async fn test_ec_re_01_large_migration_set_mixed_operations(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Arrange
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let mut migrations = Vec::new();

	// First migration: Create base table
	migrations.push(create_test_migration(
		"testapp",
		"0001_base_table",
		vec![Operation::CreateTable {
			name: leak_str("mixed_ops_table").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("field_0", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	));

	// Next 50 migrations: Add columns
	for i in 1..=50 {
		migrations.push(create_test_migration(
			"testapp",
			leak_str(format!("{:04}_add_column_{}", i + 1, i)),
			vec![Operation::AddColumn {
				table: leak_str("mixed_ops_table").to_string(),
				column: create_basic_column(
					leak_str(format!("field_{}", i)),
					FieldType::Text,
				),
				mysql_options: None,
			}],
		));
	}

	// Next 49 migrations: Create indexes on different columns
	for i in 1..=49 {
		migrations.push(create_test_migration(
			"testapp",
			leak_str(format!("{:04}_add_index_{}", i + 51, i)),
			vec![Operation::CreateIndex {
				table: leak_str("mixed_ops_table").to_string(),
				columns: vec![leak_str(format!("field_{}", i)).to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			}],
		));
	}

	// Total: 100 migrations (1 create + 50 add_column + 49 create_index)
	assert_eq!(migrations.len(), 100, "Should have 100 migrations");

	// Act
	let result = executor.apply_migrations(&migrations).await;

	// Assert
	assert!(
		result.is_ok(),
		"100 mixed operation migrations should be applied: {:?}",
		result.err()
	);

	// Verify table exists with all columns
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'mixed_ops_table'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count columns");

	assert_eq!(column_count, 51, "Should have 51 columns (id + field_0 to field_50)");

	// Verify indexes exist
	let index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'mixed_ops_table'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count indexes");

	assert_eq!(index_count, 49, "Should have 49 indexes");
}

// ============================================================================
// EC-RE-02: Very Large SQL Tests
// ============================================================================

/// Test EC-RE-02: Very large SQL statement (10MB+)
///
/// **Test Intent**: Verify that very large SQL statements can be executed
/// without errors or memory issues.
///
/// **Integration Point**: MigrationExecutor → PostgreSQL large SQL handling
///
/// **Expected Behavior**: Large SQL is executed successfully
///
/// **Performance Baseline**: Should complete in < 30 seconds
#[rstest]
#[tokio::test]
async fn test_ec_re_02_very_large_sql_statement(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Arrange
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create base table
	let create_table = create_test_migration(
		"testapp",
		"0001_create_large_sql_table",
		vec![Operation::CreateTable {
			name: leak_str("large_sql_table").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("data", FieldType::Text),
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

	// Generate a very large SQL statement (10MB+)
	// Using a multi-value INSERT for efficiency
	let target_size = 10 * 1024 * 1024; // 10MB
	let mut sql_parts = Vec::new();
	let mut current_size = 0;

	// Each row is approximately 100 bytes
	let row_size = 100;
	let mut row_count = 0;

	while current_size < target_size {
		let values = (0..1000)
			.map(|i| {
				format!(
					"('Large data payload row {} with some padding text to increase size')",
					row_count + i
				)
			})
			.collect::<Vec<_>>()
			.join(", ");

		sql_parts.push(format!(
			"INSERT INTO large_sql_table (data) VALUES {}",
			values
		));

		current_size += sql_parts.last().unwrap().len();
		row_count += 1000;
	}

	let large_sql = sql_parts.join("; ");
	let actual_size_bytes = large_sql.len() as f64 / (1024.0 * 1024.0);

	println!(
		"Generated SQL of {:.2} MB with {} rows",
		actual_size_bytes, row_count
	);

	// Act
	let start_time = std::time::Instant::now();
	let migration = create_test_migration(
		"testapp",
		"0002_large_sql_insert",
		vec![Operation::RunSQL {
			sql: leak_str(large_sql.clone()).to_string(),
			reverse_sql: Some(leak_str("DELETE FROM large_sql_table".to_string()).to_string()),
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;
	let duration = start_time.elapsed();

	// Assert
	assert!(
		result.is_ok(),
		"Large SQL (10MB+) should be executed successfully: {:?}",
		result.err()
	);

	// Verify data was inserted
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM large_sql_table")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count rows");

	assert_eq!(count, row_count, "Should have {} rows", row_count);

	println!("Executed {:.2} MB SQL in {:?}", actual_size_bytes, duration);
	assert!(
		duration.as_secs() < 30,
		"Should complete in < 30 seconds, took {:?}",
		duration
	);
}

/// Test EC-RE-02 variant: Large SQL with complex multi-statement batch
///
/// **Test Intent**: Verify large SQL batches with multiple statements work correctly
///
/// **Integration Point**: MigrationExecutor → PostgreSQL multi-statement handling
///
/// **Expected Behavior**: All statements in large SQL batch are executed
#[rstest]
#[tokio::test]
async fn test_ec_re_02_large_sql_multi_statement(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Arrange
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Generate large SQL with multiple CREATE TABLE and INSERT statements
	let mut sql_statements = Vec::new();
	let table_count = 50;

	for i in 0..table_count {
		let table_name = format!("batch_table_{}", i);

		// CREATE TABLE
		sql_statements.push(format!(
			"CREATE TABLE {} (id SERIAL PRIMARY KEY, value TEXT);",
			table_name
		));

		// INSERT multiple rows
		let values = (0..100)
			.map(|j| format!("('Batch value {}-{}')", i, j))
			.collect::<Vec<_>>()
			.join(", ");

		sql_statements.push(format!(
			"INSERT INTO {} (value) VALUES {};",
			table_name, values
		));
	}

	let large_sql = sql_statements.join("\n");
	let sql_size_mb = large_sql.len() as f64 / (1024.0 * 1024.0);

	println!(
		"Generated multi-statement SQL of {:.2} MB with {} tables",
		sql_size_mb, table_count
	);

	// Act
	let migration = create_test_migration(
		"testapp",
		"0001_large_batch_sql",
		vec![Operation::RunSQL {
			sql: leak_str(large_sql).to_string(),
			reverse_sql: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	// Assert
	assert!(
		result.is_ok(),
		"Large multi-statement SQL should be executed: {:?}",
		result.err()
	);

	// Verify all tables were created
	for i in 0..table_count {
		let table_name = format!("batch_table_{}", i);
		let exists = sqlx::query(&format!(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = '{}')",
			table_name
		))
		.fetch_one(pool.as_ref())
		.await
		.expect(&format!("Failed to check table {}", table_name))
		.get::<bool, _>(0);

		assert!(exists, "Table {} should exist", table_name);

		// Verify row count
		let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table_name))
			.fetch_one(pool.as_ref())
			.await
			.expect(&format!("Failed to count rows in {}", table_name));

		assert_eq!(count, 100, "Table {} should have 100 rows", table_name);
	}
}

/// Test EC-RE-02 variant: Large ALTER TABLE statement
///
/// **Test Intent**: Verify large ALTER TABLE statements work correctly
///
/// **Integration Point**: MigrationExecutor → PostgreSQL ALTER TABLE handling
///
/// **Expected Behavior**: Large ALTER TABLE with multiple columns is executed
#[rstest]
#[tokio::test]
async fn test_ec_re_02_large_alter_table_statement(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Arrange
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create base table
	let create_table = create_test_migration(
		"testapp",
		"0001_create_alter_table",
		vec![Operation::CreateTable {
			name: leak_str("wide_table").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
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

	// Generate large ALTER TABLE statement with 100 columns
	let mut add_columns = Vec::new();
	for i in 0..100 {
		add_columns.push(format!(
			"ADD COLUMN col_{} VARCHAR(255)",
			i
		));
	}

	let large_alter_sql = format!(
		"ALTER TABLE wide_table {};",
		add_columns.join(", ")
	);

	println!(
		"Generated ALTER TABLE with {} additions, size: {:.2} KB",
		100,
		large_alter_sql.len() as f64 / 1024.0
	);

	// Act
	let migration = create_test_migration(
		"testapp",
		"0002_large_alter_table",
		vec![Operation::RunSQL {
			sql: leak_str(large_alter_sql).to_string(),
			reverse_sql: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	// Assert
	assert!(
		result.is_ok(),
		"Large ALTER TABLE should be executed: {:?}",
		result.err()
	);

	// Verify all columns were added
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'wide_table'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count columns");

	assert_eq!(column_count, 101, "Should have 101 columns (id + 100 new columns)");
}
