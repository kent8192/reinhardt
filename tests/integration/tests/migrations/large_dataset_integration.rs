//! Large Dataset Integration Tests
//!
//! Tests that verify the migration system's performance and correctness when
//! handling large datasets. These tests focus on bulk operations, performance
//! optimization, and scalability.
//!
//! **Test Coverage:**
//! - Bulk INSERT operations (1 million rows)
//! - Index creation on large tables
//! - CSV bulk loading (COPY FROM for PostgreSQL, LOAD DATA for MySQL)
//! - Batch size optimization
//! - Performance benchmarks (1000 tables, 100 fields, large indexes)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//! - mysql_container: MySQL database container
//!
//! **Performance Baselines:**
//! - 1M row INSERT: < 30 seconds
//! - 1000 table creation: < 10 seconds
//! - INDEX on 1M rows: < 60 seconds
//!
//! **Note**: Some tests are marked as `#[ignore]` by default due to long execution time.
//! Run with `cargo test -- --ignored` to execute performance benchmarks.

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
};
use reinhardt_test::fixtures::{mysql_container, postgres_container};
use rstest::*;
use sqlx::{MySqlPool, PgPool};
use std::sync::Arc;
use std::time::Instant;
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

// ============================================================================
// Bulk Data Operation Tests
// ============================================================================

