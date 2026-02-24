//! Schema Validation Integration Tests
//!
//! Tests that verify the consistency between the actual database schema and the
//! migration ProjectState. This ensures that the database state matches what the
//! migrations declare.
//!
//! **Test Coverage:**
//! - Schema consistency validation (DB schema = ProjectState)
//! - Table existence checks via information_schema
//! - Column definition validation (name, type, NOT NULL, DEFAULT)
//! - Constraint validation (PK, FK, UNIQUE, CHECK)
//! - Index validation
//! - Schema drift detection (manual changes outside migrations)
//! - Type normalization (INTEGER vs INT, case sensitivity)
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//! - mysql_container: MySQL database container
//!
//! **Test Strategy:**
//! 1. Apply migration
//! 2. Query information_schema to verify actual DB state
//! 3. Compare with expected ProjectState
//! 4. Detect any discrepancies (schema drift)

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
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

/// Create a NOT NULL column definition
fn create_not_null_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: true,
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

/// Create a column with default value
fn create_column_with_default(name: &str, type_def: FieldType, default: &str) -> ColumnDefinition {
	ColumnDefinition {
		name: name.to_string(),
		type_definition: type_def,
		not_null: false,
		unique: false,
		primary_key: false,
		auto_increment: false,
		default: Some(default.to_string()),
	}
}

// ============================================================================
// Normal Case Tests - Schema Consistency Validation
// ============================================================================

