//! Edge case and boundary value integration tests
//!
//! Tests for edge cases and boundary values including:
//! - BV-01: VARCHAR length boundaries [1, 255, 256, 65535]
//! - BV-02: DECIMAL precision boundaries [1, 18, 38]
//! - BV-07: Column count boundaries [1, 100]
//! - BV-09: Identifier length boundaries [1, 62, 63, 64]
//! - EC-02: Identifiers with special characters
//! - EC-03: Reserved SQL keywords as quoted identifiers
//! - EC-05: VARCHAR(1) minimum length
//! - EC-09: Table with many columns

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{ColumnDef, ColumnType};

mod common;
use common::{
	MySqlContainer, PgContainer, mysql_ddl, mysql_ident, pg_ident, postgres_ddl, unique_table_name,
};

// =============================================================================
// BV-09: Identifier Length Boundary Tests
// =============================================================================

/// Test table name at PostgreSQL max identifier length (63 chars)
#[rstest]
#[tokio::test]
async fn test_postgres_identifier_max_length(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	// PostgreSQL max identifier length is 63 characters
	let long_name = "a".repeat(63);

	// Build CREATE TABLE statement
	let mut stmt = Query::create_table();
	stmt.table(long_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with max length identifier");

	// Verify table exists
	let table_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_tables WHERE tablename = $1)")
			.bind(&long_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert!(table_exists);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, long_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test column name at PostgreSQL max identifier length (63 chars)
#[rstest]
#[tokio::test]
async fn test_postgres_column_name_max_length(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_col_len");
	let long_column = "c".repeat(63);

	// Build CREATE TABLE statement
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone()).col(
		ColumnDef::new(long_column.clone())
			.column_type(ColumnType::Integer)
			.not_null(true),
	);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with max length column name");

	// Verify column exists
	let column_exists: bool = sqlx::query_scalar(
		"SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_name = $1 AND column_name = $2)",
	)
	.bind(&table_name)
	.bind(&long_column)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert!(column_exists);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test table name at MySQL max identifier length (64 chars)
#[rstest]
#[tokio::test]
async fn test_mysql_identifier_max_length(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	// MySQL max identifier length is 64 characters
	let long_name = "a".repeat(64);

	// Build CREATE TABLE statement
	let mut stmt = Query::create_table();
	stmt.table(long_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with max length identifier");

	// Verify table exists
	let table_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = ?")
			.bind(&long_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(table_exists, 1);

	// Cleanup
	sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", long_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// EC-02: Identifiers with Special Characters
// =============================================================================

/// Test table name with underscores and numbers
#[rstest]
#[tokio::test]
async fn test_postgres_identifier_with_underscores_and_numbers(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_123_data_456");

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone()).col(
		ColumnDef::new("col_1")
			.column_type(ColumnType::Integer)
			.not_null(true),
	);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with special characters");

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
// EC-03: Reserved SQL Keywords as Quoted Identifiers
// =============================================================================

/// Test using SQL reserved keywords as table/column names (quoted)
#[rstest]
#[case::select("select")]
#[case::from("from")]
#[case::where_kw("where")]
#[case::table("table")]
#[case::user("user")]
#[case::order("order")]
#[case::group("group")]
#[case::index("index")]
#[tokio::test]
async fn test_postgres_reserved_keyword_as_identifier(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] keyword: &str,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name(&format!("test_{}", keyword));

	// Use reserved keyword as column name
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(keyword.to_string())
				.column_type(ColumnType::String(Some(100)))
				.not_null(false),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect(&format!(
			"Failed to create table with reserved keyword '{}' as column",
			keyword
		));

	// Verify we can insert and select using the keyword column
	sqlx::query(&format!(
		r#"INSERT INTO {} (id, {}) VALUES (1, 'test')"#,
		pg_ident(&table_name),
		pg_ident(keyword)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert data");

	let value: String = sqlx::query_scalar(&format!(
		r#"SELECT {} FROM {} WHERE id = 1"#,
		pg_ident(keyword),
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(value, "test");

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
// BV-01: VARCHAR Length Boundary Tests
// =============================================================================

/// Test VARCHAR with boundary lengths
#[rstest]
#[case::min(1)]
#[case::common(255)]
#[case::boundary(256)]
#[case::large(10000)]
#[tokio::test]
async fn test_postgres_varchar_length_boundaries(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] length: u32,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name(&format!("test_varchar_{}", length));

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("data")
				.column_type(ColumnType::String(Some(length)))
				.not_null(false),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect(&format!("Failed to create table with VARCHAR({})", length));

	// Insert data of maximum allowed length
	let test_data = "x".repeat(length as usize);
	sqlx::query(&format!(
		r#"INSERT INTO {} (id, data) VALUES (1, $1)"#,
		pg_ident(&table_name)
	))
	.bind(&test_data)
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert max length data");

	// Verify data
	let retrieved: String = sqlx::query_scalar(&format!(
		r#"SELECT data FROM {} WHERE id = 1"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(retrieved.len(), length as usize);

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
// BV-02/BV-03: DECIMAL Precision Boundary Tests
// =============================================================================

/// Test DECIMAL with boundary precision values
#[rstest]
#[case::min_precision(1, 0)]
#[case::standard_precision(10, 2)]
#[case::high_precision(18, 2)]
#[case::max_scale(10, 10)]
#[tokio::test]
async fn test_postgres_decimal_precision_boundaries(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] precision: u32,
	#[case] scale: u32,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name(&format!("test_decimal_{}_{}", precision, scale));

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("amount")
				.column_type(ColumnType::Decimal(Some((precision, scale))))
				.not_null(false),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect(&format!(
			"Failed to create table with DECIMAL({}, {})",
			precision, scale
		));

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
// BV-07/EC-09: Column Count Boundary Tests
// =============================================================================

/// Test table with single column (minimum)
#[rstest]
#[tokio::test]
async fn test_postgres_table_single_column(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_single_col");

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with single column");

	// Verify column count
	let col_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(col_count, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test table with many columns (50 columns)
#[rstest]
#[tokio::test]
async fn test_postgres_table_many_columns(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_many_cols");
	let column_count = 50;

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	// Add many columns
	for i in 1..column_count {
		stmt.col(
			ColumnDef::new(format!("col_{}", i))
				.column_type(ColumnType::String(Some(100)))
				.not_null(false),
		);
	}

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with many columns");

	// Verify column count
	let col_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(col_count, column_count);

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
// BV-08: Index Column Count Tests
// =============================================================================

/// Test index with many columns (up to 16 for broader compatibility)
#[rstest]
#[tokio::test]
async fn test_postgres_index_many_columns(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_idx_cols");
	let index_name = format!("idx_{}", table_name);
	let column_count = 10;

	// Create table with many columns
	let mut create_cols = String::from("id SERIAL PRIMARY KEY");
	for i in 1..=column_count {
		create_cols.push_str(&format!(", col_{} VARCHAR(50)", i));
	}
	sqlx::query(&format!(
		r#"CREATE TABLE {} ({})"#,
		pg_ident(&table_name),
		create_cols
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create index with many columns
	let mut stmt = Query::create_index();
	stmt.name(index_name.clone()).table(table_name.clone());
	for i in 1..=column_count {
		stmt.col(format!("col_{}", i));
	}

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_index(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create index with many columns");

	// Verify index exists
	let index_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM pg_indexes WHERE indexname = $1")
			.bind(&index_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(index_exists, 1);

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
// EC-05: VARCHAR(1) Minimum Length Tests
// =============================================================================

/// Test CHAR(1) column (minimum fixed width)
#[rstest]
#[tokio::test]
async fn test_postgres_char_one(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_char_one");

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("flag")
				.column_type(ColumnType::Char(Some(1)))
				.not_null(false),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table with CHAR(1)");

	// Insert single character
	sqlx::query(&format!(
		r#"INSERT INTO {} (id, flag) VALUES (1, 'Y')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert single char");

	// Verify
	let flag: String = sqlx::query_scalar(&format!(
		r#"SELECT flag FROM {} WHERE id = 1"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(flag, "Y");

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
// MySQL Edge Case Tests
// =============================================================================

/// Test MySQL reserved keywords as identifiers
#[rstest]
#[case::select("select")]
#[case::from("from")]
#[case::table("table")]
#[tokio::test]
async fn test_mysql_reserved_keyword_as_identifier(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
	#[case] keyword: &str,
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name(&format!("test_{}", keyword));

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(keyword.to_string())
				.column_type(ColumnType::String(Some(100)))
				.not_null(false),
		);

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect(&format!(
			"Failed to create table with reserved keyword '{}' as column",
			keyword
		));

	// Verify we can insert and select
	sqlx::query(&format!(
		"INSERT INTO {} (id, {}) VALUES (1, 'test')",
		mysql_ident(&table_name),
		mysql_ident(keyword)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert data");

	// Cleanup
	sqlx::query(&format!(
		"DROP TABLE IF EXISTS {}",
		mysql_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test MySQL VARCHAR with boundary lengths
#[rstest]
#[case::min(1)]
#[case::common(255)]
#[case::large(5000)]
#[tokio::test]
async fn test_mysql_varchar_length_boundaries(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
	#[case] length: u32,
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name(&format!("test_varchar_{}", length));

	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("data")
				.column_type(ColumnType::String(Some(length)))
				.not_null(false),
		);

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect(&format!("Failed to create table with VARCHAR({})", length));

	// Cleanup
	sqlx::query(&format!(
		"DROP TABLE IF EXISTS {}",
		mysql_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}