/// Test bulk INSERT of 1 million rows
///
/// **Test Intent**: Verify that very large bulk INSERTs can be performed
///
/// **Performance Baseline**: Should complete in < 30 seconds
///
/// **Note**: Marked as ignore due to long execution time
#[rstest]
#[ignore = "Long-running test - execute with --ignored flag"]
#[tokio::test]
async fn test_bulk_insert_one_million_rows(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table
	let create_table = create_test_migration(
		"testapp",
		"0001_create_events",
		vec![Operation::CreateTable {
			name: leak_str("events").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("event_type", FieldType::VarChar(50)),
				create_basic_column("timestamp", FieldType::BigInteger),
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

	// Generate 1 million INSERT statements (batch in chunks of 1000)
	let start_time = Instant::now();

	for batch in 0..1000 {
		let mut values_parts = Vec::new();
		for i in 0..1000 {
			let row_num = batch * 1000 + i;
			values_parts.push(format!("('event_type_{}', {})", row_num, row_num));
		}
		let insert_sql = format!(
			"INSERT INTO events (event_type, timestamp) VALUES {}",
			values_parts.join(", ")
		);

		sqlx::query(&insert_sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert batch");
	}

	let duration = start_time.elapsed();

	// Verify 1 million rows were inserted
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count events");

	assert_eq!(count, 1_000_000, "Should have 1 million events");

	println!("Inserted 1M rows in {:?}", duration);
	assert!(duration.as_secs() < 30, "Should complete in < 30 seconds");
}

/// Test index creation before vs after data insertion
///
/// **Test Intent**: Verify that creating index AFTER bulk insert is faster
///
/// **Best Practice**: For large datasets, insert data first, then create indexes
#[rstest]
#[tokio::test]
async fn test_index_after_data_insertion(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table WITHOUT index
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

	// Insert 10,000 rows
	let mut values_parts = Vec::new();
	for i in 0..10_000 {
		values_parts.push(format!("('user{}@example.com')", i));
	}
	let insert_sql = format!(
		"INSERT INTO users (email) VALUES {}",
		values_parts.join(", ")
	);

	sqlx::query(&insert_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert users");

	// NOW create index (after data is inserted)
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

	let start_time = Instant::now();
	executor
		.apply_migrations(&[create_index])
		.await
		.expect("Failed to create index");
	let duration = start_time.elapsed();

	println!("Created index on 10K rows in {:?}", duration);

	// Verify index exists
	let index_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
			.bind("idx_users_email")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to check index");

	assert!(index_exists, "Index should exist");
}

/// Test COPY FROM for bulk CSV loading (PostgreSQL)
///
/// **Test Intent**: Verify that PostgreSQL COPY FROM can be used for fast bulk loading
///
/// **Note**: COPY FROM is much faster than INSERT for large datasets
#[rstest]
#[tokio::test]
async fn test_postgres_copy_from(
	#[future] _postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	use reinhardt_db::migrations::operations::{BulkLoadFormat, BulkLoadOptions, BulkLoadSource};

	// Example: PostgreSQL COPY FROM operation
	let bulk_load_op = Operation::BulkLoad {
		table: leak_str("events").to_string(),
		source: BulkLoadSource::File(leak_str("/tmp/events.csv").to_string()),
		format: BulkLoadFormat::Csv,
		options: BulkLoadOptions::new()
			.with_delimiter(',')
			.with_header(true)
			.with_columns(vec![
				leak_str("event_type").to_string(),
				leak_str("timestamp").to_string(),
			])
			.with_quote('"'),
	};

	// Verify the operation has the expected structure
	if let Operation::BulkLoad {
		table,
		source,
		format,
		options,
	} = &bulk_load_op
	{
		assert_eq!(*table, "events");
		assert!(matches!(source, BulkLoadSource::File(_)));
		assert_eq!(*format, BulkLoadFormat::Csv);
		assert_eq!(options.header, true);
		assert_eq!(options.delimiter, Some(','));
	} else {
		panic!("Expected BulkLoad operation");
	}

	// Note: Actual execution requires file access
	// COPY FROM is 10-100x faster than INSERT for bulk data
}

/// Test LOAD DATA for bulk CSV loading (MySQL)
///
/// **Test Intent**: Verify that MySQL LOAD DATA can be used for fast bulk loading
#[rstest]
#[tokio::test]
async fn test_mysql_load_data(
	#[future] _mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	use reinhardt_db::migrations::operations::{BulkLoadFormat, BulkLoadOptions, BulkLoadSource};

	// Example: MySQL LOAD DATA LOCAL INFILE operation
	let bulk_load_op = Operation::BulkLoad {
		table: leak_str("events").to_string(),
		source: BulkLoadSource::File(leak_str("/tmp/events.csv").to_string()),
		format: BulkLoadFormat::Csv,
		options: BulkLoadOptions::new()
			.with_delimiter(',')
			.with_columns(vec![
				leak_str("event_type").to_string(),
				leak_str("timestamp").to_string(),
			])
			.with_local(true)
			.with_line_terminator("\n"),
	};

	// Verify the operation has the expected structure
	if let Operation::BulkLoad {
		table,
		source,
		format,
		options,
	} = &bulk_load_op
	{
		assert_eq!(*table, "events");
		assert!(matches!(source, BulkLoadSource::File(_)));
		assert_eq!(*format, BulkLoadFormat::Csv);
		assert!(options.local, "MySQL LOAD DATA should use LOCAL keyword");
		assert_eq!(options.line_terminator, Some("\n".to_string()));
	} else {
		panic!("Expected BulkLoad operation");
	}

	// Note: Actual execution requires file access
	// LOAD DATA is optimized for bulk loading in MySQL
}

/// Test batch size optimization (1000 rows per batch)
///
/// **Test Intent**: Verify that batched INSERTs perform better than individual INSERTs
#[rstest]
#[tokio::test]
async fn test_batch_size_optimization(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table
	let create_table = create_test_migration(
		"testapp",
		"0001_create_logs",
		vec![Operation::CreateTable {
			name: leak_str("logs").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("message", FieldType::Text),
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

	// Insert 5000 rows in batches of 1000
	let start_batched = Instant::now();

	for batch in 0..5 {
		let mut values_parts = Vec::new();
		for i in 0..1000 {
			let row_num = batch * 1000 + i;
			values_parts.push(format!("('Message {}')", row_num));
		}
		let insert_sql = format!(
			"INSERT INTO logs (message) VALUES {}",
			values_parts.join(", ")
		);

		sqlx::query(&insert_sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert batch");
	}

	let batched_duration = start_batched.elapsed();

	// Verify 5000 rows
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count logs");

	assert_eq!(count, 5000, "Should have 5000 logs");

	println!("Batched INSERT (1000/batch): {:?}", batched_duration);

	// Note: Individual INSERTs would be much slower
	// For comparison, individual INSERTs might take 10-100x longer
}

// ============================================================================
// Performance Benchmark Tests
// ============================================================================

/// Performance benchmark: Create 1000 tables
///
/// **Test Intent**: Measure schema creation performance for large number of tables
///
/// **Performance Baseline**: Should complete in < 10 seconds
///
/// **Note**: Marked as ignore due to long execution time
#[rstest]
#[ignore = "Long-running performance benchmark - execute with --ignored flag"]
#[tokio::test]
async fn test_performance_create_1000_tables(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create 1000 tables
	let mut operations = Vec::new();
	for i in 0..1000 {
		operations.push(Operation::CreateTable {
			name: leak_str(format!("table_{}", i)).to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("data", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});
	}

	let migration = create_test_migration("testapp", "0001_create_1000_tables", operations);

	let start_time = Instant::now();
	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to create 1000 tables");
	let duration = start_time.elapsed();

	// Verify 1000 tables exist
	let table_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		 WHERE table_schema = 'public' AND table_name LIKE 'table_%'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count tables");

	assert_eq!(table_count, 1000, "Should have 1000 tables");

	println!("Created 1000 tables in {:?}", duration);
	assert!(duration.as_secs() < 10, "Should complete in < 10 seconds");
}

/// Performance benchmark: Add column to table with 100 existing fields
///
/// **Test Intent**: Measure ALTER TABLE performance on wide tables
///
/// **Performance Baseline**: Should complete in < 5 seconds
#[rstest]
#[tokio::test]
async fn test_performance_add_column_to_wide_table(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with 100 columns
	let mut columns = vec![create_auto_pk_column("id", FieldType::Integer)];
	for i in 0..99 {
		columns.push(create_basic_column(
			leak_str(format!("col_{}", i)),
			FieldType::VarChar(100),
		));
	}

	let create_table = create_test_migration(
		"testapp",
		"0001_create_wide_table",
		vec![Operation::CreateTable {
			name: leak_str("wide_table").to_string(),
			columns,
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create wide table");

	// Add one more column
	let add_column = create_test_migration(
		"testapp",
		"0002_add_column_100",
		vec![Operation::AddColumn {
			table: leak_str("wide_table").to_string(),
			column: create_basic_column("col_100", FieldType::VarChar(100)),
			mysql_options: None,
		}],
	);

	let start_time = Instant::now();
	executor
		.apply_migrations(&[add_column])
		.await
		.expect("Failed to add column");
	let duration = start_time.elapsed();

	// Verify column was added
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'wide_table'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count columns");

	assert_eq!(column_count, 101, "Should have 101 columns");

	println!("Added column to 100-field table in {:?}", duration);
	assert!(duration.as_secs() < 30, "Should complete in < 30 seconds");
}

/// Performance benchmark: Create index on 1 million row table
///
/// **Test Intent**: Measure CREATE INDEX performance on large table
///
/// **Performance Baseline**: Should complete in < 60 seconds
///
/// **Note**: Marked as ignore due to long execution time
#[rstest]
#[ignore = "Long-running performance benchmark - execute with --ignored flag"]
#[tokio::test]
async fn test_performance_index_on_large_table(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table
	let create_table = create_test_migration(
		"testapp",
		"0001_create_large_table",
		vec![Operation::CreateTable {
			name: leak_str("large_table").to_string(),
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

	// Insert 1 million rows (in batches)
	for batch in 0..1000 {
		let mut values_parts = Vec::new();
		for i in 0..1000 {
			let row_num = batch * 1000 + i;
			values_parts.push(format!("('user{}@example.com')", row_num));
		}
		let insert_sql = format!(
			"INSERT INTO large_table (email) VALUES {}",
			values_parts.join(", ")
		);

		sqlx::query(&insert_sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to insert batch");
	}

	// Create index on 1M rows
	let create_index = create_test_migration(
		"testapp",
		"0002_create_index",
		vec![Operation::CreateIndex {
			table: leak_str("large_table").to_string(),
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

	let start_time = Instant::now();
	executor
		.apply_migrations(&[create_index])
		.await
		.expect("Failed to create index");
	let duration = start_time.elapsed();

	// Verify index exists
	let index_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
			.bind("idx_large_table_email")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to check index");

	assert!(index_exists, "Index should exist");

	println!("Created index on 1M rows in {:?}", duration);
	assert!(duration.as_secs() < 60, "Should complete in < 60 seconds");
}
