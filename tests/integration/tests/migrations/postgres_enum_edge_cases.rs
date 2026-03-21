//! PostgreSQL ENUM edge case tests
//!
//! Tests PostgreSQL ENUM type modifications:
//! - Adding ENUM values
//! - Removing ENUM values (type recreation)
//! - ENUM in column definitions
//!
//! **Test Coverage:**
//! - EC-DB-04: ENUM type modifications
//!   - Adding new ENUM values using ALTER TYPE ADD VALUE
//!   - Removing ENUM values requiring type recreation
//!   - ENUM columns in table definitions
//!   - Renaming ENUM types
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//!
//! **PostgreSQL ENUM Handling:**
//! PostgreSQL ENUM types have special restrictions:
//! - `CREATE TYPE name AS ENUM ('val1', 'val2')` - Creates ENUM type
//! - `ALTER TYPE name ADD VALUE 'new_val'` - Adds value (cannot run in transaction)
//! - Removing values requires recreating the entire type
//!   - Cannot use ALTER TYPE to remove values
//!   - Must drop dependent columns, drop type, recreate type, add columns back

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
// Test 1: Create ENUM Type
// ============================================================================

/// Test creating a PostgreSQL ENUM type using RunSQL
///
/// **Test Intent**: Verify that ENUM types can be created successfully
///
/// **Integration Point**: MigrationExecutor → PostgreSQL CREATE TYPE
///
/// **Expected Behavior**: ENUM type is created and usable in column definitions
#[rstest]
#[tokio::test]
async fn test_create_enum_type(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Arrange
	let enum_name = "order_status";
	let migration = create_test_migration(
		"testapp",
		"0001_create_enum",
		vec![Operation::RunSQL {
			sql: format!(
				"CREATE TYPE {} AS ENUM ('pending', 'processing', 'shipped', 'delivered');",
				enum_name
			),
			reverse_sql: Some(format!("DROP TYPE {};", enum_name)),
		}],
	);

	// Act
	let result = executor.apply_migrations(&[migration.clone()]).await;

	// Assert
	assert!(
		result.is_ok(),
		"ENUM type creation should succeed: {:?}",
		result.err()
	);

	// Verify ENUM type exists
	let row = sqlx::query("SELECT typname FROM pg_type WHERE typname = $1")
		.bind(enum_name)
		.fetch_one(pool.as_ref())
		.await
		.expect("ENUM type should exist in pg_type");

	assert_eq!(row.get::<String, _>("typname"), enum_name);

	// Verify ENUM values
	let rows = sqlx::query(
		"SELECT enumlabel FROM pg_enum WHERE enumtypid = (SELECT oid FROM pg_type WHERE typname = $1) ORDER BY enumsortorder",
	)
	.bind(enum_name)
	.fetch_all(pool.as_ref())
	.await
	.expect("Should retrieve ENUM values");

	let values: Vec<String> = rows
		.iter()
		.map(|r| r.get::<String, _>("enumlabel"))
		.collect();
	assert_eq!(
		values,
		vec!["pending", "processing", "shipped", "delivered"]
	);
}

// ============================================================================
// Test 2: Create Table with ENUM Column
// ============================================================================

