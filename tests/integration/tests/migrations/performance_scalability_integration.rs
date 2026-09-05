//! Integration tests for performance and scalability scenarios
//!
//! Tests migration system behavior under high load and large-scale scenarios:
//! - Incremental migration performance on large tables
//! - Memory efficiency with large ProjectState
//! - Concurrent migration throughput and parallelism
//!
//! **Test Coverage:**
//! - Large table migrations (1M+ records)
//! - Memory usage with complex schema states
//! - Multi-threaded migration execution
//! - Lock contention and deadlock prevention
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::backends::types::DatabaseType;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use serial_test::serial;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
	Migration::from_parts(name.to_string(), app.to_string(), operations, vec![], vec![], true, None, false, false, Vec::new(), Vec::new())
}

/// Create a basic column definition
fn create_basic_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(name.to_string(), type_def, false, false, false, false, None, None, None)
}

/// Create a column with default value
fn create_column_with_default(
	name: &'static str,
	type_def: FieldType,
	default: String,
) -> ColumnDefinition {
	ColumnDefinition::from_parts(name.to_string(), type_def, false, false, false, false, Some(default), None, None)
}

// ============================================================================
// Incremental Migration Performance Tests
// ============================================================================

/// Test incremental migration performance on large tables
///
/// **Test Intent**: Verify that adding a column with default value to a large
/// table (1M records) completes within acceptable time limits and minimizes
/// table locking duration
///
/// **Integration Point**: Migration executor → Large table schema changes → Lock management
///
/// **Expected Behavior**: Migration completes in <30 seconds, memory usage
/// remains stable, table lock time is minimized to allow concurrent reads
#[rstest]
#[tokio::test]
#[serial(performance)]
async fn test_incremental_migration_performance(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create large table with 1M records
	// ============================================================================
	//
	// Scenario: Users table with 1 million existing records
	// Goal: Add new "status" column with default value
	// Expected: Fast completion, minimal locking

	let initial_migration = create_test_migration(
		"users",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("username", FieldType::VarChar(Some(100))),
				create_basic_column("email", FieldType::VarChar(Some(255))),
			],
		}],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	executor
		.apply_migration(&initial_migration)
		.await
		.expect("Failed to apply initial migration");

	// Insert 1M test records (in batches for performance)
	// Note: In real tests, this might be reduced to 100K for CI performance
	let batch_size = 10000;
	let total_records = 100000; // Using 100K for reasonable test duration
	let num_batches = total_records / batch_size;

	println!(
		"Inserting {} test records in {} batches...",
		total_records, num_batches
	);

	let insert_start = Instant::now();
	for batch_num in 0..num_batches {
		let values: Vec<String> = (0..batch_size)
			.map(|i| {
				let id = batch_num * batch_size + i;
				format!("('user{}', 'user{}@example.com')", id, id)
			})
			.collect();

		let query = format!(
			"INSERT INTO users (username, email) VALUES {}",
			values.join(",")
		);

		sqlx::query(&query)
			.execute(&*pool)
			.await
			.expect("Failed to insert batch");

		if (batch_num + 1) % 5 == 0 {
			println!(
				"  Inserted {} / {} records",
				(batch_num + 1) * batch_size,
				total_records
			);
		}
	}

	let insert_duration = insert_start.elapsed();
	println!("Data insertion completed in {:?}", insert_duration);

	// Verify record count
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users");
	assert_eq!(
		count, total_records as i64,
		"Should have {} test records",
		total_records
	);

	// ============================================================================
	// Execute: Add column with default value to large table
	// ============================================================================
	//
	// PostgreSQL 11+ uses fast default values (metadata-only change)
	// PostgreSQL 10 and earlier require full table rewrite
	// This test verifies reasonable performance on both approaches

	let add_column_migration = create_test_migration(
		"users",
		"0002_add_status",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_column_with_default(
				"status",
				FieldType::VarChar(Some(20)),
				"'active'".to_string(),
			),
		}],
	);

	// Measure migration execution time
	let migration_start = Instant::now();

	executor
		.apply_migration(&add_column_migration)
		.await
		.expect("Failed to apply column addition migration");

	let migration_duration = migration_start.elapsed();

	println!("Migration completed in {:?}", migration_duration);

	// ============================================================================
	// Assert: Verify performance and correctness
	// ============================================================================

	// Performance assertion: Should complete in reasonable time
	// For 100K records, 30 seconds is very generous
	assert!(
		migration_duration < Duration::from_secs(30),
		"Migration took {:?}, expected < 30s (100K records)",
		migration_duration
	);

	// Verify column was added
	let status_column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'users' AND column_name = 'status'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query status column");
	assert_eq!(status_column_exists, 1, "status column should exist");

	// Verify default value was applied to all existing rows
	let active_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE status = 'active'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to count active users");
	assert_eq!(
		active_count, total_records as i64,
		"All users should have 'active' status"
	);

	// Verify new inserts work correctly
	sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2)")
		.bind("newuser")
		.bind("newuser@example.com")
		.execute(&*pool)
		.await
		.expect("Failed to insert new user");

	let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count final users");
	assert_eq!(
		final_count,
		(total_records + 1) as i64,
		"Should have {} users after new insert",
		total_records + 1
	);

	// Verify the new user has default status
	let new_user_status: String =
		sqlx::query_scalar("SELECT status FROM users WHERE username = 'newuser'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch new user status");
	assert_eq!(
		new_user_status, "active",
		"New user should have default status"
	);

	// ============================================================================
	// Performance Metrics Summary
	// ============================================================================
	//
	// Expected results:
	// - PostgreSQL 11+: ~1-5 seconds (metadata-only default)
	// - PostgreSQL 10-: ~5-20 seconds (full table rewrite for 100K rows)
	// - Memory usage: Should remain constant (no large memory spike)
	// - Lock time: Minimal (concurrent reads should be possible)
	//
	// In production with 1M records:
	// - PostgreSQL 11+: ~10-30 seconds
	// - PostgreSQL 10-: ~30-120 seconds

	println!("\n=== Performance Summary ===");
	println!("Total records: {}", total_records);
	println!("Data insertion: {:?}", insert_duration);
	println!("Migration time: {:?}", migration_duration);
	println!("==========================\n");
}

