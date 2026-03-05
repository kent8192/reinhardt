// Edge case tests for DELETE statement

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

/// Test delete zero rows
///
/// Verifies that deleting with a non-matching condition affects 0 rows.
#[rstest]
#[tokio::test]
async fn test_delete_zero_rows(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::col("email").eq("nonexistent@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 0, "Should affect 0 rows");

	// Verify no users deleted
	let users = sqlx::query("SELECT * FROM users")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch all users");

	assert!(users.len() >= 3, "Should still have all original users");
}

/// Test delete with cascade
///
/// Verifies that DELETE with CASCADE works correctly.
/// Note: This test creates an orders table with FK cascade to test cascade deletion.
#[rstest]
#[tokio::test]
async fn test_delete_with_cascade(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Create orders table with FK cascade
	use reinhardt_query::prelude::{
		ColumnDef, ForeignKeyAction, PostgresQueryBuilder as PgBuilder, Query as Q,
		QueryStatementBuilder,
	};

	let mut create_table = Q::create_table();
	create_table
		.table("orders")
		.if_not_exists()
		.col(
			ColumnDef::new("id")
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new("user_id").integer().not_null(true))
		.col(ColumnDef::new("total_amount").big_integer().not_null(true))
		.col(ColumnDef::new("status").string_len(50).not_null(true))
		.foreign_key(
			vec!["user_id"],
			"users",
			vec!["id"],
			Some(ForeignKeyAction::Cascade),
			Some(ForeignKeyAction::Cascade),
		);

	let create_sql = create_table.to_string(PgBuilder::new());
	sqlx::query(&create_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create orders table");

	// Insert an order for Alice
	let user_id = _ids[0];
	sqlx::query("INSERT INTO orders (user_id, total_amount, status) VALUES ($1, $2, $3)")
		.bind(user_id)
		.bind(10000i64)
		.bind("pending")
		.execute(pool.as_ref())
		.await
		.expect("Failed to insert order");

	// Delete Alice (should cascade delete the order)
	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::col("email").eq("alice@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	let result = bind_and_execute!(pool, sql, values);
	assert_eq!(result.rows_affected(), 1, "Should delete Alice");

	// Verify cascade deletion
	let order_count: i64 = sqlx::query("SELECT COUNT(*) FROM orders")
		.fetch_one(pool.as_ref())
		.await
		.expect("Failed to count orders")
		.get("count");

	assert_eq!(order_count, 0, "All orders should be cascade deleted");
}

/// Test soft delete pattern
///
/// Verifies that soft delete pattern (setting active=false instead of deleting) works correctly.
#[rstest]
#[tokio::test]
async fn test_delete_soft_delete_pattern(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Soft delete: set active=false instead of actual deletion
	let stmt = Query::update()
		.table("users")
		.values([(("active", Value::Bool(Some(false))))])
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	bind_and_execute!(pool, sql, values);

	// Verify soft deletion
	let users = sqlx::query("SELECT * FROM users WHERE email = $1")
		.bind("bob@example.com")
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch user");

	assert_eq!(users.len(), 1);
	assert_eq!(users[0].get::<bool, _>("active"), false);

	// Verify that filtering by active=true excludes soft deleted users
	let active_users = sqlx::query("SELECT * FROM users WHERE active = $1")
		.bind(true)
		.fetch_all(pool.as_ref())
		.await
		.expect("Should fetch active users");

	let bob_is_active = active_users
		.iter()
		.any(|u| u.get::<String, _>("email") == "bob@example.com");
	assert!(!bob_is_active, "Bob should not be in active users");
}
