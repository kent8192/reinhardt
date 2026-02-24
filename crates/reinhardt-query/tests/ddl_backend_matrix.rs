//! Cross-backend compatibility matrix tests
//!
//! Tests that verify DDL operations work correctly across different database backends.
//! Uses rstest matrix feature to test combinations of:
//! - Column types across backends
//! - Index configurations
//! - Constraint types
//! - DROP behaviors

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{
	ColumnDef, ColumnType, ForeignKeyAction, IntoIden, IntoTableRef, TableConstraint,
};

mod common;
use common::{
	ColumnTypeFactory, MySqlContainer, PgContainer, mysql_ddl, postgres_ddl,
	unique_constraint_name, unique_index_name, unique_table_name,
};

// =============================================================================
// Column Type × Backend Matrix (CB-01)
// =============================================================================

/// Test column types across PostgreSQL backend
#[rstest]
#[tokio::test]
async fn test_postgres_column_types_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	// Get all PostgreSQL-compatible column types
	let types = ColumnTypeFactory::postgres_specific();

	for (i, col_type) in types.iter().enumerate() {
		let table_name = unique_table_name(&format!("type_test_{}", i));

		let mut stmt = Query::create_table();
		stmt.table(table_name.clone())
			.col(
				ColumnDef::new("id")
					.column_type(ColumnType::Integer)
					.not_null(true)
					.primary_key(true),
			)
			.col(ColumnDef::new("test_col").column_type(col_type.clone()));

		let (sql, _values) = builder.build_create_table(&stmt);
		let result = sqlx::query(&sql).execute(pool.as_ref()).await;

		assert!(
			result.is_ok(),
			"Failed to create table with column type {:?}: {:?}",
			col_type,
			result.err()
		);

		// Cleanup
		sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
			.execute(pool.as_ref())
			.await
			.unwrap();
	}
}

/// Test column types across MySQL backend
#[rstest]
#[tokio::test]
async fn test_mysql_column_types_matrix(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let builder = MySqlQueryBuilder::new();

	// Get MySQL-compatible column types
	let types = ColumnTypeFactory::mysql_compatible();

	for (i, col_type) in types.iter().enumerate() {
		let table_name = unique_table_name(&format!("type_test_{}", i));

		let mut stmt = Query::create_table();
		stmt.table(table_name.clone())
			.col(
				ColumnDef::new("id")
					.column_type(ColumnType::Integer)
					.not_null(true)
					.primary_key(true),
			)
			.col(ColumnDef::new("test_col").column_type(col_type.clone()));

		let (sql, _values) = builder.build_create_table(&stmt);
		let result = sqlx::query(&sql).execute(pool.as_ref()).await;

		assert!(
			result.is_ok(),
			"Failed to create table with column type {:?}: {:?}",
			col_type,
			result.err()
		);

		// Cleanup
		sqlx::query(&format!("DROP TABLE IF EXISTS `{}`", table_name))
			.execute(pool.as_ref())
			.await
			.unwrap();
	}
}

// =============================================================================
// Index Type Matrix (CB-02)
// =============================================================================

/// Test unique × non-unique index combinations on PostgreSQL
#[rstest]
#[case::single_non_unique(false, vec!["col1"])]
#[case::single_unique(true, vec!["col1"])]
#[case::multi_non_unique(false, vec!["col1", "col2"])]
#[case::multi_unique(true, vec!["col1", "col2"])]
#[case::triple_non_unique(false, vec!["col1", "col2", "col3"])]
#[case::triple_unique(true, vec!["col1", "col2", "col3"])]
#[tokio::test]
async fn test_postgres_index_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] unique: bool,
	#[case] columns: Vec<&str>,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("idx_matrix");
	let index_name = unique_index_name("idx_test");
	let builder = PostgresQueryBuilder::new();

	// Create table with enough columns
	let mut create_table = Query::create_table();
	create_table
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new("col1")
				.column_type(ColumnType::String(Some(100)))
				.not_null(false),
		)
		.col(
			ColumnDef::new("col2")
				.column_type(ColumnType::String(Some(100)))
				.not_null(false),
		)
		.col(
			ColumnDef::new("col3")
				.column_type(ColumnType::String(Some(100)))
				.not_null(false),
		);

	let (sql, _values) = builder.build_create_table(&create_table);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Create index
	let mut create_index = Query::create_index();
	create_index
		.name(index_name.clone())
		.table(table_name.clone().into_table_ref());

	if unique {
		create_index.unique();
	}

	for col in &columns {
		create_index.col((*col).to_string().into_iden());
	}

	let (sql, _values) = builder.build_create_index(&create_index);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create {} index on columns {:?}",
		if unique { "unique" } else { "non-unique" },
		columns
	);

	// Verify index exists
	let index_exists: bool =
		sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
			.bind(&index_name)
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert!(index_exists, "Index should exist");

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// DROP Behavior Matrix (CB-05)
// =============================================================================

