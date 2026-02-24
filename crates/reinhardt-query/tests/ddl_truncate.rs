//! TRUNCATE TABLE operations integration tests
//!
//! Tests for TRUNCATE TABLE operations including:
//! - Basic TRUNCATE
//! - RESTART IDENTITY (PostgreSQL)
//! - CASCADE (PostgreSQL)
//! - State transition tests

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{ColumnDef, ColumnType, IntoIden, IntoTableRef, TableConstraint};

mod common;
use common::{
	MySqlContainer, PgContainer, mysql_ddl, pg_ident, postgres_ddl, unique_constraint_name,
	unique_sequence_name, unique_table_name,
};

// =============================================================================
// Happy Path Tests - Basic TRUNCATE
// =============================================================================

/// HP-18: Test basic TRUNCATE TABLE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_truncate_table_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_truncate");

	let builder = PostgresQueryBuilder::new();

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
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Insert test data
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "name") VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Carol')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert test data");

	// Verify data exists
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 3);

	// TRUNCATE TABLE
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(table_name.clone());

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to truncate table");

	// Verify table is empty
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 0);

	// Verify table structure still exists
	let table_exists: i64 =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = $1")
			.bind(&table_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(table_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// HP-18: Test basic TRUNCATE TABLE on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_truncate_table_basic(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_truncate");

	let builder = MySqlQueryBuilder::new();

	// Create table
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

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Insert test data
	sqlx::query(&format!(
		"INSERT INTO `{}` (`name`) VALUES ('Alice'), ('Bob'), ('Carol')",
		table_name
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert test data");

	// Verify data exists
	let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{}`", table_name))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(count, 3);

	// TRUNCATE TABLE
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(table_name.clone());

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to truncate table");

	// Verify table is empty
	let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{}`", table_name))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(count, 0);

	// Cleanup
	sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Happy Path Tests - RESTART IDENTITY
// =============================================================================

/// HP-19: Test TRUNCATE TABLE RESTART IDENTITY on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_truncate_restart_identity(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_truncate_restart");
	let sequence_name = unique_sequence_name("seq_id");

	let builder = PostgresQueryBuilder::new();

	// Create sequence
	let mut seq_stmt = Query::create_sequence();
	seq_stmt.name(sequence_name.clone()).start(1);

	let (sql, _values) = builder.build_create_sequence(&seq_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create sequence");

	// Create table with sequence default
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
            "id" INTEGER NOT NULL PRIMARY KEY DEFAULT nextval('{}'),
            "name" VARCHAR(255) NOT NULL
        )"#,
		pg_ident(&table_name),
		pg_ident(&sequence_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Associate sequence with column
	sqlx::query(&format!(
		r#"ALTER SEQUENCE {} OWNED BY {}."id""#,
		pg_ident(&sequence_name),
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to set sequence ownership");

	// Insert data (sequence advances)
	sqlx::query(&format!(
		r#"INSERT INTO {} ("name") VALUES ('Alice'), ('Bob'), ('Carol')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert test data");

	// Verify sequence value advanced
	let max_id: i32 = sqlx::query_scalar(&format!(
		r#"SELECT MAX("id") FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(max_id, 3);

	// TRUNCATE TABLE RESTART IDENTITY
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(table_name.clone()).restart_identity();

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to truncate table");

	// Insert new data - should start from 1 again
	sqlx::query(&format!(
		r#"INSERT INTO {} ("name") VALUES ('Dave')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert after truncate");

	// Verify ID restarted from 1
	let new_id: i32 = sqlx::query_scalar(&format!(
		r#"SELECT "id" FROM {} WHERE "name" = 'Dave'"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(new_id, 1);

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
// Happy Path Tests - CASCADE
// =============================================================================

/// Test TRUNCATE TABLE CASCADE on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_truncate_cascade(
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

	// Create child table with FK
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

	// Add FK constraint
	let mut add_fk = Query::alter_table();
	add_fk
		.table(child_table.clone())
		.add_constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".into_iden()],
			on_delete: None,
			on_update: None,
		});

	let (sql, _values) = builder.build_alter_table(&add_fk);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add FK");

	// Insert data
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "name") VALUES (1, 'Parent1'), (2, 'Parent2')"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert parent data");

	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id") VALUES (1, 1), (2, 1), (3, 2)"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert child data");

	// TRUNCATE parent with CASCADE
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(parent_table.clone()).cascade();

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to truncate with cascade");

	// Verify parent is empty
	let parent_count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&parent_table)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(parent_count, 0);

	// Verify child is also empty (cascaded)
	let child_count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&child_table)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(child_count, 0);

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
// Error Path Tests
// =============================================================================

/// EP-13: Test TRUNCATE table with FK references without CASCADE fails
#[rstest]
#[tokio::test]
async fn test_postgres_truncate_fk_reference_without_cascade_fails(
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

	// Create child table with FK
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

	// Add FK constraint
	let mut add_fk = Query::alter_table();
	add_fk
		.table(child_table.clone())
		.add_constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".into_iden()],
			on_delete: None,
			on_update: None,
		});

	let (sql, _values) = builder.build_alter_table(&add_fk);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add FK");

	// Insert data
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id") VALUES (1)"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id") VALUES (1, 1)"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// TRUNCATE parent without CASCADE should fail
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(parent_table.clone());

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail due to FK constraint
	assert!(result.is_err());

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
// State Transition Tests
// =============================================================================

/// ST-08: Test CREATE TABLE → TRUNCATE → INSERT → verify state transition
#[rstest]
#[tokio::test]
async fn test_postgres_state_transition_truncate_insert(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_state_truncate");

	let builder = PostgresQueryBuilder::new();

	// State 1: Create table with data
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
			ColumnDef::new("value")
				.column_type(ColumnType::String(Some(100)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Insert initial data
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "value") VALUES (1, 'initial_1'), (2, 'initial_2')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert initial data");

	// State 2: TRUNCATE
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(table_name.clone());

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to truncate");

	// Verify empty
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 0);

	// State 3: Insert new data
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "value") VALUES (10, 'new_1'), (20, 'new_2'), (30, 'new_3')"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert new data");

	// Verify new data
	let count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {}"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(count, 3);

	// Verify original IDs are gone
	let old_data_exists: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM {} WHERE "id" IN (1, 2)"#,
		pg_ident(&table_name)
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(old_data_exists, 0);

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
// Decision Table Tests
// =============================================================================

/// DT-07: TRUNCATE decision table - cascade × fk_refs combinations
#[rstest]
#[case::no_cascade_no_refs(false, false, true)] // No FK refs, no cascade needed
#[case::no_cascade_with_refs(false, true, false)] // FK refs, no cascade = error
#[case::cascade_no_refs(true, false, true)] // No FK refs, cascade is fine
#[case::cascade_with_refs(true, true, true)] // FK refs with cascade = success
#[tokio::test]
async fn test_postgres_truncate_decision_table(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] use_cascade: bool,
	#[case] has_fk_reference: bool,
	#[case] should_succeed: bool,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let parent_table = unique_table_name("test_dt_parent");
	let child_table = unique_table_name("test_dt_child");
	let fk_name = unique_constraint_name("fk_dt");

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

	// Insert parent data
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id") VALUES (1)"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	if has_fk_reference {
		// Create child table with FK
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

		// Add FK
		let mut add_fk = Query::alter_table();
		add_fk
			.table(child_table.clone())
			.add_constraint(TableConstraint::ForeignKey {
				name: Some(fk_name.clone().into_iden()),
				columns: vec!["parent_id".into_iden()],
				ref_table: Box::new(parent_table.clone().into_table_ref()),
				ref_columns: vec!["id".into_iden()],
				on_delete: None,
				on_update: None,
			});

		let (sql, _values) = builder.build_alter_table(&add_fk);
		sqlx::query(&sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to add FK");

		// Insert child data
		sqlx::query(&format!(
			r#"INSERT INTO {} ("id", "parent_id") VALUES (1, 1)"#,
			pg_ident(&child_table)
		))
		.execute(pool.as_ref())
		.await
		.unwrap();
	}

	// Execute TRUNCATE
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(parent_table.clone());
	if use_cascade {
		truncate_stmt.cascade();
	}

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert_eq!(
		result.is_ok(),
		should_succeed,
		"TRUNCATE with cascade={}, fk_refs={} should {}",
		use_cascade,
		has_fk_reference,
		if should_succeed { "succeed" } else { "fail" }
	);

	// Cleanup
	if has_fk_reference {
		sqlx::query(&format!(
			r#"DROP TABLE IF EXISTS {}"#,
			pg_ident(&child_table)
		))
		.execute(pool.as_ref())
		.await
		.unwrap();
	}
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// Multiple Tables Test
// =============================================================================

/// Test TRUNCATE multiple tables at once on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_truncate_multiple_tables(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table1 = unique_table_name("test_multi1");
	let table2 = unique_table_name("test_multi2");

	let builder = PostgresQueryBuilder::new();

	// Create first table
	let mut create1 = Query::create_table();
	create1.table(table1.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&create1);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table 1");

	// Create second table
	let mut create2 = Query::create_table();
	create2.table(table2.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&create2);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table 2");

	// Insert data into both tables
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id") VALUES (1), (2), (3)"#,
		pg_ident(&table1)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	sqlx::query(&format!(
		r#"INSERT INTO {} ("id") VALUES (10), (20)"#,
		pg_ident(&table2)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// TRUNCATE both tables
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(table1.clone()).table(table2.clone());

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to truncate multiple tables");

	// Verify both tables are empty
	let count1: i64 = sqlx::query_scalar(&format!(r#"SELECT COUNT(*) FROM {}"#, pg_ident(&table1)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(count1, 0);

	let count2: i64 = sqlx::query_scalar(&format!(r#"SELECT COUNT(*) FROM {}"#, pg_ident(&table2)))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(count2, 0);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS {}"#, pg_ident(&table1)))
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS {}"#, pg_ident(&table2)))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// RESTRICT Option Test
// =============================================================================

/// Test TRUNCATE TABLE RESTRICT on PostgreSQL (default behavior)
#[rstest]
#[tokio::test]
async fn test_postgres_truncate_restrict(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let parent_table = unique_table_name("test_restrict_parent");
	let child_table = unique_table_name("test_restrict_child");
	let fk_name = unique_constraint_name("fk_restrict");

	let builder = PostgresQueryBuilder::new();

	// Create parent and child tables with FK
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

	// Add FK
	let mut add_fk = Query::alter_table();
	add_fk
		.table(child_table.clone())
		.add_constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".into_iden()],
			on_delete: None,
			on_update: None,
		});

	let (sql, _values) = builder.build_alter_table(&add_fk);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add FK");

	// Insert data
	sqlx::query(&format!(
		r#"INSERT INTO {} ("id") VALUES (1)"#,
		pg_ident(&parent_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	sqlx::query(&format!(
		r#"INSERT INTO {} ("id", "parent_id") VALUES (1, 1)"#,
		pg_ident(&child_table)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// TRUNCATE with explicit RESTRICT
	let mut truncate_stmt = Query::truncate_table();
	truncate_stmt.table(parent_table.clone()).restrict();

	let (sql, _values) = builder.build_truncate_table(&truncate_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// RESTRICT should fail when FK references exist
	assert!(result.is_err());

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
