//! Plan-only EXPLAIN statement builder.
//!
//! This module wraps a typed [`SelectStatement`] in a backend-specific
//! diagnostic command. The API intentionally has no `ANALYZE` or arbitrary
//! option-string escape hatch, so building an [`ExplainStatement`] cannot opt
//! into executing the data-producing query.

use crate::{
	QueryBuildError,
	backend::{
		CockroachDBQueryBuilder, MySqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder,
	},
	expr::{Condition, ConditionExpression, SimpleExpr},
	query::SelectStatement,
	types::{JoinOn, OrderExpr, OrderExprKind, TableRef, WindowStatement},
	value::Values,
};

/// Output formats exposed by plan-only EXPLAIN statements.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExplainFormat {
	/// Human-readable or tabular backend output.
	#[default]
	Text,
	/// Machine-readable JSON output.
	Json,
	/// PostgreSQL XML output.
	Xml,
	/// PostgreSQL YAML output.
	Yaml,
	/// MySQL tree output.
	Tree,
}

/// Typed, non-executing options for an EXPLAIN statement.
///
/// The PostgreSQL-only fields are rejected by other backends. `ANALYZE`,
/// buffer/timing statistics, and arbitrary option strings are deliberately
/// absent because they can execute or otherwise exceed plan-only diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExplainOptions {
	/// Requested output format.
	pub format: ExplainFormat,
	/// Include PostgreSQL's verbose planner details.
	pub verbose: bool,
	/// Override PostgreSQL's cost-estimate display setting.
	pub costs: Option<bool>,
	/// Include PostgreSQL planner-affecting settings.
	pub settings: bool,
}

impl ExplainOptions {
	/// Requests one supported output format.
	#[must_use]
	pub const fn format(mut self, format: ExplainFormat) -> Self {
		self.format = format;
		self
	}

	/// Requests PostgreSQL verbose planner details.
	#[must_use]
	pub const fn verbose(mut self) -> Self {
		self.verbose = true;
		self
	}

	/// Controls PostgreSQL cost-estimate output.
	#[must_use]
	pub const fn costs(mut self, enabled: bool) -> Self {
		self.costs = Some(enabled);
		self
	}

	/// Requests PostgreSQL planner-affecting settings.
	#[must_use]
	pub const fn settings(mut self) -> Self {
		self.settings = true;
		self
	}
}

/// A typed SELECT wrapped in a plan-only EXPLAIN command.
#[derive(Debug, Clone)]
pub struct ExplainStatement {
	select: SelectStatement,
	options: ExplainOptions,
}

impl ExplainStatement {
	/// Wraps a SELECT statement with typed plan-only options.
	pub fn new(select: SelectStatement, options: ExplainOptions) -> Self {
		Self { select, options }
	}

	/// Returns the wrapped SELECT statement.
	pub fn select(&self) -> &SelectStatement {
		&self.select
	}

	/// Returns the requested plan-only options.
	pub fn options(&self) -> ExplainOptions {
		self.options
	}

	/// Builds PostgreSQL EXPLAIN SQL after validating backend capabilities.
	pub fn build_postgres_checked(&self) -> Result<(String, Values), QueryBuildError> {
		if self.options.format == ExplainFormat::Tree {
			return Err(unsupported("EXPLAIN FORMAT TREE", "PostgreSQL"));
		}
		let (select, values) = PostgresQueryBuilder.build_select_checked(&self.select)?;
		Ok((self.postgres_sql(select), values))
	}

	/// Builds MySQL EXPLAIN SQL after validating backend capabilities.
	pub fn build_mysql_checked(&self) -> Result<(String, Values), QueryBuildError> {
		self.reject_postgres_options("MySQL")?;
		if mysql_explain_may_evaluate_query(&self.select) {
			return Err(unsupported(
				"plan-only EXPLAIN for subqueries or unchecked expressions",
				"MySQL",
			));
		}
		let format = match self.options.format {
			ExplainFormat::Text => "TRADITIONAL",
			ExplainFormat::Json => "JSON",
			ExplainFormat::Tree => "TREE",
			ExplainFormat::Xml => {
				return Err(unsupported("EXPLAIN FORMAT XML", "MySQL"));
			}
			ExplainFormat::Yaml => {
				return Err(unsupported("EXPLAIN FORMAT YAML", "MySQL"));
			}
		};
		let (select, values) = MySqlQueryBuilder.build_select_checked(&self.select)?;
		Ok((format!("EXPLAIN FORMAT={format} {select}"), values))
	}

	/// Builds SQLite EXPLAIN QUERY PLAN SQL after validating backend capabilities.
	pub fn build_sqlite_checked(&self) -> Result<(String, Values), QueryBuildError> {
		self.reject_postgres_options("SQLite")?;
		if self.options.format != ExplainFormat::Text {
			return Err(unsupported(format_feature(self.options.format), "SQLite"));
		}
		let (select, values) = SqliteQueryBuilder.build_select_checked(&self.select)?;
		Ok((format!("EXPLAIN QUERY PLAN {select}"), values))
	}