/// Test DROP TABLE with/without IF EXISTS on existent/non-existent tables
#[rstest]
#[case::drop_existing_no_if_exists(true, false, true)]
#[case::drop_existing_with_if_exists(true, true, true)]
#[case::drop_nonexisting_with_if_exists(false, true, true)]
#[case::drop_nonexisting_no_if_exists(false, false, false)]
#[tokio::test]
async fn test_postgres_drop_table_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] create_first: bool,
	#[case] use_if_exists: bool,
	#[case] should_succeed: bool,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("drop_matrix");
	let builder = PostgresQueryBuilder::new();

	// Optionally create the table first
	if create_first {
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
	}

	// Build DROP TABLE statement
	let mut drop_stmt = Query::drop_table();
	drop_stmt.table(table_name.clone());
	if use_if_exists {
		drop_stmt.if_exists();
	}

	let (sql, _values) = builder.build_drop_table(&drop_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	if should_succeed {
		assert!(
			result.is_ok(),
			"DROP TABLE should succeed (create={}, if_exists={})",
			create_first,
			use_if_exists
		);
	} else {
		assert!(
			result.is_err(),
			"DROP TABLE should fail (create={}, if_exists={})",
			create_first,
			use_if_exists
		);
	}
}

/// Test DROP INDEX with/without IF EXISTS
#[rstest]
#[case::drop_existing_no_if_exists(true, false, true)]
#[case::drop_existing_with_if_exists(true, true, true)]
#[case::drop_nonexisting_with_if_exists(false, true, true)]
#[case::drop_nonexisting_no_if_exists(false, false, false)]
#[tokio::test]
async fn test_postgres_drop_index_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] create_first: bool,
	#[case] use_if_exists: bool,
	#[case] should_succeed: bool,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("drop_idx_matrix");
	let index_name = unique_index_name("drop_idx_test");
	let builder = PostgresQueryBuilder::new();

	// Always create the table
	let mut create_table = Query::create_table();
	create_table.table(table_name.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&create_table);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Optionally create the index first
	if create_first {
		let mut create_index = Query::create_index();
		create_index
			.name(index_name.clone())
			.table(table_name.clone().into_table_ref())
			.col("id".to_string().into_iden());

		let (sql, _values) = builder.build_create_index(&create_index);
		sqlx::query(&sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to create index");
	}

	// Build DROP INDEX statement
	let mut drop_stmt = Query::drop_index();
	drop_stmt.name(index_name.clone());
	if use_if_exists {
		drop_stmt.if_exists();
	}

	let (sql, _values) = builder.build_drop_index(&drop_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	if should_succeed {
		assert!(
			result.is_ok(),
			"DROP INDEX should succeed (create={}, if_exists={})",
			create_first,
			use_if_exists
		);
	} else {
		assert!(
			result.is_err(),
			"DROP INDEX should fail (create={}, if_exists={})",
			create_first,
			use_if_exists
		);
	}

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// FK Actions Matrix (CB-10)
// =============================================================================

/// Test foreign key ON DELETE actions
#[rstest]
#[case::restrict(ForeignKeyAction::Restrict)]
#[case::cascade(ForeignKeyAction::Cascade)]
#[case::set_null(ForeignKeyAction::SetNull)]
#[case::no_action(ForeignKeyAction::NoAction)]
#[tokio::test]
async fn test_postgres_fk_on_delete_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] action: ForeignKeyAction,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let parent_table = unique_table_name("parent");
	let child_table = unique_table_name("child");
	let fk_name = unique_constraint_name("fk");
	let builder = PostgresQueryBuilder::new();

	// Create parent table
	let mut create_parent = Query::create_table();
	create_parent.table(parent_table.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&create_parent);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create parent table");

	// Create child table with FK
	// For SET NULL, the column must be nullable
	let is_nullable = matches!(action, ForeignKeyAction::SetNull);

	let mut create_child = Query::create_table();
	create_child
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
				.not_null(!is_nullable),
		)
		.constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".to_string().into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(action.clone()),
			on_update: None,
		});

	let (sql, _values) = builder.build_create_table(&create_child);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with ON DELETE {:?}: {:?}",
		action,
		result.err()
	);

	// Cleanup (child first due to FK)
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, child_table))
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, parent_table))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test foreign key ON UPDATE actions
#[rstest]
#[case::restrict(ForeignKeyAction::Restrict)]
#[case::cascade(ForeignKeyAction::Cascade)]
#[case::set_null(ForeignKeyAction::SetNull)]
#[case::no_action(ForeignKeyAction::NoAction)]
#[tokio::test]
async fn test_postgres_fk_on_update_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] action: ForeignKeyAction,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let parent_table = unique_table_name("parent");
	let child_table = unique_table_name("child");
	let fk_name = unique_constraint_name("fk");
	let builder = PostgresQueryBuilder::new();

	// Create parent table
	let mut create_parent = Query::create_table();
	create_parent.table(parent_table.clone()).col(
		ColumnDef::new("id")
			.column_type(ColumnType::Integer)
			.not_null(true)
			.primary_key(true),
	);

	let (sql, _values) = builder.build_create_table(&create_parent);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create parent table");

	// Create child table with FK
	let is_nullable = matches!(action, ForeignKeyAction::SetNull);

	let mut create_child = Query::create_table();
	create_child
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
				.not_null(!is_nullable),
		)
		.constraint(TableConstraint::ForeignKey {
			name: Some(fk_name.clone().into_iden()),
			columns: vec!["parent_id".to_string().into_iden()],
			ref_table: Box::new(parent_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: None,
			on_update: Some(action.clone()),
		});

	let (sql, _values) = builder.build_create_table(&create_child);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with ON UPDATE {:?}: {:?}",
		action,
		result.err()
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, child_table))
		.execute(pool.as_ref())
		.await
		.unwrap();
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, parent_table))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Nullable Matrix (EQ-04)
// =============================================================================

