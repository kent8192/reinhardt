// Combination tests for DELETE statement
//
// These tests verify DELETE combined with advanced SQL features:
// subquery in WHERE clause (IN subquery) and EXISTS subquery pattern.

use reinhardt_query::prelude::*;
use rstest::*;

/// Test DELETE with subquery
///
/// Verifies that DELETE can use a subquery in the WHERE clause via `in_subquery`.
#[rstest]
fn test_delete_with_subquery() {
	// Arrange
	let subquery = Query::select()
		.column("user_id")
		.from("inactive_logs")
		.and_where(Expr::col("last_login").lt("2024-01-01"))
		.to_owned();

	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::col("id").in_subquery(subquery))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"DELETE FROM "users" WHERE "id" IN (SELECT "user_id" FROM "inactive_logs" WHERE "last_login" < $1)"#
	);
	assert_eq!(values.len(), 1);
}

/// Test DELETE with EXISTS subquery pattern
///
/// Verifies that DELETE can use an EXISTS subquery to filter rows
/// based on conditions in a related table (simulating JOIN-based DELETE).
#[rstest]
fn test_delete_with_join() {
	// Arrange
	let exists_subquery = Query::select()
		.expr(Expr::val(1i32))
		.from("orders")
		.and_where(Expr::col(("orders", "user_id")).equals(("users", "id")))
		.and_where(Expr::col(("orders", "status")).eq("cancelled"))
		.to_owned();

	let stmt = Query::delete()
		.from_table("users")
		.and_where(Expr::exists(exists_subquery))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_delete(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"DELETE FROM "users" WHERE EXISTS (SELECT $1 FROM "orders" WHERE "orders"."user_id" = "users"."id" AND "orders"."status" = $2)"#
	);
	assert_eq!(values.len(), 2);
}
