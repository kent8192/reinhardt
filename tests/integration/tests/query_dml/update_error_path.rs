// Error path tests for UPDATE statement

use super::fixtures::{TestPool, users_with_data};
use reinhardt_query::prelude::*;
use rstest::*;

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

/// Test duplicate key violation
///
/// Verifies that updating to a duplicate unique key results in a database error.
#[rstest]
#[tokio::test]
async fn test_update_duplicate_key_violation(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Try to update Bob's email to Alice's email (duplicate)
	let stmt = Query::update()
		.table("users")
		.values([(
			"email",
			Value::String(Some(Box::new("alice@example.com".to_string()))),
		)])
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, _values) = builder.build_update(&stmt);

	let result = sqlx::query(&sql)
		.bind("alice@example.com")
		.bind("bob@example.com")
		.execute(pool.as_ref())
		.await;

	assert!(result.is_err(), "Should fail with duplicate key violation");
	let err = result.unwrap_err();
	let err_msg = err.to_string();
	assert!(
		err_msg.contains("duplicate key") || err_msg.contains("unique"),
		"Error should mention duplicate key or unique constraint: {}",
		err_msg
	);
}

/// Test NOT NULL constraint violation
///
/// Verifies that updating a NOT NULL column to NULL results in a database error.
#[rstest]
#[tokio::test]
async fn test_update_null_not_null_violation(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Try to update name to NULL (NOT NULL column)
	let stmt = Query::update()
		.table("users")
		.values([("name", Value::String(None))])
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, _values) = builder.build_update(&stmt);

	// NULL values are inlined directly in SQL (not parameterized),
	// so only the WHERE clause value needs to be bound.
	let result = sqlx::query(&sql)
		.bind("alice@example.com")
		.execute(pool.as_ref())
		.await;

	assert!(result.is_err(), "Should fail with NOT NULL violation");
	let err = result.unwrap_err();
	let err_msg = err.to_string();
	assert!(
		err_msg.contains("null") || err_msg.contains("NOT NULL"),
		"Error should mention NULL constraint: {}",
		err_msg
	);
}

/// Test foreign key constraint violation
///
/// Verifies that updating with invalid foreign key results in a database error.
/// Note: This test is simplified as we don't have FK relationships in the users table.
#[rstest]
#[tokio::test]
async fn test_update_fk_violation(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (_pool, _ids) = users_with_data.await;

	// This test verifies SQL structure since users table doesn't have FK
	let stmt = Query::update()
		.table("users")
		.values([(
			"name",
			Value::String(Some(Box::new("Test User".to_string()))),
		)])
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, _values) = builder.build_update(&stmt);

	// Verify SQL structure
	assert!(sql.contains("UPDATE"), "SQL should be an UPDATE statement");
	assert!(sql.contains("users"), "SQL should specify users table");
	assert!(sql.contains("SET"), "SQL should include SET clause");
	assert!(sql.contains("WHERE"), "SQL should include WHERE clause");
}

/// Test CHECK constraint violation
///
/// Verifies that CHECK constraints are enforced. Since our test tables don't have CHECK constraints, we verify SQL structure instead.
#[rstest]
#[tokio::test]
async fn test_update_check_constraint_violation(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Update with valid data
	let stmt = Query::update()
		.table("users")
		.values([("age", Value::Int(Some(25)))])
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	// Verify SQL structure
	assert!(sql.contains("UPDATE"), "SQL should be an UPDATE statement");
	assert!(sql.contains("users"), "SQL should specify users table");

	// Execute should succeed (no CHECK constraint in our schema)
	bind_and_execute!(pool, sql, values);
}

/// Test update nonexistent row
///
/// Verifies that updating a nonexistent row affects 0 rows.
#[rstest]
#[tokio::test]
async fn test_update_nonexistent_row(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::update()
		.table("users")
		.values([(
			"name",
			Value::String(Some(Box::new("Ghost User".to_string()))),
		)])
		.and_where(Expr::col("email").eq("nonexistent@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 0, "Should affect 0 rows");
}
