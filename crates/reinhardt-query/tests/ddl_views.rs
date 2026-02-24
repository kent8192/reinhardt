//! View operations integration tests
//!
//! Tests for CREATE/DROP VIEW operations.

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;

mod common;
use common::{
	MySqlContainer, PgContainer, mysql_ddl, postgres_ddl, unique_table_name, unique_view_name,
};

/// Test basic CREATE VIEW on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_view_basic(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");
	let view_name = unique_view_name("user_emails");

	// Create table first
	let create_table_sql = format!(
		r#"CREATE TABLE "{}" (id SERIAL PRIMARY KEY, name VARCHAR(255), email VARCHAR(255))"#,
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Create view directly with SQL
	let create_view_sql = format!(
		r#"CREATE VIEW "{}" AS SELECT id, email FROM "{}""#,
		view_name, table_name
	);
	sqlx::query(&create_view_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create view");

	// Verify view exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.views WHERE table_name = $1")
			.bind(&view_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_view_sql = format!(r#"DROP VIEW IF EXISTS "{}""#, view_name);
	sqlx::query(&drop_view_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();

	let drop_table_sql = format!(r#"DROP TABLE IF EXISTS "{}""#, table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test DROP VIEW on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_view(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_users");
	let view_name = unique_view_name("user_emails");

	// Create table and view
	let create_table_sql = format!(
		r#"CREATE TABLE "{}" (id SERIAL PRIMARY KEY, name VARCHAR(255), email VARCHAR(255))"#,
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	let create_view_sql = format!(
		r#"CREATE VIEW "{}" AS SELECT id, email FROM "{}""#,
		view_name, table_name
	);
	sqlx::query(&create_view_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create view");

	// Verify view exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.views WHERE table_name = $1")
			.bind(&view_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(result, Some(1));

	// Build DROP VIEW statement
	let mut stmt = Query::drop_view();
	stmt.name(view_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_view(&stmt);

	// Execute DROP VIEW
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop view");

	// Verify view no longer exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.views WHERE table_name = $1")
			.bind(&view_name)
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

/// Test basic CREATE VIEW on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_create_view_basic(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_users");
	let view_name = unique_view_name("user_emails");

	// Create table first
	let create_table_sql = format!(
		"CREATE TABLE `{}` (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255), email VARCHAR(255))",
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Create view directly with SQL
	let create_view_sql = format!(
		"CREATE VIEW `{}` AS SELECT id, email FROM `{}`",
		view_name, table_name
	);
	sqlx::query(&create_view_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create view");

	// Verify view exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.views WHERE table_name = ?")
			.bind(&view_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();

	assert_eq!(result, Some(1));

	// Cleanup
	let drop_view_sql = format!("DROP VIEW IF EXISTS `{}`", view_name);
	sqlx::query(&drop_view_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();

	let drop_table_sql = format!("DROP TABLE IF EXISTS `{}`", table_name);
	sqlx::query(&drop_table_sql)
		.execute(pool.as_ref())
		.await
		.unwrap();
}

/// Test DROP VIEW on MySQL
#[rstest]
#[tokio::test]
async fn test_mysql_drop_view(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_users");
	let view_name = unique_view_name("user_emails");

	// Create table and view
	let create_table_sql = format!(
		"CREATE TABLE `{}` (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255), email VARCHAR(255))",
		table_name
	);
	sqlx::query(&create_table_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	let create_view_sql = format!(
		"CREATE VIEW `{}` AS SELECT id, email FROM `{}`",
		view_name, table_name
	);
	sqlx::query(&create_view_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create view");

	// Verify view exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.views WHERE table_name = ?")
			.bind(&view_name)
			.fetch_optional(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(result, Some(1));

	// Build DROP VIEW statement
	let mut stmt = Query::drop_view();
	stmt.name(view_name.clone());

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_drop_view(&stmt);

	// Execute DROP VIEW
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop view");

	// Verify view no longer exists
	let result: Option<i64> =
		sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.views WHERE table_name = ?")
			.bind(&view_name)
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
