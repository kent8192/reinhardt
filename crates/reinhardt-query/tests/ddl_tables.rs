//! Table operations integration tests
//!
//! Tests for CREATE/DROP TABLE operations including:
//! - Basic CREATE/DROP TABLE
//! - Composite primary keys
//! - Foreign key constraints
//! - CHECK constraints
//! - IF NOT EXISTS / IF EXISTS behavior
//! - Various column types

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{
	ColumnDef, ColumnType, ForeignKeyAction, IntoIden, IntoTableRef, TableConstraint,
};

mod common;
use common::{
	ColumnTypeFactory, MySqlContainer, PgContainer, mysql_ddl, pg_ident, postgres_ddl,
	unique_constraint_name, unique_table_name,
};

/// Test basic CREATE TABLE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_table_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");

	// Build CREATE TABLE statement
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(None))
				.not_null(true),
		)
		.col(ColumnDef::new("email").column_type(ColumnType::String(None)));

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&stmt);

	// Execute CREATE TABLE
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify table exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&table_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test DROP TABLE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_table(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");

	// Create table
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
			ColumnDef::new("name")
				.column_type(ColumnType::String(None))
				.not_null(true),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify table exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&table_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(result, Some(1));

	// Drop table
	let mut drop_stmt = Query::drop_table();
	drop_stmt.table(table_name.clone());

	let (sql, _values) = builder.build_drop_table(&drop_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop table");

	// Verify table no longer exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&table_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(result, Some(0));
}

/// Test basic CREATE TABLE on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_create_table_basic(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_users");

	// Build CREATE TABLE statement
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(None))
				.not_null(true),
		)
		.col(ColumnDef::new("email").column_type(ColumnType::String(None)));

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
	sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Happy Path Tests - Composite Primary Key
// =============================================================================

