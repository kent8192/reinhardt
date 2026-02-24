//! Property-based tests for DDL operations
//!
//! Uses proptest to verify invariants and properties of DDL statements:
//! - SQL syntax is always valid
//! - Identifiers are properly quoted
//! - CREATE → DROP = clean state
//! - IF NOT EXISTS / IF EXISTS idempotence

use std::sync::Arc;

use proptest::prelude::*;
use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::query::CreateTableStatement;
use reinhardt_query::types::{ColumnDef, ColumnType};

mod common;
use common::{PgContainer, postgres_ddl, unique_table_name};

// =============================================================================
// Proptest Strategy Definitions
// =============================================================================

/// Strategy for generating valid SQL identifiers
fn valid_identifier() -> impl Strategy<Value = String> {
	// Start with letter or underscore, then letters, digits, or underscores
	// Length 1-63 for PostgreSQL
	prop::string::string_regex("[a-z_][a-z0-9_]{0,62}")
		.unwrap()
		.prop_filter("non-empty identifier", |s| !s.is_empty())
}

/// Strategy for generating valid table names with test prefix
fn valid_table_name() -> impl Strategy<Value = String> {
	valid_identifier().prop_map(|s| format!("prop_test_{}", s))
}

/// Strategy for generating valid column names
fn valid_column_name() -> impl Strategy<Value = String> {
	valid_identifier().prop_filter("reserved words", |s| {
		let reserved = [
			"select", "from", "where", "table", "column", "index", "create", "drop", "alter",
			"insert", "update", "delete", "primary", "key", "null", "not", "and", "or",
		];
		!reserved.contains(&s.as_str())
	})
}

/// Strategy for generating VARCHAR lengths
fn varchar_length() -> impl Strategy<Value = u32> {
	prop::num::u32::ANY.prop_map(|n| (n % 65535) + 1) // 1-65535
}

/// Strategy for generating DECIMAL precision
fn decimal_precision() -> impl Strategy<Value = (u32, u32)> {
	(1u32..=38u32).prop_flat_map(|precision| (Just(precision), 0..=precision))
}

// =============================================================================
// Helper Functions for Tests
// =============================================================================

/// Create a simple CREATE TABLE statement for testing
fn create_table_stmt(name: &str) -> CreateTableStatement {
	let mut stmt = Query::create_table();
	stmt.table(name.to_string()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);
	stmt
}

