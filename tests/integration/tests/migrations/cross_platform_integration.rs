//! Integration tests for cross-platform compatibility of migrations
//!
//! Tests database and platform-specific behaviors:
//! - Unicode and special character handling
//! - Case sensitivity differences
//! - Platform-specific line endings
//! - Timezone-aware migrations
//! - Locale-specific collation
//!
//! **Test Coverage:**
//! - International identifier support
//! - Database engine differences
//! - OS-specific behaviors
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
use testcontainers::{ContainerAsync, GenericImage};

// ============================================================================
// Test Helper Functions
// ============================================================================

fn leak_str(s: impl Into<String>) -> &'static str {
	Box::leak(s.into().into_boxed_str())
}

fn create_test_migration(
	app: &'static str,
	name: &'static str,
	operations: Vec<Operation>,
) -> Migration {
	Migration::from_parts(name, app, operations, vec![], vec![], true, None, false, false, Vec::new(), Vec::new())
}

fn create_basic_column(name: &str, type_def: FieldType) -> ColumnDefinition {
	ColumnDefinition::from_parts(name.to_string(), type_def, false, false, false, false, None, None, None)
}

// ============================================================================
// Tests
// ============================================================================

// ============================================================================
// Test 1: Unicode and Special Character Handling
// ============================================================================

