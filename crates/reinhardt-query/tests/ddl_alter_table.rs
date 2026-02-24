//! ALTER TABLE operations integration tests
//!
//! Tests for ALTER TABLE operations including:
//! - ADD COLUMN
//! - DROP COLUMN
//! - RENAME COLUMN
//! - MODIFY COLUMN
//! - ADD/DROP CONSTRAINT
//! - RENAME TABLE

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{
	ColumnDef, ColumnType, ForeignKeyAction, IntoIden, IntoTableRef, TableConstraint,
};

mod common;
use common::{
	MySqlContainer, PgContainer, mysql_ddl, pg_ident, postgres_ddl, unique_constraint_name,
	unique_table_name,
};

// =============================================================================
// Happy Path Tests - ADD COLUMN
// =============================================================================

/// HP-05: Test ALTER TABLE ADD COLUMN on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_add_column(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_alter_add");

	// Create initial table
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
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify initial column count
	let initial_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(initial_count, 2);

	// ALTER TABLE ADD COLUMN
	let mut alter_stmt = Query::alter_table();
	alter_stmt.table(table_name.clone()).add_column(
		ColumnDef::new("age")
			.column_type(ColumnType::Integer)
			.not_null(false),
	);

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add column");

	// Verify column was added
	let new_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(new_count, 3);

	// Verify the new column exists
	let column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1 AND column_name = $2",
	)
	.bind(&table_name)
	.bind("age")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(column_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// HP-05: Test ALTER TABLE ADD COLUMN on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_alter_table_add_column(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_alter_add");

	// Create initial table
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		);

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// ALTER TABLE ADD COLUMN
	let mut alter_stmt = Query::alter_table();
	alter_stmt.table(table_name.clone()).add_column(
		ColumnDef::new("age")
			.column_type(ColumnType::Integer)
			.not_null(false),
	);

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add column");

	// Verify column was added
	let column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = ? AND column_name = ?",
	)
	.bind(&table_name)
	.bind("age")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(column_exists, 1);

	// Cleanup
	sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Happy Path Tests - DROP COLUMN
// =============================================================================

/// HP-06: Test ALTER TABLE DROP COLUMN on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_drop_column(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_alter_drop");

	// Create table with extra column
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
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("to_remove")
				.column_type(ColumnType::Text)
				.not_null(false),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify column exists before drop
	let column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1 AND column_name = $2",
	)
	.bind(&table_name)
	.bind("to_remove")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(column_exists, 1);

	// ALTER TABLE DROP COLUMN
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(table_name.clone())
		.drop_column("to_remove");

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop column");

	// Verify column was dropped
	let column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1 AND column_name = $2",
	)
	.bind(&table_name)
	.bind("to_remove")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(column_exists, 0);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// HP-06: Test ALTER TABLE DROP COLUMN on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_alter_table_drop_column(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_alter_drop");

	// Create table with extra column
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("to_remove")
				.column_type(ColumnType::Text)
				.not_null(false),
		);

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// ALTER TABLE DROP COLUMN
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(table_name.clone())
		.drop_column("to_remove");

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop column");

	// Verify column was dropped
	let column_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = ? AND column_name = ?",
	)
	.bind(&table_name)
	.bind("to_remove")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(column_exists, 0);

	// Cleanup
	sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Happy Path Tests - RENAME COLUMN
// =============================================================================

/// Test ALTER TABLE RENAME COLUMN on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_rename_column(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_alter_rename");

	// Create initial table
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
			ColumnDef::new("old_name")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// ALTER TABLE RENAME COLUMN
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(table_name.clone())
		.rename_column("old_name", "new_name");

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to rename column");

	// Verify old column name no longer exists
	let old_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1 AND column_name = $2",
	)
	.bind(&table_name)
	.bind("old_name")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(old_exists, 0);

	// Verify new column name exists
	let new_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1 AND column_name = $2",
	)
	.bind(&table_name)
	.bind("new_name")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(new_exists, 1);

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
// Happy Path Tests - ADD CONSTRAINT
// =============================================================================

/// HP-07: Test ALTER TABLE ADD CONSTRAINT (UNIQUE) on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_add_unique_constraint(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_alter_unique");
	let constraint_name = unique_constraint_name("uq_email");

	// Create table without unique constraint
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
				.not_null(true),
		);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// ALTER TABLE ADD CONSTRAINT UNIQUE
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(table_name.clone())
		.add_constraint(TableConstraint::Unique {
			name: Some(constraint_name.clone().into_iden()),
			columns: vec!["email".into_iden()],
		});

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add unique constraint");

	// Verify constraint exists
	let constraint_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.table_constraints
         WHERE table_name = $1 AND constraint_name = $2 AND constraint_type = 'UNIQUE'",
	)
	.bind(&table_name)
	.bind(&constraint_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(constraint_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// HP-07: Test ALTER TABLE ADD CONSTRAINT (FOREIGN KEY) on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_add_foreign_key_constraint(
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

	// Create child table without FK
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
		);

	let (sql, _values) = builder.build_create_table(&child_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create child table");

	// ALTER TABLE ADD FOREIGN KEY
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(child_table.clone())
		.add_constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: Some(ForeignKeyAction::Restrict),
		});

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add foreign key constraint");

	// Verify FK exists
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
// Happy Path Tests - DROP CONSTRAINT
// =============================================================================

