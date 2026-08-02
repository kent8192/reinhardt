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
	types::{BinOper, FrameType, JoinOn, OrderExpr, OrderExprKind, TableRef, WindowStatement},
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
		self.reject_select_lock("PostgreSQL")?;
		if self.options.format == ExplainFormat::Tree {
			return Err(unsupported("EXPLAIN FORMAT TREE", "PostgreSQL"));
		}
		let (select, values) = PostgresQueryBuilder.build_select_checked(&self.select)?;
		Ok((self.postgres_sql(select), values))
	}

	/// Builds MySQL EXPLAIN SQL after validating backend capabilities.
	pub fn build_mysql_checked(&self) -> Result<(String, Values), QueryBuildError> {
		self.reject_select_lock("MySQL")?;
		self.reject_postgres_options("MySQL")?;
		self.reject_mysql_window_null_ordering()?;
		self.reject_mysql_unsupported_operators()?;
		if mysql_explain_may_evaluate_query(&self.select) {
			return Err(unsupported(
				"plan-only EXPLAIN for subqueries or unchecked expressions",
				"MySQL",
			));
		}
		let prefix = match self.options.format {
			// Plain EXPLAIN selects the traditional tabular output on both MySQL and
			// MariaDB. MariaDB does not accept `FORMAT=TRADITIONAL`.
			ExplainFormat::Text => "EXPLAIN",
			ExplainFormat::Json => "EXPLAIN FORMAT=JSON",
			ExplainFormat::Tree => {
				// MySqlBackend also represents MariaDB, which does not support TREE output.
				return Err(unsupported("EXPLAIN FORMAT TREE", "MySQL/MariaDB"));
			}
			ExplainFormat::Xml => {
				return Err(unsupported("EXPLAIN FORMAT XML", "MySQL"));
			}
			ExplainFormat::Yaml => {
				return Err(unsupported("EXPLAIN FORMAT YAML", "MySQL"));
			}
		};
		let mut select_statement = self.select.clone();
		quote_mysql_like_templates(&mut select_statement);
		let (select, values) = MySqlQueryBuilder.build_select_checked(&select_statement)?;
		Ok((format!("{prefix} {select}"), values))
	}

	/// Builds SQLite EXPLAIN QUERY PLAN SQL after validating backend capabilities.
	pub fn build_sqlite_checked(&self) -> Result<(String, Values), QueryBuildError> {
		self.reject_select_lock("SQLite")?;
		self.reject_postgres_options("SQLite")?;
		self.reject_sqlite_groups_window_frames()?;
		self.reject_sqlite_ilike()?;
		if self.options.format != ExplainFormat::Text {
			return Err(unsupported(format_feature(self.options.format), "SQLite"));
		}
		let (select, values) = SqliteQueryBuilder.build_select_checked(&self.select)?;
		Ok((format!("EXPLAIN QUERY PLAN {select}"), values))
	}

	/// Builds CockroachDB EXPLAIN SQL after validating backend capabilities.
	pub fn build_cockroachdb_checked(&self) -> Result<(String, Values), QueryBuildError> {
		self.reject_select_lock("CockroachDB")?;
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

	fn reject_mysql_window_null_ordering(&self) -> Result<(), QueryBuildError> {
		if statement_has_window_feature(&self.select, |window| {
			window.order_by.iter().any(|order| order.nulls.is_some())
		}) {
			return Err(unsupported("NULLS FIRST/LAST in window ordering", "MySQL"));
		}
		Ok(())
	}

	fn reject_mysql_unsupported_operators(&self) -> Result<(), QueryBuildError> {
		if statement_has_expression(&self.select, &|expression| {
			matches!(
				expression,
				SimpleExpr::Binary(_, BinOper::ILike | BinOper::NotILike, _)
			)
		}) {
			return Err(unsupported("ILIKE", "MySQL"));
		}
		if statement_has_expression(&self.select, &|expression| {
			matches!(
				expression,
				SimpleExpr::Binary(
					_,
					BinOper::SimilarTo
						| BinOper::NotSimilarTo
						| BinOper::Matches | BinOper::NotMatches,
					_
				)
			)
		}) {
			return Err(unsupported("PostgreSQL pattern operators", "MySQL"));
		}
		if statement_has_expression(&self.select, &|expression| {
			matches!(expression, SimpleExpr::Binary(_, BinOper::PgOperator(_), _))
		}) {
			return Err(unsupported("PostgreSQL operators", "MySQL"));
		}
		Ok(())
	}

	fn reject_select_lock(&self, backend: &'static str) -> Result<(), QueryBuildError> {
		if statement_has_select(&self.select, &|statement| statement.lock.is_some()) {
			return Err(unsupported("SELECT lock clauses in EXPLAIN", backend));
		}
		Ok(())
	}

	fn reject_sqlite_groups_window_frames(&self) -> Result<(), QueryBuildError> {
		if statement_has_window_feature(&self.select, |window| {
			matches!(
				window.frame.as_ref().map(|frame| frame.frame_type.clone()),
				Some(FrameType::Groups)
			)
		}) {
			return Err(unsupported("GROUPS window frames", "SQLite"));
		}
		Ok(())
	}

	fn reject_sqlite_ilike(&self) -> Result<(), QueryBuildError> {
		if statement_has_expression(&self.select, &|expression| {
			matches!(
				expression,
				SimpleExpr::Binary(_, BinOper::ILike | BinOper::NotILike, _)
			) || matches!(expression, SimpleExpr::CustomWithExpr(template, _) if template.to_ascii_uppercase().contains("ILIKE"))
		}) {
			return Err(unsupported("ILIKE", "SQLite"));
		}
		if statement_has_expression(&self.select, &|expression| {
			matches!(
				expression,
				SimpleExpr::Binary(
					_,
					BinOper::SimilarTo
						| BinOper::NotSimilarTo
						| BinOper::Matches | BinOper::NotMatches,
					_
				)
			)
		}) {
			return Err(unsupported("PostgreSQL pattern operators", "SQLite"));
		}
		if statement_has_expression(&self.select, &|expression| {
			matches!(expression, SimpleExpr::Binary(_, BinOper::PgOperator(_), _))
		}) {
			return Err(unsupported("PostgreSQL operators", "SQLite"));
		}
		Ok(())
	}
}

