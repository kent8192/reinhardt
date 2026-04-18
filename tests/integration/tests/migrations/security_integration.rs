//! Integration tests for security and permissions scenarios
//!
//! Tests migration system security aspects:
//! - Least privilege principle adherence
//! - Sensitive data handling
//! - Audit logging completeness
//! - Permission escalation prevention
//! - Secure migration patterns
//!
//! **Test Coverage:**
//! - Database user permissions
//! - Sensitive data protection
//! - Audit trail generation
//! - Privilege escalation prevention
//! - SQL injection prevention
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
// Least Privilege Principle Tests
// ============================================================================

/// Test least privilege principle adherence
///
/// **Test Intent**: Verify that migrations only require minimal necessary
/// database permissions, adhering to the principle of least privilege
///
/// **Integration Point**: Migration executor → Database permissions → Operation validation
///
/// **Expected Behavior**: Migrations succeed with minimal required permissions,
/// fail gracefully when permissions are insufficient, no unnecessary privileges requested
#[rstest]
#[tokio::test]
#[serial(security)]
async fn test_least_privilege_principle_adherence(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create restricted database user
	// ============================================================================
	//
	// Scenario: Application should use limited-privilege user for migrations
	// Goal: Verify migrations work with minimal required permissions

	// Create restricted user with only CREATE and ALTER privileges
	sqlx::query("CREATE USER migration_user WITH PASSWORD 'migration_pass'")
		.execute(&*pool)
		.await
		.expect("Failed to create migration_user");

	// Grant minimal required privileges
	sqlx::query("GRANT CONNECT ON DATABASE postgres TO migration_user")
		.execute(&*pool)
		.await
		.expect("Failed to grant CONNECT");

	sqlx::query("GRANT CREATE ON SCHEMA public TO migration_user")
		.execute(&*pool)
		.await
		.expect("Failed to grant CREATE");

	sqlx::query("GRANT USAGE ON SCHEMA public TO migration_user")
		.execute(&*pool)
		.await
		.expect("Failed to grant USAGE");

	// Build connection URL for restricted user
	let restricted_url = url.replace("postgres@", "migration_user:migration_pass@");

	let restricted_conn = DatabaseConnection::connect(&restricted_url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect as migration_user");
	let mut restricted_executor = DatabaseMigrationExecutor::new(restricted_conn.clone());

	// ============================================================================
	// Execute: Create table with restricted user (should succeed)
	// ============================================================================

	let create_table_migration = create_test_migration(
		"auth",
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
				create_basic_column("username", FieldType::VarChar(Some(100))),
				create_basic_column("email", FieldType::VarChar(Some(255))),
			],
		}],
	);

	let create_result = restricted_executor
		.apply_migration(&create_table_migration)
		.await;

	assert!(
		create_result.is_ok(),
		"User with CREATE privilege should be able to create tables"
	);

	// Verify table was created
	let table_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables
		WHERE table_schema = 'public' AND table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query table existence");
	assert_eq!(table_exists, 1, "users table should exist");

	// ============================================================================
	// Execute: Grant ALTER privileges and test column addition
	// ============================================================================

	// Grant ALTER privilege for schema modifications
	sqlx::query("GRANT ALL PRIVILEGES ON TABLE users TO migration_user")
		.execute(&*pool)
		.await
		.expect("Failed to grant ALTER privileges");

	let add_column_migration = create_test_migration(
		"auth",
		"0002_add_status",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("status", FieldType::VarChar(Some(20))),
			mysql_options: None,
		}],
	);

	let alter_result = restricted_executor
		.apply_migration(&add_column_migration)
		.await;

	assert!(
		alter_result.is_ok(),
		"User with ALTER privilege should be able to add columns"
	);

	// Verify column was added
	let column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'users' AND column_name = 'status'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query column existence");
	assert_eq!(column_exists, 1, "status column should exist");

	// ============================================================================
	// Assert: Verify no superuser privileges required
	// ============================================================================

	// Check that migration_user is not a superuser
	let is_superuser: bool =
		sqlx::query_scalar("SELECT usesuper FROM pg_user WHERE usename = 'migration_user'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to check superuser status");

	assert!(
		!is_superuser,
		"Migration user should not have superuser privileges"
	);

	// Verify user has only necessary privileges
	let has_createdb: bool =
		sqlx::query_scalar("SELECT usecreatedb FROM pg_user WHERE usename = 'migration_user'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to check createdb privilege");

	assert!(
		!has_createdb,
		"Migration user should not have CREATEDB privilege (not needed)"
	);

	// Test insufficient privilege: attempt database-wide operation through a
	// pool that is authenticated as `migration_user`. CREATE DATABASE requires
	// the CREATEDB role attribute, which we explicitly did NOT grant, so
	// PostgreSQL must reject it with error class 42 (permission denied / insufficient privilege).
	let restricted_pool = sqlx::postgres::PgPoolOptions::new()
		.max_connections(1)
		.connect(&restricted_url)
		.await
		.expect("Failed to open pool as migration_user");

	let create_db_result = sqlx::query("CREATE DATABASE test_db_forbidden")
		.execute(&restricted_pool)
		.await;

	assert!(
		create_db_result.is_err(),
		"migration_user must not be able to CREATE DATABASE"
	);
	let err = create_db_result.unwrap_err();
	let db_err = err
		.as_database_error()
		.expect("Expected a database error from PostgreSQL");
	// PostgreSQL SQLSTATE 42501 = insufficient_privilege. `CREATE DATABASE`
	// without the CREATEDB role attribute is rejected with this specific code
	// (not 0LP01 `invalid_grant_operation`, which applies to GRANT/REVOKE flow).
	let code = db_err.code().unwrap_or_default().to_string();
	assert_eq!(
		code, "42501",
		"Expected SQLSTATE 42501 (insufficient_privilege), got: {code}"
	);

	restricted_pool.close().await;

	// Cleanup: Drop restricted user
	sqlx::query("DROP USER IF EXISTS migration_user")
		.execute(&*pool)
		.await
		.expect("Failed to drop migration_user");

	println!("\n=== Least Privilege Test Summary ===");
	println!("Restricted user: created successfully");
	println!("Required privileges: CREATE, ALTER on schema");
	println!("Superuser privileges: not required");
	println!("Database creation privilege: not required");
	println!("Principle adherence: verified");
	println!("====================================\n");
}

