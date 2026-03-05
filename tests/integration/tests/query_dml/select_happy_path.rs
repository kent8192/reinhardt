// Happy path tests for SELECT statement

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

/// Test select all columns
///
/// Verifies that Query::select() can select all columns correctly.
#[rstest]
#[tokio::test]
async fn test_select_all_columns(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select().from("users").to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert!(rows.len() >= 3, "Should have at least 3 users");

	// Verify column access
	let row = &rows[0];
	let _id: i32 = row.get("id");
	let _name: String = row.get("name");
	let _email: String = row.get("email");
}

/// Test select specific columns
///
/// Verifies that Query::select() can select specific columns correctly.
#[rstest]
#[tokio::test]
async fn test_select_specific_columns(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select()
		.column("name")
		.column("email")
		.from("users")
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert!(rows.len() >= 3, "Should have at least 3 users");
}

/// Test select with WHERE clause
///
/// Verifies that Query::select() can filter rows using WHERE clause.
#[rstest]
#[tokio::test]
async fn test_select_with_where(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select()
		.from("users")
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert_eq!(rows.len(), 1, "Should find exactly one user");
}

/// Test select with ORDER BY
///
/// Verifies that Query::select() can sort results using ORDER BY.
#[rstest]
#[tokio::test]
async fn test_select_with_order_by(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select()
		.from("users")
		.order_by("age", Order::Desc)
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert!(rows.len() >= 3, "Should have at least 3 users");

	// Verify ordering (Charlie has age 35, should be first)
	let first_name: String = rows[0].get("name");
	assert_eq!(first_name, "Charlie", "Charlie (age 35) should be first");
}

/// Test select with LIMIT
///
/// Verifies that Query::select() can limit results using LIMIT.
#[rstest]
#[tokio::test]
async fn test_select_with_limit(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select().from("users").limit(2).to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert_eq!(rows.len(), 2, "Should return exactly 2 rows");
}

/// Test select with OFFSET
///
/// Verifies that Query::select() can skip rows using OFFSET.
#[rstest]
#[tokio::test]
async fn test_select_with_offset(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::select().from("users").limit(2).offset(1).to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	let rows = bind_and_execute_query!(pool, sql, values);
	assert_eq!(rows.len(), 2, "Should return exactly 2 rows");
}
