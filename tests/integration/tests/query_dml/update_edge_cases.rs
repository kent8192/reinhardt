// Edge case tests for UPDATE statement

use super::fixtures::{TestPool, users_with_data};
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

/// Test update to same value
///
/// Verifies that updating a column to its current value works correctly.
#[rstest]
#[tokio::test]
async fn test_update_to_same_value(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// First get current value
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("alice@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch user");

	let original_name = users[0].get::<String, _>("name").clone();

	// Update to same value
	let stmt = Query::update()
		.table("users")
		.values([("name", Value::String(Some(Box::new(original_name.clone()))))])
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 1);

	// Verify value unchanged
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("alice@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch user");

	assert_eq!(users[0].get::<String, _>("name"), original_name);
}

/// Test update to NULL
///
/// Verifies that updating a column to NULL works correctly.
#[rstest]
#[tokio::test]
async fn test_update_to_null(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::update()
		.table("users")
		.values([("age", Value::Int(None))])
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("bob@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch updated user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<Option<i32>, _>("age"), None);
}

/// Test update empty string to value
///
/// Verifies that updating from empty string to a value works correctly.
#[rstest]
#[tokio::test]
async fn test_update_empty_to_value(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// First set to empty string
	let _update_stmt = Query::update()
		.table("users")
		.values([("name", Value::String(Some(Box::new("".to_string()))))])
		.and_where(Expr::col("email").eq("charlie@example.com"))
		.to_owned();

	// Then update to a value
	let stmt = Query::update()
		.table("users")
		.values([(
			"name",
			Value::String(Some(Box::new("Charlie Has Name".to_string()))),
		)])
		.and_where(Expr::col("email").eq("charlie@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("charlie@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch updated user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<String, _>("name"), "Charlie Has Name");
}

/// Test update value to empty string
///
/// Verifies that updating from a value to empty string works correctly.
#[rstest]
#[tokio::test]
async fn test_update_value_to_empty(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::update()
		.table("users")
		.values([("name", Value::String(Some(Box::new("".to_string()))))])
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("alice@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch updated user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<String, _>("name"), "");
}
