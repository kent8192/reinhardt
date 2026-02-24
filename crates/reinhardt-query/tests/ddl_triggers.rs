//! Trigger operations integration tests
//!
//! Tests for CREATE/DROP TRIGGER operations including:
//! - Basic trigger creation (BEFORE/AFTER INSERT/UPDATE/DELETE)
//! - Trigger timing and event combinations
//! - DROP TRIGGER with/without IF EXISTS
//! - Error cases (non-existent table)
//! - State transitions

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

mod common;
use common::{
	MySqlContainer, PgContainer, mysql_ddl, mysql_ident, pg_ident, postgres_ddl,
	unique_function_name, unique_table_name, unique_trigger_name,
};

// =============================================================================
// PostgreSQL Trigger Tests
// =============================================================================

/// HP-14: Test CREATE TRIGGER (BEFORE INSERT) on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_trigger_before_insert(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_before_insert");
	let function_name = unique_function_name("fn_audit");

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255),
			created_at TIMESTAMP DEFAULT NOW()
		)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create a trigger function (required for PostgreSQL triggers)
	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS TRIGGER AS $$
		BEGIN
			RETURN NEW;
		END;
		$$ LANGUAGE plpgsql"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Build CREATE TRIGGER statement
	let mut stmt = Query::create_trigger();
	stmt.name(trigger_name.clone())
		.timing(TriggerTiming::Before)
		.event(TriggerEvent::Insert)
		.on_table(table_name.clone())
		.for_each(TriggerScope::Row)
		.execute_function(&function_name);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_trigger(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE t.tgname = $1 AND c.relname = $2",
	)
	.bind(&trigger_name)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TRIGGER IF EXISTS {} ON {}"#,
		pg_ident(&trigger_name),
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// HP-15: Test CREATE TRIGGER (AFTER UPDATE) on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_trigger_after_update(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_after_update");
	let function_name = unique_function_name("fn_audit");

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255),
			updated_at TIMESTAMP
		)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create a trigger function
	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS TRIGGER AS $$
		BEGIN
			NEW.updated_at = NOW();
			RETURN NEW;
		END;
		$$ LANGUAGE plpgsql"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Build CREATE TRIGGER statement
	let mut stmt = Query::create_trigger();
	stmt.name(trigger_name.clone())
		.timing(TriggerTiming::After)
		.event(TriggerEvent::Update { columns: None })
		.on_table(table_name.clone())
		.for_each(TriggerScope::Row)
		.execute_function(&function_name);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_trigger(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE t.tgname = $1 AND c.relname = $2",
	)
	.bind(&trigger_name)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TRIGGER IF EXISTS {} ON {}"#,
		pg_ident(&trigger_name),
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test CREATE TRIGGER (AFTER DELETE) on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_create_trigger_after_delete(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_after_delete");
	let function_name = unique_function_name("fn_audit");

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (
			id SERIAL PRIMARY KEY,
			name VARCHAR(255)
		)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create a trigger function
	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS TRIGGER AS $$
		BEGIN
			RETURN OLD;
		END;
		$$ LANGUAGE plpgsql"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Build CREATE TRIGGER statement
	let mut stmt = Query::create_trigger();
	stmt.name(trigger_name.clone())
		.timing(TriggerTiming::After)
		.event(TriggerEvent::Delete)
		.on_table(table_name.clone())
		.for_each(TriggerScope::Row)
		.execute_function(&function_name);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_trigger(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE t.tgname = $1 AND c.relname = $2",
	)
	.bind(&trigger_name)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TRIGGER IF EXISTS {} ON {}"#,
		pg_ident(&trigger_name),
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test DROP TRIGGER on PostgreSQL
#[rstest]
#[tokio::test]
async fn test_postgres_drop_trigger(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_drop");
	let function_name = unique_function_name("fn_audit");

	// Create table and function
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS TRIGGER AS $$
		BEGIN RETURN NEW; END; $$ LANGUAGE plpgsql"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Create trigger using raw SQL
	sqlx::query(&format!(
		r#"CREATE TRIGGER {} BEFORE INSERT ON {} FOR EACH ROW EXECUTE FUNCTION {}()"#,
		pg_ident(&trigger_name),
		pg_ident(&table_name),
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE t.tgname = $1 AND c.relname = $2",
	)
	.bind(&trigger_name)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 1);

	// Build DROP TRIGGER statement
	let mut stmt = Query::drop_trigger();
	stmt.name(trigger_name.clone()).on_table(table_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_trigger(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop trigger");

	// Verify trigger no longer exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE t.tgname = $1 AND c.relname = $2",
	)
	.bind(&trigger_name)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 0);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test DROP TRIGGER IF EXISTS on non-existent trigger
#[rstest]
#[tokio::test]
async fn test_postgres_drop_trigger_if_exists_nonexistent(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_nonexistent");

	// Create table (trigger doesn't exist)
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Build DROP TRIGGER IF EXISTS statement
	let mut stmt = Query::drop_trigger();
	stmt.name(trigger_name.clone())
		.on_table(table_name.clone())
		.if_exists();

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_trigger(&stmt);

	// Should succeed (no-op)
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;
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
// Error Path Tests
// =============================================================================

/// EP-09: Test CREATE TRIGGER on non-existent table fails
#[rstest]
#[tokio::test]
async fn test_postgres_create_trigger_nonexistent_table_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("nonexistent_table");
	let trigger_name = unique_trigger_name("trg_bad");
	let function_name = unique_function_name("fn_audit");

	// Create function only (no table)
	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS TRIGGER AS $$
		BEGIN RETURN NEW; END; $$ LANGUAGE plpgsql"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Build CREATE TRIGGER statement for non-existent table
	let mut stmt = Query::create_trigger();
	stmt.name(trigger_name.clone())
		.timing(TriggerTiming::Before)
		.event(TriggerEvent::Insert)
		.on_table(table_name.clone())
		.for_each(TriggerScope::Row)
		.execute_function(&function_name);

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_create_trigger(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail - table doesn't exist
	assert!(result.is_err());

	// Cleanup
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test DROP TRIGGER without IF EXISTS on non-existent trigger fails
#[rstest]
#[tokio::test]
async fn test_postgres_drop_trigger_nonexistent_fails(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_nonexistent");

	// Create table only (no trigger)
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Build DROP TRIGGER statement (no IF EXISTS)
	let mut stmt = Query::drop_trigger();
	stmt.name(trigger_name.clone()).on_table(table_name.clone());

	let builder = PostgresQueryBuilder::new();
	let (sql, _values) = builder.build_drop_trigger(&stmt);
	let result = sqlx::query(&sql).execute(pool.as_ref()).await;

	// Should fail - trigger doesn't exist
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
// MySQL Trigger Tests
//
// NOTE: MySQL CREATE/DROP TRIGGER commands are not supported in the prepared
// statement protocol. These tests are disabled until we can find a way to
// bypass the prepared statement limitation or implement them as unit tests.
// =============================================================================

/// Test CREATE TRIGGER (BEFORE INSERT) on MySQL
#[rstest]
#[tokio::test]
#[ignore = "MySQL CREATE TRIGGER not supported in prepared statement protocol"]
async fn test_mysql_create_trigger_before_insert(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_insert");

	// Create table
	sqlx::query(&format!(
		"CREATE TABLE {} (
			id INT AUTO_INCREMENT PRIMARY KEY,
			name VARCHAR(255),
			created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
		)",
		mysql_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Build CREATE TRIGGER statement with body
	let mut stmt = Query::create_trigger();
	stmt.name(trigger_name.clone())
		.timing(TriggerTiming::Before)
		.event(TriggerEvent::Insert)
		.on_table(table_name.clone())
		.for_each(TriggerScope::Row)
		.body(TriggerBody::single("SET NEW.created_at = NOW()"));

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_create_trigger(&stmt);
	// MySQL CREATE TRIGGER cannot use prepared statement protocol
	// Execute the generated SQL directly via format! to bypass prepared statement limitations
	sqlx::query(&format!("{}", sql))
		.execute(pool.as_ref())
		.await
		.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.triggers
		 WHERE trigger_name = ? AND event_object_table = ?",
	)
	.bind(&trigger_name)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		"DROP TRIGGER IF EXISTS {}",
		mysql_ident(&trigger_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		"DROP TABLE IF EXISTS {}",
		mysql_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

/// Test DROP TRIGGER on MySQL
#[rstest]
#[tokio::test]
#[ignore = "MySQL DROP TRIGGER not supported in prepared statement protocol"]
async fn test_mysql_drop_trigger(
	#[future] mysql_ddl: (MySqlContainer, Arc<sqlx::MySqlPool>, u16, String),
) {
	let (_container, pool, _port, _url) = mysql_ddl.await;
	let table_name = unique_table_name("test_trig");
	let trigger_name = unique_trigger_name("trg_drop");

	// Create table
	sqlx::query(&format!(
		"CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(255))",
		mysql_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create trigger using raw SQL
	sqlx::query(&format!(
		"CREATE TRIGGER {} BEFORE INSERT ON {} FOR EACH ROW SET NEW.name = UPPER(NEW.name)",
		mysql_ident(&trigger_name),
		mysql_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_name = ?",
	)
	.bind(&trigger_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 1);

	// Build DROP TRIGGER statement
	let mut stmt = Query::drop_trigger();
	stmt.name(trigger_name.clone());

	let builder = MySqlQueryBuilder::new();
	let (sql, _values) = builder.build_drop_trigger(&stmt);
	// MySQL DROP TRIGGER cannot use prepared statement protocol
	// Execute the generated SQL directly via format! to bypass prepared statement limitations
	sqlx::query(&format!("{}", sql))
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop trigger");

	// Verify trigger no longer exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_name = ?",
	)
	.bind(&trigger_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 0);

	// Cleanup
	sqlx::query(&format!(
		"DROP TABLE IF EXISTS {}",
		mysql_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// Combination Tests - CB-08: Trigger timing × event matrix
// =============================================================================

/// Test trigger timing and event combinations on PostgreSQL
#[rstest]
#[case::before_insert(TriggerTiming::Before, TriggerEvent::Insert)]
#[case::before_update(TriggerTiming::Before, TriggerEvent::Update { columns: None })]
#[case::before_delete(TriggerTiming::Before, TriggerEvent::Delete)]
#[case::after_insert(TriggerTiming::After, TriggerEvent::Insert)]
#[case::after_update(TriggerTiming::After, TriggerEvent::Update { columns: None })]
#[case::after_delete(TriggerTiming::After, TriggerEvent::Delete)]
#[tokio::test]
async fn test_postgres_trigger_timing_event_combinations(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
	#[case] timing: TriggerTiming,
	#[case] event: TriggerEvent,
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_combo");
	let trigger_name = unique_trigger_name("trg_combo");
	let function_name = unique_function_name("fn_combo");

	let builder = PostgresQueryBuilder::new();

	// Create table
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY, name VARCHAR(255))"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	// Create a trigger function
	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS TRIGGER AS $$
		BEGIN
			IF TG_OP = 'DELETE' THEN
				RETURN OLD;
			ELSE
				RETURN NEW;
			END IF;
		END;
		$$ LANGUAGE plpgsql"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Build CREATE TRIGGER statement
	let mut stmt = Query::create_trigger();
	stmt.name(trigger_name.clone())
		.timing(timing)
		.event(event)
		.on_table(table_name.clone())
		.for_each(TriggerScope::Row)
		.execute_function(&function_name);

	let (sql, _values) = builder.build_create_trigger(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create trigger");

	// Verify trigger exists
	let trigger_exists: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE t.tgname = $1 AND c.relname = $2",
	)
	.bind(&trigger_name)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_exists, 1);

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TRIGGER IF EXISTS {} ON {}"#,
		pg_ident(&trigger_name),
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}

// =============================================================================
// State Transition Tests
// =============================================================================

/// State transition: CREATE TABLE → CREATE TRIGGER → DROP TRIGGER
#[rstest]
#[tokio::test]
async fn test_postgres_trigger_state_transition(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let table_name = unique_table_name("test_state");
	let trigger_name = unique_trigger_name("trg_state");
	let function_name = unique_function_name("fn_state");

	let builder = PostgresQueryBuilder::new();

	// State 1: Create table only
	sqlx::query(&format!(
		r#"CREATE TABLE {} (id SERIAL PRIMARY KEY)"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create table");

	sqlx::query(&format!(
		r#"CREATE FUNCTION {}() RETURNS TRIGGER AS $$
		BEGIN RETURN NEW; END; $$ LANGUAGE plpgsql"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to create function");

	// Verify: No trigger exists
	let trigger_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE c.relname = $1 AND t.tgname = $2",
	)
	.bind(&table_name)
	.bind(&trigger_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_count, 0, "State 1: No trigger should exist");

	// State 2: Create trigger
	let mut create_stmt = Query::create_trigger();
	create_stmt
		.name(trigger_name.clone())
		.timing(TriggerTiming::Before)
		.event(TriggerEvent::Insert)
		.on_table(table_name.clone())
		.for_each(TriggerScope::Row)
		.execute_function(&function_name);

	let (sql, _values) = builder.build_create_trigger(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create trigger");

	let trigger_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE c.relname = $1 AND t.tgname = $2",
	)
	.bind(&table_name)
	.bind(&trigger_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_count, 1, "State 2: Trigger should exist");

	// State 3: Drop trigger
	let mut drop_stmt = Query::drop_trigger();
	drop_stmt
		.name(trigger_name.clone())
		.on_table(table_name.clone());

	let (sql, _values) = builder.build_drop_trigger(&drop_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to drop trigger");

	let trigger_count: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pg_trigger t
		 JOIN pg_class c ON t.tgrelid = c.oid
		 WHERE c.relname = $1 AND t.tgname = $2",
	)
	.bind(&table_name)
	.bind(&trigger_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(trigger_count, 0, "State 3: Trigger should be dropped");

	// Cleanup
	sqlx::query(&format!(
		r#"DROP TABLE IF EXISTS {}"#,
		pg_ident(&table_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
	sqlx::query(&format!(
		r#"DROP FUNCTION IF EXISTS {}()"#,
		pg_ident(&function_name)
	))
	.execute(pool.as_ref())
	.await
	.unwrap();
}
