//! Type operations integration tests (PostgreSQL specific)
//!
//! Tests for CREATE/ALTER/DROP TYPE operations including:
//! - ENUM type creation
//! - COMPOSITE type creation
//! - ALTER TYPE (add value to enum, rename)
//! - DROP TYPE with/without IF EXISTS
//! - Error cases
//! - State transitions

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;

mod common;
use common::{PgContainer, pg_ident, postgres_ddl, unique_table_name, unique_type_name};

// =============================================================================
// PostgreSQL CREATE TYPE (ENUM) Tests
// =============================================================================

/// HP-16: Test CREATE TYPE ENUM on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_type_enum(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("mood");

	// Build CREATE TYPE ENUM statement
	let mut stmt = Query::create_type();
	stmt.name(type_name.clone()).as_enum(vec![
		"happy".to_string(),
		"sad".to_string(),
		"neutral".to_string(),
	]);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_type(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create type");

	// Verify type exists
	let type_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pg_type WHERE typname = $1")
		.bind(&type_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(type_exists, 1);

	// Verify enum values
	let enum_values: Vec<String> = sqlx::query_scalar(
		"SELECT enumlabel::text FROM pg_enum e
		 JOIN pg_type t ON e.enumtypid = t.oid
		 WHERE t.typname = $1
		 ORDER BY e.enumsortorder",
	)
	.bind(&type_name)
	.fetch_all(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(enum_values, vec!["happy", "sad", "neutral"]);

	// Cleanup
	sqlx::query(&format!(r#"DROP TYPE IF EXISTS {}"#, pg_ident(&type_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// EC-13: Test CREATE TYPE ENUM with single value
#[rstest]
#[tokio::test]
async fn test_postgres_create_type_enum_single_value(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("single_enum");

	// Build CREATE TYPE ENUM statement with single value
	let mut stmt = Query::create_type();
	stmt.name(type_name.clone())
		.as_enum(vec!["only".to_string()]);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_type(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create type");

	// Verify enum has single value
	let count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_enum e
		 JOIN pg_type t ON e.enumtypid = t.oid
		 WHERE t.typname = $1",
	)
	.bind(&type_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 1);

	// Cleanup
	sqlx::query(&format!(r#"DROP TYPE IF EXISTS {}"#, pg_ident(&type_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// PostgreSQL CREATE TYPE (COMPOSITE) Tests
// =============================================================================

/// HP-17: Test CREATE TYPE COMPOSITE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_type_composite(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("address");

	// Build CREATE TYPE COMPOSITE statement
	let mut stmt = Query::create_type();
	stmt.name(type_name.clone()).as_composite(vec![
		("street".to_string(), "text".to_string()),
		("city".to_string(), "text".to_string()),
		("zip".to_string(), "integer".to_string()),
	]);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_type(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create type");

	// Verify type exists
	let type_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_type WHERE typname = $1 AND typtype = 'c'")
			.bind(&type_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(type_exists, 1);

	// Verify composite attributes
	let attr_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_attribute a
		 JOIN pg_type t ON a.attrelid = t.typrelid
		 WHERE t.typname = $1 AND a.attnum > 0",
	)
	.bind(&type_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(attr_count, 3);

	// Cleanup
	sqlx::query(&format!(r#"DROP TYPE IF EXISTS {}"#, pg_ident(&type_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test using composite type in a table
#[rstest]
#[tokio::test]
async fn test_postgres_composite_type_in_table(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("point2d");
	let table_name = unique_table_name("locations");

	// Create composite type
	let mut stmt = Query::create_type();
	stmt.name(type_name.clone()).as_composite(vec![
		("x".to_string(), "integer".to_string()),
		("y".to_string(), "integer".to_string()),
	]);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_type(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create type");

	// Create table using the composite type
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
			id SERIAL PRIMARY KEY,
			name TEXT,
			position {} NOT NULL
		)"#,
		pg_ident(&table_name),
		pg_ident(&type_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Insert data
	sqlx::query(&format!(
		r#"INSERT INTO {} (name, position) VALUES ('Origin', ROW(0, 0))"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert data");

	// Query data
	let name: String = sqlx::query_scalar(&format!(
		r#"SELECT name FROM {} LIMIT 1"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(name, "Origin");

	// Cleanup (table first, then type)
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(r#"DROP TYPE IF EXISTS {}"#, pg_ident(&type_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// PostgreSQL DROP TYPE Tests
// =============================================================================

/// Test DROP TYPE
#[rstest]
#[tokio::test]
async fn test_postgres_drop_type(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("drop_me");
	let builder = PostgresQueryBuilder::new();

	// Create type using raw SQL
	sqlx::query(&format!(
		r#"CREATE TYPE {} AS ENUM ('a', 'b', 'c')"#,
		pg_ident(&type_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create type");

	// Verify type exists
	let type_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pg_type WHERE typname = $1")
		.bind(&type_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(type_exists, 1);

	// Build DROP TYPE statement
	let mut stmt = Query::drop_type();
	stmt.name(type_name.clone());

	let (sql, _values) = builder.build_drop_type(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop type");

	// Verify type no longer exists
	let type_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pg_type WHERE typname = $1")
		.bind(&type_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(type_exists, 0);
}

/// Test DROP TYPE IF EXISTS on non-existent type
#[rstest]
#[tokio::test]
async fn test_postgres_drop_type_if_exists_nonexistent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("nonexistent");
	let builder = PostgresQueryBuilder::new();

	// Build DROP TYPE IF EXISTS statement
	let mut stmt = Query::drop_type();
	stmt.name(type_name.clone()).if_exists();

	let (sql, _values) = builder.build_drop_type(&stmt);

	// Should succeed (no-op)
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;
	assert!(result.is_ok());
}

/// Test DROP TYPE without IF EXISTS on non-existent type fails
#[rstest]
#[tokio::test]
async fn test_postgres_drop_type_nonexistent_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("nonexistent");
	let builder = PostgresQueryBuilder::new();

	// Build DROP TYPE statement (no IF EXISTS)
	let mut stmt = Query::drop_type();
	stmt.name(type_name.clone());

	let (sql, _values) = builder.build_drop_type(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail
	assert!(result.is_err());
}

// =============================================================================
// PostgreSQL ALTER TYPE Tests
// =============================================================================

/// ST-07: Test CREATE TYPE ENUM → ALTER ADD VALUE → use enum
#[rstest]
#[tokio::test]
async fn test_postgres_alter_type_add_value(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("status");
	let table_name = unique_table_name("items");

	let builder = PostgresQueryBuilder::new();

	// Create ENUM type
	let mut create_stmt = Query::create_type();
	create_stmt
		.name(type_name.clone())
		.as_enum(vec!["pending".to_string(), "active".to_string()]);

	let (sql, _values) = builder.build_create_type(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create type");

	// Create table using the enum
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
			id SERIAL PRIMARY KEY,
			status {} NOT NULL
		)"#,
		pg_ident(&table_name),
		pg_ident(&type_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Verify we can't use 'archived' yet
	let result = sqlx::query(&format!(
		r#"INSERT INTO {} (status) VALUES ('archived')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await;
	assert!(result.is_err(), "Should not be able to use 'archived' yet");

	// Add new value to enum
	let mut alter_stmt = Query::alter_type();
	alter_stmt
		.name(type_name.clone())
		.add_value("archived", None);

	let (sql, _values) = builder.build_alter_type(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to alter type");

	// Now we can use 'archived'
	sqlx::query(&format!(
		r#"INSERT INTO {} (status) VALUES ('archived')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Should be able to use 'archived' now");

	// Verify data
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {} WHERE status = 'archived'"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(r#"DROP TYPE IF EXISTS {}"#, pg_ident(&type_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Error Path Tests
// =============================================================================

/// EP-11: Test CREATE TYPE with duplicate enum values fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_type_duplicate_enum_values_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("bad_enum");
	let builder = PostgresQueryBuilder::new();

	// Build CREATE TYPE ENUM with duplicate values
	let mut stmt = Query::create_type();
	stmt.name(type_name.clone())
		.as_enum(vec!["a".to_string(), "b".to_string(), "a".to_string()]);

	let (sql, _values) = builder.build_create_type(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail - duplicate enum value
	assert!(result.is_err());
}

/// Test CREATE TYPE with duplicate name fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_duplicate_type_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("dup_type");
	let builder = PostgresQueryBuilder::new();

	// Create first type
	let mut stmt1 = Query::create_type();
	stmt1.name(type_name.clone()).as_enum(vec!["a".to_string()]);

	let (sql, _values) = builder.build_create_type(&stmt1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("First type creation should succeed");

	// Try to create another type with same name
	let mut stmt2 = Query::create_type();
	stmt2.name(type_name.clone()).as_enum(vec!["b".to_string()]);

	let (sql, _values) = builder.build_create_type(&stmt2);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail - duplicate type name
	assert!(result.is_err());

	// Cleanup
	sqlx::query(&format!(r#"DROP TYPE IF EXISTS {}"#, pg_ident(&type_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// State Transition Tests
// =============================================================================

/// State transition: CREATE TYPE → use in table → DROP CASCADE
#[rstest]
#[tokio::test]
async fn test_postgres_type_state_transition_with_cascade(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let type_name = unique_type_name("state_type");
	let table_name = unique_table_name("state_table");

	let builder = PostgresQueryBuilder::new();

	// State 1: Create type
	let mut create_stmt = Query::create_type();
	create_stmt
		.name(type_name.clone())
		.as_enum(vec!["x".to_string(), "y".to_string()]);

	let (sql, _values) = builder.build_create_type(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create type");

	let type_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pg_type WHERE typname = $1")
		.bind(&type_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(type_exists, 1, "State 1: Type should exist");

	// State 2: Use type in table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL, val {})"#,
		pg_ident(&table_name),
		pg_ident(&type_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// State 3: DROP TYPE without CASCADE should fail
	let mut drop_stmt = Query::drop_type();
	drop_stmt.name(type_name.clone());

	let (sql, _values) = builder.build_drop_type(&drop_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;
	assert!(result.is_err(), "State 3: DROP without CASCADE should fail");

	// State 4: DROP TYPE CASCADE should succeed
	let mut drop_cascade_stmt = Query::drop_type();
	drop_cascade_stmt.name(type_name.clone()).cascade();

	let (sql, _values) = builder.build_drop_type(&drop_cascade_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("State 4: DROP CASCADE should succeed");

	// Verify type is gone
	let type_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pg_type WHERE typname = $1")
		.bind(&type_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(type_exists, 0, "State 4: Type should be dropped");

	// Cleanup (table should be gone due to CASCADE, but try anyway)
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.ok();
}