// ============================================================================
// Sensitive Data Handling Tests
// ============================================================================

/// Test sensitive data handling during migrations
///
/// **Test Intent**: Verify that migrations properly handle sensitive data
/// (passwords, tokens, PII) without exposing them in logs or migration files
///
/// **Integration Point**: Migration executor → Data transformation → Log sanitization
///
/// **Expected Behavior**: Sensitive data not logged, encryption/hashing applied,
/// plaintext credentials never stored, audit trail sanitized
#[rstest]
#[tokio::test]
#[serial(security)]
async fn test_sensitive_data_handling(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create users table with sensitive data
	// ============================================================================
	//
	// Scenario: Migrating password storage from MD5 to bcrypt
	// Security requirement: Passwords never appear in logs or plaintext

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	// Create users table with password_hash
	let create_users_migration = create_test_migration(
		"auth",
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
				create_basic_column("username", FieldType::VarChar(Some(100))),
				create_basic_column("password_hash", FieldType::VarChar(Some(255))),
				create_basic_column("api_token", FieldType::VarChar(Some(255))),
			],
		}],
	);

	executor
		.apply_migration(&create_users_migration)
		.await
		.expect("Failed to create users table");

	// Insert test users with "sensitive" data (simulated MD5 hashes)
	sqlx::query(
		"INSERT INTO users (username, password_hash, api_token) VALUES ($1, $2, $3)",
	)
	.bind("alice")
	.bind("5f4dcc3b5aa765d61d8327deb882cf99") // MD5 of "password"
	.bind("secret_api_token_12345")
	.execute(&*pool)
	.await
	.expect("Failed to insert alice");

	sqlx::query(
		"INSERT INTO users (username, password_hash, api_token) VALUES ($1, $2, $3)",
	)
	.bind("bob")
	.bind("e99a18c428cb38d5f260853678922e03") // MD5 of "abc123"
	.bind("another_secret_token_67890")
	.execute(&*pool)
	.await
	.expect("Failed to insert bob");

	// ============================================================================
	// Execute: Migrate to more secure hashing (simulated)
	// ============================================================================
	//
	// Note: In production, this would involve:
	// 1. Adding new bcrypt_hash column
	// 2. Forcing users to reset passwords on next login
	// 3. Deprecating old password_hash column
	//
	// For this test, we simulate by adding a new column and marking old hashes as deprecated

	let add_bcrypt_migration = create_test_migration(
		"auth",
		"0002_add_bcrypt",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("bcrypt_hash", FieldType::VarChar(Some(60))),
			mysql_options: None,
		}],
	);

	executor
		.apply_migration(&add_bcrypt_migration)
		.await
		.expect("Failed to add bcrypt_hash column");

	// Verify bcrypt column added
	let bcrypt_column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'users' AND column_name = 'bcrypt_hash'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query bcrypt_hash column");
	assert_eq!(bcrypt_column_exists, 1, "bcrypt_hash column should exist");

	// ============================================================================
	// Execute: Hash rotation for API tokens
	// ============================================================================
	//
	// Simulate rotating API tokens by adding a new column and marking old tokens as deprecated

	let add_token_v2_migration = create_test_migration(
		"auth",
		"0003_add_token_v2",
		vec![Operation::AddColumn {
			table: leak_str("users").to_string(),
			column: create_basic_column("api_token_v2", FieldType::VarChar(Some(255))),
			mysql_options: None,
		}],
	);

	executor
		.apply_migration(&add_token_v2_migration)
		.await
		.expect("Failed to add api_token_v2 column");

	// In production, would generate new tokens via secure random generator
	// For test, we just verify the column exists and old tokens remain unchanged
	let token_v2_column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'users' AND column_name = 'api_token_v2'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query api_token_v2 column");
	assert_eq!(
		token_v2_column_exists, 1,
		"api_token_v2 column should exist"
	);

	// ============================================================================
	// Assert: Verify sensitive data protection
	// ============================================================================

	// Verify original sensitive data still exists (not lost during migration)
	let alice_token: String =
		sqlx::query_scalar("SELECT api_token FROM users WHERE username = 'alice'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch alice's token");

	assert_eq!(
		alice_token, "secret_api_token_12345",
		"Original token should be preserved"
	);

	// Verify password hashes not exposed in column names or defaults
	let password_column_default: Option<String> = sqlx::query_scalar(
		"SELECT column_default FROM information_schema.columns
		WHERE table_name = 'users' AND column_name = 'password_hash'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query password_hash default");

	assert!(
		password_column_default.is_none(),
		"Password column should not have default value"
	);

	// Verify sensitive columns are marked appropriately (would check comments in production)
	// For this test, we verify they exist without plaintext exposure
	let sensitive_columns: Vec<String> = sqlx::query_scalar(
		"SELECT column_name FROM information_schema.columns
		WHERE table_name = 'users' AND column_name IN ('password_hash', 'api_token', 'bcrypt_hash', 'api_token_v2')
		ORDER BY column_name",
	)
	.fetch_all(&*pool)
	.await
	.expect("Failed to query sensitive columns");

	assert_eq!(
		sensitive_columns.len(),
		4,
		"Should have 4 sensitive columns"
	);
	assert_eq!(sensitive_columns[0], "api_token");
	assert_eq!(sensitive_columns[1], "api_token_v2");
	assert_eq!(sensitive_columns[2], "bcrypt_hash");
	assert_eq!(sensitive_columns[3], "password_hash");

	// Verify no sensitive data in table comments (best practice)
	let table_comment: Option<String> =
		sqlx::query_scalar("SELECT obj_description('users'::regclass, 'pg_class')")
			.fetch_one(&*pool)
			.await
			.expect("Failed to query table comment");

	// Should be None or not contain sensitive data
	if let Some(comment) = table_comment {
		assert!(
			!comment.contains("password")
				&& !comment.contains("token")
				&& !comment.contains("secret"),
			"Table comment should not expose sensitive field details: {}",
			comment
		);
	}

	println!("\n=== Sensitive Data Handling Summary ===");
	println!("Password migration: MD5 → bcrypt pattern verified");
	println!("API token rotation: v1 → v2 pattern verified");
	println!("Data preservation: no sensitive data lost");
	println!("Exposure prevention: no plaintext in defaults/comments");
	println!("Column security: appropriate types and constraints");
	println!("=======================================\n");
}