fn statement_has_window_feature(
	statement: &SelectStatement,
	predicate: impl Fn(&WindowStatement) -> bool + Copy,
) -> bool {
	statement
		.windows
		.iter()
		.any(|(_, window)| predicate(window))
		|| statement_has_expression(statement, &|expression| match expression {
		SimpleExpr::Window { func, window } => {
			predicate(window) || expression_has_window_feature(func, predicate)
		}
		_ => expression_has_window_feature(expression, predicate),
	}) || statement
		.ctes
		.iter()
		.any(|cte| statement_has_window_feature(&cte.query, predicate))
		|| statement
			.unions
			.iter()
			.any(|(_, union)| statement_has_window_feature(union, predicate))
		|| statement
			.from
			.iter()
			.any(|table| table_has_window_feature(table, predicate))
		|| statement.join.iter().any(|join| {
			table_has_window_feature(&join.table, predicate)
				|| matches!(&join.on, Some(JoinOn::Condition(condition)) if conditions_have_window_feature(&condition.conditions, predicate))
		})
}

fn table_has_window_feature(
	table: &TableRef,
	predicate: impl Fn(&WindowStatement) -> bool + Copy,
) -> bool {
	matches!(table, TableRef::SubQuery(query, _) if statement_has_window_feature(query, predicate))
}

fn conditions_have_window_feature(
	conditions: &[ConditionExpression],
	predicate: impl Fn(&WindowStatement) -> bool + Copy,
) -> bool {
	conditions.iter().any(|condition| match condition {
		ConditionExpression::SimpleExpr(expression) => {
			expression_has_window_feature(expression, predicate)
		}
		ConditionExpression::Condition(condition) => {
			conditions_have_window_feature(&condition.conditions, predicate)
		}
	})
}