/// Test ALTER TABLE DROP CONSTRAINT on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_drop_constraint(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_drop_const");
	let constraint_name = unique_constraint_name("uq_code");

	let builder = PostgresQueryBuilder::new();

	// Create table with unique constraint
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
			ColumnDef::new("code")
				.column_type(ColumnType::String(Some(50)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Add unique constraint first
	let mut add_stmt = Query::alter_table();
	add_stmt
		.table(table_name.clone())
		.add_constraint(TableConstraint::Unique {
			name: Some(constraint_name.clone().into_iden()),
			columns: vec!["code".into_iden()],
		});

	let (sql, _values) = builder.build_alter_table(&add_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add constraint");

	// Verify constraint exists
	let constraint_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.table_constraints WHERE constraint_name = $1",
	)
	.bind(&constraint_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(constraint_exists, 1);

	// DROP CONSTRAINT
	let mut drop_stmt = Query::alter_table();
	drop_stmt
		.table(table_name.clone())
		.drop_constraint(constraint_name.clone());

	let (sql, _values) = builder.build_alter_table(&drop_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop constraint");

	// Verify constraint no longer exists
	let constraint_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.table_constraints WHERE constraint_name = $1",
	)
	.bind(&constraint_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(constraint_exists, 0);

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
// Happy Path Tests - RENAME TABLE
// =============================================================================

/// Test ALTER TABLE RENAME TABLE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_rename_table(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let old_name = unique_table_name("test_old_name");
	let new_name = unique_table_name("test_new_name");

	let builder = PostgresQueryBuilder::new();

	// Create table with old name
	let mut create_stmt = Query::create_table();
	create_stmt.table(old_name.clone()).col(
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

	// Verify old name exists
	let old_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&old_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(old_exists, 1);

	// RENAME TABLE
	let mut rename_stmt = Query::alter_table();
	rename_stmt
		.table(old_name.clone())
		.rename_table(new_name.clone());

	let (sql, _values) = builder.build_alter_table(&rename_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to rename table");

	// Verify old name no longer exists
	let old_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&old_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(old_exists, 0);

	// Verify new name exists
	let new_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&new_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(new_exists, 1);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS {}"#, pg_ident(&new_name)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Error Path Tests
// =============================================================================

/// EP-03: Test ALTER TABLE DROP non-existent column fails
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_drop_nonexistent_column_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_drop_noexist");

	let builder = PostgresQueryBuilder::new();

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

	// Try to drop non-existent column
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(table_name.clone())
		.drop_column("nonexistent_column");

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
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

/// Test ALTER TABLE DROP COLUMN IF EXISTS on non-existent column (should not fail)
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_drop_column_if_exists(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_drop_ifexists");

	let builder = PostgresQueryBuilder::new();

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

	// Drop non-existent column with IF EXISTS
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(table_name.clone())
		.drop_column_if_exists("nonexistent_column");

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should succeed with IF EXISTS
	assert!(result.is_ok());

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

/// ST-01: Test CREATE TABLE → ALTER add col → verify state transition
#[rstest]
#[tokio::test]
async fn test_postgres_state_transition_add_column(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_state_add");

	let builder = PostgresQueryBuilder::new();

	// State 1: Create table
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

	// Insert data in initial state
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id") VALUES (1)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert");

	// State 2: Add column
	let mut alter_stmt = Query::alter_table();
	alter_stmt.table(table_name.clone()).add_column(
		ColumnDef::new("status")
			.column_type(ColumnType::String(Some(50)))
			.not_null(false),
	);

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add column");

	// Verify: existing row has NULL for new column
	let status: Option<String> = sqlx::query_scalar(&format!(
		r#"SELECT "status" FROM {} WHERE "id" = 1"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert!(status.is_none());

	// Verify: new column is usable
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "status") VALUES (2, 'active')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert with new column");

	let status: Option<String> = sqlx::query_scalar(&format!(
		r#"SELECT "status" FROM {} WHERE "id" = 2"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(status, Some("active".to_string()));

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// ST-06: Test CREATE TABLE → ADD FK → DROP FK → ADD FK state transitions
#[rstest]
#[tokio::test]
async fn test_postgres_state_transition_foreign_key_cycle(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let parent_table = unique_table_name("test_parent_fk");
	let child_table = unique_table_name("test_child_fk");
	let fk_name = unique_constraint_name("fk_parent");

	let builder = PostgresQueryBuilder::new();

	// Create parent table
	let mut parent_stmt = Query::create_table();
	parent_stmt.table(parent_table.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&parent_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create parent table");

	// Create child table
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
				.not_null(false),
		);

	let (sql, _values) = builder.build_create_table(&child_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create child table");

	// State 1: No FK - can insert orphan
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id") VALUES (1, 999)"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.expect("Should be able to insert orphan without FK");

	// Clean up orphan for next test
	sqlx::query(&format!(
		r#"DELETE FROM {} WHERE "id" = 1"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// State 2: Add FK
	let mut add_fk_stmt = Query::alter_table();
	add_fk_stmt
		.table(child_table.clone())
		.add_constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		});

	let (sql, _values) = builder.build_alter_table(&add_fk_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add FK");

	// Insert valid parent first
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id") VALUES (1)"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Valid FK insert should work
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id") VALUES (1, 1)"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.expect("Should be able to insert with valid FK");

	// Invalid FK insert should fail
	let invalid_insert = sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id") VALUES (2, 999)"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await;
	assert!(invalid_insert.is_err());

	// State 3: Drop FK
	let mut drop_fk_stmt = Query::alter_table();
	drop_fk_stmt
		.table(child_table.clone())
		.drop_constraint(fk_name.clone());

	let (sql, _values) = builder.build_alter_table(&drop_fk_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop FK");

	// After dropping FK, orphan insert should work again
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id") VALUES (2, 999)"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.expect("Should be able to insert orphan after dropping FK");

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

/// ST-12: Test multiple ALTER TABLE operations in sequence
#[rstest]
#[tokio::test]
async fn test_postgres_multiple_alter_operations_sequence(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_multi_alter");
	let constraint_name = unique_constraint_name("uq_code");

	let builder = PostgresQueryBuilder::new();

	// Initial state: single column
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

	// Step 1: Add first column
	let mut alter1 = Query::alter_table();
	alter1.table(table_name.clone()).add_column(
		ColumnDef::new("name")
			.column_type(ColumnType::String(Some(255)))
			.not_null(false),
	);

	let (sql, _values) = builder.build_alter_table(&alter1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add first column");

	// Step 2: Add second column
	let mut alter2 = Query::alter_table();
	alter2.table(table_name.clone()).add_column(
		ColumnDef::new("code")
			.column_type(ColumnType::String(Some(50)))
			.not_null(false),
	);

	let (sql, _values) = builder.build_alter_table(&alter2);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add second column");

	// Step 3: Add unique constraint
	let mut alter3 = Query::alter_table();
	alter3
		.table(table_name.clone())
		.add_constraint(TableConstraint::Unique {
			name: Some(constraint_name.clone().into_iden()),
			columns: vec!["code".into_iden()],
		});

	let (sql, _values) = builder.build_alter_table(&alter3);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add constraint");

	// Verify final state: 3 columns + 1 unique constraint
	let column_count: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.columns WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(column_count, 3);

	let constraint_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.table_constraints
         WHERE table_name = $1 AND constraint_type = 'UNIQUE'",
	)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(constraint_count, 1);

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
// MODIFY COLUMN Tests
// =============================================================================

/// Test ALTER TABLE MODIFY COLUMN on PostgreSQL (ALTER COLUMN TYPE)
#[rstest]
#[tokio::test]
async fn test_postgres_alter_table_modify_column_type(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_modify_col");

	let builder = PostgresQueryBuilder::new();

	// Create table with VARCHAR(50)
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
			ColumnDef::new("description")
				.column_type(ColumnType::String(Some(50)))
				.not_null(false),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Verify initial type
	let initial_max_length: Option<i32> = sqlx::query_scalar(
		"SELECT character_maximum_length::int FROM information_schema.columns
         WHERE table_name = $1 AND column_name = $2",
	)
	.bind(&table_name)
	.bind("description")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(initial_max_length, Some(50));

	// MODIFY COLUMN to VARCHAR(255)
	let mut alter_stmt = Query::alter_table();
	alter_stmt.table(table_name.clone()).modify_column(
		ColumnDef::new("description")
			.column_type(ColumnType::String(Some(255)))
			.not_null(false),
	);

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to modify column");

	// Verify new type
	let new_max_length: Option<i32> = sqlx::query_scalar(
		"SELECT character_maximum_length::int FROM information_schema.columns
         WHERE table_name = $1 AND column_name = $2",
	)
	.bind(&table_name)
	.bind("description")
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(new_max_length, Some(255));

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}