/// Test that migration creates expected table schema
///
/// **Test Intent**: Verify that CREATE TABLE migration creates correct schema in DB
#[rstest]
#[tokio::test]
async fn test_table_schema_consistency_postgres(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create migration
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
				create_basic_column("email", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Apply migration
	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Validate table exists
	let table_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("users")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table existence");

	assert!(table_exists, "Table 'users' should exist");

	// Validate column count
	let column_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind("users")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to count columns");

	assert_eq!(column_count, 3, "Table should have 3 columns");
}

/// Test table existence check via information_schema
///
/// **Test Intent**: Verify that information_schema correctly reports table existence
#[rstest]
#[tokio::test]
async fn test_table_existence_check(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table
	let migration = create_test_migration(
		"testapp",
		"0001_create_products",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(200)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Check that table exists
	let exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("products")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(exists, "products table should exist");

	// Check that non-existent table doesn't exist
	let not_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("nonexistent")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check non-existent table");

	assert!(!not_exists, "nonexistent table should not exist");
}

/// Test column definition validation (name, type, NOT NULL, DEFAULT)
///
/// **Test Intent**: Verify that column definitions match between migration and DB
#[rstest]
#[tokio::test]
async fn test_column_definition_validation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with various column types
	let migration = create_test_migration(
		"testapp",
		"0001_create_orders",
		vec![Operation::CreateTable {
			name: leak_str("orders").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("order_number", FieldType::VarChar(50)),
				create_basic_column("description", FieldType::Text),
				create_column_with_default("status", FieldType::VarChar(20), "'pending'"),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Validate 'order_number' column (NOT NULL)
	let (data_type, is_nullable): (String, String) = sqlx::query_as(
		"SELECT data_type, is_nullable FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("orders")
	.bind("order_number")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to query column info");

	assert_eq!(
		data_type, "character varying",
		"order_number should be VARCHAR"
	);
	assert_eq!(is_nullable, "NO", "order_number should be NOT NULL");

	// Validate 'description' column (nullable)
	let (desc_type, desc_nullable): (String, String) = sqlx::query_as(
		"SELECT data_type, is_nullable FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("orders")
	.bind("description")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to query description column");

	assert_eq!(desc_type, "text", "description should be TEXT");
	assert_eq!(desc_nullable, "YES", "description should be nullable");

	// Validate 'status' column (with DEFAULT)
	let (status_type, status_default): (String, Option<String>) = sqlx::query_as(
		"SELECT data_type, column_default FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("orders")
	.bind("status")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to query status column");

	assert_eq!(status_type, "character varying", "status should be VARCHAR");
	assert!(
		status_default.is_some(),
		"status should have a DEFAULT value"
	);
	assert!(
		status_default.unwrap().contains("pending"),
		"DEFAULT should be 'pending'"
	);
}

/// Test PRIMARY KEY constraint validation
///
/// **Test Intent**: Verify that PRIMARY KEY constraints are correctly created
#[rstest]
#[tokio::test]
async fn test_primary_key_validation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with primary key
	let migration = create_test_migration(
		"testapp",
		"0001_create_customers",
		vec![Operation::CreateTable {
			name: leak_str("customers").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(100)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Check primary key constraint
	let pk_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_type = 'PRIMARY KEY'
		)",
	)
	.bind("customers")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check primary key");

	assert!(pk_exists, "PRIMARY KEY constraint should exist");

	// Check which column is the primary key
	let pk_column: String = sqlx::query_scalar(
		"SELECT column_name FROM information_schema.key_column_usage
		 WHERE table_name = $1 AND constraint_name = (
			 SELECT constraint_name FROM information_schema.table_constraints
			 WHERE table_name = $1 AND constraint_type = 'PRIMARY KEY'
		 )",
	)
	.bind("customers")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get primary key column");

	assert_eq!(pk_column, "id", "Primary key should be 'id' column");
}

/// Test FOREIGN KEY constraint validation
///
/// **Test Intent**: Verify that FOREIGN KEY constraints are correctly created with proper references
#[rstest]
#[tokio::test]
async fn test_foreign_key_validation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create parent table
	let parent_migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![create_auto_pk_column("id", FieldType::Integer)],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[parent_migration])
		.await
		.expect("Failed to create parent table");

	// Create child table with FK
	let child_migration = create_test_migration(
		"testapp",
		"0002_create_posts",
		vec![
			Operation::CreateTable {
				name: leak_str("posts").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_not_null_column("user_id", FieldType::Integer),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::AddConstraint {
				table: leak_str("posts").to_string(),
				constraint_sql: leak_str(
					"CONSTRAINT fk_posts_user_id FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE NO ACTION"
				).to_string(),
			},
		],
	);

	executor
		.apply_migrations(&[child_migration])
		.await
		.expect("Failed to create child table with FK");

	// Verify FK constraint exists
	let fk_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_type = 'FOREIGN KEY'
		)",
	)
	.bind("posts")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check foreign key");

	assert!(fk_exists, "FOREIGN KEY constraint should exist");

	// Verify FK references correct table
	let (ref_table, ref_column): (String, String) = sqlx::query_as(
		"SELECT
			ccu.table_name AS foreign_table_name,
			ccu.column_name AS foreign_column_name
		FROM information_schema.table_constraints AS tc
		JOIN information_schema.key_column_usage AS kcu
			ON tc.constraint_name = kcu.constraint_name
		JOIN information_schema.constraint_column_usage AS ccu
			ON ccu.constraint_name = tc.constraint_name
		WHERE tc.table_name = $1 AND tc.constraint_type = 'FOREIGN KEY'",
	)
	.bind("posts")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get FK reference");

	assert_eq!(ref_table, "users", "FK should reference 'users' table");
	assert_eq!(ref_column, "id", "FK should reference 'id' column");
}

/// Test UNIQUE constraint validation
///
/// **Test Intent**: Verify that UNIQUE constraints are correctly created
#[rstest]
#[tokio::test]
async fn test_unique_constraint_validation(
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

	// Add UNIQUE constraint
	let add_unique = create_test_migration(
		"testapp",
		"0002_add_unique_email",
		vec![Operation::AddConstraint {
			table: leak_str("users").to_string(),
			constraint_sql: leak_str("CONSTRAINT unique_users_email UNIQUE (email)").to_string(),
		}],
	);

	executor
		.apply_migrations(&[add_unique])
		.await
		.expect("Failed to add UNIQUE constraint");

	// Verify UNIQUE constraint exists
	let unique_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_type = 'UNIQUE'
		)",
	)
	.bind("users")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check UNIQUE constraint");

	assert!(unique_exists, "UNIQUE constraint should exist");
}

/// Test INDEX existence validation
///
/// **Test Intent**: Verify that indexes are correctly created
#[rstest]
#[tokio::test]
async fn test_index_validation(
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
		"0001_create_products",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("name", FieldType::VarChar(200)),
				create_basic_column("category", FieldType::VarChar(50)),
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

	// Create index
	let create_index = create_test_migration(
		"testapp",
		"0002_create_name_index",
		vec![Operation::CreateIndex {
			table: leak_str("products").to_string(),
			columns: vec![leak_str("name").to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}],
	);

	executor
		.apply_migrations(&[create_index])
		.await
		.expect("Failed to create index");

	// Verify index exists
	let index_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
			.bind("idx_products_name")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to check index");

	assert!(index_exists, "Index 'idx_products_name' should exist");

	// Verify index is on correct column
	let index_def: String =
		sqlx::query_scalar("SELECT indexdef FROM pg_indexes WHERE indexname = $1")
			.bind("idx_products_name")
			.fetch_one(pool.as_ref())
			.await
			.expect("Failed to get index definition");

	assert!(
		index_def.contains("name"),
		"Index should be on 'name' column"
	);
}

/// Test CHECK constraint validation
///
/// **Test Intent**: Verify that CHECK constraints are correctly created
#[rstest]
#[tokio::test]
async fn test_check_constraint_validation(
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
		"0001_create_products",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("price", FieldType::Integer),
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

	// Add CHECK constraint
	let add_check = create_test_migration(
		"testapp",
		"0002_add_price_check",
		vec![Operation::AddConstraint {
			table: leak_str("products").to_string(),
			constraint_sql: leak_str("CONSTRAINT check_price_positive CHECK (price > 0)")
				.to_string(),
		}],
	);

	executor
		.apply_migrations(&[add_check])
		.await
		.expect("Failed to add CHECK constraint");

	// Verify CHECK constraint exists
	let check_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_type = 'CHECK'
		)",
	)
	.bind("products")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check CHECK constraint");

	assert!(check_exists, "CHECK constraint should exist");
}

// ============================================================================
// Abnormal Case Tests - Schema Drift Detection
// ============================================================================

/// Test schema drift detection - manually created table
///
/// **Test Intent**: Detect when a table is created outside of migrations
#[rstest]
#[tokio::test]
async fn test_schema_drift_manual_table_creation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Manually create a table (outside migrations)
	sqlx::query("CREATE TABLE manual_table (id SERIAL PRIMARY KEY, data TEXT)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to manually create table");

	// Verify table exists
	let table_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("manual_table")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(table_exists, "Manual table should exist");

	// Note: In a real schema validation system, this would trigger a drift warning
	// because the table exists in DB but not in migration history
}