fn expression_has_window_feature(
	expression: &SimpleExpr,
	predicate: impl Fn(&WindowStatement) -> bool + Copy,
) -> bool {
	match expression {
		SimpleExpr::Window { func, window } => {
			predicate(window) || expression_has_window_feature(func, predicate)
		}
		SimpleExpr::WindowNamed { func, .. }
		| SimpleExpr::Unary(_, func)
		| SimpleExpr::AsEnum(_, func)
		| SimpleExpr::ExprAlias(func, _)
		| SimpleExpr::Cast(func, _) => expression_has_window_feature(func, predicate),
		SimpleExpr::Binary(left, _, right) => {
			expression_has_window_feature(left, predicate)
				|| expression_has_window_feature(right, predicate)
		}
		SimpleExpr::FunctionCall(_, expressions)
		| SimpleExpr::Tuple(expressions)
		| SimpleExpr::CustomWithExpr(_, expressions) => expressions
			.iter()
			.any(|expression| expression_has_window_feature(expression, predicate)),
		SimpleExpr::Case(case) => {
			case.when_clauses.iter().any(|(condition, result)| {
				expression_has_window_feature(condition, predicate)
					|| expression_has_window_feature(result, predicate)
			}) || case
				.else_clause
				.as_ref()
				.is_some_and(|result| expression_has_window_feature(result, predicate))
		}
		SimpleExpr::SubQuery(_, query) => statement_has_window_feature(query, predicate),
		_ => false,
	}
}

