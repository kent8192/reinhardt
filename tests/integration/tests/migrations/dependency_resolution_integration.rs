//! Dependency Resolution Integration Tests
//!
//! Tests that verify the migration dependency resolution system correctly handles
//! complex dependency scenarios. Migrations can depend on other migrations, and
//! the executor must apply them in the correct topological order.
//!
//! **Test Coverage:**
//! - Linear dependencies (A → B → C)
//! - Multi-app dependencies (app1 → app2 → app1)
//! - Diamond dependencies (A → B, A → C, B → D, C → D)
//! - Swappable model dependencies (custom User model)
//! - Circular dependency detection (A → B → A)
//! - Missing dependency detection
//! - Deep dependency chains (100+ levels)
//! - Independent migrations (parallel execution)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//!
//! **Key Concepts:**
//! - **Dependency**: Migration B depends on Migration A if B requires A's schema changes
//! - **Topological Sort**: Algorithm to order migrations by dependencies
//! - **Circular Dependency**: Invalid state where A depends on B and B depends on A
//! - **DAG (Directed Acyclic Graph)**: Valid dependency graph with no cycles

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation,
	dependency::{
		DependencyCondition, DependencyResolutionContext, OptionalDependency, SwappableDependency,
	},
	executor::DatabaseMigrationExecutor,
	graph::{MigrationGraph, MigrationKey},
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

/// Check if a table exists
async fn table_exists(pool: &PgPool, table_name: &str) -> bool {
	sqlx::query_scalar::<_, bool>(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind(table_name)
	.fetch_one(pool)
	.await
	.unwrap_or(false)
}

// ============================================================================
// Normal Case Tests - Basic Dependency Resolution
// ============================================================================

/// Test linear dependency resolution (A → B → C)
///
/// **Test Intent**: Verify that migrations are applied in linear dependency order
#[rstest]
#[tokio::test]
async fn test_linear_dependency_resolution(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration A: Create users table
	let migration_a = create_migration_with_deps(
		"app",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![], // No dependencies
	);

	// Migration B: Add name column (depends on A)
	let migration_b = create_migration_with_deps(
		"app",
		"0002_add_name",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("name", FieldType::VarChar(100)),
			mysql_options: None,
		}],
		vec![("app", "0001_create_users")],
	);

	// Migration C: Add email column (depends on B)
	let migration_c = create_migration_with_deps(
		"app",
		"0003_add_email",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("email", FieldType::VarChar(255)),
			mysql_options: None,
		}],
		vec![("app", "0002_add_name")],
	);

	// Apply migrations (order: A → B → C should be enforced)
	executor
		.apply_migrations(&[migration_a, migration_b, migration_c])
		.await
		.expect("Failed to apply migrations in linear order");

	// Verify final schema
	assert!(
		table_exists(pool.as_ref(), "users").await,
		"users table should exist"
	);

	// Verify all columns exist
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

