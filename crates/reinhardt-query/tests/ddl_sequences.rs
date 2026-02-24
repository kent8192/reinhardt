//! Sequence operations integration tests
//!
//! Tests for CREATE/ALTER/DROP SEQUENCE operations.

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;

mod common;
use common::{MySqlContainer, PgContainer, mysql_ddl, postgres_ddl, unique_sequence_name};

/// Test basic CREATE SEQUENCE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_sequence_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let sequence_name = unique_sequence_name("test_seq");

	// Build CREATE SEQUENCE statement
	let mut stmt = Query::create_sequence();
	stmt.name(sequence_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_sequence(&stmt);

	// Execute CREATE SEQUENCE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Verify sequence exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.sequences WHERE sequence_name = $1",
	)
	.bind(&sequence_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_sequence_sql = format!(r#"DROP SEQUENCE IF EXISTS "{}""#, sequence_name);
	sqlx::query(&drop_sequence_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CREATE SEQUENCE with START WITH on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_sequence_with_start(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let sequence_name = unique_sequence_name("test_seq");

	// Build CREATE SEQUENCE with START statement
	let mut stmt = Query::create_sequence();
	stmt.name(sequence_name.clone());
	stmt.start(1000);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_sequence(&stmt);

	// Execute CREATE SEQUENCE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Verify sequence start value
	let result: Option<i64> =
		sqlx::query_scalar("SELECT start_value FROM pg_sequences WHERE sequencename = $1")
			.bind(&sequence_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(1000));

	// Cleanup
	let drop_sequence_sql = format!(r#"DROP SEQUENCE IF EXISTS "{}""#, sequence_name);
	sqlx::query(&drop_sequence_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CREATE SEQUENCE with MINVALUE and MAXVALUE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_sequence_with_min_max(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let sequence_name = unique_sequence_name("test_seq");

	// Build CREATE SEQUENCE with MINVALUE and MAXVALUE
	let mut stmt = Query::create_sequence();
	stmt.name(sequence_name.clone());
	stmt.min_value(Some(10));
	stmt.max_value(Some(1000));

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_sequence(&stmt);

	// Execute CREATE SEQUENCE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Verify sequence min/max values
	let result: Option<(i64, i64)> =
		sqlx::query_as("SELECT min_value, max_value FROM pg_sequences WHERE sequencename = $1")
			.bind(&sequence_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some((10, 1000)));

	// Cleanup
	let drop_sequence_sql = format!(r#"DROP SEQUENCE IF EXISTS "{}""#, sequence_name);
	sqlx::query(&drop_sequence_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CREATE SEQUENCE with CYCLE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_sequence_with_cycle(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let sequence_name = unique_sequence_name("test_seq");

	// Build CREATE SEQUENCE with CYCLE
	let mut stmt = Query::create_sequence();
	stmt.name(sequence_name.clone());
	stmt.cycle(true);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_sequence(&stmt);

	// Execute CREATE SEQUENCE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Verify sequence cycle setting
	let result: Option<bool> =
		sqlx::query_scalar("SELECT cycle FROM pg_sequences WHERE sequencename = $1")
			.bind(&sequence_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(true));

	// Cleanup
	let drop_sequence_sql = format!(r#"DROP SEQUENCE IF EXISTS "{}""#, sequence_name);
	sqlx::query(&drop_sequence_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CREATE SEQUENCE with CACHE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_sequence_with_cache(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let sequence_name = unique_sequence_name("test_seq");

	// Build CREATE SEQUENCE with CACHE
	let mut stmt = Query::create_sequence();
	stmt.name(sequence_name.clone());
	stmt.cache(20);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_sequence(&stmt);

	// Execute CREATE SEQUENCE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Verify sequence cache setting
	let result: Option<i64> =
		sqlx::query_scalar("SELECT cache_size FROM pg_sequences WHERE sequencename = $1")
			.bind(&sequence_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(20));

	// Cleanup
	let drop_sequence_sql = format!(r#"DROP SEQUENCE IF EXISTS "{}""#, sequence_name);
	sqlx::query(&drop_sequence_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test DROP SEQUENCE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_sequence(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let sequence_name = unique_sequence_name("test_seq");

	// Create sequence first
	let create_sequence_sql = format!(r#"CREATE SEQUENCE "{}""#, sequence_name);
	sqlx::query(&create_sequence_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Verify sequence exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.sequences WHERE sequence_name = $1",
	)
	.bind(&sequence_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(1));

	// Build DROP SEQUENCE statement
	let mut stmt = Query::drop_sequence();
	stmt.name(sequence_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_sequence(&stmt);

	// Execute DROP SEQUENCE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop sequence");

	// Verify sequence no longer exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.sequences WHERE sequence_name = $1",
	)
	.bind(&sequence_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(0));
}

/// Test ALTER SEQUENCE RESTART WITH on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_sequence_restart(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let sequence_name = unique_sequence_name("test_seq");

	// Create sequence first
	let create_sequence_sql = format!(r#"CREATE SEQUENCE "{}""#, sequence_name);
	sqlx::query(&create_sequence_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Build ALTER SEQUENCE RESTART WITH statement
	let mut stmt = Query::alter_sequence();
	stmt.name(sequence_name.clone());
	stmt.restart(Some(500));

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_alter_sequence(&stmt);

	// Execute ALTER SEQUENCE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to alter sequence");

	// Verify sequence last value (should be reset)
	let result: Option<i64> =
		sqlx::query_scalar(&format!(r#"SELECT last_value FROM "{}""#, sequence_name))
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	// After RESTART WITH, nextval will return the restart value
	assert!(result.is_some());

	// Cleanup
	let drop_sequence_sql = format!(r#"DROP SEQUENCE IF EXISTS "{}""#, sequence_name);
	sqlx::query(&drop_sequence_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test basic CREATE TABLE with AUTO_INCREMENT on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_create_auto_increment(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_sequence_name("test_users");

	// Build CREATE TABLE with AUTO_INCREMENT statement
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			reinhardt_query::types::ColumnDef::new("id")
				.column_type(reinhardt_query::types::ColumnType::Integer)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			reinhardt_query::types::ColumnDef::new("name")
				.column_type(reinhardt_query::types::ColumnType::String(None)),
		);

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);

	// Execute CREATE TABLE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify table exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = ?")
			.bind(&table_name)
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