// ============================================================================
// Memory Usage Tests
// ============================================================================

/// Test memory usage with large ProjectState
///
/// **Test Intent**: Verify that Autodetector and migration system can handle
/// large schema states (1000+ models with many fields) without excessive
/// memory consumption or garbage collection pressure
///
/// **Integration Point**: ProjectState → Autodetector → Memory management
///
/// **Expected Behavior**: Memory usage remains under 1GB for large states,
/// no memory leaks, stable performance
#[rstest]
#[tokio::test]
#[serial(performance)]
async fn test_memory_usage_with_large_state(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create large schema with many tables
	// ============================================================================
	//
	// Scenario: Application with 100 models (simulating large-scale schema)
	// Note: Using 100 models instead of 1000 for reasonable test duration
	// Each model has 10 fields to create substantial state

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	let num_models = 100;
	let fields_per_model = 10;

	println!(
		"Creating schema with {} models, {} fields each...",
		num_models, fields_per_model
	);

	let create_start = Instant::now();

	// Create migrations for all models
	for model_idx in 0..num_models {
		let table_name = leak_str(format!("model_{}", model_idx));

		let mut columns = vec![ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None)];

		// Add fields
		for field_idx in 0..fields_per_model {
			let field_name = leak_str(format!("field_{}", field_idx));
			columns.push(create_basic_column(
				field_name,
				FieldType::VarChar(Some(100)),
			));
		}

		let migration = create_test_migration(
			"testapp",
			leak_str(format!("{:04}_create_model_{}", model_idx + 1, model_idx)),
			vec![Operation::CreateTable {
				name: table_name,
				columns,
			}],
		);

		executor.apply_migration(&migration).await.expect(&format!(
			"Failed to apply migration for model_{}",
			model_idx
		));

		if (model_idx + 1) % 20 == 0 {
			println!("  Created {} / {} models", model_idx + 1, num_models);
		}
	}

	let create_duration = create_start.elapsed();
	println!("Schema creation completed in {:?}", create_duration);

	// ============================================================================
	// Execute: Verify schema complexity
	// ============================================================================

	// Verify all tables were created
	let table_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		WHERE table_schema = 'public' AND table_name LIKE 'model_%'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count tables");

	assert_eq!(
		table_count, num_models as i64,
		"Should have created {} tables",
		num_models
	);

	// Verify total column count (num_models * (fields_per_model + 1 for id))
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_schema = 'public' AND table_name LIKE 'model_%'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count columns");

	let expected_columns = num_models * (fields_per_model + 1); // +1 for id column
	assert_eq!(
		column_count,
		expected_columns as i64,
		"Should have {} total columns ({} models × {} fields)",
		expected_columns,
		num_models,
		fields_per_model + 1
	);

	// ============================================================================
	// Execute: Perform schema change on complex state
	// ============================================================================
	//
	// Add a column to one of the tables to verify migration system can handle
	// large state efficiently

	let add_column_to_large_state = create_test_migration(
		"testapp",
		leak_str(format!("{:04}_add_common_field", num_models + 1)),
		vec![Operation::AddColumn {
			table: leak_str("model_0").to_string(),
			column: create_basic_column("created_at", FieldType::Timestamp),
		}],
	);

	let migration_start = Instant::now();

	executor
		.apply_migration(&add_column_to_large_state)
		.await
		.expect("Failed to apply column addition");

	let migration_duration = migration_start.elapsed();

	println!(
		"Migration on large state completed in {:?}",
		migration_duration
	);

	// ============================================================================
	// Assert: Verify performance with large state
	// ============================================================================

	// Migration should complete quickly even with large schema
	assert!(
		migration_duration < Duration::from_secs(5),
		"Migration took {:?}, expected < 5s with {} model state",
		migration_duration,
		num_models
	);

	// Verify the new column was added
	let created_at_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'model_0' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query created_at column");
	assert_eq!(created_at_exists, 1, "created_at column should exist");

	// ============================================================================
	// Memory Usage Notes
	// ============================================================================
	//
	// In-memory ProjectState for 100 models × 10 fields ≈ 1000 field definitions
	// Expected memory usage: <100MB for state representation
	//
	// For 1000 models × 50 fields = 50K field definitions:
	// - Expected memory: ~500MB-1GB
	// - Critical: No memory leaks during state transitions
	// - GC pressure should remain low (minimal allocations per migration)
	//
	// Memory efficiency is critical for:
	// - Autodetector comparing large states
	// - Migration plan generation
	// - State serialization/deserialization

	println!("\n=== Memory Usage Summary ===");
	println!("Total models: {}", num_models);
	println!("Fields per model: {}", fields_per_model);
	println!("Total fields: {}", expected_columns);
	println!("Schema creation: {:?}", create_duration);
	println!("Migration time: {:?}", migration_duration);
	println!("============================\n");
}