/// Check if SQL contains the expected table name pattern
fn sql_contains_table_name(sql: &str, name: &str) -> bool {
	let pattern = format!(r#""{}""#, name);
	sql.contains(&pattern)
}

// =============================================================================
// Unit Tests - SQL Generation Properties
// =============================================================================

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]

	/// Property: Valid identifiers produce parseable SQL
	#[test]
	fn prop_valid_identifier_produces_sql(name in valid_table_name()) {
		let stmt = create_table_stmt(&name);
		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_create_table(&stmt);

		// SQL should contain the table name (quoted)
		prop_assert!(sql_contains_table_name(&sql, &name), "SQL should contain quoted table name");
	}

	/// Property: VARCHAR length is preserved in SQL
	#[test]
	fn prop_varchar_length_preserved(length in varchar_length()) {
		let mut stmt = Query::create_table();
		stmt.table("test_varchar")
			.col(
				ColumnDef::new("data")
					.column_type(ColumnType::String(Some(length)))
					.not_null(false),
			);

		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_create_table(&stmt);

		// SQL should contain the VARCHAR with correct length
		let expected = format!("VARCHAR({})", length);
		prop_assert!(sql.contains(&expected), "SQL should contain VARCHAR with length");
	}

	/// Property: DECIMAL precision is preserved in SQL
	#[test]
	fn prop_decimal_precision_preserved((precision, scale) in decimal_precision()) {
		let mut stmt = Query::create_table();
		stmt.table("test_decimal")
			.col(
				ColumnDef::new("amount")
					.column_type(ColumnType::Decimal(Some((precision, scale))))
					.not_null(false),
			);

		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_create_table(&stmt);

		// SQL should contain DECIMAL with correct precision and scale
		let expected = format!("NUMERIC({}, {})", precision, scale);
		prop_assert!(sql.contains(&expected), "SQL should contain NUMERIC with precision and scale");
	}

	/// Property: Column order is preserved
	#[test]
	fn prop_column_order_preserved(
		col1 in valid_column_name(),
		col2 in valid_column_name(),
		col3 in valid_column_name(),
	) {
		// Skip if columns have same name
		prop_assume!(col1 != col2 && col2 != col3 && col1 != col3);

		let mut stmt = Query::create_table();
		stmt.table("test_order")
			.col(ColumnDef::new(col1.clone()).column_type(ColumnType::Integer))
			.col(ColumnDef::new(col2.clone()).column_type(ColumnType::Integer))
			.col(ColumnDef::new(col3.clone()).column_type(ColumnType::Integer));

		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_create_table(&stmt);

		// Verify column order
		let pattern1 = format!(r#""{}""#, col1);
		let pattern2 = format!(r#""{}""#, col2);
		let pattern3 = format!(r#""{}""#, col3);

		let pos1 = sql.find(&pattern1).unwrap_or(usize::MAX);
		let pos2 = sql.find(&pattern2).unwrap_or(usize::MAX);
		let pos3 = sql.find(&pattern3).unwrap_or(usize::MAX);

		prop_assert!(pos1 < pos2, "col1 should come before col2");
		prop_assert!(pos2 < pos3, "col2 should come before col3");
	}

	/// Property: Identifiers are properly quoted
	#[test]
	fn prop_identifiers_quoted(name in valid_table_name()) {
		let stmt = create_table_stmt(&name);
		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_create_table(&stmt);

		// Table name should be double-quoted in PostgreSQL
		prop_assert!(sql_contains_table_name(&sql, &name), "Table name should be double-quoted");
	}

	/// Property: IF NOT EXISTS generates correct SQL
	#[test]
	fn prop_if_not_exists_syntax(name in valid_table_name()) {
		let mut stmt = Query::create_table();
		stmt.table(name.clone())
			.if_not_exists()
			.col(ColumnDef::new("id").column_type(ColumnType::Integer));

		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_create_table(&stmt);

		prop_assert!(sql.contains("IF NOT EXISTS"), "SQL should contain IF NOT EXISTS");
		prop_assert!(sql_contains_table_name(&sql, &name), "SQL should contain table name");
	}

	/// Property: DROP IF EXISTS generates correct SQL
	#[test]
	fn prop_drop_if_exists_syntax(name in valid_table_name()) {
		let mut stmt = Query::drop_table();
		stmt.table(name.clone()).if_exists();

		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_drop_table(&stmt);

		prop_assert!(sql.contains("DROP TABLE IF EXISTS"), "SQL should contain DROP TABLE IF EXISTS");
		prop_assert!(sql_contains_table_name(&sql, &name), "SQL should contain table name");
	}

	/// Property: Multiple column definitions don't corrupt each other
	#[test]
	fn prop_multiple_columns_independent(
		name1 in valid_column_name(),
		name2 in valid_column_name(),
	) {
		prop_assume!(name1 != name2);

		let mut stmt = Query::create_table();
		stmt.table("test_multi_col")
			.col(
				ColumnDef::new(name1.clone())
					.column_type(ColumnType::Integer)
					.not_null(true),
			)
			.col(
				ColumnDef::new(name2.clone())
					.column_type(ColumnType::String(Some(100)))
					.not_null(false),
			);

		let builder = PostgresQueryBuilder::new();
		let (sql, _values) = builder.build_create_table(&stmt);

		// Both columns should appear in SQL
		let pattern1 = format!(r#""{}""#, name1);
		let pattern2 = format!(r#""{}""#, name2);

		prop_assert!(sql.contains(&pattern1), "SQL should contain first column");
		prop_assert!(sql.contains(&pattern2), "SQL should contain second column");
		// Column types should match
		prop_assert!(sql.contains("INTEGER"), "SQL should contain INTEGER type");
		prop_assert!(sql.contains("VARCHAR(100)"), "SQL should contain VARCHAR type");
	}
}

// =============================================================================
// Integration Tests - Database Execution Properties
// =============================================================================

/// Property: CREATE → DROP leaves no residual objects
#[rstest]
#[tokio::test]
async fn test_create_drop_clean_state(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	// Run multiple cycles
	for i in 0..5 {
		let table_name = unique_table_name(&format!("prop_cycle_{}", i));

		// Create table
		let mut create_stmt = Query::create_table();
		create_stmt.table(table_name.clone()).col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		);

		let (sql, _values) = builder.build_create_table(&create_stmt);
		sqlx::query(&sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to create table");

		// Verify exists
		let exists: bool = sqlx::query_scalar(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
		)
		.bind(&table_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
		assert!(exists, "Table should exist after CREATE");

		// Drop table
		let mut drop_stmt = Query::drop_table();
		drop_stmt.table(table_name.clone());

		let (sql, _values) = builder.build_drop_table(&drop_stmt);
		sqlx::query(&sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to drop table");

		// Verify does not exist
		let exists: bool = sqlx::query_scalar(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
		)
		.bind(&table_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
		assert!(!exists, "Table should not exist after DROP");
	}
}

/// Property: IF NOT EXISTS is idempotent
#[rstest]
#[tokio::test]
async fn test_if_not_exists_idempotent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("prop_idempotent");
	let builder = PostgresQueryBuilder::new();

	// Create multiple times with IF NOT EXISTS
	for i in 0..5 {
		let mut stmt = Query::create_table();
		stmt.table(table_name.clone()).if_not_exists().col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		);

		let (sql, _values) = builder.build_create_table(&stmt);
		let result = sqlx::query(&sql).execute(pool.as_ref()).await;

		assert!(
			result.is_ok(),
			"CREATE IF NOT EXISTS should succeed on attempt {}",
			i + 1
		);
	}

	// Verify table exists exactly once
	let count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(count, 1, "Table should exist exactly once");

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Property: IF EXISTS is idempotent for non-existent objects
#[rstest]
#[tokio::test]
async fn test_if_exists_idempotent_nonexistent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	// Drop non-existent table multiple times with IF EXISTS
	for i in 0..5 {
		let table_name = unique_table_name(&format!("nonexistent_{}", i));

		let mut stmt = Query::drop_table();
		stmt.table(table_name).if_exists();

		let (sql, _values) = builder.build_drop_table(&stmt);
		let result = sqlx::query(&sql).execute(pool.as_ref()).await;

		assert!(
			result.is_ok(),
			"DROP IF EXISTS should succeed for non-existent table"
		);
	}
}

/// Property: Sequence MIN < MAX < START constraints
#[rstest]
#[tokio::test]
async fn test_sequence_boundary_constraints(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	// Test valid sequence configurations
	let valid_configs = [
		(1i64, 100i64, 1i64),
		(1, 1000000, 500000),
		(-100, 100, 0),
		(0, i64::MAX / 2, 1000),
	];

	for (i, (min, max, start)) in valid_configs.iter().enumerate() {
		let seq_name = unique_table_name(&format!("prop_seq_{}", i));

		let mut stmt = Query::create_sequence();
		stmt.name(seq_name.clone())
			.min_value(Some(*min))
			.max_value(Some(*max))
			.start(*start);

		let (sql, _values) = builder.build_create_sequence(&stmt);
		let result = sqlx::query(&sql).execute(pool.as_ref()).await;
		assert!(
			result.is_ok(),
			"Valid sequence config ({}, {}, {}) should succeed",
			min,
			max,
			start
		);

		// Cleanup
		sqlx::query(&format!(r#"DROP SEQUENCE IF EXISTS "{}""#, seq_name))
			.execute(pool.as_ref())
			.await
			.unwrap();
	}
}

/// Property: Column NOT NULL constraint is enforced
#[rstest]
#[tokio::test]
async fn test_not_null_constraint_enforced(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("prop_not_null");
	let builder = PostgresQueryBuilder::new();

	// Create table with NOT NULL column
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("required_field")
				.column_type(ColumnType::String(Some(100)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Attempt to insert NULL should fail
	let result = sqlx::query(&format!(
		r#"INSERT INTO "{}" (id, required_field) VALUES (1, NULL)"#,
		table_name
	))
	.execute(pool.as_ref())
	.await;

	assert!(
		result.is_err(),
		"INSERT with NULL should fail for NOT NULL column"
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Property: Primary key uniqueness is enforced
#[rstest]
#[tokio::test]
async fn test_primary_key_uniqueness_enforced(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("prop_pk");
	let builder = PostgresQueryBuilder::new();

	// Create table with primary key
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Insert first row
	sqlx::query(&format!(r#"INSERT INTO "{}" (id) VALUES (1)"#, table_name))
		.execute(pool.as_ref())
		.await
		.expect("First insert should succeed");

	// Insert duplicate should fail
	let result = sqlx::query(&format!(r#"INSERT INTO "{}" (id) VALUES (1)"#, table_name))
		.execute(pool.as_ref())
		.await;

	assert!(result.is_err(), "Duplicate primary key insert should fail");

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}