	/// Builds CockroachDB EXPLAIN SQL after validating backend capabilities.
	pub fn build_cockroachdb_checked(&self) -> Result<(String, Values), QueryBuildError> {
		self.reject_postgres_options("CockroachDB")?;
		if self.options.format != ExplainFormat::Text {
			return Err(unsupported(
				format_feature(self.options.format),
				"CockroachDB",
			));
		}
		let (select, values) = CockroachDBQueryBuilder::new().build_select_checked(&self.select)?;
		Ok((format!("EXPLAIN {select}"), values))
	}

	fn postgres_sql(&self, select: String) -> String {
		let mut options = Vec::new();
		if self.options.verbose {
			options.push("VERBOSE TRUE".to_string());
		}
		if let Some(costs) = self.options.costs {
			options.push(format!("COSTS {}", sql_bool(costs)));
		}
		if self.options.settings {
			options.push("SETTINGS TRUE".to_string());
		}
		match self.options.format {
			ExplainFormat::Text => {}
			ExplainFormat::Json => options.push("FORMAT JSON".to_string()),
			ExplainFormat::Xml => options.push("FORMAT XML".to_string()),
			ExplainFormat::Yaml => options.push("FORMAT YAML".to_string()),
			ExplainFormat::Tree => unreachable!("TREE format was rejected before SQL generation"),
		}

		if options.is_empty() {
			format!("EXPLAIN {select}")
		} else {
			format!("EXPLAIN ({}) {select}", options.join(", "))
		}
	}

	fn reject_postgres_options(&self, backend: &'static str) -> Result<(), QueryBuildError> {
		if self.options.verbose {
			return Err(unsupported("EXPLAIN VERBOSE", backend));
		}
		if self.options.costs.is_some() {
			return Err(unsupported("EXPLAIN COSTS", backend));
		}
		if self.options.settings {
			return Err(unsupported("EXPLAIN SETTINGS", backend));
		}
		Ok(())
	}
}

fn sql_bool(value: bool) -> &'static str {
	if value { "TRUE" } else { "FALSE" }
}

fn format_feature(format: ExplainFormat) -> &'static str {
	match format {
		ExplainFormat::Text => "EXPLAIN FORMAT TEXT",
		ExplainFormat::Json => "EXPLAIN FORMAT JSON",
		ExplainFormat::Xml => "EXPLAIN FORMAT XML",
		ExplainFormat::Yaml => "EXPLAIN FORMAT YAML",
		ExplainFormat::Tree => "EXPLAIN FORMAT TREE",
	}
}

fn unsupported(feature: &'static str, backend: &'static str) -> QueryBuildError {
	QueryBuildError::UnsupportedBackendFeature { feature, backend }
}