/// HP-02: Test CREATE TABLE with composite primary key on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_table_composite_primary_key(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_composite_pk");

	let builder = PostgresQueryBuilder::new();

	// Build CREATE TABLE with composite PK
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("tenant_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("entity_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("value")
				.column_type(ColumnType::String(Some(255)))
				.not_null(false),
		)
		.constraint(TableConstraint::PrimaryKey {
			name: None,
			columns: vec!["tenant_id".into_iden(), "entity_id".into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify table exists with 3 columns
	let column_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(column_count, 3);

	// Verify primary key constraint exists
	let pk_columns: Vec<String> = sqlx::query_scalar(
		"SELECT a.attname FROM pg_index i
         JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
         JOIN pg_class c ON c.oid = i.indrelid
         WHERE c.relname = $1 AND i.indisprimary
         ORDER BY array_position(i.indkey, a.attnum)",
	)
	.bind(&table_name)
	.fetch_all(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(pk_columns, vec!["tenant_id", "entity_id"]);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// HP-02: Test CREATE TABLE with composite primary key on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_create_table_composite_primary_key(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_composite_pk");

	let builder = MySqlQueryBuilder::new();

	// Build CREATE TABLE with composite PK
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("tenant_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("entity_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("value")
				.column_type(ColumnType::String(Some(255)))
				.not_null(false),
		)
		.constraint(TableConstraint::PrimaryKey {
			name: None,
			columns: vec!["tenant_id".into_iden(), "entity_id".into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify table exists
	let table_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = ?")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(table_exists, 1);

	// Cleanup
	sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Happy Path Tests - Foreign Key Constraints
// =============================================================================

/// HP-03: Test CREATE TABLE with foreign key constraint on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_table_with_foreign_key(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let parent_table = unique_table_name("test_parent");
	let child_table = unique_table_name("test_child");
	let fk_name = unique_constraint_name("fk_parent");

	let builder = PostgresQueryBuilder::new();

	// Create parent table
	let mut parent_stmt = Query::create_table();
	parent_stmt
		.table(parent_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&parent_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create parent table");

	// Create child table with FK constraint
	let mut child_stmt = Query::create_table();
	child_stmt
		.table(child_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("parent_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("content")
				.column_type(ColumnType::Text)
				.not_null(false),
		)
		.constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: Some(ForeignKeyAction::Restrict),
		});

	let (sql, _values) = builder.build_create_table(&child_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create child table");

	// Verify FK constraint exists
	let fk_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.table_constraints
         WHERE table_name = $1 AND constraint_name = $2 AND constraint_type = 'FOREIGN KEY'",
	)
	.bind(&child_table)
	.bind(&fk_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(fk_exists, 1);

	// Test FK behavior - insert parent first
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "name") VALUES (1, 'Parent')"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Valid FK insert
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id", "content") VALUES (1, 1, 'Content')"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.expect("Valid FK insert should succeed");

	// Invalid FK insert should fail
	let invalid_insert = sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id", "content") VALUES (2, 999, 'Invalid')"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await;
	assert!(invalid_insert.is_err());

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// Happy Path Tests - CHECK Constraints (PostgreSQL only)
// =============================================================================

/// HP-04: Test CREATE TABLE with CHECK constraint on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_table_with_check_constraint(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_check");

	let builder = PostgresQueryBuilder::new();

	// Build CREATE TABLE with CHECK constraint on column
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("quantity")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.check(Expr::col("quantity").gte(0)),
		)
		.col(
			ColumnDef::new("status")
				.column_type(ColumnType::String(Some(20)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Valid insert (quantity >= 0)
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "quantity", "status") VALUES (1, 10, 'active')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Valid insert should succeed");

	// Invalid insert (quantity < 0) should fail
	let invalid_insert = sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "quantity", "status") VALUES (2, -5, 'active')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await;
	assert!(invalid_insert.is_err());

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
// Happy Path Tests - Multiple Column Types
// =============================================================================

/// HP-01: Test CREATE TABLE with all common column types on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_table_all_column_types(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_all_types");

	let builder = PostgresQueryBuilder::new();

	// Build CREATE TABLE with various column types
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("tiny_val")
				.column_type(ColumnType::TinyInteger)
				.not_null(false),
		)
		.col(
			ColumnDef::new("small_val")
				.column_type(ColumnType::SmallInteger)
				.not_null(false),
		)
		.col(
			ColumnDef::new("big_val")
				.column_type(ColumnType::BigInteger)
				.not_null(false),
		)
		.col(
			ColumnDef::new("float_val")
				.column_type(ColumnType::Float)
				.not_null(false),
		)
		.col(
			ColumnDef::new("double_val")
				.column_type(ColumnType::Double)
				.not_null(false),
		)
		.col(
			ColumnDef::new("decimal_val")
				.column_type(ColumnType::Decimal(Some((10, 2))))
				.not_null(false),
		)
		.col(
			ColumnDef::new("bool_val")
				.column_type(ColumnType::Boolean)
				.not_null(false),
		)
		.col(
			ColumnDef::new("char_val")
				.column_type(ColumnType::Char(Some(10)))
				.not_null(false),
		)
		.col(
			ColumnDef::new("varchar_val")
				.column_type(ColumnType::String(Some(255)))
				.not_null(false),
		)
		.col(
			ColumnDef::new("text_val")
				.column_type(ColumnType::Text)
				.not_null(false),
		)
		.col(
			ColumnDef::new("date_val")
				.column_type(ColumnType::Date)
				.not_null(false),
		)
		.col(
			ColumnDef::new("time_val")
				.column_type(ColumnType::Time)
				.not_null(false),
		)
		.col(
			ColumnDef::new("datetime_val")
				.column_type(ColumnType::DateTime)
				.not_null(false),
		)
		.col(
			ColumnDef::new("timestamp_val")
				.column_type(ColumnType::Timestamp)
				.not_null(false),
		)
		.col(
			ColumnDef::new("timestamptz_val")
				.column_type(ColumnType::TimestampWithTimeZone)
				.not_null(false),
		)
		.col(
			ColumnDef::new("binary_val")
				.column_type(ColumnType::Binary(Some(100)))
				.not_null(false),
		)
		.col(
			ColumnDef::new("blob_val")
				.column_type(ColumnType::Blob)
				.not_null(false),
		)
		.col(
			ColumnDef::new("uuid_val")
				.column_type(ColumnType::Uuid)
				.not_null(false),
		)
		.col(
			ColumnDef::new("json_val")
				.column_type(ColumnType::Json)
				.not_null(false),
		)
		.col(
			ColumnDef::new("jsonb_val")
				.column_type(ColumnType::JsonBinary)
				.not_null(false),
		);

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify column count
	let column_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(column_count, 21);

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
// Decision Table Tests - IF NOT EXISTS / IF EXISTS
// =============================================================================

/// DT-01: Test CREATE TABLE IF NOT EXISTS decision table
#[rstest]
#[case::if_not_exists_table_not_exists(true, false, true)] // With IF NOT EXISTS, table doesn't exist -> success
#[case::if_not_exists_table_exists(true, true, true)] // With IF NOT EXISTS, table exists -> success (no-op)
#[case::no_if_not_exists_table_not_exists(false, false, true)] // Without IF NOT EXISTS, table doesn't exist -> success
#[case::no_if_not_exists_table_exists(false, true, false)] // Without IF NOT EXISTS, table exists -> error
#[tokio::test]
async fn test_postgres_create_table_if_not_exists_decision(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] use_if_not_exists: bool,
	#[case] table_exists: bool,
	#[case] should_succeed: bool,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_if_not_exists");

	let builder = PostgresQueryBuilder::new();

	// Pre-create table if needed
	if table_exists {
		sqlx::query(&format!(
			r#"CREATE TABLE {} ("id" INTEGER PRIMARY KEY)"#,
			pg_ident(&table_name)
		))
		.execute(pool.as_ref())
		.await
		.expect("Failed to pre-create table");
	}

	// Build CREATE TABLE statement
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);
	if use_if_not_exists {
		stmt.if_not_exists();
	}

	let (sql, _values) = builder.build_create_table(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert_eq!(
		result.is_ok(),
		should_succeed,
		"CREATE TABLE with if_not_exists={}, table_exists={} should {}",
		use_if_not_exists,
		table_exists,
		if should_succeed { "succeed" } else { "fail" }
	);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// DT-02: Test DROP TABLE IF EXISTS decision table
#[rstest]
#[case::if_exists_table_exists(true, true, true)] // With IF EXISTS, table exists -> success
#[case::if_exists_table_not_exists(true, false, true)] // With IF EXISTS, table doesn't exist -> success (no-op)
#[case::no_if_exists_table_exists(false, true, true)] // Without IF EXISTS, table exists -> success
#[case::no_if_exists_table_not_exists(false, false, false)] // Without IF EXISTS, table doesn't exist -> error
#[tokio::test]
async fn test_postgres_drop_table_if_exists_decision(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] use_if_exists: bool,
	#[case] table_exists: bool,
	#[case] should_succeed: bool,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_drop_if_exists");

	let builder = PostgresQueryBuilder::new();

	// Pre-create table if needed
	if table_exists {
		sqlx::query(&format!(
			r#"CREATE TABLE {} ("id" INTEGER PRIMARY KEY)"#,
			pg_ident(&table_name)
		))
		.execute(pool.as_ref())
		.await
		.expect("Failed to pre-create table");
	}

	// Build DROP TABLE statement
	let mut stmt = Query::drop_table();
	stmt.table(table_name.clone());
	if use_if_exists {
		stmt.if_exists();
	}

	let (sql, _values) = builder.build_drop_table(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert_eq!(
		result.is_ok(),
		should_succeed,
		"DROP TABLE with if_exists={}, table_exists={} should {}",
		use_if_exists,
		table_exists,
		if should_succeed { "succeed" } else { "fail" }
	);

	// Cleanup (just in case)
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.ok();
}

// =============================================================================
// Error Path Tests
// =============================================================================

/// EP-01: Test DROP TABLE non-existent table fails without IF EXISTS
#[rstest]
#[tokio::test]
async fn test_postgres_drop_nonexistent_table_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_nonexistent");

	let builder = PostgresQueryBuilder::new();

	// Try to drop non-existent table without IF EXISTS
	let mut stmt = Query::drop_table();
	stmt.table(table_name.clone());

	let (sql, _values) = builder.build_drop_table(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail
	assert!(result.is_err());
}

/// EP-02: Test CREATE TABLE duplicate name fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_duplicate_table_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_duplicate");

	let builder = PostgresQueryBuilder::new();

	// Create table first time
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
		.expect("First creation should succeed");

	// Try to create same table again without IF NOT EXISTS
	let mut stmt2 = Query::create_table();
	stmt2.table(table_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&stmt2);
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
// UNIQUE Constraint Tests
// =============================================================================

/// Test CREATE TABLE with UNIQUE constraint
#[rstest]
#[tokio::test]
async fn test_postgres_create_table_with_unique_constraint(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_unique");
	let constraint_name = unique_constraint_name("uq_email");

	let builder = PostgresQueryBuilder::new();

	// Build CREATE TABLE with UNIQUE constraint
	let mut stmt = Query::create_table();
	stmt.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("email")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.constraint(TableConstraint::Unique {
			name: Some(constraint_name.clone().into_iden()),
			columns: vec!["email".into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Insert first row
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "email") VALUES (1, 'test@example.com')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("First insert should succeed");

	// Duplicate email should fail
	let duplicate_insert = sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "email") VALUES (2, 'test@example.com')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await;
	assert!(duplicate_insert.is_err());

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
// Column Type Iterator Test
// =============================================================================

/// Test CREATE TABLE with integer type variants
#[rstest]
#[tokio::test]
async fn test_postgres_integer_type_variants(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	for (idx, col_type) in ColumnTypeFactory::integer_types().into_iter().enumerate() {
		let table_name = unique_table_name(&format!("test_int_type_{}", idx));

		let mut stmt = Query::create_table();
		stmt.table(table_name.clone())
			.col(
				ColumnDef::new("id")
					.column_type(ColumnType::Integer)
					.not_null(true)
					.primary_key(true),
			)
			.col(
				ColumnDef::new("value")
					.column_type(col_type)
					.not_null(false),
			);

		let (sql, _values) = builder.build_create_table(&stmt);
		sqlx::query(&sql)
			.execute(pool.as_ref())
			.await
			.expect(&format!(
				"Failed to create table with integer type variant {}",
				idx
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
}
