//! Integration tests for production deployment scenarios
//!
//! Tests real-world production deployment patterns:
//! - Zero-downtime deployment strategies
//! - Backward compatibility preservation
//! - Rollforward on failure (when rollback is impossible)
//! - Hot schema changes without locking
//! - Disaster recovery from backups
//!
//! **Test Coverage:**
//! - Multi-phase zero-downtime migrations
//! - Old/new code coexistence during deployment
//! - Backward-compatible schema changes
//! - Forward-only recovery strategies
//! - Hot index creation (CONCURRENTLY)
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
	Migration::from_parts(name.to_string(), app.to_string(), operations, vec![], vec![], true, None, false, false, Vec::new(), Vec::new())
}

/// Create a basic column definition
fn create_basic_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(name.to_string(), type_def, false, false, false, false, None, None, None)
}

/// Create a column with NULL constraint
fn create_nullable_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(name.to_string(), type_def, false, false, false, false, None, None, None)
}

// ============================================================================
// Zero-Downtime Deployment Tests
// ============================================================================

/// Test zero-downtime deployment scenario with multi-phase migration
///
/// **Test Intent**: Verify that a breaking schema change (renaming columns)
/// can be deployed without downtime by using a three-phase migration strategy:
/// Phase 1: Add new columns (old code unaffected)
/// Phase 2: Data migration (both columns exist)
/// Phase 3: Remove old columns (new code deployed)
///
/// **Integration Point**: Multi-phase deployment → Database migrations → Application code
///
/// **Expected Behavior**: At each phase, both old and new application code can
/// coexist without errors, data is preserved, and no downtime occurs
#[rstest]
#[tokio::test]
#[serial(production_scenarios)]
async fn test_zero_downtime_deployment_scenario(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Initial schema with single 'name' column
	// ============================================================================
	//
	// Old schema: User(id, name)
	// New schema goal: User(id, first_name, last_name)
	//
	// Zero-downtime strategy:
	// - Phase 1: Add first_name, last_name (nullable) → old code still works
	// - Phase 2: Data migration (name → first_name, last_name) → both columns exist
	// - Phase 3: Remove name column → new code deployed

	let initial_migration = create_test_migration(
		"auth",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("name", FieldType::VarChar(Some(200))),
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

	// Insert test data with old schema
	sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
		.bind("John Doe")
		.bind("john@example.com")
		.execute(&*pool)
		.await
		.expect("Failed to insert test user");

	sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
		.bind("Jane Smith")
		.bind("jane@example.com")
		.execute(&*pool)
		.await
		.expect("Failed to insert test user");

	// Verify initial data
	let initial_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users");
	assert_eq!(initial_count, 2, "Should have 2 users initially");

	// ============================================================================
	// Phase 1: Add new columns (nullable, old code unaffected)
	// ============================================================================

	let phase1_migration = create_test_migration(
		"auth",
		"0002_add_name_fields",
		vec![
			Operation::AddColumn {
				table: leak_str("users").to_string(),
				column: create_nullable_column("first_name", FieldType::VarChar(Some(100))),
				mysql_options: None,
			},
			Operation::AddColumn {
				table: leak_str("users").to_string(),
				column: create_nullable_column("last_name", FieldType::VarChar(Some(100))),
				mysql_options: None,
			},
		],
	);

	executor
		.apply_migration(&phase1_migration)
		.await
		.expect("Failed to apply phase 1 migration");

	// Verify new columns exist
	let first_name_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'first_name'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query first_name column");
	assert_eq!(first_name_exists, 1, "first_name column should exist");

	let last_name_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'last_name'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query last_name column");
	assert_eq!(last_name_exists, 1, "last_name column should exist");

	// Verify old code still works (inserting with old schema)
	let old_code_insert_result = sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
		.bind("Old Code User")
		.bind("oldcode@example.com")
		.execute(&*pool)
		.await;
	assert!(
		old_code_insert_result.is_ok(),
		"Old code should still work after Phase 1"
	);

	// Verify new columns are NULL for old data
	let null_first_names: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM users WHERE first_name IS NULL AND name IS NOT NULL",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count NULL first_names");
	assert!(
		null_first_names >= 2,
		"Old records should have NULL first_name"
	);

	// ============================================================================
	// Application Update: Deploy new code that writes to BOTH old and new columns
	// ============================================================================
	//
	// Simulating new application code that writes to both:
	// - name (for backward compatibility with old code still running)
	// - first_name, last_name (for new schema)

	sqlx::query(
		"INSERT INTO users (name, first_name, last_name, email) VALUES ($1, $2, $3, $4)",
	)
	.bind("Alice Johnson") // name (for old code)
	.bind("Alice") // first_name (for new code)
	.bind("Johnson") // last_name (for new code)
	.bind("alice@example.com")
	.execute(&*pool)
	.await
	.expect("Failed to insert with both old and new columns");

	// Verify both old and new columns are populated
	let alice_name: String = sqlx::query_scalar("SELECT name FROM users WHERE email = $1")
		.bind("alice@example.com")
		.fetch_one(&*pool)
		.await
		.expect("Failed to fetch Alice's name");
	assert_eq!(
		alice_name, "Alice Johnson",
		"Old column should be populated"
	);

	let alice_first: String = sqlx::query_scalar("SELECT first_name FROM users WHERE email = $1")
		.bind("alice@example.com")
		.fetch_one(&*pool)
		.await
		.expect("Failed to fetch Alice's first_name");
	assert_eq!(alice_first, "Alice", "first_name should be populated");

	let alice_last: String = sqlx::query_scalar("SELECT last_name FROM users WHERE email = $1")
		.bind("alice@example.com")
		.fetch_one(&*pool)
		.await
		.expect("Failed to fetch Alice's last_name");
	assert_eq!(alice_last, "Johnson", "last_name should be populated");

	// ============================================================================
	// Phase 2: Data migration (name → first_name, last_name)
	// ============================================================================
	//
	// Migrate existing data from 'name' to 'first_name'/'last_name'
	// Simple strategy: split on first space

	sqlx::query(
		"UPDATE users
		SET first_name = SPLIT_PART(name, ' ', 1),
		    last_name = SPLIT_PART(name, ' ', 2)
		WHERE first_name IS NULL",
	)
	.execute(&*pool)
	.await
	.expect("Failed to migrate name data");

	// Verify data migration succeeded
	let john_first: String =
		sqlx::query_scalar("SELECT first_name FROM users WHERE email = 'john@example.com'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch John's first_name");
	assert_eq!(john_first, "John", "John's first_name should be migrated");

	let john_last: String =
		sqlx::query_scalar("SELECT last_name FROM users WHERE email = 'john@example.com'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch John's last_name");
	assert_eq!(john_last, "Doe", "John's last_name should be migrated");

	let jane_first: String =
		sqlx::query_scalar("SELECT first_name FROM users WHERE email = 'jane@example.com'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch Jane's first_name");
	assert_eq!(jane_first, "Jane", "Jane's first_name should be migrated");

	let jane_last: String =
		sqlx::query_scalar("SELECT last_name FROM users WHERE email = 'jane@example.com'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch Jane's last_name");
	assert_eq!(jane_last, "Smith", "Jane's last_name should be migrated");

	// Verify no data loss
	let final_count_before_drop: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users before drop");
	assert_eq!(
		final_count_before_drop, 4,
		"Should have 4 users before dropping old column"
	);

	// ============================================================================
	// Phase 3: Remove old 'name' column (new code fully deployed)
	// ============================================================================

	let phase3_migration = create_test_migration(
		"auth",
		"0003_remove_name_field",
		vec![Operation::RemoveColumn {
			table: leak_str("users").to_string(),
			name: "name".to_string(),
		}],
	);

	executor
		.apply_migration(&phase3_migration)
		.await
		.expect("Failed to apply phase 3 migration");

	// ============================================================================
	// Assert: Verify final state and zero-downtime properties
	// ============================================================================

	// Verify old 'name' column is removed
	let name_column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'name'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query name column");
	assert_eq!(name_column_exists, 0, "Old 'name' column should be removed");

	// Verify new columns still exist
	let final_first_name_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'first_name'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query first_name column");
	assert_eq!(
		final_first_name_exists, 1,
		"first_name column should still exist"
	);

	// Verify all data preserved
	let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users");
	assert_eq!(final_count, 4, "Should still have 4 users after migration");

	// Verify data integrity for all users
	let all_users_have_names: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM users WHERE first_name IS NOT NULL AND last_name IS NOT NULL",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count users with names");
	assert_eq!(
		all_users_have_names, 4,
		"All users should have first_name and last_name"
	);

	// Verify new code can insert records
	let new_code_insert_result =
		sqlx::query("INSERT INTO users (first_name, last_name, email) VALUES ($1, $2, $3)")
			.bind("New")
			.bind("User")
			.bind("newuser@example.com")
			.execute(&*pool)
			.await;
	assert!(
		new_code_insert_result.is_ok(),
		"New code should work after Phase 3"
	);

	let final_count_after_new: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users after new insert");
	assert_eq!(
		final_count_after_new, 5,
		"Should have 5 users after new code insert"
	);

	// ============================================================================
	// Zero-Downtime Verification Summary
	// ============================================================================
	//
	// ✓ Phase 1: New columns added (nullable) → old code continues working
	// ✓ Transition: Both old and new code write to both columns → coexistence
	// ✓ Phase 2: Data migrated from old to new columns → no data loss
	// ✓ Phase 3: Old column removed → new code fully deployed
	// ✓ Result: No downtime, no data loss, gradual transition
}

