// Edge case tests for SELECT statement

use super::fixtures::{TestPool, users_with_data};
use reinhardt_query::prelude::*;
use rstest::*;
use sqlx::Row;

/// Macro to bind values and execute query
macro_rules! bind_and_execute_query {
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
			.fetch_all($pool.as_ref())
			.await
			.expect("Query execution failed")
	}};
}

/// Test select empty result
///
/// Verifies that SELECT with non-matching condition returns empty result set.
#[rstest]
#[tokio::test]
async fn test_select_empty_result(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select()
		.from("users")
		.and_where(Expr::col("email").eq("nonexistent@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert_eq!(rows.len(), 0, "Should return empty result set");
}

/// Test select with NULL values
///
/// Verifies that SELECT correctly handles NULL values in results.
#[rstest]
#[tokio::test]
async fn test_select_with_null_values(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// First, update one user to have NULL age
	let update_stmt = Query::update()
		.table("users")
		.values([("age", Value::Int(None))])
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (update_sql, update_values) = builder.build_update(&update_stmt);

	let mut query: sqlx::query::Query<'_, sqlx::Postgres, _> = sqlx::query(&update_sql);
	for value in &update_values.0 {
		query = match value {
			Value::Int(None) => query.bind::<Option<i32>>(None),
			Value::String(Some(v)) => query.bind(v.as_str()),
			_ => query,
		};
	}
	query.execute(pool.as_ref()).await.expect("Update failed");

	// Now select and verify NULL handling
	let stmt = Query::select()
		.from("users")
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert_eq!(rows.len(), 1);

	let age: Option<i32> = rows[0].get("age");
	assert_eq!(age, None, "Age should be NULL");
}

/// Test select with LIMIT zero
///
/// Verifies that SELECT with LIMIT 0 returns empty result set.
#[rstest]
#[tokio::test]
async fn test_select_limit_zero(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select().from("users").limit(0).to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert_eq!(rows.len(), 0, "Should return empty result with LIMIT 0");
}

/// Test select with OFFSET beyond rows
///
/// Verifies that SELECT with OFFSET larger than result set returns empty result set.
#[rstest]
#[tokio::test]
async fn test_select_offset_beyond_rows(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select().from("users").offset(1000).to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert_eq!(
		rows.len(),
		0,
		"Should return empty result with OFFSET beyond rows"
	);
}
