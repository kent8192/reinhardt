//! Index operations integration tests
//!
//! Tests for CREATE/DROP/ALTER INDEX and REINDEX operations including:
//! - Basic CREATE/DROP INDEX
//! - UNIQUE INDEX
//! - Multi-column indexes
//! - Partial indexes (WHERE clause)
//! - INCLUDE columns (PostgreSQL)
//! - Error cases (duplicate index name)
//! - State transitions

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{ColumnDef, ColumnType};

mod common;
use common::{
	MySqlContainer, PgContainer, mysql_ddl, pg_ident, postgres_ddl, unique_index_name,
	unique_table_name,
};

/// Test basic CREATE INDEX on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_index_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");
	let index_name = unique_index_name("idx_email");

	// Create table first
	let create_table_sql = format!(
		r#"CREATE TABLE "{}" (id SERIAL PRIMARY KEY, email VARCHAR(255))"#,
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Build CREATE INDEX statement
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone())
		.table(table_name.clone())
		.col("email");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt);

	// Execute CREATE INDEX
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	// Verify index exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE indexname = $1")
			.bind(&index_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_table_sql = format!(r#"DROP TABLE IF EXISTS "{}""#, table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CREATE UNIQUE INDEX on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_unique_index(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");
	let index_name = unique_index_name("idx_email_unique");

	// Create table first
	let create_table_sql = format!(
		r#"CREATE TABLE "{}" (id SERIAL PRIMARY KEY, email VARCHAR(255))"#,
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Build CREATE UNIQUE INDEX statement
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone())
		.table(table_name.clone())
		.col("email")
		.unique();

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt);

	// Execute CREATE UNIQUE INDEX
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create unique index");

	// Verify index is unique
	let result: Option<bool> = sqlx::query_scalar(
		"SELECT indisunique FROM pg_indexes i
		JOIN pg_class c ON i.indexname = c.relname
		JOIN pg_index idx ON c.oid = idx.indexrelid
		WHERE i.indexname = $1",
	)
	.bind(&index_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();

	assert_eq!(result, Some(true));

	// Cleanup
	let drop_table_sql = format!(r#"DROP TABLE IF EXISTS "{}""#, table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test DROP INDEX on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_index(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");
	let index_name = unique_index_name("idx_email");

	// Create table and index
	let create_table_sql = format!(
		r#"CREATE TABLE "{}" (id SERIAL PRIMARY KEY, email VARCHAR(255))"#,
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	let create_index_sql = format!(
		r#"CREATE INDEX "{}" ON "{}" (email)"#,
		index_name, table_name
	);
	sqlx::query(&create_index_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	// Verify index exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE indexname = $1")
			.bind(&index_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(result, Some(1));

	// Build DROP INDEX statement
	let mut stmt = Query::drop_index();
	stmt.name(index_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_index(&stmt);

	// Execute DROP INDEX
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop index");

	// Verify index no longer exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE indexname = $1")
			.bind(&index_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(result, Some(0));

	// Cleanup
	let drop_table_sql = format!(r#"DROP TABLE IF EXISTS "{}""#, table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test REINDEX on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_reindex(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");
	let index_name = unique_index_name("idx_email");

	// Create table and index
	let create_table_sql = format!(
		r#"CREATE TABLE "{}" (id SERIAL PRIMARY KEY, email VARCHAR(255))"#,
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	let create_index_sql = format!(
		r#"CREATE INDEX "{}" ON "{}" (email)"#,
		index_name, table_name
	);
	sqlx::query(&create_index_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	// Build REINDEX statement
	let mut stmt = Query::reindex();
	stmt.index(index_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_reindex(&stmt);

	// Execute REINDEX
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to reindex");

	// Verify index still exists after reindex
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE indexname = $1")
			.bind(&index_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_table_sql = format!(r#"DROP TABLE IF EXISTS "{}""#, table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test basic CREATE INDEX on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_create_index_basic(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_users");
	let index_name = unique_index_name("idx_email");

	// Create table first
	let create_table_sql = format!(
		"CREATE TABLE `{}` (id INT AUTO_INCREMENT PRIMARY KEY, email VARCHAR(255))",
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Build CREATE INDEX statement
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone())
		.table(table_name.clone())
		.col("email");

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt);

	// Execute CREATE INDEX
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	// Verify index exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.statistics WHERE table_name = ? AND index_name = ?",
	)
	.bind(&table_name)
	.bind(&index_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_table_sql = format!("DROP TABLE IF EXISTS `{}`", table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test DROP INDEX on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_drop_index(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_users");
	let index_name = unique_index_name("idx_email");

	// Create table and index
	let create_table_sql = format!(
		"CREATE TABLE `{}` (id INT AUTO_INCREMENT PRIMARY KEY, email VARCHAR(255))",
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	let create_index_sql = format!("CREATE INDEX `{}` ON `{}` (email)", index_name, table_name);
	sqlx::query(&create_index_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	// Verify index exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.statistics WHERE table_name = ? AND index_name = ?",
	)
	.bind(&table_name)
	.bind(&index_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(1));

	// Build DROP INDEX statement (MySQL requires table name)
	let mut stmt = Query::drop_index();
	stmt.name(index_name.clone());
	stmt.table(table_name.clone());

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_drop_index(&stmt);

	// Execute DROP INDEX
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop index");

	// Verify index no longer exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.statistics WHERE table_name = ? AND index_name = ?",
	)
	.bind(&table_name)
	.bind(&index_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(0));

	// Cleanup
	let drop_table_sql = format!("DROP TABLE IF EXISTS `{}`", table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Multi-Column Index Tests
// =============================================================================

/// Test CREATE INDEX with multiple columns on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_multi_column_index(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_multi_col");
	let index_name = unique_index_name("idx_multi");

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            first_name VARCHAR(100),
            last_name VARCHAR(100),
            email VARCHAR(255)
        )"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Build CREATE INDEX with multiple columns
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone())
		.table(table_name.clone())
		.col("last_name")
		.col("first_name");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create multi-column index");

	// Verify index exists and has 2 columns
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(a.attname) FROM pg_index i
         JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
         JOIN pg_class c ON c.oid = i.indexrelid
         WHERE c.relname = $1",
	)
	.bind(&index_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(column_count, 2);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test CREATE INDEX with multiple columns on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_create_multi_column_index(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_multi_col");
	let index_name = unique_index_name("idx_multi");

	// Create table
	sqlx::query(&format!(
		"CREATE TABLE `{}` (
            id INT AUTO_INCREMENT PRIMARY KEY,
            first_name VARCHAR(100),
            last_name VARCHAR(100),
            email VARCHAR(255)
        )",
		table_name
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Build CREATE INDEX with multiple columns
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone())
		.table(table_name.clone())
		.col("last_name")
		.col("first_name");

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create multi-column index");

	// Verify index has 2 columns
	let column_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(DISTINCT column_name) FROM information_schema.statistics
         WHERE table_name = ? AND index_name = ?",
	)
	.bind(&table_name)
	.bind(&index_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(column_count, 2);

	// Cleanup
	sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// NOTE: Partial index tests (with WHERE clause) and INCLUDE column tests
// are omitted because CreateIndexStatement does not currently support
// the `cond()` and `include()` methods.

// =============================================================================
// Error Path Tests
// =============================================================================

/// EP-05: Test CREATE INDEX on non-existent column fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_index_nonexistent_column_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_idx_err");
	let index_name = unique_index_name("idx_bad");

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY, email VARCHAR(255))"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Try to create index on non-existent column
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone())
		.table(table_name.clone())
		.col("nonexistent_column");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail
	assert!(result.is_err());

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// EP-14: Test CREATE INDEX duplicate name fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_duplicate_index_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_dup_idx");
	let index_name = unique_index_name("idx_dup");

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY, email VARCHAR(255), name VARCHAR(100))"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create first index
	let mut stmt1 = Query::create_index();
	stmt1
		.name(index_name.clone())
		.table(table_name.clone())
		.col("email");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("First index creation should succeed");

	// Try to create another index with same name
	let mut stmt2 = Query::create_index();
	stmt2
		.name(index_name.clone())
		.table(table_name.clone())
		.col("name");

	let (sql, _values) = builder.build_create_index(&stmt2);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail with duplicate
	assert!(result.is_err());

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
// State Transition Tests
// =============================================================================

/// ST-02: Test CREATE TABLE → CREATE INDEX → DROP INDEX state transition
#[rstest]
#[tokio::test]
async fn test_postgres_index_state_transition(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_state");
	let index_name = unique_index_name("idx_state");

	let builder = PostgresQueryBuilder::new();

	// Create table using Query builder
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("email")
				.column_type(ColumnType::String(Some(255)))
				.not_null(false),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// State 1: Table only, no index
	let index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = $1 AND indexname = $2",
	)
	.bind(&table_name)
	.bind(&index_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(index_count, 0, "State 1: No index should exist");

	// State 2: Create index
	let mut create_idx = Query::create_index();
	create_idx
		.name(index_name.clone())
		.table(table_name.clone())
		.col("email");

	let (sql, _values) = builder.build_create_index(&create_idx);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	let index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = $1 AND indexname = $2",
	)
	.bind(&table_name)
	.bind(&index_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(index_count, 1, "State 2: Index should exist");

	// State 3: Drop index
	let mut drop_idx = Query::drop_index();
	drop_idx.name(index_name.clone());

	let (sql, _values) = builder.build_drop_index(&drop_idx);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop index");

	let index_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_indexes WHERE tablename = $1 AND indexname = $2",
	)
	.bind(&table_name)
	.bind(&index_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(index_count, 0, "State 3: Index should be dropped");

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// ST-11: Test CREATE INDEX → REINDEX → verify intact
#[rstest]
#[tokio::test]
async fn test_postgres_reindex_preserves_index(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_reindex_state");
	let index_name = unique_index_name("idx_reindex");

	let builder = PostgresQueryBuilder::new();

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY, value VARCHAR(255))"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create index
	let mut create_idx = Query::create_index();
	create_idx
		.name(index_name.clone())
		.table(table_name.clone())
		.col("value");

	let (sql, _values) = builder.build_create_index(&create_idx);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index");

	// Insert some data
	for i in 0..10 {
		sqlx::query(&format!(
			r#"INSERT INTO {} ("value") VALUES ('value_{}')"#,
			pg_ident(&table_name),
			i
		))
		.execute(pool.as_ref())
		.await
		.unwrap();
	}

	// REINDEX
	let mut reindex_stmt = Query::reindex();
	reindex_stmt.index(index_name.clone());

	let (sql, _values) = builder.build_reindex(&reindex_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to reindex");

	// Verify index still exists and is usable
	let index_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE indexname = $1")
			.bind(&index_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(index_exists, 1);

	// Verify data is still queryable via index (this tests the index is working)
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {} WHERE "value" = 'value_5'"#,
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
}

// =============================================================================
// Combination Tests
// =============================================================================

/// CB-02: Test INDEX: unique × columns combinations
#[rstest]
#[case::single_non_unique(false, 1)]
#[case::single_unique(true, 1)]
#[case::multi_non_unique(false, 2)]
#[case::multi_unique(true, 2)]
#[tokio::test]
async fn test_postgres_index_combinations(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] is_unique: bool,
	#[case] column_count: usize,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_combo");
	let index_name = unique_index_name("idx_combo");

	let builder = PostgresQueryBuilder::new();

	// Create table with enough columns
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
            id SERIAL PRIMARY KEY,
            col1 VARCHAR(100),
            col2 VARCHAR(100),
            col3 VARCHAR(100)
        )"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Build index with specified properties
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone()).table(table_name.clone());

	for i in 1..=column_count {
		stmt.col(format!("col{}", i));
	}

	if is_unique {
		stmt.unique();
	}

	let (sql, _values) = builder.build_create_index(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect(&format!(
			"Failed to create index with unique={}, columns={}",
			is_unique, column_count
		));

	// Verify index properties
	let index_info: (bool, i64) = sqlx::query_as(
		"SELECT idx.indisunique, array_length(idx.indkey, 1)::bigint
         FROM pg_index idx
         JOIN pg_class c ON c.oid = idx.indexrelid
         WHERE c.relname = $1",
	)
	.bind(&index_name)
	.fetch_one(pool.as_ref())
	.await
	.expect("Failed to query index info");

	assert_eq!(index_info.0, is_unique, "Unique property mismatch");
	assert_eq!(index_info.1, column_count as i64, "Column count mismatch");

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
// DROP INDEX IF EXISTS Tests
// =============================================================================

/// Test DROP INDEX IF EXISTS on non-existent index (should not fail)
#[rstest]
#[tokio::test]
async fn test_postgres_drop_index_if_exists_nonexistent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let index_name = unique_index_name("idx_nonexistent");

	let builder = PostgresQueryBuilder::new();

	// Build DROP INDEX IF EXISTS for non-existent index
	let mut stmt = Query::drop_index();
	stmt.name(index_name.clone()).if_exists();

	let (sql, _values) = builder.build_drop_index(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should succeed (no-op)
	assert!(result.is_ok());
}

/// Test DROP INDEX without IF EXISTS on non-existent index (should fail)
#[rstest]
#[tokio::test]
async fn test_postgres_drop_index_nonexistent_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let index_name = unique_index_name("idx_nonexistent");

	let builder = PostgresQueryBuilder::new();

	// Build DROP INDEX for non-existent index (no IF EXISTS)
	let mut stmt = Query::drop_index();
	stmt.name(index_name.clone());

	let (sql, _values) = builder.build_drop_index(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail
	assert!(result.is_err());
}
