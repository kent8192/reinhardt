// Error path tests for DELETE statement

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

/// Test delete nonexistent row
///
/// Verifies that deleting a nonexistent row affects 0 rows.
#[rstest]
#[tokio::test]
async fn test_delete_nonexistent_row(#[future] users_with_data: (TestPool, Vec<i32>)) {
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

/// Test foreign key constraint violation
///
/// Verifies that deleting a row referenced by foreign key results in an error without CASCADE.
/// Note: This test creates an orders table with FK to test constraint violation.
#[rstest]
#[tokio::test]
async fn test_delete_fk_violation(#[future] users_with_data: (TestPool, Vec<i32>)) {
	let (pool, _ids) = users_with_data.await;

	// Create orders table with FK restrict (no cascade)
	use reinhardt_query::prelude::{
		ColumnDef, ForeignKeyAction, PostgresQueryBuilder as PgBuilder, Query as Q,
		QueryStatementBuilder,
	};

	let mut create_table = Q::create_table();
	create_table
		.table("orders_no_cascade")
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
			Some(ForeignKeyAction::Restrict),
			Some(ForeignKeyAction::Restrict),
		);

	let create_sql = create_table.to_string(PgBuilder::new());
	sqlx::query(&create_sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create orders table");

	// Insert an order for Bob
	let user_id = _ids[1];
	sqlx::query(
		"INSERT INTO orders_no_cascade (user_id, total_amount, status) VALUES ($1, $2, $3)",
	)
	.bind(user_id)
	.bind(5000i64)
	.bind("pending")
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert order");

	// Try to delete Bob (should fail due to FK constraint)
	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::col("email").eq("bob@example.com"))
		.to_owned();

	let builder = PostgresQueryBuilder;
	let (sql, _values) = builder.build_delete(&stmt);

	let result = sqlx::query(&sql)
		.bind("bob@example.com")
		.execute(pool.as_ref())
		.await;

	assert!(
		result.is_err(),
		"Should fail with foreign key constraint violation"
	);
	let err = result.unwrap_err();
	let err_msg = err.to_string();
	assert!(
		err_msg.contains("foreign key")
			|| err_msg.contains("violates")
			|| err_msg.contains("still referenced"),
		"Error should mention foreign key constraint: {}",
		err_msg
	);

	// Cleanup
	sqlx::query("DROP TABLE orders_no_cascade")
		.execute(pool.as_ref())
		.await
		.ok();
}
