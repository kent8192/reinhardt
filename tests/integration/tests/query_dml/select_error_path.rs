// Error path tests for SELECT statement

use super::fixtures::{TestPool, users_with_data};
use reinhardt_query::prelude::*;
use rstest::*;

/// Test select invalid column
///
/// Verifies that selecting a non-existent column results in a database error.
#[rstest]
#[tokio::test]
async fn test_select_invalid_column(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Try to select a non-existent column
	let stmt = Query::select()
		.column("invalid_column")
		.from("users")
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, _values) = builder.build_select(&stmt);

	let result = sqlx::query(&sql).fetch_all(pool.as_ref()).await;

	assert!(result.is_err(), "Should fail with invalid column error");
	let err = result.unwrap_err();
	let err_msg = err.to_string();
	assert!(
		err_msg.contains("column") || err_msg.contains("does not exist"),
		"Error should mention invalid column: {}",
		err_msg
	);
}

/// Test select invalid table
///
/// Verifies that selecting from a non-existent table results in a database error.
#[rstest]
#[tokio::test]
async fn test_select_invalid_table(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Try to select from a non-existent table
	let stmt = Query::select().from("invalid_table").to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, _values) = builder.build_select(&stmt);

	let result = sqlx::query(&sql).fetch_all(pool.as_ref()).await;

	assert!(result.is_err(), "Should fail with invalid table error");
	let err = result.unwrap_err();
	let err_msg = err.to_string();
	assert!(
		err_msg.contains("table")
			|| err_msg.contains("does not exist")
			|| err_msg.contains("relation"),
		"Error should mention invalid table: {}",
		err_msg
	);
}