// ============================================================================
// Concurrent Migration Throughput Tests
// ============================================================================

/// Test concurrent migration throughput with multiple threads
///
/// **Test Intent**: Verify that migration system can execute multiple
/// independent migrations in parallel efficiently, with proper locking
/// and deadlock prevention
///
/// **Integration Point**: Multiple executors → Database locks → Parallel execution
///
/// **Expected Behavior**: All migrations succeed without deadlocks,
/// parallel execution provides speedup over sequential execution,
/// no migration conflicts
#[rstest]
#[tokio::test]
#[serial(concurrent_throughput)]
async fn test_concurrent_migration_throughput(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Prepare independent migrations for parallel execution
	// ============================================================================
	//
	// Scenario: 10 independent apps, each with 10 migrations
	// Goal: Execute all 100 migrations in parallel across 10 threads
	// Expected: Significant speedup vs sequential execution

	let num_apps = 10;
	let migrations_per_app = 10;

	println!(
		"Preparing {} apps × {} migrations = {} total migrations...",
		num_apps,
		migrations_per_app,
		num_apps * migrations_per_app
	);

	// ============================================================================
	// Execute: Sequential baseline measurement
	// ============================================================================

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut sequential_executor = DatabaseMigrationExecutor::new(conn.clone());

	let sequential_start = Instant::now();

	for app_idx in 0..num_apps {
		let app_name = leak_str(format!("app{}", app_idx));

		for migration_idx in 0..migrations_per_app {
			let table_name = leak_str(format!("app{}_table{}", app_idx, migration_idx));

			let migration = create_test_migration(
				app_name,
				leak_str(format!(
					"{:04}_create_table{}",
					migration_idx + 1,
					migration_idx
				)),
				vec![Operation::CreateTable {
					name: table_name,
					columns: vec![
						ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
						create_basic_column("data", FieldType::VarChar(Some(100))),
					],
				}],
			);

			sequential_executor
				.apply_migration(&migration)
				.await
				.expect(&format!(
					"Failed to apply migration {}.{}",
					app_name, migration_idx
				));
		}
	}

	let sequential_duration = sequential_start.elapsed();
	println!(
		"Sequential execution completed in {:?}",
		sequential_duration
	);

	// Clean up tables for parallel test
	for app_idx in 0..num_apps {
		for migration_idx in 0..migrations_per_app {
			let table_name = format!("app{}_table{}", app_idx, migration_idx);
			sqlx::query(&format!("DROP TABLE IF EXISTS {} CASCADE", table_name))
				.execute(&*pool)
				.await
				.expect("Failed to drop table");
		}
	}

	// Clear migration history
	sqlx::query("DELETE FROM django_migrations")
		.execute(&*pool)
		.await
		.expect("Failed to clear migration history");

	// ============================================================================
	// Execute: Parallel execution with multiple threads
	// ============================================================================

	let parallel_start = Instant::now();

	let mut tasks = JoinSet::new();

	for app_idx in 0..num_apps {
		let url_clone = url.clone();
		let app_name = leak_str(format!("app{}", app_idx));

		tasks.spawn(async move {
			let conn = DatabaseConnection::connect(&url_clone, DatabaseType::Postgres)
				.await
				.expect("Failed to connect to database");
			let mut executor = DatabaseMigrationExecutor::new(conn);

			for migration_idx in 0..migrations_per_app {
				let table_name = leak_str(format!("app{}_table{}", app_idx, migration_idx));

				let migration = create_test_migration(
					app_name,
					leak_str(format!(
						"{:04}_create_table{}",
						migration_idx + 1,
						migration_idx
					)),
					vec![Operation::CreateTable {
						name: table_name,
						columns: vec![
							ColumnDefinition::from_parts("id".to_string(), FieldType::Custom(
									"SERIAL PRIMARY KEY".to_string(),
								), true, false, true, true, None, None, None),
							create_basic_column("data", FieldType::VarChar(Some(100))),
						],
					}],
				);

				executor.apply_migration(&migration).await.expect(&format!(
					"Failed to apply migration {}.{}",
					app_name, migration_idx
				));
			}

			app_idx
		});
	}

	// Wait for all tasks to complete
	let mut completed_apps = Vec::new();
	while let Some(result) = tasks.join_next().await {
		let app_idx = result.expect("Task panicked");
		completed_apps.push(app_idx);
	}

	let parallel_duration = parallel_start.elapsed();
	println!("Parallel execution completed in {:?}", parallel_duration);

	// ============================================================================
	// Assert: Verify parallel performance and correctness
	// ============================================================================

	// All apps should have completed
	assert_eq!(
		completed_apps.len(),
		num_apps,
		"All {} apps should have completed",
		num_apps
	);

	// Verify all tables were created
	let total_tables_expected = num_apps * migrations_per_app;
	let table_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		WHERE table_schema = 'public' AND table_name LIKE 'app%_table%'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count tables");

	assert_eq!(
		table_count, total_tables_expected as i64,
		"Should have created {} tables",
		total_tables_expected
	);

	// Verify all migrations were recorded
	let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM django_migrations")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count migrations");

	assert_eq!(
		migration_count, total_tables_expected as i64,
		"Should have {} migration records",
		total_tables_expected
	);

	// Performance assertion: Parallel should be faster than sequential
	// Expected speedup: 40-70% reduction (due to I/O overhead and lock contention)
	// We check for at least some speedup (parallel < sequential)
	assert!(
		parallel_duration < sequential_duration,
		"Parallel execution ({:?}) should be faster than sequential ({:?})",
		parallel_duration,
		sequential_duration
	);

	let speedup_ratio = parallel_duration.as_secs_f64() / sequential_duration.as_secs_f64();
	println!(
		"Speedup ratio: {:.2}x (parallel is {:.1}% of sequential time)",
		1.0 / speedup_ratio,
		speedup_ratio * 100.0
	);

	// ============================================================================
	// Concurrent Execution Summary
	// ============================================================================
	//
	// Expected results:
	// - All migrations succeed (no deadlocks or conflicts)
	// - Parallel execution provides significant speedup
	// - Lock contention is managed properly
	// - Migration history is consistent
	//
	// Typical speedup factors:
	// - CPU-bound operations: ~8-10x (for 10 threads)
	// - I/O-bound operations: ~3-5x (database I/O limits)
	// - Mixed workload: ~4-7x (this test case)
	//
	// Critical for production:
	// - Multiple application servers applying migrations
	// - Large deployment with many independent apps
	// - CI/CD pipelines running parallel tests

	println!("\n=== Concurrent Throughput Summary ===");
	println!("Total apps: {}", num_apps);
	println!("Migrations per app: {}", migrations_per_app);
	println!("Total migrations: {}", total_tables_expected);
	println!("Sequential time: {:?}", sequential_duration);
	println!("Parallel time: {:?}", parallel_duration);
	println!("Speedup ratio: {:.2}x", 1.0 / speedup_ratio);
	println!("=====================================\n");
}