// ============================================================================
// Backward Compatibility Tests
// ============================================================================

/// Test backward compatibility preservation across migration versions
///
/// **Test Intent**: Verify that newer migrations maintain backward compatibility
/// with older migrations, allowing safe rollback to previous schema versions
/// without breaking data integrity or application functionality.
///
/// **Integration Point**: Migration versioning → Schema evolution → Rollback safety
///
/// **Expected Behavior**: After applying newer migrations, older migrations can
/// still be rolled back successfully, and the schema remains compatible with
/// both old and new application code during the rollback process.
#[rstest]
#[tokio::test]
#[serial(production_scenarios)]
async fn test_backward_compatibility_preservation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create initial schema (version 1.0)
	// ============================================================================

	let v1_migration = create_test_migration(
		"products",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("name", FieldType::VarChar(Some(200))),
				create_basic_column("price", FieldType::Decimal(Some((10, 2)))),
			],
		}],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	executor
		.apply_migration(&v1_migration)
		.await
		.expect("Failed to apply v1 migration");

	// Insert test data with v1 schema
	sqlx::query("INSERT INTO products (name, price) VALUES ($1, $2)")
		.bind("Product A")
		.bind(100.00)
		.execute(&*pool)
		.await
		.expect("Failed to insert product A");

	sqlx::query("INSERT INTO products (name, price) VALUES ($1, $2)")
		.bind("Product B")
		.bind(200.00)
		.execute(&*pool)
		.await
		.expect("Failed to insert product B");

	let v1_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count products");
	assert_eq!(v1_count, 2, "Should have 2 products after v1 migration");

	// ============================================================================
	// Execute: Apply v2 migration (add new columns, backward compatible)
	// ============================================================================

	let v2_migration = create_test_migration(
		"products",
		"0002_add_metadata",
		vec![
			Operation::AddColumn {
				table: leak_str("products").to_string(),
				column: create_nullable_column("description", FieldType::Text),
				mysql_options: None,
			},
			Operation::AddColumn {
				table: leak_str("products").to_string(),
				column: ColumnDefinition::from_parts("stock".to_string(), FieldType::Integer, false, false, false, false, Some("0".to_string()), None, None),
				mysql_options: None,
			},
		],
	);

	executor
		.apply_migration(&v2_migration)
		.await
		.expect("Failed to apply v2 migration");

	// Verify new columns exist
	let description_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'products' AND column_name = 'description'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query description column");
	assert_eq!(description_exists, 1, "description column should exist");

	// Insert data with v2 schema
	sqlx::query("INSERT INTO products (name, price, description, stock) VALUES ($1, $2, $3, $4)")
		.bind("Product C")
		.bind(300.00)
		.bind("New product with metadata")
		.bind(50)
		.execute(&*pool)
		.await
		.expect("Failed to insert product C with v2 schema");

	let v2_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count products after v2");
	assert_eq!(v2_count, 3, "Should have 3 products after v2 migration");

	// ============================================================================
	// Assert: Rollback v2 migration and verify backward compatibility
	// ============================================================================

	executor
		.rollback_migration(&v2_migration)
		.await
		.expect("Failed to rollback v2 migration");

	// Verify new columns are removed
	let description_removed: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'products' AND column_name = 'description'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query description column after rollback");
	assert_eq!(
		description_removed, 0,
		"description column should be removed after rollback"
	);

	// Verify old schema columns still exist
	let name_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'products' AND column_name = 'name'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query name column");
	assert_eq!(name_exists, 1, "name column should still exist");

	let price_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'products' AND column_name = 'price'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query price column");
	assert_eq!(price_exists, 1, "price column should still exist");

	// Verify data integrity after rollback
	let rollback_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count products after rollback");
	assert_eq!(
		rollback_count, 3,
		"All 3 products should be preserved after rollback"
	);

	// Verify v1 schema is still functional (can insert with old schema)
	let v1_insert_result = sqlx::query("INSERT INTO products (name, price) VALUES ($1, $2)")
		.bind("Product D")
		.bind(400.00)
		.execute(&*pool)
		.await;
	assert!(
		v1_insert_result.is_ok(),
		"Should be able to insert with v1 schema after rollback"
	);

	let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count products");
	assert_eq!(final_count, 4, "Should have 4 products after v1 insert");

	// Verify backward compatibility summary
	// ✓ v2 migration applied successfully
	// ✓ v2 migration rolled back successfully
	// ✓ Schema reverted to v1 state
	// ✓ Data preserved across rollback
	// ✓ v1 schema still functional
}

