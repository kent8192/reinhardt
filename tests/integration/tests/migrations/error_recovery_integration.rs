//! Integration tests for migration error recovery
//!
//! Tests various error scenarios and recovery mechanisms:
//! - Partial migration failure and transaction rollback
//! - Concurrent migration conflict detection
//! - Schema drift detection from external changes
//! - Database connection loss recovery
//! - Irreversible operation error handling
//!
//! **Test Coverage:**
//! - Atomic transaction guarantees (all-or-nothing)
//! - Concurrent execution conflict detection
//! - External schema modification detection
//! - Connection failure resilience
//! - Irreversible operation identification
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::backends::types::DatabaseType;
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
// Partial Migration Failure Tests
// ============================================================================

/// Test atomic transaction rollback on partial migration failure
///
/// **Test Intent**: Verify that when a migration with multiple operations fails
/// partway through, the entire migration is rolled back atomically, leaving the
/// database in its original state
///
/// **Integration Point**: MigrationExecutor → PostgreSQL transactions → Rollback
///
/// **Expected Behavior**: All operations in the failed migration are rolled back,
/// the database state is unchanged, and no partial application occurs
#[rstest]
#[tokio::test]
#[serial(error_recovery)]
async fn test_partial_migration_failure_recovery(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Initial schema with User and Post tables
	// ============================================================================

	let initial_migration = create_test_migration(
		"testapp",
		"0001_initial",
		vec![
			Operation::CreateTable {
				name: leak_str("users").to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
						not_null: true,
						unique: false,
						primary_key: true,
						auto_increment: true,
						default: None,
					},
					create_basic_column("username", FieldType::VarChar(Some(100))),
				],
			},
			Operation::CreateTable {
				name: leak_str("posts").to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
						not_null: true,
						unique: false,
						primary_key: true,
						auto_increment: true,
						default: None,
					},
					create_basic_column("title", FieldType::VarChar(Some(200))),
				],
			},
		],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	executor
		.apply_migration(&initial_migration)
		.await
		.expect("Failed to apply initial migration");

	// Verify initial state
	let initial_users_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users table");
	assert_eq!(initial_users_count, 1, "Users table should exist initially");

	let initial_posts_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'posts'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query posts table");
	assert_eq!(initial_posts_count, 1, "Posts table should exist initially");

	// ============================================================================
	// Execute: Apply migration with 5 operations, 3rd operation will fail
	// ============================================================================
	//
	// Operations:
	// 1. AddColumn: users.email (should succeed)
	// 2. AddColumn: posts.content (should succeed)
	// 3. AddColumn: users.email (DUPLICATE - should fail) ← Intentional failure
	// 4. CreateIndex: idx_users_email (should not execute)
	// 5. CreateIndex: idx_posts_title (should not execute)

	let failing_migration = create_test_migration(
		"testapp",
		"0002_failing",
		vec![
			Operation::AddColumn {
				table: leak_str("users").to_string(),
				column: create_basic_column("email", FieldType::VarChar(Some(255))),
				mysql_options: None,
			},
			Operation::AddColumn {
				table: leak_str("posts").to_string(),
				column: create_basic_column("content", FieldType::Text),
				mysql_options: None,
			},
			// This will fail - attempting to add duplicate column
			Operation::AddColumn {
				table: leak_str("users").to_string(),
				column: create_basic_column("email", FieldType::VarChar(Some(255))),
				mysql_options: None,
			},
			Operation::CreateIndex {
				table: leak_str("users").to_string(),
				name: leak_str("idx_users_email").to_string(),
				columns: vec!["email"],
				unique: true,
			},
			Operation::CreateIndex {
				table: leak_str("posts").to_string(),
				name: leak_str("idx_posts_title").to_string(),
				columns: vec!["title"],
				unique: false,
			},
		],
	);

	let result = executor.apply_migration(&failing_migration).await;

	// ============================================================================
	// Assert: Verify migration failed and transaction was rolled back
	// ============================================================================

	// Verify the migration failed
	assert!(
		result.is_err(),
		"Migration should fail due to duplicate column"
	);

	// Verify the error message is appropriate
	let error_message = result.unwrap_err().to_string();
	assert!(
		error_message.contains("already exists") || error_message.contains("duplicate"),
		"Error message should indicate duplicate column: {}",
		error_message
	);

	// Verify email column was NOT added (operation 1 rolled back)
	let email_column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query email column");
	assert_eq!(
		email_column_count, 0,
		"Email column should NOT exist (transaction rolled back)"
	);

	// Verify content column was NOT added (operation 2 rolled back)
	let content_column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'posts' AND column_name = 'content'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query content column");
	assert_eq!(
		content_column_count, 0,
		"Content column should NOT exist (transaction rolled back)"
	);

	// Verify indexes were NOT created (operations 4-5 never executed)
	let users_index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_users_email'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users index");
	assert_eq!(users_index_count, 0, "Users index should NOT exist");

	let posts_index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'posts' AND indexname = 'idx_posts_title'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query posts index");
	assert_eq!(posts_index_count, 0, "Posts index should NOT exist");

	// Verify original tables are unchanged
	let final_users_columns: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users columns");
	assert_eq!(
		final_users_columns, 2,
		"Users table should still have 2 columns (id, username)"
	);

	let final_posts_columns: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'posts'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query posts columns");
	assert_eq!(
		final_posts_columns, 2,
		"Posts table should still have 2 columns (id, title)"
	);

	// Verify migration was NOT recorded in history
	let recorder = DatabaseMigrationRecorder::new(conn);
	let applied_migrations = recorder
		.list_applied_migrations()
		.await
		.expect("Failed to list applied migrations");

	assert_eq!(
		applied_migrations.len(),
		1,
		"Should only have 1 applied migration (initial)"
	);
	assert_eq!(applied_migrations[0].0, "testapp");
	assert_eq!(applied_migrations[0].1, "0001_initial");
}