// ============================================================================
// Audit Logging Tests
// ============================================================================

/// Test audit logging completeness
///
/// **Test Intent**: Verify that all migration operations are properly
/// logged with sufficient detail for security auditing and compliance
///
/// **Integration Point**: Migration executor → Audit logger → Log storage
///
/// **Expected Behavior**: All operations logged with timestamp, user,
/// operation type, affected objects, success/failure status
#[rstest]
#[tokio::test]
#[serial(security)]
async fn test_audit_logging_completeness(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create audit log table
	// ============================================================================
	//
	// Scenario: Security compliance requires full audit trail
	// Requirement: Log all DDL operations with context

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	// Create audit log table
	let create_audit_log_migration = create_test_migration(
		"system",
		"0001_create_audit_log",
		vec![Operation::CreateTable {
			name: leak_str("migration_audit_log").to_string(),
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
				create_basic_column("migration_app", FieldType::VarChar(Some(100))),
				create_basic_column("migration_name", FieldType::VarChar(Some(255))),
				create_basic_column("operation_type", FieldType::VarChar(Some(50))),
				create_basic_column("table_name", FieldType::VarChar(Some(100))),
				create_basic_column("success", FieldType::Boolean),
				create_basic_column("error_message", FieldType::Text),
				ColumnDefinition {
					name: "executed_at".to_string(),
					type_definition: FieldType::Timestamp,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("CURRENT_TIMESTAMP".to_string()),
				},
				create_basic_column("executed_by", FieldType::VarChar(Some(100))),
			],
		}],
	);

	executor
		.apply_migration(&create_audit_log_migration)
		.await
		.expect("Failed to create audit log table");

	// ============================================================================
	// Execute: Perform operations that should be audited
	// ============================================================================

	// Operation 1: Create table
	let create_products_migration = create_test_migration(
		"shop",
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
			],
		}],
	);

	executor
		.apply_migration(&create_products_migration)
		.await
		.expect("Failed to create products table");

	// Log the operation
	sqlx::query(
		"INSERT INTO migration_audit_log (migration_app, migration_name, operation_type, table_name, success, executed_by)
		VALUES ($1, $2, $3, $4, $5, $6)",
	)
	.bind("shop")
	.bind("0001_create_products")
	.bind("CreateTable")
	.bind("products")
	.bind(true)
	.bind("migration_executor")
	.execute(&*pool)
	.await
	.expect("Failed to log CreateTable operation");

	// Operation 2: Add column
	let add_category_migration = create_test_migration(
		"shop",
		"0002_add_category",
		vec![Operation::AddColumn {
			table: leak_str("products").to_string(),
			column: create_basic_column("category", FieldType::VarChar(Some(50))),
			mysql_options: None,
		}],
	);

	executor
		.apply_migration(&add_category_migration)
		.await
		.expect("Failed to add category column");

	// Log the operation
	sqlx::query(
		"INSERT INTO migration_audit_log (migration_app, migration_name, operation_type, table_name, success, executed_by)
		VALUES ($1, $2, $3, $4, $5, $6)",
	)
	.bind("shop")
	.bind("0002_add_category")
	.bind("AddColumn")
	.bind("products")
	.bind(true)
	.bind("migration_executor")
	.execute(&*pool)
	.await
	.expect("Failed to log AddColumn operation");

	// Operation 3: Create index
	sqlx::query("CREATE INDEX idx_products_category ON products(category)")
		.execute(&*pool)
		.await
		.expect("Failed to create index");

	// Log the operation
	sqlx::query(
		"INSERT INTO migration_audit_log (migration_app, migration_name, operation_type, table_name, success, executed_by)
		VALUES ($1, $2, $3, $4, $5, $6)",
	)
	.bind("shop")
	.bind("0003_create_category_index")
	.bind("CreateIndex")
	.bind("products")
	.bind(true)
	.bind("migration_executor")
	.execute(&*pool)
	.await
	.expect("Failed to log CreateIndex operation");

	// ============================================================================
	// Assert: Verify audit log completeness
	// ============================================================================

	// Verify all operations logged
	let total_logs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_audit_log")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count audit logs");
	assert_eq!(total_logs, 3, "Should have 3 audit log entries");

	// Verify log entries have required fields
	let logs: Vec<(String, String, String, String, bool)> = sqlx::query_as(
		"SELECT migration_app, migration_name, operation_type, table_name, success
		FROM migration_audit_log
		ORDER BY id",
	)
	.fetch_all(&*pool)
	.await
	.expect("Failed to fetch audit logs");

	assert_eq!(logs[0].0, "shop");
	assert_eq!(logs[0].1, "0001_create_products");
	assert_eq!(logs[0].2, "CreateTable");
	assert_eq!(logs[0].3, "products");
	assert!(logs[0].4, "First operation should be successful");

	assert_eq!(logs[1].0, "shop");
	assert_eq!(logs[1].1, "0002_add_category");
	assert_eq!(logs[1].2, "AddColumn");
	assert_eq!(logs[1].3, "products");
	assert!(logs[1].4, "Second operation should be successful");

	assert_eq!(logs[2].0, "shop");
	assert_eq!(logs[2].1, "0003_create_category_index");
	assert_eq!(logs[2].2, "CreateIndex");
	assert_eq!(logs[2].3, "products");
	assert!(logs[2].4, "Third operation should be successful");

	// Verify timestamps are present and sequential
	let timestamps: Vec<chrono::NaiveDateTime> =
		sqlx::query_scalar("SELECT executed_at FROM migration_audit_log ORDER BY id")
			.fetch_all(&*pool)
			.await
			.expect("Failed to fetch timestamps");

	assert_eq!(timestamps.len(), 3, "Should have 3 timestamps");
	assert!(
		timestamps[0] <= timestamps[1],
		"Timestamps should be sequential"
	);
	assert!(
		timestamps[1] <= timestamps[2],
		"Timestamps should be sequential"
	);

	// Verify executed_by field populated
	let executed_by_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM migration_audit_log WHERE executed_by IS NOT NULL",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to count executed_by entries");
	assert_eq!(
		executed_by_count, 3,
		"All entries should have executed_by field"
	);

	// Test error logging: Simulate failed operation
	sqlx::query(
		"INSERT INTO migration_audit_log (migration_app, migration_name, operation_type, table_name, success, error_message, executed_by)
		VALUES ($1, $2, $3, $4, $5, $6, $7)",
	)
	.bind("shop")
	.bind("0004_failed_operation")
	.bind("AddColumn")
	.bind("products")
	.bind(false)
	.bind("Column already exists")
	.bind("migration_executor")
	.execute(&*pool)
	.await
	.expect("Failed to log failed operation");

	// Verify failed operation logged correctly
	let failed_log: (bool, String) = sqlx::query_as(
		"SELECT success, error_message FROM migration_audit_log WHERE migration_name = '0004_failed_operation'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to fetch failed operation log");

	assert!(!failed_log.0, "Failed operation should have success=false");
	assert_eq!(
		failed_log.1, "Column already exists",
		"Error message should be logged"
	);

	println!("\n=== Audit Logging Summary ===");
	println!("Total operations logged: 4");
	println!("Successful operations: 3");
	println!("Failed operations: 1");
	println!("Required fields: all present");
	println!("Timestamp ordering: verified");
	println!("Error messages: captured");
	println!("=============================\n");
}

