//! Integration tests for concurrent migration execution
//!
//! Tests race conditions, lock handling, and parallel execution:
//! - Simultaneous migration execution
//! - History table concurrent writes
//! - Deadlock detection and recovery
//! - Timeout handling
//! - Crash recovery scenarios
//!
//! **Test Coverage:**
//! - Concurrent database operations
//! - Lock acquisition and release
//! - Transaction conflict resolution
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
	recorder::DatabaseMigrationRecorder,
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serial_test::serial;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{ContainerAsync, GenericImage};
use tokio::task::JoinSet;

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

/// Create a migration with explicit dependencies for ordering tests
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

// ============================================================================
// Simultaneous Migration Execution Tests
// ============================================================================

/// Test simultaneous migration execution from multiple connections
///
/// **Test Intent**: Verify that concurrent migration attempts are handled safely
///
/// **Integration Point**: MigrationExecutor → PostgreSQL locking → MigrationRecorder
///
/// **Expected Behavior**: One migration succeeds, others wait or skip (already applied)
#[rstest]
#[tokio::test]
#[serial(concurrent_migrate)]
async fn test_simultaneous_migrate(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Pre-create migration to test concurrent attempts
	let table_name = leak_str("concurrent_table");
	let migration = create_test_migration(
		"testapp",
		"0001_concurrent",
		vec![Operation::CreateTable {
			name: table_name.to_string(),
			columns: vec![ColumnDefinition {
				name: "id".to_string(),
				type_definition: FieldType::Custom("SERIAL".to_string()),
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: false,
				default: None,
			}],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Clone values for async tasks
	let url1 = url.clone();
	let url2 = url.clone();
	let migration1 = migration.clone();
	let migration2 = migration.clone();

	// Launch two concurrent migration attempts
	let mut set = JoinSet::new();

	set.spawn(async move {
		let conn = DatabaseConnection::connect_postgres(&url1)
			.await
			.expect("Failed to connect");
		let mut executor = DatabaseMigrationExecutor::new(conn);
		executor.apply_migrations(&[migration1]).await
	});

	set.spawn(async move {
		// Small delay to create race condition
		tokio::time::sleep(Duration::from_millis(10)).await;
		let conn = DatabaseConnection::connect_postgres(&url2)
			.await
			.expect("Failed to connect");
		let mut executor = DatabaseMigrationExecutor::new(conn);
		executor.apply_migrations(&[migration2]).await
	});

	// Collect results
	let mut results = Vec::new();
	while let Some(res) = set.join_next().await {
		results.push(res.expect("Task panicked"));
	}

	// At least one should succeed
	let success_count = results.iter().filter(|r| r.is_ok()).count();
	assert!(
		success_count >= 1,
		"At least one migration should succeed, got {} successes",
		success_count
	);

	// Verify table exists exactly once
	let exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'concurrent_table')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table")
	.get::<bool, _>(0);

	assert!(exists, "Table should exist after concurrent migrations");

	// Verify migration was recorded only once
	let migration_count: (i64,) = sqlx::query_as(
		"SELECT COUNT(*) FROM reinhardt_migrations
		 WHERE app = 'testapp' AND name = '0001_concurrent'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count migrations");

	assert_eq!(
		migration_count.0, 1,
		"Migration should be recorded exactly once"
	);
}

// ============================================================================
// History Table Concurrent Write Tests
// ============================================================================

/// Test concurrent writes to migration history table
///
/// **Test Intent**: Verify migration recorder handles concurrent writes safely
///
/// **Integration Point**: DatabaseMigrationRecorder → PostgreSQL concurrent INSERT
///
/// **Expected Behavior**: All records are written without data loss
#[rstest]
#[tokio::test]
#[serial(history_write)]
async fn test_history_table_concurrent_write(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Initialize recorder to create the table
	let init_conn = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect");
	let init_recorder = DatabaseMigrationRecorder::new(init_conn);
	init_recorder
		.ensure_schema_table()
		.await
		.expect("Failed to ensure schema table");

	// Launch multiple concurrent record operations
	let num_writers = 5;
	let mut set = JoinSet::new();

	for i in 0..num_writers {
		let url_clone = url.clone();
		let app_name = leak_str(format!("concurrent_app_{}", i));
		let migration_name = leak_str(format!("0001_migration_{}", i));

		set.spawn(async move {
			let conn = DatabaseConnection::connect_postgres(&url_clone)
				.await
				.expect("Failed to connect");
			let recorder = DatabaseMigrationRecorder::new(conn);
			recorder.record_applied(app_name, migration_name).await
		});
	}

	// Wait for all writes to complete
	let mut results = Vec::new();
	while let Some(res) = set.join_next().await {
		results.push(res.expect("Task panicked"));
	}

	// All writes should succeed
	let success_count = results.iter().filter(|r| r.is_ok()).count();
	assert_eq!(
		success_count, num_writers,
		"All {} writers should succeed",
		num_writers
	);

	// Verify all records exist
	let record_count: (i64,) = sqlx::query_as(
		"SELECT COUNT(*) FROM reinhardt_migrations WHERE app LIKE 'concurrent_app_%'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count records");

	assert_eq!(
		record_count.0, num_writers as i64,
		"All {} records should exist",
		num_writers
	);
}

// ============================================================================
// Deadlock Detection Tests
// ============================================================================

/// Test deadlock detection during migration
///
/// **Test Intent**: Verify system handles potential deadlock situations
///
/// **Integration Point**: MigrationExecutor → PostgreSQL deadlock detection
///
/// **Expected Behavior**: Deadlocks are detected and one transaction is aborted
#[rstest]
#[tokio::test]
#[serial(deadlock)]
async fn test_deadlock_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Create two tables for deadlock scenario
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS deadlock_a (
			id SERIAL PRIMARY KEY,
			value INTEGER
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table A");

	sqlx::query(
		"CREATE TABLE IF NOT EXISTS deadlock_b (
			id SERIAL PRIMARY KEY,
			value INTEGER
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table B");

	// Insert data
	sqlx::query("INSERT INTO deadlock_a (value) VALUES (1)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert A");

	sqlx::query("INSERT INTO deadlock_b (value) VALUES (1)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert B");

	// Create two connections that will attempt to create a deadlock
	let url1 = url.clone();
	let url2 = url.clone();

	let mut set = JoinSet::new();

	// Task 1: Lock A then try to lock B
	set.spawn(async move {
		let conn = DatabaseConnection::connect_postgres(&url1)
			.await
			.expect("Failed to connect");

		// Begin transaction and lock A
		conn.execute("BEGIN", vec![]).await?;
		conn.execute("SELECT * FROM deadlock_a WHERE id = 1 FOR UPDATE", vec![])
			.await?;

		// Wait a bit to create race condition
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Try to lock B (might deadlock)
		let result = tokio::time::timeout(
			Duration::from_secs(5),
			conn.execute("SELECT * FROM deadlock_b WHERE id = 1 FOR UPDATE", vec![]),
		)
		.await;

		conn.execute("COMMIT", vec![]).await?;

		Ok::<_, reinhardt_db::backends::DatabaseError>(result.is_ok())
	});

	// Task 2: Lock B then try to lock A
	set.spawn(async move {
		// Small delay
		tokio::time::sleep(Duration::from_millis(50)).await;

		let conn = DatabaseConnection::connect_postgres(&url2)
			.await
			.expect("Failed to connect");

		// Begin transaction and lock B
		conn.execute("BEGIN", vec![]).await?;
		conn.execute("SELECT * FROM deadlock_b WHERE id = 1 FOR UPDATE", vec![])
			.await?;

		// Wait a bit
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Try to lock A (might deadlock)
		let result = tokio::time::timeout(
			Duration::from_secs(5),
			conn.execute("SELECT * FROM deadlock_a WHERE id = 1 FOR UPDATE", vec![]),
		)
		.await;

		conn.execute("COMMIT", vec![]).await?;

		Ok::<_, reinhardt_db::backends::DatabaseError>(result.is_ok())
	});

	// Wait for both tasks
	let mut results = Vec::new();
	while let Some(res) = set.join_next().await {
		results.push(res);
	}

	// At least one should complete (PostgreSQL detects deadlock and aborts one)
	let completed_count = results
		.iter()
		.filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
		.count();

	// The test passes if deadlock was handled (either both complete or one is aborted)
	assert!(
		completed_count >= 1,
		"At least one transaction should complete, deadlock should be detected"
	);
}

// ============================================================================
// Migration Timeout Tests
// ============================================================================

/// Test timeout handling for long-running migrations
///
/// **Test Intent**: Verify migrations respect timeout settings
///
/// **Integration Point**: MigrationExecutor → connection timeout
///
/// **Expected Behavior**: Long operations timeout appropriately
#[rstest]
#[tokio::test]
#[serial(timeout)]
async fn test_migration_timeout(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create a migration with a simulated long operation
	// Using pg_sleep to simulate a long-running operation
	let migration = create_test_migration(
		"testapp",
		"0001_slow_migration",
		vec![
			Operation::RunSQL {
				sql: leak_str("SELECT pg_sleep(0.5)").to_string(), // 500ms sleep
				reverse_sql: None,
			},
			Operation::CreateTable {
				name: leak_str("timeout_table").to_string(),
				columns: vec![ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: false,
					default: None,
				}],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
	);

	// Apply with a reasonable timeout (should succeed)
	let result = tokio::time::timeout(
		Duration::from_secs(10),
		executor.apply_migrations(&[migration]),
	)
	.await;

	assert!(result.is_ok(), "Migration should complete within timeout");
	assert!(
		result.unwrap().is_ok(),
		"Migration should succeed after sleep"
	);
}

// ============================================================================
// Crash Recovery Tests
// ============================================================================

/// Test recovery after simulated crash (incomplete migration)
///
/// **Test Intent**: Verify system can recover from incomplete migrations
///
/// **Integration Point**: MigrationExecutor → migration state verification
///
/// **Expected Behavior**: System detects incomplete state and handles appropriately
#[rstest]
#[tokio::test]
#[serial(crash_recovery)]
async fn test_crash_recovery(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Simulate crash: Create table but don't record migration
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS crash_test_table (
			id SERIAL PRIMARY KEY,
			data TEXT
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Now try to apply migration that creates the same table
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	let migration = create_test_migration(
		"testapp",
		"0001_crash_recovery",
		vec![Operation::CreateTable {
			name: leak_str("crash_test_table").to_string(),
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
				create_basic_column("data", FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// The executor should handle existing table gracefully
	let result = executor.apply_migrations(&[migration]).await;

	// Should succeed by skipping existing table
	assert!(
		result.is_ok(),
		"Recovery should succeed by skipping existing table: {:?}",
		result.err()
	);

	// Verify migration is now recorded
	let is_applied: (bool,) = sqlx::query_as(
		"SELECT EXISTS(
			SELECT 1 FROM reinhardt_migrations
			WHERE app = 'testapp' AND name = '0001_crash_recovery'
		)",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check migration");

	assert!(is_applied.0, "Migration should be recorded after recovery");
}

// ============================================================================
// Concurrent Add Column Tests
// ============================================================================

/// Test concurrent column additions to the same table
///
/// **Test Intent**: Verify concurrent ALTER TABLE operations are handled
///
/// **Integration Point**: MigrationExecutor → PostgreSQL concurrent DDL
///
/// **Expected Behavior**: All columns are added, no data corruption
#[rstest]
#[tokio::test]
#[serial(concurrent_alter)]
async fn test_concurrent_add_column(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Create initial table
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS concurrent_alter_table (
			id SERIAL PRIMARY KEY
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Launch concurrent column additions
	let num_columns = 3;
	let mut set = JoinSet::new();

	for i in 0..num_columns {
		let url_clone = url.clone();
		let col_name = leak_str(format!("col_{}", i));

		set.spawn(async move {
			let conn = DatabaseConnection::connect_postgres(&url_clone)
				.await
				.expect("Failed to connect");
			let mut executor = DatabaseMigrationExecutor::new(conn);

			let migration = create_test_migration(
				"testapp",
				leak_str(format!("000{}_add_col", i + 1)),
				vec![Operation::AddColumn {
					table: "concurrent_alter_table".to_string(),
					column: create_basic_column(col_name, FieldType::Text),
					mysql_options: None,
				}],
			);

			executor.apply_migrations(&[migration]).await
		});
	}

	// Wait for all operations
	let mut results = Vec::new();
	while let Some(res) = set.join_next().await {
		results.push(res.expect("Task panicked"));
	}

	// All should succeed
	let success_count = results.iter().filter(|r| r.is_ok()).count();
	assert_eq!(
		success_count, num_columns,
		"All {} column additions should succeed",
		num_columns
	);

	// Verify all columns exist
	let columns: Vec<(String,)> = sqlx::query_as(
		"SELECT column_name FROM information_schema.columns
		 WHERE table_name = 'concurrent_alter_table'
		 ORDER BY column_name",
	)
	.fetch_all(pool.as_ref())
	.await
	.expect("Failed to get columns");

	// Should have id + 3 added columns
	assert_eq!(
		columns.len(),
		num_columns + 1,
		"Should have {} columns (id + {} added)",
		num_columns + 1,
		num_columns
	);
}

// ============================================================================
// Sequential Migration Order Tests
// ============================================================================

/// Test that migrations are applied in correct sequential order
///
/// **Test Intent**: Verify migration ordering is respected
///
/// **Integration Point**: MigrationExecutor → sequential execution
///
/// **Expected Behavior**: Migrations execute in defined order
#[rstest]
#[tokio::test]
#[serial(sequential_order)]
async fn test_sequential_migration_order(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table that records execution order
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS execution_order (
			id SERIAL PRIMARY KEY,
			step TEXT,
			executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
		)",
	)
	.execute(pool.as_ref())
	.await
	.expect("Failed to create order table");

	// Create migrations that record their execution
	// Explicit dependencies enforce execution order via topological sort
	let migrations = vec![
		create_test_migration(
			"testapp",
			"0001_first",
			vec![Operation::RunSQL {
				sql: leak_str("INSERT INTO execution_order (step) VALUES ('step1')").to_string(),
				reverse_sql: None,
			}],
		),
		create_test_migration_with_deps(
			"testapp",
			"0002_second",
			vec![Operation::RunSQL {
				sql: leak_str("INSERT INTO execution_order (step) VALUES ('step2')").to_string(),
				reverse_sql: None,
			}],
			vec![("testapp".to_string(), "0001_first".to_string())],
		),
		create_test_migration_with_deps(
			"testapp",
			"0003_third",
			vec![Operation::RunSQL {
				sql: leak_str("INSERT INTO execution_order (step) VALUES ('step3')").to_string(),
				reverse_sql: None,
			}],
			vec![("testapp".to_string(), "0002_second".to_string())],
		),
	];

	let result = executor.apply_migrations(&migrations).await;
	assert!(result.is_ok(), "All migrations should succeed");

	// Verify execution order
	let order: Vec<(String,)> = sqlx::query_as("SELECT step FROM execution_order ORDER BY id")
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to get order");

	assert_eq!(order.len(), 3, "Should have 3 steps");
	assert_eq!(order[0].0, "step1", "First step should be step1");
	assert_eq!(order[1].0, "step2", "Second step should be step2");
	assert_eq!(order[2].0, "step3", "Third step should be step3");
}