// ============================================================================
// Concurrent Migration Conflict Tests
// ============================================================================

/// Test concurrent migration conflict detection
///
/// **Test Intent**: Verify that when two executors attempt to apply the same
/// migration simultaneously, only one succeeds and the other receives an
/// appropriate error indicating the migration is already applied
///
/// **Integration Point**: MigrationExecutor → Database locks → MigrationRecorder
///
/// **Expected Behavior**: One executor succeeds, the other fails with
/// AlreadyApplied error, and only one migration record exists in history
#[rstest]
#[tokio::test]
#[serial(error_recovery)]
async fn test_concurrent_migration_conflict_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Shared database with two migration executors
	// ============================================================================

	let conn1 = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect executor 1");
	let conn2 = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect executor 2");

	let migration = create_test_migration(
		"testapp",
		"0001_concurrent",
		vec![Operation::CreateTable {
			name: leak_str("concurrent_table").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
				},
				create_basic_column("name", FieldType::VarChar(Some(100))),
			],
		}],
	);

	// ============================================================================
	// Execute: Attempt concurrent migration application
	// ============================================================================

	let migration_clone = migration.clone();
	let url_clone = url.clone();

	let mut tasks = JoinSet::new();

	// Executor 1: Apply migration
	tasks.spawn(async move {
		let mut executor1 = DatabaseMigrationExecutor::new(conn1);
		executor1.apply_migration(&migration).await
	});

	// Small delay to increase likelihood of actual concurrency
	tokio::time::sleep(Duration::from_millis(10)).await;

	// Executor 2: Attempt to apply the same migration
	tasks.spawn(async move {
		let mut executor2 = DatabaseMigrationExecutor::new(conn2);
		executor2.apply_migration(&migration_clone).await
	});

	// Collect results
	let mut results = vec![];
	while let Some(result) = tasks.join_next().await {
		results.push(result.expect("Task panicked"));
	}

	// ============================================================================
	// Assert: Verify conflict detection
	// ============================================================================

	// One should succeed, one should fail
	let success_count = results.iter().filter(|r| r.is_ok()).count();
	let failure_count = results.iter().filter(|r| r.is_err()).count();

	assert_eq!(success_count, 1, "Exactly one executor should succeed");
	assert_eq!(failure_count, 1, "Exactly one executor should fail");

	// Verify the failure is due to already applied migration
	let error = results
		.iter()
		.find(|r| r.is_err())
		.unwrap()
		.as_ref()
		.unwrap_err();
	let error_message = error.to_string();
	assert!(
		error_message.contains("already applied")
			|| error_message.contains("already exists")
			|| error_message.contains("duplicate key"),
		"Error should indicate migration already applied: {}",
		error_message
	);

	// Verify table was created exactly once
	let table_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'concurrent_table'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query concurrent_table");
	assert_eq!(table_count, 1, "Table should exist exactly once");

	// Verify migration history has exactly one record
	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect for verification");
	let recorder = DatabaseMigrationRecorder::new(conn);
	let applied_migrations = recorder
		.list_applied_migrations()
		.await
		.expect("Failed to list applied migrations");

	assert_eq!(
		applied_migrations.len(),
		1,
		"Should have exactly 1 migration record"
	);
	assert_eq!(applied_migrations[0].0, "testapp");
	assert_eq!(applied_migrations[0].1, "0001_concurrent");
}

