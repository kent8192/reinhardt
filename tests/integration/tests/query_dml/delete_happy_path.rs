// Happy path tests for DELETE statement

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

/// Test single row deletion
///
/// Verifies that Query::delete() can delete a single row correctly.
#[rstest]
#[tokio::test]
async fn test_delete_single_row(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Verify initial count
	let users = sqlx::query("SELECT * FROM users")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch all users");
	let initial_count = users.len();

	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 1, "Should delete exactly one row");

	// Verify deletion
	let users = sqlx::query("SELECT * FROM users")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch all users");
	assert_eq!(users.len(), initial_count - 1);
}

/// Test multiple rows deletion
///
/// Verifies that Query::delete() can delete multiple rows in a single statement.
#[rstest]
#[tokio::test]
async fn test_delete_multiple_rows(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Delete users with age < 30
	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::col("age").lt(30))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert!(result.rows_affected() > 0, "Should delete at least one row");

	// Verify deletions
	let users = sqlx::query("SELECT * FROM users")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch all users");

	let young_users = users.iter().filter(|u| {
		u.get::<Option<i32>, _>("age")
			.map(|a| a < 30)
			.unwrap_or(false)
	});
	assert_eq!(young_users.count(), 0, "Should have no users with age < 30");
}

/// Test deletion with RETURNING clause
///
/// Verifies that Query::delete() can return deleted values using RETURNING.
#[rstest]
#[tokio::test]
async fn test_delete_with_returning(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::col("email").eq("bob@example.com"))
		.returning_all()
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify deletion
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("bob@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch users");

	assert_eq!(users.len(), 0, "User should be deleted");
}

/// Test delete all rows
///
/// Verifies that Query::delete() can delete all rows from a table.
#[rstest]
#[tokio::test]
async fn test_delete_all_rows(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::delete().from_table("users").to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert!(result.rows_affected() > 0, "Should delete all rows");

	// Verify all rows deleted
	let users = sqlx::query("SELECT * FROM users")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch all users");

	assert_eq!(users.len(), 0, "Should have no users left");
}