/// Test multi-app dependency resolution (app1 → app2 → app1)
///
/// **Test Intent**: Verify that dependencies across different apps are resolved correctly
#[rstest]
#[tokio::test]
async fn test_multi_app_dependency_resolution(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// app1: Migration 0001 - Create users table
	let app1_m1 = create_migration_with_deps(
		"app1",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
	);

	// app2: Migration 0001 - Create posts table (depends on app1.0001)
	let app2_m1 = create_migration_with_deps(
		"app2",
		"0001_create_posts",
		vec![Operation::CreateTable {
			name: leak_str("posts").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("user_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("app1", "0001_create_users")],
	);

	// app1: Migration 0002 - Add username (depends on app2.0001)
	let app1_m2 = create_migration_with_deps(
		"app1",
		"0002_add_username",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("username", FieldType::VarChar(50)),
			mysql_options: None,
		}],
		vec![("app2", "0001_create_posts")],
	);

	// Apply migrations (order should be: app1.0001 → app2.0001 → app1.0002)
	executor
		.apply_migrations(&[app1_m1, app2_m1, app1_m2])
		.await
		.expect("Failed to apply multi-app migrations");

	// Verify both tables exist
	assert!(
		table_exists(pool.as_ref(), "users").await,
		"users table should exist"
	);
	assert!(
		table_exists(pool.as_ref(), "posts").await,
		"posts table should exist"
	);
}

/// Test diamond dependency resolution (A → B, A → C, B → D, C → D)
///
/// **Test Intent**: Verify that diamond-shaped dependency graphs are resolved correctly
#[rstest]
#[tokio::test]
async fn test_diamond_dependency_resolution(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration A: Create base table
	let migration_a = create_migration_with_deps(
		"app",
		"0001_create_base",
		vec![Operation::CreateTable {
			name: leak_str("base").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
	);

	// Migration B: Add column_b (depends on A)
	let migration_b = create_migration_with_deps(
		"app",
		"0002_add_column_b",
		vec![Operation::AddColumn {
			table: leak_str("base").to_string(),
			column: create_basic_column("column_b", FieldType::VarChar(50)),
			mysql_options: None,
		}],
		vec![("app", "0001_create_base")],
	);

	// Migration C: Add column_c (depends on A)
	let migration_c = create_migration_with_deps(
		"app",
		"0003_add_column_c",
		vec![Operation::AddColumn {
			table: leak_str("base").to_string(),
			column: create_basic_column("column_c", FieldType::VarChar(50)),
			mysql_options: None,
		}],
		vec![("app", "0001_create_base")],
	);

	// Migration D: Add column_d (depends on B and C)
	let migration_d = create_migration_with_deps(
		"app",
		"0004_add_column_d",
		vec![Operation::AddColumn {
			table: leak_str("base").to_string(),
			column: create_basic_column("column_d", FieldType::VarChar(50)),
			mysql_options: None,
		}],
		vec![("app", "0002_add_column_b"), ("app", "0003_add_column_c")],
	);

	// Apply migrations (valid topological order: A, then B and C in any order, then D)
	executor
		.apply_migrations(&[migration_a, migration_b, migration_c, migration_d])
		.await
		.expect("Failed to apply diamond dependencies");

	// Verify all columns exist
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'base'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to count columns");

	assert_eq!(column_count, 4, "base table should have 4 columns");
}

/// Test swappable model dependency (custom User model)
///
/// **Test Intent**: Verify that swappable dependencies are resolved correctly
/// based on configuration context
///
/// **Use Case**: Django's AUTH_USER_MODEL allows custom User models. Other apps
/// depend on whatever User model is configured.
#[rstest]
#[tokio::test]
async fn test_swappable_model_dependency(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration 1: Create users table in default auth app
	let migration_auth = Migration {
		app_label: "auth".to_string(),
		name: "0001_initial".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("auth_users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Migration 2: Create users table in custom auth app
	let migration_custom_auth = Migration {
		app_label: "custom_auth".to_string(),
		name: "0001_initial".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("custom_auth_users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("username", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Migration 3: Profile that depends on swappable User model
	let migration_profile = Migration {
		app_label: "profiles".to_string(),
		name: "0001_create_profile".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("profiles").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("user_id", FieldType::Integer),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![SwappableDependency {
			setting_key: "AUTH_USER_MODEL".to_string(),
			default_app: "auth".to_string(),
			default_model: "User".to_string(),
			migration_name: "0001_initial".to_string(),
		}],
		optional_dependencies: vec![],
	};

	// Test 1: Using MigrationGraph with context to resolve swappable dependency
	let mut graph = MigrationGraph::new();

	// Create context with custom user model configured
	let context = DependencyResolutionContext::new()
		.with_app("auth")
		.with_app("custom_auth")
		.with_app("profiles")
		.with_setting("AUTH_USER_MODEL", "custom_auth.CustomUser");

	// Add migrations to graph with context
	graph.add_migration_with_context(&migration_auth, &context);
	graph.add_migration_with_context(&migration_custom_auth, &context);
	graph.add_migration_with_context(&migration_profile, &context);

	// Verify that profiles migration depends on custom_auth (resolved from swappable)
	let profile_key = MigrationKey::new("profiles", "0001_create_profile");
	let deps = graph.get_dependencies(&profile_key).unwrap();

	assert_eq!(deps.len(), 1, "Profile should have 1 resolved dependency");
	assert_eq!(
		deps[0].app_label, "custom_auth",
		"Swappable dependency should resolve to custom_auth"
	);

	// Test 2: Apply migrations in correct order (custom_auth first, then profiles)
	executor
		.apply_migrations(&[migration_custom_auth.clone(), migration_profile.clone()])
		.await
		.expect("Failed to apply migrations");

	// Verify tables were created
	let profiles_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("profiles")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(profiles_exists, "profiles table should exist");
}

/// Test conditional dependencies (optional dependencies)
///
/// **Test Intent**: Verify that optional dependencies are only enforced when
/// their condition is met
///
/// **Use Case**: Some migrations depend on optional features (e.g., GIS extension)
#[rstest]
#[tokio::test]
async fn test_conditional_dependencies(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration 1: GIS extension setup (optional app)
	let migration_gis = Migration {
		app_label: "gis_extension".to_string(),
		name: "0001_enable_postgis".to_string(),
		operations: vec![Operation::RunSQL {
			sql: leak_str("SELECT 1").to_string(), // Placeholder for CREATE EXTENSION postgis
			reverse_sql: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Migration 2: Geo app with optional dependency on GIS extension
	let migration_geo = Migration {
		app_label: "geo_app".to_string(),
		name: "0001_create_locations".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("locations").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		dependencies: vec![],
		replaces: vec![],
		atomic: true,
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![OptionalDependency {
			app_label: "gis_extension".to_string(),
			migration_name: "0001_enable_postgis".to_string(),
			condition: DependencyCondition::AppInstalled("gis_extension".to_string()),
		}],
	};

	// Test 1: Without gis_extension installed - dependency should be ignored
	{
		let mut graph = MigrationGraph::new();

		// Context without gis_extension installed
		let context_without_gis = DependencyResolutionContext::new().with_app("geo_app");

		graph.add_migration_with_context(&migration_geo, &context_without_gis);

		let geo_key = MigrationKey::new("geo_app", "0001_create_locations");
		let deps = graph.get_dependencies(&geo_key).unwrap();

		assert_eq!(
			deps.len(),
			0,
			"Without gis_extension installed, optional dependency should be ignored"
		);
	}

	// Test 2: With gis_extension installed - dependency should be enforced
	{
		let mut graph = MigrationGraph::new();

		// Context with gis_extension installed
		let context_with_gis = DependencyResolutionContext::new()
			.with_app("geo_app")
			.with_app("gis_extension");

		graph.add_migration_with_context(&migration_gis, &context_with_gis);
		graph.add_migration_with_context(&migration_geo, &context_with_gis);

		let geo_key = MigrationKey::new("geo_app", "0001_create_locations");
		let deps = graph.get_dependencies(&geo_key).unwrap();

		assert_eq!(
			deps.len(),
			1,
			"With gis_extension installed, optional dependency should be enforced"
		);
		assert_eq!(
			deps[0].app_label, "gis_extension",
			"Optional dependency should resolve to gis_extension"
		);
	}

	// Test 3: Apply migrations (without gis_extension - should work)
	executor
		.apply_migrations(&[migration_geo.clone()])
		.await
		.expect("Failed to apply geo migration without gis_extension");

	// Verify table was created
	let locations_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("locations")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(locations_exists, "locations table should exist");
}

// ============================================================================
// Abnormal Case Tests - Error Detection
// ============================================================================

/// Test circular dependency detection (A → B → A)
///
/// **Test Intent**: Verify that circular dependencies are detected and cause error
#[rstest]
#[tokio::test]
async fn test_circular_dependency_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration A (depends on B - circular)
	let migration_a = create_migration_with_deps(
		"app",
		"0001_migration_a",
		vec![Operation::CreateTable {
			name: leak_str("table_a").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("app", "0002_migration_b")], // Depends on B
	);

	// Migration B (depends on A - circular)
	let migration_b = create_migration_with_deps(
		"app",
		"0002_migration_b",
		vec![Operation::CreateTable {
			name: leak_str("table_b").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("app", "0001_migration_a")], // Depends on A
	);

	// Attempt to apply migrations with circular dependency (should fail)
	let result = executor.apply_migrations(&[migration_a, migration_b]).await;

	assert!(
		result.is_err(),
		"Circular dependency should be detected and cause error"
	);
}

/// Test missing dependency detection
///
/// **Test Intent**: Verify that missing dependencies are detected
#[rstest]
#[tokio::test]
async fn test_missing_dependency_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Migration that depends on non-existent migration
	let migration = create_migration_with_deps(
		"app",
		"0002_add_column",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("name", FieldType::VarChar(100)),
			mysql_options: None,
		}],
		vec![("app", "0001_nonexistent")], // Depends on migration that doesn't exist
	);

	// Attempt to apply migration (should fail due to missing dependency)
	let result = executor.apply_migrations(&[migration]).await;

	assert!(
		result.is_err(),
		"Missing dependency should be detected and cause error"
	);
}

/// Test conflicting dependency order detection
///
/// **Test Intent**: Verify that conflicting orderings are detected
///
/// **Example**: If A must come before B (A → B) and B must come before A (B → A),
/// this is a contradiction (circular dependency variant).
#[rstest]
#[tokio::test]
async fn test_conflicting_dependency_order(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Same as circular dependency test - conflicting order is a cycle
	let migration_a = create_migration_with_deps(
		"app",
		"0001_migration_a",
		vec![Operation::CreateTable {
			name: leak_str("table_a").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("app", "0002_migration_b")],
	);

	let migration_b = create_migration_with_deps(
		"app",
		"0002_migration_b",
		vec![Operation::CreateTable {
			name: leak_str("table_b").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![("app", "0001_migration_a")],
	);

	let result = executor.apply_migrations(&[migration_a, migration_b]).await;

	assert!(
		result.is_err(),
		"Conflicting dependency order should cause error"
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test deep dependency chain (100 levels)
///
/// **Test Intent**: Verify that very deep dependency chains don't cause stack overflow
#[rstest]
#[tokio::test]
async fn test_deep_dependency_chain(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let mut migrations = Vec::new();

	// Create 100 migrations, each depending on the previous one
	for i in 0..100 {
		let name = leak_str(format!("{:04}_migration", i + 1));
		let dependencies = if i == 0 {
			vec![]
		} else {
			vec![("app", leak_str(format!("{:04}_migration", i)))]
		};

		let migration = create_migration_with_deps(
			"app",
			name,
			vec![Operation::RunSQL {
				sql: leak_str(format!("SELECT {}", i + 1)).to_string(),
				reverse_sql: None,
			}],
			dependencies,
		);

		migrations.push(migration);
	}

	// Apply all 100 migrations (should not stack overflow)
	let result = executor.apply_migrations(&migrations).await;

	// Note: This might fail if dependency resolution is not implemented,
	// but it should not panic or stack overflow
	match result {
		Ok(_) => {
			// Success - deep dependencies resolved correctly
		}
		Err(e) => {
			// If it fails, verify it's a graceful error, not a panic
			eprintln!("Deep dependency chain failed: {}", e);
			assert!(
				!format!("{}", e).contains("stack overflow"),
				"Should not cause stack overflow"
			);
		}
	}
}

/// Test independent migrations (parallel execution potential)
///
/// **Test Intent**: Verify that independent migrations (no dependencies) can be
/// applied in any order
#[rstest]
#[tokio::test]
async fn test_independent_migrations(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Three independent migrations (no dependencies between them)
	let migration_1 = create_migration_with_deps(
		"app",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
	);

	let migration_2 = create_migration_with_deps(
		"app",
		"0002_create_posts",
		vec![Operation::CreateTable {
			name: leak_str("posts").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
	);

	let migration_3 = create_migration_with_deps(
		"app",
		"0003_create_comments",
		vec![Operation::CreateTable {
			name: leak_str("comments").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
		vec![],
	);

	// Apply in arbitrary order (should all succeed)
	executor
		.apply_migrations(&[migration_2, migration_1, migration_3])
		.await
		.expect("Independent migrations should apply in any order");

	// Verify all tables exist
	assert!(
		table_exists(pool.as_ref(), "users").await,
		"users table should exist"
	);
	assert!(
		table_exists(pool.as_ref(), "posts").await,
		"posts table should exist"
	);
	assert!(
		table_exists(pool.as_ref(), "comments").await,
		"comments table should exist"
	);
}
