// Aggregation tests for SELECT statement
//
// These tests verify that the query builder correctly generates SQL
// for aggregate functions (COUNT, SUM, AVG, MIN, MAX) and GROUP BY / HAVING.

use reinhardt_query::prelude::*;
use rstest::*;

/// Test COUNT aggregation
///
/// Verifies that `Func::count` generates correct COUNT SQL.
#[rstest]
fn test_select_count() {
	// Arrange
	let stmt = Query::select()
		.expr_as(Func::count(Expr::asterisk().into_simple_expr()), "cnt")
		.from("users")
		.to_owned();

	// Act
	let pg = PostgresQueryBuilder;
	let (pg_sql, pg_values) = pg.build_select(&stmt);

	let mysql = MySqlQueryBuilder;
	let (my_sql, my_values) = mysql.build_select(&stmt);

	let sqlite = SqliteQueryBuilder;
	let (sl_sql, sl_values) = sqlite.build_select(&stmt);

	// Assert
	assert_eq!(pg_sql, r#"SELECT COUNT(*) AS "cnt" FROM "users""#);
	assert_eq!(pg_values.len(), 0);

	assert_eq!(my_sql, "SELECT COUNT(*) AS `cnt` FROM `users`");
	assert_eq!(my_values.len(), 0);

	assert_eq!(sl_sql, r#"SELECT COUNT(*) AS "cnt" FROM "users""#);
	assert_eq!(sl_values.len(), 0);
}

/// Test SUM aggregation
///
/// Verifies that `Func::sum` generates correct SUM SQL.
#[rstest]
fn test_select_sum() {
	// Arrange
	let stmt = Query::select()
		.expr_as(Func::sum(Expr::col("age").into_simple_expr()), "total")
		.from("users")
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(sql, r#"SELECT SUM("age") AS "total" FROM "users""#);
	assert_eq!(values.len(), 0);
}

/// Test AVG aggregation
///
/// Verifies that `Func::avg` generates correct AVG SQL.
#[rstest]
fn test_select_avg() {
	// Arrange
	let stmt = Query::select()
		.expr_as(Func::avg(Expr::col("age").into_simple_expr()), "average")
		.from("users")
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(sql, r#"SELECT AVG("age") AS "average" FROM "users""#);
	assert_eq!(values.len(), 0);
}

/// Test MIN/MAX aggregation
///
/// Verifies that `Func::min` and `Func::max` generate correct SQL.
#[rstest]
fn test_select_min_max() {
	// Arrange
	let stmt = Query::select()
		.expr_as(Func::min(Expr::col("age").into_simple_expr()), "youngest")
		.expr_as(Func::max(Expr::col("age").into_simple_expr()), "oldest")
		.from("users")
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT MIN("age") AS "youngest", MAX("age") AS "oldest" FROM "users""#
	);
	assert_eq!(values.len(), 0);
}

/// Test GROUP BY
///
/// Verifies that `group_by` generates correct GROUP BY SQL with aggregate.
#[rstest]
fn test_select_group_by() {
	// Arrange
	let stmt = Query::select()
		.column("active")
		.expr_as(Func::count(Expr::asterisk().into_simple_expr()), "cnt")
		.from("users")
		.group_by("active")
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "active", COUNT(*) AS "cnt" FROM "users" GROUP BY "active""#
	);
	assert_eq!(values.len(), 0);
}

/// Test HAVING
///
/// Verifies that `and_having` generates correct HAVING clause with aggregate condition.
#[rstest]
fn test_select_having() {
	// Arrange
	let stmt = Query::select()
		.column("active")
		.expr_as(Func::count(Expr::asterisk().into_simple_expr()), "cnt")
		.from("users")
		.group_by("active")
		.and_having(Func::count(Expr::asterisk().into_simple_expr()).gte(2i32))
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "active", COUNT(*) AS "cnt" FROM "users" GROUP BY "active" HAVING COUNT(*) >= $1"#
	);
	assert_eq!(values.len(), 1);
}

/// Test GROUP BY with multiple columns
///
/// Verifies that `group_by_columns` generates correct multi-column GROUP BY SQL.
#[rstest]
fn test_select_group_by_multiple_columns() {
	// Arrange
	let stmt = Query::select()
		.column("active")
		.column("age")
		.expr_as(Func::count(Expr::asterisk().into_simple_expr()), "cnt")
		.from("users")
		.group_by_columns(["active", "age"])
		.to_owned();

	// Act
	let builder = PostgresQueryBuilder;
	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"SELECT "active", "age", COUNT(*) AS "cnt" FROM "users" GROUP BY "active", "age""#
	);
	assert_eq!(values.len(), 0);
}