fn statement_has_expression(
	statement: &SelectStatement,
	predicate: &impl Fn(&SimpleExpr) -> bool,
) -> bool {
	statement.selects.iter().any(|select| expression_matches(&select.expr, predicate))
		|| statement.groups.iter().any(|expression| expression_matches(expression, predicate))
		|| statement.orders.iter().any(|order| matches!(&order.expr, OrderExprKind::Expr(expression) if expression_matches(expression, predicate)))
		|| conditions_match(&statement.r#where.conditions, predicate)
		|| conditions_match(&statement.having.conditions, predicate)
		|| statement.windows.iter().any(|(_, window)| {
			window
				.partition_by
				.iter()
				.any(|expression| expression_matches(expression, predicate))
				|| window.order_by.iter().any(|order| {
					matches!(&order.expr, OrderExprKind::Expr(expression) if expression_matches(expression, predicate))
				})
			})
		|| statement.ctes.iter().any(|cte| statement_has_expression(&cte.query, predicate))
		|| statement.unions.iter().any(|(_, union)| statement_has_expression(union, predicate))
		|| statement.from.iter().any(|table| table_has_expression(table, predicate))
		|| statement.join.iter().any(|join| {
			table_has_expression(&join.table, predicate)
				|| matches!(&join.on, Some(JoinOn::Condition(condition)) if conditions_match(&condition.conditions, predicate))
		})
}

fn statement_has_select(
	statement: &SelectStatement,
	predicate: &impl Fn(&SelectStatement) -> bool,
) -> bool {
	predicate(statement)
		|| statement.ctes.iter().any(|cte| statement_has_select(&cte.query, predicate))
		|| statement.unions.iter().any(|(_, union)| statement_has_select(union, predicate))
		|| statement.from.iter().any(|table| table_has_select(table, predicate))
		|| statement.join.iter().any(|join| {
			table_has_select(&join.table, predicate)
				|| matches!(&join.on, Some(JoinOn::Condition(condition)) if conditions_have_select(&condition.conditions, predicate))
		})
		|| statement.selects.iter().any(|select| expression_has_select(&select.expr, predicate))
		|| statement.groups.iter().any(|expression| expression_has_select(expression, predicate))
		|| conditions_have_select(&statement.r#where.conditions, predicate)
		|| conditions_have_select(&statement.having.conditions, predicate)
		|| statement.orders.iter().any(|order| matches!(&order.expr, OrderExprKind::Expr(expression) if expression_has_select(expression, predicate)))
		|| statement.windows.iter().any(|(_, window)| {
			window.partition_by.iter().any(|expression| expression_has_select(expression, predicate))
				|| window.order_by.iter().any(|order| matches!(&order.expr, OrderExprKind::Expr(expression) if expression_has_select(expression, predicate)))
		})
}

fn table_has_expression(table: &TableRef, predicate: &impl Fn(&SimpleExpr) -> bool) -> bool {
	matches!(table, TableRef::SubQuery(query, _) if statement_has_expression(query, predicate))
}

fn table_has_select(table: &TableRef, predicate: &impl Fn(&SelectStatement) -> bool) -> bool {
	matches!(table, TableRef::SubQuery(query, _) if statement_has_select(query, predicate))
}

fn conditions_have_select(
	conditions: &[ConditionExpression],
	predicate: &impl Fn(&SelectStatement) -> bool,
) -> bool {
	conditions.iter().any(|condition| match condition {
		ConditionExpression::SimpleExpr(expression) => expression_has_select(expression, predicate),
		ConditionExpression::Condition(condition) => {
			conditions_have_select(&condition.conditions, predicate)
		}
	})
}

fn conditions_match(
	conditions: &[ConditionExpression],
	predicate: &impl Fn(&SimpleExpr) -> bool,
) -> bool {
	conditions.iter().any(|condition| match condition {
		ConditionExpression::SimpleExpr(expression) => expression_matches(expression, predicate),
		ConditionExpression::Condition(condition) => {
			conditions_match(&condition.conditions, predicate)
		}
	})
}

fn expression_matches(expression: &SimpleExpr, predicate: &impl Fn(&SimpleExpr) -> bool) -> bool {
	predicate(expression)
		|| match expression {
			SimpleExpr::Unary(_, expression)
			| SimpleExpr::AsEnum(_, expression)
			| SimpleExpr::ExprAlias(expression, _)
			| SimpleExpr::Cast(expression, _)
			| SimpleExpr::WindowNamed {
				func: expression, ..
			} => expression_matches(expression, predicate),
		SimpleExpr::Window { func, window } => {
			expression_matches(func, predicate)
				|| window
					.partition_by
					.iter()
					.any(|expression| expression_matches(expression, predicate))
				|| window.order_by.iter().any(|order| {
					matches!(&order.expr, OrderExprKind::Expr(expression) if expression_matches(expression, predicate))
				})
		}
			SimpleExpr::SubQuery(_, query) => statement_has_expression(query, predicate),
			SimpleExpr::Binary(left, _, right) => {
				expression_matches(left, predicate) || expression_matches(right, predicate)
			}
			SimpleExpr::FunctionCall(_, expressions)
			| SimpleExpr::Tuple(expressions)
			| SimpleExpr::CustomWithExpr(_, expressions) => expressions
				.iter()
				.any(|expression| expression_matches(expression, predicate)),
			SimpleExpr::Case(case) => {
				case.when_clauses.iter().any(|(condition, result)| {
					expression_matches(condition, predicate)
						|| expression_matches(result, predicate)
				}) || case
					.else_clause
					.as_ref()
					.is_some_and(|result| expression_matches(result, predicate))
			}
			_ => false,
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

fn expression_has_select(
	expression: &SimpleExpr,
	predicate: &impl Fn(&SelectStatement) -> bool,
) -> bool {
	match expression {
		SimpleExpr::SubQuery(_, query) => statement_has_select(query, predicate),
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::WindowNamed {
			func: expression, ..
		} => expression_has_select(expression, predicate),
		SimpleExpr::Binary(left, _, right) => {
			expression_has_select(left, predicate) || expression_has_select(right, predicate)
		}
		SimpleExpr::FunctionCall(_, expressions)
		| SimpleExpr::Tuple(expressions)
		| SimpleExpr::CustomWithExpr(_, expressions) => expressions
			.iter()
			.any(|expression| expression_has_select(expression, predicate)),
		SimpleExpr::Case(case) => {
			case.when_clauses.iter().any(|(condition, result)| {
				expression_has_select(condition, predicate)
					|| expression_has_select(result, predicate)
			}) || case
				.else_clause
				.as_ref()
				.is_some_and(|result| expression_has_select(result, predicate))
		}
		SimpleExpr::Window { func, window } => {
			expression_has_select(func, predicate)
				|| window
					.partition_by
					.iter()
					.any(|expression| expression_has_select(expression, predicate))
				|| window.order_by.iter().any(|order| {
					matches!(&order.expr, OrderExprKind::Expr(expression) if expression_has_select(expression, predicate))
				})
		}
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Value(_)
		| SimpleExpr::Custom(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk => false,
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
		SimpleExpr::SubQuery(_, _) | SimpleExpr::Custom(_) => true,
		SimpleExpr::FunctionCall(function, expressions) => {
			!is_safe_aggregate_function(&function.to_string())
				|| expressions.iter().any(unsafe_expr)
		}
		SimpleExpr::CustomWithExpr(template, expressions) => {
			(!is_generated_like_template(template) && !is_structural_aggregate_template(template))
				|| expressions.iter().any(unsafe_expr)
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

/// Returns whether `template` is the limited SQL shape emitted by the typed
/// queryset `contains`, `startswith`, and `endswith` lookups.
///
/// Accepting only a quoted column path prevents this narrow exception from
/// allowing a caller-supplied custom expression to run during MySQL EXPLAIN.
fn is_generated_like_template(template: &str) -> bool {
	let Some(column_path) = template.strip_suffix(" LIKE ? ESCAPE '\\'") else {
		return false;
	};
	let mut remaining = column_path;
	loop {
		let Some(after_opening_quote) = remaining.strip_prefix('"') else {
			return false;
		};
		let mut index = 0;
		let bytes = after_opening_quote.as_bytes();
		let mut closed = false;
		while index < bytes.len() {
			if bytes[index] != b'"' {
				index += 1;
				continue;
			}
			if bytes.get(index + 1) == Some(&b'"') {
				index += 2;
				continue;
			}
			if index == 0 {
				return false;
			}
			remaining = &after_opening_quote[index + 1..];
			closed = true;
			break;
		}
		if !closed {
			return false;
		}
		if remaining.is_empty() {
			return true;
		}
		let Some(after_separator) = remaining.strip_prefix('.') else {
			return false;
		};
		remaining = after_separator;
	}
}

/// Returns whether `template` is the static AST template used for a typed
/// distinct aggregate annotation. Its sole expression is still checked
/// recursively, so this exception cannot make a custom expression executable
/// during MySQL EXPLAIN.
fn is_structural_aggregate_template(template: &str) -> bool {
	matches!(
		template,
		"COUNT(DISTINCT ?)"
			| "SUM(DISTINCT ?)"
			| "AVG(DISTINCT ?)"
			| "MIN(DISTINCT ?)"
			| "MAX(DISTINCT ?)"
	)
}

/// Returns whether a function is one of the side-effect-free aggregate forms
/// emitted by typed annotations. Its arguments remain subject to the regular
/// recursive safety check.
fn is_safe_aggregate_function(function: &str) -> bool {
	matches!(function, "COUNT" | "SUM" | "AVG" | "MIN" | "MAX")
}

fn quote_mysql_like_templates(statement: &mut SelectStatement) {
	for cte in &mut statement.ctes {
		quote_mysql_like_templates(&mut cte.query);
	}
	for select in &mut statement.selects {
		quote_mysql_like_template_expr(&mut select.expr);
	}
	for table in &mut statement.from {
		quote_mysql_like_template_table(table);
	}
	for join in &mut statement.join {
		quote_mysql_like_template_table(&mut join.table);
		if let Some(JoinOn::Condition(condition)) = &mut join.on {
			quote_mysql_like_template_conditions(&mut condition.conditions);
		}
	}
	quote_mysql_like_template_conditions(&mut statement.r#where.conditions);
	for group in &mut statement.groups {
		quote_mysql_like_template_expr(group);
	}
	quote_mysql_like_template_conditions(&mut statement.having.conditions);
	for (_, union) in &mut statement.unions {
		quote_mysql_like_templates(union);
	}
	for order in &mut statement.orders {
		quote_mysql_like_template_order(order);
	}
	for (_, window) in &mut statement.windows {
		quote_mysql_like_template_window(window);
	}
}

fn quote_mysql_like_template_table(table: &mut TableRef) {
	if let TableRef::SubQuery(query, _) = table {
		quote_mysql_like_templates(query);
	}
}

fn quote_mysql_like_template_conditions(conditions: &mut [ConditionExpression]) {
	for condition in conditions {
		match condition {
			ConditionExpression::SimpleExpr(expression) => {
				quote_mysql_like_template_expr(expression)
			}
			ConditionExpression::Condition(condition) => {
				quote_mysql_like_template_conditions(&mut condition.conditions);
			}
		}
	}
}

fn quote_mysql_like_template_order(order: &mut OrderExpr) {
	if let OrderExprKind::Expr(expression) = &mut order.expr {
		quote_mysql_like_template_expr(expression);
	}
}

fn quote_mysql_like_template_window(window: &mut WindowStatement) {
	for partition in &mut window.partition_by {
		quote_mysql_like_template_expr(partition);
	}
	for order in &mut window.order_by {
		quote_mysql_like_template_order(order);
	}
}

fn quote_mysql_like_template_expr(expression: &mut SimpleExpr) {
	match expression {
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::WindowNamed {
			func: expression, ..
		} => quote_mysql_like_template_expr(expression),
		SimpleExpr::Binary(left, _, right) => {
			quote_mysql_like_template_expr(left);
			quote_mysql_like_template_expr(right);
		}
		SimpleExpr::FunctionCall(_, expressions) | SimpleExpr::Tuple(expressions) => {
			for expression in expressions {
				quote_mysql_like_template_expr(expression);
			}
		}
		SimpleExpr::SubQuery(_, query) => quote_mysql_like_templates(query),
		SimpleExpr::CustomWithExpr(template, expressions) => {
			for expression in expressions {
				quote_mysql_like_template_expr(expression);
			}
			if is_generated_like_template(template) {
				*template = translate_generated_like_template_to_mysql(template);
			}
		}
		SimpleExpr::Case(case) => {
			for (condition, result) in &mut case.when_clauses {
				quote_mysql_like_template_expr(condition);
				quote_mysql_like_template_expr(result);
			}
			if let Some(result) = &mut case.else_clause {
				quote_mysql_like_template_expr(result);
			}
		}
		SimpleExpr::Window { func, window } => {
			quote_mysql_like_template_expr(func);
			quote_mysql_like_template_window(window);
		}
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Value(_)
		| SimpleExpr::Custom(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk => {}
	}
}

/// Converts a validated ORM LIKE template to MySQL syntax.
///
/// `is_generated_like_template` has already validated the input as a sequence
/// of SQL-standard quoted identifier components followed by the fixed LIKE
/// suffix. Parsing those components preserves a doubled double quote as a
/// literal identifier character, while doubling embedded backticks prevents
/// them from terminating a MySQL identifier. MySQL's default backslash-escape
/// mode also requires two backslashes in the ESCAPE string literal.
fn translate_generated_like_template_to_mysql(template: &str) -> String {
	let column_path = template
		.strip_suffix(" LIKE ? ESCAPE '\\'")
		.expect("generated LIKE templates have the validated suffix");
	let quoted_columns = parse_generated_like_identifier_components(column_path)
		.expect("generated LIKE templates contain validated identifier components")
		.into_iter()
		.map(|component| format!("`{}`", component.replace('`', "``")))
		.collect::<Vec<_>>()
		.join(".");
	format!("{quoted_columns} LIKE ? ESCAPE '\\\\'")
}

/// Parses SQL-standard quoted identifier components from a validated LIKE
/// template, unescaping doubled double quotes within each component.
fn parse_generated_like_identifier_components(column_path: &str) -> Option<Vec<String>> {
	let mut components = Vec::new();
	let mut remaining = column_path;

	loop {
		let mut characters = remaining.strip_prefix('"')?.chars();
		let mut component = String::new();
		loop {
			match characters.next()? {
				'"' if characters.as_str().starts_with('"') => {
					component.push('"');
					characters.next();
				}
				'"' => break,
				character => component.push(character),
			}
		}
		components.push(component);
		remaining = characters.as_str();
		if remaining.is_empty() {
			return Some(components);
		}
		remaining = remaining.strip_prefix('.')?;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		expr::{Expr, ExprTrait},
		query::Query,
		types::{Frame, FrameClause, FrameType, Order, OrderExpr, OrderExprKind, WindowStatement},
	};
	use rstest::rstest;

	fn filtered_select() -> SelectStatement {
		Query::select()
			.column("id")
			.from("users")
			.and_where(Expr::col("active").eq(true))
			.to_owned()
	}

	#[rstest]
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

	#[rstest]
	fn mysql_build_rejects_tree_format_for_mariadb_compatibility() {
		let statement = ExplainStatement::new(
			filtered_select(),
			ExplainOptions {
				format: ExplainFormat::Tree,
				..ExplainOptions::default()
			},
		);

		assert_eq!(
			statement.build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "EXPLAIN FORMAT TREE",
				backend: "MySQL/MariaDB",
			})
		);
	}

	#[rstest]
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

	#[rstest]
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

	#[rstest]
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

	#[rstest]
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

	#[rstest]
	fn sqlite_explain_rejects_groups_window_frames_before_rendering() {
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

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_sqlite_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "GROUPS window frames",
				backend: "SQLite",
			})
		);
	}

	#[rstest]
	fn mysql_explain_rejects_nulls_window_ordering_before_rendering() {
		let mut select = filtered_select();
		select.window_as(
			"ranked",
			WindowStatement {
				partition_by: Vec::new(),
				order_by: vec![OrderExpr::new("id").nulls(crate::types::NullOrdering::First)],
				frame: None,
			},
		);

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "NULLS FIRST/LAST in window ordering",
				backend: "MySQL",
			})
		);
	}

	#[rstest]
	fn mysql_explain_rejects_inline_nulls_window_ordering_before_rendering() {
		let window = WindowStatement {
			partition_by: Vec::new(),
			order_by: vec![OrderExpr::new("id").nulls(crate::types::NullOrdering::Last)],
			frame: None,
		};
		let select = Query::select()
			.expr(Expr::row_number().over(window))
			.from("users")
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "NULLS FIRST/LAST in window ordering",
				backend: "MySQL",
			})
		);
	}

	#[rstest]
	fn sqlite_explain_rejects_groups_window_frame_in_nested_select() {
		let mut nested = filtered_select();
		nested.window_as(
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
		let select = Query::select()
			.column("id")
			.from_subquery(nested, "ranked_users")
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_sqlite_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "GROUPS window frames",
				backend: "SQLite",
			})
		);
	}

	#[rstest]
	fn sqlite_explain_rejects_generated_ilike_template_before_rendering() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(Expr::cust_with_values(
				"\"username\" ILIKE ? ESCAPE '\\'",
				["ada"],
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_sqlite_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "ILIKE",
				backend: "SQLite",
			})
		);
	}

	#[rstest]
	fn postgres_explain_rejects_lock_in_nested_select() {
		let mut nested = filtered_select();
		nested.lock_exclusive();
		let select = Query::select()
			.column("id")
			.from_subquery(nested, "locked_users")
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_postgres_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "SELECT lock clauses in EXPLAIN",
				backend: "PostgreSQL",
			})
		);
	}

	#[rstest]
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

	#[rstest]
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

	#[rstest]
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

	#[rstest]
	fn mysql_explain_allows_typed_like_lookup_template() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(Expr::cust_with_values(
				"\"username\" LIKE ? ESCAPE '\\'",
				["ada"],
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default())
				.build_mysql_checked()
				.expect("typed LIKE lookup should be safe to explain")
				.0,
			"EXPLAIN SELECT `id` FROM `users` WHERE `username` LIKE ? ESCAPE '\\\\'"
		);
	}

	#[rstest]
	fn mysql_explain_preserves_embedded_double_quotes_in_typed_like_identifiers() {
		let select = Query::select()
			.column(crate::types::Alias::new("id\"value"))
			.from(crate::types::Alias::new("users\"archive"))
			.and_where(Expr::cust_with_values(
				"\"user\"\"name\" LIKE ? ESCAPE '\\'",
				["ada"],
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default())
				.build_mysql_checked()
				.expect("typed LIKE lookup should be safe to explain")
				.0,
			"EXPLAIN SELECT `id\"value` FROM `users\"archive` WHERE `user\"name` LIKE ? ESCAPE '\\\\'"
		);
	}

	#[rstest]
	fn mysql_explain_escapes_embedded_backticks_in_typed_like_identifiers() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(Expr::cust_with_values(
				"\"user`name\" LIKE ? ESCAPE '\\'",
				["ada"],
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default())
				.build_mysql_checked()
				.expect("typed LIKE lookup should be safe to explain")
				.0,
			"EXPLAIN SELECT `id` FROM `users` WHERE `user``name` LIKE ? ESCAPE '\\\\'"
		);
	}

	#[rstest]
	fn mysql_explain_rejects_ilike_in_inline_window_partition_before_rendering() {
		let window = WindowStatement {
			partition_by: vec![SimpleExpr::Binary(
				Box::new(Expr::col("username").into_simple_expr()),
				BinOper::ILike,
				Box::new(Expr::val("ada").into_simple_expr()),
			)],
			order_by: Vec::new(),
			frame: None,
		};
		let select = Query::select()
			.expr(Expr::row_number().over(window))
			.from("users")
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "ILIKE",
				backend: "MySQL",
			})
		);
	}

	#[rstest]
	fn sqlite_explain_rejects_postgres_operator_in_inline_window_ordering_before_rendering() {
		let window = WindowStatement {
			partition_by: Vec::new(),
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Expr(Box::new(SimpleExpr::Binary(
					Box::new(Expr::col("attributes").into_simple_expr()),
					BinOper::PgOperator(crate::types::PgBinOper::Contains),
					Box::new(Expr::val("admin").into_simple_expr()),
				))),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};
		let select = Query::select()
			.expr(Expr::row_number().over(window))
			.from("users")
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_sqlite_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "PostgreSQL operators",
				backend: "SQLite",
			})
		);
	}

	#[rstest]
	fn mysql_explain_rejects_postgres_pattern_operators_before_rendering() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(SimpleExpr::Binary(
				Box::new(Expr::col("username").into_simple_expr()),
				BinOper::SimilarTo,
				Box::new(Expr::val("ada").into_simple_expr()),
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "PostgreSQL pattern operators",
				backend: "MySQL",
			})
		);
	}

	#[rstest]
	fn sqlite_explain_rejects_postgres_pattern_operators_before_rendering() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(SimpleExpr::Binary(
				Box::new(Expr::col("username").into_simple_expr()),
				BinOper::Matches,
				Box::new(Expr::val("ada").into_simple_expr()),
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_sqlite_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "PostgreSQL pattern operators",
				backend: "SQLite",
			})
		);
	}

	#[rstest]
	fn sqlite_explain_rejects_postgres_operators_before_rendering() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(SimpleExpr::Binary(
				Box::new(Expr::col("attributes").into_simple_expr()),
				BinOper::PgOperator(crate::types::PgBinOper::Contains),
				Box::new(Expr::val("admin").into_simple_expr()),
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_sqlite_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "PostgreSQL operators",
				backend: "SQLite",
			})
		);
	}

	#[rstest]
	fn mysql_explain_rejects_postgres_operators_before_rendering() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(SimpleExpr::Binary(
				Box::new(Expr::col("attributes").into_simple_expr()),
				BinOper::PgOperator(crate::types::PgBinOper::Contains),
				Box::new(Expr::val("admin").into_simple_expr()),
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "PostgreSQL operators",
				backend: "MySQL",
			})
		);
	}

	#[rstest]
	fn mysql_explain_rejects_custom_like_template() {
		let select = Query::select()
			.column("id")
			.from("users")
			.and_where(Expr::cust_with_values(
				"LOWER(\"username\") LIKE ? ESCAPE '\\'",
				["ada"],
			))
			.to_owned();

		assert_eq!(
			ExplainStatement::new(select, ExplainOptions::default()).build_mysql_checked(),
			Err(QueryBuildError::UnsupportedBackendFeature {
				feature: "plan-only EXPLAIN for subqueries or unchecked expressions",
				backend: "MySQL",
			})
		);
	}
}
