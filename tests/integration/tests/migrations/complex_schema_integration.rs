//! Integration tests for complex schema and data scenarios
//!
//! Tests migration system behavior with complex database structures:
//! - Large-scale schema changes (100+ tables)
//! - Composite foreign keys and complex relationships
//! - Circular dependencies between tables
//! - Virtual/generated columns (PostgreSQL)
//! - Spatial data types (PostGIS)
//!
//! **Test Coverage:**
//! - Bulk schema modifications
//! - Multi-column primary keys and foreign keys
//! - Self-referential and circular foreign keys
//! - Computed columns with automatic updates
//! - Geographic/geometry data types
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

/// Create a NOT NULL column
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

// ============================================================================
// Large-Scale Schema Changes Tests
// ============================================================================

/// Test large-scale schema changes with many tables
///
/// **Test Intent**: Verify that migration system can efficiently handle
/// bulk schema modifications affecting 100+ tables simultaneously,
/// completing within reasonable time and memory constraints
///
/// **Integration Point**: Migration executor → Bulk DDL operations → Performance
///
/// **Expected Behavior**: All tables modified successfully, operation
/// completes in <10 seconds, memory usage remains stable
#[rstest]
#[tokio::test]
#[serial(complex_schema)]
async fn test_large_scale_schema_changes(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create 100 tables
	// ============================================================================
	//
	// Scenario: Large application with many models (e.g., multi-tenant SaaS)
	// Goal: Add a common "created_at" column to all tables
	// Expected: Fast bulk modification

	let num_tables = 100;
	let fields_per_table = 5;

	println!(
		"Creating {} tables with {} fields each...",
		num_tables, fields_per_table
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	let setup_start = Instant::now();

	// Create all tables
	for table_idx in 0..num_tables {
		let table_name = leak_str(format!("table_{}", table_idx));

		let mut columns = vec![ColumnDefinition {
			name: "id".to_string(),
			type_definition: FieldType::Custom("SERIAL PRIMARY KEY".to_string()),
			not_null: true,
			unique: false,
			primary_key: true,
			auto_increment: true,
			default: None,
		}];

		for field_idx in 0..fields_per_table {
			columns.push(create_basic_column(
				leak_str(format!("field_{}", field_idx)),
				FieldType::VarChar(Some(100)),
			));
		}

		let migration = create_test_migration(
			"testapp",
			leak_str(format!("{:04}_create_table_{}", table_idx + 1, table_idx)),
			vec![Operation::CreateTable {
				name: table_name,
				columns,
			}],
		);

		executor
			.apply_migration(&migration)
			.await
			.expect(&format!("Failed to create table_{}", table_idx));

		if (table_idx + 1) % 20 == 0 {
			println!("  Created {} / {} tables", table_idx + 1, num_tables);
		}
	}

	let setup_duration = setup_start.elapsed();
	println!("Table creation completed in {:?}", setup_duration);

	// Verify all tables exist
	let initial_table_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		WHERE table_schema = 'public' AND table_name LIKE 'table_%'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count tables");
	assert_eq!(
		initial_table_count, num_tables as i64,
		"Should have {} tables",
		num_tables
	);

	// ============================================================================
	// Execute: Add column to all tables
	// ============================================================================
	//
	// In production, this would be done with a single migration containing
	// multiple AddColumn operations, or multiple migrations applied in sequence

	let bulk_change_start = Instant::now();

	for table_idx in 0..num_tables {
		let table_name = leak_str(format!("table_{}", table_idx));

		let add_column_migration = create_test_migration(
			"testapp",
			leak_str(format!(
				"{:04}_add_created_at_{}",
				num_tables + table_idx + 1,
				table_idx
			)),
			vec![Operation::AddColumn {
				table: table_name,
				column: ColumnDefinition {
					name: "created_at".to_string(),
					type_definition: FieldType::Timestamp,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("CURRENT_TIMESTAMP".to_string()),
				},
				mysql_options: None,
			}],
		);

		executor
			.apply_migration(&add_column_migration)
			.await
			.expect(&format!("Failed to add created_at to table_{}", table_idx));
	}

	let bulk_change_duration = bulk_change_start.elapsed();
	println!(
		"Bulk column addition completed in {:?}",
		bulk_change_duration
	);

	// ============================================================================
	// Assert: Verify bulk changes
	// ============================================================================

	// Performance assertion: Should complete in reasonable time
	assert!(
		bulk_change_duration < Duration::from_secs(10),
		"Bulk changes took {:?}, expected < 10s for {} tables",
		bulk_change_duration,
		num_tables
	);

	// Verify all tables have the new column
	let tables_with_created_at: i64 = sqlx::query_scalar(
		"SELECT COUNT(DISTINCT table_name) FROM information_schema.columns
		WHERE table_schema = 'public' AND table_name LIKE 'table_%' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count tables with created_at");

	assert_eq!(
		tables_with_created_at, num_tables as i64,
		"All {} tables should have created_at column",
		num_tables
	);

	// Verify column properties for a sample table
	let column_default: Option<String> = sqlx::query_scalar(
		"SELECT column_default FROM information_schema.columns
		WHERE table_name = 'table_0' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query column default");

	assert!(
		column_default.is_some(),
		"created_at should have default value"
	);

	// Test insertion with automatic timestamp
	sqlx::query("INSERT INTO table_0 (field_0) VALUES ($1)")
		.bind("test")
		.execute(&*pool)
		.await
		.expect("Failed to insert test record");

	let created_at_value: Option<chrono::NaiveDateTime> =
		sqlx::query_scalar("SELECT created_at FROM table_0 WHERE field_0 = 'test'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch created_at");

	assert!(
		created_at_value.is_some(),
		"created_at should be automatically set"
	);

	println!("\n=== Large-Scale Schema Summary ===");
	println!("Total tables: {}", num_tables);
	println!("Setup time: {:?}", setup_duration);
	println!("Bulk change time: {:?}", bulk_change_duration);
	println!("==================================\n");
}

// ============================================================================
// Composite Foreign Keys Tests
// ============================================================================

/// Test migration of composite foreign keys
///
/// **Test Intent**: Verify that migration system correctly handles
/// multi-column primary keys and foreign keys, maintaining referential
/// integrity across composite relationships
///
/// **Integration Point**: Schema introspection → Composite constraint detection → FK creation
///
/// **Expected Behavior**: Composite PK and FK constraints created correctly,
/// referential integrity enforced, data insertion respects constraints
#[rstest]
#[tokio::test]
#[serial(complex_schema)]
async fn test_composite_foreign_keys_migration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create tables with composite primary keys
	// ============================================================================
	//
	// Scenario: Order system with composite keys
	// - orders table: composite PK (order_id, customer_id)
	// - order_items table: composite FK referencing orders

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	// Create orders table with composite PK
	let create_orders_migration = create_test_migration(
		"orders",
		"0001_create_orders",
		vec![Operation::CreateTable {
			name: leak_str("orders").to_string(),
			columns: vec![
				create_not_null_column("order_id", FieldType::Integer),
				create_not_null_column("customer_id", FieldType::Integer),
				create_basic_column(
					"total_amount",
					FieldType::Custom("DECIMAL(10, 2)".to_string()),
				),
				create_basic_column("status", FieldType::VarChar(Some(20))),
			],
		}],
	);

	executor
		.apply_migration(&create_orders_migration)
		.await
		.expect("Failed to create orders table");

	// Add composite primary key constraint
	sqlx::query("ALTER TABLE orders ADD CONSTRAINT pk_orders PRIMARY KEY (order_id, customer_id)")
		.execute(&*pool)
		.await
		.expect("Failed to add composite PK");

	// Create order_items table
	let create_order_items_migration = create_test_migration(
		"orders",
		"0002_create_order_items",
		vec![Operation::CreateTable {
			name: leak_str("order_items").to_string(),
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
				create_not_null_column("order_id", FieldType::Integer),
				create_not_null_column("customer_id", FieldType::Integer),
				create_basic_column("product_name", FieldType::VarChar(Some(200))),
				create_basic_column("quantity", FieldType::Integer),
				create_basic_column("price", FieldType::Custom("DECIMAL(10, 2)".to_string())),
			],
		}],
	);

	executor
		.apply_migration(&create_order_items_migration)
		.await
		.expect("Failed to create order_items table");

	// ============================================================================
	// Execute: Add composite foreign key
	// ============================================================================

	sqlx::query(
		"ALTER TABLE order_items
		ADD CONSTRAINT fk_order_items_orders
		FOREIGN KEY (order_id, customer_id)
		REFERENCES orders(order_id, customer_id)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to add composite FK");

	// ============================================================================
	// Assert: Verify composite key constraints
	// ============================================================================

	// Verify composite PK exists
	let pk_constraint_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_constraint
		WHERE conrelid = 'orders'::regclass AND contype = 'p' AND conname = 'pk_orders'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count PK constraints");
	assert_eq!(pk_constraint_count, 1, "Composite PK should exist");

	// Verify composite FK exists
	let fk_constraint_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_constraint
		WHERE conrelid = 'order_items'::regclass AND contype = 'f' AND conname = 'fk_order_items_orders'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count FK constraints");
	assert_eq!(fk_constraint_count, 1, "Composite FK should exist");

	// Test referential integrity: Insert valid order
	sqlx::query(
		"INSERT INTO orders (order_id, customer_id, total_amount, status) VALUES ($1, $2, $3, $4)",
	)
	.bind(1001)
	.bind(5001)
	.bind(199.99)
	.bind("pending")
	.execute(&*pool)
	.await
	.expect("Failed to insert order");

	// Test referential integrity: Insert valid order item (matching FK)
	let valid_item_result = sqlx::query(
		"INSERT INTO order_items (order_id, customer_id, product_name, quantity, price)
		VALUES ($1, $2, $3, $4, $5)",
	)
	.bind(1001)
	.bind(5001)
	.bind("Widget")
	.bind(2)
	.bind(99.99)
	.execute(&*pool)
	.await;

	assert!(
		valid_item_result.is_ok(),
		"Should allow order item with valid composite FK"
	);

	// Test referential integrity: Insert invalid order item (FK violation)
	let invalid_item_result = sqlx::query(
		"INSERT INTO order_items (order_id, customer_id, product_name, quantity, price)
		VALUES ($1, $2, $3, $4, $5)",
	)
	.bind(1001)
	.bind(9999) // Non-existent customer_id
	.bind("Invalid Widget")
	.bind(1)
	.bind(49.99)
	.execute(&*pool)
	.await;

	assert!(
		invalid_item_result.is_err(),
		"Should reject order item with invalid composite FK"
	);

	let error_msg = invalid_item_result.unwrap_err().to_string();
	assert!(
		error_msg.contains("foreign key") || error_msg.contains("violates"),
		"Error should indicate FK violation: {}",
		error_msg
	);

	// Verify data integrity
	let order_items_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM order_items WHERE order_id = 1001 AND customer_id = 5001",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count order items");
	assert_eq!(order_items_count, 1, "Should have 1 valid order item");

	// Test composite key uniqueness: Attempt duplicate order
	let duplicate_order_result = sqlx::query(
		"INSERT INTO orders (order_id, customer_id, total_amount, status) VALUES ($1, $2, $3, $4)",
	)
	.bind(1001)
	.bind(5001) // Duplicate composite key
	.bind(299.99)
	.bind("confirmed")
	.execute(&*pool)
	.await;

	assert!(
		duplicate_order_result.is_err(),
		"Should reject duplicate composite PK"
	);

	// Test partial uniqueness: Different customer_id with same order_id should succeed
	let different_customer_result = sqlx::query(
		"INSERT INTO orders (order_id, customer_id, total_amount, status) VALUES ($1, $2, $3, $4)",
	)
	.bind(1001)
	.bind(5002) // Different customer_id
	.bind(149.99)
	.bind("pending")
	.execute(&*pool)
	.await;

	assert!(
		different_customer_result.is_ok(),
		"Should allow same order_id with different customer_id"
	);

	let total_orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count orders");
	assert_eq!(
		total_orders, 2,
		"Should have 2 orders with different composite keys"
	);
}

// ============================================================================
// Circular Dependencies Tests
// ============================================================================

/// Test migration of tables with circular dependencies
///
/// **Test Intent**: Verify that migration system can handle circular
/// foreign key relationships through deferred constraint creation,
/// allowing proper data insertion without deadlocks
///
/// **Integration Point**: Migration executor → Deferred constraint creation → Circular FK handling
///
/// **Expected Behavior**: Circular references created successfully,
/// data insertion works with proper ordering, no deadlocks
#[rstest]
#[tokio::test]
#[serial(complex_schema)]
async fn test_circular_dependencies_with_data(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create tables with circular dependencies
	// ============================================================================
	//
	// Scenario: Organizational structure with circular references
	// - User → Department (department_id FK)
	// - User → User (manager_id FK, self-referential)
	// - Department → User (head_user_id FK)
	//
	// Challenge: Cannot create both FKs immediately due to circular dependency

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	// Phase 1: Create users table without FK constraints
	let create_users_migration = create_test_migration(
		"org",
		"0001_create_users",
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
				create_basic_column("name", FieldType::VarChar(Some(100))),
				create_basic_column("email", FieldType::VarChar(Some(255))),
				create_basic_column("manager_id", FieldType::Integer), // Self-referential FK (deferred)
				create_basic_column("department_id", FieldType::Integer), // FK to departments (deferred)
			],
		}],
	);

	executor
		.apply_migration(&create_users_migration)
		.await
		.expect("Failed to create users table");

	// Phase 2: Create departments table without FK constraints
	let create_departments_migration = create_test_migration(
		"org",
		"0002_create_departments",
		vec![Operation::CreateTable {
			name: leak_str("departments").to_string(),
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
				create_basic_column("head_user_id", FieldType::Integer), // FK to users (deferred)
			],
		}],
	);

	executor
		.apply_migration(&create_departments_migration)
		.await
		.expect("Failed to create departments table");

	// ============================================================================
	// Execute: Insert test data before adding FK constraints
	// ============================================================================
	//
	// Strategy: Insert data first, then add FK constraints
	// This avoids chicken-and-egg problem with circular dependencies

	// Insert users without department assignments
	for i in 1..=100 {
		sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
			.bind(format!("User {}", i))
			.bind(format!("user{}@example.com", i))
			.execute(&*pool)
			.await
			.expect(&format!("Failed to insert user {}", i));
	}

	// Insert departments
	for i in 1..=10 {
		sqlx::query("INSERT INTO departments (name) VALUES ($1)")
			.bind(format!("Department {}", i))
			.execute(&*pool)
			.await
			.expect(&format!("Failed to insert department {}", i));
	}

	// Update users with department assignments and managers
	for i in 1..=100 {
		let department_id = (i % 10) + 1; // Distribute users across 10 departments
		let manager_id = if i > 10 { Some(i - 10) } else { None }; // First 10 users have no manager

		if let Some(mgr_id) = manager_id {
			sqlx::query("UPDATE users SET department_id = $1, manager_id = $2 WHERE id = $3")
				.bind(department_id)
				.bind(mgr_id)
				.bind(i)
				.execute(&*pool)
				.await
				.expect(&format!("Failed to update user {}", i));
		} else {
			sqlx::query("UPDATE users SET department_id = $1 WHERE id = $2")
				.bind(department_id)
				.bind(i)
				.execute(&*pool)
				.await
				.expect(&format!("Failed to update user {}", i));
		}
	}

	// Update departments with head users (first user in each department)
	for i in 1..=10 {
		sqlx::query("UPDATE departments SET head_user_id = $1 WHERE id = $2")
			.bind(i)
			.bind(i)
			.execute(&*pool)
			.await
			.expect(&format!("Failed to update department {}", i));
	}

	// ============================================================================
	// Execute: Add FK constraints after data insertion
	// ============================================================================

	// Add self-referential FK: users.manager_id → users.id
	sqlx::query(
		"ALTER TABLE users
		ADD CONSTRAINT fk_users_manager
		FOREIGN KEY (manager_id) REFERENCES users(id)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to add self-referential FK");

	// Add FK: users.department_id → departments.id
	sqlx::query(
		"ALTER TABLE users
		ADD CONSTRAINT fk_users_department
		FOREIGN KEY (department_id) REFERENCES departments(id)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to add user-department FK");

	// Add FK: departments.head_user_id → users.id (completes circular dependency)
	sqlx::query(
		"ALTER TABLE departments
		ADD CONSTRAINT fk_departments_head_user
		FOREIGN KEY (head_user_id) REFERENCES users(id)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to add department-user FK");

	// ============================================================================
	// Assert: Verify circular dependency integrity
	// ============================================================================

	// Verify all FK constraints exist
	let user_fk_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_constraint
		WHERE conrelid = 'users'::regclass AND contype = 'f'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count user FK constraints");
	assert_eq!(user_fk_count, 2, "users table should have 2 FK constraints");

	let dept_fk_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_constraint
		WHERE conrelid = 'departments'::regclass AND contype = 'f'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count department FK constraints");
	assert_eq!(
		dept_fk_count, 1,
		"departments table should have 1 FK constraint"
	);

	// Verify data integrity: All users have valid department_id
	let users_with_valid_dept: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM users u
		WHERE u.department_id IS NOT NULL
		AND EXISTS (SELECT 1 FROM departments d WHERE d.id = u.department_id)",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count users with valid departments");
	assert_eq!(
		users_with_valid_dept, 100,
		"All 100 users should have valid department_id"
	);

	// Verify data integrity: All departments have valid head_user_id
	let depts_with_valid_head: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM departments d
		WHERE d.head_user_id IS NOT NULL
		AND EXISTS (SELECT 1 FROM users u WHERE u.id = d.head_user_id)",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count departments with valid head users");
	assert_eq!(
		depts_with_valid_head, 10,
		"All 10 departments should have valid head_user_id"
	);

	// Verify self-referential FK: Users with managers have valid manager_id
	let users_with_valid_manager: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM users u1
		WHERE u1.manager_id IS NOT NULL
		AND EXISTS (SELECT 1 FROM users u2 WHERE u2.id = u1.manager_id)",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count users with valid managers");
	assert_eq!(
		users_with_valid_manager, 90,
		"90 users should have valid manager_id (100 total - 10 top-level)"
	);

	// Test constraint enforcement: Attempt to insert user with invalid department
	let invalid_dept_result = sqlx::query(
		"INSERT INTO users (name, email, department_id) VALUES ($1, $2, $3)",
	)
	.bind("Invalid User")
	.bind("invalid@example.com")
	.bind(9999) // Non-existent department
	.execute(&*pool)
	.await;

	assert!(
		invalid_dept_result.is_err(),
		"Should reject user with invalid department_id"
	);

	// Test constraint enforcement: Attempt to insert user with invalid manager
	let invalid_manager_result = sqlx::query(
		"INSERT INTO users (name, email, manager_id) VALUES ($1, $2, $3)",
	)
	.bind("Invalid Manager User")
	.bind("invalidmgr@example.com")
	.bind(9999) // Non-existent manager
	.execute(&*pool)
	.await;

	assert!(
		invalid_manager_result.is_err(),
		"Should reject user with invalid manager_id"
	);

	// Verify no deadlocks occurred during data insertion and FK creation
	let final_user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users");
	assert_eq!(final_user_count, 100, "Should have 100 users");

	let final_dept_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM departments")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count departments");
	assert_eq!(final_dept_count, 10, "Should have 10 departments");
}