/// Test creating a table with an ENUM column
///
/// **Test Intent**: Verify that tables can use custom ENUM types as column types
///
/// **Integration Point**: MigrationExecutor → PostgreSQL CREATE TABLE with ENUM
///
/// **Expected Behavior**: Table is created with ENUM column type
#[rstest]
#[tokio::test]
async fn test_create_table_with_enum_column(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Arrange
	let enum_name = "task_priority";
	let table_name = leak_str("tasks");

	// Create ENUM type first
	let create_enum_migration = create_test_migration(
		"testapp",
		"0001_create_enum",
		vec![Operation::RunSQL {
			sql: format!(
				"CREATE TYPE {} AS ENUM ('low', 'medium', 'high', 'urgent');",
				enum_name
			),
			reverse_sql: Some(format!("DROP TYPE {};", enum_name)),
		}],
	);

	let create_table_migration = create_test_migration(
		"testapp",
		"0002_create_table",
		vec![Operation::CreateTable {
			name: table_name.to_string(),
			columns: vec![
				create_column_with_constraints("id", FieldType::Integer, true, true),
				create_basic_column("title", FieldType::VarChar(255)),
				create_basic_column("priority", FieldType::Custom(enum_name.to_string())),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		}],
	);

	// Act
	executor
		.apply_migrations(&[create_enum_migration.clone()])
		.await
		.expect("ENUM creation should succeed");
	executor
		.apply_migrations(&[create_table_migration.clone()])
		.await
		.expect("Table creation should succeed");

	// Assert
	// Verify table exists
	let row = sqlx::query(
		"SELECT column_name, data_type FROM information_schema.columns WHERE table_name = $1 AND column_name = $2",
	)
	.bind(table_name)
	.bind("priority")
	.fetch_one(pool.as_ref())
	.await
	.expect("Column should exist");

	assert_eq!(row.get::<String, _>("column_name"), "priority");
	assert_eq!(row.get::<String, _>("data_type"), "USER-DEFINED");

	// Verify we can insert and retrieve ENUM values
	sqlx::query("INSERT INTO tasks (title, priority) VALUES ('Task 1', 'high')")
		.execute(pool.as_ref())
		.await
		.expect("Insert should succeed");

	// Cast ENUM to text since sqlx cannot decode custom ENUM type OIDs as String
	let row = sqlx::query("SELECT priority::text FROM tasks WHERE title = 'Task 1'")
		.fetch_one(pool.as_ref())
		.await
		.expect("Query should succeed");

	let priority: String = row.get("priority");
	assert_eq!(priority, "high");
}

// ============================================================================
// Test 3: Add ENUM Value (Non-Transactional)
// ============================================================================

/// Test adding a value to an existing ENUM type
///
/// **Test Intent**: Verify that new values can be added to ENUM types
///
/// **Integration Point**: MigrationExecutor → PostgreSQL ALTER TYPE ADD VALUE
///
/// **Expected Behavior**: New value is added to ENUM type
///
/// **Note**: ALTER TYPE ADD VALUE cannot be run inside a transaction.
/// This test verifies the system handles this PostgreSQL restriction.
#[rstest]
#[tokio::test]
async fn test_add_enum_value(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Arrange
	let enum_name = "subscription_status";

	// Create initial ENUM
	let create_migration = create_test_migration(
		"testapp",
		"0001_create_enum",
		vec![Operation::RunSQL {
			sql: format!("CREATE TYPE {} AS ENUM ('active', 'inactive');", enum_name),
			reverse_sql: Some(format!("DROP TYPE {};", enum_name)),
		}],
	);

	executor
		.apply_migrations(&[create_migration.clone()])
		.await
		.expect("ENUM creation should succeed");

	// Act: Add a new value
	let add_value_migration = create_test_migration(
		"testapp",
		"0002_add_enum_value",
		vec![Operation::RunSQL {
			sql: format!("ALTER TYPE {} ADD VALUE 'suspended';", enum_name),
			reverse_sql: None, // Cannot easily reverse adding ENUM values
		}],
	);

	let result = executor
		.apply_migrations(&[add_value_migration.clone()])
		.await;

	// Assert
	assert!(
		result.is_ok(),
		"Adding ENUM value should succeed: {:?}",
		result.err()
	);

	// Verify new value exists
	let rows = sqlx::query(
		"SELECT enumlabel FROM pg_enum WHERE enumtypid = (SELECT oid FROM pg_type WHERE typname = $1) ORDER BY enumsortorder",
	)
	.bind(enum_name)
	.fetch_all(pool.as_ref())
	.await
	.expect("Should retrieve ENUM values");

	let values: Vec<String> = rows
		.iter()
		.map(|r| r.get::<String, _>("enumlabel"))
		.collect();
	assert_eq!(values, vec!["active", "inactive", "suspended"]);
}

// ============================================================================
// Test 4: Remove ENUM Value (Type Recreation)
// ============================================================================

/// Test removing an ENUM value by recreating the type
///
/// **Test Intent**: Verify that ENUM values can be removed through type recreation
///
/// **Integration Point**: MigrationExecutor → PostgreSQL type recreation workflow
///
/// **Expected Behavior**: ENUM type is recreated without the specified value
///
/// **Note**: PostgreSQL does not support removing ENUM values directly.
/// The workaround is:
/// 1. ALTER COLUMN to use plain text type
/// 2. DROP the old ENUM type
/// 3. CREATE new ENUM type without the value
/// 4. ALTER COLUMN back to new ENUM type
#[rstest]
#[tokio::test]
async fn test_remove_enum_value_type_recreation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Arrange
	let enum_name = "user_status";
	let table_name = "users";
	let temp_enum_name = "user_status_new";

	// Create initial ENUM with values
	let create_enum_migration = create_test_migration(
		"testapp",
		"0001_create_enum",
		vec![Operation::RunSQL {
			sql: format!(
				"CREATE TYPE {} AS ENUM ('pending', 'active', 'suspended', 'deleted');",
				enum_name
			),
			reverse_sql: Some(format!("DROP TYPE {};", enum_name)),
		}],
	);

	// Create table with ENUM column
	let create_table_migration = create_test_migration(
		"testapp",
		"0002_create_table",
		vec![Operation::RunSQL {
			sql: format!(
				"CREATE TABLE {} (id SERIAL PRIMARY KEY, name VARCHAR(255), status {} NOT NULL);",
				table_name, enum_name
			),
			reverse_sql: Some(format!("DROP TABLE {};", table_name)),
		}],
	);

	executor
		.apply_migrations(&[create_enum_migration.clone()])
		.await
		.expect("ENUM creation should succeed");
	executor
		.apply_migrations(&[create_table_migration.clone()])
		.await
		.expect("Table creation should succeed");

	// Insert test data
	sqlx::query(
		"INSERT INTO users (name, status) VALUES ('User1', 'active'), ('User2', 'pending')",
	)
	.execute(pool.as_ref())
	.await
	.expect("Insert should succeed");

	// Act: Remove 'deleted' value by recreating type
	let remove_value_migration = create_test_migration(
		"testapp",
		"0003_remove_enum_value",
		vec![
			// Step 1: Create new ENUM type without 'deleted'
			Operation::RunSQL {
				sql: format!(
					"CREATE TYPE {} AS ENUM ('pending', 'active', 'suspended');",
					temp_enum_name
				),
				reverse_sql: Some(format!("DROP TYPE {};", temp_enum_name)),
			},
			// Step 2: Alter column to use new ENUM type (casting existing values)
			Operation::RunSQL {
				sql: format!(
					"ALTER TABLE {} ALTER COLUMN status TYPE {} USING status::text::{};",
					table_name, temp_enum_name, temp_enum_name
				),
				reverse_sql: Some(format!(
					"ALTER TABLE {} ALTER COLUMN status TYPE {} USING status::text::{};",
					table_name, enum_name, enum_name
				)),
			},
			// Step 3: Drop old ENUM type
			Operation::RunSQL {
				sql: format!("DROP TYPE {};", enum_name),
				reverse_sql: None,
			},
		],
	);

	let result = executor
		.apply_migrations(&[remove_value_migration.clone()])
		.await;

	// Assert
	assert!(
		result.is_ok(),
		"ENUM value removal should succeed: {:?}",
		result.err()
	);

	// Verify 'deleted' value no longer exists
	let rows = sqlx::query(
		"SELECT enumlabel FROM pg_enum WHERE enumtypid = (SELECT oid FROM pg_type WHERE typname = $1) ORDER BY enumsortorder",
	)
	.bind(temp_enum_name)
	.fetch_all(pool.as_ref())
	.await
	.expect("Should retrieve ENUM values");

	let values: Vec<String> = rows
		.iter()
		.map(|r| r.get::<String, _>("enumlabel"))
		.collect();
	assert_eq!(values, vec!["pending", "active", "suspended"]);
	assert!(!values.contains(&"deleted".to_string()));

	// Verify existing data is still accessible
	// Cast ENUM to text since sqlx cannot decode custom ENUM type OIDs as String
	let row = sqlx::query("SELECT status::text FROM users WHERE name = 'User1'")
		.fetch_one(pool.as_ref())
		.await
		.expect("Query should succeed");

	let status: String = row.get("status");
	assert_eq!(status, "active");
}