fn mysql_explain_may_evaluate_query(statement: &SelectStatement) -> bool {
	!statement.ctes.is_empty()
		|| !statement.unions.is_empty()
		|| statement
			.selects
			.iter()
			.any(|select| unsafe_expr(&select.expr))
		|| statement.from.iter().any(unsafe_table)
		|| statement.join.iter().any(|join| {
			unsafe_table(&join.table)
				|| matches!(&join.on, Some(JoinOn::Condition(condition)) if unsafe_condition(condition))
		}) || unsafe_conditions(&statement.r#where.conditions)
		|| statement.groups.iter().any(unsafe_expr)
		|| unsafe_conditions(&statement.having.conditions)
		|| statement.orders.iter().any(unsafe_order)
		|| statement
			.windows
			.iter()
			.any(|(_, window)| unsafe_window(window))
}

fn unsafe_table(table: &TableRef) -> bool {
	matches!(table, TableRef::SubQuery(_, _))
}

fn unsafe_conditions(conditions: &[ConditionExpression]) -> bool {
	conditions.iter().any(|condition| match condition {
		ConditionExpression::SimpleExpr(expression) => unsafe_expr(expression),
		ConditionExpression::Condition(condition) => unsafe_condition(condition),
	})
}

fn unsafe_condition(condition: &Condition) -> bool {
	unsafe_conditions(&condition.conditions)
}

fn unsafe_order(order: &OrderExpr) -> bool {
	matches!(&order.expr, OrderExprKind::Expr(expression) if unsafe_expr(expression))
}

fn unsafe_window(window: &WindowStatement) -> bool {
	window.partition_by.iter().any(unsafe_expr) || window.order_by.iter().any(unsafe_order)
}

fn unsafe_expr(expression: &SimpleExpr) -> bool {
	match expression {
		SimpleExpr::SubQuery(_, _) | SimpleExpr::Custom(_) | SimpleExpr::FunctionCall(_, _) => true,
		SimpleExpr::CustomWithExpr(template, expressions) => {
			template != "? LIKE ? ESCAPE '\\'" || expressions.iter().any(unsafe_expr)
		}
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _) => unsafe_expr(expression),
		SimpleExpr::Binary(left, _, right) => unsafe_expr(left) || unsafe_expr(right),
		SimpleExpr::Tuple(expressions) => expressions.iter().any(unsafe_expr),
		SimpleExpr::Case(statement) => {
			statement
				.when_clauses
				.iter()
				.any(|(condition, result)| unsafe_expr(condition) || unsafe_expr(result))
				|| statement.else_clause.as_ref().is_some_and(unsafe_expr)
		}
		SimpleExpr::Window { func, window } => unsafe_expr(func) || unsafe_window(window),
		SimpleExpr::WindowNamed { func, .. } => unsafe_expr(func),
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Value(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk => false,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		expr::{Expr, ExprTrait},
		query::Query,
		types::{Frame, FrameClause, FrameType, WindowStatement},
	};

	fn filtered_select() -> SelectStatement {
		Query::select()
			.column("id")
			.from("users")
			.and_where(Expr::col("active").eq(true))
			.to_owned()
	}

	#[test]
	fn postgres_build_preserves_select_bindings_and_safe_options() {
		let statement = ExplainStatement::new(
			filtered_select(),
			ExplainOptions {
				format: ExplainFormat::Json,
				verbose: true,
				costs: Some(false),
				settings: true,
			},
		);

		let (sql, values) = statement
			.build_postgres_checked()
			.expect("PostgreSQL options should build");

		assert_eq!(
			sql,
			r#"EXPLAIN (VERBOSE TRUE, COSTS FALSE, SETTINGS TRUE, FORMAT JSON) SELECT "id" FROM "users" WHERE "active" = $1"#
		);
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn mysql_build_uses_explicit_tree_format() {
		let statement = ExplainStatement::new(
			filtered_select(),
			ExplainOptions {
				format: ExplainFormat::Tree,
				..ExplainOptions::default()
			},
		);

		let (sql, values) = statement
			.build_mysql_checked()
			.expect("MySQL TREE format should build");

		assert_eq!(
			sql,
			"EXPLAIN FORMAT=TREE SELECT `id` FROM `users` WHERE `active` = ?"
		);
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn sqlite_build_uses_query_plan_and_preserves_bindings() {
		let statement = ExplainStatement::new(filtered_select(), ExplainOptions::default());

		let (sql, values) = statement
			.build_sqlite_checked()
			.expect("SQLite text plan should build");

		assert_eq!(
			sql,
			r#"EXPLAIN QUERY PLAN SELECT "id" FROM "users" WHERE "active" = ?"#
		);
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn checked_non_postgres_explain_rejects_distinct_on() {
		let mut select = Query::select();
		select.column("id").from("users").distinct_on(["id"]);
		let statement = ExplainStatement::new(select.to_owned(), ExplainOptions::default());

		let error = statement
			.build_mysql_checked()
			.expect_err("MySQL must reject PostgreSQL-only DISTINCT ON before rendering");
		assert_eq!(
			error,
			QueryBuildError::UnsupportedBackendFeature {
				feature: "DISTINCT ON",
				backend: "MySQL",
			}
		);
	}

	#[test]
	fn cockroachdb_explain_permits_distinct_on() {
		let mut select = Query::select();
		select.column("id").from("users").distinct_on(["id"]);
		let statement = ExplainStatement::new(select.to_owned(), ExplainOptions::default());

		assert_eq!(
			statement.build_cockroachdb_checked(),
			Ok((
				"EXPLAIN SELECT DISTINCT ON (\"id\") \"id\" FROM \"users\"".to_string(),
				Values::default(),
			))
		);
	}

	#[test]
	fn mysql_explain_rejects_groups_window_frames_before_rendering() {
		let mut select = filtered_select();
		select.window_as(
			"ranked",
			WindowStatement {
				partition_by: Vec::new(),
				order_by: Vec::new(),
				frame: Some(FrameClause {
					frame_type: FrameType::Groups,
					start: Frame::CurrentRow,
					end: None,
				}),
			},
		);
		let statement = ExplainStatement::new(select, ExplainOptions::default());

		assert_eq!(
			statement.build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "GROUPS window frames",
				backend: "MySQL",
			})
		);
	}

	#[test]
	fn mysql_explain_rejects_full_outer_joins_before_rendering() {
		let select = Query::select()
			.column(("users", "id"))
			.from("users")
			.full_outer_join(
				"accounts",
				Expr::col(("users", "account_id")).equals(("accounts", "id")),
			)
			.to_owned();
		let statement = ExplainStatement::new(select, ExplainOptions::default());

		assert_eq!(
			statement.build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "FULL OUTER JOIN",
				backend: "MySQL",
			})
		);
	}

	#[test]
	fn unsupported_format_returns_capability_error() {
		let statement = ExplainStatement::new(
			filtered_select(),
			ExplainOptions {
				format: ExplainFormat::Xml,
				..ExplainOptions::default()
			},
		);

		assert_eq!(
			statement.build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "EXPLAIN FORMAT XML",
				backend: "MySQL",
			})
		);
	}

	#[test]
	fn mysql_rejects_subquery_before_building_sql() {
		let subquery = Query::select().column("id").from("accounts").to_owned();
		let statement = ExplainStatement::new(
			Query::select()
				.column("id")
				.from_subquery(subquery, "account_ids")
				.to_owned(),
			ExplainOptions::default(),
		);

		assert_eq!(
			statement.build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "plan-only EXPLAIN for subqueries or unchecked expressions",
				backend: "MySQL",
			})
		);
	}
}