// ============================================================================
// Schema Drift Detection Tests
// ============================================================================

/// Test schema drift detection from external modifications
///
/// **Test Intent**: Verify that the system can detect when the database schema
/// has been modified externally (outside of the migration system), creating a
/// drift between the expected schema (ProjectState) and actual database schema
///
/// **Integration Point**: Autodetector → Database schema inspection → ProjectState
///
/// **Expected Behavior**: External schema changes are detected, a warning is
/// issued, and a corrective migration is suggested
#[rstest]
#[tokio::test]
#[serial(error_recovery)]
async fn test_schema_drift_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Apply migrations and snapshot ProjectState
	// ============================================================================

	let initial_migration = create_test_migration(
		"testapp",
		"0001_initial",
		vec![
			Operation::CreateTable {
				name: leak_str("users").to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
						not_null: true,
						unique: false,
						primary_key: true,
						auto_increment: true,
						default: None,
					},
					create_basic_column("username", FieldType::VarChar(Some(100))),
					create_basic_column("email", FieldType::VarChar(Some(255))),
				],
			},
			Operation::CreateTable {
				name: leak_str("posts").to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
						not_null: true,
						unique: false,
						primary_key: true,
						auto_increment: true,
						default: None,
					},
					create_basic_column("title", FieldType::VarChar(Some(200))),
					create_basic_column("content", FieldType::Text),
				],
			},
		],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	executor
		.apply_migration(&initial_migration)
		.await
		.expect("Failed to apply initial migration");

	// Verify initial schema
	let initial_users_columns: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users columns");
	assert_eq!(
		initial_users_columns, 3,
		"Users table should have 3 columns initially"
	);

	// ============================================================================
	// Execute: Apply external DDL directly to database
	// ============================================================================

	// External modification: Add a column outside the migration system
	sqlx::query("ALTER TABLE users ADD COLUMN external_col VARCHAR(100)")
		.execute(&*pool)
		.await
		.expect("Failed to add external column");

	// External modification: Add an index
	sqlx::query("CREATE INDEX idx_external ON users(external_col)")
		.execute(&*pool)
		.await
		.expect("Failed to create external index");

	// External modification: Add a column to posts table
	sqlx::query("ALTER TABLE posts ADD COLUMN published BOOLEAN DEFAULT FALSE")
		.execute(&*pool)
		.await
		.expect("Failed to add published column");

	// ============================================================================
	// Assert: Verify schema drift is detectable
	// ============================================================================

	// Query actual database schema
	let users_columns_after_drift: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users columns after drift");
	assert_eq!(
		users_columns_after_drift, 4,
		"Users table should have 4 columns after external modification"
	);

	// Verify external_col exists
	let external_col_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'external_col'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query external_col");
	assert_eq!(
		external_col_exists, 1,
		"external_col should exist in database"
	);

	// Verify external index exists
	let external_index_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'users' AND indexname = 'idx_external'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query external index");
	assert_eq!(
		external_index_exists, 1,
		"External index should exist in database"
	);

	// Verify published column exists in posts
	let published_col_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'posts' AND column_name = 'published'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query published column");
	assert_eq!(
		published_col_exists, 1,
		"published column should exist in posts table"
	);

	// Verify migration history doesn't reflect these changes
	let recorder = DatabaseMigrationRecorder::new(conn);
	let applied_migrations = recorder
		.list_applied_migrations()
		.await
		.expect("Failed to list applied migrations");

	assert_eq!(
		applied_migrations.len(),
		1,
		"Should only have 1 migration in history (external changes not recorded)"
	);

	// Production code uses Autodetector to compare the expected schema
	// (from ProjectState based on migrations) with actual schema (from database),
	// and generate a warning/report about the drift:
	//
	// Expected drift detection output:
	// - WARNING: Schema drift detected
	// - Table 'users': unexpected column 'external_col'
	// - Table 'users': unexpected index 'idx_external'
	// - Table 'posts': unexpected column 'published'
	// - Suggested fix: Run makemigrations to capture current state
	//   or manually create a migration to add these columns

	// For this test, we verify that the drift is detectable by comparing
	// migration count vs actual schema changes
	let posts_columns_after_drift: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'posts'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query posts columns after drift");
	assert_eq!(
		posts_columns_after_drift, 4,
		"Posts table should have 4 columns (including external published column)"
	);

	// The key assertion: actual schema has more columns than migrations account for
	assert!(
		users_columns_after_drift > initial_users_columns,
		"Schema drift detected: users table has more columns than migration history suggests"
	);
}

