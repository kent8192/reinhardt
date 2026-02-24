//! Database operations integration tests
//!
//! Tests for CREATE/ALTER/DROP DATABASE operations.
//!
//! Note: DATABASE operations require special handling because:
//! - CREATE/DROP DATABASE cannot be executed within a transaction
//! - We need to connect to a different database for these operations
//! - Cannot drop the database we're currently connected to

use std::sync::Arc;

use rstest::rstest;
use sqlx::postgres::PgPoolOptions;

use reinhardt_query::prelude::*;

mod common;
use common::{PgContainer, postgres_ddl, unique_database_name};

// =============================================================================
// Helper to create admin connection to postgres database
// =============================================================================

/// Create a connection to the postgres system database for admin operations
async fn admin_pool(port: u16) -> Arc<sqlx::PgPool> {
	let url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);
	let pool = PgPoolOptions::new()
		.max_connections(1)
		.connect(&url)
		.await
		.expect("Failed to connect to postgres database");
	Arc::new(pool)
}

// =============================================================================
// PostgreSQL CREATE DATABASE Tests
// =============================================================================

/// Test basic CREATE DATABASE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_database_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("test_db");

	// Build CREATE DATABASE statement
	let mut stmt = Query::create_database();
	stmt.name(db_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_database(&stmt);
	sqlx::query(&sql)
		.execute(admin.as_ref())
		.await
		.expect("Failed to create database");

	// Verify database exists
	let db_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
			.bind(&db_name)
			.fetch_one(admin.as_ref())
			.await
			.unwrap();
	assert!(db_exists);

	// Cleanup
	sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}""#, db_name))
		.execute(admin.as_ref())
		.await
		.unwrap();
}

/// Test CREATE DATABASE IF NOT EXISTS on PostgreSQL
///
/// Note: PostgreSQL does NOT support IF NOT EXISTS for CREATE DATABASE.
/// This test verifies that the behavior is correct - second attempt should fail
/// because IF NOT EXISTS is ignored (not supported by PostgreSQL).
#[rstest]
#[tokio::test]
async fn test_postgres_create_database_if_not_exists(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("test_db_ine");

	let builder = PostgresQueryBuilder::new();

	// Create database first time
	let mut stmt1 = Query::create_database();
	stmt1.name(db_name.clone()).if_not_exists();

	let (sql, _values) = builder.build_create_database(&stmt1);
	sqlx::query(&sql)
		.execute(admin.as_ref())
		.await
		.expect("First create should succeed");

	// Create database again with IF NOT EXISTS
	// PostgreSQL does NOT support IF NOT EXISTS for CREATE DATABASE,
	// so the second attempt should fail with "database already exists"
	let mut stmt2 = Query::create_database();
	stmt2.name(db_name.clone()).if_not_exists();

	let (sql, _values) = builder.build_create_database(&stmt2);
	let result = sqlx::query(&sql).execute(admin.as_ref()).await;
	assert!(
		result.is_err(),
		"Second create should fail - PostgreSQL does not support IF NOT EXISTS for CREATE DATABASE"
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}""#, db_name))
		.execute(admin.as_ref())
		.await
		.unwrap();
}

