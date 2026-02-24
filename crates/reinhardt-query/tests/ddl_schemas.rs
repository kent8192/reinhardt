//! Schema operations integration tests
//!
//! Tests for CREATE/ALTER/DROP SCHEMA operations.

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;

mod common;
use common::{PgContainer, postgres_ddl, unique_schema_name};

/// Test basic CREATE SCHEMA on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_schema_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let schema_name = unique_schema_name("test_schema");

	// Build CREATE SCHEMA statement
	let mut stmt = Query::create_schema();
	stmt.name(schema_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_schema(&stmt);

	// Execute CREATE SCHEMA
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create schema");

	// Verify schema exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_schema_sql = format!(r#"DROP SCHEMA IF EXISTS "{}""#, schema_name);
	sqlx::query(&drop_schema_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CREATE SCHEMA with IF NOT EXISTS on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_schema_if_not_exists(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let schema_name = unique_schema_name("test_schema");

	// Build CREATE SCHEMA IF NOT EXISTS statement
	let mut stmt = Query::create_schema();
	stmt.name(schema_name.clone());
	stmt.if_not_exists();

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_schema(&stmt);

	// Execute CREATE SCHEMA IF NOT EXISTS
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create schema");

	// Verify schema exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();

	assert_eq!(result, Some(1));

	// Try to create again (should not fail due to IF NOT EXISTS)
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create schema again");

	// Cleanup
	let drop_schema_sql = format!(r#"DROP SCHEMA IF EXISTS "{}""#, schema_name);
	sqlx::query(&drop_schema_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test DROP SCHEMA on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_schema(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let schema_name = unique_schema_name("test_schema");

	// Create schema first
	let create_schema_sql = format!(r#"CREATE SCHEMA "{}""#, schema_name);
	sqlx::query(&create_schema_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create schema");

	// Verify schema exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(1));

	// Build DROP SCHEMA statement
	let mut stmt = Query::drop_schema();
	stmt.name(schema_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_schema(&stmt);

	// Execute DROP SCHEMA
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop schema");

	// Verify schema no longer exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(0));
}

/// Test DROP SCHEMA with IF EXISTS on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_schema_if_exists(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let schema_name = unique_schema_name("test_schema");

	// Build DROP SCHEMA IF EXISTS statement
	let mut stmt = Query::drop_schema();
	stmt.name(schema_name.clone());
	stmt.if_exists();

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_schema(&stmt);

	// Execute DROP SCHEMA IF EXISTS (schema doesn't exist, should not fail)
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop schema");

	// Verify schema does not exist
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();

	assert_eq!(result, Some(0));
}

/// Test ALTER SCHEMA RENAME TO on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_schema_rename(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let schema_name = unique_schema_name("test_schema");
	let new_schema_name = unique_schema_name("new_schema");

	// Create schema first
	let create_schema_sql = format!(r#"CREATE SCHEMA "{}""#, schema_name);
	sqlx::query(&create_schema_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create schema");

	// Build ALTER SCHEMA RENAME TO statement
	let mut stmt = Query::alter_schema();
	stmt.name(schema_name.clone());
	stmt.rename_to(new_schema_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_alter_schema(&stmt);

	// Execute ALTER SCHEMA RENAME TO
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to alter schema");

	// Verify old schema no longer exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(0));

	// Verify new schema exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&new_schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(result, Some(1));

	// Cleanup
	let drop_schema_sql = format!(r#"DROP SCHEMA IF EXISTS "{}""#, new_schema_name);
	sqlx::query(&drop_schema_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CREATE SCHEMA with AUTHORIZATION on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_schema_with_authorization(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let schema_name = unique_schema_name("test_schema");

	// Build CREATE SCHEMA with AUTHORIZATION statement
	let mut stmt = Query::create_schema();
	stmt.name(schema_name.clone());
	stmt.authorization("postgres");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_schema(&stmt);

	// Execute CREATE SCHEMA
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create schema");

	// Verify schema exists
	let result: Option<i64> = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = $1",
	)
	.bind(&schema_name)
	.fetch_optional(pool.as_ref())
	.await
	.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_schema_sql = format!(r#"DROP SCHEMA IF EXISTS "{}""#, schema_name);
	sqlx::query(&drop_schema_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}
