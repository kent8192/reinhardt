//! Data Migration Integration Tests
//!
//! Tests that verify data manipulation operations within migrations. Unlike schema
//! migrations that only modify table structures, data migrations perform operations
//! on actual data (INSERT, UPDATE, DELETE, data transformations).
//!
//! **Test Coverage:**
//! - RunSQL for data operations (INSERT, UPDATE, DELETE)
//! - RunCode for Rust closure execution (Django's RunPython equivalent)
//! - Data transformation (type conversions, computed defaults)
//! - Data cleaning (removing invalid data)
//! - Bulk data operations
//! - Stored procedures and triggers
//! - Error handling in data operations
//!
//! **Fixtures Used:**
//! - postgres_container: PostgreSQL database container
//! - mysql_container: MySQL database container
//!
//! **Key Concepts:**
//! - **Schema migrations**: Modify database structure (CREATE TABLE, ADD COLUMN, etc.)
//! - **Data migrations**: Modify database content (INSERT, UPDATE, data transformations)
//! - **RunSQL**: Execute arbitrary SQL statements during migration
//! - **RunCode**: Execute Rust code during migration (Django's RunPython equivalent)

use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, Operation, executor::DatabaseMigrationExecutor,
	operations::special::RunCode,
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

// ============================================================================
// Normal Case Tests - Data Operations
// ============================================================================

/// Test RunCode basic operation (Rust closure for data updates)
///
/// **Test Intent**: Verify that Rust code can be executed during migration to update data
///
/// **Django Equivalent**: RunPython operation allowing arbitrary code in migrations
#[rstest]
#[tokio::test]
async fn test_run_code_basic_operation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	// Create RunCode operation with description
	let run_code = RunCode::new("Update NULL emails to default", |_conn| {
		// RunCode provides sync closure API with database connection access
		// For complex async database operations, use RunSQL instead
		Ok(())
	});

	// Verify RunCode struct properties
	assert_eq!(run_code.description, "Update NULL emails to default");
	assert!(run_code.reverse_code.is_none());

	// Execute the RunCode operation
	run_code
		.execute(&connection)
		.expect("Failed to execute RunCode");

	// Test RunCode with reverse code
	let reversible_code = RunCode::new("Reversible data migration", |_conn| {
		// Forward migration logic
		Ok(())
	})
	.with_reverse_code(|_conn| {
		// Reverse migration logic
		Ok(())
	});

	assert!(reversible_code.reverse_code.is_some());
	reversible_code
		.execute(&connection)
		.expect("Failed to execute forward");
	reversible_code
		.execute_reverse(&connection)
		.expect("Failed to execute reverse");
}