/// Test NULL/NOT NULL column definitions
#[rstest]
#[case::nullable(false)]
#[case::not_null(true)]
#[tokio::test]
async fn test_postgres_nullable_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] not_null: bool,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("nullable_test");
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
			ColumnDef::new("test_col")
				.column_type(ColumnType::String(Some(100)))
				.not_null(not_null),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Test inserting NULL
	let insert_null = sqlx::query(&format!(
		r#"INSERT INTO "{}" (id, test_col) VALUES (1, NULL)"#,
		table_name
	))
	.execute(pool.as_ref())
	.await;

	if not_null {
		assert!(
			insert_null.is_err(),
			"INSERT NULL should fail for NOT NULL column"
		);
	} else {
		assert!(
			insert_null.is_ok(),
			"INSERT NULL should succeed for nullable column"
		);
	}

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Integer Type Matrix (EQ-01)
// =============================================================================

/// Test integer type variants
#[rstest]
#[case::tiny_int(ColumnType::TinyInteger)]
#[case::small_int(ColumnType::SmallInteger)]
#[case::integer(ColumnType::Integer)]
#[case::big_int(ColumnType::BigInteger)]
#[tokio::test]
async fn test_postgres_integer_type_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] int_type: ColumnType,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("int_type");
	let builder = PostgresQueryBuilder::new();

	// Create table with the integer type
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(ColumnDef::new("int_col").column_type(int_type.clone()));

	let (sql, _values) = builder.build_create_table(&create_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with {:?}: {:?}",
		int_type,
		result.err()
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// String Type Matrix (EQ-02)
// =============================================================================

/// Test string type variants
#[rstest]
#[case::char_10(ColumnType::Char(Some(10)))]
#[case::varchar_100(ColumnType::String(Some(100)))]
#[case::text(ColumnType::Text)]
#[tokio::test]
async fn test_postgres_string_type_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] str_type: ColumnType,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("str_type");
	let builder = PostgresQueryBuilder::new();

	// Create table with the string type
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(ColumnDef::new("str_col").column_type(str_type.clone()));

	let (sql, _values) = builder.build_create_table(&create_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with {:?}: {:?}",
		str_type,
		result.err()
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Temporal Type Matrix (EQ-12)
// =============================================================================

/// Test temporal type variants
#[rstest]
#[case::date(ColumnType::Date)]
#[case::time(ColumnType::Time)]
#[case::datetime(ColumnType::DateTime)]
#[case::timestamp(ColumnType::Timestamp)]
#[case::timestamp_tz(ColumnType::TimestampWithTimeZone)]
#[tokio::test]
async fn test_postgres_temporal_type_matrix(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] temporal_type: ColumnType,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("temporal_type");
	let builder = PostgresQueryBuilder::new();

	// Create table with the temporal type
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true),
		)
		.col(ColumnDef::new("temporal_col").column_type(temporal_type.clone()));

	let (sql, _values) = builder.build_create_table(&create_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with {:?}: {:?}",
		temporal_type,
		result.err()
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// Constraint Type Matrix (EQ-09)
// =============================================================================

/// Test PRIMARY KEY constraint on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_primary_key_constraint(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("pk_constraint");
	let constraint_name = unique_constraint_name("pk");
	let builder = PostgresQueryBuilder::new();

	// Create table with PRIMARY KEY constraint
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.constraint(TableConstraint::PrimaryKey {
			name: Some(constraint_name.clone().into_iden()),
			columns: vec!["id".to_string().into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&create_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with PRIMARY KEY: {:?}",
		result.err()
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test UNIQUE constraint on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_unique_constraint(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("uq_constraint");
	let constraint_name = unique_constraint_name("uq");
	let builder = PostgresQueryBuilder::new();

	// Create table with UNIQUE constraint
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
			ColumnDef::new("unique_col")
				.column_type(ColumnType::String(Some(100)))
				.not_null(false),
		)
		.constraint(TableConstraint::Unique {
			name: Some(constraint_name.clone().into_iden()),
			columns: vec!["unique_col".to_string().into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&create_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with UNIQUE: {:?}",
		result.err()
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test CHECK constraint on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_check_constraint(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("chk_constraint");
	let builder = PostgresQueryBuilder::new();

	// Create table with CHECK constraint on column
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
			ColumnDef::new("check_col")
				.column_type(ColumnType::Integer)
				.not_null(false)
				.check(Expr::col("check_col").gt(0)),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	assert!(
		result.is_ok(),
		"Failed to create table with CHECK: {:?}",
		result.err()
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}
