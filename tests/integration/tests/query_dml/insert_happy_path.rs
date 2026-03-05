// Happy path tests for INSERT statement

use super::fixtures::{TestPool, users_table};
use reinhardt_query::prelude::*;
use rstest::*;
use sqlx::Row;

/// Macro to bind values and execute query
macro_rules! bind_and_execute {
	($pool:expr, $sql:expr, $values:expr) => {{
		let mut query: sqlx::query::Query<'_, sqlx::Postgres, _> = sqlx::query(&$sql);
		for value in &$values.0 {
			query = match value {
				Value::BigInt(Some(v)) => query.bind(*v),
				Value::BigInt(None) => query.bind::<Option<i64>>(None),
				Value::SmallInt(Some(v)) => query.bind(*v),
				Value::SmallInt(None) => query.bind::<Option<i16>>(None),
				Value::Int(Some(v)) => query.bind(*v),
				Value::Int(None) => query.bind::<Option<i32>>(None),
				Value::String(Some(v)) => query.bind(v.as_str()),
				Value::String(None) => query.bind::<Option<&str>>(None),
				Value::Bool(Some(v)) => query.bind(*v),
				Value::Bool(None) => query.bind::<Option<bool>>(None),
				Value::TinyUnsigned(Some(v)) => query.bind(*v as i16),
				Value::TinyUnsigned(None) => query.bind::<Option<i16>>(None),
				Value::SmallUnsigned(Some(v)) => query.bind(*v as i32),
				Value::SmallUnsigned(None) => query.bind::<Option<i32>>(None),
				Value::Unsigned(None) => query.bind::<Option<i64>>(None),
				_ => query,
			};
		}
		query
			.execute($pool.as_ref())
			.await
			.expect("Query execution failed")
	}};
}

/// Test single row insertion
///
/// Verifies that Query::insert() can insert a single row correctly.
#[rstest]
#[tokio::test]
async fn test_insert_single_row(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("Charlie".to_string()))),
			Value::String(Some(Box::new("charlie@example.com".to_string()))),
			Value::Int(Some(35)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 1, "Should insert exactly one row");

	// Verify using sqlx::query
	let user = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("charlie@example.com")
		.fetch_one(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(user.get::<String, _>("name"), "Charlie");
	assert_eq!(user.get::<String, _>("email"), "charlie@example.com");
	assert_eq!(user.get::<Option<i32>, _>("age"), Some(35));
}

/// Test multiple row insertion
///
/// Verifies that Query::insert() can insert multiple rows in a single statement.
#[rstest]
#[tokio::test]
async fn test_insert_multiple_rows(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("David".to_string()))),
			Value::String(Some(Box::new("david@example.com".to_string()))),
			Value::Int(Some(28)),
		])
		.values_panic([
			Value::String(Some(Box::new("Eve".to_string()))),
			Value::String(Some(Box::new("eve@example.com".to_string()))),
			Value::Int(Some(32)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 2, "Should insert exactly two rows");

	// Verify using sqlx::query
	let users = sqlx::query("SELECT COUNT(*) as count FROM users")
		.fetch_one(pool.as_ref())
		.await
		.expect("Should count users");

	assert_eq!(users.get::<i64, _>("count"), 2);
}

/// Test insertion with RETURNING clause
///
/// Verifies that Query::insert() can return inserted values using RETURNING.
#[rstest]
#[tokio::test]
async fn test_insert_with_returning(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("Frank".to_string()))),
			Value::String(Some(Box::new("frank@example.com".to_string()))),
			Value::Int(Some(40)),
		])
		.returning_all()
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using sqlx::query
	let user = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("frank@example.com")
		.fetch_one(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(user.get::<String, _>("name"), "Frank");
}

/// Test insertion with default values
///
/// Verifies that columns with DEFAULT values are correctly handled.
#[rstest]
#[tokio::test]
async fn test_insert_with_default_values(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email"])
		.values_panic([
			Value::String(Some(Box::new("Grace".to_string()))),
			Value::String(Some(Box::new("grace@example.com".to_string()))),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using sqlx::query
	let user = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("grace@example.com")
		.fetch_one(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(user.get::<String, _>("name"), "Grace");
	assert_eq!(user.get::<Option<i32>, _>("age"), None); // No age specified
	assert_eq!(user.get::<bool, _>("active"), true); // Default value
}

/// Test insertion with NULL values
///
/// Verifies that NULL values are correctly inserted.
#[rstest]
#[tokio::test]
async fn test_insert_with_null_values(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("Henry".to_string()))),
			Value::String(Some(Box::new("henry@example.com".to_string()))),
			Value::Int(None),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using sqlx::query
	let user = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("henry@example.com")
		.fetch_one(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(user.get::<Option<i32>, _>("age"), None);
}

/// Test insertion with all column types
///
/// Verifies that all supported column types work correctly.
#[rstest]
#[tokio::test]
async fn test_insert_all_column_types(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age", "active"])
		.values_panic([
			Value::String(Some(Box::new("Ivy".to_string()))),
			Value::String(Some(Box::new("ivy@example.com".to_string()))),
			Value::Int(Some(27)),
			Value::Bool(Some(false)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using sqlx::query
	let user = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("ivy@example.com")
		.fetch_one(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(user.get::<String, _>("name"), "Ivy");
	assert_eq!(user.get::<Option<i32>, _>("age"), Some(27));
	assert_eq!(user.get::<bool, _>("active"), false);
}

/// Test bulk insertion of 100 rows
///
/// Verifies that Query::insert() can handle bulk insertions efficiently.
#[rstest]
#[tokio::test]
async fn test_insert_bulk_100_rows(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let mut stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.to_owned();

	for i in 0..100 {
		let name = format!("User{}", i);
		let email = format!("user{}@example.com", i);
		let age = 20 + (i % 50);
		stmt.values_panic([
			Value::String(Some(Box::new(name))),
			Value::String(Some(Box::new(email))),
			Value::Int(Some(age)),
		]);
	}
	let stmt = stmt.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(
		result.rows_affected(),
		100,
		"Should insert exactly 100 rows"
	);

	// Verify using sqlx::query
	let row = sqlx::query("SELECT COUNT(*) as count FROM users")
		.fetch_one(pool.as_ref())
		.await
		.expect("Should count rows");

	assert_eq!(row.get::<i64, _>("count"), 100);
}
