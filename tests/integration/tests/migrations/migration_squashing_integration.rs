//! Migration Squashing Integration Tests
//!
//! Tests that verify the migration squashing (compression) feature. Squashing combines
//! multiple migrations into a single migration to reduce the number of migration files
//! and improve performance when applying migrations from scratch.
//!
//! **Test Coverage:**
//! - Squashing multiple migrations into one
//! - Preserving dependencies after squashing
//! - replaces attribute verification
//! - Skipping already-applied migrations (--fake-initial equivalent)
//! - Detecting non-squashable operations (RunSQL, RunCode)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//!
//! **Key Concepts:**
//! - **Squashing**: Combining multiple migrations into one optimized migration
//! - **replaces**: Attribute listing which migrations the squashed migration replaces
//! - **--fake-initial**: Django feature to skip migrations if tables already exist
//! - **Non-squashable**: Operations that can't be optimized (e.g., RunSQL with data changes)
//!
//! **Django Equivalent**: `python manage.py squashmigrations app 0001 0010`

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation,
	executor::DatabaseMigrationExecutor,
	squash::{MigrationSquasher, SquashOptions},
};
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

// Utility function for creating static string references from owned strings
fn leak_str(s: impl Into<String>) -> &'static str {
	Box::leak(s.into().into_boxed_str())
}

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Create a migration with dependencies
fn create_migration_with_deps(
	app: &str,
	name: &str,
	operations: Vec<Operation>,
	dependencies: Vec<(&str, &str)>,
	replaces: Vec<(&str, &str)>,
) -> Migration {
	Migration {
		app_label: app.to_string(),
		name: name.to_string(),
		operations,
		dependencies: dependencies
			.into_iter()
			.map(|(a, n)| (a.to_string(), n.to_string()))
			.collect(),
		replaces: replaces
			.into_iter()
			.map(|(a, n)| (a.to_string(), n.to_string()))
			.collect(),
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
// Normal Case Tests - Migration Squashing
// ============================================================================

/// Test squashing 3 migrations into 1
///
/// **Test Intent**: Verify that multiple sequential migrations can be combined
///
/// **Example**:
/// - Before: 0001_create (CREATE TABLE), 0002_add_col1 (ADD COLUMN), 0003_add_col2 (ADD COLUMN)
/// - After: 0001_squashed (CREATE TABLE with both columns)
#[rstest]
#[tokio::test]
async fn test_squash_three_migrations(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create original migrations
	let migration_1 =
		Migration::new("0001_create_users", "app").add_operation(Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});

	let migration_2 = Migration::new("0002_add_name", "app")
		.add_dependency("app", "0001_create_users")
		.add_operation(Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("name", FieldType::VarChar(100)),
			mysql_options: None,
		});

	let migration_3 = Migration::new("0003_add_email", "app")
		.add_dependency("app", "0002_add_name")
		.add_operation(Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("email", FieldType::VarChar(255)),
			mysql_options: None,
		});

	let migrations = vec![migration_1, migration_2, migration_3];

	// Squash the migrations
	let squasher = MigrationSquasher::new();
	let squashed = squasher
		.squash(&migrations, "0001_squashed_0003", SquashOptions::default())
		.expect("Failed to squash migrations");

	// Verify squashed migration properties
	assert_eq!(squashed.name, "0001_squashed_0003");
	assert_eq!(squashed.app_label, "app");
	assert_eq!(squashed.replaces.len(), 3);

	// Apply the squashed migration
	executor
		.apply_migrations(&[squashed])
		.await
		.expect("Failed to apply squashed migration");

	// Verify the schema was created correctly
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'users'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count columns");

	assert_eq!(
		column_count, 3,
		"users table should have 3 columns (id, name, email)"
	);
}

/// Test dependency preservation after squashing
///
/// **Test Intent**: Verify that external dependencies are preserved in squashed migration
///
/// **Example**:
/// - migration_1 depends on external app: ("other_app", "0001_initial")
/// - migration_2 adds column
/// - migration_3 adds another column
/// - Squashed migration must still depend on ("other_app", "0001_initial")
#[rstest]
#[tokio::test]
async fn test_squashing_preserves_external_dependencies(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let _ = postgres_container.await; // Consume the fixture

	// Migration 1: depends on external app
	let migration_1 = Migration::new("0001_initial", "app")
		.add_dependency("other_app", "0001_initial")
		.add_operation(Operation::CreateTable {
			name: leak_str("posts").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});

	// Migration 2: depends on migration 1 (internal dependency)
	let migration_2 = Migration::new("0002_add_title", "app")
		.add_dependency("app", "0001_initial")
		.add_operation(Operation::AddColumn {
			table: leak_str("posts").to_string(),
			column: create_basic_column("title", FieldType::VarChar(200)),
			mysql_options: None,
		});

	// Migration 3: depends on migration 2 (internal dependency)
	let migration_3 = Migration::new("0003_add_content", "app")
		.add_dependency("app", "0002_add_title")
		.add_operation(Operation::AddColumn {
			table: leak_str("posts").to_string(),
			column: create_basic_column("content", FieldType::Text),
			mysql_options: None,
		});

	let migrations = vec![migration_1, migration_2, migration_3];

	// Squash the migrations
	let squasher = MigrationSquasher::new();
	let squashed = squasher
		.squash(&migrations, "0001_squashed_0003", SquashOptions::default())
		.expect("Failed to squash migrations");

	// Verify external dependency is preserved
	assert_eq!(squashed.dependencies.len(), 1);
	assert_eq!(squashed.dependencies[0].0, "other_app");
	assert_eq!(squashed.dependencies[0].1, "0001_initial");

	// Verify replaces list
	assert_eq!(squashed.replaces.len(), 3);
}