// ============================================================================
// Test 5: Rename ENUM Type
// ============================================================================

/// Test renaming an ENUM type
///
/// **Test Intent**: Verify that ENUM types can be renamed
///
/// **Integration Point**: MigrationExecutor → PostgreSQL ALTER TYPE RENAME
///
/// **Expected Behavior**: ENUM type is renamed and columns continue to work
#[rstest]
#[tokio::test]
async fn test_rename_enum_type(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Arrange
	let old_enum_name = "old_status";
	let new_enum_name = "new_status";
	let table_name = "documents";

	// Create ENUM and table
	let create_migration = create_test_migration(
		"testapp",
		"0001_create",
		vec![
			Operation::RunSQL {
				sql: format!(
					"CREATE TYPE {} AS ENUM ('draft', 'published', 'archived');",
					old_enum_name
				),
				reverse_sql: Some(format!("DROP TYPE {};", old_enum_name)),
			},
			Operation::RunSQL {
				sql: format!(
					"CREATE TABLE {} (id SERIAL PRIMARY KEY, title VARCHAR(255), status {});",
					table_name, old_enum_name
				),
				reverse_sql: Some(format!("DROP TABLE {};", table_name)),
			},
		],
	);

	executor
		.apply_migrations(&[create_migration.clone()])
		.await
		.expect("Creation should succeed");

	// Act: Rename ENUM type
	let rename_migration = create_test_migration(
		"testapp",
		"0002_rename_enum",
		vec![Operation::RunSQL {
			sql: format!("ALTER TYPE {} RENAME TO {};", old_enum_name, new_enum_name),
			reverse_sql: Some(format!(
				"ALTER TYPE {} RENAME TO {};",
				new_enum_name, old_enum_name
			)),
		}],
	);

	let result = executor.apply_migrations(&[rename_migration.clone()]).await;

	// Assert
	assert!(
		result.is_ok(),
		"ENUM rename should succeed: {:?}",
		result.err()
	);

	// Verify old type name no longer exists
	let old_exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_type WHERE typname = $1)")
		.bind(old_enum_name)
		.fetch_one(pool.as_ref())
		.await
		.expect("Query should succeed");

	assert!(
		!old_exists.get::<bool, _>("exists"),
		"Old enum name should not exist"
	);

	// Verify new type name exists
	let new_exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_type WHERE typname = $1)")
		.bind(new_enum_name)
		.fetch_one(pool.as_ref())
		.await
		.expect("Query should succeed");

	assert!(
		new_exists.get::<bool, _>("exists"),
		"New enum name should exist"
	);

	// Verify column still works
	sqlx::query("INSERT INTO documents (title, status) VALUES ('Doc1', 'published')")
		.execute(pool.as_ref())
		.await
		.expect("Insert should succeed");

	// Cast ENUM to text since sqlx cannot decode custom ENUM type OIDs as String
	let row = sqlx::query("SELECT status::text FROM documents WHERE title = 'Doc1'")
		.fetch_one(pool.as_ref())
		.await
		.expect("Query should succeed");

	let status: String = row.get("status");
	assert_eq!(status, "published");
}

