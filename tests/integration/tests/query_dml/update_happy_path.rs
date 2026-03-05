// Happy path tests for UPDATE statement

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

/// Test single column update
///
/// Verifies that Query::update() can update a single column correctly.
#[rstest]
#[tokio::test]
async fn test_update_single_column(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::update()
		.table("users")
		.values([(
			"name",
			Value::String(Some(Box::new("Alice Updated".to_string()))),
		)])
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using sqlx::query
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("alice@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch updated user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<String, _>("name"), "Alice Updated");
}

/// Test multiple columns update
///
/// Verifies that Query::update() can update multiple columns in a single statement.
#[rstest]
#[tokio::test]
async fn test_update_multiple_columns(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::update()
		.table("users")
		.values([
			(
				"name",
				Value::String(Some(Box::new("Bob Updated".to_string()))),
			),
			("age", Value::Int(Some(26))),
		])
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using sqlx::query
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("bob@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch updated user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<String, _>("name"), "Bob Updated");
	assert_eq!(users[0].get::<Option<i32>, _>("age"), Some(26));
}

/// Test update with RETURNING clause
///
/// Verifies that Query::update() can return updated values using RETURNING.
#[rstest]
#[tokio::test]
async fn test_update_with_returning(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::update()
		.table("users")
		.values([(
			"name",
			Value::String(Some(Box::new("Charlie Updated".to_string()))),
		)])
		.and_where(Expr::col("email").eq("charlie@example.com"))
		.returning_all()
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using sqlx::query
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("charlie@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch updated user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<String, _>("name"), "Charlie Updated");
}

/// Test update multiple rows with condition
///
/// Verifies that Query::update() can update multiple rows matching a condition.
#[rstest]
#[tokio::test]
async fn test_update_multiple_rows_condition(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Update all users with age < 30
	let stmt = Query::update()
		.table("users")
		.values([("active", Value::Bool(Some(false)))])
		.and_where(Expr::col("age").lt(30))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert!(result.rows_affected() > 0);

	// Verify using sqlx::query
	let users = sqlx::query("SELECT * FROM users")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch all users");

	let inactive_count = users.iter().filter(|u| !u.get::<bool, _>("active")).count();
	assert!(inactive_count > 0, "Should have inactive users");
}