// ============================================================================
// Virtual/Generated Columns Tests
// ============================================================================

/// Test migration with virtual/generated columns (PostgreSQL GENERATED COLUMN)
///
/// **Test Intent**: Verify that migration system can create and manage
/// generated columns that are automatically computed from other columns
///
/// **Integration Point**: Schema creation → Generated column definition → Automatic computation
///
/// **Expected Behavior**: Generated column created correctly, values
/// computed automatically on insert/update, cannot be manually set
#[rstest]
#[tokio::test]
#[serial(complex_schema)]
async fn test_virtual_or_generated_columns(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create table with basic columns
	// ============================================================================
	//
	// Scenario: Product pricing with tax calculation
	// - price: base price
	// - tax_rate: tax rate (decimal, e.g., 0.10 for 10%)
	// - total_price: GENERATED ALWAYS AS (price * (1 + tax_rate))

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	let create_products_migration = create_test_migration(
		"products",
		"0001_create_products",
		vec![Operation::CreateTable {
			name: leak_str("products").to_string(),
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
				create_basic_column("name", FieldType::VarChar(Some(200))),
				create_basic_column("price", FieldType::Custom("DECIMAL(10, 2)".to_string())),
				create_basic_column("tax_rate", FieldType::Custom("DECIMAL(5, 4)".to_string())),
			],
		}],
	);

	executor
		.apply_migration(&create_products_migration)
		.await
		.expect("Failed to create products table");

	// ============================================================================
	// Execute: Add generated column
	// ============================================================================
	//
	// PostgreSQL 12+ supports GENERATED ALWAYS AS (expression) STORED
	// Note: VIRTUAL (not stored) is not supported in PostgreSQL

	sqlx::query(
		"ALTER TABLE products
		ADD COLUMN total_price DECIMAL(10, 2)
		GENERATED ALWAYS AS (price * (1 + tax_rate)) STORED",
	)
	.execute(&*pool)
	.await
	.expect("Failed to add generated column");

	// ============================================================================
	// Assert: Verify generated column functionality
	// ============================================================================

	// Verify column exists and is generated
	let column_info: (String, String) = sqlx::query_as(
		"SELECT column_name, is_generated FROM information_schema.columns
		WHERE table_name = 'products' AND column_name = 'total_price'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query column info");

	assert_eq!(column_info.0, "total_price", "Column should be total_price");
	assert_eq!(column_info.1, "ALWAYS", "Column should be generated");

	// Test automatic computation: Insert product
	sqlx::query(
		"INSERT INTO products (name, price, tax_rate) VALUES ($1, $2, $3)",
	)
	.bind("Widget")
	.bind(100.00)
	.bind(0.10) // 10% tax
	.execute(&*pool)
	.await
	.expect("Failed to insert product");

	// Verify computed value: 100.00 * (1 + 0.10) = 110.00
	let total_price: f64 =
		sqlx::query_scalar("SELECT total_price FROM products WHERE name = 'Widget'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch total_price");

	assert!(
		(total_price - 110.00).abs() < 0.01,
		"total_price should be 110.00, got {}",
		total_price
	);

	// Test update: Changing price should update total_price automatically
	sqlx::query("UPDATE products SET price = $1 WHERE name = $2")
		.bind(150.00)
		.bind("Widget")
		.execute(&*pool)
		.await
		.expect("Failed to update product price");

	let updated_total: f64 =
		sqlx::query_scalar("SELECT total_price FROM products WHERE name = 'Widget'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch updated total_price");

	// New total: 150.00 * (1 + 0.10) = 165.00
	assert!(
		(updated_total - 165.00).abs() < 0.01,
		"Updated total_price should be 165.00, got {}",
		updated_total
	);

	// Test update: Changing tax_rate should update total_price automatically
	sqlx::query("UPDATE products SET tax_rate = $1 WHERE name = $2")
		.bind(0.20) // 20% tax
		.bind("Widget")
		.execute(&*pool)
		.await
		.expect("Failed to update tax_rate");

	let new_total: f64 =
		sqlx::query_scalar("SELECT total_price FROM products WHERE name = 'Widget'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch new total_price");

	// New total: 150.00 * (1 + 0.20) = 180.00
	assert!(
		(new_total - 180.00).abs() < 0.01,
		"New total_price should be 180.00, got {}",
		new_total
	);

	// Test constraint: Cannot manually set generated column
	let manual_set_result = sqlx::query(
		"INSERT INTO products (name, price, tax_rate, total_price) VALUES ($1, $2, $3, $4)",
	)
	.bind("Gadget")
	.bind(200.00)
	.bind(0.15)
	.bind(999.99) // Attempt to manually set generated column
	.execute(&*pool)
	.await;

	assert!(
		manual_set_result.is_err(),
		"Should reject manual assignment to generated column"
	);

	// Insert without specifying total_price (correct usage)
	sqlx::query(
		"INSERT INTO products (name, price, tax_rate) VALUES ($1, $2, $3)",
	)
	.bind("Gadget")
	.bind(200.00)
	.bind(0.15) // 15% tax
	.execute(&*pool)
	.await
	.expect("Failed to insert product without total_price");

	let gadget_total: f64 =
		sqlx::query_scalar("SELECT total_price FROM products WHERE name = 'Gadget'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch gadget total_price");

	// Expected: 200.00 * (1 + 0.15) = 230.00
	assert!(
		(gadget_total - 230.00).abs() < 0.01,
		"Gadget total_price should be 230.00, got {}",
		gadget_total
	);

	let product_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count products");
	assert_eq!(product_count, 2, "Should have 2 products");
}

