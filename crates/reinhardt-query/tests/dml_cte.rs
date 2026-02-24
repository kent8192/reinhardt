//! CTE (Common Table Expression) SQL generation tests
//!
//! Tests for WITH clause support across all backends:
//! - Basic CTE with a single named query
//! - Multiple CTEs in a single WITH clause
//! - RECURSIVE CTE support
//! - CTE referenced in JOIN
//! - CTE with parameterized WHERE conditions
//! - CTE with aggregation functions

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::TableRef;

// =============================================================================
// PostgreSQL CTE tests
// =============================================================================

/// Basic single CTE generates correct WITH clause on PostgreSQL
#[rstest]
fn test_postgres_single_cte_sql_generation() {
	// Arrange
	let builder = PostgresQueryBuilder::new();

	let mut cte_query = Query::select();
	cte_query
		.column("id")
		.column("name")
		.from("users")
		.and_where(Expr::col("active").eq(true));

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("active_users", cte_query)
		.column("name")
		.from("active_users");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"WITH "active_users" AS (SELECT "id", "name" FROM "users" WHERE "active" = $1) SELECT "name" FROM "active_users""#
	);
	assert_eq!(values.len(), 1);
}

/// Multiple CTEs are separated by commas on PostgreSQL
#[rstest]
fn test_postgres_multiple_ctes_sql_generation() {
	// Arrange
	let builder = PostgresQueryBuilder::new();

	let mut cte1 = Query::select();
	cte1.column("id")
		.column("name")
		.from("employees")
		.and_where(Expr::col("department").eq("Engineering"));

	let mut cte2 = Query::select();
	cte2.column("id")
		.column("name")
		.from("employees")
		.and_where(Expr::col("department").eq("Sales"));

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("eng_emp", cte1)
		.with_cte("sales_emp", cte2)
		.column("name")
		.from("eng_emp");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(sql.starts_with("WITH"), "SQL must start with WITH keyword");
	assert!(
		sql.contains(r#""eng_emp" AS ("#),
		"First CTE must be present"
	);
	assert!(
		sql.contains(r#""sales_emp" AS ("#),
		"Second CTE must be present"
	);
	// Two parameters: one per CTE WHERE clause
	assert_eq!(values.len(), 2);
}

/// RECURSIVE CTE generates WITH RECURSIVE keyword on PostgreSQL
#[rstest]
fn test_postgres_recursive_cte_sql_generation() {
	// Arrange
	let builder = PostgresQueryBuilder::new();

	let mut cte_query = Query::select();
	cte_query
		.column("id")
		.column("name")
		.column("manager_id")
		.from("employees");

	// Act
	let mut stmt = Query::select();
	stmt.with_recursive_cte("employee_hierarchy", cte_query)
		.column("name")
		.from("employee_hierarchy");

	let (sql, _values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"WITH RECURSIVE "employee_hierarchy" AS (SELECT "id", "name", "manager_id" FROM "employees") SELECT "name" FROM "employee_hierarchy""#
	);
}

/// CTE referenced in INNER JOIN generates correct SQL on PostgreSQL
#[rstest]
fn test_postgres_cte_referenced_in_join() {
	// Arrange
	let builder = PostgresQueryBuilder::new();

	let mut cte_query = Query::select();
	cte_query
		.column("user_id")
		.column("order_count")
		.from("orders")
		.group_by("user_id");

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("user_orders", cte_query)
		.column(("users", "name"))
		.column(("uo", "order_count"))
		.from("users")
		.inner_join(
			TableRef::table_alias("user_orders", "uo"),
			Expr::col(("users", "id")).eq(Expr::col(("uo", "user_id"))),
		);

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(
		sql.contains(r#"WITH "user_orders" AS"#),
		"CTE definition must be present"
	);
	assert!(
		sql.contains(r#"INNER JOIN "user_orders" AS "uo""#),
		"JOIN referencing CTE must be present"
	);
	assert!(
		sql.contains(r#""users"."id" = "uo"."user_id""#),
		"JOIN condition must be present"
	);
	assert_eq!(values.len(), 0);
}

/// CTE with WHERE parameters uses correct placeholder numbering on PostgreSQL
#[rstest]
fn test_postgres_cte_parameter_numbering() {
	// Arrange
	let builder = PostgresQueryBuilder::new();

	// CTE introduces parameter $1 (status = 'completed')
	let mut cte_query = Query::select();
	cte_query
		.column("id")
		.column("total")
		.from("orders")
		.and_where(Expr::col("status").eq("completed"));

	// Main query introduces parameter $2 (min_total)
	let mut stmt = Query::select();
	stmt.with_cte("completed_orders", cte_query)
		.column("id")
		.column("total")
		.from("completed_orders")
		.and_where(Expr::col("total").gt(100i64));

	let (sql, values) = builder.build_select(&stmt);

	// Assert: CTE param is $1, main query param is $2
	assert!(sql.contains("$1"), "CTE parameter placeholder must be $1");
	assert!(
		sql.contains("$2"),
		"Main query parameter placeholder must be $2"
	);
	assert_eq!(values.len(), 2);
}

// =============================================================================
// MySQL CTE tests
// =============================================================================

/// Basic single CTE generates correct WITH clause on MySQL
#[rstest]
fn test_mysql_single_cte_sql_generation() {
	// Arrange
	let builder = MySqlQueryBuilder::new();

	let mut cte_query = Query::select();
	cte_query
		.column("id")
		.column("name")
		.from("users")
		.and_where(Expr::col("active").eq(true));

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("active_users", cte_query)
		.column("name")
		.from("active_users");

	let (sql, values) = builder.build_select(&stmt);

	// Assert: MySQL uses backtick quoting
	assert_eq!(
		sql,
		"WITH `active_users` AS (SELECT `id`, `name` FROM `users` WHERE `active` = ?) SELECT `name` FROM `active_users`"
	);
	assert_eq!(values.len(), 1);
}

/// RECURSIVE CTE generates WITH RECURSIVE keyword on MySQL
#[rstest]
fn test_mysql_recursive_cte_sql_generation() {
	// Arrange
	let builder = MySqlQueryBuilder::new();

	let mut cte_query = Query::select();
	cte_query
		.column("id")
		.column("name")
		.column("manager_id")
		.from("employees");

	// Act
	let mut stmt = Query::select();
	stmt.with_recursive_cte("employee_hierarchy", cte_query)
		.column("name")
		.from("employee_hierarchy");

	let (sql, _values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		"WITH RECURSIVE `employee_hierarchy` AS (SELECT `id`, `name`, `manager_id` FROM `employees`) SELECT `name` FROM `employee_hierarchy`"
	);
}

/// Multiple CTEs on MySQL contain all CTE definitions
#[rstest]
fn test_mysql_multiple_ctes_sql_generation() {
	// Arrange
	let builder = MySqlQueryBuilder::new();

	let mut cte1 = Query::select();
	cte1.column("id")
		.column("name")
		.from("employees")
		.and_where(Expr::col("department").eq("Engineering"));

	let mut cte2 = Query::select();
	cte2.column("id")
		.column("name")
		.from("employees")
		.and_where(Expr::col("department").eq("Sales"));

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("eng_emp", cte1)
		.with_cte("sales_emp", cte2)
		.column("name")
		.from("eng_emp");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(sql.starts_with("WITH"), "SQL must start with WITH keyword");
	assert!(sql.contains("`eng_emp` AS ("), "First CTE must be present");
	assert!(
		sql.contains("`sales_emp` AS ("),
		"Second CTE must be present"
	);
	assert_eq!(values.len(), 2);
}

// =============================================================================
// SQLite CTE tests
// =============================================================================

/// Basic single CTE generates correct WITH clause on SQLite
#[rstest]
fn test_sqlite_single_cte_sql_generation() {
	// Arrange
	let builder = SqliteQueryBuilder::new();

	let mut cte_query = Query::select();
	cte_query
		.column("id")
		.column("name")
		.from("users")
		.and_where(Expr::col("active").eq(true));

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("active_users", cte_query)
		.column("name")
		.from("active_users");

	let (sql, values) = builder.build_select(&stmt);

	// Assert: SQLite uses double-quote quoting like PostgreSQL but ? placeholders
	assert_eq!(
		sql,
		r#"WITH "active_users" AS (SELECT "id", "name" FROM "users" WHERE "active" = ?) SELECT "name" FROM "active_users""#
	);
	assert_eq!(values.len(), 1);
}

/// RECURSIVE CTE generates WITH RECURSIVE keyword on SQLite
#[rstest]
fn test_sqlite_recursive_cte_sql_generation() {
	// Arrange
	let builder = SqliteQueryBuilder::new();

	let mut cte_query = Query::select();
	cte_query
		.column("id")
		.column("name")
		.column("manager_id")
		.from("employees");

	// Act
	let mut stmt = Query::select();
	stmt.with_recursive_cte("employee_hierarchy", cte_query)
		.column("name")
		.from("employee_hierarchy");

	let (sql, _values) = builder.build_select(&stmt);

	// Assert
	assert_eq!(
		sql,
		r#"WITH RECURSIVE "employee_hierarchy" AS (SELECT "id", "name", "manager_id" FROM "employees") SELECT "name" FROM "employee_hierarchy""#
	);
}

/// Multiple CTEs on SQLite contain all CTE definitions
#[rstest]
fn test_sqlite_multiple_ctes_sql_generation() {
	// Arrange
	let builder = SqliteQueryBuilder::new();

	let mut cte1 = Query::select();
	cte1.column("id")
		.column("name")
		.from("employees")
		.and_where(Expr::col("department").eq("Engineering"));

	let mut cte2 = Query::select();
	cte2.column("id")
		.column("name")
		.from("employees")
		.and_where(Expr::col("department").eq("Sales"));

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("eng_emp", cte1)
		.with_cte("sales_emp", cte2)
		.column("name")
		.from("eng_emp");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(sql.starts_with("WITH"), "SQL must start with WITH keyword");
	assert!(
		sql.contains(r#""eng_emp" AS ("#),
		"First CTE must be present"
	);
	assert!(
		sql.contains(r#""sales_emp" AS ("#),
		"Second CTE must be present"
	);
	assert_eq!(values.len(), 2);
}

// =============================================================================
// Cross-backend structural tests
// =============================================================================

/// CTE with no WHERE clause generates no parameters on PostgreSQL
#[rstest]
fn test_postgres_cte_no_params_generates_empty_values() {
	// Arrange
	let builder = PostgresQueryBuilder::new();

	let mut cte = Query::select();
	cte.column("id").column("name").from("products");

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("all_products", cte)
		.column("name")
		.from("all_products");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(sql.starts_with("WITH"), "SQL must start with WITH keyword");
	assert_eq!(values.len(), 0);
}

/// CTE with no WHERE clause generates no parameters on MySQL
#[rstest]
fn test_mysql_cte_no_params_generates_empty_values() {
	// Arrange
	let builder = MySqlQueryBuilder::new();

	let mut cte = Query::select();
	cte.column("id").column("name").from("products");

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("all_products", cte)
		.column("name")
		.from("all_products");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(sql.starts_with("WITH"), "SQL must start with WITH keyword");
	assert_eq!(values.len(), 0);
}

/// CTE with no WHERE clause generates no parameters on SQLite
#[rstest]
fn test_sqlite_cte_no_params_generates_empty_values() {
	// Arrange
	let builder = SqliteQueryBuilder::new();

	let mut cte = Query::select();
	cte.column("id").column("name").from("products");

	// Act
	let mut stmt = Query::select();
	stmt.with_cte("all_products", cte)
		.column("name")
		.from("all_products");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(sql.starts_with("WITH"), "SQL must start with WITH keyword");
	assert_eq!(values.len(), 0);
}

/// SelectStatement without CTE generates no WITH clause
#[rstest]
fn test_no_cte_no_with_clause_postgres() {
	// Arrange
	let builder = PostgresQueryBuilder::new();

	// Act
	let mut stmt = Query::select();
	stmt.column("id").column("name").from("users");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(
		!sql.contains("WITH"),
		"SELECT without CTE must not contain WITH keyword"
	);
	assert_eq!(sql, r#"SELECT "id", "name" FROM "users""#);
	assert_eq!(values.len(), 0);
}

/// SelectStatement without CTE generates no WITH clause on MySQL
#[rstest]
fn test_no_cte_no_with_clause_mysql() {
	// Arrange
	let builder = MySqlQueryBuilder::new();

	// Act
	let mut stmt = Query::select();
	stmt.column("id").column("name").from("users");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(
		!sql.contains("WITH"),
		"SELECT without CTE must not contain WITH keyword"
	);
	assert_eq!(sql, "SELECT `id`, `name` FROM `users`");
	assert_eq!(values.len(), 0);
}

/// SelectStatement without CTE generates no WITH clause on SQLite
#[rstest]
fn test_no_cte_no_with_clause_sqlite() {
	// Arrange
	let builder = SqliteQueryBuilder::new();

	// Act
	let mut stmt = Query::select();
	stmt.column("id").column("name").from("users");

	let (sql, values) = builder.build_select(&stmt);

	// Assert
	assert!(
		!sql.contains("WITH"),
		"SELECT without CTE must not contain WITH keyword"
	);
	assert_eq!(sql, r#"SELECT "id", "name" FROM "users""#);
	assert_eq!(values.len(), 0);
}