/// Test CREATE DATABASE with options on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_database_with_options(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("test_db_opts");

	// Build CREATE DATABASE with options
	let mut stmt = Query::create_database();
	stmt.name(db_name.clone())
		.encoding("UTF8")
		.template("template0");

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_database(&stmt);
	sqlx::query(&sql)
		.execute(admin.as_ref())
		.await
		.expect("Failed to create database with options");

	// Verify database exists with correct encoding
	let encoding: String = sqlx::query_scalar(
		"SELECT pg_encoding_to_char(encoding) FROM pg_database WHERE datname = $1",
	)
	.bind(&db_name)
	.fetch_one(admin.as_ref())
	.await
	.unwrap();
	assert_eq!(encoding, "UTF8");

	// Cleanup
	sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}""#, db_name))
		.execute(admin.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// PostgreSQL DROP DATABASE Tests
// =============================================================================

/// Test DROP DATABASE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_database(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("test_db_drop");

	// Create database using raw SQL
	sqlx::query(&format!(r#"CREATE DATABASE "{}""#, db_name))
		.execute(admin.as_ref())
		.await
		.expect("Failed to create database");

	// Verify database exists
	let db_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
			.bind(&db_name)
			.fetch_one(admin.as_ref())
			.await
			.unwrap();
	assert!(db_exists);

	// Build DROP DATABASE statement
	let mut stmt = Query::drop_database();
	stmt.name(db_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_database(&stmt);
	sqlx::query(&sql)
		.execute(admin.as_ref())
		.await
		.expect("Failed to drop database");

	// Verify database no longer exists
	let db_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
			.bind(&db_name)
			.fetch_one(admin.as_ref())
			.await
			.unwrap();
	assert!(!db_exists);
}

/// Test DROP DATABASE IF EXISTS on non-existent database
#[rstest]
#[tokio::test]
async fn test_postgres_drop_database_if_exists_nonexistent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("nonexistent_db");

	// Build DROP DATABASE IF EXISTS statement
	let mut stmt = Query::drop_database();
	stmt.name(db_name.clone()).if_exists();

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_database(&stmt);

	// Should succeed (no-op)
	let result = sqlx::query(&sql).execute(admin.as_ref()).await;
	assert!(result.is_ok());
}

/// Test DROP DATABASE without IF EXISTS on non-existent database fails
#[rstest]
#[tokio::test]
async fn test_postgres_drop_database_nonexistent_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("nonexistent_db");

	// Build DROP DATABASE statement (no IF EXISTS)
	let mut stmt = Query::drop_database();
	stmt.name(db_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_database(&stmt);
	let result = sqlx::query(&sql).execute(admin.as_ref()).await;

	// Should fail
	assert!(result.is_err());
}

// =============================================================================
// State Transition Tests
// =============================================================================

/// State transition: CREATE DATABASE → verify exists → DROP DATABASE
#[rstest]
#[tokio::test]
async fn test_postgres_database_state_transition(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("state_db");

	let builder = PostgresQueryBuilder::new();

	// State 1: No database
	let db_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
			.bind(&db_name)
			.fetch_one(admin.as_ref())
			.await
			.unwrap();
	assert!(!db_exists, "State 1: Database should not exist");

	// State 2: Create database
	let mut create_stmt = Query::create_database();
	create_stmt.name(db_name.clone());

	let (sql, _values) = builder.build_create_database(&create_stmt);
	sqlx::query(&sql)
		.execute(admin.as_ref())
		.await
		.expect("Failed to create database");

	let db_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
			.bind(&db_name)
			.fetch_one(admin.as_ref())
			.await
			.unwrap();
	assert!(db_exists, "State 2: Database should exist");

	// State 3: Drop database
	let mut drop_stmt = Query::drop_database();
	drop_stmt.name(db_name.clone());

	let (sql, _values) = builder.build_drop_database(&drop_stmt);
	sqlx::query(&sql)
		.execute(admin.as_ref())
		.await
		.expect("Failed to drop database");

	let db_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
			.bind(&db_name)
			.fetch_one(admin.as_ref())
			.await
			.unwrap();
	assert!(!db_exists, "State 3: Database should not exist");
}

// =============================================================================
// Error Path Tests
// =============================================================================

/// Test CREATE DATABASE without IF NOT EXISTS on existing database fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_duplicate_database_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, _pool, port, _url) = postgres_ddl.await;
	let admin = admin_pool(port).await;
	let db_name = unique_database_name("dup_db");

	let builder = PostgresQueryBuilder::new();

	// Create first database
	let mut stmt1 = Query::create_database();
	stmt1.name(db_name.clone());

	let (sql, _values) = builder.build_create_database(&stmt1);
	sqlx::query(&sql)
		.execute(admin.as_ref())
		.await
		.expect("First create should succeed");

	// Try to create another database with same name (no IF NOT EXISTS)
	let mut stmt2 = Query::create_database();
	stmt2.name(db_name.clone());

	let (sql, _values) = builder.build_create_database(&stmt2);
	let result = sqlx::query(&sql).execute(admin.as_ref()).await;

	// Should fail - duplicate database
	assert!(result.is_err());

	// Cleanup
	sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{}""#, db_name))
		.execute(admin.as_ref())
		.await
		.unwrap();
}
