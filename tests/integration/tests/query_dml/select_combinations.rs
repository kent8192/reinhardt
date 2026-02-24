// Combination tests for SELECT statement
//
// These tests verify SELECT combined with advanced SQL features:
// CTE (WITH clause), UNION, DISTINCT ON, and window functions.

use reinhardt_query::prelude::*;
use reinhardt_query::types::{OrderExpr, OrderExprKind, WindowStatement};
use rstest::*;

/// Test SELECT with CTE (WITH clause)
///
/// Verifies that `with_cte` generates correct WITH (Common Table Expression) SQL.
#[rstest]
fn test_select_with_cte() {
	// Arrange
	let cte_query = Query::select()
		.column("id")
		.column("name")
		.from("users")
		.and_where(Expr::col("active").eq(true))
		.to_owned();

	let stmt = Query::select()
		.with_cte("active_users", cte_query)
		.column("id")
		.column("name")
		.from("active_users")
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"WITH "active_users" AS (SELECT "id", "name" FROM "users" WHERE "active" = $1) SELECT "id", "name" FROM "active_users""#
	);
	assert_eq!(values.len(), 1);
}

/// Test SELECT with UNION
///
/// Verifies that `union` generates correct UNION SQL combining two queries.
#[rstest]
fn test_select_with_union() {
	// Arrange
	let second_query = Query::select()
		.column("name")
		.column("email")
		.from("archived_users")
		.to_owned();

	let stmt = Query::select()
		.column("name")
		.column("email")
		.from("users")
		.union(second_query)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "name", "email" FROM "users" UNION SELECT "name", "email" FROM "archived_users""#
	);
	assert_eq!(values.len(), 0);
}

/// Test SELECT with DISTINCT ON
///
/// Verifies that `distinct_on` generates correct DISTINCT ON SQL (PostgreSQL-specific).
#[rstest]
fn test_select_with_distinct_on() {
	// Arrange
	let stmt = Query::select()
		.distinct_on(["department"])
		.column("department")
		.column("name")
		.column("salary")
		.from("employees")
		.order_by("department", Order::Asc)
		.order_by("salary", Order::Desc)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT DISTINCT ON ("department") "department", "name", "salary" FROM "employees" ORDER BY "department" ASC, "salary" DESC"#
	);
	assert_eq!(values.len(), 0);
}

/// Test SELECT with window function
///
/// Verifies that window functions with named WINDOW clause generate correct SQL.
#[rstest]
fn test_select_with_window_function() {
	// Arrange
	let window = WindowStatement {
		partition_by: vec![Expr::col("department").into_simple_expr()],
		order_by: vec![OrderExpr {
			expr: OrderExprKind::Expr(Box::new(Expr::col("salary").into_simple_expr())),
			order: Order::Desc,
			nulls: None,
		}],
		frame: None,
	};

	let stmt = Query::select()
		.column("name")
		.column("department")
		.column("salary")
		.expr_as(Expr::row_number().over_named("w"), "rank")
		.from("employees")
		.window_as("w", window)
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "name", "department", "salary", ROW_NUMBER() OVER "w" AS "rank" FROM "employees" WINDOW "w" AS ( PARTITION BY "department" ORDER BY "salary" DESC )"#
	);
	assert_eq!(values.len(), 0);
}