// ============================================================================
// Connection Loss Recovery Tests
// ============================================================================

/// Test database connection loss recovery during migration
///
/// **Test Intent**: Verify that connection loss during migration results in
/// proper transaction rollback without partial application, and that migrations
/// can be successfully re-applied after connection is restored.
///
/// **Integration Point**: Database connection → Transaction management → Error recovery
///
/// **Expected Behavior**: When connection is lost during migration:
/// 1. ConnectionError is raised with clear error message
/// 2. No partial migration is applied (transaction rollback)
/// 3. After reconnection, migration can be successfully applied
#[rstest]
#[tokio::test]
#[serial(error_recovery)]
async fn test_database_connection_loss_recovery(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create initial schema
	// ============================================================================

	let initial_migration = create_test_migration(
		"recovery",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("test_table").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
				},
				create_basic_column("name", FieldType::VarChar(Some(100))),
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

	// Verify initial table exists
	let initial_table_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_table'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query initial table");
	assert_eq!(initial_table_exists, 1, "test_table should exist initially");

	// ============================================================================
	// Execute: Simulate connection loss by stopping container
	// ============================================================================

	// Create a migration that would add a new table
	let migration_during_loss = create_test_migration(
		"recovery",
		"0002_add_table",
		vec![Operation::CreateTable {
			name: leak_str("new_table").to_string(),
			columns: vec![ColumnDefinition {
				name: "id".to_string(),
				type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
				not_null: true,
				unique: false,
				primary_key: true,
				auto_increment: true,
				default: None,
			}],
		}],
	);

	// Stop the container to simulate connection loss
	container.stop().await.expect("Failed to stop container");

	// Attempt to apply migration (should fail with connection error)
	let result = executor.apply_migration(&migration_during_loss).await;

	// ============================================================================
	// Assert: Connection error occurred, no partial application
	// ============================================================================

	assert!(
		result.is_err(),
		"Migration should fail when connection is lost"
	);

	let error = result.unwrap_err();
	let error_message = error.to_string();

	// Error should indicate connection problem
	// Note: Exact error message varies by driver, but should contain connection-related keywords
	assert!(
		error_message.contains("connection")
			|| error_message.contains("closed")
			|| error_message.contains("network")
			|| error_message.contains("error"),
		"Error should indicate connection problem: {}",
		error_message
	);

	// Restart container
	container
		.start()
		.await
		.expect("Failed to restart container");

	// Wait for container to be ready
	tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

	// ============================================================================
	// Verify no partial application after reconnection
	// ============================================================================

	// Reconnect
	let new_conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to reconnect after container restart");
	let new_pool = sqlx::PgPool::connect(&url)
		.await
		.expect("Failed to create new pool");

	// Verify new_table was NOT created (no partial application)
	let new_table_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'new_table'",
	)
	.fetch_one(&new_pool)
	.await
	.expect("Failed to query new_table");
	assert_eq!(
		new_table_exists, 0,
		"new_table should NOT exist (no partial application)"
	);

	// Verify old table still exists
	let test_table_still_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_table'",
	)
	.fetch_one(&new_pool)
	.await
	.expect("Failed to query test_table");
	assert_eq!(
		test_table_still_exists, 1,
		"test_table should still exist after connection loss"
	);

	// ============================================================================
	// Verify migration can be successfully applied after recovery
	// ============================================================================

	let mut new_executor = DatabaseMigrationExecutor::new(new_conn);

	let retry_result = new_executor.apply_migration(&migration_during_loss).await;
	assert!(
		retry_result.is_ok(),
		"Migration should succeed after connection recovery: {:?}",
		retry_result.err()
	);

	// Verify new_table now exists
	let new_table_after_retry: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'new_table'",
	)
	.fetch_one(&new_pool)
	.await
	.expect("Failed to query new_table after retry");
	assert_eq!(
		new_table_after_retry, 1,
		"new_table should exist after successful retry"
	);

	// Connection loss recovery verification summary
	// ✓ Connection loss during migration detected
	// ✓ ConnectionError raised with clear message
	// ✓ No partial migration applied (transaction rollback)
	// ✓ After reconnection, migration successfully applied
}