/// Test replaces attribute verification
///
/// **Test Intent**: Verify that squashed migration correctly lists replaced migrations
#[rstest]
#[tokio::test]
async fn test_replaces_attribute_verification(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let _ = postgres_container.await; // Consume the fixture

	// Create 5 migrations to squash
	let migrations: Vec<Migration> = (1..=5)
		.map(|i| {
			let name = leak_str(format!("{:04}_migration", i));
			let mut migration = Migration::new(name, "app");
			if i > 1 {
				let prev_name = leak_str(format!("{:04}_migration", i - 1));
				migration = migration.add_dependency("app", prev_name);
			}
			migration.add_operation(Operation::RunSQL {
				sql: leak_str(format!("SELECT {}", i)).to_string(),
				reverse_sql: None,
			})
		})
		.collect();

	// Squash all 5 migrations
	let squasher = MigrationSquasher::new();
	let squashed = squasher
		.squash(&migrations, "0001_squashed_0005", SquashOptions::default())
		.expect("Failed to squash migrations");

	// Verify replaces attribute contains all original migrations
	assert_eq!(squashed.replaces.len(), 5);

	// Verify replaces list is in correct order
	for (i, (app, name)) in squashed.replaces.iter().enumerate() {
		assert_eq!(*app, "app");
		let expected_name = format!("{:04}_migration", i + 1);
		assert_eq!(*name, expected_name);
	}
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test --fake-initial equivalent (skip migrations if tables already exist)
///
/// **Test Intent**: Verify that migrations can be marked as applied without executing SQL
///
/// **Django Feature**: `python manage.py migrate --fake-initial`
/// - If initial migration creates tables that already exist, mark as applied without error
///
/// **Use Case**: Migrating legacy database to reinhardt-db migrations
///
/// **Note**: The executor automatically skips CreateTable operations if the table already exists.
/// This behavior serves as a partial implementation of Django's --fake-initial concept.
#[rstest]
#[tokio::test]
async fn test_fake_initial_skip_existing_tables(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// Manually create a table (simulating legacy database)
	sqlx::query("CREATE TABLE users (id SERIAL PRIMARY KEY, name VARCHAR(100))")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create legacy table");

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration that creates the same table
	let migration = create_migration_with_deps(
		"app",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
		vec![],
	);

	// The executor automatically skips CreateTable operations if the table already exists
	// (see executor.rs lines 554-562: table existence check with continue).
	// This allows smooth integration of legacy databases.
	let result = executor.apply_migrations(&[migration]).await;

	// Migration should succeed because the executor skips CreateTable for existing tables
	assert!(
		result.is_ok(),
		"Migration should succeed, skipping existing table creation"
	);
}

/// Test detection of non-squashable operations
///
/// **Test Intent**: Verify that migrations with RunSQL are preserved (not optimized away)
///
/// **Rationale**: Some operations can't be safely squashed:
/// - RunSQL with data modifications (can't optimize)
/// - RunCode (Rust closures can't be merged)
/// - Operations with state-dependent logic
///
/// **Behavior**: MigrationSquasher preserves RunSQL operations in the squashed migration
#[rstest]
#[tokio::test]
async fn test_detect_non_squashable_operations(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let _ = postgres_container.await; // Consume the fixture

	// Migration 1: CreateTable (optimizable)
	let migration_1 =
		Migration::new("0001_create_users", "app").add_operation(Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		});

	// Migration 2: RunSQL with data insertion (non-optimizable, should be preserved)
	let migration_2 = Migration::new("0002_populate_data", "app")
		.add_dependency("app", "0001_create_users")
		.add_operation(Operation::RunSQL {
			sql: leak_str("INSERT INTO users (id) VALUES (1)").to_string(),
			reverse_sql: None,
		});

	// Migration 3: AddColumn (optimizable)
	let migration_3 = Migration::new("0003_add_email", "app")
		.add_dependency("app", "0002_populate_data")
		.add_operation(Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("email", FieldType::VarChar(255)),
			mysql_options: None,
		});

	let migrations = vec![migration_1, migration_2, migration_3];

	// Squash with optimization enabled
	let squasher = MigrationSquasher::new();
	let squashed = squasher
		.squash(&migrations, "0001_squashed_0003", SquashOptions::default())
		.expect("Failed to squash migrations");

	// Verify RunSQL is preserved in the squashed migration
	let has_run_sql = squashed
		.operations
		.iter()
		.any(|op| matches!(op, Operation::RunSQL { .. }));
	assert!(has_run_sql, "RunSQL operation should be preserved");

	// Verify all 3 operations are present (CreateTable, RunSQL, AddColumn)
	assert_eq!(squashed.operations.len(), 3);
}