// ============================================================================
// Spatial Data Types Tests
// ============================================================================

/// Test migration with spatial data types (PostGIS)
///
/// **Test Intent**: Verify that migration system can handle PostGIS
/// spatial data types and spatial indexes (GiST)
///
/// **Integration Point**: PostGIS extension → Spatial column creation → GiST indexing
///
/// **Expected Behavior**: Spatial columns created correctly, spatial
/// indexes function properly, geometric operations work
#[rstest]
#[tokio::test]
#[serial(complex_schema)]
async fn test_spatial_data_types_migration(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Enable PostGIS extension
	// ============================================================================
	//
	// Note: This test requires PostGIS to be available in the PostgreSQL container
	// If PostGIS is not available, the test will skip gracefully

	let postgis_available = sqlx::query("CREATE EXTENSION IF NOT EXISTS postgis")
		.execute(&*pool)
		.await;

	if postgis_available.is_err() {
		println!("PostGIS extension not available, skipping spatial data type test");
		return;
	}

	println!("PostGIS extension enabled successfully");

	// ============================================================================
	// Execute: Create table with spatial column
	// ============================================================================

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	let create_locations_migration = create_test_migration(
		"locations",
		"0001_create_locations",
		vec![Operation::CreateTable {
			name: leak_str("locations").to_string(),
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
				create_basic_column("name", FieldType::VarChar(Some(200))),
				create_basic_column(
					"point",
					FieldType::Custom("GEOMETRY(Point, 4326)".to_string()),
				),
			],
		}],
	);

	executor
		.apply_migration(&create_locations_migration)
		.await
		.expect("Failed to create locations table");

	// Add spatial index (GiST)
	sqlx::query("CREATE INDEX idx_locations_point ON locations USING GIST (point)")
		.execute(&*pool)
		.await
		.expect("Failed to create GiST index");

	// ============================================================================
	// Assert: Verify spatial functionality
	// ============================================================================

	// Verify GiST index exists
	let gist_index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes
		WHERE tablename = 'locations' AND indexname = 'idx_locations_point'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count GiST indexes");
	assert_eq!(gist_index_count, 1, "GiST index should exist");

	// Verify index type is GiST
	let index_type: String = sqlx::query_scalar(
		"SELECT indexdef FROM pg_indexes
		WHERE tablename = 'locations' AND indexname = 'idx_locations_point'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query index definition");
	assert!(
		index_type.contains("USING gist"),
		"Index should use GiST: {}",
		index_type
	);

	// Insert test locations
	sqlx::query(
		"INSERT INTO locations (name, point) VALUES ($1, ST_SetSRID(ST_MakePoint($2, $3), 4326))",
	)
	.bind("Tokyo Station")
	.bind(139.7673) // Longitude
	.bind(35.6812) // Latitude
	.execute(&*pool)
	.await
	.expect("Failed to insert Tokyo location");

	sqlx::query(
		"INSERT INTO locations (name, point) VALUES ($1, ST_SetSRID(ST_MakePoint($2, $3), 4326))",
	)
	.bind("Shibuya Station")
	.bind(139.7016)
	.bind(35.6580)
	.execute(&*pool)
	.await
	.expect("Failed to insert Shibuya location");

	sqlx::query(
		"INSERT INTO locations (name, point) VALUES ($1, ST_SetSRID(ST_MakePoint($2, $3), 4326))",
	)
	.bind("Shinjuku Station")
	.bind(139.7006)
	.bind(35.6896)
	.execute(&*pool)
	.await
	.expect("Failed to insert Shinjuku location");

	// Test spatial query: Find locations within 5km of Tokyo Station
	let nearby_locations: Vec<String> = sqlx::query_scalar(
		"SELECT name FROM locations
		WHERE ST_DWithin(point, ST_SetSRID(ST_MakePoint(139.7673, 35.6812), 4326)::geography, 5000)
		AND name != 'Tokyo Station'
		ORDER BY name",
	)
	.fetch_all(&*pool)
	.await
	.expect("Failed to query nearby locations");

	assert!(
		nearby_locations.len() >= 1,
		"Should find at least 1 location within 5km of Tokyo Station"
	);
	assert!(
		nearby_locations.contains(&"Shibuya Station".to_string())
			|| nearby_locations.contains(&"Shinjuku Station".to_string()),
		"Should find either Shibuya or Shinjuku within 5km: {:?}",
		nearby_locations
	);

	// Test spatial function: Calculate distance between two points
	let distance: f64 = sqlx::query_scalar(
		"SELECT ST_Distance(
			ST_SetSRID(ST_MakePoint(139.7673, 35.6812), 4326)::geography,
			ST_SetSRID(ST_MakePoint(139.7016, 35.6580), 4326)::geography
		)",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to calculate distance");

	// Distance between Tokyo Station and Shibuya Station should be ~5-7km
	assert!(
		distance > 4000.0 && distance < 8000.0,
		"Distance should be ~5-7km, got {}m",
		distance
	);

	let location_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM locations")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count locations");
	assert_eq!(location_count, 3, "Should have 3 locations");

	println!("\n=== Spatial Data Test Summary ===");
	println!("PostGIS extension: enabled");
	println!("GiST index: created");
	println!("Spatial queries: functional");
	println!("Distance calculation: functional");
	println!("=================================\n");
}