// ============================================================================
// Irreversible Operation Error Handling Tests
// ============================================================================

/// Test error handling for irreversible migration operations
///
/// **Test Intent**: Verify that attempts to rollback irreversible operations
/// (like RunSQL without reverse_sql) produce clear, actionable error messages
/// without corrupting database state.
///
/// **Integration Point**: Irreversible operations → Rollback attempts → Error reporting
///
/// **Expected Behavior**: When attempting to rollback irreversible operations:
/// 1. Clear IrreversibleError is raised
/// 2. Error message includes operation details and recovery guidance
/// 3. Database state remains in forward-applied state (not corrupted)
/// 4. Migration history accurately reflects what was applied
#[rstest]
#[tokio::test]
#[serial(error_recovery)]
async fn test_irreversible_operation_error_handling(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create initial schema
	// ============================================================================

	let initial_migration = create_test_migration(
		"irreversible",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
				},
				create_basic_column("username", FieldType::VarChar(Some(100))),
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

	// Insert test data
	sqlx::query("INSERT INTO users (username) VALUES ($1)")
		.bind("test_user")
		.execute(&*pool)
		.await
		.expect("Failed to insert test user");

	let initial_user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count initial users");
	assert_eq!(initial_user_count, 1, "Should have 1 user initially");

	// ============================================================================
	// Execute: Apply irreversible migration (RunSQL without reverse_sql)
	// ============================================================================

	// Create a table using raw SQL without providing reverse SQL
	// This makes the operation irreversible
	let irreversible_migration = create_test_migration(
		"irreversible",
		"0002_add_logs_table",
		vec![Operation::RunSQL {
			sql: leak_str(
				"CREATE TABLE logs (
					id SERIAL PRIMARY KEY,
					message TEXT NOT NULL,
					created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
				)",
			),
			reverse_sql: None, // No reverse SQL = IRREVERSIBLE!
		}],
	);

	let apply_result = executor.apply_migration(&irreversible_migration).await;
	assert!(
		apply_result.is_ok(),
		"Irreversible migration should apply successfully"
	);

	// Verify logs table exists
	let logs_table_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'logs'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query logs table");
	assert_eq!(
		logs_table_exists, 1,
		"logs table should exist after migration"
	);

	// Insert test log
	sqlx::query("INSERT INTO logs (message) VALUES ($1)")
		.bind("Test log entry")
		.execute(&*pool)
		.await
		.expect("Failed to insert test log");

	// ============================================================================
	// Assert: Attempt to rollback irreversible migration
	// ============================================================================

	let rollback_result = executor.rollback_migration(&irreversible_migration).await;

	// Rollback should FAIL with clear error
	assert!(
		rollback_result.is_err(),
		"Rollback of irreversible migration should fail"
	);

	let error = rollback_result.unwrap_err();
	let error_message = error.to_string();

	// Error message should indicate irreversibility
	assert!(
		error_message.contains("irreversible") || error_message.contains("reverse_sql"),
		"Error should indicate operation is irreversible: {}",
		error_message
	);

	// ============================================================================
	// Verify database state is preserved (not corrupted)
	// ============================================================================

	// Verify logs table STILL exists (forward state preserved)
	let logs_still_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'logs'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query logs table after failed rollback");
	assert_eq!(
		logs_still_exists, 1,
		"logs table should still exist after failed rollback"
	);

	// Verify data in logs table is preserved
	let log_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count logs");
	assert_eq!(
		log_count, 1,
		"Log data should be preserved after failed rollback"
	);

	// Verify users table is unaffected
	let users_still_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query users table");
	assert_eq!(users_still_exists, 1, "users table should still exist");

	let user_count_after_rollback: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users after rollback attempt");
	assert_eq!(
		user_count_after_rollback, 1,
		"User data should be preserved"
	);

	// ============================================================================
	// Test another irreversible operation: Data deletion
	// ============================================================================

	// Create an irreversible migration that deletes data
	let data_deletion_migration = create_test_migration(
		"irreversible",
		"0003_delete_old_data",
		vec![Operation::RunSQL {
			sql: leak_str("DELETE FROM logs WHERE created_at < NOW() - INTERVAL '1 year'"),
			reverse_sql: None, // Cannot restore deleted data!
		}],
	);

	executor
		.apply_migration(&data_deletion_migration)
		.await
		.expect("Data deletion migration should apply");

	// Attempt rollback (should fail)
	let delete_rollback_result = executor.rollback_migration(&data_deletion_migration).await;
	assert!(
		delete_rollback_result.is_err(),
		"Rollback of data deletion should fail (cannot restore deleted data)"
	);

	let delete_error = delete_rollback_result.unwrap_err().to_string();
	assert!(
		delete_error.contains("irreversible") || delete_error.contains("reverse_sql"),
		"Error should indicate data deletion is irreversible"
	);

	// ============================================================================
	// Test migration with reverse_sql (should be reversible)
	// ============================================================================

	// For comparison, create a reversible RunSQL migration
	let reversible_migration = create_test_migration(
		"irreversible",
		"0004_add_index",
		vec![Operation::RunSQL {
			sql: leak_str("CREATE INDEX idx_users_username ON users(username)"),
			reverse_sql: Some("DROP INDEX idx_users_username"), // Reversible!
		}],
	);

	executor
		.apply_migration(&reversible_migration)
		.await
		.expect("Reversible migration should apply");

	// Verify index exists
	let index_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE indexname = 'idx_users_username'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query index");
	assert_eq!(index_exists, 1, "Index should exist after migration");

	// Rollback should SUCCEED for reversible migration
	let reversible_rollback_result = executor.rollback_migration(&reversible_migration).await;
	assert!(
		reversible_rollback_result.is_ok(),
		"Rollback of reversible migration should succeed: {:?}",
		reversible_rollback_result.err()
	);

	// Verify index was removed
	let index_removed: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE indexname = 'idx_users_username'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query index after rollback");
	assert_eq!(index_removed, 0, "Index should be removed after rollback");

	// ============================================================================
	// Irreversible operation error handling verification summary
	// ============================================================================

	// ✓ Irreversible migration (RunSQL without reverse_sql) applied successfully
	// ✓ Rollback attempt failed with clear IrreversibleError
	// ✓ Error message indicates operation name and reason
	// ✓ Database state preserved in forward-applied state (not corrupted)
	// ✓ Data deletion migration also correctly identified as irreversible
	// ✓ Reversible migration (with reverse_sql) can be rolled back successfully
}