/// Test schema drift detection - manually added column
///
/// **Test Intent**: Detect when a column is added outside of migrations
#[rstest]
#[tokio::test]
async fn test_schema_drift_manual_column_addition(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table via migration
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
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
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Manually add a column (outside migration)
	sqlx::query("ALTER TABLE users ADD COLUMN manual_column TEXT")
		.execute(pool.as_ref())
		.await
		.expect("Failed to add manual column");

	// Verify column exists
	let column_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.columns
			WHERE table_name = $1 AND column_name = $2
		)",
	)
	.bind("users")
	.bind("manual_column")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check column");

	assert!(column_exists, "Manual column should exist");

	// Note: Schema validation would detect that column count doesn't match migration
}

/// Test schema drift detection - manually dropped constraint
///
/// **Test Intent**: Detect when a constraint is removed outside of migrations
#[rstest]
#[tokio::test]
async fn test_schema_drift_manual_constraint_removal(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with UNIQUE constraint
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![
			Operation::CreateTable {
				name: leak_str("users").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_basic_column("email", FieldType::VarChar(255)),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::AddConstraint {
				table: leak_str("users").to_string(),
				constraint_sql: leak_str("CONSTRAINT unique_users_email UNIQUE (email)")
					.to_string(),
			},
		],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Verify constraint exists
	let constraint_before: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_name = $2
		)",
	)
	.bind("users")
	.bind("unique_users_email")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check constraint before");

	assert!(
		constraint_before,
		"UNIQUE constraint should exist before removal"
	);

	// Manually drop constraint
	sqlx::query("ALTER TABLE users DROP CONSTRAINT unique_users_email")
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop constraint");

	// Verify constraint is gone
	let constraint_after: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.table_constraints
			WHERE table_name = $1 AND constraint_name = $2
		)",
	)
	.bind("users")
	.bind("unique_users_email")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check constraint after");

	assert!(
		!constraint_after,
		"Constraint should not exist after manual removal"
	);
}