// ============================================================================
// Composite Primary Key Tests
// ============================================================================

/// Test composite primary key creation and data integrity
///
/// **Test Intent**: Verify that composite primary keys (multi-column PKs) can be
/// created, modified, and referenced correctly, with proper uniqueness constraints
/// and referential integrity enforcement.
///
/// **Integration Point**: Composite PK definition → Uniqueness constraints → FK references
///
/// **Expected Behavior**: Composite primary keys should:
/// 1. Enforce uniqueness across the combination of columns
/// 2. Allow duplicate values in individual columns
/// 3. Be referenceable by foreign keys from other tables
/// 4. Support data integrity constraints
#[rstest]
#[tokio::test]
#[serial(complex_schema)]
async fn test_composite_primary_key_creation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create tables with composite primary keys
	// ============================================================================
	//
	// Scenario: E-commerce system with order line items
	// - Orders table: Simple PK (order_id)
	// - OrderItems table: Composite PK (order_id, line_number)
	// - ProductTags table: Composite PK (product_id, tag_id) - many-to-many

	// Create Orders table (simple PK for reference)
	let create_orders_migration = create_test_migration(
		"commerce",
		"0001_create_orders",
		vec![Operation::CreateTable {
			name: leak_str("orders").to_string(),
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
				create_basic_column("customer_name", FieldType::VarChar(Some(200))),
				ColumnDefinition {
					name: "created_at".to_string(),
					type_definition: FieldType::Custom("TIMESTAMP".to_string()),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("CURRENT_TIMESTAMP".to_string()),
				},
			],
		}],
	);

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	executor
		.apply_migration(&create_orders_migration)
		.await
		.expect("Failed to create orders table");

	// Insert test orders
	sqlx::query("INSERT INTO orders (customer_name) VALUES ($1)")
		.bind("Customer A")
		.execute(&*pool)
		.await
		.expect("Failed to insert order 1");

	sqlx::query("INSERT INTO orders (customer_name) VALUES ($1)")
		.bind("Customer B")
		.execute(&*pool)
		.await
		.expect("Failed to insert order 2");

	// ============================================================================
	// Execute: Create table with composite primary key
	// ============================================================================

	// Create OrderItems table with composite PK (order_id, line_number).
	// Build it in three operations so every schema change flows through a
	// native `Operation` variant: CREATE TABLE, then attach the composite PK,
	// then add the FK constraint.
	let create_order_items_migration = create_test_migration(
		"commerce",
		"0002_create_order_items",
		vec![
			Operation::CreateTable {
				name: leak_str("order_items").to_string(),
				columns: vec![
					ColumnDefinition {
						name: "order_id".to_string(),
						type_definition: FieldType::Integer,
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "line_number".to_string(),
						type_definition: FieldType::Integer,
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					create_basic_column("product_name", FieldType::VarChar(Some(200))),
					ColumnDefinition {
						name: "quantity".to_string(),
						type_definition: FieldType::Integer,
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "price".to_string(),
						type_definition: FieldType::Custom("DECIMAL(10, 2)".to_string()),
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
				],
			},
			Operation::CreateCompositePrimaryKey {
				table: "order_items".to_string(),
				columns: vec!["order_id".to_string(), "line_number".to_string()],
				constraint_name: None,
			},
			Operation::AddConstraint {
				table: "order_items".to_string(),
				constraint_sql:
					"CONSTRAINT order_items_order_id_fkey FOREIGN KEY (order_id) \
					 REFERENCES orders(id) ON DELETE CASCADE"
						.to_string(),
			},
		],
	);

	executor
		.apply_migration(&create_order_items_migration)
		.await
		.expect("Failed to create order_items table");

	// Verify composite PK exists
	let composite_pk_constraint: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.table_constraints
		WHERE table_name = 'order_items' AND constraint_type = 'PRIMARY KEY'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query composite PK constraint");
	assert_eq!(
		composite_pk_constraint, 1,
		"order_items should have a PRIMARY KEY constraint"
	);

	// Verify composite PK has 2 columns
	let pk_columns: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.key_column_usage
		WHERE table_name = 'order_items'
		AND constraint_name = (
			SELECT constraint_name FROM information_schema.table_constraints
			WHERE table_name = 'order_items' AND constraint_type = 'PRIMARY KEY'
		)",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count PK columns");
	assert_eq!(pk_columns, 2, "Composite PK should have 2 columns");

	// ============================================================================
	// Assert: Test composite PK uniqueness constraints
	// ============================================================================

	// Test 1: Insert valid data (order_id=1, line_number=1)
	let insert_1_1 = sqlx::query(
		"INSERT INTO order_items (order_id, line_number, product_name, quantity, price)
		VALUES ($1, $2, $3, $4, $5)",
	)
	.bind(1) // order_id
	.bind(1) // line_number
	.bind("Product A")
	.bind(10)
	.bind(100.00)
	.execute(&*pool)
	.await;
	assert!(insert_1_1.is_ok(), "Should insert (1, 1) successfully");

	// Test 2: Insert another line for same order (order_id=1, line_number=2)
	let insert_1_2 = sqlx::query(
		"INSERT INTO order_items (order_id, line_number, product_name, quantity, price)
		VALUES ($1, $2, $3, $4, $5)",
	)
	.bind(1) // order_id (same)
	.bind(2) // line_number (different)
	.bind("Product B")
	.bind(5)
	.bind(50.00)
	.execute(&*pool)
	.await;
	assert!(
		insert_1_2.is_ok(),
		"Should insert (1, 2) successfully - different line_number"
	);

	// Test 3: Insert same line_number but different order_id (order_id=2, line_number=1)
	let insert_2_1 = sqlx::query(
		"INSERT INTO order_items (order_id, line_number, product_name, quantity, price)
		VALUES ($1, $2, $3, $4, $5)",
	)
	.bind(2) // order_id (different)
	.bind(1) // line_number (same as first insert)
	.bind("Product C")
	.bind(3)
	.bind(30.00)
	.execute(&*pool)
	.await;
	assert!(
		insert_2_1.is_ok(),
		"Should insert (2, 1) successfully - different order_id"
	);

	// Test 4: Attempt duplicate composite key (order_id=1, line_number=1) - should FAIL
	let insert_duplicate = sqlx::query(
		"INSERT INTO order_items (order_id, line_number, product_name, quantity, price)
		VALUES ($1, $2, $3, $4, $5)",
	)
	.bind(1) // order_id (duplicate)
	.bind(1) // line_number (duplicate)
	.bind("Product D")
	.bind(1)
	.bind(10.00)
	.execute(&*pool)
	.await;
	assert!(
		insert_duplicate.is_err(),
		"Should FAIL to insert duplicate composite key (1, 1)"
	);

	let error_message = insert_duplicate.unwrap_err().to_string();
	assert!(
		error_message.contains("duplicate key") || error_message.contains("unique"),
		"Error should indicate duplicate key violation: {}",
		error_message
	);

	// Verify correct number of items inserted (3, not 4)
	let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM order_items")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count order_items");
	assert_eq!(
		item_count, 3,
		"Should have exactly 3 items (duplicate insert rejected)"
	);

	// ============================================================================
	// Test composite PK with foreign key references
	// ============================================================================

	// Create a table that references the composite PK
	let create_item_notes_migration = create_test_migration(
		"commerce",
		"0003_create_item_notes",
		vec![Operation::RunSQL {
			sql: leak_str(
				"CREATE TABLE item_notes (
					id SERIAL PRIMARY KEY,
					order_id INTEGER NOT NULL,
					line_number INTEGER NOT NULL,
					note TEXT NOT NULL,
					created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
					FOREIGN KEY (order_id, line_number)
						REFERENCES order_items(order_id, line_number)
						ON DELETE CASCADE
				)",
			),
			reverse_sql: Some("DROP TABLE item_notes"),
		}],
	);

	executor
		.apply_migration(&create_item_notes_migration)
		.await
		.expect("Failed to create item_notes table");

	// Test FK constraint: Insert valid reference
	let insert_note_valid = sqlx::query(
		"INSERT INTO item_notes (order_id, line_number, note) VALUES ($1, $2, $3)",
	)
	.bind(1) // order_id exists
	.bind(1) // line_number exists
	.bind("This is a note for order 1, line 1")
	.execute(&*pool)
	.await;
	assert!(
		insert_note_valid.is_ok(),
		"Should insert note with valid FK reference"
	);

	// Test FK constraint: Insert invalid reference (should FAIL)
	let insert_note_invalid = sqlx::query(
		"INSERT INTO item_notes (order_id, line_number, note) VALUES ($1, $2, $3)",
	)
	.bind(1) // order_id exists
	.bind(999) // line_number does NOT exist
	.bind("This note references non-existent line")
	.execute(&*pool)
	.await;
	assert!(
		insert_note_invalid.is_err(),
		"Should FAIL to insert note with invalid FK reference"
	);

	let fk_error = insert_note_invalid.unwrap_err().to_string();
	assert!(
		fk_error.contains("foreign key") || fk_error.contains("violates"),
		"Error should indicate FK violation: {}",
		fk_error
	);

	// Verify CASCADE DELETE: Delete order item should cascade to notes
	let delete_item =
		sqlx::query("DELETE FROM order_items WHERE order_id = $1 AND line_number = $2")
			.bind(1)
			.bind(1)
			.execute(&*pool)
			.await;
	assert!(delete_item.is_ok(), "Should delete order item successfully");

	// Verify note was cascaded
	let note_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item_notes")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count notes");
	assert_eq!(
		note_count, 0,
		"Note should be cascade deleted when order item is deleted"
	);

	// ============================================================================
	// Test many-to-many with composite PK
	// ============================================================================

	// Create Products and Tags tables
	sqlx::query(
		"CREATE TABLE products (
			id SERIAL PRIMARY KEY,
			name VARCHAR(200) NOT NULL
		)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create products table");

	sqlx::query(
		"CREATE TABLE tags (
			id SERIAL PRIMARY KEY,
			name VARCHAR(100) NOT NULL
		)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create tags table");

	// Create junction table with composite PK (product_id, tag_id)
	sqlx::query(
		"CREATE TABLE product_tags (
			product_id INTEGER NOT NULL,
			tag_id INTEGER NOT NULL,
			PRIMARY KEY (product_id, tag_id),
			FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
			FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
		)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create product_tags table");

	// Insert test data
	sqlx::query("INSERT INTO products (name) VALUES ($1), ($2)")
		.bind("Laptop")
		.bind("Mouse")
		.execute(&*pool)
		.await
		.expect("Failed to insert products");

	sqlx::query("INSERT INTO tags (name) VALUES ($1), ($2), ($3)")
		.bind("electronics")
		.bind("peripherals")
		.bind("wireless")
		.execute(&*pool)
		.await
		.expect("Failed to insert tags");

	// Tag product 1 with tags 1 and 2
	sqlx::query("INSERT INTO product_tags (product_id, tag_id) VALUES ($1, $2)")
		.bind(1)
		.bind(1)
		.execute(&*pool)
		.await
		.expect("Failed to tag product 1 with tag 1");

	sqlx::query("INSERT INTO product_tags (product_id, tag_id) VALUES ($1, $2)")
		.bind(1)
		.bind(2)
		.execute(&*pool)
		.await
		.expect("Failed to tag product 1 with tag 2");

	// Tag product 2 with tag 2 (same tag, different product)
	sqlx::query("INSERT INTO product_tags (product_id, tag_id) VALUES ($1, $2)")
		.bind(2)
		.bind(2)
		.execute(&*pool)
		.await
		.expect("Failed to tag product 2 with tag 2");

	// Attempt duplicate (product_id=1, tag_id=1) - should FAIL
	let duplicate_tag =
		sqlx::query("INSERT INTO product_tags (product_id, tag_id) VALUES ($1, $2)")
			.bind(1)
			.bind(1)
			.execute(&*pool)
			.await;
	assert!(
		duplicate_tag.is_err(),
		"Should FAIL to create duplicate product-tag association"
	);

	// Verify correct number of associations
	let association_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_tags")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count product_tags");
	assert_eq!(
		association_count, 3,
		"Should have exactly 3 product-tag associations"
	);

	// Verify many-to-many query: Product 1 has 2 tags
	let product1_tags: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM product_tags WHERE product_id = $1")
			.bind(1)
			.fetch_one(&*pool)
			.await
			.expect("Failed to count product 1 tags");
	assert_eq!(product1_tags, 2, "Product 1 should have 2 tags");

	// Verify many-to-many query: Tag 2 is used by 2 products
	let tag2_products: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM product_tags WHERE tag_id = $1")
			.bind(2)
			.fetch_one(&*pool)
			.await
			.expect("Failed to count tag 2 products");
	assert_eq!(tag2_products, 2, "Tag 2 should be used by 2 products");

	// ============================================================================
	// Test composite PK modification (add column to existing composite PK)
	// ============================================================================

	// Create a test table to modify
	sqlx::query(
		"CREATE TABLE versioned_data (
			entity_id INTEGER NOT NULL,
			version INTEGER NOT NULL,
			data TEXT,
			PRIMARY KEY (entity_id, version)
		)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create versioned_data table");

	// Insert test data
	sqlx::query("INSERT INTO versioned_data (entity_id, version, data) VALUES ($1, $2, $3)")
		.bind(1)
		.bind(1)
		.bind("v1 data")
		.execute(&*pool)
		.await
		.expect("Failed to insert v1 data");

	sqlx::query("INSERT INTO versioned_data (entity_id, version, data) VALUES ($1, $2, $3)")
		.bind(1)
		.bind(2)
		.bind("v2 data")
		.execute(&*pool)
		.await
		.expect("Failed to insert v2 data");

	// Modify composite PK: Drop old PK, add new column, create new 3-column PK
	// Step 1: Drop existing PK constraint
	sqlx::query("ALTER TABLE versioned_data DROP CONSTRAINT versioned_data_pkey")
		.execute(&*pool)
		.await
		.expect("Failed to drop PK constraint");

	// Step 2: Add new column
	sqlx::query("ALTER TABLE versioned_data ADD COLUMN tenant_id INTEGER NOT NULL DEFAULT 1")
		.execute(&*pool)
		.await
		.expect("Failed to add tenant_id column");

	// Step 3: Create new composite PK with 3 columns
	sqlx::query("ALTER TABLE versioned_data ADD PRIMARY KEY (tenant_id, entity_id, version)")
		.execute(&*pool)
		.await
		.expect("Failed to add new composite PK");

	// Verify new PK has 3 columns
	let new_pk_columns: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.key_column_usage
		WHERE table_name = 'versioned_data'
		AND constraint_name = (
			SELECT constraint_name FROM information_schema.table_constraints
			WHERE table_name = 'versioned_data' AND constraint_type = 'PRIMARY KEY'
		)",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count new PK columns");
	assert_eq!(new_pk_columns, 3, "New composite PK should have 3 columns");

	// Test new PK uniqueness: Same tenant_id, entity_id, version should fail
	let duplicate_3col = sqlx::query(
		"INSERT INTO versioned_data (tenant_id, entity_id, version, data) VALUES ($1, $2, $3, $4)",
	)
	.bind(1) // tenant_id (same)
	.bind(1) // entity_id (same)
	.bind(1) // version (same)
	.bind("duplicate v1")
	.execute(&*pool)
	.await;
	assert!(
		duplicate_3col.is_err(),
		"Should FAIL with 3-column composite PK duplicate"
	);

	// Test new PK uniqueness: Different tenant_id allows same entity_id + version
	let different_tenant = sqlx::query(
		"INSERT INTO versioned_data (tenant_id, entity_id, version, data) VALUES ($1, $2, $3, $4)",
	)
	.bind(2) // tenant_id (different)
	.bind(1) // entity_id (same)
	.bind(1) // version (same)
	.bind("tenant 2 v1")
	.execute(&*pool)
	.await;
	assert!(
		different_tenant.is_ok(),
		"Should succeed with different tenant_id in 3-column composite PK"
	);

	// ============================================================================
	// Composite PK verification summary
	// ============================================================================

	println!("\n=== Composite Primary Key Test Summary ===");
	println!("✓ 2-column composite PK (order_id, line_number): created and enforced");
	println!(
		"✓ Uniqueness constraint: individual columns can duplicate, combination must be unique"
	);
	println!("✓ Foreign key referencing composite PK: functional");
	println!("✓ CASCADE DELETE through composite FK: functional");
	println!("✓ Many-to-many with composite PK: functional");
	println!("✓ Composite PK modification (2 cols → 3 cols): successful");
	println!("✓ Multi-tenant composite PK pattern: functional");
	println!("===========================================\n");
}
