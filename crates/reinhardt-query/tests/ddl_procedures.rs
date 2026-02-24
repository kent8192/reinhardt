//! Procedure operations integration tests
//!
//! Tests for CREATE/ALTER/DROP PROCEDURE operations including:
//! - Basic procedure creation
//! - Procedure with IN/OUT parameters
//! - CREATE OR REPLACE PROCEDURE
//! - DROP PROCEDURE with/without IF EXISTS
//! - Error cases
//! - State transitions
//!
//! Note: Unlike functions, procedures do NOT have a return type.

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::function::FunctionLanguage;

mod common;
use common::{PgContainer, pg_ident, postgres_ddl, unique_procedure_name, unique_table_name};

// =============================================================================
// PostgreSQL CREATE PROCEDURE Tests
// =============================================================================

/// HP-13: Test CREATE PROCEDURE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_procedure_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_basic");
	let table_name = unique_table_name("proc_test_log");

	// Create a test table for the procedure to insert into
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY, message TEXT)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create test table");

	// Build CREATE PROCEDURE statement
	let mut stmt = Query::create_procedure();
	stmt.name(procedure_name.clone())
		.add_parameter("msg", "text")
		.language(FunctionLanguage::PlPgSql)
		.body(format!(
			"BEGIN INSERT INTO {} (message) VALUES (msg); END;",
			pg_ident(&table_name)
		));

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_procedure(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create procedure");

	// Verify procedure exists
	let procedure_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1 AND prokind = 'p'")
			.bind(&procedure_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(procedure_exists, 1);

	// Test procedure works
	sqlx::query(&format!(r#"CALL {}('Hello')"#, pg_ident(&procedure_name)))
		.execute(pool.as_ref())
		.await
		.expect("Failed to call procedure");

	// Verify data was inserted
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP PROCEDURE IF EXISTS {}(text)"#,
		pg_ident(&procedure_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test CREATE PROCEDURE without parameters
#[rstest]
#[tokio::test]
async fn test_postgres_create_procedure_no_parameters(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_no_params");
	let table_name = unique_table_name("proc_test_count");

	// Create a test table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (counter INTEGER DEFAULT 0)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create test table");

	// Insert initial row
	sqlx::query(&format!(
		r#"INSERT INTO {} (counter) VALUES (0)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Build CREATE PROCEDURE statement
	let mut stmt = Query::create_procedure();
	stmt.name(procedure_name.clone())
		.language(FunctionLanguage::PlPgSql)
		.body(format!(
			"BEGIN UPDATE {} SET counter = counter + 1; END;",
			pg_ident(&table_name)
		));

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_procedure(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create procedure");

	// Call procedure multiple times
	for _ in 0..3 {
		sqlx::query(&format!(r#"CALL {}()"#, pg_ident(&procedure_name)))
			.execute(pool.as_ref())
			.await
			.expect("Failed to call procedure");
	}

	// Verify counter was incremented
	let counter: i32 = sqlx::query_scalar(&format!(
		r#"SELECT counter FROM {} LIMIT 1"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(counter, 3);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP PROCEDURE IF EXISTS {}()"#,
		pg_ident(&procedure_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test CREATE OR REPLACE PROCEDURE
#[rstest]
#[tokio::test]
async fn test_postgres_create_or_replace_procedure(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_replace");
	let table_name = unique_table_name("proc_replace_log");

	// Create a test table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (value INTEGER)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create test table");

	let builder = PostgresQueryBuilder::new();

	// Create initial procedure (inserts 1)
	let mut stmt1 = Query::create_procedure();
	stmt1
		.name(procedure_name.clone())
		.language(FunctionLanguage::PlPgSql)
		.body(format!(
			"BEGIN INSERT INTO {} (value) VALUES (1); END;",
			pg_ident(&table_name)
		));

	let (sql, _values) = builder.build_create_procedure(&stmt1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create procedure");

	// Call first version
	sqlx::query(&format!(r#"CALL {}()"#, pg_ident(&procedure_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();

	// Replace procedure (inserts 100)
	let mut stmt2 = Query::create_procedure();
	stmt2
		.name(procedure_name.clone())
		.or_replace()
		.language(FunctionLanguage::PlPgSql)
		.body(format!(
			"BEGIN INSERT INTO {} (value) VALUES (100); END;",
			pg_ident(&table_name)
		));

	let (sql, _values) = builder.build_create_procedure(&stmt2);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to replace procedure");

	// Call replaced version
	sqlx::query(&format!(r#"CALL {}()"#, pg_ident(&procedure_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();

	// Verify both values exist
	let sum: i64 = sqlx::query_scalar(&format!(
		r#"SELECT SUM(value) FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(sum, 101); // 1 + 100

	// Cleanup
	sqlx::query(&format!(
		r#"DROP PROCEDURE IF EXISTS {}()"#,
		pg_ident(&procedure_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// DROP PROCEDURE Tests
// =============================================================================

/// Test DROP PROCEDURE
#[rstest]
#[tokio::test]
async fn test_postgres_drop_procedure(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_drop");
	let builder = PostgresQueryBuilder::new();

	// Create procedure using raw SQL
	sqlx::query(&format!(
		r#"CREATE PROCEDURE {}() LANGUAGE SQL AS 'SELECT 1'"#,
		pg_ident(&procedure_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create procedure");

	// Verify procedure exists
	let procedure_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1 AND prokind = 'p'")
			.bind(&procedure_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(procedure_exists, 1);

	// Build DROP PROCEDURE statement
	let mut stmt = Query::drop_procedure();
	stmt.name(procedure_name.clone());

	let (sql, _values) = builder.build_drop_procedure(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop procedure");

	// Verify procedure no longer exists
	let procedure_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1 AND prokind = 'p'")
			.bind(&procedure_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(procedure_exists, 0);
}

/// Test DROP PROCEDURE IF EXISTS on non-existent procedure
#[rstest]
#[tokio::test]
async fn test_postgres_drop_procedure_if_exists_nonexistent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_nonexistent");
	let builder = PostgresQueryBuilder::new();

	// Build DROP PROCEDURE IF EXISTS statement
	let mut stmt = Query::drop_procedure();
	stmt.name(procedure_name.clone()).if_exists();

	let (sql, _values) = builder.build_drop_procedure(&stmt);

	// Should succeed (no-op)
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;
	assert!(result.is_ok());
}

/// Test DROP PROCEDURE without IF EXISTS on non-existent procedure fails
#[rstest]
#[tokio::test]
async fn test_postgres_drop_procedure_nonexistent_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_nonexistent");
	let builder = PostgresQueryBuilder::new();

	// Build DROP PROCEDURE statement (no IF EXISTS)
	let mut stmt = Query::drop_procedure();
	stmt.name(procedure_name.clone());

	let (sql, _values) = builder.build_drop_procedure(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail
	assert!(result.is_err());
}

// =============================================================================
// State Transition Tests
// =============================================================================

/// State transition: CREATE PROCEDURE → CALL → DROP
#[rstest]
#[tokio::test]
async fn test_postgres_procedure_state_transition(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_state");
	let table_name = unique_table_name("proc_state_log");

	let builder = PostgresQueryBuilder::new();

	// Create a test table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (step INTEGER)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create test table");

	// State 1: Create procedure
	let mut create_stmt = Query::create_procedure();
	create_stmt
		.name(procedure_name.clone())
		.add_parameter("step_num", "integer")
		.language(FunctionLanguage::PlPgSql)
		.body(format!(
			"BEGIN INSERT INTO {} (step) VALUES (step_num); END;",
			pg_ident(&table_name)
		));

	let (sql, _values) = builder.build_create_procedure(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create procedure");

	// Verify procedure exists
	let procedure_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1 AND prokind = 'p'")
			.bind(&procedure_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(procedure_exists, 1, "State 1: Procedure should exist");

	// State 2: Call procedure multiple times
	for i in 1..=3 {
		sqlx::query(&format!(r#"CALL {}({})"#, pg_ident(&procedure_name), i))
			.execute(pool.as_ref())
			.await
			.expect("Failed to call procedure");
	}

	// Verify data
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 3, "State 2: Should have 3 rows");

	// State 3: Drop procedure
	let mut drop_stmt = Query::drop_procedure();
	drop_stmt.name(procedure_name.clone());

	let (sql, _values) = builder.build_drop_procedure(&drop_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop procedure");

	let procedure_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_proc WHERE proname = $1 AND prokind = 'p'")
			.bind(&procedure_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(procedure_exists, 0, "State 3: Procedure should be dropped");

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// Error Path Tests
// =============================================================================

/// Test CREATE PROCEDURE without OR REPLACE on existing procedure fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_duplicate_procedure_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let procedure_name = unique_procedure_name("proc_dup");
	let builder = PostgresQueryBuilder::new();

	// Create first procedure
	let mut stmt1 = Query::create_procedure();
	stmt1
		.name(procedure_name.clone())
		.language(FunctionLanguage::Sql)
		.body("SELECT 1");

	let (sql, _values) = builder.build_create_procedure(&stmt1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("First procedure creation should succeed");

	// Try to create another procedure with same name (no OR REPLACE)
	let mut stmt2 = Query::create_procedure();
	stmt2
		.name(procedure_name.clone())
		.language(FunctionLanguage::Sql)
		.body("SELECT 2");

	let (sql, _values) = builder.build_create_procedure(&stmt2);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail - duplicate procedure
	assert!(result.is_err());

	// Cleanup
	sqlx::query(&format!(
		r#"DROP PROCEDURE IF EXISTS {}()"#,
		pg_ident(&procedure_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}