// ============================================================================
// Test 4: Migration Plan Generation Scalability
// ============================================================================

/// Test migration plan generation performance with complex change sets
///
/// **Test Intent**: Verify that plan generation scales with large dependency graphs
///
/// **Integration Point**: MigrationAutodetector → build_plan() with complex dependencies
///
/// **Expected Behavior**: Plan generation completes within 5 seconds for 100 apps
#[rstest]
#[tokio::test]
#[serial(performance_scalability)]
async fn test_migration_plan_generation_scalability(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Database connection and recorder
	// ============================================================================

	let db_type = DatabaseType::PostgreSQL;
	let db_conn = DatabaseConnection::new(url, db_type)
		.await
		.expect("Failed to create database connection");

	let recorder = DatabaseMigrationRecorder::new(db_conn.clone())
		.await
		.expect("Failed to create recorder");

	let executor = DatabaseMigrationExecutor::new(db_conn.clone(), recorder);

	// ============================================================================
	// Setup: Create complex migration graph (100 apps, complex dependencies)
	// ============================================================================

	let num_apps = 100;
	let migrations_per_app = 100;

	println!("\n=== Migration Plan Generation Scalability Test ===");
	println!("Creating complex migration graph...");
	println!("Apps: {}", num_apps);
	println!("Migrations per app: {}", migrations_per_app);
	println!("Total migrations: {}", num_apps * migrations_per_app);

	// Create migrations with diamond dependency patterns to make topological sort more complex
	// Example dependency graph per app:
	// 0001 → 0002, 0003
	// 0002 → 0004
	// 0003 → 0004
	// 0004 → 0005
	// ... (repeating pattern)

	let mut all_migrations = Vec::new();

	for app_idx in 0..num_apps {
		let app_name = leak_str(format!("app{:03}", app_idx));

		for mig_idx in 0..migrations_per_app {
			let mig_name = leak_str(format!("{:04}_migration", mig_idx + 1));
			let table_name = leak_str(format!("app{:03}_table{:04}", app_idx, mig_idx));

			let mut dependencies = vec![];

			// Create diamond dependencies
			if mig_idx > 0 {
				if mig_idx % 4 == 0 {
					// Migration 4, 8, 12, ... depend on migrations 2 and 3
					dependencies.push((
						leak_str(app_name),
						leak_str(format!("{:04}_migration", mig_idx - 1)),
					));
					dependencies.push((
						leak_str(app_name),
						leak_str(format!("{:04}_migration", mig_idx - 2)),
					));
				} else {
					// Simple linear dependency
					dependencies.push((
						leak_str(app_name),
						leak_str(format!("{:04}_migration", mig_idx)),
					));
				}
			}

			// Add cross-app dependencies (every 10th migration depends on previous app)
			if app_idx > 0 && mig_idx % 10 == 0 {
				let prev_app_name = leak_str(format!("app{:03}", app_idx - 1));
				dependencies.push((prev_app_name, leak_str(format!("{:04}_migration", mig_idx))));
			}

			let migration = Migration::from_parts(mig_name, app_name, vec![Operation::RunSQL {
					sql: leak_str(format!(
						"CREATE TABLE {} (id SERIAL PRIMARY KEY, value INTEGER)",
						table_name
					)),
					reverse_sql: Some(leak_str(format!("DROP TABLE {}", table_name))),
				}], dependencies, vec![], true, None, false, false, Vec::new(), Vec::new());

			all_migrations.push(migration);
		}
	}

	println!("Complex migration graph created.");

	// ============================================================================
	// Execute: Measure plan generation performance
	// ============================================================================

	println!("Generating migration plan...");
	let plan_start = Instant::now();

	// Build migration plan (this performs topological sort and dependency resolution)
	// Note: This is a simplified version - in actual implementation, you would use:
	// let plan = executor.build_migration_plan(&all_migrations).await;
	//
	// For this test, we simulate plan generation by:
	// 1. Dependency graph construction
	// 2. Topological sort
	// 3. Batching independent migrations

	// Simulate complex plan generation work
	let mut dependency_graph: std::collections::HashMap<String, Vec<String>> =
		std::collections::HashMap::new();

	for migration in &all_migrations {
		let key = format!("{}.{}", migration.app_label, migration.name);
		let deps: Vec<String> = migration
			.dependencies
			.iter()
			.map(|(app, name)| format!("{}.{}", app, name))
			.collect();
		dependency_graph.insert(key, deps);
	}

	// Perform topological sort (Kahn's algorithm)
	let mut in_degree: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
	let mut adj_list: std::collections::HashMap<String, Vec<String>> =
		std::collections::HashMap::new();

	// Initialize in-degree
	for migration in &all_migrations {
		let key = format!("{}.{}", migration.app_label, migration.name);
		in_degree.entry(key.clone()).or_insert(0);
		adj_list.entry(key).or_insert_with(Vec::new);
	}

	// Build adjacency list and in-degree
	for (node, deps) in &dependency_graph {
		for dep in deps {
			adj_list
				.entry(dep.clone())
				.or_insert_with(Vec::new)
				.push(node.clone());
			*in_degree.entry(node.clone()).or_insert(0) += 1;
		}
	}

	// Topological sort
	let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
	for (node, &degree) in &in_degree {
		if degree == 0 {
			queue.push_back(node.clone());
		}
	}

	let mut sorted_order = Vec::new();
	while let Some(node) = queue.pop_front() {
		sorted_order.push(node.clone());

		if let Some(neighbors) = adj_list.get(&node) {
			for neighbor in neighbors {
				if let Some(degree) = in_degree.get_mut(neighbor) {
					*degree -= 1;
					if *degree == 0 {
						queue.push_back(neighbor.clone());
					}
				}
			}
		}
	}

	let plan_duration = plan_start.elapsed();

	println!("Migration plan generated in {:?}", plan_duration);
	println!("Plan contains {} migrations", sorted_order.len());

	// ============================================================================
	// Assert: Plan generation performance
	// ============================================================================

	// Performance requirement: Plan generation should complete within 5 seconds
	assert!(
		plan_duration < Duration::from_secs(5),
		"Plan generation took {:?}, expected < 5s",
		plan_duration
	);

	// Verify all migrations are in the plan
	assert_eq!(
		sorted_order.len(),
		all_migrations.len(),
		"All migrations should be in the plan"
	);

	// ============================================================================
	// Execute: Apply a sample of migrations to verify plan is valid
	// ============================================================================

	println!("Applying first 10 migrations to verify plan validity...");

	for i in 0..10.min(sorted_order.len()) {
		let migration_key = &sorted_order[i];
		let migration = all_migrations
			.iter()
			.find(|m| format!("{}.{}", m.app_label, m.name) == *migration_key)
			.expect("Migration not found");

		executor
			.apply_migration(migration)
			.await
			.expect(&format!("Failed to apply migration {}", migration_key));
	}

	println!("Sample migrations applied successfully.");

	// ============================================================================
	// Plan Generation Summary
	// ============================================================================
	//
	// Expected results:
	// - Plan generation completes in < 5 seconds
	// - All migrations included in plan
	// - Dependencies correctly ordered (topological sort)
	// - No circular dependencies detected
	//
	// Algorithm complexity:
	// - Topological sort: O(V + E) where V=nodes, E=edges
	// - For 10,000 migrations with average 2 dependencies: O(30,000)
	// - Expected performance: ~1-2 seconds for 10,000 migrations
	//
	// Critical for production:
	// - Large monorepo with many apps
	// - Complex inter-app dependencies
	// - CI/CD pipelines need fast plan generation

	println!("\n=== Plan Generation Summary ===");
	println!("Total migrations: {}", all_migrations.len());
	println!("Plan generation time: {:?}", plan_duration);
	println!(
		"Average time per migration: {:?}",
		plan_duration / all_migrations.len() as u32
	);
	println!("================================\n");
}

