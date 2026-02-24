//! Function operations integration tests
//!
//! Tests for CREATE/ALTER/DROP FUNCTION operations including:
//! - Basic function creation with parameters
//! - Function with RETURNS type
//! - Function behaviors (IMMUTABLE/STABLE/VOLATILE)
//! - DROP FUNCTION with/without IF EXISTS
//! - Error cases
//! - State transitions

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::function::{FunctionBehavior, FunctionLanguage};

mod common;
use common::{PgContainer, pg_ident, postgres_ddl, unique_function_name};

// =============================================================================
// PostgreSQL CREATE FUNCTION Tests
// =============================================================================

/// HP-11: Test CREATE FUNCTION with parameters on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_function_with_parameters(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_add");

	// Build CREATE FUNCTION statement
	let mut stmt = Query::create_function();
	stmt.name(function_name.clone())
		.add_parameter("a", "integer")
		.add_parameter("b", "integer")
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT a + b");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_function(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create function");

	// Verify function exists
	let function_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1")
			.bind(&function_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(function_exists, 1);

	// Test function works
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}(2, 3)"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 5);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}(integer, integer)"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test CREATE FUNCTION without parameters
#[rstest]
#[tokio::test]
async fn test_postgres_create_function_no_parameters(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_one");

	// Build CREATE FUNCTION statement
	let mut stmt = Query::create_function();
	stmt.name(function_name.clone())
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT 1");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_function(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create function");

	// Verify function exists
	let function_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1")
			.bind(&function_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(function_exists, 1);

	// Test function works
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}()"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test CREATE FUNCTION with PL/pgSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_function_plpgsql(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_plpgsql");

	// Build CREATE FUNCTION statement with PL/pgSQL
	let mut stmt = Query::create_function();
	stmt.name(function_name.clone())
		.add_parameter("n", "integer")
		.returns("integer")
		.language(FunctionLanguage::PlPgSql)
		.body("BEGIN RETURN n * 2; END;");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_function(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create function");

	// Test function works
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}(5)"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 10);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}(integer)"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test CREATE OR REPLACE FUNCTION
#[rstest]
#[tokio::test]
async fn test_postgres_create_or_replace_function(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_replace");
	let builder = PostgresQueryBuilder::new();

	// Create initial function
	let mut stmt1 = Query::create_function();
	stmt1
		.name(function_name.clone())
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT 1");

	let (sql, _values) = builder.build_create_function(&stmt1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create function");

	// Verify first version
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}()"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 1);

	// Replace function with new version
	let mut stmt2 = Query::create_function();
	stmt2
		.name(function_name.clone())
		.or_replace()
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT 42");

	let (sql, _values) = builder.build_create_function(&stmt2);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to replace function");

	// Verify replaced version
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}()"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 42);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// DROP FUNCTION Tests
// =============================================================================

