// Combination tests for UPDATE statement
//
// These tests verify UPDATE combined with advanced SQL features:
// WHERE IN subquery (simulating JOIN), CASE expression in WHERE,
// and subquery-based filtering.

use reinhardt_query::prelude::*;
use rstest::*;

/// Test UPDATE with subquery in WHERE (simulating JOIN)
///
/// Verifies that UPDATE can use `in_subquery` to filter rows based on
/// a related table, simulating JOIN-based UPDATE behavior.
#[rstest]
fn test_update_with_join() {
	// Arrange
	let subquery = Query::select()
		.column("user_id")
		.from("orders")
		.and_where(Expr::col("status").eq("vip"))
		.to_owned();

	let stmt = Query::update()
		.table("users")
		.values([(
			"name",
			Value::String(Some(Box::new("VIP User".to_string()))),
		)])
		.and_where(Expr::col("id").in_subquery(subquery))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"UPDATE "users" SET "name" = $1 WHERE "id" IN (SELECT "user_id" FROM "orders" WHERE "status" = $2)"#
	);
	assert_eq!(values.len(), 2);
}

/// Test UPDATE with CASE expression in WHERE clause
///
/// Verifies that UPDATE can use CASE expression for conditional filtering.
#[rstest]
fn test_update_with_case_expression() {
	// Arrange
	let case_expr = Expr::case()
		.when(Expr::col("age").gte(30i32), true)
		.else_result(false);

	let stmt = Query::update()
		.table("users")
		.values([("active", Value::Bool(Some(false)))])
		.and_where(case_expr)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"UPDATE "users" SET "active" = $1 WHERE CASE WHEN "age" >= $2 THEN $3 ELSE $4 END"#
	);
	assert_eq!(values.len(), 4);
}

/// Test UPDATE with subquery in WHERE clause
///
/// Verifies that UPDATE can use `not_in_subquery` to exclude rows
/// matching a subquery result.
#[rstest]
fn test_update_with_subquery() {
	// Arrange
	let subquery = Query::select()
		.column("user_id")
		.from("premium_members")
		.to_owned();

	let stmt = Query::update()
		.table("users")
		.values([(
			"name",
			Value::String(Some(Box::new("Standard User".to_string()))),
		)])
		.and_where(Expr::col("id").not_in_subquery(subquery))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_update(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"UPDATE "users" SET "name" = $1 WHERE "id" NOT IN (SELECT "user_id" FROM "premium_members")"#
	);
	assert_eq!(values.len(), 1);
}
