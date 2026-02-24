// JOIN tests for SELECT statement
//
// These tests verify that the query builder correctly generates SQL
// for various JOIN types: INNER, LEFT, RIGHT, FULL OUTER, CROSS, SELF, and multiple JOINs.

use reinhardt_query::prelude::*;
use rstest::*;

/// Test INNER JOIN
///
/// Verifies that `inner_join` generates correct INNER JOIN SQL.
#[rstest]
fn test_select_inner_join() {
	// Arrange
	let stmt = Query::select()
		.column(("users", "name"))
		.column(("orders", "total_amount"))
		.from("users")
		.inner_join(
			"orders",
			Expr::col(("users", "id")).equals(("orders", "user_id")),
		)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "users"."name", "orders"."total_amount" FROM "users" INNER JOIN "orders" ON "users"."id" = "orders"."user_id""#
	);
	assert_eq!(values.len(), 0);
}

/// Test LEFT JOIN
///
/// Verifies that `left_join` generates correct LEFT JOIN SQL.
#[rstest]
fn test_select_left_join() {
	// Arrange
	let stmt = Query::select()
		.column(("users", "name"))
		.column(("orders", "total_amount"))
		.from("users")
		.left_join(
			"orders",
			Expr::col(("users", "id")).equals(("orders", "user_id")),
		)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "users"."name", "orders"."total_amount" FROM "users" LEFT JOIN "orders" ON "users"."id" = "orders"."user_id""#
	);
	assert_eq!(values.len(), 0);
}

/// Test RIGHT JOIN
///
/// Verifies that `right_join` generates correct RIGHT JOIN SQL.
#[rstest]
fn test_select_right_join() {
	// Arrange
	let stmt = Query::select()
		.column(("users", "name"))
		.column(("orders", "total_amount"))
		.from("users")
		.right_join(
			"orders",
			Expr::col(("users", "id")).equals(("orders", "user_id")),
		)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "users"."name", "orders"."total_amount" FROM "users" RIGHT JOIN "orders" ON "users"."id" = "orders"."user_id""#
	);
	assert_eq!(values.len(), 0);
}

/// Test FULL OUTER JOIN
///
/// Verifies that `full_outer_join` generates correct FULL OUTER JOIN SQL.
#[rstest]
fn test_select_full_outer_join() {
	// Arrange
	let stmt = Query::select()
		.column(("users", "name"))
		.column(("orders", "total_amount"))
		.from("users")
		.full_outer_join(
			"orders",
			Expr::col(("users", "id")).equals(("orders", "user_id")),
		)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "users"."name", "orders"."total_amount" FROM "users" FULL OUTER JOIN "orders" ON "users"."id" = "orders"."user_id""#
	);
	assert_eq!(values.len(), 0);
}

/// Test CROSS JOIN
///
/// Verifies that `cross_join` generates correct CROSS JOIN SQL.
#[rstest]
fn test_select_cross_join() {
	// Arrange
	let stmt = Query::select()
		.column(("users", "name"))
		.column(("products", "name"))
		.from("users")
		.cross_join("products")
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "users"."name", "products"."name" FROM "users" CROSS JOIN "products""#
	);
	assert_eq!(values.len(), 0);
}

/// Test SELF JOIN
///
/// Verifies that a table can be joined with itself using an alias.
#[rstest]
fn test_select_self_join() {
	// Arrange
	let stmt = Query::select()
		.column(("u1", "name"))
		.column(("u2", "name"))
		.from_as("users", "u1")
		.inner_join(
			TableRef::TableAlias("users".into_iden(), "u2".into_iden()),
			Expr::col(("u1", "age")).equals(("u2", "age")),
		)
		.and_where(Expr::col(("u1", "id")).ne(Expr::col(("u2", "id"))))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "u1"."name", "u2"."name" FROM "users" AS "u1" INNER JOIN "users" AS "u2" ON "u1"."age" = "u2"."age" WHERE "u1"."id" <> "u2"."id""#
	);
	assert_eq!(values.len(), 0);
}

/// Test multiple JOINs
///
/// Verifies that multiple JOIN clauses can be chained in a single query.
#[rstest]
fn test_select_multiple_joins() {
	// Arrange
	let stmt = Query::select()
		.column(("users", "name"))
		.column(("orders", "total_amount"))
		.column(("products", "name"))
		.from("users")
		.inner_join(
			"orders",
			Expr::col(("users", "id")).equals(("orders", "user_id")),
		)
		.left_join(
			"products",
			Expr::col(("orders", "product_id")).equals(("products", "id")),
		)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "users"."name", "orders"."total_amount", "products"."name" FROM "users" INNER JOIN "orders" ON "users"."id" = "orders"."user_id" LEFT JOIN "products" ON "orders"."product_id" = "products"."id""#
	);
	assert_eq!(values.len(), 0);
}