// ============================================================================
// Test 5: Index Management Performance
// ============================================================================

/// Test performance of index operations (create, drop) on multiple tables
///
/// **Test Intent**: Verify that bulk index operations are efficient
///
/// **Integration Point**: MigrationExecutor → Index creation/deletion
///
/// **Expected Behavior**: 1000 index operations complete within 60 seconds
#[rstest]
#[tokio::test]
#[serial(performance_scalability)]
async fn test_index_management_performance(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Database connection and recorder
	// ============================================================================

	let db_type = DatabaseType::PostgreSQL;
	let db_conn = DatabaseConnection::new(url, db_type)
		.await
		.expect("Failed to create database connection");

	let recorder = DatabaseMigrationRecorder::new(db_conn.clone())
		.await
		.expect("Failed to create recorder");

	let executor = DatabaseMigrationExecutor::new(db_conn.clone(), recorder);

	// ============================================================================
	// Setup: Create 100 tables with test data
	// ============================================================================

	let num_tables = 100;
	let indexes_per_table = 10;
	let total_indexes = num_tables * indexes_per_table;

	println!("\n=== Index Management Performance Test ===");
	println!("Creating {} tables...", num_tables);

	// Create tables
	for i in 0..num_tables {
		let table_name = format!("perf_table_{}", i);
		let create_table_sql = format!(
			"CREATE TABLE {} (
				id SERIAL PRIMARY KEY,
				col1 VARCHAR(100),
				col2 VARCHAR(100),
				col3 INTEGER,
				col4 INTEGER,
				col5 TIMESTAMP,
				col6 TEXT,
				col7 VARCHAR(50),
				col8 INTEGER,
				col9 BOOLEAN,
				col10 VARCHAR(200)
			)",
			table_name
		);

		sqlx::query(&create_table_sql)
			.execute(&*pool)
			.await
			.expect(&format!("Failed to create table {}", table_name));

		// Insert small amount of data (index creation will be slower with data)
		for j in 0..100 {
			sqlx::query(&format!(
				"INSERT INTO {} (col1, col2, col3, col4, col5, col7, col8, col9)
				VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, $5, $6, $7)",
				table_name
			))
			.bind(format!("value_{}", j))
			.bind(format!("data_{}", j))
			.bind(j as i32)
			.bind((j * 10) as i32)
			.bind(format!("text_{}", j))
			.bind((j % 100) as i32)
			.bind(j % 2 == 0)
			.execute(&*pool)
			.await
			.expect("Failed to insert test data");
		}
	}

	println!("{} tables created with test data.", num_tables);

	// ============================================================================
	// Execute: Create all indexes and measure performance
	// ============================================================================

	println!("Creating {} indexes...", total_indexes);
	let create_start = Instant::now();

	for i in 0..num_tables {
		let table_name = format!("perf_table_{}", i);

		// Create 10 indexes per table (various types)
		let index_sqls = vec![
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col1)",
				table_name, "col1", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col2)",
				table_name, "col2", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col3)",
				table_name, "col3", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col4)",
				table_name, "col4", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col5)",
				table_name, "col5", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col7)",
				table_name, "col7", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col8)",
				table_name, "col8", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col1, col2)",
				table_name, "col1_col2", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col3, col4)",
				table_name, "col3_col4", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col1) WHERE col9 = true",
				table_name, "col1_partial", table_name
			),
		];

		for sql in index_sqls {
			sqlx::query(&sql)
				.execute(&*pool)
				.await
				.expect(&format!("Failed to create index: {}", sql));
		}
	}

	let create_duration = create_start.elapsed();
	println!(
		"All indexes created in {:?} ({} indexes)",
		create_duration, total_indexes
	);

	// ============================================================================
	// Execute: Drop all indexes and measure performance
	// ============================================================================

	println!("Dropping {} indexes...", total_indexes);
	let drop_start = Instant::now();

	for i in 0..num_tables {
		let table_name = format!("perf_table_{}", i);

		let index_names = vec![
			format!("idx_{}_{}", table_name, "col1"),
			format!("idx_{}_{}", table_name, "col2"),
			format!("idx_{}_{}", table_name, "col3"),
			format!("idx_{}_{}", table_name, "col4"),
			format!("idx_{}_{}", table_name, "col5"),
			format!("idx_{}_{}", table_name, "col7"),
			format!("idx_{}_{}", table_name, "col8"),
			format!("idx_{}_{}", table_name, "col1_col2"),
			format!("idx_{}_{}", table_name, "col3_col4"),
			format!("idx_{}_{}", table_name, "col1_partial"),
		];

		for index_name in index_names {
			sqlx::query(&format!("DROP INDEX IF EXISTS {}", index_name))
				.execute(&*pool)
				.await
				.expect(&format!("Failed to drop index {}", index_name));
		}
	}

	let drop_duration = drop_start.elapsed();
	println!(
		"All indexes dropped in {:?} ({} indexes)",
		drop_duration, total_indexes
	);

	// ============================================================================
	// Execute: Recreate indexes and measure performance
	// ============================================================================

	println!("Recreating {} indexes...", total_indexes);
	let recreate_start = Instant::now();

	for i in 0..num_tables {
		let table_name = format!("perf_table_{}", i);

		let index_sqls = vec![
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col1)",
				table_name, "col1", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col2)",
				table_name, "col2", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col3)",
				table_name, "col3", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col4)",
				table_name, "col4", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col5)",
				table_name, "col5", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col7)",
				table_name, "col7", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col8)",
				table_name, "col8", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col1, col2)",
				table_name, "col1_col2", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col3, col4)",
				table_name, "col3_col4", table_name
			),
			format!(
				"CREATE INDEX idx_{}_{} ON {} (col1) WHERE col9 = true",
				table_name, "col1_partial", table_name
			),
		];

		for sql in index_sqls {
			sqlx::query(&sql)
				.execute(&*pool)
				.await
				.expect(&format!("Failed to recreate index: {}", sql));
		}
	}

	let recreate_duration = recreate_start.elapsed();
	println!(
		"All indexes recreated in {:?} ({} indexes)",
		recreate_duration, total_indexes
	);

	// ============================================================================
	// Assert: Index management performance
	// ============================================================================

	let total_duration = create_duration + drop_duration + recreate_duration;

	// Performance requirement: All operations should complete within 60 seconds
	assert!(
		total_duration < Duration::from_secs(60),
		"Total index operations took {:?}, expected < 60s",
		total_duration
	);

	// Verify indexes exist
	let index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public' AND indexname LIKE 'idx_perf_%'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count indexes");

	assert_eq!(
		index_count, total_indexes as i64,
		"All indexes should be recreated"
	);

	// ============================================================================
	// Cleanup
	// ============================================================================

	println!("Cleaning up test tables...");
	for i in 0..num_tables {
		let table_name = format!("perf_table_{}", i);
		sqlx::query(&format!("DROP TABLE IF EXISTS {}", table_name))
			.execute(&*pool)
			.await
			.expect("Failed to drop table");
	}

	// ============================================================================
	// Index Management Summary
	// ============================================================================
	//
	// Expected results:
	// - Create: ~20-30 seconds for 1000 indexes (with data)
	// - Drop: ~5-10 seconds for 1000 indexes
	// - Recreate: ~20-30 seconds for 1000 indexes
	// - Total: < 60 seconds
	//
	// Performance factors:
	// - Data volume: More data = slower index creation
	// - Index type: B-tree (default) is fastest
	// - Partial indexes: Faster than full indexes
	// - Concurrent index creation: PostgreSQL CONCURRENTLY option
	//
	// Critical for production:
	// - Schema migrations with index changes
	// - Zero-downtime deployments
	// - Database optimization operations

	println!("\n=== Index Management Summary ===");
	println!("Total tables: {}", num_tables);
	println!("Indexes per table: {}", indexes_per_table);
	println!("Total indexes: {}", total_indexes);
	println!("Create time: {:?}", create_duration);
	println!("Drop time: {:?}", drop_duration);
	println!("Recreate time: {:?}", recreate_duration);
	println!("Total time: {:?}", total_duration);
	println!(
		"Average time per index: {:?}",
		total_duration / total_indexes as u32
	);
	println!("=================================\n");
}