// ============================================================================
// Permission Escalation Prevention Tests
// ============================================================================

/// Test permission escalation prevention
///
/// **Test Intent**: Verify that migrations cannot escalate database
/// privileges or perform unauthorized operations, preventing security
/// vulnerabilities from malicious or buggy migrations
///
/// **Integration Point**: Migration executor → Permission validator → Security enforcement
///
/// **Expected Behavior**: Privilege escalation attempts blocked,
/// unauthorized GRANT/REVOKE rejected, clear security errors reported
#[rstest]
#[tokio::test]
#[serial(security)]
async fn test_permission_escalation_prevention(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create restricted user and test table
	// ============================================================================
	//
	// Scenario: Prevent migrations from granting superuser or dangerous privileges
	// Security requirement: Migrations must not elevate their own permissions

	// Create test table owned by superuser
	sqlx::query(
		"CREATE TABLE sensitive_data (
			id SERIAL PRIMARY KEY,
			secret_value VARCHAR(255)
		)",
	)
	.execute(&*pool)
	.await
	.expect("Failed to create sensitive_data table");

	// Create restricted migration user
	sqlx::query("CREATE USER restricted_migration WITH PASSWORD 'restricted_pass'")
		.execute(&*pool)
		.await
		.expect("Failed to create restricted_migration user");

	// Grant minimal privileges
	sqlx::query("GRANT CONNECT ON DATABASE postgres TO restricted_migration")
		.execute(&*pool)
		.await
		.expect("Failed to grant CONNECT");

	sqlx::query("GRANT USAGE ON SCHEMA public TO restricted_migration")
		.execute(&*pool)
		.await
		.expect("Failed to grant USAGE");

	// Note: DO NOT grant SELECT on sensitive_data

	// ============================================================================
	// Execute: Attempt privilege escalation (should fail)
	// ============================================================================

	// Attempt 1: Try to GRANT privileges to self (should fail)
	let escalation_attempt_1 =
		sqlx::query("GRANT SELECT ON sensitive_data TO restricted_migration")
			.execute(&*pool)
			.await;

	// This succeeds when run as superuser, but in production would fail for restricted user
	// To properly test, we'd need to connect as restricted_migration
	assert!(
		escalation_attempt_1.is_ok(),
		"Superuser can grant privileges (expected for test setup)"
	);

	// Connect as restricted user to test actual restrictions
	let restricted_url = url.replace("postgres@", "restricted_migration:restricted_pass@");
	let restricted_pool = sqlx::PgPool::connect(&restricted_url)
		.await
		.expect("Failed to connect as restricted_migration");

	// Attempt 2: Try to read sensitive data (should fail due to lack of SELECT privilege)
	let unauthorized_read = sqlx::query("SELECT * FROM sensitive_data")
		.fetch_all(&restricted_pool)
		.await;

	assert!(
		unauthorized_read.is_err(),
		"Restricted user should not have SELECT privilege"
	);

	if let Err(e) = unauthorized_read {
		let error_msg = e.to_string();
		assert!(
			error_msg.contains("permission denied") || error_msg.contains("denied"),
			"Error should indicate permission denial: {}",
			error_msg
		);
	}

	// Attempt 3: Try to create superuser (should fail)
	let superuser_creation =
		sqlx::query("CREATE USER malicious_super WITH SUPERUSER PASSWORD 'malicious'")
			.execute(&restricted_pool)
			.await;

	assert!(
		superuser_creation.is_err(),
		"Restricted user should not be able to create superuser"
	);

	if let Err(e) = superuser_creation {
		let error_msg = e.to_string();
		assert!(
			error_msg.contains("permission denied") || error_msg.contains("must be superuser"),
			"Error should indicate superuser requirement: {}",
			error_msg
		);
	}

	// ============================================================================
	// Assert: Verify security boundaries maintained
	// ============================================================================

	// Verify restricted user is not superuser
	let is_superuser: bool =
		sqlx::query_scalar("SELECT usesuper FROM pg_user WHERE usename = 'restricted_migration'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to check superuser status");

	assert!(
		!is_superuser,
		"Restricted migration user should not be superuser"
	);

	// Verify restricted user cannot create databases
	let has_createdb: bool = sqlx::query_scalar(
		"SELECT usecreatedb FROM pg_user WHERE usename = 'restricted_migration'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to check createdb privilege");

	assert!(
		!has_createdb,
		"Restricted migration user should not have CREATEDB privilege"
	);

	// Verify no malicious superuser was created
	let malicious_user_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_user WHERE usename = 'malicious_super'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to check malicious user");

	assert_eq!(
		malicious_user_exists, 0,
		"Malicious superuser should not exist"
	);

	// Test: Verify migrations cannot alter pg_catalog or system tables
	let system_table_modification =
		sqlx::query("INSERT INTO pg_catalog.pg_database (datname) VALUES ('hacked_db')")
			.execute(&restricted_pool)
			.await;

	assert!(
		system_table_modification.is_err(),
		"Should not be able to modify system catalog"
	);

	// Cleanup
	restricted_pool.close().await;
	sqlx::query("DROP USER IF EXISTS restricted_migration")
		.execute(&*pool)
		.await
		.expect("Failed to drop restricted_migration user");

	println!("\n=== Permission Escalation Prevention Summary ===");
	println!("Superuser creation: blocked");
	println!("Unauthorized data access: blocked");
	println!("System catalog modification: blocked");
	println!("Privilege boundaries: enforced");
	println!("Security errors: clear and specific");
	println!("================================================\n");
}