// ============================================================================
// Test 6: Multiple ENUM Columns Same Type
// ============================================================================

/// Test using the same ENUM type in multiple columns
///
/// **Test Intent**: Verify that a single ENUM type can be reused across columns
///
/// **Integration Point**: MigrationExecutor → PostgreSQL ENUM type sharing
///
/// **Expected Behavior**: Multiple columns can use the same ENUM type
#[rstest]
#[tokio::test]
async fn test_enum_type_multiple_columns(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(connection);

	// Arrange
	let enum_name = "priority_level";

	// Create ENUM
	let create_migration = create_test_migration(
		"testapp",
		"0001_create",
		vec![
			Operation::RunSQL {
				sql: format!(
					"CREATE TYPE {} AS ENUM ('low', 'medium', 'high');",
					enum_name
				),
				reverse_sql: Some(format!("DROP TYPE {};", enum_name)),
			},
			Operation::CreateTable {
				name: "projects".to_string(),
				columns: vec![
					create_column_with_constraints("id", FieldType::Integer, true, true),
					create_basic_column("name", FieldType::VarChar(255)),
					create_basic_column(
						"initial_priority",
						FieldType::Custom(enum_name.to_string()),
					),
					create_basic_column(
						"current_priority",
						FieldType::Custom(enum_name.to_string()),
					),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
	);

	// Act
	let result = executor.apply_migrations(&[create_migration.clone()]).await;

	// Assert
	assert!(
		result.is_ok(),
		"Table with multiple ENUM columns should succeed: {:?}",
		result.err()
	);

	// Verify both columns work
	sqlx::query("INSERT INTO projects (name, initial_priority, current_priority) VALUES ('Project1', 'low', 'high')")
		.execute(pool.as_ref())
		.await
		.expect("Insert should succeed");

	// Cast ENUM columns to text since sqlx cannot decode custom ENUM type OIDs as String
	let row = sqlx::query(
		"SELECT initial_priority::text, current_priority::text FROM projects WHERE name = 'Project1'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Query should succeed");

	let initial: String = row.get("initial_priority");
	let current: String = row.get("current_priority");
	assert_eq!(initial, "low");
	assert_eq!(current, "high");
}