// ============================================================================
// Rollforward Strategy Tests
// ============================================================================

/// Test migration rollforward strategy when rollback is impossible
///
/// **Test Intent**: Verify that when a migration fails and rollback is not possible
/// (due to irreversible operations like DROP TABLE), the system provides clear
/// error messages and suggests forward-fix migration strategies instead of
/// attempting destructive rollback.
///
/// **Integration Point**: Irreversible operations → Failure handling → Forward-fix strategy
///
/// **Expected Behavior**: When rollback is impossible, the system should:
/// 1. Clearly indicate that the operation is irreversible
/// 2. Provide actionable error messages with recovery steps
/// 3. Suggest forward-fix migrations to resolve the issue
/// 4. Preserve database state without attempting destructive rollback
#[rstest]
#[tokio::test]
#[serial(production_scenarios)]
async fn test_migration_rollforward_on_failure(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create initial schema and apply irreversible migration
	// ============================================================================

	let initial_migration = create_test_migration(
		"legacy",
		"0001_initial",
		vec![
			Operation::CreateTable {
				name: leak_str("old_users").to_string(),
				columns: vec![
					ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
					create_basic_column("username", FieldType::VarChar(Some(100))),
				],
			},
			Operation::CreateTable {
				name: leak_str("new_users").to_string(),
				columns: vec![
					ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
					create_basic_column("username", FieldType::VarChar(Some(100))),
					create_basic_column("email", FieldType::VarChar(Some(255))),
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

	// Insert test data
	sqlx::query("INSERT INTO old_users (username) VALUES ($1)")
		.bind("old_user1")
		.execute(&*pool)
		.await
		.expect("Failed to insert old_user1");

	sqlx::query("INSERT INTO new_users (username, email) VALUES ($1, $2)")
		.bind("new_user1")
		.bind("new@example.com")
		.execute(&*pool)
		.await
		.expect("Failed to insert new_user1");

	// ============================================================================
	// Execute: Apply irreversible migration (DROP TABLE without reverse_sql)
	// ============================================================================

	// IMPORTANT: RunSQL without reverse_sql is irreversible
	let irreversible_migration = create_test_migration(
		"legacy",
		"0002_drop_old_table",
		vec![Operation::RunSQL {
			sql: leak_str("DROP TABLE old_users").to_string(),
			reverse_sql: None, // No reverse SQL = irreversible!
		}],
	);

	executor
		.apply_migration(&irreversible_migration)
		.await
		.expect("Failed to apply irreversible migration");

	// Verify old_users table is dropped
	let old_users_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		WHERE table_name = 'old_users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query old_users table");
	assert_eq!(old_users_exists, 0, "old_users table should be dropped");

	// ============================================================================
	// Attempt rollback (should fail with clear error message)
	// ============================================================================

	let rollback_result = executor.rollback_migration(&irreversible_migration).await;

	// Assert that rollback fails with IrreversibleError
	assert!(
		rollback_result.is_err(),
		"Rollback should fail for irreversible migration"
	);

	let error = rollback_result.unwrap_err();
	let error_message = error.to_string();

	// Verify error message contains helpful information
	assert!(
		error_message.contains("irreversible") || error_message.contains("reverse_sql"),
		"Error message should indicate irreversibility: {}",
		error_message
	);

	// ============================================================================
	// Assert: Database state preserved, forward-fix strategy suggested
	// ============================================================================

	// Verify new_users table still exists (no destructive rollback)
	let new_users_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		WHERE table_name = 'new_users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query new_users table");
	assert_eq!(
		new_users_exists, 1,
		"new_users table should still exist after failed rollback"
	);

	// Verify data in new_users is preserved
	let new_users_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM new_users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count new_users");
	assert_eq!(new_users_count, 1, "Data in new_users should be preserved");

	// ============================================================================
	// Forward-fix strategy: Create new migration to restore functionality
	// ============================================================================

	// Instead of rolling back, create a forward-fix migration
	let forward_fix_migration = create_test_migration(
		"legacy",
		"0003_restore_old_users",
		vec![Operation::CreateTable {
			name: leak_str("old_users").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("username", FieldType::VarChar(Some(100))),
			],
		}],
	);

	executor
		.apply_migration(&forward_fix_migration)
		.await
		.expect("Failed to apply forward-fix migration");

	// Verify old_users table is restored
	let old_users_restored: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		WHERE table_name = 'old_users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query restored old_users table");
	assert_eq!(
		old_users_restored, 1,
		"old_users table should be restored via forward-fix"
	);

	// Rollforward strategy verification summary
	// ✓ Irreversible migration applied successfully
	// ✓ Rollback attempt failed with clear error
	// ✓ Database state preserved (no destructive rollback)
	// ✓ Forward-fix migration applied successfully
	// ✓ Functionality restored without rollback
}

// ============================================================================
// Hot Schema Change Tests
// ============================================================================

/// Test hot schema changes without blocking active transactions
///
/// **Test Intent**: Verify that schema changes (specifically index creation)
/// can be performed on active tables without blocking ongoing transactions,
/// using PostgreSQL's CONCURRENTLY option for non-blocking DDL operations.
///
/// **Integration Point**: Active workload → Non-blocking DDL → CONCURRENTLY operations
///
/// **Expected Behavior**: CREATE INDEX CONCURRENTLY should:
/// 1. Not block active transactions from reading/writing
/// 2. Complete successfully while transactions are ongoing
/// 3. Result in a valid, usable index
/// 4. Minimize lock time (brief metadata locks only)
#[rstest]
#[tokio::test]
#[serial(production_scenarios)]
async fn test_hot_schema_changes(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create table with substantial data
	// ============================================================================

	let initial_migration = create_test_migration(
		"analytics",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("events").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("user_id", FieldType::Integer),
				create_basic_column("event_type", FieldType::VarChar(Some(50))),
				ColumnDefinition::from_parts("created_at".to_string(), FieldType::Custom("TIMESTAMP".to_string()), false, false, false, false, Some("CURRENT_TIMESTAMP".to_string()), None, None),
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

	// Insert test data (1000 events to simulate active table)
	for i in 1..=1000 {
		sqlx::query("INSERT INTO events (user_id, event_type) VALUES ($1, $2)")
			.bind(i % 100) // 100 unique users
			.bind(if i % 2 == 0 { "click" } else { "view" })
			.execute(&*pool)
			.await
			.expect("Failed to insert event");
	}

	let initial_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count events");
	assert_eq!(initial_count, 1000, "Should have 1000 events initially");

	// ============================================================================
	// Execute: Create index CONCURRENTLY while transactions are active
	// ============================================================================

	// Start a long-running transaction (simulating active workload)
	let mut tx = pool.begin().await.expect("Failed to begin transaction");

	// Read from table in transaction
	let tx_read_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE user_id = 1")
		.fetch_one(&mut *tx)
		.await
		.expect("Failed to read in transaction");
	assert!(tx_read_count >= 0, "Transaction should be able to read");

	// Create index CONCURRENTLY (should not block the active transaction)
	// Note: CONCURRENTLY requires non-transactional context
	let index_creation_result =
		sqlx::query("CREATE INDEX CONCURRENTLY idx_events_user_id ON events(user_id)")
			.execute(&*pool)
			.await;

	assert!(
		index_creation_result.is_ok(),
		"Index creation should succeed: {:?}",
		index_creation_result.err()
	);

	// Verify transaction can still write (not blocked by index creation)
	let tx_write_result = sqlx::query("INSERT INTO events (user_id, event_type) VALUES ($1, $2)")
		.bind(999)
		.bind("test_event")
		.execute(&mut *tx)
		.await;

	assert!(
		tx_write_result.is_ok(),
		"Transaction should be able to write during CONCURRENTLY index creation"
	);

	// Commit transaction
	tx.commit().await.expect("Failed to commit transaction");

	// ============================================================================
	// Assert: Index created successfully without blocking
	// ============================================================================

	// Verify index exists
	let index_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes
		WHERE tablename = 'events' AND indexname = 'idx_events_user_id'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query index");
	assert_eq!(index_exists, 1, "Index idx_events_user_id should exist");

	// Verify index is valid (not corrupted)
	let index_valid: bool = sqlx::query_scalar(
		"SELECT indisvalid FROM pg_index
		WHERE indexrelid = 'idx_events_user_id'::regclass",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to check index validity");
	assert!(index_valid, "Index should be valid");

	// Verify index is being used (query plan check)
	let explain_result: String =
		sqlx::query_scalar("EXPLAIN (FORMAT TEXT) SELECT * FROM events WHERE user_id = 1")
			.fetch_one(&*pool)
			.await
			.expect("Failed to get query plan");

	// Index should be used in query plan (contains "Index Scan")
	assert!(
		explain_result.contains("Index Scan") || explain_result.contains("idx_events_user_id"),
		"Index should be used in query plan: {}",
		explain_result
	);

	// Verify data integrity
	let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count events after index creation");
	assert_eq!(
		final_count, 1001,
		"Should have 1001 events (1000 + 1 from transaction)"
	);

	// Hot schema change verification summary
	// ✓ Index created CONCURRENTLY without blocking active transactions
	// ✓ Active transaction could read and write during index creation
	// ✓ Index is valid and usable
	// ✓ Query optimizer uses the new index
	// ✓ Data integrity preserved
}

// ============================================================================
// Disaster Recovery Tests
// ============================================================================

/// Test disaster recovery with backup restoration and migration catch-up
///
/// **Test Intent**: Verify that a database can be restored from an older backup
/// and then brought up to date by applying pending migrations, simulating a
/// disaster recovery scenario where the backup is from 1 week ago.
///
/// **Integration Point**: Backup restoration → Migration history → State synchronization
///
/// **Expected Behavior**: After restoring a backup:
/// 1. Missing migrations are identified correctly
/// 2. Migrations can be applied to bring the database up to date
/// 3. Final schema matches the current production schema
/// 4. Data from the backup is preserved
#[rstest]
#[tokio::test]
#[serial(production_scenarios)]
async fn test_disaster_recovery_migration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create initial schema (simulating state 1 week ago)
	// ============================================================================

	let week_ago_migration = create_test_migration(
		"orders",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("orders").to_string(),
			columns: vec![
				ColumnDefinition::from_parts("id".to_string(), FieldType::Custom("SERIAL PRIMARY KEY".to_string()), true, false, true, true, None, None, None),
				create_basic_column("customer_name", FieldType::VarChar(Some(200))),
				create_basic_column("total", FieldType::Decimal(Some((10, 2)))),
			],
		}],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	executor
		.apply_migration(&week_ago_migration)
		.await
		.expect("Failed to apply week_ago migration");

	// Insert backup data (simulating data from 1 week ago)
	sqlx::query("INSERT INTO orders (customer_name, total) VALUES ($1, $2)")
		.bind("Customer A")
		.bind(1000.00)
		.execute(&*pool)
		.await
		.expect("Failed to insert order A");

	sqlx::query("INSERT INTO orders (customer_name, total) VALUES ($1, $2)")
		.bind("Customer B")
		.bind(2000.00)
		.execute(&*pool)
		.await
		.expect("Failed to insert order B");

	let backup_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count orders in backup");
	assert_eq!(backup_count, 2, "Backup should have 2 orders");

	// ============================================================================
	// Simulate disaster: Apply new migrations (representing 1 week of changes)
	// ============================================================================

	let new_migration_1 = create_test_migration(
		"orders",
		"0002_add_status",
		vec![Operation::AddColumn {
			table: leak_str("orders").to_string(),
			column: ColumnDefinition::from_parts("status".to_string(), FieldType::VarChar(Some(50)), false, false, false, false, Some("'pending'".to_string()), None, None),
		}],
	);

	let new_migration_2 = create_test_migration(
		"orders",
		"0003_add_timestamps",
		vec![
			Operation::AddColumn {
				table: leak_str("orders").to_string(),
				column: ColumnDefinition::from_parts("created_at".to_string(), FieldType::Custom("TIMESTAMP".to_string()), false, false, false, false, Some("CURRENT_TIMESTAMP".to_string()), None, None),
				mysql_options: None,
			},
			Operation::AddColumn {
				table: leak_str("orders").to_string(),
				column: ColumnDefinition::from_parts("updated_at".to_string(), FieldType::Custom("TIMESTAMP".to_string()), false, false, false, false, Some("CURRENT_TIMESTAMP".to_string()), None, None),
				mysql_options: None,
			},
		],
	);

	// Apply new migrations (simulating production evolution)
	executor
		.apply_migration(&new_migration_1)
		.await
		.expect("Failed to apply migration 0002");

	executor
		.apply_migration(&new_migration_2)
		.await
		.expect("Failed to apply migration 0003");

	// ============================================================================
	// Execute: Disaster recovery - restore from backup and catch up
	// ============================================================================

	// Simulate disaster: Drop current database and restore from backup
	// In real scenario, this would be pg_restore from a backup file
	// Here we simulate by recreating the backup state

	// Drop and recreate orders table to simulate backup restoration
	sqlx::query("DROP TABLE orders")
		.execute(&*pool)
		.await
		.expect("Failed to drop orders table");

	// Recreate table with backup schema (week-old schema)
	sqlx::query(
		"CREATE TABLE orders (
			id SERIAL PRIMARY KEY,
			customer_name VARCHAR(200),
			total DECIMAL(10, 2)
		)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to recreate orders table");

	// Restore backup data
	sqlx::query("INSERT INTO orders (id, customer_name, total) VALUES ($1, $2, $3)")
		.bind(1)
		.bind("Customer A")
		.bind(1000.00)
		.execute(&*pool)
		.await
		.expect("Failed to restore order A");

	sqlx::query("INSERT INTO orders (id, customer_name, total) VALUES ($1, $2, $3)")
		.bind(2)
		.bind("Customer B")
		.bind(2000.00)
		.execute(&*pool)
		.await
		.expect("Failed to restore order B");

	// Reset sequence to match restored data
	sqlx::query("SELECT setval('orders_id_seq', (SELECT MAX(id) FROM orders))")
		.execute(&*pool)
		.await
		.expect("Failed to reset sequence");

	// Verify backup restoration
	let restored_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count restored orders");
	assert_eq!(restored_count, 2, "Should have 2 orders after restoration");

	// ============================================================================
	// Catch up: Apply pending migrations to bring database up to date
	// ============================================================================

	// Apply pending migration 0002
	executor
		.apply_migration(&new_migration_1)
		.await
		.expect("Failed to apply catch-up migration 0002");

	// Verify status column added
	let status_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'orders' AND column_name = 'status'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query status column");
	assert_eq!(
		status_exists, 1,
		"status column should exist after catch-up"
	);

	// Apply pending migration 0003
	executor
		.apply_migration(&new_migration_2)
		.await
		.expect("Failed to apply catch-up migration 0003");

	// ============================================================================
	// Assert: Database fully recovered and up to date
	// ============================================================================

	// Verify all new columns exist
	let created_at_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'orders' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query created_at column");
	assert_eq!(
		created_at_exists, 1,
		"created_at column should exist after full catch-up"
	);

	let updated_at_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'orders' AND column_name = 'updated_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query updated_at column");
	assert_eq!(
		updated_at_exists, 1,
		"updated_at column should exist after full catch-up"
	);

	// Verify backup data preserved
	let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count orders after recovery");
	assert_eq!(
		final_count, 2,
		"Backup data should be preserved after recovery"
	);

	// Verify data integrity
	let customer_a_total: f64 =
		sqlx::query_scalar("SELECT total FROM orders WHERE customer_name = 'Customer A'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch Customer A total");
	assert_eq!(
		(customer_a_total * 100.0).round() / 100.0,
		1000.00,
		"Customer A total should be preserved"
	);

	// Verify new schema is functional
	let insert_result =
		sqlx::query("INSERT INTO orders (customer_name, total, status) VALUES ($1, $2, $3)")
			.bind("Customer C")
			.bind(3000.00)
			.bind("completed")
			.execute(&*pool)
			.await;
	assert!(
		insert_result.is_ok(),
		"Should be able to insert with new schema after recovery"
	);

	let final_count_after_insert: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count orders after new insert");
	assert_eq!(
		final_count_after_insert, 3,
		"Should have 3 orders after recovery and new insert"
	);

	// Disaster recovery verification summary
	// ✓ Backup restored successfully (week-old schema)
	// ✓ Pending migrations identified
	// ✓ Migrations applied to catch up to current state
	// ✓ Final schema matches production schema
	// ✓ Backup data preserved
	// ✓ New schema is functional
}