/// HP-12: Test DROP FUNCTION
#[rstest]
#[tokio::test]
async fn test_postgres_drop_function(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_drop");
	let builder = PostgresQueryBuilder::new();

	// Create function using raw SQL
	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS integer LANGUAGE SQL AS 'SELECT 1'"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Verify function exists
	let function_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1")
			.bind(&function_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(function_exists, 1);

	// Build DROP FUNCTION statement
	let mut stmt = Query::drop_function();
	stmt.name(function_name.clone());

	let (sql, _values) = builder.build_drop_function(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop function");

	// Verify function no longer exists
	let function_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1")
			.bind(&function_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(function_exists, 0);
}

/// Test DROP FUNCTION IF EXISTS on non-existent function
#[rstest]
#[tokio::test]
async fn test_postgres_drop_function_if_exists_nonexistent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_nonexistent");
	let builder = PostgresQueryBuilder::new();

	// Build DROP FUNCTION IF EXISTS statement
	let mut stmt = Query::drop_function();
	stmt.name(function_name.clone()).if_exists();

	let (sql, _values) = builder.build_drop_function(&stmt);

	// Should succeed (no-op)
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;
	assert!(result.is_ok());
}

/// Test DROP FUNCTION without IF EXISTS on non-existent function fails
#[rstest]
#[tokio::test]
async fn test_postgres_drop_function_nonexistent_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_nonexistent");
	let builder = PostgresQueryBuilder::new();

	// Build DROP FUNCTION statement (no IF EXISTS)
	let mut stmt = Query::drop_function();
	stmt.name(function_name.clone());

	let (sql, _values) = builder.build_drop_function(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail
	assert!(result.is_err());
}

// =============================================================================
// Combination Tests - CB-09: FUNCTION language × behavior
// =============================================================================

/// Test function language and behavior combinations
#[rstest]
#[case::sql_immutable(FunctionLanguage::Sql, FunctionBehavior::Immutable)]
#[case::sql_stable(FunctionLanguage::Sql, FunctionBehavior::Stable)]
#[case::sql_volatile(FunctionLanguage::Sql, FunctionBehavior::Volatile)]
#[case::plpgsql_immutable(FunctionLanguage::PlPgSql, FunctionBehavior::Immutable)]
#[case::plpgsql_stable(FunctionLanguage::PlPgSql, FunctionBehavior::Stable)]
#[case::plpgsql_volatile(FunctionLanguage::PlPgSql, FunctionBehavior::Volatile)]
#[tokio::test]
async fn test_postgres_function_language_behavior_combinations(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] language: FunctionLanguage,
	#[case] behavior: FunctionBehavior,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_combo");
	let builder = PostgresQueryBuilder::new();

	// Build body based on language
	let body = match language {
		FunctionLanguage::Sql => "SELECT 1",
		FunctionLanguage::PlPgSql => "BEGIN RETURN 1; END;",
		_ => "SELECT 1",
	};

	// Build CREATE FUNCTION statement
	let mut stmt = Query::create_function();
	stmt.name(function_name.clone())
		.returns("integer")
		.language(language.clone())
		.behavior(behavior)
		.body(body);

	let (sql, _values) = builder.build_create_function(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create function");

	// Verify function exists and works
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}()"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// State Transition Tests
// =============================================================================

/// ST-10: Test CREATE FUNCTION → ALTER FUNCTION (via OR REPLACE) → verify
#[rstest]
#[tokio::test]
async fn test_postgres_function_state_transition(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_state");
	let builder = PostgresQueryBuilder::new();

	// State 1: Create function v1
	let mut create_stmt = Query::create_function();
	create_stmt
		.name(function_name.clone())
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT 1");

	let (sql, _values) = builder.build_create_function(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create function");

	// Verify v1
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}()"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 1, "State 1: Function should return 1");

	// State 2: Replace with v2
	let mut replace_stmt = Query::create_function();
	replace_stmt
		.name(function_name.clone())
		.or_replace()
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT 100");

	let (sql, _values) = builder.build_create_function(&replace_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to replace function");

	// Verify v2
	let result: i32 = sqlx::query_scalar(&format!(r#"SELECT {}()"#, pg_ident(&function_name)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(result, 100, "State 2: Function should return 100");

	// State 3: Drop function
	let mut drop_stmt = Query::drop_function();
	drop_stmt.name(function_name.clone());

	let (sql, _values) = builder.build_drop_function(&drop_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop function");

	// Verify dropped
	let function_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1")
			.bind(&function_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(function_exists, 0, "State 3: Function should be dropped");
}

// =============================================================================
// Error Path Tests
// =============================================================================

/// EP-08: Test CREATE FUNCTION with invalid body fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_function_invalid_body_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_bad");
	let builder = PostgresQueryBuilder::new();

	// Build CREATE FUNCTION with invalid SQL body
	let mut stmt = Query::create_function();
	stmt.name(function_name.clone())
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT * FROM nonexistent_table_xyz");

	let (sql, _values) = builder.build_create_function(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail - invalid SQL in body
	assert!(result.is_err());
}

/// Test CREATE FUNCTION without OR REPLACE on existing function fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_duplicate_function_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let function_name = unique_function_name("fn_dup");
	let builder = PostgresQueryBuilder::new();

	// Create first function
	let mut stmt1 = Query::create_function();
	stmt1
		.name(function_name.clone())
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT 1");

	let (sql, _values) = builder.build_create_function(&stmt1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("First function creation should succeed");

	// Try to create another function with same name (no OR REPLACE)
	let mut stmt2 = Query::create_function();
	stmt2
		.name(function_name.clone())
		.returns("integer")
		.language(FunctionLanguage::Sql)
		.body("SELECT 2");

	let (sql, _values) = builder.build_create_function(&stmt2);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail - duplicate function
	assert!(result.is_err());

	// Cleanup
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}