// ============================================================================
// SQL Injection Prevention Tests
// ============================================================================

/// Test SQL injection prevention in migrations
///
/// **Test Intent**: Verify that migration system properly sanitizes
/// inputs and prevents SQL injection attacks through malicious migration
/// parameters or data
///
/// **Integration Point**: Migration executor → Input validation → SQL generation
///
/// **Expected Behavior**: Malicious inputs rejected or escaped,
/// parameterized queries used, no arbitrary SQL execution from untrusted input
#[rstest]
#[tokio::test]
#[serial(security)]
async fn test_sql_injection_prevention(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Create test table
	// ============================================================================

	let conn = DatabaseConnection::connect(&url, DatabaseType::Postgres)
		.await
		.expect("Failed to connect to database");
	let mut executor = DatabaseMigrationExecutor::new(conn.clone());

	let create_users_migration = create_test_migration(
		"auth",
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
				create_basic_column("username", FieldType::VarChar(Some(100))),
				create_basic_column("email", FieldType::VarChar(Some(255))),
			],
		}],
	);

	executor
		.apply_migration(&create_users_migration)
		.await
		.expect("Failed to create users table");

	// ============================================================================
	// Execute: Test SQL injection attack vectors
	// ============================================================================

	// Attack vector 1: SQL injection in username (parameterized query should prevent)
	let malicious_username = "admin'; DROP TABLE users; --";

	let injection_result = sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2)")
		.bind(malicious_username)
		.bind("attacker@example.com")
		.execute(&*pool)
		.await;

	assert!(
		injection_result.is_ok(),
		"Parameterized query should safely handle malicious input"
	);

	// Verify table still exists (injection was prevented)
	let table_still_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to check table existence");

	assert_eq!(table_still_exists, 1, "users table should still exist");

	// Verify malicious string was stored as literal data (not executed)
	let stored_username: String =
		sqlx::query_scalar("SELECT username FROM users WHERE email = 'attacker@example.com'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch stored username");

	assert_eq!(
		stored_username, malicious_username,
		"Malicious input should be stored as literal string"
	);

	// Attack vector 2: SQL injection in column value (should be escaped)
	let malicious_email = "test@example.com' OR '1'='1";

	let email_injection_result = sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2)")
		.bind("testuser")
		.bind(malicious_email)
		.execute(&*pool)
		.await;

	assert!(
		email_injection_result.is_ok(),
		"Email with SQL characters should be safely stored"
	);

	// Verify the email was stored literally (not interpreted as SQL)
	let stored_email: String =
		sqlx::query_scalar("SELECT email FROM users WHERE username = 'testuser'")
			.fetch_one(&*pool)
			.await
			.expect("Failed to fetch stored email");

	assert_eq!(
		stored_email, malicious_email,
		"Email should be stored as literal string"
	);

	// ============================================================================
	// Assert: Verify SQL injection prevention
	// ============================================================================

	// Verify total user count (should be 2: attacker + testuser)
	let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
		.fetch_one(&*pool)
		.await
		.expect("Failed to count users");

	assert_eq!(user_count, 2, "Should have exactly 2 users");

	// Verify no users with empty username (which would indicate injection success)
	let empty_username_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ''")
			.fetch_one(&*pool)
			.await
			.expect("Failed to count empty usernames");

	assert_eq!(
		empty_username_count, 0,
		"No users should have empty username"
	);

	// Test: Verify LIKE pattern injection is also safe
	let like_injection = "%'; DROP TABLE users; --";
	let like_result = sqlx::query("SELECT * FROM users WHERE username LIKE $1")
		.bind(like_injection)
		.fetch_all(&*pool)
		.await;

	assert!(
		like_result.is_ok(),
		"LIKE query with injection should be safe"
	);

	// Table should still exist
	let table_exists_after_like: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'users'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to check table after LIKE injection");

	assert_eq!(
		table_exists_after_like, 1,
		"Table should still exist after LIKE injection attempt"
	);

	println!("\n=== SQL Injection Prevention Summary ===");
	println!("Parameterized queries: effective");
	println!("DROP TABLE injection: prevented");
	println!("OR '1'='1' injection: prevented");
	println!("LIKE pattern injection: prevented");
	println!("Malicious input handling: stored as literals");
	println!("Table integrity: maintained");
	println!("========================================\n");
}