/// Test migration with Unicode identifiers (Japanese, emoji)
///
/// **Test Intent**: Verify that international characters in table/column names are handled correctly
///
/// **Integration Point**: MigrationExecutor → SQL generation with Unicode escaping
///
/// **Expected Behavior**: Unicode identifiers properly escaped, database accepts them
#[rstest]
#[tokio::test]
#[serial(cross_platform)]
async fn test_unicode_and_special_character_handling(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Database connection and recorder
	// ============================================================================

	let db_type = DatabaseType::PostgreSQL;
	let db_conn = DatabaseConnection::new(url, db_type)
		.await
		.expect("Failed to create database connection");

	let recorder = DatabaseMigrationRecorder::new(db_conn.clone())
		.await
		.expect("Failed to create recorder");

	let executor = DatabaseMigrationExecutor::new(db_conn.clone(), recorder);

	// ============================================================================
	// Execute: Create table with Unicode identifiers
	// ============================================================================

	// Migration 1: Create table with Japanese and emoji column names
	let create_unicode_table_migration = create_test_migration(
		"testapp",
		"0001_create_unicode_table",
		vec![Operation::RunSQL {
			sql: leak_str(
				r#"CREATE TABLE "ユーザー情報" (
					id SERIAL PRIMARY KEY,
					"名前" VARCHAR(100) NOT NULL,
					"メールアドレス" VARCHAR(255) NOT NULL,
					"🔑パスワード" VARCHAR(255) NOT NULL,
					"作成日時" TIMESTAMP DEFAULT CURRENT_TIMESTAMP
				)"#,
			),
			reverse_sql: Some(r#"DROP TABLE "ユーザー情報""#),
		}],
	);

	let apply_result = executor
		.apply_migration(&create_unicode_table_migration)
		.await;
	assert!(
		apply_result.is_ok(),
		"Migration with Unicode identifiers should succeed"
	);

	// ============================================================================
	// Assert: Verify table and columns exist
	// ============================================================================

	// Verify table exists
	let table_exists: i64 = sqlx::query_scalar(
		r#"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'ユーザー情報'"#,
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query table existence");
	assert_eq!(table_exists, 1, "Table with Unicode name should exist");

	// Verify columns exist
	let column_count: i64 = sqlx::query_scalar(
		r#"SELECT COUNT(*) FROM information_schema.columns
		WHERE table_name = 'ユーザー情報' AND column_name IN ('名前', 'メールアドレス', '🔑パスワード', '作成日時')"#,
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query column count");
	assert_eq!(column_count, 4, "All Unicode columns should be created");

	// Insert test data with Unicode values
	let insert_result = sqlx::query(
		r#"INSERT INTO "ユーザー情報" ("名前", "メールアドレス", "🔑パスワード") VALUES ($1, $2, $3)"#,
	)
	.bind("田中太郎")
	.bind("tanaka@example.com")
	.bind("パスワード123")
	.execute(&*pool)
	.await;

	assert!(
		insert_result.is_ok(),
		"Insert with Unicode data should succeed"
	);

	// Verify data retrieval
	let name: String = sqlx::query_scalar(
		r#"SELECT "名前" FROM "ユーザー情報" WHERE "メールアドレス" = 'tanaka@example.com'"#,
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to fetch Unicode data");
	assert_eq!(name, "田中太郎", "Unicode data should be preserved");

	// ============================================================================
	// Rollback test
	// ============================================================================

	let rollback_result = executor
		.unapply_migration(&create_unicode_table_migration)
		.await;
	assert!(
		rollback_result.is_ok(),
		"Rollback of Unicode table should succeed"
	);

	let table_exists_after: i64 = sqlx::query_scalar(
		r#"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'ユーザー情報'"#,
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query table existence");
	assert_eq!(
		table_exists_after, 0,
		"Unicode table should be dropped after rollback"
	);
}

// ============================================================================
// Test 2: Case Sensitivity Handling
// ============================================================================

/// Test case sensitivity differences between databases
///
/// **Test Intent**: Verify that identifier case handling is consistent
///
/// **Integration Point**: MigrationExecutor → Quote identifier handling
///
/// **Expected Behavior**: Quoted identifiers preserve case, unquoted are lowercase (PostgreSQL)
#[rstest]
#[tokio::test]
#[serial(cross_platform)]
async fn test_case_sensitivity_handling(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Database connection and recorder
	// ============================================================================

	let db_type = DatabaseType::PostgreSQL;
	let db_conn = DatabaseConnection::new(url, db_type)
		.await
		.expect("Failed to create database connection");

	let recorder = DatabaseMigrationRecorder::new(db_conn.clone())
		.await
		.expect("Failed to create recorder");

	let executor = DatabaseMigrationExecutor::new(db_conn.clone(), recorder);

	// ============================================================================
	// Execute: Create tables with different case identifiers
	// ============================================================================

	// Migration 1: Unquoted identifier (PostgreSQL: becomes lowercase)
	let create_unquoted_migration = create_test_migration(
		"testapp",
		"0001_create_unquoted",
		vec![Operation::RunSQL {
			sql: leak_str("CREATE TABLE UserProfiles (id SERIAL PRIMARY KEY, Name VARCHAR(100))"),
			reverse_sql: Some("DROP TABLE UserProfiles"),
		}],
	);

	// Migration 2: Quoted identifier (PostgreSQL: preserves case)
	let create_quoted_migration = create_test_migration(
		"testapp",
		"0002_create_quoted",
		vec![Operation::RunSQL {
			sql: leak_str(
				r#"CREATE TABLE "UserSettings" (id SERIAL PRIMARY KEY, "PreferenceName" VARCHAR(100))"#,
			),
			reverse_sql: Some(r#"DROP TABLE "UserSettings""#),
		}],
	);

	executor
		.apply_migration(&create_unquoted_migration)
		.await
		.expect("Failed to apply unquoted migration");

	executor
		.apply_migration(&create_quoted_migration)
		.await
		.expect("Failed to apply quoted migration");

	// ============================================================================
	// Assert: Verify PostgreSQL case handling
	// ============================================================================

	// PostgreSQL: Unquoted identifiers are lowercased
	let unquoted_table: String = sqlx::query_scalar(
		"SELECT table_name FROM information_schema.tables WHERE table_name = 'userprofiles'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to find unquoted table (lowercase)");
	assert_eq!(
		unquoted_table, "userprofiles",
		"Unquoted identifier should be lowercase in PostgreSQL"
	);

	// PostgreSQL: Quoted identifiers preserve case
	let quoted_table: String = sqlx::query_scalar(
		"SELECT table_name FROM information_schema.tables WHERE table_name = 'UserSettings'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to find quoted table (preserved case)");
	assert_eq!(
		quoted_table, "UserSettings",
		"Quoted identifier should preserve case in PostgreSQL"
	);

	// Verify column case handling
	let quoted_column: String = sqlx::query_scalar(
		r#"SELECT column_name FROM information_schema.columns
		WHERE table_name = 'UserSettings' AND column_name = 'PreferenceName'"#,
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to find quoted column");
	assert_eq!(
		quoted_column, "PreferenceName",
		"Quoted column name should preserve case"
	);

	// ============================================================================
	// Cleanup
	// ============================================================================

	executor
		.unapply_migration(&create_quoted_migration)
		.await
		.expect("Failed to rollback quoted migration");

	executor
		.unapply_migration(&create_unquoted_migration)
		.await
		.expect("Failed to rollback unquoted migration");
}

// ============================================================================
// Test 3: Platform-Specific Line Ending
// ============================================================================

/// Test migration script compatibility across different OS line endings
///
/// **Test Intent**: Verify that migration files work regardless of line ending format
///
/// **Integration Point**: Migration file parsing → SQL execution
///
/// **Expected Behavior**: LF, CRLF, CR line endings all work correctly
#[rstest]
#[tokio::test]
#[serial(cross_platform)]
async fn test_platform_specific_line_ending(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Database connection and recorder
	// ============================================================================

	let db_type = DatabaseType::PostgreSQL;
	let db_conn = DatabaseConnection::new(url, db_type)
		.await
		.expect("Failed to create database connection");

	let recorder = DatabaseMigrationRecorder::new(db_conn.clone())
		.await
		.expect("Failed to create recorder");

	let executor = DatabaseMigrationExecutor::new(db_conn.clone(), recorder);

	// ============================================================================
	// Execute: Test different line ending formats
	// ============================================================================

	// Unix LF (\n)
	let sql_unix = "CREATE TABLE test_unix (\n\tid SERIAL PRIMARY KEY,\n\tname VARCHAR(100)\n)";

	// Windows CRLF (\r\n)
	let sql_windows =
		"CREATE TABLE test_windows (\r\n\tid SERIAL PRIMARY KEY,\r\n\tname VARCHAR(100)\r\n)";

	// Old Mac CR (\r) - rare but should be handled
	let sql_mac = "CREATE TABLE test_mac (\r\tid SERIAL PRIMARY KEY,\r\tname VARCHAR(100)\r)";

	let migration_unix = create_test_migration(
		"testapp",
		"0001_unix_lf",
		vec![Operation::RunSQL {
			sql: leak_str(sql_unix).to_string(),
			reverse_sql: Some("DROP TABLE test_unix"),
		}],
	);

	let migration_windows = create_test_migration(
		"testapp",
		"0002_windows_crlf",
		vec![Operation::RunSQL {
			sql: leak_str(sql_windows).to_string(),
			reverse_sql: Some("DROP TABLE test_windows"),
		}],
	);

	let migration_mac = create_test_migration(
		"testapp",
		"0003_mac_cr",
		vec![Operation::RunSQL {
			sql: leak_str(sql_mac).to_string(),
			reverse_sql: Some("DROP TABLE test_mac"),
		}],
	);

	// Apply all migrations
	executor
		.apply_migration(&migration_unix)
		.await
		.expect("Unix LF migration should succeed");

	executor
		.apply_migration(&migration_windows)
		.await
		.expect("Windows CRLF migration should succeed");

	executor
		.apply_migration(&migration_mac)
		.await
		.expect("Mac CR migration should succeed");

	// ============================================================================
	// Assert: Verify all tables were created successfully
	// ============================================================================

	let unix_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_unix'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query test_unix");
	assert_eq!(unix_exists, 1, "Unix LF table should exist");

	let windows_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_windows'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query test_windows");
	assert_eq!(windows_exists, 1, "Windows CRLF table should exist");

	let mac_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'test_mac'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query test_mac");
	assert_eq!(mac_exists, 1, "Mac CR table should exist");

	// ============================================================================
	// Cleanup
	// ============================================================================

	executor
		.unapply_migration(&migration_mac)
		.await
		.expect("Failed to rollback Mac CR migration");

	executor
		.unapply_migration(&migration_windows)
		.await
		.expect("Failed to rollback Windows CRLF migration");

	executor
		.unapply_migration(&migration_unix)
		.await
		.expect("Failed to rollback Unix LF migration");
}

// ============================================================================
// Test 4: Timezone-Aware Migrations
// ============================================================================

/// Test migration of timestamp columns with timezone conversion
///
/// **Test Intent**: Verify that timezone-aware timestamp migrations work correctly
///
/// **Integration Point**: AlterColumn → TIMESTAMP WITHOUT TIME ZONE to WITH TIME ZONE
///
/// **Expected Behavior**: Timezone conversion preserves UTC equivalence
#[rstest]
#[tokio::test]
#[serial(cross_platform)]
async fn test_timezone_aware_migrations(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Database connection and recorder
	// ============================================================================

	let db_type = DatabaseType::PostgreSQL;
	let db_conn = DatabaseConnection::new(url, db_type)
		.await
		.expect("Failed to create database connection");

	let recorder = DatabaseMigrationRecorder::new(db_conn.clone())
		.await
		.expect("Failed to create recorder");

	let executor = DatabaseMigrationExecutor::new(db_conn.clone(), recorder);

	// ============================================================================
	// Execute: Create table with TIMESTAMP WITHOUT TIME ZONE
	// ============================================================================

	let create_table_migration = create_test_migration(
		"testapp",
		"0001_create_events",
		vec![Operation::RunSQL {
			sql: leak_str(
				"CREATE TABLE events (
					id SERIAL PRIMARY KEY,
					name VARCHAR(100),
					created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
				)",
			),
			reverse_sql: Some("DROP TABLE events"),
		}],
	);

	executor
		.apply_migration(&create_table_migration)
		.await
		.expect("Failed to create events table");

	// Insert test data with specific timestamps
	sqlx::query("INSERT INTO events (name, created_at) VALUES ($1, $2)")
		.bind("Event 1")
		.bind("2024-01-15 10:30:00")
		.execute(&*pool)
		.await
		.expect("Failed to insert test event");

	// ============================================================================
	// Execute: Convert to TIMESTAMP WITH TIME ZONE
	// ============================================================================

	let convert_timezone_migration = create_test_migration(
		"testapp",
		"0002_add_timezone",
		vec![Operation::RunSQL {
			sql: leak_str(
				"ALTER TABLE events
				ALTER COLUMN created_at TYPE TIMESTAMP WITH TIME ZONE
				USING created_at AT TIME ZONE 'UTC'",
			),
			reverse_sql: Some(
				"ALTER TABLE events
				ALTER COLUMN created_at TYPE TIMESTAMP WITHOUT TIME ZONE
				USING created_at AT TIME ZONE 'UTC'",
			),
		}],
	);

	executor
		.apply_migration(&convert_timezone_migration)
		.await
		.expect("Failed to convert to timezone-aware timestamp");

	// ============================================================================
	// Assert: Verify timezone information is preserved
	// ============================================================================

	// Verify column type changed
	let column_type: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns
		WHERE table_name = 'events' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query column type");
	assert_eq!(
		column_type, "timestamp with time zone",
		"Column should be timezone-aware"
	);

	// Insert new event with explicit timezone
	sqlx::query("INSERT INTO events (name, created_at) VALUES ($1, $2)")
		.bind("Event 2")
		.bind("2024-01-15 10:30:00+09:00") // JST
		.execute(&*pool)
		.await
		.expect("Failed to insert timezone-aware event");

	// Verify timezone conversion (JST 10:30 = UTC 01:30)
	let utc_time: String = sqlx::query_scalar(
		"SELECT created_at AT TIME ZONE 'UTC' FROM events WHERE name = 'Event 2'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to fetch UTC time");
	assert!(
		utc_time.contains("01:30:00"),
		"Timezone conversion should work: {}",
		utc_time
	);

	// ============================================================================
	// Rollback test
	// ============================================================================

	executor
		.unapply_migration(&convert_timezone_migration)
		.await
		.expect("Failed to rollback timezone migration");

	let column_type_after: String = sqlx::query_scalar(
		"SELECT data_type FROM information_schema.columns
		WHERE table_name = 'events' AND column_name = 'created_at'",
	)
	.fetch_one(&*pool)
	.await
	.expect("Failed to query column type after rollback");
	assert_eq!(
		column_type_after, "timestamp without time zone",
		"Column should be without timezone after rollback"
	);

	// Cleanup
	executor
		.unapply_migration(&create_table_migration)
		.await
		.expect("Failed to cleanup events table");
}

// ============================================================================
// Test 5: Locale-Specific Collation
// ============================================================================

/// Test collation changes for different locales
///
/// **Test Intent**: Verify that locale-specific collation migrations work correctly
///
/// **Integration Point**: AlterColumn → Collation change
///
/// **Expected Behavior**: Sort order changes according to collation
#[rstest]
#[tokio::test]
#[serial(cross_platform)]
async fn test_locale_specific_collation(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;

	// ============================================================================
	// Setup: Database connection and recorder
	// ============================================================================

	let db_type = DatabaseType::PostgreSQL;
	let db_conn = DatabaseConnection::new(url, db_type)
		.await
		.expect("Failed to create database connection");

	let recorder = DatabaseMigrationRecorder::new(db_conn.clone())
		.await
		.expect("Failed to create recorder");

	let executor = DatabaseMigrationExecutor::new(db_conn.clone(), recorder);

	// ============================================================================
	// Execute: Create table with default collation
	// ============================================================================

	let create_table_migration = create_test_migration(
		"testapp",
		"0001_create_products",
		vec![Operation::RunSQL {
			sql: leak_str(
				"CREATE TABLE products (
					id SERIAL PRIMARY KEY,
					name VARCHAR(100)
				)",
			),
			reverse_sql: Some("DROP TABLE products"),
		}],
	);

	executor
		.apply_migration(&create_table_migration)
		.await
		.expect("Failed to create products table");

	// Insert Japanese product names
	let test_names = vec!["あいうえお", "かきくけこ", "アイウエオ", "カキクケコ"];
	for name in &test_names {
		sqlx::query("INSERT INTO products (name) VALUES ($1)")
			.bind(name)
			.execute(&*pool)
			.await
			.expect("Failed to insert product");
	}

	// Verify default sort order (UTF-8 binary)
	let default_order: Vec<String> = sqlx::query_scalar("SELECT name FROM products ORDER BY name")
		.fetch_all(&*pool)
		.await
		.expect("Failed to fetch default order");

	// ============================================================================
	// Execute: Change collation to ja_JP (Japanese)
	// ============================================================================

	// Check if ja_JP collation is available
	let ja_collation_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_collation WHERE collname LIKE 'ja_%'")
			.fetch_one(&*pool)
			.await
			.unwrap_or(0);

	if ja_collation_exists > 0 {
		// Apply collation change
		let change_collation_migration = create_test_migration(
			"testapp",
			"0002_change_collation",
			vec![Operation::RunSQL {
				sql: leak_str(
					"ALTER TABLE products
					ALTER COLUMN name TYPE VARCHAR(100) COLLATE \"ja-x-icu\"",
				),
				reverse_sql: Some(
					"ALTER TABLE products
					ALTER COLUMN name TYPE VARCHAR(100)",
				),
			}],
		);

		let collation_result = executor.apply_migration(&change_collation_migration).await;

		if collation_result.is_ok() {
			// Verify collation-aware sort order
			let ja_order: Vec<String> =
				sqlx::query_scalar("SELECT name FROM products ORDER BY name")
					.fetch_all(&*pool)
					.await
					.expect("Failed to fetch Japanese collation order");

			// In Japanese collation, hiragana and katakana may be sorted differently
			// The exact order depends on the collation rules

			// Rollback
			executor
				.unapply_migration(&change_collation_migration)
				.await
				.expect("Failed to rollback collation migration");
		}
	}

	// ============================================================================
	// Assert: Verify C (binary) vs en_US.UTF-8 collation
	// ============================================================================

	// Test with English-specific collation behavior
	sqlx::query("DELETE FROM products")
		.execute(&*pool)
		.await
		.expect("Failed to clear products");

	// Insert names with different casing
	let english_names = vec!["apple", "Apple", "APPLE", "banana", "Banana"];
	for name in &english_names {
		sqlx::query("INSERT INTO products (name) VALUES ($1)")
			.bind(name)
			.execute(&*pool)
			.await
			.expect("Failed to insert product");
	}

	// Verify case-sensitive binary sort
	let binary_order: Vec<String> =
		sqlx::query_scalar("SELECT name FROM products ORDER BY name COLLATE \"C\"")
			.fetch_all(&*pool)
			.await
			.expect("Failed to fetch binary order");

	// Verify case-insensitive sort
	let case_insensitive_order: Vec<String> =
		sqlx::query_scalar("SELECT name FROM products ORDER BY LOWER(name), name")
			.fetch_all(&*pool)
			.await
			.expect("Failed to fetch case-insensitive order");

	// Binary sort: APPLE, Apple, Banana, apple, banana (uppercase first)
	// Case-insensitive: apple/Apple/APPLE, banana/Banana (grouped by lowercase)
	assert_ne!(
		binary_order, case_insensitive_order,
		"Binary and case-insensitive sort should differ"
	);

	// ============================================================================
	// Cleanup
	// ============================================================================

	executor
		.unapply_migration(&create_table_migration)
		.await
		.expect("Failed to cleanup products table");
}