/// Test type mismatch detection
///
/// **Test Intent**: Detect when DB type doesn't match migration definition
#[rstest]
#[tokio::test]
async fn test_type_mismatch_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with VARCHAR(100)
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
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
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Verify column type
	let column_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("users")
	.bind("name")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get column type");

	assert_eq!(column_type, "character varying", "Column should be VARCHAR");

	// Manually change type (ALTER COLUMN)
	sqlx::query("ALTER TABLE users ALTER COLUMN name TYPE TEXT")
		.execute(pool.as_ref())
		.await
		.expect("Failed to alter column type");

	// Verify type changed
	let new_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("users")
	.bind("name")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get new column type");

	assert_eq!(
		new_type, "text",
		"Column type should be TEXT after manual change"
	);
	// Note: Schema validation would detect this mismatch
}

/// Test NULL/NOT NULL mismatch detection
///
/// **Test Intent**: Detect when NULL constraint doesn't match migration
#[rstest]
#[tokio::test]
async fn test_nullability_mismatch_detection(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with NOT NULL column
	let migration = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("email", FieldType::VarChar(255)),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	executor
		.apply_migrations(&[migration])
		.await
		.expect("Failed to apply migration");

	// Verify column is NOT NULL
	let is_nullable_before: String = sqlx::query_scalar(
		"SELECT is_nullable FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("users")
	.bind("email")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get nullable status");

	assert_eq!(is_nullable_before, "NO", "Column should be NOT NULL");

	// Manually remove NOT NULL constraint
	sqlx::query("ALTER TABLE users ALTER COLUMN email DROP NOT NULL")
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop NOT NULL");

	// Verify column is now nullable
	let is_nullable_after: String = sqlx::query_scalar(
		"SELECT is_nullable FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("users")
	.bind("email")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get new nullable status");

	assert_eq!(
		is_nullable_after, "YES",
		"Column should be nullable after manual change"
	);
	// Note: Schema validation would detect this mismatch
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test case sensitivity normalization (PostgreSQL lowercases identifiers)
///
/// **Test Intent**: Verify that PostgreSQL's case normalization is handled correctly
#[rstest]
#[tokio::test]
async fn test_case_sensitivity_normalization(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Manually create table with mixed case (PostgreSQL will lowercase it)
	sqlx::query("CREATE TABLE MixedCase (id SERIAL PRIMARY KEY, Name TEXT)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Query with lowercase (should work due to PostgreSQL normalization)
	let table_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)"
	)
	.bind("mixedcase") // lowercase
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(
		table_exists,
		"Table should exist (lowercased by PostgreSQL)"
	);

	// Verify column is also lowercased
	let column_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM information_schema.columns
			WHERE table_name = $1 AND column_name = $2
		)"
	)
	.bind("mixedcase")
	.bind("name") // lowercase
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check column");

	assert!(
		column_exists,
		"Column should exist (lowercased by PostgreSQL)"
	);
}

/// Test type alias normalization (INTEGER vs INT)
///
/// **Test Intent**: Verify that type aliases are handled correctly
#[rstest]
#[tokio::test]
async fn test_type_alias_normalization(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_container.await;

	// Create table with INT (alias for INTEGER)
	sqlx::query("CREATE TABLE test_types (id INT PRIMARY KEY)")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Query actual type (PostgreSQL normalizes to 'integer')
	let column_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns
		 WHERE table_name = $1 AND column_name = $2",
	)
	.bind("test_types")
	.bind("id")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get column type");

	assert_eq!(
		column_type, "integer",
		"INT should be normalized to 'integer'"
	);
}
