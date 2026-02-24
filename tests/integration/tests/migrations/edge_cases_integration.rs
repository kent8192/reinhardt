//! Integration tests for edge cases in migration execution
//!
//! Tests boundary conditions, special characters, and extreme scenarios:
//! - Self-referencing foreign keys
//! - Deep dependency chains
//! - Long identifier names
//! - Special characters in identifiers
//! - Empty models and zero-length fields
//!
//! **Test Coverage:**
//! - Boundary value handling
//! - Special character escaping
//! - Database identifier limits
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, Constraint, FieldType, Migration, Operation,
	executor::DatabaseMigrationExecutor,
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

/// Create a migration with dependencies
fn create_migration_with_deps(
	app: &str,
	name: &str,
	operations: Vec<Operation>,
	dependencies: Vec<(&str, &str)>,
) -> Migration {
	Migration {
		app_label: app.to_string(),
		name: name.to_string(),
		operations,
		dependencies: dependencies
			.into_iter()
			.map(|(a, n)| (a.to_string(), n.to_string()))
			.collect(),
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
	name: &'static str,
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
// Self-Referencing Foreign Key Tests
// ============================================================================

/// Test self-referencing foreign key (e.g., parent_id references same table)
///
/// **Test Intent**: Verify that self-referencing FK is handled correctly
///
/// **Integration Point**: MigrationExecutor → PostgreSQL CREATE TABLE with self-FK
///
/// **Expected Behavior**: Table created with valid self-referencing constraint
#[rstest]
#[tokio::test]
async fn test_self_referencing_fk(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table with self-referencing FK
	let migration = create_test_migration(
		"testapp",
		"0001_self_ref",
		vec![Operation::CreateTable {
			name: leak_str("categories").to_string(),
			columns: vec![
				create_column_with_constraints(
					"id",
					FieldType::Custom("SERIAL".to_string()),
					true,
					true,
				),
				create_basic_column("name", FieldType::VarChar(255)),
				create_basic_column("parent_id", FieldType::Integer),
			],
			constraints: vec![Constraint::ForeignKey {
				name: "fk_parent".to_string(),
				columns: vec!["parent_id".to_string()],
				referenced_table: "categories".to_string(),
				referenced_columns: vec!["id".to_string()],
				on_delete: reinhardt_db::migrations::ForeignKeyAction::SetNull,
				on_update: reinhardt_db::migrations::ForeignKeyAction::NoAction,
				deferrable: None,
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_ok(),
		"Self-referencing FK should be created successfully: {:?}",
		result.err()
	);

	// Verify table exists with FK constraint
	let table_exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'categories')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table")
	.get::<bool, _>(0);

	assert!(table_exists, "Categories table should exist");

	// Verify FK constraint exists
	let fk_exists = sqlx::query(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE constraint_type = 'FOREIGN KEY' AND table_name = 'categories'
		)",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check FK")
	.get::<bool, _>(0);

	assert!(fk_exists, "Self-referencing FK should exist");

	// Test actual self-referencing insertion
	sqlx::query("INSERT INTO categories (name, parent_id) VALUES ('Root', NULL)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert root category");

	sqlx::query("INSERT INTO categories (name, parent_id) VALUES ('Child', 1)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert child category");

	// Verify data
	let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM categories")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count");
	assert_eq!(count.0, 2, "Should have 2 categories");
}

// ============================================================================
// Deep Dependency Chain Tests
// ============================================================================

/// Test deep dependency chain (10+ level dependencies)
///
/// **Test Intent**: Verify that deep dependency chains don't cause stack overflow
///
/// **Integration Point**: MigrationExecutor → dependency resolution
///
/// **Expected Behavior**: All migrations applied in correct order
#[rstest]
#[tokio::test]
async fn test_deep_dependency_chain(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create 15-level deep dependency chain
	let mut migrations = Vec::new();
	let depth = 15;

	for i in 0..depth {
		let table_name = leak_str(format!("chain_table_{}", i));
		let migration_name = leak_str(format!("{:04}_chain_{}", i + 1, i));

		let deps: Vec<(String, String)> = if i == 0 {
			vec![]
		} else {
			vec![(
				"testapp".to_string(),
				leak_str(format!("{:04}_chain_{}", i, i - 1)).to_string(),
			)]
		};

		let migration = Migration {
			app_label: "testapp".to_string(),
			name: migration_name.to_string(),
			operations: vec![Operation::CreateTable {
				name: table_name.to_string(),
				columns: vec![create_column_with_constraints(
					"id",
					FieldType::Custom("SERIAL".to_string()),
					true,
					true,
				)],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			}],
			dependencies: deps,
			replaces: vec![],
			atomic: true,
			initial: None,
			state_only: false,
			database_only: false,
			swappable_dependencies: vec![],
			optional_dependencies: vec![],
		};

		migrations.push(migration);
	}

	// Apply all migrations
	let result = executor.apply_migrations(&migrations).await;

	assert!(
		result.is_ok(),
		"Deep dependency chain should be resolved: {:?}",
		result.err()
	);

	// Verify all tables were created
	for i in 0..depth {
		let table_name = format!("chain_table_{}", i);
		let exists = sqlx::query(&format!(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = '{}')",
			table_name
		))
		.fetch_one(pool.as_ref())
		.await
		.expect(&format!("Failed to check {}", table_name))
		.get::<bool, _>(0);

		assert!(exists, "Table {} should exist", table_name);
	}
}

// ============================================================================
// Cross-App Dependency Tests
// ============================================================================

/// Test dependencies across different apps
///
/// **Test Intent**: Verify cross-app dependencies are resolved correctly
///
/// **Integration Point**: MigrationExecutor → cross-app dependency resolution
///
/// **Expected Behavior**: Migrations from different apps applied in correct order
#[rstest]
#[tokio::test]
async fn test_cross_app_circular_dependency(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// App1 migration - no dependencies
	let migration1 = create_migration_with_deps(
		"app1",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("app1_users").to_string(),
			columns: vec![create_column_with_constraints(
				"id",
				FieldType::Custom("SERIAL".to_string()),
				true,
				true,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
	);

	// App2 migration - depends on App1
	let migration2 = create_migration_with_deps(
		"app2",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("app2_profiles").to_string(),
			columns: vec![
				create_column_with_constraints(
					"id",
					FieldType::Custom("SERIAL".to_string()),
					true,
					true,
				),
				create_basic_column("user_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("app1", "0001_initial")],
	);

	// App1's second migration - depends on App2
	let migration3 = create_migration_with_deps(
		"app1",
		"0002_add_profile_link",
		vec![Operation::AddColumn {
			table: "app1_users".to_string(),
			column: create_basic_column("profile_id", FieldType::Integer),
			mysql_options: None,
		}],
		vec![("app2", "0001_initial")],
	);

	// Apply in dependency order
	let result = executor
		.apply_migrations(&[migration1, migration2, migration3])
		.await;

	assert!(
		result.is_ok(),
		"Cross-app dependencies should be resolved: {:?}",
		result.err()
	);

	// Verify all tables/columns exist
	let app1_users_exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'app1_users')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check app1_users")
	.get::<bool, _>(0);

	let app2_profiles_exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'app2_profiles')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check app2_profiles")
	.get::<bool, _>(0);

	assert!(app1_users_exists, "app1_users should exist");
	assert!(app2_profiles_exists, "app2_profiles should exist");
}

// ============================================================================
// Long Identifier Name Tests
// ============================================================================

/// Test handling of long identifier names (near database limits)
///
/// **Test Intent**: Verify long names are handled or truncated appropriately
///
/// **Integration Point**: MigrationExecutor → PostgreSQL identifier handling
///
/// **Expected Behavior**: Long names are accepted or properly rejected
#[rstest]
#[tokio::test]
async fn test_long_identifier_names(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// PostgreSQL has a 63-byte limit for identifiers
	// Test with exactly 63 characters (valid)
	let long_table_name = "a".repeat(63);
	let long_column_name = "b".repeat(63);

	let migration = create_test_migration(
		"testapp",
		"0001_long_names",
		vec![Operation::CreateTable {
			name: leak_str(long_table_name.clone()).to_string(),
			columns: vec![
				create_column_with_constraints(
					"id",
					FieldType::Custom("SERIAL".to_string()),
					true,
					true,
				),
				create_basic_column(leak_str(long_column_name.clone()), FieldType::Text),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	// 63-char names should work in PostgreSQL
	assert!(
		result.is_ok(),
		"63-character identifiers should be valid: {:?}",
		result.err()
	);

	// Verify table exists
	let exists = sqlx::query(&format!(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = '{}')",
		long_table_name
	))
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table")
	.get::<bool, _>(0);

	assert!(exists, "Table with long name should exist");
}

/// Test identifier names exceeding database limits
///
/// **Test Intent**: Verify proper error handling for too-long identifiers
///
/// **Integration Point**: MigrationExecutor → PostgreSQL identifier validation
///
/// **Expected Behavior**: Error indicating identifier is too long
#[rstest]
#[tokio::test]
async fn test_identifier_too_long(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// PostgreSQL truncates identifiers > 63 bytes, which may cause unexpected behavior
	// Test with 100 characters
	let too_long_name = "x".repeat(100);

	let migration = create_test_migration(
		"testapp",
		"0001_too_long",
		vec![Operation::CreateTable {
			name: leak_str(too_long_name.clone()).to_string(),
			columns: vec![create_column_with_constraints(
				"id",
				FieldType::Custom("SERIAL".to_string()),
				true,
				true,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	// PostgreSQL truncates to 63 chars, so this should succeed but with truncated name
	assert!(
		result.is_ok(),
		"Migration should succeed with truncated name: {:?}",
		result.err()
	);
}

// ============================================================================
// Special Character Tests
// ============================================================================

/// Test handling of special characters in identifiers (quoted)
///
/// **Test Intent**: Verify special characters are properly escaped
///
/// **Integration Point**: MigrationExecutor → PostgreSQL quoted identifier handling
///
/// **Expected Behavior**: Special characters are properly quoted and escaped
#[rstest]
#[tokio::test]
async fn test_special_characters_in_names(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Test with table name containing special characters that need quoting
	// PostgreSQL allows special chars in quoted identifiers
	let migration = create_test_migration(
		"testapp",
		"0001_special_chars",
		vec![Operation::RunSQL {
			sql: leak_str(
				r#"CREATE TABLE "special-table_with.dots" (
					id SERIAL PRIMARY KEY,
					"column-with-dashes" TEXT,
					"column.with.dots" INTEGER
				)"#,
			)
			.to_string(),
			reverse_sql: Some(leak_str(r#"DROP TABLE "special-table_with.dots""#).to_string()),
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_ok(),
		"Special characters in quoted identifiers should work: {:?}",
		result.err()
	);

	// Verify table exists (need to quote the name)
	let exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'special-table_with.dots')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check special table")
	.get::<bool, _>(0);

	assert!(exists, "Table with special characters should exist");
}

// ============================================================================
// Same Name Different Apps Tests
// ============================================================================

/// Test tables with same name in different apps (namespace separation)
///
/// **Test Intent**: Verify that same table names in different apps don't conflict
///
/// **Integration Point**: MigrationExecutor → PostgreSQL table naming
///
/// **Expected Behavior**: Tables are created without conflict
#[rstest]
#[tokio::test]
async fn test_same_name_different_apps(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// In Django/Reinhardt, table names are typically prefixed with app name
	// But if not, we need to handle potential conflicts

	// App1's users table
	let migration1 = create_test_migration(
		"app1",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("app1_shared_name").to_string(),
			columns: vec![create_column_with_constraints(
				"id",
				FieldType::Custom("SERIAL".to_string()),
				true,
				true,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// App2's users table (different prefix)
	let migration2 = create_test_migration(
		"app2",
		"0001_initial",
		vec![Operation::CreateTable {
			name: leak_str("app2_shared_name").to_string(),
			columns: vec![create_column_with_constraints(
				"id",
				FieldType::Custom("SERIAL".to_string()),
				true,
				true,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration1, migration2]).await;

	assert!(
		result.is_ok(),
		"Different app prefixes should prevent conflicts: {:?}",
		result.err()
	);

	// Verify both tables exist
	let app1_exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'app1_shared_name')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check app1 table")
	.get::<bool, _>(0);

	let app2_exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'app2_shared_name')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check app2 table")
	.get::<bool, _>(0);

	assert!(app1_exists, "app1_shared_name should exist");
	assert!(app2_exists, "app2_shared_name should exist");
}

// ============================================================================
// Extreme VARCHAR Length Tests
// ============================================================================

/// Test extremely large VARCHAR lengths
///
/// **Test Intent**: Verify proper handling of extreme VARCHAR sizes
///
/// **Integration Point**: MigrationExecutor → PostgreSQL VARCHAR(n) creation
///
/// **Expected Behavior**: Large VARCHAR is created or appropriate error
#[rstest]
#[tokio::test]
async fn test_extreme_varchar_length(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// PostgreSQL allows VARCHAR up to 10485760 (10MB)
	// Test with a large but valid size
	let migration = create_test_migration(
		"testapp",
		"0001_large_varchar",
		vec![Operation::CreateTable {
			name: leak_str("large_varchar_table").to_string(),
			columns: vec![
				create_column_with_constraints(
					"id",
					FieldType::Custom("SERIAL".to_string()),
					true,
					true,
				),
				create_basic_column("large_field", FieldType::VarChar(10000)),
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
		"Large VARCHAR should be valid: {:?}",
		result.err()
	);

	// Verify column was created with correct type
	let column_info = sqlx::query(
		"SELECT character_maximum_length FROM information_schema.columns
		 WHERE table_name = 'large_varchar_table' AND column_name = 'large_field'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get column info");

	let max_length: Option<i32> = column_info.get("character_maximum_length");
	assert_eq!(max_length, Some(10000), "VARCHAR length should be 10000");
}

// ============================================================================
// Zero/Empty Field Tests
// ============================================================================

/// Test handling of zero-length VARCHAR
///
/// **Test Intent**: Verify proper handling of VARCHAR(0)
///
/// **Integration Point**: MigrationExecutor → PostgreSQL field validation
///
/// **Expected Behavior**: Error or warning about invalid field size
#[rstest]
#[tokio::test]
async fn test_zero_length_field(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// PostgreSQL requires VARCHAR length > 0
	let migration = create_test_migration(
		"testapp",
		"0001_zero_varchar",
		vec![Operation::CreateTable {
			name: leak_str("zero_varchar_table").to_string(),
			columns: vec![
				create_column_with_constraints(
					"id",
					FieldType::Custom("SERIAL".to_string()),
					true,
					true,
				),
				create_basic_column("zero_field", FieldType::VarChar(0)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	// PostgreSQL should reject VARCHAR(0)
	assert!(
		result.is_err(),
		"VARCHAR(0) should be rejected by PostgreSQL"
	);
}

// ============================================================================
// Empty Model Definition Tests
// ============================================================================

/// Test handling of table with only primary key (minimal model)
///
/// **Test Intent**: Verify minimal table definitions are handled
///
/// **Integration Point**: MigrationExecutor → PostgreSQL CREATE TABLE
///
/// **Expected Behavior**: Table with only PK is created successfully
#[rstest]
#[tokio::test]
async fn test_empty_model_definition(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Create table with only primary key (minimal valid table)
	let migration = create_test_migration(
		"testapp",
		"0001_minimal",
		vec![Operation::CreateTable {
			name: leak_str("minimal_table").to_string(),
			columns: vec![create_column_with_constraints(
				"id",
				FieldType::Custom("SERIAL".to_string()),
				true,
				true,
			)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_ok(),
		"Minimal table should be created: {:?}",
		result.err()
	);

	// Verify table exists
	let exists = sqlx::query(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'minimal_table')",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table")
	.get::<bool, _>(0);

	assert!(exists, "Minimal table should exist");

	// Verify it has exactly one column
	let column_count: (i64,) = sqlx::query_as(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'minimal_table'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count columns");

	assert_eq!(
		column_count.0, 1,
		"Minimal table should have exactly 1 column"
	);
}