/// Test RunSQL basic operation (INSERT/UPDATE/DELETE)
///
/// **Test Intent**: Verify that RunSQL can perform data manipulation operations
#[rstest]
#[tokio::test]
async fn test_run_sql_data_manipulation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table first
	let create_table = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![Operation::CreateTable {
			name: leak_str("users").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("name", FieldType::VarChar(100)),
				create_basic_column("status", FieldType::VarChar(20)),
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

	// Data migration: INSERT, UPDATE, DELETE
	let data_migration = create_test_migration(
		"testapp",
		"0002_populate_initial_data",
		vec![
			// INSERT initial users
			Operation::RunSQL {
				sql: leak_str(
					"INSERT INTO users (name, status) VALUES
					 ('Alice', 'active'),
					 ('Bob', 'pending'),
					 ('Charlie', 'active')",
				)
				.to_string(),
				reverse_sql: Some(
					leak_str("DELETE FROM users WHERE name IN ('Alice', 'Bob', 'Charlie')")
						.to_string(),
				),
			},
			// UPDATE Bob's status
			Operation::RunSQL {
				sql: leak_str("UPDATE users SET status = 'active' WHERE name = 'Bob'").to_string(),
				reverse_sql: Some(
					leak_str("UPDATE users SET status = 'pending' WHERE name = 'Bob'").to_string(),
				),
			},
		],
	);

	executor
		.apply_migrations(&[data_migration])
		.await
		.expect("Failed to apply data migration");

	// Verify data was inserted
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count users");

	assert_eq!(count, 3, "Should have 3 users");

	// Verify Bob's status was updated
	let bob_status: String = sqlx::query_scalar("SELECT status FROM users WHERE name = 'Bob'")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to get Bob's status");

	assert_eq!(bob_status, "active", "Bob's status should be 'active'");
}

/// Test setting default values with computed expressions
///
/// **Test Intent**: Verify that computed default values (NOW(), UUID, etc.) can be applied
/// to existing rows
#[rstest]
#[tokio::test]
async fn test_computed_default_values(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with existing data (no created_at column initially)
	let create_table = create_test_migration(
		"testapp",
		"0001_create_posts",
		vec![
			Operation::CreateTable {
				name: leak_str("posts").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_not_null_column("title", FieldType::VarChar(200)),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			// Insert some initial data
			Operation::RunSQL {
				sql: leak_str("INSERT INTO posts (title) VALUES ('First Post'), ('Second Post')")
					.to_string(),
				reverse_sql: None,
			},
		],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table and insert data");

	// Add created_at column with default NOW()
	let add_created_at = create_test_migration(
		"testapp",
		"0002_add_created_at",
		vec![
			Operation::AddColumn {
				table: leak_str("posts").to_string(),
				column: create_basic_column("created_at", FieldType::DateTime),
				mysql_options: None,
			},
			// Backfill existing rows with current timestamp
			Operation::RunSQL {
				sql: leak_str("UPDATE posts SET created_at = NOW() WHERE created_at IS NULL")
					.to_string(),
				reverse_sql: None,
			},
		],
	);

	executor
		.apply_migrations(&[add_created_at])
		.await
		.expect("Failed to add created_at column");

	// Verify all rows have created_at values
	let null_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE created_at IS NULL")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count NULL created_at");

	assert_eq!(null_count, 0, "All posts should have created_at values");
}

/// Test data type conversion (VARCHAR → INTEGER)
///
/// **Test Intent**: Verify that data can be converted between types
#[rstest]
#[tokio::test]
async fn test_data_type_conversion(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with VARCHAR age column
	let create_table = create_test_migration(
		"testapp",
		"0001_create_users",
		vec![
			Operation::CreateTable {
				name: leak_str("users").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_basic_column("age", FieldType::VarChar(10)), // Initially VARCHAR
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::RunSQL {
				sql: leak_str("INSERT INTO users (age) VALUES ('25'), ('30'), ('35')").to_string(),
				reverse_sql: None,
			},
		],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table");

	// Convert age from VARCHAR to INTEGER
	let convert_age = create_test_migration(
		"testapp",
		"0002_convert_age_to_int",
		vec![
			// Add new INTEGER column
			Operation::AddColumn {
				table: leak_str("users").to_string(),
				column: create_basic_column("age_int", FieldType::Integer),
				mysql_options: None,
			},
			// Copy data with type conversion
			Operation::RunSQL {
				sql: leak_str("UPDATE users SET age_int = CAST(age AS INTEGER)").to_string(),
				reverse_sql: None,
			},
			// Drop old VARCHAR column
			Operation::DropColumn {
				table: leak_str("users").to_string(),
				column: leak_str("age").to_string(),
			},
			// Rename age_int to age
			Operation::RenameColumn {
				table: leak_str("users").to_string(),
				old_name: leak_str("age_int").to_string(),
				new_name: leak_str("age").to_string(),
			},
		],
	);

	executor
		.apply_migrations(&[convert_age])
		.await
		.expect("Failed to convert age column");

	// Verify column type is now integer
	let column_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns
		 WHERE table_name = 'users' AND column_name = 'age'",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to get column type");

	assert_eq!(column_type, "integer", "age column should be INTEGER");

	// Verify data was preserved
	let ages: Vec<i32> = sqlx::query_scalar("SELECT age FROM users ORDER BY age")
		.fetch_all(pool.as_ref())
		.await
		.expect("Failed to fetch ages");

	assert_eq!(
		ages,
		vec![25, 30, 35],
		"Ages should be preserved as integers"
	);
}

/// Test data cleaning (removing invalid data)
///
/// **Test Intent**: Verify that invalid data can be cleaned during migration
#[rstest]
#[tokio::test]
async fn test_data_cleaning(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table with potentially invalid data
	let create_table = create_test_migration(
		"testapp",
		"0001_create_products",
		vec![
			Operation::CreateTable {
				name: leak_str("products").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_basic_column("name", FieldType::VarChar(200)),
					create_basic_column("price", FieldType::Integer),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::RunSQL {
				sql: leak_str(
					"INSERT INTO products (name, price) VALUES
					 ('Valid Product', 100),
					 ('Invalid - Negative Price', -50),
					 ('Invalid - Zero Price', 0),
					 ('Valid Product 2', 200)",
				)
				.to_string(),
				reverse_sql: None,
			},
		],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table");

	// Data cleaning migration: remove invalid data
	let clean_data = create_test_migration(
		"testapp",
		"0002_clean_invalid_products",
		vec![Operation::RunSQL {
			sql: leak_str("DELETE FROM products WHERE price <= 0").to_string(),
			reverse_sql: None, // Cleaning is typically irreversible
		}],
	);

	executor
		.apply_migrations(&[clean_data])
		.await
		.expect("Failed to clean data");

	// Verify invalid data was removed
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count products");

	assert_eq!(count, 2, "Only valid products should remain");

	// Verify all remaining products have positive prices
	let min_price: i32 = sqlx::query_scalar("SELECT MIN(price) FROM products")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to get min price");

	assert!(
		min_price > 0,
		"All remaining products should have positive prices"
	);
}

/// Test bulk data insertion (1000 rows)
///
/// **Test Intent**: Verify that large bulk data operations can be performed
#[rstest]
#[tokio::test]
async fn test_bulk_data_insertion(
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
				create_not_null_column("event_type", FieldType::VarChar(50)),
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

	// Generate 1000 INSERT statements
	let mut values_parts = Vec::new();
	for i in 0..1000 {
		values_parts.push(format!("('event_type_{}')", i));
	}
	let insert_sql = format!(
		"INSERT INTO events (event_type) VALUES {}",
		values_parts.join(", ")
	);

	// Bulk insert migration
	let bulk_insert = create_test_migration(
		"testapp",
		"0002_bulk_insert_events",
		vec![Operation::RunSQL {
			sql: leak_str(insert_sql).to_string(),
			reverse_sql: Some(leak_str("DELETE FROM events").to_string()),
		}],
	);

	executor
		.apply_migrations(&[bulk_insert])
		.await
		.expect("Failed to bulk insert");

	// Verify 1000 rows were inserted
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count events");

	assert_eq!(count, 1000, "Should have 1000 events");
}

/// Test stored procedure creation and invocation
///
/// **Test Intent**: Verify that stored procedures can be created and called in migrations
#[rstest]
#[tokio::test]
async fn test_stored_procedure_creation(
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
		"0001_create_counters",
		vec![Operation::CreateTable {
			name: leak_str("counters").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_not_null_column("name", FieldType::VarChar(50)),
				create_basic_column("value", FieldType::Integer),
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

	// Create stored procedure
	let create_procedure = create_test_migration(
		"testapp",
		"0002_create_increment_procedure",
		vec![Operation::RunSQL {
			sql: leak_str(
				"CREATE OR REPLACE FUNCTION increment_counter(counter_name VARCHAR(50))
				 RETURNS INTEGER AS $$
				 DECLARE
					 new_value INTEGER;
				 BEGIN
					 UPDATE counters SET value = COALESCE(value, 0) + 1
					 WHERE name = counter_name
					 RETURNING value INTO new_value;
					 RETURN new_value;
				 END;
				 $$ LANGUAGE plpgsql",
			)
			.to_string(),
			reverse_sql: Some(
				leak_str("DROP FUNCTION IF EXISTS increment_counter(VARCHAR)").to_string(),
			),
		}],
	);

	executor
		.apply_migrations(&[create_procedure])
		.await
		.expect("Failed to create stored procedure");

	// Verify procedure exists
	let proc_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM pg_proc
			WHERE proname = 'increment_counter'
		)",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check procedure");

	assert!(proc_exists, "Stored procedure should exist");
}

/// Test trigger creation and deletion
///
/// **Test Intent**: Verify that database triggers can be created in migrations
#[rstest]
#[tokio::test]
async fn test_trigger_creation(
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
		"0001_create_audit_log",
		vec![
			Operation::CreateTable {
				name: leak_str("users").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_not_null_column("name", FieldType::VarChar(100)),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateTable {
				name: leak_str("audit_log").to_string(),
				columns: vec![
					create_auto_pk_column("id", FieldType::Integer),
					create_basic_column("action", FieldType::VarChar(50)),
					create_basic_column("user_id", FieldType::Integer),
				],
				constraints: vec![],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
		],
	);

	executor
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create tables");

	// Create trigger
	let create_trigger = create_test_migration(
		"testapp",
		"0002_create_audit_trigger",
		vec![
			// Create trigger function
			Operation::RunSQL {
				sql: leak_str(
					"CREATE OR REPLACE FUNCTION audit_user_changes()
					 RETURNS TRIGGER AS $$
					 BEGIN
						 INSERT INTO audit_log (action, user_id)
						 VALUES (TG_OP, NEW.id);
						 RETURN NEW;
					 END;
					 $$ LANGUAGE plpgsql",
				)
				.to_string(),
				reverse_sql: Some(
					leak_str("DROP FUNCTION IF EXISTS audit_user_changes()").to_string(),
				),
			},
			// Create trigger
			Operation::RunSQL {
				sql: leak_str(
					"CREATE TRIGGER user_audit_trigger
					 AFTER INSERT ON users
					 FOR EACH ROW
					 EXECUTE FUNCTION audit_user_changes()",
				)
				.to_string(),
				reverse_sql: Some(
					leak_str("DROP TRIGGER IF EXISTS user_audit_trigger ON users").to_string(),
				),
			},
		],
	);

	executor
		.apply_migrations(&[create_trigger])
		.await
		.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(
			SELECT 1 FROM pg_trigger
			WHERE tgname = 'user_audit_trigger'
		)",
	)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check trigger");

	assert!(trigger_exists, "Trigger should exist");

	// Test trigger functionality
	sqlx::query("INSERT INTO users (name) VALUES ('Test User')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert user");

	// Verify audit log was populated by trigger
	let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count audit log");

	assert_eq!(audit_count, 1, "Audit log should have 1 entry from trigger");
}

// ============================================================================
// Abnormal Case Tests - Error Handling
// ============================================================================

/// Test error handling in RunCode (closure returns error)
///
/// **Test Intent**: Verify that errors in RunCode are properly propagated
///
/// **Django Equivalent**: RunPython exception handling with transaction rollback
#[rstest]
#[tokio::test]
async fn test_run_code_error_handling(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	// Create RunCode that returns an error
	let failing_code = RunCode::new("Failing operation", |_conn| {
		Err("Intentional error for testing".to_string())
	});

	// Execute should return the error
	let result = failing_code.execute(&connection);
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), "Intentional error for testing");

	// Verify reverse_code requirement when calling execute_reverse without setting it
	let code_without_reverse = RunCode::new("No reverse", |_| Ok(()));
	let reverse_result = code_without_reverse.execute_reverse(&connection);
	assert!(reverse_result.is_err());
	assert_eq!(
		reverse_result.unwrap_err(),
		"This operation is not reversible"
	);

	// Verify successful reverse execution when reverse_code is set
	let reversible_code =
		RunCode::new("Reversible operation", |_| Ok(())).with_reverse_code(|_| Ok(()));
	let reverse_result = reversible_code.execute_reverse(&connection);
	assert!(reverse_result.is_ok());
}

/// Test error handling in RunSQL (SQL syntax error)
///
/// **Test Intent**: Verify that SQL errors cause migration failure and rollback
#[rstest]
#[tokio::test]
async fn test_run_sql_error_handling(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create table first
	let create_table = create_test_migration(
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
		.apply_migrations(&[create_table])
		.await
		.expect("Failed to create table");

	// Migration with SQL syntax error
	let bad_migration = create_test_migration(
		"testapp",
		"0002_bad_sql",
		vec![Operation::RunSQL {
			sql: leak_str("INSERT INTO nonexistent_table (col) VALUES (1)").to_string(), // Table doesn't exist
			reverse_sql: None,
		}],
	);

	// Attempt migration (should fail)
	let result = executor.apply_migrations(&[bad_migration]).await;

	assert!(result.is_err(), "Migration with invalid SQL should fail");

	// Verify table still exists (rollback didn't drop it)
	let table_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("users")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(
		table_exists,
		"Table should still exist after failed migration"
	);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test SeparateDatabaseAndState equivalent (state_only: update ProjectState only)
///
/// **Test Intent**: Verify that migrations with state_only=true skip database operations
///
/// **Django Equivalent**: SeparateDatabaseAndState(state_operations=[...], database_operations=[])
///
/// **Use Case**: When database was manually modified but migrations need to catch up
#[rstest]
#[tokio::test]
async fn test_state_only_migration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create a migration with state_only=true
	// This migration should NOT execute database operations
	let state_only_migration = Migration {
		app_label: "testapp".to_string(),
		name: "0001_state_only".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("state_only_table").to_string(),
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
		state_only: true, // Skip database operations
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Apply migration with state_only=true
	executor
		.apply_migrations(&[state_only_migration])
		.await
		.expect("Failed to apply state_only migration");

	// Verify that the table was NOT created in the database
	// (state_only skips database operations)
	let table_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("state_only_table")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(
		!table_exists,
		"Table should NOT be created when state_only=true"
	);
}

/// Test SeparateDatabaseAndState equivalent (database_only: update DB only)
///
/// **Test Intent**: Verify that migrations with database_only=true execute database
/// operations but skip ProjectState updates
///
/// **Django Equivalent**: SeparateDatabaseAndState(state_operations=[], database_operations=[...])
///
/// **Use Case**: Temporary database changes that shouldn't be reflected in models
#[rstest]
#[tokio::test]
async fn test_database_only_migration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	// Create a migration with database_only=true
	// This migration executes database operations but doesn't update ProjectState
	let database_only_migration = Migration {
		app_label: "testapp".to_string(),
		name: "0001_db_only".to_string(),
		operations: vec![Operation::CreateTable {
			name: leak_str("database_only_table").to_string(),
			columns: vec![
				create_auto_pk_column("id", FieldType::Integer),
				create_basic_column("data", FieldType::Text),
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
		database_only: true, // Execute SQL but skip ProjectState updates
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	};

	// Apply migration with database_only=true
	executor
		.apply_migrations(&[database_only_migration])
		.await
		.expect("Failed to apply database_only migration");

	// Verify that the table WAS created in the database
	// (database_only still executes SQL operations)
	let table_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
	)
	.bind("database_only_table")
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to check table");

	assert!(
		table_exists,
		"Table should be created when database_only=true (SQL is executed)"
	);

	// Verify we can insert data into the table
	sqlx::query("INSERT INTO database_only_table (data) VALUES ('test data')")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert data");

	// Note: ProjectState updates are currently not implemented in the executor,
	// so we cannot verify that ProjectState was not updated.
	// When ProjectState management is implemented, this test should be extended
	// to verify that the model is not reflected in ProjectState.
}
