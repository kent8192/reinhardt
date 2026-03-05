// Edge case tests for INSERT statement

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

/// Test insertion with empty string
///
/// Verifies that empty strings are correctly inserted (not treated as NULL).
#[rstest]
#[tokio::test]
async fn test_insert_empty_string(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("".to_string()))),
			Value::String(Some(Box::new("empty@example.com".to_string()))),
			Value::Int(Some(25)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("empty@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<String, _>("name"), "");
}

/// Test insertion with max length string
///
/// Verifies that strings at the maximum length limit are correctly handled.
#[rstest]
#[tokio::test]
async fn test_insert_max_length_string(#[future] users_table: TestPool) {
	let pool = users_table.await;

	// Create a string with exactly 255 characters (max length for name field)
	let max_name = "a".repeat(255);
	let max_email = format!("{}@example.com", "b".repeat(243)); // 255 total

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new(max_name.clone()))),
			Value::String(Some(Box::new(max_email.clone()))),
			Value::Int(Some(30)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 1);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind(max_email.as_str())
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<String, _>("name").len(), 255);
}

/// Test insertion with zero values
///
/// Verifies that zero values (0, 0.0) are correctly inserted (not treated as NULL).
#[rstest]
#[tokio::test]
async fn test_insert_zero_values(#[future] users_table: TestPool) {
	let pool = users_table.await;

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("Zero User".to_string()))),
			Value::String(Some(Box::new("zero@example.com".to_string()))),
			Value::Int(Some(0)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("zero@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<Option<i32>, _>("age"), Some(0));
}

/// Test insertion with negative values
///
/// Verifies that negative values are correctly inserted.
#[rstest]
#[tokio::test]
async fn test_insert_negative_values(#[future] users_table: TestPool) {
	let pool = users_table.await;

	// Note: Age doesn't make sense as negative, but we're testing the capability
	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new("Negative User".to_string()))),
			Value::String(Some(Box::new("negative@example.com".to_string()))),
			Value::Int(Some(-5)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("negative@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<Option<i32>, _>("age"), Some(-5));
}

/// Test insertion with Unicode characters
///
/// Verifies that Unicode strings (emoji, multi-byte characters) are correctly handled.
#[rstest]
#[tokio::test]
async fn test_insert_unicode_chars(#[future] users_table: TestPool) {
	let pool = users_table.await;

	// Test with emoji and multi-byte characters
	let unicode_name = "🎉 テスト User 👍";
	let unicode_email = "test+🚀@example.com";

	let stmt = Query::insert()
		.into_table("users")
		.columns(["name", "email", "age"])
		.values_panic([
			Value::String(Some(Box::new(unicode_name.to_string()))),
			Value::String(Some(Box::new(unicode_email.to_string()))),
			Value::Int(Some(25)),
		])
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_insert(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify using Model
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind(unicode_email)
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch inserted user");

	assert_eq!(users.len(), 1);
	assert_eq!(
		users[0].get::<String, _>("name"),
		String::from(unicode_name)
	);
}
