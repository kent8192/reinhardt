//! PostgreSQL query builder backend
//!
//! This module implements the SQL generation backend for PostgreSQL.
//!
//! # SQL Identifier Quoting
//!
//! PostgreSQL uses **double quotes** (`"`) for SQL identifiers (table names, column names,
//! index names, etc.). This is different from string literals, which use single quotes (`'`).
//!
//! ## Quoting Behavior
//!
//! - All identifiers are automatically wrapped in double quotes
//! - Double quotes within identifiers are escaped by doubling (`"` becomes `""`)
//! - Case sensitivity is preserved when identifiers are quoted
//!
//! ## Examples
//!
//! | Input Identifier | Quoted Output |
//! |-----------------|---------------|
//! | `users` | `"users"` |
//! | `user_name` | `"user_name"` |
//! | `column"with"quotes` | `"column""with""quotes"` |
//!
//! ## Testing Generated SQL
//!
//! When writing tests for generated SQL, ensure you account for identifier quoting:
//!
//! ```rust,ignore
//! use reinhardt_query::backend::{PostgresQueryBuilder, QueryBuilder};
//! use reinhardt_query::prelude::*;
//!
//! let builder = PostgresQueryBuilder::new();
//! let stmt = Query::select().column("name").from("users");
//! let (sql, _) = builder.build_select(&stmt);
//!
//! // Note the double quotes around identifiers
//! assert_eq!(sql, r#"SELECT "name" FROM "users""#);
//! ```
//!
//! For more details on SQL syntax, see the
//! [PostgreSQL Documentation](https://www.postgresql.org/docs/current/sql-syntax-lexical.html#SQL-SYNTAX-IDENTIFIERS).

use super::{QueryBuilder, SqlWriter};
use crate::{
	expr::{Condition, SimpleExpr},
	query::{
		AlterIndexStatement, AlterTableOperation, AlterTableStatement, CheckTableStatement,
		CreateIndexStatement, CreateTableStatement, CreateTriggerStatement, CreateViewStatement,
		DeleteStatement, DropIndexStatement, DropTableStatement, DropTriggerStatement,
		DropViewStatement, InsertStatement, OptimizeTableStatement, ReindexStatement,
		RepairTableStatement, SelectStatement, TruncateTableStatement, UpdateStatement,
	},
	types::{BinOper, ColumnRef, TableRef, TriggerBody},
	value::Values,
};

/// PostgreSQL query builder
///
/// This struct implements SQL generation for PostgreSQL, using the following conventions:
/// - Identifiers: Double quotes (`"table_name"`)
/// - Placeholders: Numbered (`$1`, `$2`, ...)
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::backend::{PostgresQueryBuilder, QueryBuilder};
/// use reinhardt_query::prelude::*;
///
/// let builder = PostgresQueryBuilder::new();
/// let stmt = Query::select()
///     .column("id")
///     .from("users");
///
/// let (sql, values) = builder.build_select(&stmt);
/// // sql: SELECT "id" FROM "users"
/// ```
#[derive(Debug, Clone, Default)]
pub struct PostgresQueryBuilder;

impl PostgresQueryBuilder {
	/// Create a new PostgreSQL query builder
	pub fn new() -> Self {
		Self
	}

	/// Escape an identifier for PostgreSQL
	///
	/// PostgreSQL uses double quotes for identifiers.
	///
	/// # Arguments
	///
	/// * `ident` - The identifier to escape
	///
	/// # Returns
	///
	/// The escaped identifier (e.g., `"user"`)
	fn escape_iden(&self, ident: &str) -> String {
		// Escape double quotes within the identifier
		let escaped = ident.replace('"', "\"\"");
		format!("\"{}\"", escaped)
	}

	/// Format a placeholder for PostgreSQL
	///
	/// PostgreSQL uses numbered placeholders ($1, $2, ...).
	///
	/// # Arguments
	///
	/// * `index` - The parameter index (1-based)
	///
	/// # Returns
	///
	/// The placeholder string (e.g., `$1`)
	fn placeholder(&self, index: usize) -> String {
		format!("${}", index)
	}

	/// Write a table reference
	fn write_table_ref(&self, writer: &mut SqlWriter, table_ref: &TableRef) {
		match table_ref {
			TableRef::Table(iden) => {
				writer.push_identifier(&iden.to_string(), |s| self.escape_iden(s));
			}
			TableRef::SchemaTable(schema, table) => {
				writer.push_identifier(&schema.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
			}
			TableRef::DatabaseSchemaTable(db, schema, table) => {
				writer.push_identifier(&db.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&schema.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
			}
			TableRef::TableAlias(table, alias) => {
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push_keyword("AS");
				writer.push_space();
				writer.push_identifier(&alias.to_string(), |s| self.escape_iden(s));
			}
			TableRef::SchemaTableAlias(schema, table, alias) => {
				writer.push_identifier(&schema.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push_keyword("AS");
				writer.push_space();
				writer.push_identifier(&alias.to_string(), |s| self.escape_iden(s));
			}
			TableRef::SubQuery(query, alias) => {
				let (subquery_sql, subquery_values) = self.build_select(query);

				// Adjust placeholders for PostgreSQL ($1, $2, ... -> $N, $N+1, ...)
				let offset = writer.param_index() - 1;
				let adjusted_sql = if offset > 0 {
					let mut sql = subquery_sql;
					for i in (1..=subquery_values.len()).rev() {
						sql = sql.replace(&format!("${}", i), &format!("${}", i + offset));
					}
					sql
				} else {
					subquery_sql
				};

				writer.push("(");
				writer.push(&adjusted_sql);
				writer.push(")");
				writer.push_keyword("AS");
				writer.push_space();
				writer.push_identifier(&alias.to_string(), |s| self.escape_iden(s));

				// Merge the values from the subquery
				writer.append_values(&subquery_values);
			}
		}
	}

	/// Write a column reference
	fn write_column_ref(&self, writer: &mut SqlWriter, col_ref: &ColumnRef) {
		match col_ref {
			ColumnRef::Column(iden) => {
				writer.push_identifier(&iden.to_string(), |s| self.escape_iden(s));
			}
			ColumnRef::TableColumn(table, col) => {
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&col.to_string(), |s| self.escape_iden(s));
			}
			ColumnRef::SchemaTableColumn(schema, table, col) => {
				writer.push_identifier(&schema.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&col.to_string(), |s| self.escape_iden(s));
			}
			ColumnRef::Asterisk => {
				writer.push("*");
			}
			ColumnRef::TableAsterisk(table) => {
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push(".*");
			}
		}
	}

	/// Write a simple expression
	fn write_simple_expr(&self, writer: &mut SqlWriter, expr: &SimpleExpr) {
		match expr {
			SimpleExpr::Column(col_ref) => {
				self.write_column_ref(writer, col_ref);
			}
			SimpleExpr::Value(value) => {
				writer.push_value(value.clone(), |i| self.placeholder(i));
			}
			SimpleExpr::Binary(left, op, right) => match (op, right.as_ref()) {
				(BinOper::Between | BinOper::NotBetween, SimpleExpr::Tuple(items))
					if items.len() == 2 =>
				{
					self.write_simple_expr(writer, left);
					writer.push_space();
					writer.push(op.as_str());
					writer.push_space();
					self.write_simple_expr(writer, &items[0]);
					writer.push(" AND ");
					self.write_simple_expr(writer, &items[1]);
				}
				(BinOper::In | BinOper::NotIn, SimpleExpr::Tuple(items)) => {
					self.write_simple_expr(writer, left);
					writer.push_space();
					writer.push(op.as_str());
					writer.push(" (");
					writer.push_list(items, ", ", |w, item| {
						self.write_simple_expr(w, item);
					});
					writer.push(")");
				}
				_ => {
					self.write_simple_expr(writer, left);
					writer.push_space();
					writer.push(op.as_str());
					writer.push_space();
					self.write_simple_expr(writer, right);
				}
			},
			SimpleExpr::Unary(op, expr) => {
				writer.push(op.as_str());
				writer.push_space();
				self.write_simple_expr(writer, expr);
			}
			SimpleExpr::FunctionCall(func_name, args) => {
				writer.push(&func_name.to_string());
				writer.push("(");
				writer.push_list(args, ", ", |w, arg| {
					self.write_simple_expr(w, arg);
				});
				writer.push(")");
			}
			SimpleExpr::Constant(val) => {
				writer.push(val.as_str());
			}
			SimpleExpr::SubQuery(op, select_stmt) => {
				use crate::expr::SubQueryOper;

				// Write the operator keyword if present
				if let Some(operator) = op {
					match operator {
						SubQueryOper::Exists => {
							writer.push("EXISTS");
							writer.push_space();
						}
						SubQueryOper::NotExists => {
							writer.push("NOT EXISTS");
							writer.push_space();
						}
						SubQueryOper::In | SubQueryOper::NotIn => {
							// These are handled in Binary expressions
						}
						SubQueryOper::All => {
							writer.push("ALL");
							writer.push_space();
						}
						SubQueryOper::Any => {
							writer.push("ANY");
							writer.push_space();
						}
						SubQueryOper::Some => {
							writer.push("SOME");
							writer.push_space();
						}
					}
				}

				// Write the subquery wrapped in parentheses
				writer.push("(");

				// Recursively build the subquery
				let (subquery_sql, subquery_values) = self.build_select(select_stmt);

				// Adjust placeholders for PostgreSQL ($1, $2, ... -> $N, $N+1, ...)
				let offset = writer.param_index() - 1;
				let adjusted_sql = if offset > 0 {
					let mut sql = subquery_sql;
					for i in (1..=subquery_values.len()).rev() {
						sql = sql.replace(&format!("${}", i), &format!("${}", i + offset));
					}
					sql
				} else {
					subquery_sql
				};

				writer.push(&adjusted_sql);
				writer.push(")");

				// Merge the values from the subquery
				writer.append_values(&subquery_values);
			}
			SimpleExpr::Window { func, window } => {
				// Write the function
				self.write_simple_expr(writer, func);
				writer.push_space();
				writer.push_keyword("OVER");
				writer.push_space();
				writer.push("(");
				self.write_window_statement(writer, window);
				writer.push(")");
			}
			SimpleExpr::WindowNamed { func, name } => {
				// Write the function
				self.write_simple_expr(writer, func);
				writer.push_space();
				writer.push_keyword("OVER");
				writer.push_space();
				writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
			}
			SimpleExpr::Tuple(items) => {
				writer.push("(");
				writer.push_list(items, ", ", |w, item| {
					self.write_simple_expr(w, item);
				});
				writer.push(")");
			}
			SimpleExpr::Case(case) => {
				writer.push_keyword("CASE");
				for (condition, result) in &case.when_clauses {
					writer.push_space();
					writer.push_keyword("WHEN");
					writer.push_space();
					self.write_simple_expr(writer, condition);
					writer.push_space();
					writer.push_keyword("THEN");
					writer.push_space();
					self.write_simple_expr(writer, result);
				}
				if let Some(else_result) = &case.else_clause {
					writer.push_space();
					writer.push_keyword("ELSE");
					writer.push_space();
					self.write_simple_expr(writer, else_result);
				}
				writer.push_space();
				writer.push_keyword("END");
			}
			SimpleExpr::Custom(sql) => {
				writer.push(sql);
			}
			SimpleExpr::CustomWithExpr(template, exprs) => {
				// Replace `?` placeholders with the rendered expressions
				let mut parts = template.split('?');
				if let Some(first) = parts.next() {
					writer.push(first);
				}
				let mut expr_iter = exprs.iter();
				for part in parts {
					if let Some(expr) = expr_iter.next() {
						self.write_simple_expr(writer, expr);
					}
					writer.push(part);
				}
			}
			SimpleExpr::Asterisk => {
				writer.push("*");
			}
			SimpleExpr::TableColumn(table, col) => {
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&col.to_string(), |s| self.escape_iden(s));
			}
			SimpleExpr::AsEnum(name, expr) => {
				self.write_simple_expr(writer, expr);
				writer.push("::");
				writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
			}
			SimpleExpr::Cast(expr, type_name) => {
				writer.push("CAST(");
				self.write_simple_expr(writer, expr);
				writer.push(" AS ");
				writer.push_identifier(&type_name.to_string(), |s| self.escape_iden(s));
				writer.push(")");
			}
		}
	}

	/// Write a simple expression with values inlined (no placeholders)
	///
	/// This is used for CHECK constraints in DDL statements, which cannot use
	/// parameterized queries. Values are written directly as SQL literals.
	fn write_simple_expr_unquoted(&self, writer: &mut SqlWriter, expr: &SimpleExpr) {
		use crate::types::BinOper;
		use crate::value::Value;

		match expr {
			SimpleExpr::Column(col_ref) => {
				self.write_column_ref(writer, col_ref);
			}
			SimpleExpr::Value(value) => {
				// Write values inline as SQL literals instead of using placeholders
				match value {
					Value::Int(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::BigInt(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::TinyInt(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::SmallInt(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::Unsigned(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::SmallUnsigned(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::TinyUnsigned(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::BigUnsigned(Some(n)) => {
						writer.push(&n.to_string());
					}
					Value::String(Some(s)) => {
						let escaped = s.as_str().replace('\'', "''");
						writer.push(&format!("'{}'", escaped));
					}
					Value::Bool(Some(b)) => {
						writer.push(if *b { "TRUE" } else { "FALSE" });
					}
					Value::Float(Some(f)) => {
						writer.push(&f.to_string());
					}
					Value::Double(Some(d)) => {
						writer.push(&d.to_string());
					}
					// None values render as NULL
					_ => {
						writer.push("NULL");
					}
				}
			}
			SimpleExpr::Binary(left, op, right) => match (op, right.as_ref()) {
				(BinOper::Between | BinOper::NotBetween, SimpleExpr::Tuple(items))
					if items.len() == 2 =>
				{
					self.write_simple_expr_unquoted(writer, left);
					writer.push_space();
					writer.push(op.as_str());
					writer.push_space();
					self.write_simple_expr_unquoted(writer, &items[0]);
					writer.push(" AND ");
					self.write_simple_expr_unquoted(writer, &items[1]);
				}
				(BinOper::In | BinOper::NotIn, SimpleExpr::Tuple(items)) => {
					self.write_simple_expr_unquoted(writer, left);
					writer.push_space();
					writer.push(op.as_str());
					writer.push(" (");
					writer.push_list(items, ", ", |w, item| {
						self.write_simple_expr_unquoted(w, item);
					});
					writer.push(")");
				}
				_ => {
					self.write_simple_expr_unquoted(writer, left);
					writer.push_space();
					writer.push(op.as_str());
					writer.push_space();
					self.write_simple_expr_unquoted(writer, right);
				}
			},
			SimpleExpr::Unary(op, expr) => {
				writer.push(op.as_str());
				writer.push_space();
				self.write_simple_expr_unquoted(writer, expr);
			}
			SimpleExpr::FunctionCall(func_name, args) => {
				writer.push(&func_name.to_string());
				writer.push("(");
				writer.push_list(args, ", ", |w, arg| {
					self.write_simple_expr_unquoted(w, arg);
				});
				writer.push(")");
			}
			SimpleExpr::Constant(val) => {
				writer.push(val.as_str());
			}
			SimpleExpr::Tuple(items) => {
				writer.push("(");
				writer.push_list(items, ", ", |w, item| {
					self.write_simple_expr_unquoted(w, item);
				});
				writer.push(")");
			}
			SimpleExpr::Case(case) => {
				writer.push_keyword("CASE");
				for (condition, result) in &case.when_clauses {
					writer.push_space();
					writer.push_keyword("WHEN");
					writer.push_space();
					self.write_simple_expr_unquoted(writer, condition);
					writer.push_space();
					writer.push_keyword("THEN");
					writer.push_space();
					self.write_simple_expr_unquoted(writer, result);
				}
				if let Some(else_result) = &case.else_clause {
					writer.push_space();
					writer.push_keyword("ELSE");
					writer.push_space();
					self.write_simple_expr_unquoted(writer, else_result);
				}
				writer.push_space();
				writer.push_keyword("END");
			}
			// Subqueries are not supported in CHECK constraints
			SimpleExpr::SubQuery(_, _) => {
				writer.push("(TRUE)");
			}
			// Window expressions are not supported in CHECK constraints
			SimpleExpr::Window { .. } | SimpleExpr::WindowNamed { .. } => {
				writer.push("(TRUE)");
			}
			SimpleExpr::Custom(sql) => {
				writer.push(sql);
			}
			SimpleExpr::CustomWithExpr(template, exprs) => {
				let mut parts = template.split('?');
				if let Some(first) = parts.next() {
					writer.push(first);
				}
				let mut expr_iter = exprs.iter();
				for part in parts {
					if let Some(expr) = expr_iter.next() {
						self.write_simple_expr_unquoted(writer, expr);
					}
					writer.push(part);
				}
			}
			SimpleExpr::Asterisk => {
				writer.push("*");
			}
			SimpleExpr::TableColumn(table, col) => {
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&col.to_string(), |s| self.escape_iden(s));
			}
			SimpleExpr::AsEnum(name, expr) => {
				self.write_simple_expr_unquoted(writer, expr);
				writer.push("::");
				writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
			}
			SimpleExpr::Cast(expr, type_name) => {
				writer.push("CAST(");
				self.write_simple_expr_unquoted(writer, expr);
				writer.push(" AS ");
				writer.push_identifier(&type_name.to_string(), |s| self.escape_iden(s));
				writer.push(")");
			}
		}
	}

	/// Write a condition
	fn write_condition(&self, writer: &mut SqlWriter, condition: &Condition) {
		use crate::expr::ConditionType;

		if condition.conditions.is_empty() {
			return;
		}

		if condition.negate {
			writer.push("NOT ");
		}

		if condition.conditions.len() == 1 {
			self.write_condition_expr(writer, &condition.conditions[0]);
			return;
		}

		writer.push("(");
		let separator = match condition.condition_type {
			ConditionType::All => " AND ",
			ConditionType::Any => " OR ",
		};
		writer.push_list(&condition.conditions, separator, |w, cond_expr| {
			self.write_condition_expr(w, cond_expr);
		});
		writer.push(")");
	}

	/// Write a condition expression
	fn write_condition_expr(
		&self,
		writer: &mut SqlWriter,
		cond_expr: &crate::expr::ConditionExpression,
	) {
		use crate::expr::ConditionExpression;

		match cond_expr {
			ConditionExpression::Condition(cond) => {
				self.write_condition(writer, cond);
			}
			ConditionExpression::SimpleExpr(expr) => {
				self.write_simple_expr(writer, expr);
			}
		}
	}

	/// Write a JOIN expression
	fn write_join_expr(&self, writer: &mut SqlWriter, join: &crate::types::JoinExpr) {
		use crate::types::JoinOn;

		// JOIN type
		writer.push_keyword(join.join.as_str());
		writer.push_space();

		// Table
		self.write_table_ref(writer, &join.table);

		// ON or USING clause
		if let Some(on) = &join.on {
			match on {
				JoinOn::Columns(pair) => {
					writer.push_keyword("ON");
					writer.push_space();
					self.write_column_spec(writer, &pair.left);
					writer.push(" = ");
					self.write_column_spec(writer, &pair.right);
				}
				JoinOn::Condition(cond) => {
					writer.push_keyword("ON");
					writer.push_space();
					self.write_condition(writer, cond);
				}
				JoinOn::Using(cols) => {
					writer.push_keyword("USING");
					writer.push_space();
					writer.push("(");
					writer.push_list(cols, ", ", |w, col| {
						w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
					});
					writer.push(")");
				}
			}
		}
	}

	/// Write a column specification (for JOIN ON conditions)
	fn write_column_spec(&self, writer: &mut SqlWriter, spec: &crate::types::ColumnSpec) {
		match spec {
			crate::types::ColumnSpec::Column(iden) => {
				writer.push_identifier(&iden.to_string(), |s| self.escape_iden(s));
			}
			crate::types::ColumnSpec::TableColumn(table, col) => {
				writer.push_identifier(&table.to_string(), |s| self.escape_iden(s));
				writer.push(".");
				writer.push_identifier(&col.to_string(), |s| self.escape_iden(s));
			}
		}
	}

	/// Write a window statement (PARTITION BY, ORDER BY, frame clause)
	fn write_window_statement(
		&self,
		writer: &mut SqlWriter,
		window: &crate::types::WindowStatement,
	) {
		// PARTITION BY clause
		if !window.partition_by.is_empty() {
			writer.push_keyword("PARTITION BY");
			writer.push_space();
			writer.push_list(&window.partition_by, ", ", |w, expr| {
				self.write_simple_expr(w, expr);
			});
			writer.push_space();
		}

		// ORDER BY clause
		if !window.order_by.is_empty() {
			writer.push_keyword("ORDER BY");
			writer.push_space();
			writer.push_list(&window.order_by, ", ", |w, order_expr| {
				use crate::types::OrderExprKind;
				match &order_expr.expr {
					OrderExprKind::Column(iden) => {
						w.push_identifier(&iden.to_string(), |s| self.escape_iden(s));
					}
					OrderExprKind::TableColumn(table, col) => {
						w.push_identifier(&table.to_string(), |s| self.escape_iden(s));
						w.push(".");
						w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
					}
					OrderExprKind::Expr(expr) => {
						self.write_simple_expr(w, expr);
					}
				}
				match order_expr.order {
					crate::types::Order::Asc => {
						w.push_keyword("ASC");
					}
					crate::types::Order::Desc => {
						w.push_keyword("DESC");
					}
				}
				if let Some(nulls) = order_expr.nulls {
					w.push_space();
					w.push(nulls.as_str());
				}
			});
			writer.push_space();
		}

		// Frame clause
		if let Some(frame) = &window.frame {
			self.write_frame_clause(writer, frame);
		}
	}

	/// Write a frame clause (ROWS/RANGE/GROUPS BETWEEN ... AND ...)
	fn write_frame_clause(&self, writer: &mut SqlWriter, frame: &crate::types::FrameClause) {
		use crate::types::FrameType;

		// Frame type (ROWS, RANGE, GROUPS)
		match frame.frame_type {
			FrameType::Rows => writer.push_keyword("ROWS"),
			FrameType::Range => writer.push_keyword("RANGE"),
			FrameType::Groups => writer.push_keyword("GROUPS"),
		}
		writer.push_space();

		// Frame specification
		if let Some(end) = &frame.end {
			writer.push_keyword("BETWEEN");
			writer.push_space();
			self.write_frame_boundary(writer, &frame.start);
			writer.push_keyword("AND");
			writer.push_space();
			self.write_frame_boundary(writer, end);
		} else {
			self.write_frame_boundary(writer, &frame.start);
		}
	}

	/// Write a frame boundary (UNBOUNDED PRECEDING, CURRENT ROW, etc.)
	fn write_frame_boundary(&self, writer: &mut SqlWriter, frame: &crate::types::Frame) {
		use crate::types::Frame;
		match frame {
			Frame::UnboundedPreceding => writer.push("UNBOUNDED PRECEDING"),
			Frame::Preceding(n) => {
				writer.push(&n.to_string());
				writer.push(" PRECEDING");
			}
			Frame::CurrentRow => writer.push("CURRENT ROW"),
			Frame::Following(n) => {
				writer.push(&n.to_string());
				writer.push(" FOLLOWING");
			}
			Frame::UnboundedFollowing => writer.push("UNBOUNDED FOLLOWING"),
		}
	}
}

impl QueryBuilder for PostgresQueryBuilder {
	fn build_select(&self, stmt: &SelectStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// WITH clause (Common Table Expressions)
		if !stmt.ctes.is_empty() {
			// Check if any CTE is recursive
			let has_recursive = stmt.ctes.iter().any(|cte| cte.recursive);

			// Write WITH [RECURSIVE]
			writer.push_keyword("WITH");
			writer.push_space();
			if has_recursive {
				writer.push_keyword("RECURSIVE");
				writer.push_space();
			}

			// Write each CTE
			writer.push_list(&stmt.ctes, ", ", |w, cte| {
				// CTE name
				w.push_identifier(&cte.name.to_string(), |s| self.escape_iden(s));
				w.push_space();
				w.push_keyword("AS");
				w.push_space();
				w.push("(");

				// Recursively build the CTE query
				let (cte_sql, cte_values) = self.build_select(&cte.query);

				// Adjust placeholders for PostgreSQL ($1, $2, ... -> $N, $N+1, ...)
				let offset = w.param_index() - 1;
				let adjusted_sql = if offset > 0 {
					let mut sql = cte_sql;
					for i in (1..=cte_values.len()).rev() {
						sql = sql.replace(&format!("${}", i), &format!("${}", i + offset));
					}
					sql
				} else {
					cte_sql
				};

				w.push(&adjusted_sql);
				w.push(")");

				// Merge the values from the CTE query
				w.append_values(&cte_values);
			});

			writer.push_space();
		}

		// SELECT clause
		writer.push("SELECT");
		writer.push_space();

		// DISTINCT clause
		if let Some(distinct) = &stmt.distinct {
			use crate::query::SelectDistinct;
			match distinct {
				SelectDistinct::All => {
					// SELECT ALL - explicit but not required in PostgreSQL
				}
				SelectDistinct::Distinct => {
					writer.push_keyword("DISTINCT");
					writer.push_space();
				}
				SelectDistinct::DistinctRow => {
					panic!("PostgreSQL does not support DISTINCT ROW. Use DISTINCT instead.");
				}
				SelectDistinct::DistinctOn(cols) => {
					writer.push_keyword("DISTINCT ON");
					writer.push_space();
					writer.push("(");
					writer.push_list(cols, ", ", |w, col_ref| {
						self.write_column_ref(w, col_ref);
					});
					writer.push(")");
					writer.push_space();
				}
			}
		}

		if stmt.selects.is_empty() {
			writer.push("*");
		} else {
			writer.push_list(&stmt.selects, ", ", |w, select_expr| {
				self.write_simple_expr(w, &select_expr.expr);
				if let Some(alias) = &select_expr.alias {
					w.push_keyword("AS");
					w.push_space();
					w.push_identifier(&alias.to_string(), |s| self.escape_iden(s));
				}
			});
		}

		// FROM clause
		if !stmt.from.is_empty() {
			writer.push_keyword("FROM");
			writer.push_space();
			writer.push_list(&stmt.from, ", ", |w, table_ref| {
				self.write_table_ref(w, table_ref);
			});
		}

		// JOIN clauses
		for join in &stmt.join {
			writer.push_space();
			self.write_join_expr(&mut writer, join);
		}

		// WHERE clause
		if !stmt.r#where.is_empty() {
			writer.push_keyword("WHERE");
			writer.push_space();
			// Write all conditions in the ConditionHolder with AND
			writer.push_list(&stmt.r#where.conditions, " AND ", |w, cond_expr| {
				self.write_condition_expr(w, cond_expr);
			});
		}

		// GROUP BY clause
		if !stmt.groups.is_empty() {
			writer.push_keyword("GROUP BY");
			writer.push_space();
			writer.push_list(&stmt.groups, ", ", |w, expr| {
				self.write_simple_expr(w, expr);
			});
		}

		// HAVING clause
		if !stmt.having.conditions.is_empty() {
			writer.push_keyword("HAVING");
			writer.push_space();
			// Write all conditions in the ConditionHolder with AND
			writer.push_list(&stmt.having.conditions, " AND ", |w, cond_expr| {
				self.write_condition_expr(w, cond_expr);
			});
		}

		// ORDER BY clause
		if !stmt.orders.is_empty() {
			writer.push_keyword("ORDER BY");
			writer.push_space();
			writer.push_list(&stmt.orders, ", ", |w, order_expr| {
				use crate::types::OrderExprKind;
				match &order_expr.expr {
					OrderExprKind::Column(iden) => {
						w.push_identifier(&iden.to_string(), |s| self.escape_iden(s));
					}
					OrderExprKind::TableColumn(table, col) => {
						w.push_identifier(&table.to_string(), |s| self.escape_iden(s));
						w.push(".");
						w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
					}
					OrderExprKind::Expr(expr) => {
						self.write_simple_expr(w, expr);
					}
				}
				match order_expr.order {
					crate::types::Order::Asc => {
						w.push_keyword("ASC");
					}
					crate::types::Order::Desc => {
						w.push_keyword("DESC");
					}
				}
				if let Some(nulls) = order_expr.nulls {
					w.push_space();
					w.push(nulls.as_str());
				}
			});
		}

		// WINDOW clause
		if !stmt.windows.is_empty() {
			writer.push_keyword("WINDOW");
			writer.push_space();
			writer.push_list(&stmt.windows, ", ", |w, (name, window)| {
				w.push_identifier(&name.to_string(), |s| self.escape_iden(s));
				w.push_space();
				w.push_keyword("AS");
				w.push_space();
				w.push("(");
				self.write_window_statement(w, window);
				w.push(")");
			});
		}

		// LIMIT clause
		if let Some(limit) = &stmt.limit {
			writer.push_keyword("LIMIT");
			writer.push_space();
			writer.push_value(limit.clone(), |i| self.placeholder(i));
		}

		// OFFSET clause
		if let Some(offset) = &stmt.offset {
			writer.push_keyword("OFFSET");
			writer.push_space();
			writer.push_value(offset.clone(), |i| self.placeholder(i));
		}

		// UNION/INTERSECT/EXCEPT clauses
		for (union_type, union_stmt) in &stmt.unions {
			writer.push_space();
			use crate::query::UnionType;
			match union_type {
				UnionType::Distinct => {
					writer.push_keyword("UNION");
				}
				UnionType::All => {
					writer.push_keyword("UNION ALL");
				}
				UnionType::Intersect => {
					writer.push_keyword("INTERSECT");
				}
				UnionType::Except => {
					writer.push_keyword("EXCEPT");
				}
			}
			writer.push_space();

			// Recursively build the union query
			let (mut union_sql, union_values) = self.build_select(union_stmt);

			// Adjust placeholders for PostgreSQL ($1, $2, ... -> $N, $N+1, ...)
			let offset = writer.param_index() - 1;
			if offset > 0 {
				for i in (1..=union_values.len()).rev() {
					union_sql = union_sql.replace(&format!("${}", i), &format!("${}", i + offset));
				}
			}

			// Append the union SQL (wrapped in parentheses if it has unions itself)
			if !union_stmt.unions.is_empty() {
				writer.push("(");
				writer.push(&union_sql);
				writer.push(")");
			} else {
				writer.push(&union_sql);
			}

			// Merge the values from the union query
			writer.append_values(&union_values);
		}

		writer.finish()
	}

	fn build_insert(&self, stmt: &InsertStatement) -> (String, Values) {
		use crate::query::insert::InsertSource;

		let mut writer = SqlWriter::new();

		// INSERT INTO clause
		writer.push("INSERT INTO");
		writer.push_space();

		if let Some(table) = &stmt.table {
			self.write_table_ref(&mut writer, table);
		} else {
			// No table specified - this should not happen in valid SQL
			writer.push("(NO_TABLE)");
		}

		// Column list
		if !stmt.columns.is_empty() {
			writer.push_space();
			writer.push("(");
			writer.push_list(&stmt.columns, ", ", |w, col| {
				w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
			});
			writer.push(")");
		}

		// VALUES clause or SELECT subquery
		match &stmt.source {
			InsertSource::Values(values) if !values.is_empty() => {
				writer.push_keyword("VALUES");
				writer.push_space();

				writer.push_list(values, ", ", |w, row| {
					w.push("(");
					w.push_list(row, ", ", |w2, value| {
						w2.push_value(value.clone(), |i| self.placeholder(i));
					});
					w.push(")");
				});
			}
			InsertSource::Subquery(select) => {
				writer.push_space();
				let (select_sql, select_values) = self.build_select(select);
				writer.push(&select_sql);
				writer.append_values(&select_values);
			}
			_ => {
				// Empty values - this is valid SQL in some contexts
			}
		}

		// ON CONFLICT clause
		if let Some(on_conflict) = &stmt.on_conflict {
			use crate::query::{OnConflictAction, OnConflictTarget};
			writer.push_keyword("ON CONFLICT");
			writer.push_space();

			// Target columns
			writer.push("(");
			match &on_conflict.target {
				OnConflictTarget::Column(col) => {
					writer.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				}
				OnConflictTarget::Columns(cols) => {
					writer.push_list(cols, ", ", |w, col| {
						w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
					});
				}
			}
			writer.push(")");

			// Action
			match &on_conflict.action {
				OnConflictAction::DoNothing => {
					writer.push_keyword("DO NOTHING");
				}
				OnConflictAction::DoUpdate(cols) => {
					writer.push_keyword("DO UPDATE SET");
					writer.push_space();
					writer.push_list(cols, ", ", |w, col| {
						let col_str = col.to_string();
						w.push_identifier(&col_str, |s| self.escape_iden(s));
						w.push(" = EXCLUDED.");
						w.push_identifier(&col_str, |s| self.escape_iden(s));
					});
				}
			}
		}

		// RETURNING clause (PostgreSQL specific)
		if let Some(returning) = &stmt.returning {
			writer.push_keyword("RETURNING");
			writer.push_space();

			use crate::query::ReturningClause;
			match returning {
				ReturningClause::All => {
					writer.push("*");
				}
				ReturningClause::Columns(cols) => {
					writer.push_list(cols, ", ", |w, col| {
						self.write_column_ref(w, col);
					});
				}
			}
		}

		writer.finish()
	}

	fn build_update(&self, stmt: &UpdateStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// UPDATE clause
		writer.push("UPDATE");
		writer.push_space();

		if let Some(table) = &stmt.table {
			self.write_table_ref(&mut writer, table);
		} else {
			writer.push("(NO_TABLE)");
		}

		// SET clause
		if !stmt.values.is_empty() {
			writer.push_keyword("SET");
			writer.push_space();

			writer.push_list(&stmt.values, ", ", |w, (col, value)| {
				w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				w.push(" = ");
				self.write_simple_expr(w, value);
			});
		}

		// WHERE clause
		if !stmt.r#where.is_empty() {
			writer.push_keyword("WHERE");
			writer.push_space();
			writer.push_list(&stmt.r#where.conditions, " AND ", |w, cond_expr| {
				self.write_condition_expr(w, cond_expr);
			});
		}

		// RETURNING clause (PostgreSQL specific)
		if let Some(returning) = &stmt.returning {
			writer.push_keyword("RETURNING");
			writer.push_space();

			use crate::query::ReturningClause;
			match returning {
				ReturningClause::All => {
					writer.push("*");
				}
				ReturningClause::Columns(cols) => {
					writer.push_list(cols, ", ", |w, col| {
						self.write_column_ref(w, col);
					});
				}
			}
		}

		writer.finish()
	}

	fn build_delete(&self, stmt: &DeleteStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// DELETE FROM clause
		writer.push("DELETE FROM");
		writer.push_space();

		if let Some(table) = &stmt.table {
			self.write_table_ref(&mut writer, table);
		} else {
			writer.push("(NO_TABLE)");
		}

		// WHERE clause
		if !stmt.r#where.is_empty() {
			writer.push_keyword("WHERE");
			writer.push_space();
			writer.push_list(&stmt.r#where.conditions, " AND ", |w, cond_expr| {
				self.write_condition_expr(w, cond_expr);
			});
		}

		// RETURNING clause (PostgreSQL specific)
		if let Some(returning) = &stmt.returning {
			writer.push_keyword("RETURNING");
			writer.push_space();

			use crate::query::ReturningClause;
			match returning {
				ReturningClause::All => {
					writer.push("*");
				}
				ReturningClause::Columns(cols) => {
					writer.push_list(cols, ", ", |w, col| {
						self.write_column_ref(w, col);
					});
				}
			}
		}

		writer.finish()
	}

	fn build_create_table(&self, stmt: &CreateTableStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push("CREATE TABLE");
		writer.push_space();

		if stmt.if_not_exists {
			writer.push_keyword("IF NOT EXISTS");
			writer.push_space();
		}

		if let Some(table) = &stmt.table {
			self.write_table_ref(&mut writer, table);
		}

		writer.push_space();
		writer.push("(");

		// Columns
		let mut first = true;
		for column in &stmt.columns {
			if !first {
				writer.push(", ");
			}
			first = false;

			// Column name
			writer.push_identifier(&column.name.to_string(), |s| self.escape_iden(s));
			writer.push_space();

			// Column type
			if let Some(col_type) = &column.column_type {
				// For auto_increment columns, use SERIAL types instead of INTEGER/BIGINT
				if column.auto_increment {
					use crate::types::ColumnType;
					let serial_type = match col_type {
						ColumnType::SmallInteger => "SMALLSERIAL",
						ColumnType::Integer => "SERIAL",
						ColumnType::BigInteger => "BIGSERIAL",
						_ => &self.column_type_to_sql(col_type),
					};
					writer.push(serial_type);
				} else {
					writer.push(&self.column_type_to_sql(col_type));
				}
			}

			// NOT NULL
			if column.not_null {
				writer.push_space();
				writer.push_keyword("NOT NULL");
			}

			// UNIQUE
			if column.unique {
				writer.push_space();
				writer.push_keyword("UNIQUE");
			}

			// PRIMARY KEY
			if column.primary_key {
				writer.push_space();
				writer.push_keyword("PRIMARY KEY");
			}

			// DEFAULT
			if let Some(default_expr) = &column.default {
				writer.push_space();
				writer.push_keyword("DEFAULT");
				writer.push_space();
				self.write_simple_expr(&mut writer, default_expr);
			}

			// CHECK
			if let Some(check_expr) = &column.check {
				writer.push_space();
				writer.push_keyword("CHECK");
				writer.push_space();
				writer.push("(");
				self.write_simple_expr_unquoted(&mut writer, check_expr);
				writer.push(")");
			}
		}

		// Table constraints
		for constraint in &stmt.constraints {
			writer.push(", ");
			self.write_table_constraint(&mut writer, constraint);
		}

		writer.push(")");

		writer.finish()
	}

	fn build_alter_table(&self, stmt: &AlterTableStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// ALTER TABLE table_name
		writer.push("ALTER TABLE");
		writer.push_space();
		if let Some(table) = &stmt.table {
			self.write_table_ref(&mut writer, table);
		}

		// Process operations
		let mut first = true;
		for operation in &stmt.operations {
			if !first {
				writer.push(",");
			}
			first = false;
			writer.push_space();

			match operation {
				AlterTableOperation::AddColumn(column_def) => {
					writer.push("ADD COLUMN");
					writer.push_space();
					writer.push_identifier(&column_def.name.to_string(), |s| self.escape_iden(s));
					writer.push_space();
					if let Some(col_type) = &column_def.column_type {
						writer.push(&self.column_type_to_sql(col_type));
					}
					if column_def.not_null {
						writer.push(" NOT NULL");
					}
					if column_def.unique {
						writer.push(" UNIQUE");
					}
					if let Some(default) = &column_def.default {
						writer.push(" DEFAULT ");
						self.write_simple_expr(&mut writer, default);
					}
					if let Some(check) = &column_def.check {
						writer.push(" CHECK (");
						self.write_simple_expr_unquoted(&mut writer, check);
						writer.push(")");
					}
				}
				AlterTableOperation::DropColumn { name, if_exists } => {
					writer.push("DROP COLUMN");
					writer.push_space();
					if *if_exists {
						writer.push("IF EXISTS");
						writer.push_space();
					}
					writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
				}
				AlterTableOperation::RenameColumn { old, new } => {
					writer.push("RENAME COLUMN");
					writer.push_space();
					writer.push_identifier(&old.to_string(), |s| self.escape_iden(s));
					writer.push_space();
					writer.push("TO");
					writer.push_space();
					writer.push_identifier(&new.to_string(), |s| self.escape_iden(s));
				}
				AlterTableOperation::ModifyColumn(column_def) => {
					// PostgreSQL uses ALTER COLUMN instead of MODIFY COLUMN
					writer.push("ALTER COLUMN");
					writer.push_space();
					writer.push_identifier(&column_def.name.to_string(), |s| self.escape_iden(s));
					writer.push_space();

					// TYPE change
					if let Some(col_type) = &column_def.column_type {
						writer.push("TYPE");
						writer.push_space();
						writer.push(&self.column_type_to_sql(col_type));
					}

					// NOT NULL / NULL
					if column_def.not_null {
						writer.push(", ALTER COLUMN ");
						writer
							.push_identifier(&column_def.name.to_string(), |s| self.escape_iden(s));
						writer.push(" SET NOT NULL");
					}

					// DEFAULT
					if let Some(default) = &column_def.default {
						writer.push(", ALTER COLUMN ");
						writer
							.push_identifier(&column_def.name.to_string(), |s| self.escape_iden(s));
						writer.push(" SET DEFAULT ");
						self.write_simple_expr(&mut writer, default);
					}
				}
				AlterTableOperation::AddConstraint(constraint) => {
					writer.push("ADD ");
					self.write_table_constraint(&mut writer, constraint);
				}
				AlterTableOperation::DropConstraint { name, if_exists } => {
					writer.push("DROP CONSTRAINT");
					writer.push_space();
					if *if_exists {
						writer.push("IF EXISTS");
						writer.push_space();
					}
					writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
				}
				AlterTableOperation::RenameTable(new_name) => {
					writer.push("RENAME TO");
					writer.push_space();
					writer.push_identifier(&new_name.to_string(), |s| self.escape_iden(s));
				}
			}
		}

		writer.finish()
	}

	fn build_drop_table(&self, stmt: &DropTableStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// DROP TABLE
		writer.push("DROP TABLE");
		writer.push_space();

		// IF EXISTS clause
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
			writer.push_space();
		}

		// Table names
		writer.push_list(&stmt.tables, ", ", |w, table_ref| {
			self.write_table_ref(w, table_ref);
		});

		// CASCADE/RESTRICT clause
		if stmt.cascade {
			writer.push_space();
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_space();
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}

	fn build_create_index(&self, stmt: &CreateIndexStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// CREATE UNIQUE INDEX IF NOT EXISTS
		writer.push("CREATE");
		writer.push_space();
		if stmt.unique {
			writer.push_keyword("UNIQUE");
			writer.push_space();
		}
		writer.push_keyword("INDEX");
		writer.push_space();
		if stmt.if_not_exists {
			writer.push_keyword("IF NOT EXISTS");
			writer.push_space();
		}

		// Index name
		if let Some(name) = &stmt.name {
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
			writer.push_space();
		}

		// ON table
		writer.push_keyword("ON");
		writer.push_space();
		if let Some(table) = &stmt.table {
			self.write_table_ref(&mut writer, table);
		}
		writer.push_space();

		// USING method
		if let Some(method) = &stmt.using {
			writer.push_keyword("USING");
			writer.push_space();
			writer.push(self.index_method_to_sql(method));
			writer.push_space();
		}

		// (column1 ASC, column2 DESC, ...)
		writer.push("(");
		let mut first = true;
		for col in &stmt.columns {
			if !first {
				writer.push(", ");
			}
			first = false;
			writer.push_identifier(&col.name.to_string(), |s| self.escape_iden(s));
			if let Some(order) = &col.order {
				writer.push_space();
				match order {
					crate::types::Order::Asc => writer.push("ASC"),
					crate::types::Order::Desc => writer.push("DESC"),
				}
			}
		}
		writer.push(")");

		// WHERE clause (partial index)
		if let Some(where_expr) = &stmt.r#where {
			writer.push_space();
			writer.push_keyword("WHERE");
			writer.push_space();
			self.write_simple_expr(&mut writer, where_expr);
		}

		writer.finish()
	}

	fn build_drop_index(&self, stmt: &DropIndexStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// DROP INDEX
		writer.push("DROP INDEX");
		writer.push_space();

		// IF EXISTS clause
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
			writer.push_space();
		}

		// Index name
		if let Some(name) = &stmt.name {
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		}

		// CASCADE/RESTRICT clause
		if stmt.cascade {
			writer.push_space();
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_space();
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}

	fn build_create_view(&self, stmt: &CreateViewStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push("CREATE");

		if stmt.or_replace {
			writer.push_keyword("OR REPLACE");
		}

		if stmt.materialized {
			writer.push_keyword("MATERIALIZED");
		}

		writer.push_keyword("VIEW");

		if stmt.if_not_exists {
			writer.push_keyword("IF NOT EXISTS");
		}

		if let Some(name) = &stmt.name {
			writer.push_space();
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		}

		if !stmt.columns.is_empty() {
			writer.push_space();
			writer.push("(");
			writer.push_list(stmt.columns.iter(), ", ", |w, col| {
				w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
			});
			writer.push(")");
		}

		writer.push_keyword("AS");

		if let Some(select) = &stmt.select {
			let (select_sql, select_values) = self.build_select(select);
			writer.push_space();
			writer.push(&select_sql);
			writer.append_values(&select_values);
		}

		writer.finish()
	}

	fn build_drop_view(&self, stmt: &DropViewStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push("DROP");

		if stmt.materialized {
			writer.push_keyword("MATERIALIZED");
		}

		writer.push_keyword("VIEW");

		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		writer.push_space();
		writer.push_list(stmt.names.iter(), ", ", |w, name| {
			w.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		});

		if stmt.cascade {
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}

	fn build_truncate_table(&self, stmt: &TruncateTableStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// TRUNCATE TABLE
		writer.push("TRUNCATE TABLE");
		writer.push_space();

		// Table names
		writer.push_list(&stmt.tables, ", ", |w, table_ref| {
			self.write_table_ref(w, table_ref);
		});

		// RESTART IDENTITY clause (PostgreSQL-specific)
		if stmt.restart_identity {
			writer.push_space();
			writer.push_keyword("RESTART IDENTITY");
		}

		// CASCADE/RESTRICT clause
		if stmt.cascade {
			writer.push_space();
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_space();
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}

	fn build_create_trigger(&self, stmt: &CreateTriggerStatement) -> (String, Values) {
		use crate::types::{TriggerEvent, TriggerScope, TriggerTiming};

		let mut writer = SqlWriter::new();

		// CREATE TRIGGER
		writer.push("CREATE TRIGGER");

		// Trigger name
		if let Some(name) = &stmt.name {
			writer.push_space();
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		}

		// Timing: BEFORE / AFTER / INSTEAD OF
		if let Some(timing) = stmt.timing {
			writer.push_space();
			match timing {
				TriggerTiming::Before => writer.push("BEFORE"),
				TriggerTiming::After => writer.push("AFTER"),
				TriggerTiming::InsteadOf => writer.push("INSTEAD OF"),
			}
		}

		// Events: INSERT / UPDATE [OF columns] / DELETE
		if !stmt.events.is_empty() {
			writer.push_space();
			let mut first = true;
			for event in &stmt.events {
				if !first {
					writer.push(" OR ");
				}
				first = false;

				match event {
					TriggerEvent::Insert => writer.push("INSERT"),
					TriggerEvent::Update { columns } => {
						writer.push("UPDATE");
						if let Some(cols) = columns {
							writer.push(" OF ");
							writer.push_list(cols.iter(), ", ", |w, col| {
								w.push_identifier(col, |s| self.escape_iden(s));
							});
						}
					}
					TriggerEvent::Delete => writer.push("DELETE"),
				}
			}
		}

		// ON table
		writer.push_keyword("ON");
		if let Some(table) = &stmt.table {
			writer.push_space();
			self.write_table_ref(&mut writer, table);
		}

		// FOR EACH ROW / FOR EACH STATEMENT
		if let Some(scope) = stmt.scope {
			writer.push_space();
			match scope {
				TriggerScope::Row => writer.push("FOR EACH ROW"),
				TriggerScope::Statement => writer.push("FOR EACH STATEMENT"),
			}
		}

		// WHEN (condition)
		if let Some(when_cond) = &stmt.when_condition {
			writer.push_keyword("WHEN");
			writer.push(" (");
			self.write_simple_expr(&mut writer, when_cond);
			writer.push(")");
		}

		// EXECUTE FUNCTION function_name() or EXECUTE PROCEDURE (older syntax)
		if let Some(body) = &stmt.body {
			writer.push_space();
			match body {
				TriggerBody::PostgresFunction(func_name) => {
					writer.push("EXECUTE FUNCTION ");
					writer.push_identifier(func_name.as_str(), |s| self.escape_iden(s));
					writer.push("()");
				}
				TriggerBody::Single(_) | TriggerBody::Multiple(_) => {
					panic!(
						"PostgreSQL triggers require EXECUTE FUNCTION, not inline SQL statements"
					);
				}
			}
		}

		writer.finish()
	}

	fn build_drop_trigger(&self, stmt: &DropTriggerStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// DROP TRIGGER
		writer.push("DROP TRIGGER");

		// IF EXISTS
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		// Trigger name
		if let Some(name) = &stmt.name {
			writer.push_space();
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		}

		// ON table (optional in PostgreSQL, but recommended)
		if let Some(table) = &stmt.table {
			writer.push_keyword("ON");
			writer.push_space();
			self.write_table_ref(&mut writer, table);
		}

		// CASCADE / RESTRICT
		if stmt.cascade {
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}

	fn build_alter_index(&self, stmt: &AlterIndexStatement) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();
		writer.push_keyword("ALTER INDEX");
		writer.push_space();

		if let Some(ref name) = stmt.name {
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		} else {
			panic!("ALTER INDEX requires an index name");
		}

		// RENAME TO clause
		if let Some(ref new_name) = stmt.rename_to {
			writer.push_space();
			writer.push_keyword("RENAME TO");
			writer.push_space();
			writer.push_identifier(&Iden::to_string(new_name.as_ref()), |s| self.escape_iden(s));
		}

		// SET TABLESPACE clause
		if let Some(ref tablespace) = stmt.set_tablespace {
			writer.push_space();
			writer.push_keyword("SET TABLESPACE");
			writer.push_space();
			writer.push_identifier(&Iden::to_string(tablespace.as_ref()), |s| {
				self.escape_iden(s)
			});
		}

		writer.finish()
	}

	fn build_reindex(&self, stmt: &ReindexStatement) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();
		writer.push_keyword("REINDEX");

		// Options (CONCURRENTLY, VERBOSE, TABLESPACE)
		let mut options = Vec::new();
		if stmt.concurrently {
			options.push("CONCURRENTLY".to_string());
		}
		if stmt.verbose {
			options.push("VERBOSE".to_string());
		}
		if let Some(ref tablespace) = stmt.tablespace {
			let escaped = self.escape_iden(&Iden::to_string(tablespace.as_ref()));
			options.push(format!("TABLESPACE {}", escaped));
		}

		if !options.is_empty() {
			writer.push_space();
			writer.push("(");
			writer.push(&options.join(", "));
			writer.push(")");
		}

		// Target (INDEX, TABLE, SCHEMA, DATABASE, SYSTEM)
		writer.push_space();
		if let Some(target) = stmt.target {
			use crate::query::ReindexTarget;
			match target {
				ReindexTarget::Index => writer.push_keyword("INDEX"),
				ReindexTarget::Table => writer.push_keyword("TABLE"),
				ReindexTarget::Schema => writer.push_keyword("SCHEMA"),
				ReindexTarget::Database => writer.push_keyword("DATABASE"),
				ReindexTarget::System => writer.push_keyword("SYSTEM"),
			}
		} else {
			panic!("REINDEX requires a target");
		}

		// Name
		writer.push_space();
		if let Some(ref name) = stmt.name {
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		} else {
			panic!("REINDEX requires a name");
		}

		writer.finish()
	}

	fn build_optimize_table(&self, _stmt: &OptimizeTableStatement) -> (String, Values) {
		panic!(
			"OPTIMIZE TABLE is MySQL-specific. PostgreSQL users should use VACUUM ANALYZE instead."
		);
	}

	fn build_repair_table(&self, _stmt: &RepairTableStatement) -> (String, Values) {
		panic!(
			"REPAIR TABLE is not supported in PostgreSQL. PostgreSQL automatically repairs corrupted data during normal operation."
		);
	}

	fn build_check_table(&self, _stmt: &CheckTableStatement) -> (String, Values) {
		panic!(
			"CHECK TABLE is not supported in PostgreSQL. Use pg_catalog system views or pg_stat_* functions to monitor table health."
		);
	}

	fn build_create_function(
		&self,
		stmt: &crate::query::CreateFunctionStatement,
	) -> (String, Values) {
		use crate::types::{
			Iden,
			function::{FunctionBehavior, FunctionLanguage, FunctionSecurity},
		};

		let mut writer = SqlWriter::new();

		// CREATE [OR REPLACE] FUNCTION
		writer.push_keyword("CREATE");
		if stmt.function_def.or_replace {
			writer.push_keyword("OR REPLACE");
		}
		writer.push_keyword("FUNCTION");

		// Function name
		writer.push_space();
		writer.push_identifier(&Iden::to_string(stmt.function_def.name.as_ref()), |s| {
			self.escape_iden(s)
		});

		// Parameters (param1 type1, param2 type2, ...)
		writer.push("(");
		let mut first = true;
		for param in &stmt.function_def.parameters {
			if !first {
				writer.push(", ");
			}
			first = false;

			// Parameter mode (IN, OUT, INOUT, VARIADIC)
			if let Some(mode) = &param.mode {
				use crate::types::function::ParameterMode;
				match mode {
					ParameterMode::In => writer.push("IN "),
					ParameterMode::Out => writer.push("OUT "),
					ParameterMode::InOut => writer.push("INOUT "),
					ParameterMode::Variadic => writer.push("VARIADIC "),
				}
			}

			// Parameter name (optional)
			if let Some(name) = &param.name {
				writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
				writer.push(" ");
			}

			// Parameter type
			if let Some(param_type) = &param.param_type {
				writer.push(param_type);
			}

			// Default value (optional)
			if let Some(default) = &param.default_value {
				writer.push(" DEFAULT ");
				writer.push(default);
			}
		}
		writer.push(")");

		// RETURNS type
		if let Some(returns) = &stmt.function_def.returns {
			writer.push_keyword("RETURNS");
			writer.push_space();
			writer.push(returns);
		}

		// LANGUAGE
		if let Some(language) = &stmt.function_def.language {
			writer.push_keyword("LANGUAGE");
			writer.push_space();
			match language {
				FunctionLanguage::Sql => writer.push("SQL"),
				FunctionLanguage::PlPgSql => writer.push("PLPGSQL"),
				FunctionLanguage::C => writer.push("C"),
				FunctionLanguage::Custom(lang) => writer.push(lang),
			}
		}

		// Behavior (IMMUTABLE/STABLE/VOLATILE)
		if let Some(behavior) = &stmt.function_def.behavior {
			writer.push_space();
			match behavior {
				FunctionBehavior::Immutable => writer.push_keyword("IMMUTABLE"),
				FunctionBehavior::Stable => writer.push_keyword("STABLE"),
				FunctionBehavior::Volatile => writer.push_keyword("VOLATILE"),
			}
		}

		// Security (SECURITY DEFINER/INVOKER)
		if let Some(security) = &stmt.function_def.security {
			writer.push_space();
			match security {
				FunctionSecurity::Definer => writer.push_keyword("SECURITY DEFINER"),
				FunctionSecurity::Invoker => writer.push_keyword("SECURITY INVOKER"),
			}
		}

		// AS 'body'
		if let Some(body) = &stmt.function_def.body {
			writer.push_keyword("AS");
			writer.push_space();
			let delimiter = generate_safe_dollar_quote_delimiter(body);
			writer.push(&delimiter);
			writer.push(body);
			writer.push(&delimiter);
		}

		writer.finish()
	}

	fn build_alter_function(
		&self,
		stmt: &crate::query::AlterFunctionStatement,
	) -> (String, Values) {
		use crate::query::function::alter_function::AlterFunctionOperation;
		use crate::types::{
			Iden,
			function::{FunctionBehavior, FunctionSecurity},
		};

		let mut writer = SqlWriter::new();

		// ALTER FUNCTION
		writer.push_keyword("ALTER FUNCTION");

		// Function name
		if let Some(name) = &stmt.name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// Function signature (for overloaded functions)
		if !stmt.parameters.is_empty() {
			writer.push("(");
			let mut first = true;
			for param in &stmt.parameters {
				if !first {
					writer.push(", ");
				}
				first = false;

				// Parameter name (optional in signature)
				if let Some(name) = &param.name {
					let name_str = Iden::to_string(name.as_ref());
					if !name_str.is_empty() {
						writer.push_identifier(&name_str, |s| self.escape_iden(s));
						writer.push(" ");
					}
				}

				// Parameter type
				if let Some(param_type) = &param.param_type {
					writer.push(param_type);
				}
			}
			writer.push(")");
		}

		// ALTER FUNCTION operation
		if let Some(operation) = &stmt.operation {
			writer.push_space();
			match operation {
				AlterFunctionOperation::RenameTo(new_name) => {
					writer.push_keyword("RENAME TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_name.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				AlterFunctionOperation::OwnerTo(new_owner) => {
					writer.push_keyword("OWNER TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_owner.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				AlterFunctionOperation::SetSchema(new_schema) => {
					writer.push_keyword("SET SCHEMA");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_schema.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				AlterFunctionOperation::SetBehavior(behavior) => match behavior {
					FunctionBehavior::Immutable => writer.push_keyword("IMMUTABLE"),
					FunctionBehavior::Stable => writer.push_keyword("STABLE"),
					FunctionBehavior::Volatile => writer.push_keyword("VOLATILE"),
				},
				AlterFunctionOperation::SetSecurity(security) => match security {
					FunctionSecurity::Definer => writer.push_keyword("SECURITY DEFINER"),
					FunctionSecurity::Invoker => writer.push_keyword("SECURITY INVOKER"),
				},
			}
		}

		writer.finish()
	}

	fn build_drop_function(&self, stmt: &crate::query::DropFunctionStatement) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// DROP FUNCTION
		writer.push_keyword("DROP FUNCTION");

		// IF EXISTS
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		// Function name
		if let Some(name) = &stmt.name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// Function signature (for overloaded functions)
		if !stmt.parameters.is_empty() {
			writer.push("(");
			let mut first = true;
			for param in &stmt.parameters {
				if !first {
					writer.push(", ");
				}
				first = false;

				// Parameter name (optional in signature)
				if let Some(name) = &param.name {
					let name_str = Iden::to_string(name.as_ref());
					if !name_str.is_empty() {
						writer.push_identifier(&name_str, |s| self.escape_iden(s));
						writer.push(" ");
					}
				}

				// Parameter type
				if let Some(param_type) = &param.param_type {
					writer.push(param_type);
				}
			}
			writer.push(")");
		}

		// CASCADE
		if stmt.cascade {
			writer.push_keyword("CASCADE");
		}

		writer.finish()
	}

	fn build_grant(&self, stmt: &crate::dcl::GrantStatement) -> (String, Values) {
		use crate::dcl::Grantee;

		let mut writer = SqlWriter::new();

		// GRANT keyword
		writer.push("GRANT");
		writer.push_space();

		// Privileges
		writer.push_list(&stmt.privileges, ", ", |w, privilege| {
			w.push(privilege.as_sql());
		});

		// ON clause
		writer.push_keyword("ON");
		writer.push_space();
		writer.push(stmt.object_type.as_sql());
		writer.push_space();

		// Objects
		writer.push_list(&stmt.objects, ", ", |w, obj| {
			w.push_identifier(&obj.to_string(), |s| self.escape_iden(s));
		});

		// TO clause
		writer.push_keyword("TO");
		writer.push_space();

		// Grantees
		writer.push_list(&stmt.grantees, ", ", |w, grantee| {
			match grantee {
				Grantee::Role(name) => {
					w.push_identifier(name, |s| self.escape_iden(s));
				}
				Grantee::User(_, _) => {
					// MySQL-specific, not supported in PostgreSQL
					w.push_identifier("(UNSUPPORTED_USER)", |s| self.escape_iden(s));
				}
				Grantee::Public => {
					w.push("PUBLIC");
				}
				Grantee::CurrentRole => {
					w.push("CURRENT_ROLE");
				}
				Grantee::CurrentUser => {
					w.push("CURRENT_USER");
				}
				Grantee::SessionUser => {
					w.push("SESSION_USER");
				}
			}
		});

		// WITH GRANT OPTION
		if stmt.with_grant_option {
			writer.push_keyword("WITH GRANT OPTION");
		}

		// GRANTED BY clause
		if let Some(grantor) = &stmt.granted_by {
			writer.push_keyword("GRANTED BY");
			writer.push_space();
			match grantor {
				Grantee::Role(name) => {
					writer.push_identifier(name, |s| self.escape_iden(s));
				}
				Grantee::User(_, _) => {
					writer.push_identifier("(UNSUPPORTED_USER)", |s| self.escape_iden(s));
				}
				Grantee::Public => {
					writer.push("PUBLIC");
				}
				Grantee::CurrentRole => {
					writer.push("CURRENT_ROLE");
				}
				Grantee::CurrentUser => {
					writer.push("CURRENT_USER");
				}
				Grantee::SessionUser => {
					writer.push("SESSION_USER");
				}
			}
		}

		writer.finish()
	}

	fn build_revoke(&self, stmt: &crate::dcl::RevokeStatement) -> (String, Values) {
		use crate::dcl::Grantee;

		let mut writer = SqlWriter::new();

		// REVOKE keyword
		writer.push("REVOKE");
		writer.push_space();

		// GRANT OPTION FOR (if specified)
		if stmt.grant_option_for {
			writer.push("GRANT OPTION FOR");
			writer.push_space();
		}

		// Privileges
		writer.push_list(&stmt.privileges, ", ", |w, privilege| {
			w.push(privilege.as_sql());
		});

		// ON clause
		writer.push_keyword("ON");
		writer.push_space();
		writer.push(stmt.object_type.as_sql());
		writer.push_space();

		// Objects
		writer.push_list(&stmt.objects, ", ", |w, obj| {
			w.push_identifier(&obj.to_string(), |s| self.escape_iden(s));
		});

		// FROM clause
		writer.push_keyword("FROM");
		writer.push_space();

		// Grantees
		writer.push_list(&stmt.grantees, ", ", |w, grantee| {
			match grantee {
				Grantee::Role(name) => {
					w.push_identifier(name, |s| self.escape_iden(s));
				}
				Grantee::User(_, _) => {
					// MySQL-specific, not supported in PostgreSQL
					w.push_identifier("(UNSUPPORTED_USER)", |s| self.escape_iden(s));
				}
				Grantee::Public => {
					w.push("PUBLIC");
				}
				Grantee::CurrentRole => {
					w.push("CURRENT_ROLE");
				}
				Grantee::CurrentUser => {
					w.push("CURRENT_USER");
				}
				Grantee::SessionUser => {
					w.push("SESSION_USER");
				}
			}
		});

		// CASCADE / RESTRICT
		if stmt.cascade {
			writer.push_keyword("CASCADE");
		}

		writer.finish()
	}

	fn build_grant_role(&self, stmt: &crate::dcl::GrantRoleStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// GRANT keyword
		writer.push("GRANT");
		writer.push_space();

		// Roles (comma-separated list)
		writer.push_list(&stmt.roles, ", ", |w, role| {
			w.push_identifier(role, |s| self.escape_iden(s));
		});

		// TO clause
		writer.push_keyword("TO");
		writer.push_space();

		// Grantees
		writer.push_list(&stmt.grantees, ", ", |w, grantee| {
			w.push(Self::format_role_specification(grantee));
		});

		// WITH ADMIN OPTION
		if stmt.with_admin_option {
			writer.push_keyword("WITH ADMIN OPTION");
		}

		// GRANTED BY
		if let Some(ref grantor) = stmt.granted_by {
			writer.push_keyword("GRANTED BY");
			writer.push_space();
			writer.push(Self::format_role_specification(grantor));
		}

		writer.finish()
	}

	fn build_revoke_role(&self, stmt: &crate::dcl::RevokeRoleStatement) -> (String, Values) {
		use crate::dcl::DropBehavior;

		let mut writer = SqlWriter::new();

		// REVOKE keyword
		writer.push("REVOKE");
		writer.push_space();

		// ADMIN OPTION FOR
		if stmt.admin_option_for {
			writer.push("ADMIN OPTION FOR");
			writer.push_space();
		}

		// Roles (comma-separated list)
		writer.push_list(&stmt.roles, ", ", |w, role| {
			w.push_identifier(role, |s| self.escape_iden(s));
		});

		// FROM clause
		writer.push_keyword("FROM");
		writer.push_space();

		// Grantees
		writer.push_list(&stmt.grantees, ", ", |w, grantee| {
			w.push(Self::format_role_specification(grantee));
		});

		// GRANTED BY
		if let Some(ref grantor) = stmt.granted_by {
			writer.push_keyword("GRANTED BY");
			writer.push_space();
			writer.push(Self::format_role_specification(grantor));
		}

		// CASCADE / RESTRICT
		if let Some(behavior) = stmt.drop_behavior {
			match behavior {
				DropBehavior::Cascade => writer.push_keyword("CASCADE"),
				DropBehavior::Restrict => writer.push_keyword("RESTRICT"),
			}
		}

		writer.finish()
	}

	fn build_create_role(&self, stmt: &crate::dcl::CreateRoleStatement) -> (String, Values) {
		use crate::dcl::RoleAttribute;
		use crate::value::Value;

		let mut writer = SqlWriter::new();

		// CREATE ROLE keyword
		writer.push("CREATE ROLE");
		writer.push_space();

		// Role name
		writer.push_identifier(&stmt.role_name, |s| self.escape_iden(s));

		// WITH keyword (optional but commonly used)
		if !stmt.attributes.is_empty() {
			writer.push_keyword("WITH");
		}

		// Attributes
		for attr in &stmt.attributes {
			writer.push_space();
			match attr {
				RoleAttribute::SuperUser => writer.push("SUPERUSER"),
				RoleAttribute::NoSuperUser => writer.push("NOSUPERUSER"),
				RoleAttribute::CreateDb => writer.push("CREATEDB"),
				RoleAttribute::NoCreateDb => writer.push("NOCREATEDB"),
				RoleAttribute::CreateRole => writer.push("CREATEROLE"),
				RoleAttribute::NoCreateRole => writer.push("NOCREATEROLE"),
				RoleAttribute::Inherit => writer.push("INHERIT"),
				RoleAttribute::NoInherit => writer.push("NOINHERIT"),
				RoleAttribute::Login => writer.push("LOGIN"),
				RoleAttribute::NoLogin => writer.push("NOLOGIN"),
				RoleAttribute::Replication => writer.push("REPLICATION"),
				RoleAttribute::NoReplication => writer.push("NOREPLICATION"),
				RoleAttribute::BypassRls => writer.push("BYPASSRLS"),
				RoleAttribute::NoBypassRls => writer.push("NOBYPASSRLS"),
				RoleAttribute::ConnectionLimit(limit) => {
					writer.push("CONNECTION LIMIT");
					writer.push_space();
					writer.push(&limit.to_string());
				}
				RoleAttribute::Password(pwd) => {
					writer.push("PASSWORD");
					writer.push_space();
					writer.push_value(Value::String(Some(Box::new(pwd.clone()))), |i| {
						self.placeholder(i)
					});
				}
				RoleAttribute::EncryptedPassword(pwd) => {
					writer.push("ENCRYPTED PASSWORD");
					writer.push_space();
					writer.push_value(Value::String(Some(Box::new(pwd.clone()))), |i| {
						self.placeholder(i)
					});
				}
				RoleAttribute::UnencryptedPassword(pwd) => {
					writer.push("UNENCRYPTED PASSWORD");
					writer.push_space();
					writer.push_value(Value::String(Some(Box::new(pwd.clone()))), |i| {
						self.placeholder(i)
					});
				}
				RoleAttribute::ValidUntil(timestamp) => {
					writer.push("VALID UNTIL");
					writer.push_space();
					writer.push("'");
					writer.push(timestamp);
					writer.push("'");
				}
				RoleAttribute::InRole(roles) => {
					writer.push("IN ROLE");
					writer.push_space();
					writer.push_list(roles, ", ", |w, role| {
						w.push_identifier(role, |s| self.escape_iden(s));
					});
				}
				RoleAttribute::Role(roles) => {
					writer.push("ROLE");
					writer.push_space();
					writer.push_list(roles, ", ", |w, role| {
						w.push_identifier(role, |s| self.escape_iden(s));
					});
				}
				RoleAttribute::Admin(roles) => {
					writer.push("ADMIN");
					writer.push_space();
					writer.push_list(roles, ", ", |w, role| {
						w.push_identifier(role, |s| self.escape_iden(s));
					});
				}
			}
		}

		writer.finish()
	}

	fn build_drop_role(&self, stmt: &crate::dcl::DropRoleStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// DROP ROLE keyword
		writer.push("DROP ROLE");
		writer.push_space();

		// IF EXISTS clause
		if stmt.if_exists {
			writer.push("IF EXISTS");
			writer.push_space();
		}

		// Role names (comma-separated)
		writer.push_list(&stmt.role_names, ", ", |w, role_name| {
			w.push_identifier(role_name, |s| self.escape_iden(s));
		});

		writer.finish()
	}

	fn build_alter_role(&self, stmt: &crate::dcl::AlterRoleStatement) -> (String, Values) {
		use crate::dcl::RoleAttribute;
		use crate::value::Value;

		let mut writer = SqlWriter::new();

		// Check for RENAME TO (special case in PostgreSQL)
		if let Some(ref new_name) = stmt.rename_to {
			writer.push("ALTER ROLE");
			writer.push_space();
			writer.push_identifier(&stmt.role_name, |s| self.escape_iden(s));
			writer.push_keyword("RENAME TO");
			writer.push_space();
			writer.push_identifier(new_name, |s| self.escape_iden(s));
			return writer.finish();
		}

		// ALTER ROLE keyword
		writer.push("ALTER ROLE");
		writer.push_space();

		// Role name
		writer.push_identifier(&stmt.role_name, |s| self.escape_iden(s));

		// WITH keyword (optional but commonly used)
		if !stmt.attributes.is_empty() {
			writer.push_keyword("WITH");
		}

		// Attributes (same as CREATE ROLE)
		for attr in &stmt.attributes {
			writer.push_space();
			match attr {
				RoleAttribute::SuperUser => writer.push("SUPERUSER"),
				RoleAttribute::NoSuperUser => writer.push("NOSUPERUSER"),
				RoleAttribute::CreateDb => writer.push("CREATEDB"),
				RoleAttribute::NoCreateDb => writer.push("NOCREATEDB"),
				RoleAttribute::CreateRole => writer.push("CREATEROLE"),
				RoleAttribute::NoCreateRole => writer.push("NOCREATEROLE"),
				RoleAttribute::Inherit => writer.push("INHERIT"),
				RoleAttribute::NoInherit => writer.push("NOINHERIT"),
				RoleAttribute::Login => writer.push("LOGIN"),
				RoleAttribute::NoLogin => writer.push("NOLOGIN"),
				RoleAttribute::Replication => writer.push("REPLICATION"),
				RoleAttribute::NoReplication => writer.push("NOREPLICATION"),
				RoleAttribute::BypassRls => writer.push("BYPASSRLS"),
				RoleAttribute::NoBypassRls => writer.push("NOBYPASSRLS"),
				RoleAttribute::ConnectionLimit(limit) => {
					writer.push("CONNECTION LIMIT");
					writer.push_space();
					writer.push(&limit.to_string());
				}
				RoleAttribute::Password(pwd) => {
					writer.push("PASSWORD");
					writer.push_space();
					writer.push_value(Value::String(Some(Box::new(pwd.clone()))), |i| {
						self.placeholder(i)
					});
				}
				RoleAttribute::EncryptedPassword(pwd) => {
					writer.push("ENCRYPTED PASSWORD");
					writer.push_space();
					writer.push_value(Value::String(Some(Box::new(pwd.clone()))), |i| {
						self.placeholder(i)
					});
				}
				RoleAttribute::UnencryptedPassword(pwd) => {
					writer.push("UNENCRYPTED PASSWORD");
					writer.push_space();
					writer.push_value(Value::String(Some(Box::new(pwd.clone()))), |i| {
						self.placeholder(i)
					});
				}
				RoleAttribute::ValidUntil(timestamp) => {
					writer.push("VALID UNTIL");
					writer.push_space();
					writer.push("'");
					writer.push(timestamp);
					writer.push("'");
				}
				RoleAttribute::InRole(roles) => {
					writer.push("IN ROLE");
					writer.push_space();
					writer.push_list(roles, ", ", |w, role| {
						w.push_identifier(role, |s| self.escape_iden(s));
					});
				}
				RoleAttribute::Role(roles) => {
					writer.push("ROLE");
					writer.push_space();
					writer.push_list(roles, ", ", |w, role| {
						w.push_identifier(role, |s| self.escape_iden(s));
					});
				}
				RoleAttribute::Admin(roles) => {
					writer.push("ADMIN");
					writer.push_space();
					writer.push_list(roles, ", ", |w, role| {
						w.push_identifier(role, |s| self.escape_iden(s));
					});
				}
			}
		}

		writer.finish()
	}

	fn build_create_user(&self, stmt: &crate::dcl::CreateUserStatement) -> (String, Values) {
		use crate::dcl::{CreateRoleStatement, RoleAttribute};

		// PostgreSQL CREATE USER is CREATE ROLE WITH LOGIN
		let mut create_role = CreateRoleStatement::new()
			.role(&stmt.user_name)
			.attribute(RoleAttribute::Login);

		// Add all attributes from CREATE USER
		for attr in &stmt.attributes {
			create_role = create_role.attribute(attr.clone());
		}

		// Use build_create_role to generate the SQL
		self.build_create_role(&create_role)
	}

	fn build_drop_user(&self, stmt: &crate::dcl::DropUserStatement) -> (String, Values) {
		use crate::dcl::DropRoleStatement;

		// PostgreSQL DROP USER is DROP ROLE
		let mut drop_role = DropRoleStatement::new();
		drop_role.role_names = stmt.user_names.clone();
		drop_role.if_exists = stmt.if_exists;

		// Use build_drop_role to generate the SQL
		self.build_drop_role(&drop_role)
	}

	fn build_alter_user(&self, stmt: &crate::dcl::AlterUserStatement) -> (String, Values) {
		use crate::dcl::AlterRoleStatement;

		// PostgreSQL ALTER USER is ALTER ROLE
		let mut alter_role = AlterRoleStatement::new().role(&stmt.user_name);

		// Add all attributes from ALTER USER
		for attr in &stmt.attributes {
			alter_role = alter_role.attribute(attr.clone());
		}

		// Use build_alter_role to generate the SQL
		self.build_alter_role(&alter_role)
	}

	fn build_rename_user(&self, _stmt: &crate::dcl::RenameUserStatement) -> (String, Values) {
		panic!("RENAME USER is not supported by PostgreSQL. Use ALTER USER ... RENAME TO instead.");
	}

	fn build_set_role(&self, stmt: &crate::dcl::SetRoleStatement) -> (String, Values) {
		use crate::dcl::RoleTarget;

		let mut writer = SqlWriter::new();

		writer.push("SET ROLE");
		writer.push_space();

		match &stmt.target {
			Some(RoleTarget::Named(name)) => {
				writer.push_identifier(name, |s| self.escape_iden(s));
			}
			Some(RoleTarget::None) => {
				writer.push("NONE");
			}
			Some(RoleTarget::All) => {
				panic!("SET ROLE ALL is not supported by PostgreSQL (MySQL only)");
			}
			Some(RoleTarget::AllExcept(_)) => {
				panic!("SET ROLE ALL EXCEPT is not supported by PostgreSQL (MySQL only)");
			}
			Some(RoleTarget::Default) => {
				panic!("SET ROLE DEFAULT is not supported by PostgreSQL (MySQL only)");
			}
			None => {
				panic!("SET ROLE requires a role target");
			}
		}

		writer.finish()
	}

	fn build_reset_role(&self, _stmt: &crate::dcl::ResetRoleStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();
		writer.push("RESET ROLE");
		writer.finish()
	}

	fn build_set_default_role(
		&self,
		_stmt: &crate::dcl::SetDefaultRoleStatement,
	) -> (String, Values) {
		panic!("SET DEFAULT ROLE is not supported by PostgreSQL (MySQL only)");
	}

	fn escape_identifier(&self, ident: &str) -> String {
		self.escape_iden(ident)
	}

	fn format_placeholder(&self, index: usize) -> String {
		self.placeholder(index)
	}

	fn build_create_schema(&self, stmt: &crate::query::CreateSchemaStatement) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// CREATE SCHEMA
		writer.push_keyword("CREATE SCHEMA");

		// IF NOT EXISTS
		if stmt.if_not_exists {
			writer.push_keyword("IF NOT EXISTS");
		}

		// Schema name
		if let Some(name) = &stmt.schema_name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// AUTHORIZATION owner
		if let Some(owner) = &stmt.authorization {
			writer.push_keyword("AUTHORIZATION");
			writer.push_space();
			writer.push_identifier(&Iden::to_string(owner.as_ref()), |s| self.escape_iden(s));
		}

		writer.finish()
	}

	fn build_alter_schema(&self, stmt: &crate::query::AlterSchemaStatement) -> (String, Values) {
		use crate::query::AlterSchemaOperation;
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// ALTER SCHEMA
		writer.push_keyword("ALTER SCHEMA");

		// Schema name
		if let Some(name) = &stmt.schema_name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// Operation
		if let Some(operation) = &stmt.operation {
			writer.push_space();
			match operation {
				AlterSchemaOperation::RenameTo(new_name) => {
					writer.push_keyword("RENAME TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_name.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				AlterSchemaOperation::OwnerTo(new_owner) => {
					writer.push_keyword("OWNER TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_owner.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
			}
		}

		writer.finish()
	}

	fn build_drop_schema(&self, stmt: &crate::query::DropSchemaStatement) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// DROP SCHEMA
		writer.push_keyword("DROP SCHEMA");

		// IF EXISTS
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		// Schema name
		if let Some(name) = &stmt.schema_name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// CASCADE or RESTRICT
		if stmt.cascade {
			writer.push_keyword("CASCADE");
		}

		writer.finish()
	}

	fn build_create_sequence(
		&self,
		stmt: &crate::query::CreateSequenceStatement,
	) -> (String, Values) {
		use crate::types::{Iden, sequence::OwnedBy};

		let mut writer = SqlWriter::new();
		let seq_def = &stmt.sequence_def;

		// CREATE SEQUENCE
		writer.push_keyword("CREATE SEQUENCE");

		// IF NOT EXISTS
		if seq_def.if_not_exists {
			writer.push_keyword("IF NOT EXISTS");
		}

		// Sequence name
		writer.push_space();
		writer.push_identifier(&Iden::to_string(seq_def.name.as_ref()), |s| {
			self.escape_iden(s)
		});

		// INCREMENT BY
		if let Some(increment) = seq_def.increment {
			writer.push_keyword("INCREMENT BY");
			writer.push_space();
			writer.push(&increment.to_string());
		}

		// MINVALUE or NO MINVALUE
		if let Some(min_value) = &seq_def.min_value {
			writer.push_space();
			match min_value {
				Some(val) => {
					writer.push_keyword("MINVALUE");
					writer.push_space();
					writer.push(&val.to_string());
				}
				None => {
					writer.push_keyword("NO MINVALUE");
				}
			}
		}

		// MAXVALUE or NO MAXVALUE
		if let Some(max_value) = &seq_def.max_value {
			writer.push_space();
			match max_value {
				Some(val) => {
					writer.push_keyword("MAXVALUE");
					writer.push_space();
					writer.push(&val.to_string());
				}
				None => {
					writer.push_keyword("NO MAXVALUE");
				}
			}
		}

		// START WITH
		if let Some(start) = seq_def.start {
			writer.push_keyword("START WITH");
			writer.push_space();
			writer.push(&start.to_string());
		}

		// CACHE
		if let Some(cache) = seq_def.cache {
			writer.push_keyword("CACHE");
			writer.push_space();
			writer.push(&cache.to_string());
		}

		// CYCLE or NO CYCLE
		if let Some(cycle) = seq_def.cycle {
			writer.push_space();
			if cycle {
				writer.push_keyword("CYCLE");
			} else {
				writer.push_keyword("NO CYCLE");
			}
		}

		// OWNED BY
		if let Some(owned_by) = &seq_def.owned_by {
			writer.push_keyword("OWNED BY");
			writer.push_space();
			match owned_by {
				OwnedBy::Column { table, column } => {
					writer
						.push_identifier(&Iden::to_string(table.as_ref()), |s| self.escape_iden(s));
					writer.push(".");
					writer.push_identifier(&Iden::to_string(column.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				OwnedBy::None => {
					writer.push_keyword("NONE");
				}
			}
		}

		writer.finish()
	}

	fn build_alter_sequence(
		&self,
		stmt: &crate::query::AlterSequenceStatement,
	) -> (String, Values) {
		use crate::types::{
			Iden,
			sequence::{OwnedBy, SequenceOption},
		};

		let mut writer = SqlWriter::new();

		// ALTER SEQUENCE
		writer.push_keyword("ALTER SEQUENCE");

		// Sequence name
		writer.push_space();
		writer.push_identifier(&Iden::to_string(stmt.name.as_ref()), |s| {
			self.escape_iden(s)
		});

		// Options
		for option in &stmt.options {
			writer.push_space();
			match option {
				SequenceOption::Restart(value) => {
					writer.push_keyword("RESTART");
					if let Some(val) = value {
						writer.push_keyword("WITH");
						writer.push_space();
						writer.push(&val.to_string());
					}
				}
				SequenceOption::IncrementBy(value) => {
					writer.push_keyword("INCREMENT BY");
					writer.push_space();
					writer.push(&value.to_string());
				}
				SequenceOption::MinValue(value) => {
					writer.push_keyword("MINVALUE");
					writer.push_space();
					writer.push(&value.to_string());
				}
				SequenceOption::NoMinValue => {
					writer.push_keyword("NO MINVALUE");
				}
				SequenceOption::MaxValue(value) => {
					writer.push_keyword("MAXVALUE");
					writer.push_space();
					writer.push(&value.to_string());
				}
				SequenceOption::NoMaxValue => {
					writer.push_keyword("NO MAXVALUE");
				}
				SequenceOption::Cache(value) => {
					writer.push_keyword("CACHE");
					writer.push_space();
					writer.push(&value.to_string());
				}
				SequenceOption::Cycle => {
					writer.push_keyword("CYCLE");
				}
				SequenceOption::NoCycle => {
					writer.push_keyword("NO CYCLE");
				}
				SequenceOption::OwnedBy(owned_by) => {
					writer.push_keyword("OWNED BY");
					writer.push_space();
					match owned_by {
						OwnedBy::Column { table, column } => {
							writer.push_identifier(&Iden::to_string(table.as_ref()), |s| {
								self.escape_iden(s)
							});
							writer.push(".");
							writer.push_identifier(&Iden::to_string(column.as_ref()), |s| {
								self.escape_iden(s)
							});
						}
						OwnedBy::None => {
							writer.push_keyword("NONE");
						}
					}
				}
			}
		}

		writer.finish()
	}

	fn build_drop_sequence(&self, stmt: &crate::query::DropSequenceStatement) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// DROP SEQUENCE
		writer.push_keyword("DROP SEQUENCE");

		// IF EXISTS
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		// Sequence name
		writer.push_space();
		writer.push_identifier(&Iden::to_string(stmt.name.as_ref()), |s| {
			self.escape_iden(s)
		});

		// CASCADE or RESTRICT
		if stmt.cascade {
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}

	fn build_comment(&self, stmt: &crate::query::CommentStatement) -> (String, Values) {
		use crate::types::{CommentTarget, Iden};

		let mut writer = SqlWriter::new();

		// COMMENT ON
		writer.push_keyword("COMMENT ON");

		// Target object type and name
		if let Some(target) = &stmt.target {
			writer.push_space();
			match target {
				CommentTarget::Table(table) => {
					writer.push_keyword("TABLE");
					writer.push_space();
					writer
						.push_identifier(&Iden::to_string(table.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::Column(table, column) => {
					writer.push_keyword("COLUMN");
					writer.push_space();
					writer
						.push_identifier(&Iden::to_string(table.as_ref()), |s| self.escape_iden(s));
					writer.push(".");
					writer.push_identifier(&Iden::to_string(column.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				CommentTarget::Index(index) => {
					writer.push_keyword("INDEX");
					writer.push_space();
					writer
						.push_identifier(&Iden::to_string(index.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::View(view) => {
					writer.push_keyword("VIEW");
					writer.push_space();
					writer
						.push_identifier(&Iden::to_string(view.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::MaterializedView(view) => {
					writer.push_keyword("MATERIALIZED VIEW");
					writer.push_space();
					writer
						.push_identifier(&Iden::to_string(view.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::Sequence(seq) => {
					writer.push_keyword("SEQUENCE");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(seq.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::Schema(schema) => {
					writer.push_keyword("SCHEMA");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(schema.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				CommentTarget::Database(db) => {
					writer.push_keyword("DATABASE");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(db.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::Function(func) => {
					writer.push_keyword("FUNCTION");
					writer.push_space();
					writer
						.push_identifier(&Iden::to_string(func.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::Trigger(trigger, table) => {
					writer.push_keyword("TRIGGER");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(trigger.as_ref()), |s| {
						self.escape_iden(s)
					});
					writer.push_keyword("ON");
					writer.push_space();
					writer
						.push_identifier(&Iden::to_string(table.as_ref()), |s| self.escape_iden(s));
				}
				CommentTarget::Type(typ) => {
					writer.push_keyword("TYPE");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(typ.as_ref()), |s| self.escape_iden(s));
				}
			}
		}

		// IS 'comment' or IS NULL
		writer.push_keyword("IS");
		writer.push_space();
		if stmt.is_null {
			writer.push_keyword("NULL");
		} else if let Some(comment) = &stmt.comment {
			// Escape single quotes in comment text
			let escaped = comment.replace('\'', "''");
			writer.push(&format!("'{}'", escaped));
		}

		writer.finish()
	}

	fn build_create_database(
		&self,
		stmt: &crate::query::CreateDatabaseStatement,
	) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// CREATE DATABASE
		writer.push_keyword("CREATE DATABASE");

		// IF NOT EXISTS - PostgreSQL does not support IF NOT EXISTS for CREATE DATABASE
		// if stmt.if_not_exists {
		//     writer.push_keyword("IF NOT EXISTS");
		// }

		// Database name
		if let Some(name) = &stmt.database_name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// OWNER
		if let Some(owner) = &stmt.owner {
			writer.push_keyword("OWNER");
			writer.push_space();
			writer.push_identifier(&Iden::to_string(owner.as_ref()), |s| self.escape_iden(s));
		}

		// TEMPLATE
		if let Some(template) = &stmt.template {
			writer.push_keyword("TEMPLATE");
			writer.push_space();
			writer.push_identifier(&Iden::to_string(template.as_ref()), |s| self.escape_iden(s));
		}

		// ENCODING
		if let Some(encoding) = &stmt.encoding {
			writer.push_keyword("ENCODING");
			writer.push_space();
			let escaped = encoding.replace('\'', "''");
			writer.push(&format!("'{}'", escaped));
		}

		// LC_COLLATE
		if let Some(lc_collate) = &stmt.lc_collate {
			writer.push_keyword("LC_COLLATE");
			writer.push_space();
			let escaped = lc_collate.replace('\'', "''");
			writer.push(&format!("'{}'", escaped));
		}

		// LC_CTYPE
		if let Some(lc_ctype) = &stmt.lc_ctype {
			writer.push_keyword("LC_CTYPE");
			writer.push_space();
			let escaped = lc_ctype.replace('\'', "''");
			writer.push(&format!("'{}'", escaped));
		}

		writer.finish()
	}

	fn build_alter_database(
		&self,
		stmt: &crate::query::AlterDatabaseStatement,
	) -> (String, Values) {
		use crate::types::{DatabaseOperation, Iden};

		let mut writer = SqlWriter::new();

		// ALTER DATABASE
		writer.push_keyword("ALTER DATABASE");

		// Database name
		if let Some(name) = &stmt.database_name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// Operations
		for (i, operation) in stmt.operations.iter().enumerate() {
			if i == 0 {
				writer.push_space();
			} else {
				writer.push(", ");
			}
			match operation {
				DatabaseOperation::RenameDatabase(new_name) => {
					writer.push_keyword("RENAME TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_name.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				DatabaseOperation::OwnerTo(new_owner) => {
					writer.push_keyword("OWNER TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_owner.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				// CockroachDB-specific operations are not supported in PostgreSQL
				DatabaseOperation::AddRegion(region) => {
					// For CockroachDB compatibility: ADD REGION 'region-name'
					writer.push_keyword("ADD REGION");
					writer.push_space();
					let escaped = region.replace('\'', "''");
					writer.push(&format!("'{}'", escaped));
				}
				DatabaseOperation::DropRegion(region) => {
					// For CockroachDB compatibility: DROP REGION 'region-name'
					writer.push_keyword("DROP REGION");
					writer.push_space();
					let escaped = region.replace('\'', "''");
					writer.push(&format!("'{}'", escaped));
				}
				DatabaseOperation::SetPrimaryRegion(region) => {
					// For CockroachDB compatibility: PRIMARY REGION 'region-name'
					writer.push_keyword("PRIMARY REGION");
					writer.push_space();
					let escaped = region.replace('\'', "''");
					writer.push(&format!("'{}'", escaped));
				}
				DatabaseOperation::ConfigureZone(zone_config) => {
					// For CockroachDB compatibility: CONFIGURE ZONE USING ...
					writer.push_keyword("CONFIGURE ZONE USING");
					writer.push_space();

					let mut parts = Vec::new();

					if let Some(num_replicas) = zone_config.num_replicas {
						parts.push(format!("num_replicas = {}", num_replicas));
					}

					if !zone_config.constraints.is_empty() {
						let constraints: Vec<String> = zone_config
							.constraints
							.iter()
							.map(ToString::to_string)
							.collect();
						parts.push(format!("constraints = '[{}]'", constraints.join(", ")));
					}

					if !zone_config.lease_preferences.is_empty() {
						let prefs: Vec<String> = zone_config
							.lease_preferences
							.iter()
							.map(|p| format!("[{}]", p))
							.collect();
						parts.push(format!("lease_preferences = '[{}]'", prefs.join(", ")));
					}

					writer.push(&parts.join(", "));
				}
			}
		}

		writer.finish()
	}

	fn build_drop_database(&self, stmt: &crate::query::DropDatabaseStatement) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// DROP DATABASE
		writer.push_keyword("DROP DATABASE");

		// IF EXISTS
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		// Database name
		if let Some(name) = &stmt.database_name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// WITH (FORCE) - PostgreSQL 13+
		if stmt.force {
			writer.push_space();
			writer.push_keyword("WITH");
			writer.push(" (");
			writer.push("FORCE");
			writer.push(")");
		}

		writer.finish()
	}

	fn build_analyze(&self, stmt: &crate::query::AnalyzeStatement) -> (String, Values) {
		use crate::types::Iden;
		let mut writer = SqlWriter::new();

		writer.push_keyword("ANALYZE");

		if stmt.verbose {
			writer.push_keyword("VERBOSE");
		}

		// Tables and columns
		if !stmt.tables.is_empty() {
			writer.push_space();
			writer.push_list(&stmt.tables, ", ", |w, table| {
				w.push_identifier(&Iden::to_string(table.table.as_ref()), |s| {
					self.escape_iden(s)
				});
				if !table.columns.is_empty() {
					w.push(" (");
					w.push_list(&table.columns, ", ", |w2, col| {
						w2.push_identifier(&Iden::to_string(col.as_ref()), |s| self.escape_iden(s));
					});
					w.push(")");
				}
			});
		}

		writer.finish()
	}

	fn build_vacuum(&self, stmt: &crate::query::VacuumStatement) -> (String, Values) {
		use crate::types::Iden;
		let mut writer = SqlWriter::new();

		writer.push_keyword("VACUUM");

		// Options
		if stmt.full {
			writer.push_keyword("FULL");
		}
		if stmt.freeze {
			writer.push_keyword("FREEZE");
		}
		if stmt.verbose {
			writer.push_keyword("VERBOSE");
		}
		if stmt.analyze {
			writer.push_keyword("ANALYZE");
		}

		// Tables
		if !stmt.tables.is_empty() {
			writer.push_space();
			writer.push_list(&stmt.tables, ", ", |w, table| {
				w.push_identifier(&Iden::to_string(table.as_ref()), |s| self.escape_iden(s));
			});
		}

		writer.finish()
	}

	fn build_create_materialized_view(
		&self,
		stmt: &crate::query::CreateMaterializedViewStatement,
	) -> (String, Values) {
		use crate::types::Iden;
		let mut writer = SqlWriter::new();

		writer.push_keyword("CREATE MATERIALIZED VIEW");

		// IF NOT EXISTS
		if stmt.def.if_not_exists {
			writer.push_keyword("IF NOT EXISTS");
		}

		// View name
		writer.push_space();
		writer.push_identifier(&Iden::to_string(stmt.def.name.as_ref()), |s| {
			self.escape_iden(s)
		});

		// Column names
		if !stmt.def.columns.is_empty() {
			writer.push_space();
			writer.push("(");
			writer.push_list(&stmt.def.columns, ", ", |w, col| {
				w.push_identifier(&Iden::to_string(col.as_ref()), |s| self.escape_iden(s));
			});
			writer.push(")");
		}

		// TABLESPACE
		if let Some(ref tablespace) = stmt.def.tablespace {
			writer.push_keyword("TABLESPACE");
			writer.push_space();
			writer.push_identifier(&Iden::to_string(tablespace.as_ref()), |s| {
				self.escape_iden(s)
			});
		}

		// AS SELECT
		if let Some(ref select) = stmt.select {
			writer.push_keyword("AS");
			writer.push_space();
			let (select_sql, select_values) = self.build_select(select);
			writer.push(&select_sql);

			// WITH [NO] DATA
			if let Some(with_data) = stmt.def.with_data {
				writer.push_space();
				if with_data {
					writer.push_keyword("WITH DATA");
				} else {
					writer.push_keyword("WITH NO DATA");
				}
			}

			let (sql, _) = writer.finish();
			return (sql, select_values);
		}

		writer.finish()
	}

	fn build_alter_materialized_view(
		&self,
		stmt: &crate::query::AlterMaterializedViewStatement,
	) -> (String, Values) {
		use crate::types::{Iden, MaterializedViewOperation};
		let mut writer = SqlWriter::new();

		writer.push_keyword("ALTER MATERIALIZED VIEW");

		// View name
		if let Some(ref name) = stmt.name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// Operations
		for operation in &stmt.operations {
			writer.push_space();
			match operation {
				MaterializedViewOperation::Rename(new_name) => {
					writer.push_keyword("RENAME TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_name.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				MaterializedViewOperation::OwnerTo(new_owner) => {
					writer.push_keyword("OWNER TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_owner.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				MaterializedViewOperation::SetSchema(schema_name) => {
					writer.push_keyword("SET SCHEMA");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(schema_name.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
			}
		}

		writer.finish()
	}

	fn build_drop_materialized_view(
		&self,
		stmt: &crate::query::DropMaterializedViewStatement,
	) -> (String, Values) {
		use crate::types::Iden;
		let mut writer = SqlWriter::new();

		writer.push_keyword("DROP MATERIALIZED VIEW");

		// IF EXISTS
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		// View names
		writer.push_space();
		writer.push_list(&stmt.names, ", ", |w, name| {
			w.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		});

		// CASCADE or RESTRICT
		if stmt.cascade {
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}

	fn build_refresh_materialized_view(
		&self,
		stmt: &crate::query::RefreshMaterializedViewStatement,
	) -> (String, Values) {
		use crate::types::Iden;
		let mut writer = SqlWriter::new();

		writer.push_keyword("REFRESH MATERIALIZED VIEW");

		// CONCURRENTLY
		if stmt.concurrently {
			writer.push_keyword("CONCURRENTLY");
		}

		// View name
		if let Some(ref name) = stmt.name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// WITH [NO] DATA
		if let Some(with_data) = stmt.with_data {
			writer.push_space();
			if with_data {
				writer.push_keyword("WITH DATA");
			} else {
				writer.push_keyword("WITH NO DATA");
			}
		}

		writer.finish()
	}

	fn build_create_procedure(
		&self,
		stmt: &crate::query::CreateProcedureStatement,
	) -> (String, Values) {
		use crate::types::{
			Iden,
			function::{FunctionBehavior, FunctionLanguage, FunctionSecurity},
		};

		let mut writer = SqlWriter::new();

		// CREATE [OR REPLACE] PROCEDURE
		writer.push_keyword("CREATE");
		if stmt.procedure_def.or_replace {
			writer.push_keyword("OR REPLACE");
		}
		writer.push_keyword("PROCEDURE");

		// Procedure name
		writer.push_space();
		writer.push_identifier(&Iden::to_string(stmt.procedure_def.name.as_ref()), |s| {
			self.escape_iden(s)
		});

		// Parameters (param1 type1, param2 type2, ...)
		writer.push("(");
		let mut first = true;
		for param in &stmt.procedure_def.parameters {
			if !first {
				writer.push(", ");
			}
			first = false;

			// Parameter mode (IN, OUT, INOUT, VARIADIC)
			if let Some(mode) = &param.mode {
				use crate::types::function::ParameterMode;
				match mode {
					ParameterMode::In => writer.push("IN "),
					ParameterMode::Out => writer.push("OUT "),
					ParameterMode::InOut => writer.push("INOUT "),
					ParameterMode::Variadic => writer.push("VARIADIC "),
				}
			}

			// Parameter name (optional)
			if let Some(name) = &param.name {
				writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
				writer.push(" ");
			}

			// Parameter type
			if let Some(param_type) = &param.param_type {
				writer.push(param_type);
			}

			// Default value (optional)
			if let Some(default) = &param.default_value {
				writer.push(" DEFAULT ");
				writer.push(default);
			}
		}
		writer.push(")");

		// LANGUAGE
		if let Some(language) = &stmt.procedure_def.language {
			writer.push_keyword("LANGUAGE");
			writer.push_space();
			match language {
				FunctionLanguage::Sql => writer.push("SQL"),
				FunctionLanguage::PlPgSql => writer.push("PLPGSQL"),
				FunctionLanguage::C => writer.push("C"),
				FunctionLanguage::Custom(lang) => writer.push(lang),
			}
		}

		// Behavior (IMMUTABLE/STABLE/VOLATILE)
		if let Some(behavior) = &stmt.procedure_def.behavior {
			writer.push_space();
			match behavior {
				FunctionBehavior::Immutable => writer.push_keyword("IMMUTABLE"),
				FunctionBehavior::Stable => writer.push_keyword("STABLE"),
				FunctionBehavior::Volatile => writer.push_keyword("VOLATILE"),
			}
		}

		// Security (SECURITY DEFINER/INVOKER)
		if let Some(security) = &stmt.procedure_def.security {
			writer.push_space();
			match security {
				FunctionSecurity::Definer => writer.push_keyword("SECURITY DEFINER"),
				FunctionSecurity::Invoker => writer.push_keyword("SECURITY INVOKER"),
			}
		}

		// AS 'body'
		if let Some(body) = &stmt.procedure_def.body {
			writer.push_keyword("AS");
			writer.push_space();
			let delimiter = generate_safe_dollar_quote_delimiter(body);
			writer.push(&delimiter);
			writer.push(body);
			writer.push(&delimiter);
		}

		writer.finish()
	}

	fn build_alter_procedure(
		&self,
		stmt: &crate::query::AlterProcedureStatement,
	) -> (String, Values) {
		use crate::types::{
			Iden,
			function::{FunctionBehavior, FunctionSecurity},
			procedure::ProcedureOperation,
		};

		let mut writer = SqlWriter::new();

		// ALTER PROCEDURE
		writer.push_keyword("ALTER PROCEDURE");

		// Procedure name
		if let Some(name) = &stmt.name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// Procedure signature (for overloaded procedures)
		if !stmt.parameters.is_empty() {
			writer.push("(");
			let mut first = true;
			for param in &stmt.parameters {
				if !first {
					writer.push(", ");
				}
				first = false;

				// Parameter name (optional in signature)
				if let Some(name) = &param.name {
					let name_str = Iden::to_string(name.as_ref());
					if !name_str.is_empty() {
						writer.push_identifier(&name_str, |s| self.escape_iden(s));
						writer.push(" ");
					}
				}

				// Parameter type
				if let Some(param_type) = &param.param_type {
					writer.push(param_type);
				}
			}
			writer.push(")");
		}

		// ALTER PROCEDURE operation
		if let Some(operation) = &stmt.operation {
			writer.push_space();
			match operation {
				ProcedureOperation::RenameTo(new_name) => {
					writer.push_keyword("RENAME TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_name.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				ProcedureOperation::OwnerTo(new_owner) => {
					writer.push_keyword("OWNER TO");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_owner.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				ProcedureOperation::SetSchema(new_schema) => {
					writer.push_keyword("SET SCHEMA");
					writer.push_space();
					writer.push_identifier(&Iden::to_string(new_schema.as_ref()), |s| {
						self.escape_iden(s)
					});
				}
				ProcedureOperation::SetBehavior(behavior) => match behavior {
					FunctionBehavior::Immutable => writer.push_keyword("IMMUTABLE"),
					FunctionBehavior::Stable => writer.push_keyword("STABLE"),
					FunctionBehavior::Volatile => writer.push_keyword("VOLATILE"),
				},
				ProcedureOperation::SetSecurity(security) => match security {
					FunctionSecurity::Definer => writer.push_keyword("SECURITY DEFINER"),
					FunctionSecurity::Invoker => writer.push_keyword("SECURITY INVOKER"),
				},
			}
		}

		writer.finish()
	}

	fn build_drop_procedure(
		&self,
		stmt: &crate::query::DropProcedureStatement,
	) -> (String, Values) {
		use crate::types::Iden;

		let mut writer = SqlWriter::new();

		// DROP PROCEDURE
		writer.push_keyword("DROP PROCEDURE");

		// IF EXISTS
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		// Procedure name
		if let Some(name) = &stmt.name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		// Procedure signature (for overloaded procedures)
		if !stmt.parameters.is_empty() {
			writer.push("(");
			let mut first = true;
			for param in &stmt.parameters {
				if !first {
					writer.push(", ");
				}
				first = false;

				// Parameter name (optional in signature)
				if let Some(name) = &param.name {
					let name_str = Iden::to_string(name.as_ref());
					if !name_str.is_empty() {
						writer.push_identifier(&name_str, |s| self.escape_iden(s));
						writer.push(" ");
					}
				}

				// Parameter type
				if let Some(param_type) = &param.param_type {
					writer.push(param_type);
				}
			}
			writer.push(")");
		}

		// CASCADE
		if stmt.cascade {
			writer.push_keyword("CASCADE");
		}

		writer.finish()
	}
	//
	fn build_create_type(&self, stmt: &crate::query::CreateTypeStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push_keyword("CREATE TYPE");
		writer.push_space();

		// Type name
		if let Some(name) = &stmt.name {
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		}

		// Type definition
		if let Some(kind) = &stmt.kind {
			use crate::types::type_def::TypeKind;
			match kind {
				TypeKind::Enum { values } => {
					writer.push_space();
					writer.push_keyword("AS ENUM");
					writer.push_space();
					writer.push("(");
					writer.push_list(values, ", ", |w, value| {
						w.push("'");
						w.push(&value.replace('\'', "''"));
						w.push("'");
					});
					writer.push(")");
				}
				TypeKind::Composite { attributes } => {
					writer.push_space();
					writer.push_keyword("AS");
					writer.push_space();
					writer.push("(");
					writer.push_list(attributes, ", ", |w, (name, type_name)| {
						w.push_identifier(name, |s| self.escape_iden(s));
						w.push_space();
						w.push(type_name);
					});
					writer.push(")");
				}
				TypeKind::Domain {
					base_type,
					constraint,
					default,
					not_null,
				} => {
					writer.push_space();
					writer.push_keyword("AS");
					writer.push_space();
					writer.push(base_type);

					// DEFAULT clause
					if let Some(default_val) = default {
						writer.push_space();
						writer.push_keyword("DEFAULT");
						writer.push_space();
						writer.push(default_val);
					}

					// CONSTRAINT clause
					if let Some(check) = constraint {
						writer.push_space();
						writer.push(check);
					}

					// NOT NULL clause
					if *not_null {
						writer.push_space();
						writer.push_keyword("NOT NULL");
					}
				}
				TypeKind::Range {
					subtype,
					subtype_diff,
					canonical,
				} => {
					writer.push_space();
					writer.push_keyword("AS RANGE");
					writer.push_space();
					writer.push("(");
					writer.push("SUBTYPE = ");
					writer.push(subtype);

					// SUBTYPE_DIFF clause
					if let Some(diff_fn) = subtype_diff {
						writer.push(", SUBTYPE_DIFF = ");
						writer.push(diff_fn);
					}

					// CANONICAL clause
					if let Some(canonical_fn) = canonical {
						writer.push(", CANONICAL = ");
						writer.push(canonical_fn);
					}

					writer.push(")");
				}
			}
		}

		writer.finish()
	}

	fn build_alter_type(&self, stmt: &crate::query::AlterTypeStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push_keyword("ALTER TYPE");
		writer.push_space();
		writer.push_identifier(&stmt.name.to_string(), |s| self.escape_iden(s));

		// Process operations
		for operation in &stmt.operations {
			writer.push_space();
			use crate::types::type_def::TypeOperation;
			match operation {
				TypeOperation::RenameTo(new_name) => {
					writer.push_keyword("RENAME TO");
					writer.push_space();
					writer.push_identifier(&new_name.to_string(), |s| self.escape_iden(s));
				}
				TypeOperation::OwnerTo(owner) => {
					writer.push_keyword("OWNER TO");
					writer.push_space();
					writer.push_identifier(&owner.to_string(), |s| self.escape_iden(s));
				}
				TypeOperation::SetSchema(schema) => {
					writer.push_keyword("SET SCHEMA");
					writer.push_space();
					writer.push_identifier(&schema.to_string(), |s| self.escape_iden(s));
				}
				TypeOperation::AddValue(value, position) => {
					writer.push_keyword("ADD VALUE");
					writer.push_space();
					writer.push("'");
					writer.push(&value.replace('\'', "''"));
					writer.push("'");

					if let Some(pos) = position {
						writer.push_space();
						writer.push_keyword("BEFORE");
						writer.push_space();
						writer.push("'");
						writer.push(&pos.replace('\'', "''"));
						writer.push("'");
					}
				}
				TypeOperation::RenameValue(old_value, new_value) => {
					writer.push_keyword("RENAME VALUE");
					writer.push_space();
					writer.push("'");
					writer.push(&old_value.replace('\'', "''"));
					writer.push("'");
					writer.push_space();
					writer.push_keyword("TO");
					writer.push_space();
					writer.push("'");
					writer.push(&new_value.replace('\'', "''"));
					writer.push("'");
				}
				TypeOperation::AddConstraint(name, check) => {
					writer.push_keyword("ADD CONSTRAINT");
					writer.push_space();
					writer.push_identifier(name, |s| self.escape_iden(s));
					writer.push_space();
					writer.push(check);
				}
				TypeOperation::DropConstraint(name, if_exists) => {
					writer.push_keyword("DROP CONSTRAINT");
					if *if_exists {
						writer.push_space();
						writer.push_keyword("IF EXISTS");
					}
					writer.push_space();
					writer.push_identifier(name, |s| self.escape_iden(s));
				}
				TypeOperation::SetDefault(value) => {
					writer.push_keyword("SET DEFAULT");
					writer.push_space();
					writer.push(value);
				}
				TypeOperation::DropDefault => {
					writer.push_keyword("DROP DEFAULT");
				}
				TypeOperation::SetNotNull => {
					writer.push_keyword("SET NOT NULL");
				}
				TypeOperation::DropNotNull => {
					writer.push_keyword("DROP NOT NULL");
				}
			}
		}

		writer.finish()
	}

	fn build_drop_type(&self, stmt: &crate::query::DropTypeStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push_keyword("DROP TYPE");
		writer.push_space();

		// IF EXISTS clause
		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
			writer.push_space();
		}

		// Type name
		writer.push_identifier(&stmt.name.to_string(), |s| self.escape_iden(s));

		// CASCADE/RESTRICT clause
		if stmt.cascade {
			writer.push_space();
			writer.push_keyword("CASCADE");
		} else if stmt.restrict {
			writer.push_space();
			writer.push_keyword("RESTRICT");
		}

		writer.finish()
	}
}

// Helper methods for DDL operations
impl PostgresQueryBuilder {
	/// Convert ColumnType to PostgreSQL SQL type string
	///
	/// Note: The `self` parameter is used in recursive calls for Array types (e.g., INTEGER[]).
	/// The clippy::only_used_in_recursion warning is allowed because this is the intended design
	/// for handling nested array types, and keeping `self` maintains consistency with other
	/// backend implementations.
	#[allow(clippy::only_used_in_recursion)]
	fn column_type_to_sql(&self, col_type: &crate::types::ColumnType) -> String {
		use crate::types::ColumnType;
		match col_type {
			ColumnType::Char(len) => format!("CHAR({})", len.unwrap_or(1)),
			ColumnType::String(len) => {
				if let Some(l) = len {
					format!("VARCHAR({})", l)
				} else {
					"VARCHAR".to_string()
				}
			}
			ColumnType::Text => "TEXT".to_string(),
			ColumnType::TinyInteger => "SMALLINT".to_string(),
			ColumnType::SmallInteger => "SMALLINT".to_string(),
			ColumnType::Integer => "INTEGER".to_string(),
			ColumnType::BigInteger => "BIGINT".to_string(),
			ColumnType::Float => "REAL".to_string(),
			ColumnType::Double => "DOUBLE PRECISION".to_string(),
			ColumnType::Decimal(precision) => {
				// PostgreSQL uses NUMERIC as the canonical name (DECIMAL is an alias)
				if let Some((p, s)) = precision {
					format!("NUMERIC({}, {})", p, s)
				} else {
					"NUMERIC".to_string()
				}
			}
			ColumnType::DateTime => "TIMESTAMP".to_string(),
			ColumnType::Timestamp => "TIMESTAMP".to_string(),
			ColumnType::TimestampWithTimeZone => "TIMESTAMP WITH TIME ZONE".to_string(),
			ColumnType::Time => "TIME".to_string(),
			ColumnType::Date => "DATE".to_string(),
			ColumnType::Binary(_len) => {
				// PostgreSQL BYTEA does not support length modifiers
				"BYTEA".to_string()
			}
			ColumnType::VarBinary(_len) => {
				// PostgreSQL BYTEA does not support length modifiers
				"BYTEA".to_string()
			}
			ColumnType::Blob => "BYTEA".to_string(),
			ColumnType::Boolean => "BOOLEAN".to_string(),
			ColumnType::Json => "JSON".to_string(),
			ColumnType::JsonBinary => "JSONB".to_string(),
			ColumnType::Uuid => "UUID".to_string(),
			ColumnType::Array(inner_type) => {
				format!("{}[]", self.column_type_to_sql(inner_type))
			}
			ColumnType::Custom(name) => name.clone(),
		}
	}

	fn write_table_constraint(
		&self,
		writer: &mut SqlWriter,
		constraint: &crate::types::TableConstraint,
	) {
		use crate::types::TableConstraint;
		match constraint {
			TableConstraint::PrimaryKey { name, columns } => {
				if let Some(n) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&n.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("PRIMARY KEY");
				writer.push_space();
				writer.push("(");
				writer.push_list(columns, ", ", |w, col| {
					w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				});
				writer.push(")");
			}
			TableConstraint::Unique { name, columns } => {
				if let Some(n) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&n.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("UNIQUE");
				writer.push_space();
				writer.push("(");
				writer.push_list(columns, ", ", |w, col| {
					w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				});
				writer.push(")");
			}
			TableConstraint::ForeignKey {
				name,
				columns,
				ref_table,
				ref_columns,
				on_delete,
				on_update,
			} => {
				if let Some(n) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&n.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("FOREIGN KEY");
				writer.push_space();
				writer.push("(");
				writer.push_list(columns, ", ", |w, col| {
					w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				});
				writer.push(")");
				writer.push_space();
				writer.push_keyword("REFERENCES");
				writer.push_space();
				self.write_table_ref(writer, ref_table);
				writer.push_space();
				writer.push("(");
				writer.push_list(ref_columns, ", ", |w, col| {
					w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				});
				writer.push(")");
				if let Some(action) = on_delete {
					writer.push_space();
					writer.push_keyword("ON DELETE");
					writer.push_space();
					writer.push_keyword(self.foreign_key_action_to_sql(action));
				}
				if let Some(action) = on_update {
					writer.push_space();
					writer.push_keyword("ON UPDATE");
					writer.push_space();
					writer.push_keyword(self.foreign_key_action_to_sql(action));
				}
			}
			TableConstraint::Check { name, expr } => {
				if let Some(n) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&n.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("CHECK");
				writer.push_space();
				writer.push("(");
				self.write_simple_expr(writer, expr);
				writer.push(")");
			}
		}
	}

	fn foreign_key_action_to_sql(&self, action: &crate::types::ForeignKeyAction) -> &'static str {
		use crate::types::ForeignKeyAction;
		match action {
			ForeignKeyAction::Restrict => "RESTRICT",
			ForeignKeyAction::Cascade => "CASCADE",
			ForeignKeyAction::SetNull => "SET NULL",
			ForeignKeyAction::SetDefault => "SET DEFAULT",
			ForeignKeyAction::NoAction => "NO ACTION",
		}
	}

	fn index_method_to_sql(&self, method: &crate::query::IndexMethod) -> &'static str {
		use crate::query::IndexMethod;
		match method {
			IndexMethod::BTree => "BTREE",
			IndexMethod::Hash => "HASH",
			IndexMethod::Gist => "GIST",
			IndexMethod::Gin => "GIN",
			IndexMethod::Brin => "BRIN",
			IndexMethod::FullText => "GIN", // PostgreSQL uses GIN for full-text search
			IndexMethod::Spatial => "GIST", // PostgreSQL uses GIST for spatial indexes
		}
	}
}

impl PostgresQueryBuilder {
	/// Format a role specification for PostgreSQL
	///
	/// # Arguments
	///
	/// * `spec` - The role specification to format
	///
	/// # Returns
	///
	/// The SQL representation of the role specification
	fn format_role_specification(spec: &crate::dcl::RoleSpecification) -> &str {
		use crate::dcl::RoleSpecification;

		match spec {
			RoleSpecification::RoleName(name) => name,
			RoleSpecification::CurrentRole => "CURRENT_ROLE",
			RoleSpecification::CurrentUser => "CURRENT_USER",
			RoleSpecification::SessionUser => "SESSION_USER",
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		expr::{Expr, ExprTrait},
		query::Query,
		types::{Alias, IntoIden},
	};
	use rstest::rstest;

	#[test]
	fn test_escape_identifier() {
		let builder = PostgresQueryBuilder::new();
		assert_eq!(builder.escape_identifier("user"), "\"user\"");
		assert_eq!(builder.escape_identifier("table_name"), "\"table_name\"");
	}

	#[test]
	fn test_escape_identifier_with_quotes() {
		let builder = PostgresQueryBuilder::new();
		assert_eq!(builder.escape_identifier("user\"name"), "\"user\"\"name\"");
	}

	#[test]
	fn test_format_placeholder() {
		let builder = PostgresQueryBuilder::new();
		assert_eq!(builder.format_placeholder(1), "$1");
		assert_eq!(builder.format_placeholder(2), "$2");
		assert_eq!(builder.format_placeholder(10), "$10");
	}

	#[test]
	fn test_select_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("id").column("name").from("users");

		let (sql, values) = builder.build_select(&stmt);
		assert_eq!(sql, "SELECT \"id\", \"name\" FROM \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_select_asterisk() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.from("users");

		let (sql, values) = builder.build_select(&stmt);
		assert_eq!(sql, "SELECT * FROM \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_select_with_where() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("id")
			.from("users")
			.and_where(Expr::col("active").eq(true));

		let (sql, _values) = builder.build_select(&stmt);
		// Note: The exact WHERE clause format depends on Expr implementation
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("FROM"));
		assert!(sql.contains("WHERE"));
	}

	#[test]
	fn test_select_with_limit_offset() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("id").from("users").limit(10).offset(20);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("FROM"));
		assert!(sql.contains("LIMIT"));
		assert!(sql.contains("OFFSET"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_insert_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name", "email"])
			.values_panic(["Alice", "alice@example.com"]);

		let (sql, values) = builder.build_insert(&stmt);
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"name\", \"email\") VALUES ($1, $2)"
		);
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_insert_multiple_rows() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name", "email"])
			.values_panic(["Alice", "alice@example.com"])
			.values_panic(["Bob", "bob@example.com"]);

		let (sql, values) = builder.build_insert(&stmt);
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"name\", \"email\") VALUES ($1, $2), ($3, $4)"
		);
		assert_eq!(values.len(), 4);
	}

	#[test]
	fn test_insert_with_returning() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name"])
			.values_panic(["Alice"])
			.returning(["id", "created_at"]);

		let (sql, values) = builder.build_insert(&stmt);
		assert!(sql.contains("INSERT INTO"));
		assert!(sql.contains("VALUES"));
		assert!(sql.contains("RETURNING"));
		assert!(sql.contains("\"id\""));
		assert!(sql.contains("\"created_at\""));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_insert_with_returning_all() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name"])
			.values_panic(["Alice"])
			.returning_all();

		let (sql, values) = builder.build_insert(&stmt);
		assert!(sql.contains("RETURNING *"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_insert_from_subquery() {
		let builder = PostgresQueryBuilder::new();

		// Create a SELECT subquery
		let select = Query::select()
			.column("name")
			.column("email")
			.from("temp_users")
			.to_owned();

		// Create an INSERT with subquery
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name", "email"])
			.from_subquery(select);

		let (sql, values) = builder.build_insert(&stmt);
		assert!(sql.contains("INSERT INTO \"users\""));
		assert!(sql.contains("\"name\", \"email\""));
		assert!(sql.contains("SELECT \"name\", \"email\" FROM \"temp_users\""));
		assert!(!sql.contains("VALUES"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_insert_from_subquery_with_where() {
		let builder = PostgresQueryBuilder::new();

		// Create a SELECT subquery with WHERE clause
		let select = Query::select()
			.column("name")
			.column("email")
			.from("temp_users")
			.and_where(Expr::col("active").eq(true))
			.to_owned();

		// Create an INSERT with subquery
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name", "email"])
			.from_subquery(select);

		let (sql, values) = builder.build_insert(&stmt);
		assert!(sql.contains("INSERT INTO \"users\""));
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("FROM \"temp_users\""));
		assert!(sql.contains("WHERE"));
		assert_eq!(values.len(), 1); // true value from WHERE clause
	}

	#[test]
	fn test_update_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::update();
		stmt.table("users")
			.value("name", "Alice")
			.value("email", "alice@example.com");

		let (sql, values) = builder.build_update(&stmt);
		assert_eq!(sql, "UPDATE \"users\" SET \"name\" = $1, \"email\" = $2");
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_update_with_where() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::update();
		stmt.table("users")
			.value("active", false)
			.and_where(Expr::col("id").eq(1));

		let (sql, values) = builder.build_update(&stmt);
		assert!(sql.contains("UPDATE"));
		assert!(sql.contains("SET"));
		assert!(sql.contains("WHERE"));
		assert_eq!(values.len(), 2); // false + 1
	}

	#[test]
	fn test_update_with_returning() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::update();
		stmt.table("users")
			.value("active", false)
			.and_where(Expr::col("id").eq(1))
			.returning(["id", "updated_at"]);

		let (sql, values) = builder.build_update(&stmt);
		assert!(sql.contains("UPDATE"));
		assert!(sql.contains("RETURNING"));
		assert!(sql.contains("\"id\""));
		assert!(sql.contains("\"updated_at\""));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_delete_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::delete();
		stmt.from_table("users")
			.and_where(Expr::col("active").eq(false));

		let (sql, values) = builder.build_delete(&stmt);
		assert!(sql.contains("DELETE FROM"));
		assert!(sql.contains("\"users\""));
		assert!(sql.contains("WHERE"));
		assert_eq!(values.len(), 1); // false
	}

	#[test]
	fn test_delete_no_where() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::delete();
		stmt.from_table("users");

		let (sql, values) = builder.build_delete(&stmt);
		assert_eq!(sql, "DELETE FROM \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_delete_with_returning() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::delete();
		stmt.from_table("users")
			.and_where(Expr::col("id").eq(1))
			.returning(["id", "name"]);

		let (sql, values) = builder.build_delete(&stmt);
		assert!(sql.contains("DELETE FROM"));
		assert!(sql.contains("RETURNING"));
		assert!(sql.contains("\"id\""));
		assert!(sql.contains("\"name\""));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_delete_with_returning_all() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::delete();
		stmt.from_table("users")
			.and_where(Expr::col("id").eq(1))
			.returning_all();

		let (sql, values) = builder.build_delete(&stmt);
		assert!(sql.contains("RETURNING *"));
		assert_eq!(values.len(), 1);
	}

	// JOIN tests

	#[test]
	fn test_inner_join_simple() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("users.name")
			.column("orders.amount")
			.from("users")
			.inner_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("FROM \"users\""));
		assert!(sql.contains("INNER JOIN \"orders\""));
		assert!(sql.contains("ON \"users\".\"id\" = \"orders\".\"user_id\""));
	}

	#[test]
	fn test_left_join() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("users.name")
			.column("profiles.bio")
			.from("users")
			.left_join(
				"profiles",
				Expr::col(("users", "id")).eq(Expr::col(("profiles", "user_id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("LEFT JOIN \"profiles\""));
		assert!(sql.contains("ON \"users\".\"id\" = \"profiles\".\"user_id\""));
	}

	#[test]
	fn test_right_join() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("users.name")
			.column("orders.amount")
			.from("users")
			.right_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("RIGHT JOIN \"orders\""));
		assert!(sql.contains("ON \"users\".\"id\" = \"orders\".\"user_id\""));
	}

	#[test]
	fn test_full_outer_join() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("users.name")
			.column("orders.amount")
			.from("users")
			.full_outer_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("FULL OUTER JOIN \"orders\""));
		assert!(sql.contains("ON \"users\".\"id\" = \"orders\".\"user_id\""));
	}

	#[test]
	fn test_cross_join() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("users.name")
			.column("roles.title")
			.from("users")
			.cross_join("roles");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("CROSS JOIN \"roles\""));
		assert!(!sql.contains("ON"));
	}

	#[test]
	fn test_multiple_joins() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("users.name")
			.column("orders.amount")
			.column("products.title")
			.from("users")
			.inner_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			)
			.inner_join(
				"products",
				Expr::col(("orders", "product_id")).eq(Expr::col(("products", "id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("INNER JOIN \"orders\""));
		assert!(sql.contains("INNER JOIN \"products\""));
		assert!(sql.contains("\"users\".\"id\" = \"orders\".\"user_id\""));
		assert!(sql.contains("\"orders\".\"product_id\" = \"products\".\"id\""));
	}

	#[test]
	fn test_join_with_complex_condition() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("users.name")
			.column("orders.amount")
			.from("users")
			.inner_join(
				"orders",
				Expr::col(("users", "id"))
					.eq(Expr::col(("orders", "user_id")))
					.and(Expr::col(("orders", "status")).eq("active")),
			);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("INNER JOIN \"orders\""));
		assert!(sql.contains("ON"));
		assert!(sql.contains("\"users\".\"id\" = \"orders\".\"user_id\""));
		assert!(sql.contains("AND"));
		assert!(sql.contains("\"orders\".\"status\" = $"));
		assert_eq!(values.len(), 1);
	}

	// GROUP BY / HAVING tests

	#[test]
	fn test_group_by_single_column() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("category")
			.from("products")
			.group_by("category");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("GROUP BY \"category\""));
	}

	#[test]
	fn test_group_by_multiple_columns() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("category")
			.column("brand")
			.from("products")
			.group_by("category")
			.group_by("brand");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("GROUP BY \"category\", \"brand\""));
	}

	#[test]
	fn test_group_by_with_count() {
		use crate::expr::SimpleExpr;
		use crate::types::{ColumnRef, IntoIden};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("category")
			.expr(SimpleExpr::FunctionCall(
				"COUNT".into_iden(),
				vec![SimpleExpr::Column(ColumnRef::Asterisk)],
			))
			.from("products")
			.group_by("category");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("COUNT(*)"));
		assert!(sql.contains("GROUP BY \"category\""));
	}

	#[test]
	fn test_having_simple() {
		use crate::expr::SimpleExpr;
		use crate::types::{BinOper, ColumnRef, IntoIden};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		let count_expr = SimpleExpr::FunctionCall(
			"COUNT".into_iden(),
			vec![SimpleExpr::Column(ColumnRef::Asterisk)],
		);

		stmt.column("category")
			.expr(count_expr.clone())
			.from("products")
			.group_by("category")
			.and_having(SimpleExpr::Binary(
				Box::new(count_expr),
				BinOper::GreaterThan,
				Box::new(SimpleExpr::Value(5.into())),
			));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("GROUP BY \"category\""));
		assert!(sql.contains("HAVING"));
		assert!(sql.contains("COUNT(*)"));
		assert!(sql.contains(">"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_group_by_having_with_sum() {
		use crate::expr::SimpleExpr;
		use crate::types::{BinOper, ColumnRef, IntoIden};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		let sum_expr = SimpleExpr::FunctionCall(
			"SUM".into_iden(),
			vec![SimpleExpr::Column(ColumnRef::column("amount"))],
		);

		stmt.column("user_id")
			.expr(sum_expr.clone())
			.from("orders")
			.group_by("user_id")
			.and_having(SimpleExpr::Binary(
				Box::new(sum_expr),
				BinOper::GreaterThan,
				Box::new(SimpleExpr::Value(1000.into())),
			));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("SUM(\"amount\")"));
		assert!(sql.contains("GROUP BY \"user_id\""));
		assert!(sql.contains("HAVING"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_select_distinct() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("category").from("products").distinct();

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.starts_with("SELECT DISTINCT"));
		assert!(sql.contains("\"category\""));
		assert!(sql.contains("FROM \"products\""));
	}

	#[test]
	fn test_select_distinct_on() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("id")
			.column("name")
			.from("users")
			.distinct_on(vec!["category"])
			.order_by("category", crate::types::Order::Asc);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("SELECT DISTINCT ON (\"category\")"));
		assert!(sql.contains("\"id\""));
		assert!(sql.contains("\"name\""));
		assert!(sql.contains("ORDER BY \"category\" ASC"));
	}

	#[test]
	#[should_panic(expected = "PostgreSQL does not support DISTINCT ROW")]
	fn test_select_distinct_row_panics() {
		use crate::query::SelectDistinct;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("products");
		stmt.distinct = Some(SelectDistinct::DistinctRow);

		let _ = builder.build_select(&stmt);
	}

	#[test]
	fn test_select_union() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt1 = Query::select();
		stmt1.column("id").from("users");

		let mut stmt2 = Query::select();
		stmt2.column("id").from("customers");

		stmt1.union(stmt2);

		let (sql, _values) = builder.build_select(&stmt1);
		assert!(sql.contains("SELECT \"id\" FROM \"users\""));
		assert!(sql.contains("UNION SELECT \"id\" FROM \"customers\""));
	}

	#[test]
	fn test_select_union_all() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt1 = Query::select();
		stmt1.column("name").from("products");

		let mut stmt2 = Query::select();
		stmt2.column("name").from("archived_products");

		stmt1.union_all(stmt2);

		let (sql, _values) = builder.build_select(&stmt1);
		assert!(sql.contains("SELECT \"name\" FROM \"products\""));
		assert!(sql.contains("UNION ALL SELECT \"name\" FROM \"archived_products\""));
	}

	#[test]
	fn test_select_intersect() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt1 = Query::select();
		stmt1.column("email").from("subscribers");

		let mut stmt2 = Query::select();
		stmt2.column("email").from("customers");

		stmt1.intersect(stmt2);

		let (sql, _values) = builder.build_select(&stmt1);
		assert!(sql.contains("SELECT \"email\" FROM \"subscribers\""));
		assert!(sql.contains("INTERSECT SELECT \"email\" FROM \"customers\""));
	}

	#[test]
	fn test_select_except() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt1 = Query::select();
		stmt1.column("id").from("all_users");

		let mut stmt2 = Query::select();
		stmt2.column("id").from("banned_users");

		stmt1.except(stmt2);

		let (sql, _values) = builder.build_select(&stmt1);
		assert!(sql.contains("SELECT \"id\" FROM \"all_users\""));
		assert!(sql.contains("EXCEPT SELECT \"id\" FROM \"banned_users\""));
	}

	#[test]
	fn test_select_multiple_unions() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt1 = Query::select();
		stmt1.column("id").from("table1");

		let mut stmt2 = Query::select();
		stmt2.column("id").from("table2");

		let mut stmt3 = Query::select();
		stmt3.column("id").from("table3");

		stmt1.union(stmt2);
		stmt1.union_all(stmt3);

		let (sql, _values) = builder.build_select(&stmt1);
		assert!(sql.contains("SELECT \"id\" FROM \"table1\""));
		assert!(sql.contains("UNION SELECT \"id\" FROM \"table2\""));
		assert!(sql.contains("UNION ALL SELECT \"id\" FROM \"table3\""));
	}

	#[test]
	fn test_select_exists_subquery() {
		use crate::expr::Expr;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		// Main query
		stmt.column("name").from("users");

		// Subquery
		let mut subquery = Query::select();
		subquery
			.column("id")
			.from("orders")
			.and_where(Expr::col(("orders", "user_id")).eq(Expr::col(("users", "id"))));

		// Add EXISTS condition
		stmt.and_where(Expr::exists(subquery));

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("SELECT \"name\" FROM \"users\""));
		assert!(sql.contains("WHERE"));
		assert!(sql.contains("EXISTS"));
		assert!(sql.contains("SELECT \"id\" FROM \"orders\""));
	}

	#[test]
	fn test_select_in_subquery() {
		use crate::expr::Expr;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		// Main query
		stmt.column("name").from("users");

		// Subquery
		let mut subquery = Query::select();
		subquery.column("user_id").from("premium_users");

		// Add IN condition
		stmt.and_where(Expr::col("id").in_subquery(subquery));

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("SELECT \"name\" FROM \"users\""));
		assert!(sql.contains("WHERE"));
		assert!(sql.contains("\"id\""));
		assert!(sql.contains("IN"));
		assert!(sql.contains("SELECT \"user_id\" FROM \"premium_users\""));
	}

	#[test]
	fn test_select_not_exists_subquery() {
		use crate::expr::Expr;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		// Main query
		stmt.column("email").from("users");

		// Subquery
		let mut subquery = Query::select();
		subquery
			.column("id")
			.from("banned_users")
			.and_where(Expr::col(("banned_users", "user_id")).eq(Expr::col(("users", "id"))));

		// Add NOT EXISTS condition
		stmt.and_where(Expr::not_exists(subquery));

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("SELECT \"email\" FROM \"users\""));
		assert!(sql.contains("WHERE"));
		assert!(sql.contains("NOT EXISTS"));
		assert!(sql.contains("SELECT \"id\" FROM \"banned_users\""));
	}

	// --- Phase 5: Subquery Edge Case Tests ---

	#[test]
	fn test_not_in_subquery() {
		let builder = PostgresQueryBuilder::new();

		let mut subquery = Query::select();
		subquery
			.column("user_id")
			.from("blocked_users")
			.and_where(Expr::col("reason").eq("spam"));

		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("id").not_in_subquery(subquery));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("NOT IN"));
		assert!(sql.contains("SELECT \"user_id\" FROM \"blocked_users\""));
		assert!(sql.contains("\"reason\" = $"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_subquery_in_select_list() {
		let builder = PostgresQueryBuilder::new();

		let mut subquery = Query::select();
		subquery
			.expr(Expr::col("count"))
			.from("order_counts")
			.and_where(Expr::col(("order_counts", "user_id")).eq(Expr::col(("users", "id"))));

		let mut stmt = Query::select();
		stmt.column("name")
			.expr(Expr::subquery(subquery))
			.from("users");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("\"name\""));
		assert!(sql.contains("(SELECT \"count\" FROM \"order_counts\""));
		assert!(sql.contains("\"order_counts\".\"user_id\" = \"users\".\"id\""));
	}

	#[test]
	fn test_multiple_exists_conditions() {
		let builder = PostgresQueryBuilder::new();

		let mut sub1 = Query::select();
		sub1.column("id")
			.from("orders")
			.and_where(Expr::col(("orders", "user_id")).eq(Expr::col(("users", "id"))));

		let mut sub2 = Query::select();
		sub2.column("id")
			.from("reviews")
			.and_where(Expr::col(("reviews", "user_id")).eq(Expr::col(("users", "id"))));

		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::exists(sub1))
			.and_where(Expr::exists(sub2));

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("EXISTS (SELECT \"id\" FROM \"orders\""));
		assert!(sql.contains("EXISTS (SELECT \"id\" FROM \"reviews\""));
	}

	#[test]
	fn test_nested_subquery() {
		let builder = PostgresQueryBuilder::new();

		let mut inner_subquery = Query::select();
		inner_subquery
			.column("department_id")
			.from("top_departments")
			.and_where(Expr::col("revenue").gt(1000000));

		let mut outer_subquery = Query::select();
		outer_subquery
			.column("id")
			.from("employees")
			.and_where(Expr::col("department_id").in_subquery(inner_subquery));

		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("employee_id").in_subquery(outer_subquery));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("IN (SELECT \"id\" FROM \"employees\""));
		assert!(sql.contains("IN (SELECT \"department_id\" FROM \"top_departments\""));
		assert!(sql.contains("\"revenue\" > $"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_subquery_with_complex_where() {
		let builder = PostgresQueryBuilder::new();

		let mut subquery = Query::select();
		subquery
			.column("product_id")
			.from("inventory")
			.and_where(Expr::col("quantity").gt(0))
			.and_where(Expr::col("warehouse").eq("main"))
			.and_where(Expr::col("status").eq("available"));

		let mut stmt = Query::select();
		stmt.column("name")
			.column("price")
			.from("products")
			.and_where(Expr::col("id").in_subquery(subquery))
			.and_where(Expr::col("active").eq(true));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("IN (SELECT \"product_id\" FROM \"inventory\""));
		assert!(sql.contains("\"quantity\" > $"));
		assert!(sql.contains("\"warehouse\" = $"));
		assert!(sql.contains("\"status\" = $"));
		assert!(sql.contains("\"active\" = $"));
		assert_eq!(values.len(), 4); // 0, "main", "available", true
	}

	#[test]
	fn test_from_subquery_preserves_parameter_values() {
		let builder = PostgresQueryBuilder::new();

		// Arrange
		let mut subquery = Query::select();
		subquery
			.column("id")
			.column("name")
			.from("users")
			.and_where(Expr::col("active").eq(true))
			.and_where(Expr::col("role").eq("admin"));

		let mut stmt = Query::select();
		stmt.column("name")
			.from_subquery(subquery, Alias::new("active_admins"))
			.and_where(Expr::col("name").like("A%"));

		// Act
		let (sql, values) = builder.build_select(&stmt);

		// Assert
		assert!(sql.contains("(SELECT"));
		assert!(sql.contains(") AS \"active_admins\""));
		// Subquery params (true, "admin") + outer param ("A%") = 3 values
		assert_eq!(values.len(), 3);
	}

	#[test]
	fn test_from_subquery_postgres_placeholder_renumbering() {
		let builder = PostgresQueryBuilder::new();

		// Arrange: outer query has params before the FROM subquery
		let mut subquery = Query::select();
		subquery
			.column("id")
			.from("users")
			.and_where(Expr::col("role").eq("admin"));

		let mut stmt = Query::select();
		stmt.column("name")
			.from_subquery(subquery, Alias::new("sub"))
			.and_where(Expr::col("status").eq("active"));

		// Act
		let (sql, values) = builder.build_select(&stmt);

		// Assert: subquery param should be $1, outer param should be $2
		assert!(sql.contains("$1"));
		assert!(sql.contains("$2"));
		assert_eq!(values.len(), 2);
	}

	// --- Phase 5: NULL Handling Tests ---

	#[test]
	fn test_where_is_null() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("deleted_at").is_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"deleted_at\" IS"));
		assert!(sql.to_uppercase().contains("NULL"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_where_is_not_null() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("email").is_not_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"email\" IS NOT"));
		assert!(sql.to_uppercase().contains("NULL"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_is_null_combined_with_other_conditions() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("active").eq(true))
			.and_where(Expr::col("deleted_at").is_null())
			.and_where(Expr::col("email").is_not_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"active\" = $"));
		assert!(sql.contains("\"deleted_at\" IS"));
		assert!(sql.contains("\"email\" IS NOT"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_is_null_with_join() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column(("users", "name"))
			.from("users")
			.left_join(
				"profiles",
				Expr::col(("users", "id")).eq(Expr::col(("profiles", "user_id"))),
			)
			.and_where(Expr::col(("profiles", "id")).is_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("LEFT JOIN \"profiles\""));
		assert!(sql.contains("\"profiles\".\"id\" IS"));
		assert_eq!(values.len(), 0);
	}

	// --- Phase 5: Complex WHERE Clause Tests ---
	#[test]
	fn test_where_or_condition() {
		use crate::expr::Condition;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("users").cond_where(
			Condition::any()
				.add(Expr::col("status").eq("active"))
				.add(Expr::col("status").eq("pending")),
		);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"status\" = $"));
		assert!(sql.contains(" OR "));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_where_between() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("products")
			.and_where(Expr::col("price").between(100, 500));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"price\" BETWEEN $"));
		assert!(sql.contains("AND $"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_where_not_between() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("products")
			.and_where(Expr::col("price").not_between(0, 10));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"price\" NOT BETWEEN $"));
		assert!(sql.contains("AND $"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_where_like() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("email").like("%@gmail.com"));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"email\" LIKE $"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_where_in_values() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("role").is_in(vec!["admin", "moderator", "editor"]));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("\"role\" IN"));
		assert_eq!(values.len(), 3);
	}

	#[test]
	fn test_insert_with_null_value() {
		use crate::value::Value;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(vec!["name", "email", "phone"])
			.values(vec![
				Value::String(Some(Box::new("John".to_string()))),
				Value::String(Some(Box::new("john@example.com".to_string()))),
				Value::String(None),
			])
			.unwrap();

		let (sql, values) = builder.build_insert(&stmt);
		assert!(sql.contains("INSERT INTO \"users\""));
		assert!(sql.contains("\"name\""));
		assert!(sql.contains("\"email\""));
		assert!(sql.contains("\"phone\""));
		// NULL values are inlined directly, not parameterized
		assert!(sql.contains("NULL"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_select_with_single_cte() {
		let builder = PostgresQueryBuilder::new();

		// Create CTE query
		let mut cte_query = Query::select();
		cte_query
			.column("id")
			.column("name")
			.from("employees")
			.and_where(Expr::col("department").eq("Engineering"));

		// Main query using the CTE
		let mut stmt = Query::select();
		stmt.with_cte("eng_employees", cte_query)
			.column("name")
			.from("eng_employees");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH"));
		assert!(sql.contains("\"eng_employees\""));
		assert!(sql.contains("AS"));
		assert!(sql.contains("SELECT \"id\", \"name\" FROM \"employees\""));
		assert!(sql.contains("SELECT \"name\" FROM \"eng_employees\""));
	}

	#[test]
	fn test_select_with_multiple_ctes() {
		let builder = PostgresQueryBuilder::new();

		// First CTE
		let mut cte1 = Query::select();
		cte1.column("id")
			.column("name")
			.from("employees")
			.and_where(Expr::col("department").eq("Engineering"));

		// Second CTE
		let mut cte2 = Query::select();
		cte2.column("id")
			.column("name")
			.from("employees")
			.and_where(Expr::col("department").eq("Sales"));

		// Main query using both CTEs
		let mut stmt = Query::select();
		stmt.with_cte("eng_emp", cte1)
			.with_cte("sales_emp", cte2)
			.column("name")
			.from("eng_emp");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH"));
		assert!(sql.contains("\"eng_emp\""));
		assert!(sql.contains("\"sales_emp\""));
		assert!(sql.contains("AS"));
		// Both CTEs should be present, and there should be a comma between them
		assert!(sql.contains("\"eng_emp\" AS"));
		assert!(sql.contains("\"sales_emp\" AS"));
	}

	#[test]
	fn test_select_with_recursive_cte() {
		let builder = PostgresQueryBuilder::new();

		// Recursive CTE for organizational hierarchy
		let mut cte_query = Query::select();
		cte_query
			.column("id")
			.column("name")
			.column("manager_id")
			.from("employees");

		// Main query using recursive CTE
		let mut stmt = Query::select();
		stmt.with_recursive_cte("employee_hierarchy", cte_query)
			.column("name")
			.from("employee_hierarchy");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH RECURSIVE"));
		assert!(sql.contains("\"employee_hierarchy\""));
		assert!(sql.contains("AS"));
		assert!(sql.contains("SELECT \"id\", \"name\", \"manager_id\" FROM \"employees\""));
		assert!(sql.contains("SELECT \"name\" FROM \"employee_hierarchy\""));
	}

	// Window function tests

	#[test]
	fn test_window_row_number_with_partition_and_order() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("department").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("salary".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::row_number().over(window))
			.column("name")
			.from("employees");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT ROW_NUMBER() OVER ( PARTITION BY "department" ORDER BY "salary" DESC ), "name" FROM "employees""#
		);
	}

	#[test]
	fn test_window_row_number_order_only() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("id".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::row_number().over(window)).from("users");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT ROW_NUMBER() OVER ( ORDER BY "id" ASC ) FROM "users""#
		);
	}

	#[test]
	fn test_window_rank_basic() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("score".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::rank().over(window))
			.column("name")
			.from("students");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT RANK() OVER ( ORDER BY "score" DESC ), "name" FROM "students""#
		);
	}

	#[test]
	fn test_window_rank_with_partition() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("class").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("score".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::rank().over(window))
			.column("name")
			.from("students");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT RANK() OVER ( PARTITION BY "class" ORDER BY "score" DESC ), "name" FROM "students""#
		);
	}

	#[test]
	fn test_window_dense_rank_basic() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("points".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::dense_rank().over(window))
			.column("player")
			.from("scores");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT DENSE_RANK() OVER ( ORDER BY "points" DESC ), "player" FROM "scores""#
		);
	}

	#[test]
	fn test_window_dense_rank_with_partition() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("league").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("points".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::dense_rank().over(window))
			.column("player")
			.from("scores");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT DENSE_RANK() OVER ( PARTITION BY "league" ORDER BY "points" DESC ), "player" FROM "scores""#
		);
	}

	#[test]
	fn test_window_ntile_four_buckets() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("salary".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::ntile(4).over(window))
			.column("name")
			.from("employees");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT NTILE($1) OVER ( ORDER BY "salary" ASC ), "name" FROM "employees""#
		);
	}

	#[test]
	fn test_window_ntile_custom_buckets() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("department").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("salary".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::ntile(3).over(window))
			.column("name")
			.from("employees");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT NTILE($1) OVER ( PARTITION BY "department" ORDER BY "salary" DESC ), "name" FROM "employees""#
		);
	}

	#[test]
	fn test_window_lead_basic() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("date".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::lead(Expr::col("price").into_simple_expr(), None, None).over(window))
			.column("date")
			.from("stocks");

		let (sql, values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT LEAD("price") OVER ( ORDER BY "date" ASC ), "date" FROM "stocks""#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_window_lead_with_offset_and_default() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("ticker").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("date".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(
			Expr::lead(
				Expr::col("price").into_simple_expr(),
				Some(2),
				Some(0.0.into()),
			)
			.over(window),
		)
		.column("date")
		.from("stocks");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("LEAD"));
		assert!(sql.contains("OVER"));
		assert!(sql.contains(r#"PARTITION BY "ticker""#));
		assert!(sql.contains(r#"ORDER BY "date" ASC"#));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_window_lag_basic() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("month".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::lag(Expr::col("revenue").into_simple_expr(), None, None).over(window))
			.column("month")
			.from("sales");

		let (sql, values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT LAG("revenue") OVER ( ORDER BY "month" ASC ), "month" FROM "sales""#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_window_lag_with_offset_and_default() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("product").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("month".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(
			Expr::lag(
				Expr::col("revenue").into_simple_expr(),
				Some(3),
				Some(0.0.into()),
			)
			.over(window),
		)
		.column("month")
		.from("sales");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("LAG"));
		assert!(sql.contains("OVER"));
		assert!(sql.contains(r#"PARTITION BY "product""#));
		assert!(sql.contains(r#"ORDER BY "month" ASC"#));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_window_first_value() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("category").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("price".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::first_value(Expr::col("name").into_simple_expr()).over(window))
			.column("name")
			.from("products");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT FIRST_VALUE("name") OVER ( PARTITION BY "category" ORDER BY "price" ASC ), "name" FROM "products""#
		);
	}

	#[test]
	fn test_window_last_value() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("category").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("price".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::last_value(Expr::col("name").into_simple_expr()).over(window))
			.column("name")
			.from("products");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT LAST_VALUE("name") OVER ( PARTITION BY "category" ORDER BY "price" DESC ), "name" FROM "products""#
		);
	}

	#[test]
	fn test_window_nth_value() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("department").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("salary".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::nth_value(Expr::col("name").into_simple_expr(), 2).over(window))
			.column("name")
			.from("employees");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("NTH_VALUE"));
		assert!(sql.contains(r#"PARTITION BY "department""#));
		assert!(sql.contains(r#"ORDER BY "salary" DESC"#));
		assert_eq!(values.len(), 1); // The "2" parameter
	}

	#[test]
	fn test_window_row_number_multiple_partition_columns() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![
				Expr::col("country").into_simple_expr(),
				Expr::col("city").into_simple_expr(),
			],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("population".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::row_number().over(window))
			.column("name")
			.from("cities");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT ROW_NUMBER() OVER ( PARTITION BY "country", "city" ORDER BY "population" DESC ), "name" FROM "cities""#
		);
	}

	#[test]
	fn test_window_ntile_with_partition() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("region").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("revenue".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::ntile(5).over(window))
			.column("store_name")
			.from("stores");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT NTILE($1) OVER ( PARTITION BY "region" ORDER BY "revenue" DESC ), "store_name" FROM "stores""#
		);
	}

	#[test]
	fn test_window_lead_with_offset_no_default() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("quarter".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::lead(Expr::col("sales").into_simple_expr(), Some(2), None).over(window))
			.column("quarter")
			.from("quarterly_sales");

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT LEAD("sales", $1) OVER ( ORDER BY "quarter" ASC ), "quarter" FROM "quarterly_sales""#
		);
	}

	#[test]
	fn test_window_lag_with_different_offset() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col("sensor_id").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("timestamp".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::lag(Expr::col("reading").into_simple_expr(), Some(5), None).over(window))
			.column("timestamp")
			.from("sensor_data");

		let (sql, values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT LAG("reading", $1) OVER ( PARTITION BY "sensor_id" ORDER BY "timestamp" ASC ), "timestamp" FROM "sensor_data""#
		);
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_window_multiple_functions_in_query() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window1 = WindowStatement {
			partition_by: vec![Expr::col("department").into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("salary".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		let window2 = WindowStatement {
			partition_by: vec![],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::Column("hire_date".into_iden()),
				order: Order::Asc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.expr(Expr::row_number().over(window1))
			.expr(Expr::rank().over(window2))
			.column("name")
			.from("employees");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(
			sql.contains(
				r#"ROW_NUMBER() OVER ( PARTITION BY "department" ORDER BY "salary" DESC )"#
			)
		);
		assert!(sql.contains(r#"RANK() OVER ( ORDER BY "hire_date" ASC )"#));
		assert!(sql.contains(r#""name""#));
		assert!(sql.contains(r#"FROM "employees""#));
	}

	// JOIN enhancement tests

	#[test]
	fn test_join_three_tables() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column(("users", "name"))
			.column(("orders", "order_date"))
			.column(("products", "product_name"))
			.from("users")
			.inner_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			)
			.inner_join(
				"products",
				Expr::col(("orders", "product_id")).eq(Expr::col(("products", "id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert_eq!(
			sql,
			r#"SELECT "users"."name", "orders"."order_date", "products"."product_name" FROM "users" INNER JOIN "orders" ON "users"."id" = "orders"."user_id" INNER JOIN "products" ON "orders"."product_id" = "products"."id""#
		);
	}

	#[test]
	fn test_self_join() {
		use crate::types::TableRef;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column(("e1", "name"))
			.column(("e2", "name"))
			.from(TableRef::table_alias("employees", "e1"))
			.inner_join(
				TableRef::table_alias("employees", "e2"),
				Expr::col(("e1", "manager_id")).eq(Expr::col(("e2", "id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains(r#"FROM "employees" AS "e1""#));
		assert!(sql.contains(r#"INNER JOIN "employees" AS "e2""#));
		assert!(sql.contains(r#"ON "e1"."manager_id" = "e2"."id""#));
	}

	#[test]
	fn test_join_complex_conditions() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.from("orders").left_join(
			"customers",
			Expr::col(("orders", "customer_id"))
				.eq(Expr::col(("customers", "id")))
				.and(Expr::col(("customers", "active")).eq(true))
				.and(
					Expr::col(("orders", "created_at"))
						.gt(Expr::col(("customers", "registered_at"))),
				),
		);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("LEFT JOIN \"customers\""));
		assert!(sql.contains("\"orders\".\"customer_id\" = \"customers\".\"id\""));
		assert!(sql.contains("AND \"customers\".\"active\" = $"));
		assert!(sql.contains("AND \"orders\".\"created_at\" > \"customers\".\"registered_at\""));
		assert_eq!(values.len(), 1); // true value
	}

	#[test]
	fn test_join_with_subquery_in_condition() {
		let builder = PostgresQueryBuilder::new();

		let mut subquery = Query::select();
		subquery.expr(Expr::col("max_id")).from("user_stats");

		let mut stmt = Query::select();
		stmt.from("users").inner_join(
			"profiles",
			Expr::col(("users", "id"))
				.eq(Expr::col(("profiles", "user_id")))
				.and(Expr::col(("users", "id")).in_subquery(subquery)),
		);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("INNER JOIN \"profiles\""));
		assert!(sql.contains("\"users\".\"id\" = \"profiles\".\"user_id\""));
		assert!(sql.contains("IN"));
		assert!(sql.contains("SELECT \"max_id\" FROM \"user_stats\""));
	}

	#[test]
	fn test_multiple_left_joins() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column(("users", "name"))
			.column(("profiles", "bio"))
			.column(("addresses", "city"))
			.column(("phone_numbers", "number"))
			.from("users")
			.left_join(
				"profiles",
				Expr::col(("users", "id")).eq(Expr::col(("profiles", "user_id"))),
			)
			.left_join(
				"addresses",
				Expr::col(("users", "id")).eq(Expr::col(("addresses", "user_id"))),
			)
			.left_join(
				"phone_numbers",
				Expr::col(("users", "id")).eq(Expr::col(("phone_numbers", "user_id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("LEFT JOIN \"profiles\""));
		assert!(sql.contains("LEFT JOIN \"addresses\""));
		assert!(sql.contains("LEFT JOIN \"phone_numbers\""));
	}

	#[test]
	fn test_mixed_join_types() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column(("users", "name"))
			.from("users")
			.inner_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			)
			.left_join(
				"reviews",
				Expr::col(("orders", "id")).eq(Expr::col(("reviews", "order_id"))),
			)
			.right_join(
				"refunds",
				Expr::col(("orders", "id")).eq(Expr::col(("refunds", "order_id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("INNER JOIN \"orders\""));
		assert!(sql.contains("LEFT JOIN \"reviews\""));
		assert!(sql.contains("RIGHT JOIN \"refunds\""));
	}

	#[test]
	fn test_join_with_group_by() {
		use crate::expr::SimpleExpr;
		use crate::types::{BinOper, ColumnRef, IntoIden};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		let count_expr = SimpleExpr::FunctionCall(
			"COUNT".into_iden(),
			vec![SimpleExpr::Column(ColumnRef::Asterisk)],
		);

		stmt.column(("users", "name"))
			.expr(count_expr.clone())
			.from("users")
			.inner_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			)
			.group_by(("users", "name"))
			.and_having(SimpleExpr::Binary(
				Box::new(count_expr),
				BinOper::GreaterThan,
				Box::new(SimpleExpr::Value(5.into())),
			));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("INNER JOIN \"orders\""));
		assert!(sql.contains("GROUP BY \"users\".\"name\""));
		assert!(sql.contains("HAVING"));
		assert!(sql.contains("COUNT(*) > $"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_join_with_window_function() {
		use crate::types::{IntoIden, Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();

		let window = WindowStatement {
			partition_by: vec![Expr::col(("departments", "name")).into_simple_expr()],
			order_by: vec![OrderExpr {
				expr: OrderExprKind::TableColumn("employees".into_iden(), "salary".into_iden()),
				order: Order::Desc,
				nulls: None,
			}],
			frame: None,
		};

		stmt.column(("employees", "name"))
			.expr(Expr::row_number().over(window))
			.from("employees")
			.inner_join(
				"departments",
				Expr::col(("employees", "department_id")).eq(Expr::col(("departments", "id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("INNER JOIN \"departments\""));
		assert!(sql.contains("ROW_NUMBER() OVER"));
		assert!(sql.contains(r#"PARTITION BY "departments"."name""#));
	}

	#[test]
	fn test_four_table_join() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column(("users", "name"))
			.column(("orders", "order_date"))
			.column(("products", "product_name"))
			.column(("categories", "category_name"))
			.from("users")
			.inner_join(
				"orders",
				Expr::col(("users", "id")).eq(Expr::col(("orders", "user_id"))),
			)
			.inner_join(
				"products",
				Expr::col(("orders", "product_id")).eq(Expr::col(("products", "id"))),
			)
			.inner_join(
				"categories",
				Expr::col(("products", "category_id")).eq(Expr::col(("categories", "id"))),
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("FROM \"users\""));
		assert!(sql.contains("INNER JOIN \"orders\""));
		assert!(sql.contains("INNER JOIN \"products\""));
		assert!(sql.contains("INNER JOIN \"categories\""));
	}

	#[test]
	fn test_join_with_cte() {
		use crate::types::TableRef;

		let builder = PostgresQueryBuilder::new();

		let mut cte = Query::select();
		cte.column("user_id")
			.expr(Expr::col("total"))
			.from("order_totals")
			.and_where(Expr::col("total").gt(1000));

		let mut stmt = Query::select();
		stmt.with_cte("high_value_customers", cte)
			.column(("users", "name"))
			.column(("hvc", "total"))
			.from("users")
			.inner_join(
				TableRef::table_alias("high_value_customers", "hvc"),
				Expr::col(("users", "id")).eq(Expr::col(("hvc", "user_id"))),
			);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH \"high_value_customers\" AS"));
		assert!(sql.contains("INNER JOIN \"high_value_customers\" AS \"hvc\""));
		assert_eq!(values.len(), 1); // 1000
	}

	#[test]
	fn test_cte_with_where_and_params() {
		let builder = PostgresQueryBuilder::new();

		let mut cte_query = Query::select();
		cte_query
			.column("id")
			.column("total")
			.from("orders")
			.and_where(Expr::col("status").eq("completed"))
			.and_where(Expr::col("amount").gt(1000));

		let mut stmt = Query::select();
		stmt.with_cte("large_orders", cte_query)
			.column("id")
			.column("total")
			.from("large_orders");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH"));
		assert!(sql.contains(r#""large_orders" AS"#));
		assert!(sql.contains(r#""status" = $"#));
		assert!(sql.contains(r#""amount" > $"#));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_cte_used_in_join() {
		use crate::types::TableRef;

		let builder = PostgresQueryBuilder::new();

		let mut cte_query = Query::select();
		cte_query
			.column("user_id")
			.column("order_count")
			.from("orders")
			.group_by("user_id");

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
		assert!(sql.contains("WITH"));
		assert!(sql.contains(r#""user_orders" AS"#));
		assert!(sql.contains(r#"INNER JOIN "user_orders" AS "uo""#));
		assert!(sql.contains(r#""users"."id" = "uo"."user_id""#));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_cte_with_aggregation() {
		use crate::expr::SimpleExpr;
		use crate::types::{ColumnRef, IntoIden};

		let builder = PostgresQueryBuilder::new();

		let mut cte_query = Query::select();
		cte_query
			.column("category")
			.expr(SimpleExpr::FunctionCall(
				"COUNT".into_iden(),
				vec![SimpleExpr::Column(ColumnRef::Asterisk)],
			))
			.expr(SimpleExpr::FunctionCall(
				"SUM".into_iden(),
				vec![SimpleExpr::Column(ColumnRef::column("price"))],
			))
			.from("products")
			.group_by("category");

		let mut stmt = Query::select();
		stmt.with_cte("category_stats", cte_query)
			.column("category")
			.from("category_stats");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH"));
		assert!(sql.contains(r#""category_stats" AS"#));
		assert!(sql.contains("COUNT(*)"));
		assert!(sql.contains(r#"SUM("price")"#));
		assert!(sql.contains(r#"GROUP BY "category""#));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_cte_with_subquery() {
		let builder = PostgresQueryBuilder::new();

		let mut sub = Query::select();
		sub.column("user_id").from("vip_users");

		let mut cte_query = Query::select();
		cte_query
			.column("id")
			.column("total")
			.from("orders")
			.and_where(Expr::col("user_id").in_subquery(sub))
			.and_where(Expr::col("status").eq("shipped"));

		let mut stmt = Query::select();
		stmt.with_cte("vip_orders", cte_query)
			.column("id")
			.column("total")
			.from("vip_orders");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH"));
		assert!(sql.contains(r#""vip_orders" AS"#));
		assert!(sql.contains("IN"));
		assert!(sql.contains(r#"SELECT "user_id" FROM "vip_users""#));
		assert!(sql.contains(r#""status" = $"#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_multiple_recursive_and_regular_ctes() {
		let builder = PostgresQueryBuilder::new();

		// Regular CTE
		let mut regular_cte = Query::select();
		regular_cte
			.column("id")
			.column("name")
			.from("departments")
			.and_where(Expr::col("active").eq(true));

		// Recursive CTE
		let mut recursive_cte = Query::select();
		recursive_cte
			.column("id")
			.column("name")
			.column("parent_id")
			.from("categories");

		// Main query
		let mut stmt = Query::select();
		stmt.with_cte("active_depts", regular_cte)
			.with_recursive_cte("category_tree", recursive_cte)
			.column("name")
			.from("category_tree");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("WITH RECURSIVE"));
		assert!(sql.contains(r#""active_depts" AS"#));
		assert!(sql.contains(r#""category_tree" AS"#));
		assert!(sql.contains(r#""active" = $"#));
		assert!(sql.contains(r#"FROM "category_tree""#));
		assert_eq!(values.len(), 1);
	}

	// CASE expression tests

	#[test]
	fn test_case_simple_when_else() {
		let builder = PostgresQueryBuilder::new();

		let case_expr = Expr::case()
			.when(Expr::col("status").eq("active"), "Active")
			.else_result("Inactive");

		let mut stmt = Query::select();
		stmt.expr_as(case_expr, "status_label").from("users");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("CASE"));
		assert!(sql.contains("WHEN"));
		assert!(sql.contains(r#""status" = $"#));
		assert!(sql.contains("THEN"));
		assert!(sql.contains("ELSE"));
		assert!(sql.contains("END"));
		assert!(sql.contains(r#"AS "status_label""#));
		assert_eq!(values.len(), 3);
	}

	#[test]
	fn test_case_multiple_when_clauses() {
		let builder = PostgresQueryBuilder::new();

		let case_expr = Expr::case()
			.when(Expr::col("score").gte(90), "A")
			.when(Expr::col("score").gte(80), "B")
			.when(Expr::col("score").gte(70), "C")
			.else_result("F");

		let mut stmt = Query::select();
		stmt.expr_as(case_expr, "grade").from("students");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("CASE"));
		// Verify multiple WHEN clauses
		let when_count = sql.matches("WHEN").count();
		assert_eq!(when_count, 3);
		let then_count = sql.matches("THEN").count();
		assert_eq!(then_count, 3);
		assert!(sql.contains("ELSE"));
		assert!(sql.contains("END"));
		// 3 score comparisons + 3 THEN values + 1 ELSE value = 7
		assert_eq!(values.len(), 7);
	}

	#[test]
	fn test_case_without_else() {
		let builder = PostgresQueryBuilder::new();

		let case_expr = Expr::case()
			.when(Expr::col("type").eq("admin"), "Administrator")
			.when(Expr::col("type").eq("user"), "Regular User")
			.build();

		let mut stmt = Query::select();
		stmt.expr_as(case_expr, "type_label").from("accounts");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("CASE"));
		assert!(sql.contains("WHEN"));
		assert!(sql.contains("THEN"));
		assert!(!sql.contains("ELSE"));
		assert!(sql.contains("END"));
		assert_eq!(values.len(), 4);
	}

	#[test]
	fn test_case_in_where_clause() {
		let builder = PostgresQueryBuilder::new();

		let case_expr = Expr::case()
			.when(Expr::col("role").eq("admin"), 1)
			.else_result(0);

		let mut stmt = Query::select();
		stmt.column("name").from("users").and_where(case_expr.eq(1));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("WHERE"));
		assert!(sql.contains("CASE"));
		assert!(sql.contains("WHEN"));
		assert!(sql.contains("END"));
		assert!(values.len() >= 3);
	}

	#[test]
	fn test_case_in_order_by() {
		let builder = PostgresQueryBuilder::new();

		let case_expr = Expr::case()
			.when(Expr::col("priority").eq("high"), 1)
			.when(Expr::col("priority").eq("medium"), 2)
			.else_result(3);

		let mut stmt = Query::select();
		stmt.column("name")
			.column("priority")
			.from("tasks")
			.order_by_expr(case_expr, crate::types::Order::Asc);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("ORDER BY"));
		assert!(sql.contains("CASE"));
		assert!(sql.contains("WHEN"));
		assert!(sql.contains("END"));
		assert!(sql.contains("ASC"));
		assert_eq!(values.len(), 5);
	}

	// ORDER BY / LIMIT edge case tests

	#[test]
	fn test_order_by_multiple_columns_mixed() {
		let builder = PostgresQueryBuilder::new();

		let mut stmt = Query::select();
		stmt.column("name")
			.column("age")
			.column("score")
			.from("students")
			.order_by("name", crate::types::Order::Asc)
			.order_by("age", crate::types::Order::Desc)
			.order_by("score", crate::types::Order::Asc);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("ORDER BY"));
		assert!(sql.contains(r#""name" ASC"#));
		assert!(sql.contains(r#""age" DESC"#));
		assert!(sql.contains(r#""score" ASC"#));
	}

	#[test]
	fn test_order_by_nulls_first() {
		use crate::types::{IntoColumnRef, NullOrdering, OrderExpr, OrderExprKind};

		let builder = PostgresQueryBuilder::new();

		let mut stmt = Query::select();
		stmt.column("name").column("created_at").from("events");
		stmt.orders.push(OrderExpr {
			expr: OrderExprKind::Expr(Box::new(SimpleExpr::Column("created_at".into_column_ref()))),
			order: crate::types::Order::Desc,
			nulls: Some(NullOrdering::First),
		});

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("ORDER BY"));
		assert!(sql.contains("DESC"));
		assert!(sql.contains("NULLS FIRST"));
	}

	#[test]
	fn test_order_by_nulls_last() {
		use crate::types::{IntoColumnRef, NullOrdering, OrderExpr, OrderExprKind};

		let builder = PostgresQueryBuilder::new();

		let mut stmt = Query::select();
		stmt.column("name").column("updated_at").from("posts");
		stmt.orders.push(OrderExpr {
			expr: OrderExprKind::Expr(Box::new(SimpleExpr::Column("updated_at".into_column_ref()))),
			order: crate::types::Order::Asc,
			nulls: Some(NullOrdering::Last),
		});

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("ORDER BY"));
		assert!(sql.contains("ASC"));
		assert!(sql.contains("NULLS LAST"));
	}

	#[test]
	fn test_limit_without_offset() {
		let builder = PostgresQueryBuilder::new();

		let mut stmt = Query::select();
		stmt.column("id").from("items").limit(5);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("LIMIT"));
		assert!(!sql.contains("OFFSET"));
		assert_eq!(values.len(), 1);
	}

	// Arithmetic / string operation tests

	#[test]
	fn test_arithmetic_add_sub() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("products");
		stmt.and_where(Expr::col("price").add(10i32).gt(100i32));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""price" + $1"#));
		assert!(sql.contains("> $2"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_arithmetic_mul_div_mod() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("items");
		stmt.and_where(
			Expr::col("quantity")
				.mul(Expr::col("unit_price"))
				.gt(1000i32),
		);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""quantity" * "unit_price""#));
		assert!(sql.contains("> $1"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_like_ilike_pattern() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("users");
		stmt.and_where(Expr::col("email").like("%@example.com"));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""email" LIKE $1"#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_pg_concat_operator() {
		use crate::types::{BinOper, IntoColumnRef, PgBinOper};
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(SimpleExpr::Binary(
			Box::new(SimpleExpr::Column("first_name".into_column_ref())),
			BinOper::PgOperator(PgBinOper::Concatenate),
			Box::new(SimpleExpr::Column("last_name".into_column_ref())),
		));
		stmt.from("users");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""first_name" || "last_name""#));
	}

	// DDL Tests

	#[test]
	fn test_drop_table_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users");

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_table_if_exists() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users").if_exists();

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE IF EXISTS \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_table_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users").cascade();

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE \"users\" CASCADE");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_table_restrict() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users").restrict();

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE \"users\" RESTRICT");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_table_multiple() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users").table("posts");

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE \"users\", \"posts\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_index_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_index();
		stmt.name("idx_email");

		let (sql, values) = builder.build_drop_index(&stmt);
		assert_eq!(sql, "DROP INDEX \"idx_email\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_index_if_exists() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_index();
		stmt.name("idx_email").if_exists();

		let (sql, values) = builder.build_drop_index(&stmt);
		assert_eq!(sql, "DROP INDEX IF EXISTS \"idx_email\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_index_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_index();
		stmt.name("idx_email").cascade();

		let (sql, values) = builder.build_drop_index(&stmt);
		assert_eq!(sql, "DROP INDEX \"idx_email\" CASCADE");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_index_restrict() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_index();
		stmt.name("idx_email").restrict();

		let (sql, values) = builder.build_drop_index(&stmt);
		assert_eq!(sql, "DROP INDEX \"idx_email\" RESTRICT");
		assert_eq!(values.len(), 0);
	}

	// CREATE TABLE tests

	#[test]
	fn test_create_table_basic() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "id".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});
		stmt.columns.push(ColumnDef {
			name: "name".into_iden(),
			column_type: Some(ColumnType::String(Some(255))),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("CREATE TABLE \"users\""));
		assert!(sql.contains("\"id\" INTEGER"));
		assert!(sql.contains("\"name\" VARCHAR(255)"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_if_not_exists() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users").if_not_exists();
		stmt.columns.push(ColumnDef {
			name: "id".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("CREATE TABLE IF NOT EXISTS \"users\""));
		assert!(sql.contains("\"id\" INTEGER"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_with_primary_key() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "id".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: true,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("CREATE TABLE \"users\""));
		assert!(sql.contains("\"id\" INTEGER PRIMARY KEY"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_with_not_null() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "email".into_iden(),
			column_type: Some(ColumnType::String(Some(255))),
			not_null: true,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("\"email\" VARCHAR(255) NOT NULL"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_with_unique() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "username".into_iden(),
			column_type: Some(ColumnType::String(Some(50))),
			not_null: false,
			unique: true,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("\"username\" VARCHAR(50) UNIQUE"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_with_default() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "active".into_iden(),
			column_type: Some(ColumnType::Boolean),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: Some(Expr::value(true).into_simple_expr()),
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("\"active\" BOOLEAN DEFAULT"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_create_table_with_check() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "age".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: Some(Expr::col("age").gte(0).into_simple_expr()),
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		// CHECK constraints use inlined values (not parameters) in PostgreSQL
		assert!(sql.contains("\"age\" INTEGER CHECK"));
		assert!(sql.contains(">= 0"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_with_table_constraint() {
		use crate::types::{ColumnDef, ColumnType, TableConstraint};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "id".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});
		stmt.columns.push(ColumnDef {
			name: "email".into_iden(),
			column_type: Some(ColumnType::String(Some(255))),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});
		stmt.constraints.push(TableConstraint::PrimaryKey {
			name: Some("pk_users".into_iden()),
			columns: vec!["id".into_iden()],
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("CONSTRAINT \"pk_users\" PRIMARY KEY (\"id\")"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_with_foreign_key() {
		use crate::types::{
			ColumnDef, ColumnType, ForeignKeyAction, IntoTableRef, TableConstraint,
		};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("posts");
		stmt.columns.push(ColumnDef {
			name: "id".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: true,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});
		stmt.columns.push(ColumnDef {
			name: "user_id".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});
		stmt.constraints.push(TableConstraint::ForeignKey {
			name: Some("fk_user".into_iden()),
			columns: vec!["user_id".into_iden()],
			ref_table: Box::new("users".into_table_ref()),
			ref_columns: vec!["id".into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: Some(ForeignKeyAction::Restrict),
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("CONSTRAINT \"fk_user\" FOREIGN KEY (\"user_id\")"));
		assert!(sql.contains("REFERENCES \"users\" (\"id\")"));
		assert!(sql.contains("ON DELETE CASCADE"));
		assert!(sql.contains("ON UPDATE RESTRICT"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_basic() {
		use crate::query::IndexColumn;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_users_email");
		stmt.table("users");
		stmt.columns.push(IndexColumn {
			name: "email".into_iden(),
			order: None,
		});

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_users_email" ON "users" ("email")"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_unique() {
		use crate::query::IndexColumn;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_users_username");
		stmt.table("users");
		stmt.unique = true;
		stmt.columns.push(IndexColumn {
			name: "username".into_iden(),
			order: None,
		});

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE UNIQUE INDEX "idx_users_username" ON "users" ("username")"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_if_not_exists() {
		use crate::query::IndexColumn;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_users_email");
		stmt.table("users");
		stmt.if_not_exists = true;
		stmt.columns.push(IndexColumn {
			name: "email".into_iden(),
			order: None,
		});

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE INDEX IF NOT EXISTS "idx_users_email" ON "users" ("email")"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_with_order() {
		use crate::query::IndexColumn;
		use crate::types::Order;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_users_created");
		stmt.table("users");
		stmt.columns.push(IndexColumn {
			name: "created_at".into_iden(),
			order: Some(Order::Desc),
		});

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_users_created" ON "users" ("created_at" DESC)"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_multiple_columns() {
		use crate::query::IndexColumn;
		use crate::types::Order;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_users_name");
		stmt.table("users");
		stmt.columns.push(IndexColumn {
			name: "last_name".into_iden(),
			order: Some(Order::Asc),
		});
		stmt.columns.push(IndexColumn {
			name: "first_name".into_iden(),
			order: Some(Order::Asc),
		});

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_users_name" ON "users" ("last_name" ASC, "first_name" ASC)"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_with_using_btree() {
		use crate::query::{IndexColumn, IndexMethod};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_users_id");
		stmt.table("users");
		stmt.using = Some(IndexMethod::BTree);
		stmt.columns.push(IndexColumn {
			name: "id".into_iden(),
			order: None,
		});

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_users_id" ON "users" USING BTREE ("id")"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_with_using_gin() {
		use crate::query::{IndexColumn, IndexMethod};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_posts_tags");
		stmt.table("posts");
		stmt.using = Some(IndexMethod::Gin);
		stmt.columns.push(IndexColumn {
			name: "tags".into_iden(),
			order: None,
		});

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_posts_tags" ON "posts" USING GIN ("tags")"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_index_partial_with_where() {
		use crate::query::IndexColumn;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_index();
		stmt.name("idx_users_active_email");
		stmt.table("users");
		stmt.columns.push(IndexColumn {
			name: "email".into_iden(),
			order: None,
		});
		stmt.r#where = Some(Expr::col("active").eq(true).into_simple_expr());

		let (sql, values) = builder.build_create_index(&stmt);
		assert_eq!(
			sql,
			r#"CREATE INDEX "idx_users_active_email" ON "users" ("email") WHERE "active" = $1"#
		);
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_alter_table_add_column() {
		use crate::query::AlterTableOperation;
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations
			.push(AlterTableOperation::AddColumn(ColumnDef {
				name: "age".into_iden(),
				column_type: Some(ColumnType::Integer),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				check: None,
				comment: None,
			}));

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(sql, r#"ALTER TABLE "users" ADD COLUMN "age" INTEGER"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_table_drop_column() {
		use crate::query::AlterTableOperation;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations.push(AlterTableOperation::DropColumn {
			name: "age".into_iden(),
			if_exists: false,
		});

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(sql, r#"ALTER TABLE "users" DROP COLUMN "age""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_table_drop_column_if_exists() {
		use crate::query::AlterTableOperation;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations.push(AlterTableOperation::DropColumn {
			name: "age".into_iden(),
			if_exists: true,
		});

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(sql, r#"ALTER TABLE "users" DROP COLUMN IF EXISTS "age""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_table_rename_column() {
		use crate::query::AlterTableOperation;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations.push(AlterTableOperation::RenameColumn {
			old: "email".into_iden(),
			new: "email_address".into_iden(),
		});

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(
			sql,
			r#"ALTER TABLE "users" RENAME COLUMN "email" TO "email_address""#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_table_modify_column_type() {
		use crate::query::AlterTableOperation;
		use crate::types::{ColumnDef, ColumnType};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations
			.push(AlterTableOperation::ModifyColumn(ColumnDef {
				name: "age".into_iden(),
				column_type: Some(ColumnType::BigInteger),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
				check: None,
				comment: None,
			}));

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(sql, r#"ALTER TABLE "users" ALTER COLUMN "age" TYPE BIGINT"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_table_add_constraint() {
		use crate::query::AlterTableOperation;
		use crate::types::TableConstraint;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations.push(AlterTableOperation::AddConstraint(
			TableConstraint::Unique {
				name: Some("unique_email".into_iden()),
				columns: vec!["email".into_iden()],
			},
		));

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(
			sql,
			r#"ALTER TABLE "users" ADD CONSTRAINT "unique_email" UNIQUE ("email")"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_table_drop_constraint() {
		use crate::query::AlterTableOperation;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations.push(AlterTableOperation::DropConstraint {
			name: "unique_email".into_iden(),
			if_exists: false,
		});

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(sql, r#"ALTER TABLE "users" DROP CONSTRAINT "unique_email""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_table_rename_table() {
		use crate::query::AlterTableOperation;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations
			.push(AlterTableOperation::RenameTable("accounts".into_iden()));

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(sql, r#"ALTER TABLE "users" RENAME TO "accounts""#);
		assert_eq!(values.len(), 0);
	}

	// TRUNCATE TABLE tests

	#[test]
	fn test_truncate_table_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users");

		let (sql, values) = builder.build_truncate_table(&stmt);
		assert_eq!(sql, r#"TRUNCATE TABLE "users""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_truncate_table_multiple() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").table("posts").table("comments");

		let (sql, values) = builder.build_truncate_table(&stmt);
		assert_eq!(sql, r#"TRUNCATE TABLE "users", "posts", "comments""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_truncate_table_restart_identity() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").restart_identity();

		let (sql, values) = builder.build_truncate_table(&stmt);
		assert_eq!(sql, r#"TRUNCATE TABLE "users" RESTART IDENTITY"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_truncate_table_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").cascade();

		let (sql, values) = builder.build_truncate_table(&stmt);
		assert_eq!(sql, r#"TRUNCATE TABLE "users" CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_truncate_table_restrict() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").restrict();

		let (sql, values) = builder.build_truncate_table(&stmt);
		assert_eq!(sql, r#"TRUNCATE TABLE "users" RESTRICT"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_truncate_table_restart_identity_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").restart_identity().cascade();

		let (sql, values) = builder.build_truncate_table(&stmt);
		assert_eq!(sql, r#"TRUNCATE TABLE "users" RESTART IDENTITY CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_trigger_basic() {
		use crate::types::{TriggerEvent, TriggerScope, TriggerTiming};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("audit_log")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Insert)
			.on_table("users")
			.for_each(TriggerScope::Row)
			.execute_function("log_user_insert");

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TRIGGER "audit_log" AFTER INSERT ON "users" FOR EACH ROW EXECUTE FUNCTION "log_user_insert"()"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_trigger_before_update() {
		use crate::types::{TriggerEvent, TriggerScope, TriggerTiming};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("update_timestamp")
			.timing(TriggerTiming::Before)
			.event(TriggerEvent::Update { columns: None })
			.on_table("users")
			.for_each(TriggerScope::Row)
			.execute_function("update_modified_at");

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TRIGGER "update_timestamp" BEFORE UPDATE ON "users" FOR EACH ROW EXECUTE FUNCTION "update_modified_at"()"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_trigger_delete_for_statement() {
		use crate::types::{TriggerEvent, TriggerScope, TriggerTiming};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("audit_delete")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Delete)
			.on_table("users")
			.for_each(TriggerScope::Statement)
			.execute_function("log_bulk_delete");

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TRIGGER "audit_delete" AFTER DELETE ON "users" FOR EACH STATEMENT EXECUTE FUNCTION "log_bulk_delete"()"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_trigger_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_trigger();
		stmt.name("audit_log").on_table("users");

		let (sql, values) = builder.build_drop_trigger(&stmt);
		assert_eq!(sql, r#"DROP TRIGGER "audit_log" ON "users""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_trigger_if_exists() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_trigger();
		stmt.name("audit_log").on_table("users").if_exists();

		let (sql, values) = builder.build_drop_trigger(&stmt);
		assert_eq!(sql, r#"DROP TRIGGER IF EXISTS "audit_log" ON "users""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_trigger_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_trigger();
		stmt.name("audit_log").on_table("users").cascade();

		let (sql, values) = builder.build_drop_trigger(&stmt);
		assert_eq!(sql, r#"DROP TRIGGER "audit_log" ON "users" CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	// CREATE FUNCTION tests
	#[test]
	fn test_create_function_basic() {
		use crate::types::function::FunctionLanguage;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("my_func")
			.returns("integer")
			.language(FunctionLanguage::Sql)
			.body("SELECT 1");

		let (sql, values) = builder.build_create_function(&stmt);
		assert_eq!(
			sql,
			r#"CREATE FUNCTION "my_func"() RETURNS integer LANGUAGE SQL AS $$SELECT 1$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_function_or_replace() {
		use crate::types::function::FunctionLanguage;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("my_func")
			.or_replace()
			.returns("integer")
			.language(FunctionLanguage::Sql)
			.body("SELECT 1");

		let (sql, values) = builder.build_create_function(&stmt);
		assert_eq!(
			sql,
			r#"CREATE OR REPLACE FUNCTION "my_func"() RETURNS integer LANGUAGE SQL AS $$SELECT 1$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_function_with_parameters() {
		use crate::types::function::FunctionLanguage;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("add_numbers")
			.add_parameter("a", "integer")
			.add_parameter("b", "integer")
			.returns("integer")
			.language(FunctionLanguage::Sql)
			.body("SELECT $1 + $2");

		let (sql, values) = builder.build_create_function(&stmt);
		assert_eq!(
			sql,
			r#"CREATE FUNCTION "add_numbers"("a" integer, "b" integer) RETURNS integer LANGUAGE SQL AS $$SELECT $1 + $2$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_function_with_behavior() {
		use crate::types::function::{FunctionBehavior, FunctionLanguage};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("my_func")
			.returns("integer")
			.language(FunctionLanguage::Sql)
			.behavior(FunctionBehavior::Immutable)
			.body("SELECT 1");

		let (sql, values) = builder.build_create_function(&stmt);
		assert_eq!(
			sql,
			r#"CREATE FUNCTION "my_func"() RETURNS integer LANGUAGE SQL IMMUTABLE AS $$SELECT 1$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_function_with_security() {
		use crate::types::function::{FunctionLanguage, FunctionSecurity};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("my_func")
			.returns("integer")
			.language(FunctionLanguage::Sql)
			.security(FunctionSecurity::Definer)
			.body("SELECT 1");

		let (sql, values) = builder.build_create_function(&stmt);
		assert_eq!(
			sql,
			r#"CREATE FUNCTION "my_func"() RETURNS integer LANGUAGE SQL SECURITY DEFINER AS $$SELECT 1$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_function_plpgsql() {
		use crate::types::function::FunctionLanguage;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("increment")
			.add_parameter("val", "integer")
			.returns("integer")
			.language(FunctionLanguage::PlPgSql)
			.body("BEGIN RETURN val + 1; END;");

		let (sql, values) = builder.build_create_function(&stmt);
		assert_eq!(
			sql,
			r#"CREATE FUNCTION "increment"("val" integer) RETURNS integer LANGUAGE PLPGSQL AS $$BEGIN RETURN val + 1; END;$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_function_all_options() {
		use crate::types::function::{FunctionBehavior, FunctionLanguage, FunctionSecurity};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("complex_func")
			.or_replace()
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.returns("integer")
			.language(FunctionLanguage::PlPgSql)
			.behavior(FunctionBehavior::Stable)
			.security(FunctionSecurity::Definer)
			.body("BEGIN RETURN a + LENGTH(b); END;");

		let (sql, values) = builder.build_create_function(&stmt);
		assert_eq!(
			sql,
			r#"CREATE OR REPLACE FUNCTION "complex_func"("a" integer, "b" text) RETURNS integer LANGUAGE PLPGSQL STABLE SECURITY DEFINER AS $$BEGIN RETURN a + LENGTH(b); END;$$"#
		);
		assert_eq!(values.len(), 0);
	}

	// ALTER FUNCTION tests
	#[test]
	fn test_alter_function_rename_to() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_function();
		stmt.name("my_func").rename_to("new_func");

		let (sql, values) = builder.build_alter_function(&stmt);
		assert_eq!(sql, r#"ALTER FUNCTION "my_func" RENAME TO "new_func""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_function_owner_to() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_function();
		stmt.name("my_func").owner_to("new_owner");

		let (sql, values) = builder.build_alter_function(&stmt);
		assert_eq!(sql, r#"ALTER FUNCTION "my_func" OWNER TO "new_owner""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_function_set_schema() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_function();
		stmt.name("my_func").set_schema("new_schema");

		let (sql, values) = builder.build_alter_function(&stmt);
		assert_eq!(sql, r#"ALTER FUNCTION "my_func" SET SCHEMA "new_schema""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_function_set_behavior_immutable() {
		use crate::types::function::FunctionBehavior;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_function();
		stmt.name("my_func")
			.set_behavior(FunctionBehavior::Immutable);

		let (sql, values) = builder.build_alter_function(&stmt);
		assert_eq!(sql, r#"ALTER FUNCTION "my_func" IMMUTABLE"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_function_set_security_definer() {
		use crate::types::function::FunctionSecurity;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_function();
		stmt.name("my_func").set_security(FunctionSecurity::Definer);

		let (sql, values) = builder.build_alter_function(&stmt);
		assert_eq!(sql, r#"ALTER FUNCTION "my_func" SECURITY DEFINER"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_function_with_parameters() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_function();
		stmt.name("my_func")
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.rename_to("new_func");

		let (sql, values) = builder.build_alter_function(&stmt);
		assert_eq!(
			sql,
			r#"ALTER FUNCTION "my_func"("a" integer, "b" text) RENAME TO "new_func""#
		);
		assert_eq!(values.len(), 0);
	}

	// DROP FUNCTION tests
	#[test]
	fn test_drop_function_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_function();
		stmt.name("my_func");

		let (sql, values) = builder.build_drop_function(&stmt);
		assert_eq!(sql, r#"DROP FUNCTION "my_func""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_function_if_exists() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_function();
		stmt.name("my_func").if_exists();

		let (sql, values) = builder.build_drop_function(&stmt);
		assert_eq!(sql, r#"DROP FUNCTION IF EXISTS "my_func""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_function_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_function();
		stmt.name("my_func").cascade();

		let (sql, values) = builder.build_drop_function(&stmt);
		assert_eq!(sql, r#"DROP FUNCTION "my_func" CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_function_with_parameters() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_function();
		stmt.name("my_func")
			.add_parameter("", "integer")
			.add_parameter("", "text");

		let (sql, values) = builder.build_drop_function(&stmt);
		assert_eq!(sql, r#"DROP FUNCTION "my_func"(integer, text)"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_function_all_options() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_function();
		stmt.name("my_func")
			.if_exists()
			.add_parameter("", "integer")
			.cascade();

		let (sql, values) = builder.build_drop_function(&stmt);
		assert_eq!(sql, r#"DROP FUNCTION IF EXISTS "my_func"(integer) CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	// Procedure tests
	#[test]
	fn test_create_procedure_basic() {
		use crate::types::function::FunctionLanguage;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_procedure();
		stmt.name("my_proc")
			.language(FunctionLanguage::Sql)
			.body("SELECT 1");

		let (sql, values) = builder.build_create_procedure(&stmt);
		assert_eq!(
			sql,
			r#"CREATE PROCEDURE "my_proc"() LANGUAGE SQL AS $$SELECT 1$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_procedure_or_replace() {
		use crate::types::function::FunctionLanguage;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_procedure();
		stmt.name("my_proc")
			.or_replace()
			.language(FunctionLanguage::PlPgSql)
			.body("BEGIN SELECT 1; END;");

		let (sql, values) = builder.build_create_procedure(&stmt);
		assert_eq!(
			sql,
			r#"CREATE OR REPLACE PROCEDURE "my_proc"() LANGUAGE PLPGSQL AS $$BEGIN SELECT 1; END;$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_procedure_with_parameters() {
		use crate::types::function::FunctionLanguage;

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_procedure();
		stmt.name("my_proc")
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.language(FunctionLanguage::PlPgSql)
			.body("BEGIN INSERT INTO log VALUES (a, b); END;");

		let (sql, values) = builder.build_create_procedure(&stmt);
		assert_eq!(
			sql,
			r#"CREATE PROCEDURE "my_proc"("a" integer, "b" text) LANGUAGE PLPGSQL AS $$BEGIN INSERT INTO log VALUES (a, b); END;$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_procedure_with_behavior() {
		use crate::types::function::{FunctionBehavior, FunctionLanguage};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_procedure();
		stmt.name("my_proc")
			.language(FunctionLanguage::Sql)
			.behavior(FunctionBehavior::Immutable)
			.body("SELECT 1");

		let (sql, values) = builder.build_create_procedure(&stmt);
		assert_eq!(
			sql,
			r#"CREATE PROCEDURE "my_proc"() LANGUAGE SQL IMMUTABLE AS $$SELECT 1$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_procedure_with_security() {
		use crate::types::function::{FunctionLanguage, FunctionSecurity};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_procedure();
		stmt.name("my_proc")
			.language(FunctionLanguage::Sql)
			.security(FunctionSecurity::Definer)
			.body("SELECT 1");

		let (sql, values) = builder.build_create_procedure(&stmt);
		assert_eq!(
			sql,
			r#"CREATE PROCEDURE "my_proc"() LANGUAGE SQL SECURITY DEFINER AS $$SELECT 1$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_procedure_all_options() {
		use crate::types::function::{FunctionBehavior, FunctionLanguage, FunctionSecurity};

		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_procedure();
		stmt.name("my_proc")
			.or_replace()
			.add_parameter("a", "integer")
			.add_parameter("b", "text")
			.language(FunctionLanguage::PlPgSql)
			.behavior(FunctionBehavior::Immutable)
			.security(FunctionSecurity::Definer)
			.body("BEGIN INSERT INTO log VALUES (a, b); END;");

		let (sql, values) = builder.build_create_procedure(&stmt);
		assert_eq!(
			sql,
			r#"CREATE OR REPLACE PROCEDURE "my_proc"("a" integer, "b" text) LANGUAGE PLPGSQL IMMUTABLE SECURITY DEFINER AS $$BEGIN INSERT INTO log VALUES (a, b); END;$$"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_procedure_rename_to() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_procedure();
		stmt.name("my_proc").rename_to("new_proc");

		let (sql, values) = builder.build_alter_procedure(&stmt);
		assert_eq!(sql, r#"ALTER PROCEDURE "my_proc" RENAME TO "new_proc""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_procedure_owner_to() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_procedure();
		stmt.name("my_proc").owner_to("new_owner");

		let (sql, values) = builder.build_alter_procedure(&stmt);
		assert_eq!(sql, r#"ALTER PROCEDURE "my_proc" OWNER TO "new_owner""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_procedure_set_schema() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_procedure();
		stmt.name("my_proc").set_schema("new_schema");

		let (sql, values) = builder.build_alter_procedure(&stmt);
		assert_eq!(sql, r#"ALTER PROCEDURE "my_proc" SET SCHEMA "new_schema""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_procedure_with_signature() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_procedure();
		stmt.name("my_proc")
			.add_parameter("a", "integer")
			.rename_to("new_proc");

		let (sql, values) = builder.build_alter_procedure(&stmt);
		assert_eq!(
			sql,
			r#"ALTER PROCEDURE "my_proc"("a" integer) RENAME TO "new_proc""#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_procedure_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_procedure();
		stmt.name("my_proc");

		let (sql, values) = builder.build_drop_procedure(&stmt);
		assert_eq!(sql, r#"DROP PROCEDURE "my_proc""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_procedure_if_exists() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_procedure();
		stmt.name("my_proc").if_exists();

		let (sql, values) = builder.build_drop_procedure(&stmt);
		assert_eq!(sql, r#"DROP PROCEDURE IF EXISTS "my_proc""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_procedure_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_procedure();
		stmt.name("my_proc").cascade();

		let (sql, values) = builder.build_drop_procedure(&stmt);
		assert_eq!(sql, r#"DROP PROCEDURE "my_proc" CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_procedure_with_signature() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_procedure();
		stmt.name("my_proc").add_parameter("", "integer");

		let (sql, values) = builder.build_drop_procedure(&stmt);
		assert_eq!(sql, r#"DROP PROCEDURE "my_proc"(integer)"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_procedure_all_options() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_procedure();
		stmt.name("my_proc")
			.if_exists()
			.add_parameter("", "integer")
			.cascade();

		let (sql, values) = builder.build_drop_procedure(&stmt);
		assert_eq!(
			sql,
			r#"DROP PROCEDURE IF EXISTS "my_proc"(integer) CASCADE"#
		);
		assert_eq!(values.len(), 0);
	}

	// CREATE TYPE tests
	#[test]
	fn test_create_type_enum() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("mood")
			.as_enum(vec!["happy".to_string(), "sad".to_string()]);

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(sql, r#"CREATE TYPE "mood" AS ENUM ('happy', 'sad')"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_enum_with_single_quote() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("test").as_enum(vec!["it's".to_string()]);

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(sql, r#"CREATE TYPE "test" AS ENUM ('it''s')"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_composite() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("address").as_composite(vec![
			("street".to_string(), "text".to_string()),
			("city".to_string(), "text".to_string()),
		]);

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TYPE "address" AS ("street" text, "city" text)"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_domain_minimal() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("positive_int").as_domain("integer".to_string());

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(sql, r#"CREATE TYPE "positive_int" AS integer"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_domain_with_constraint() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("positive_int")
			.as_domain("integer".to_string())
			.constraint(
				"check_positive".to_string(),
				"CHECK (VALUE > 0)".to_string(),
			);

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TYPE "positive_int" AS integer CHECK (VALUE > 0)"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_domain_with_default() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("my_domain")
			.as_domain("integer".to_string())
			.default_value("0".to_string());

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(sql, r#"CREATE TYPE "my_domain" AS integer DEFAULT 0"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_domain_not_null() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("my_domain")
			.as_domain("integer".to_string())
			.not_null();

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(sql, r#"CREATE TYPE "my_domain" AS integer NOT NULL"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_domain_full() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("positive_int")
			.as_domain("integer".to_string())
			.default_value("1".to_string())
			.constraint(
				"check_positive".to_string(),
				"CHECK (VALUE > 0)".to_string(),
			)
			.not_null();

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TYPE "positive_int" AS integer DEFAULT 1 CHECK (VALUE > 0) NOT NULL"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_range_minimal() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("int_range").as_range("integer".to_string());

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TYPE "int_range" AS RANGE (SUBTYPE = integer)"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_range_with_subtype_diff() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("int_range")
			.as_range("integer".to_string())
			.subtype_diff("int4range_subdiff".to_string());

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TYPE "int_range" AS RANGE (SUBTYPE = integer, SUBTYPE_DIFF = int4range_subdiff)"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_type_range_full() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("int_range")
			.as_range("integer".to_string())
			.subtype_diff("int4range_subdiff".to_string())
			.canonical("int4range_canonical".to_string());

		let (sql, values) = builder.build_create_type(&stmt);
		assert_eq!(
			sql,
			r#"CREATE TYPE "int_range" AS RANGE (SUBTYPE = integer, SUBTYPE_DIFF = int4range_subdiff, CANONICAL = int4range_canonical)"#
		);
		assert_eq!(values.len(), 0);
	}

	// ALTER TYPE tests
	#[test]
	fn test_alter_type_rename_to() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("old_name").rename_to("new_name");

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "old_name" RENAME TO "new_name""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_owner_to() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_type").owner_to("new_owner");

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "my_type" OWNER TO "new_owner""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_set_schema() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_type").set_schema("new_schema");

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "my_type" SET SCHEMA "new_schema""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_add_value() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("mood").add_value("excited", None);

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "mood" ADD VALUE 'excited'"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_add_value_before() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("mood").add_value("excited", Some("happy"));

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(
			sql,
			r#"ALTER TYPE "mood" ADD VALUE 'excited' BEFORE 'happy'"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_rename_value() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("mood").rename_value("happy", "joyful");

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "mood" RENAME VALUE 'happy' TO 'joyful'"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_add_constraint() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_domain")
			.add_constraint("positive_check", "CHECK (VALUE > 0)");

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(
			sql,
			r#"ALTER TYPE "my_domain" ADD CONSTRAINT "positive_check" CHECK (VALUE > 0)"#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_drop_constraint() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_domain")
			.drop_constraint("my_constraint", false);

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(
			sql,
			r#"ALTER TYPE "my_domain" DROP CONSTRAINT "my_constraint""#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_drop_constraint_if_exists() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_domain")
			.drop_constraint("my_constraint", true);

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(
			sql,
			r#"ALTER TYPE "my_domain" DROP CONSTRAINT IF EXISTS "my_constraint""#
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_set_default() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_domain").set_default("0");

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "my_domain" SET DEFAULT 0"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_drop_default() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_domain").drop_default();

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "my_domain" DROP DEFAULT"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_set_not_null() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_domain").set_not_null();

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "my_domain" SET NOT NULL"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_alter_type_drop_not_null() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("my_domain").drop_not_null();

		let (sql, values) = builder.build_alter_type(&stmt);
		assert_eq!(sql, r#"ALTER TYPE "my_domain" DROP NOT NULL"#);
		assert_eq!(values.len(), 0);
	}

	// DROP TYPE tests
	#[test]
	fn test_drop_type_basic() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_type();
		stmt.name("my_type");

		let (sql, values) = builder.build_drop_type(&stmt);
		assert_eq!(sql, r#"DROP TYPE "my_type""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_type_if_exists() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_type();
		stmt.name("my_type").if_exists();

		let (sql, values) = builder.build_drop_type(&stmt);
		assert_eq!(sql, r#"DROP TYPE IF EXISTS "my_type""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_type_cascade() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_type();
		stmt.name("my_type").cascade();

		let (sql, values) = builder.build_drop_type(&stmt);
		assert_eq!(sql, r#"DROP TYPE "my_type" CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_type_restrict() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_type();
		stmt.name("my_type").restrict();

		let (sql, values) = builder.build_drop_type(&stmt);
		assert_eq!(sql, r#"DROP TYPE "my_type" RESTRICT"#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_type_all_options() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::drop_type();
		stmt.name("my_type").if_exists().cascade();

		let (sql, values) = builder.build_drop_type(&stmt);
		assert_eq!(sql, r#"DROP TYPE IF EXISTS "my_type" CASCADE"#);
		assert_eq!(values.len(), 0);
	}

	// MySQL-specific maintenance command panic tests
	#[test]
	#[should_panic(expected = "PostgreSQL users should use VACUUM ANALYZE")]
	fn test_optimize_table_panics() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::optimize_table();
		stmt.table("users");

		let _ = builder.build_optimize_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "not supported in PostgreSQL")]
	fn test_repair_table_panics() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::repair_table();
		stmt.table("users");

		let _ = builder.build_repair_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "not supported in PostgreSQL")]
	fn test_check_table_panics() {
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::check_table();
		stmt.table("users");

		let _ = builder.build_check_table(&stmt);
	}

	// DCL (Data Control Language) Tests

	#[test]
	fn test_grant_single_privilege_on_table() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT SELECT ON TABLE "users" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_multiple_privileges() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privileges(vec![
				Privilege::Select,
				Privilege::Insert,
				Privilege::Update,
			])
			.on_table("users")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT SELECT, INSERT, UPDATE ON TABLE "users" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_multiple_objects() {
		use crate::dcl::{GrantStatement, ObjectType, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.object_type(ObjectType::Table)
			.object("users")
			.object("posts")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT SELECT ON TABLE "users", "posts" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_multiple_grantees() {
		use crate::dcl::{GrantStatement, Grantee, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.grantee(Grantee::role("app_user"))
			.grantee(Grantee::role("readonly_user"));

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT SELECT ON TABLE "users" TO "app_user", "readonly_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_with_grant_option() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.to("app_user")
			.with_grant_option(true);

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT SELECT ON TABLE "users" TO "app_user" WITH GRANT OPTION"#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_with_granted_by() {
		use crate::dcl::{GrantStatement, Grantee, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.to("app_user")
			.granted_by(Grantee::role("admin"));

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT SELECT ON TABLE "users" TO "app_user" GRANTED BY "admin""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_on_database() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Create)
			.on_database("mydb")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT CREATE ON DATABASE "mydb" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_on_schema() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_schema("public")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT USAGE ON SCHEMA "public" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_on_sequence() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_sequence("user_id_seq")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT USAGE ON SEQUENCE "user_id_seq" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_all_privileges() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::All)
			.on_table("users")
			.to("admin");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT ALL PRIVILEGES ON TABLE "users" TO "admin""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_to_public() {
		use crate::dcl::{GrantStatement, Grantee, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("public_data")
			.grantee(Grantee::Public);

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT SELECT ON TABLE "public_data" TO PUBLIC"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_to_current_user() {
		use crate::dcl::{GrantStatement, Grantee, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.grantee(Grantee::CurrentUser);

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT SELECT ON TABLE "users" TO CURRENT_USER"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_grant_complex() {
		use crate::dcl::{GrantStatement, Grantee, Privilege};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privileges(vec![
				Privilege::Select,
				Privilege::Insert,
				Privilege::Update,
			])
			.on_table("users")
			.on_table("posts")
			.grantee(Grantee::role("app_user"))
			.grantee(Grantee::role("readonly_user"))
			.with_grant_option(true)
			.granted_by(Grantee::role("admin"));

		let (sql, values) = builder.build_grant(&stmt);
		assert!(sql.starts_with("GRANT SELECT, INSERT, UPDATE ON TABLE"));
		assert!(sql.contains(r#""users", "posts""#));
		assert!(sql.contains(r#"TO "app_user", "readonly_user""#));
		assert!(sql.contains("WITH GRANT OPTION"));
		assert!(sql.contains(r#"GRANTED BY "admin""#));
		assert!(values.is_empty());
	}

	// REVOKE tests

	#[test]
	fn test_revoke_single_privilege() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Insert)
			.from_table("users")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE INSERT ON TABLE "users" FROM "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_multiple_privileges() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privileges(vec![
				Privilege::Select,
				Privilege::Insert,
				Privilege::Update,
			])
			.from_table("users")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE SELECT, INSERT, UPDATE ON TABLE "users" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_with_cascade() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::All)
			.from_table("users")
			.from("app_user")
			.cascade(true);

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE ALL PRIVILEGES ON TABLE "users" FROM "app_user" CASCADE"#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_grant_option_for() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Select)
			.from_table("users")
			.from("app_user")
			.grant_option_for(true);

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE GRANT OPTION FOR SELECT ON TABLE "users" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_from_database() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Create)
			.from_database("mydb")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE CREATE ON DATABASE "mydb" FROM "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_from_schema() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_schema("public")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE USAGE ON SCHEMA "public" FROM "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_from_sequence() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_sequence("user_id_seq")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE USAGE ON SEQUENCE "user_id_seq" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_from_public() {
		use crate::dcl::{Grantee, Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Select)
			.from_table("public_data")
			.grantee(Grantee::Public);

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE SELECT ON TABLE "public_data" FROM PUBLIC"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_from_current_user() {
		use crate::dcl::{Grantee, Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Select)
			.from_table("users")
			.grantee(Grantee::CurrentUser);

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE SELECT ON TABLE "users" FROM CURRENT_USER"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_revoke_complex() {
		use crate::dcl::{Grantee, Privilege, RevokeStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privileges(vec![Privilege::Select, Privilege::Insert])
			.from_table("users")
			.from_table("posts")
			.grantee(Grantee::role("app_user"))
			.grantee(Grantee::role("readonly_user"))
			.cascade(true);

		let (sql, values) = builder.build_revoke(&stmt);
		assert!(sql.starts_with("REVOKE SELECT, INSERT ON TABLE"));
		assert!(sql.contains(r#""users", "posts""#));
		assert!(sql.contains(r#"FROM "app_user", "readonly_user""#));
		assert!(sql.contains("CASCADE"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_create_role_simple() {
		use crate::dcl::CreateRoleStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = CreateRoleStatement::new().role("developer");

		let (sql, values) = builder.build_create_role(&stmt);
		assert_eq!(sql, r#"CREATE ROLE "developer""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_create_role_with_login() {
		use crate::dcl::{CreateRoleStatement, RoleAttribute};
		use crate::value::Value;

		let builder = PostgresQueryBuilder::new();
		let stmt = CreateRoleStatement::new()
			.role("app_user")
			.attribute(RoleAttribute::Login)
			.attribute(RoleAttribute::Password("secret".to_string()));

		let (sql, values) = builder.build_create_role(&stmt);
		assert_eq!(sql, r#"CREATE ROLE "app_user" WITH LOGIN PASSWORD $1"#);
		assert_eq!(values.len(), 1);
		assert_eq!(
			values[0],
			Value::String(Some(Box::new("secret".to_string())))
		);
	}

	#[test]
	fn test_create_role_with_multiple_attributes() {
		use crate::dcl::{CreateRoleStatement, RoleAttribute};

		let builder = PostgresQueryBuilder::new();
		let stmt = CreateRoleStatement::new()
			.role("superuser")
			.attribute(RoleAttribute::SuperUser)
			.attribute(RoleAttribute::CreateDb)
			.attribute(RoleAttribute::CreateRole)
			.attribute(RoleAttribute::ConnectionLimit(10));

		let (sql, values) = builder.build_create_role(&stmt);
		assert_eq!(
			sql,
			r#"CREATE ROLE "superuser" WITH SUPERUSER CREATEDB CREATEROLE CONNECTION LIMIT 10"#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_drop_role_simple() {
		use crate::dcl::DropRoleStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = DropRoleStatement::new().role("old_role");

		let (sql, values) = builder.build_drop_role(&stmt);
		assert_eq!(sql, r#"DROP ROLE "old_role""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_drop_role_if_exists() {
		use crate::dcl::DropRoleStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = DropRoleStatement::new().role("old_role").if_exists(true);

		let (sql, values) = builder.build_drop_role(&stmt);
		assert_eq!(sql, r#"DROP ROLE IF EXISTS "old_role""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_drop_role_multiple() {
		use crate::dcl::DropRoleStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = DropRoleStatement::new()
			.role("role1")
			.role("role2")
			.role("role3");

		let (sql, values) = builder.build_drop_role(&stmt);
		assert_eq!(sql, r#"DROP ROLE "role1", "role2", "role3""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_alter_role_with_attributes() {
		use crate::dcl::{AlterRoleStatement, RoleAttribute};

		let builder = PostgresQueryBuilder::new();
		let stmt = AlterRoleStatement::new()
			.role("developer")
			.attribute(RoleAttribute::NoLogin)
			.attribute(RoleAttribute::ConnectionLimit(5));

		let (sql, values) = builder.build_alter_role(&stmt);
		assert_eq!(
			sql,
			r#"ALTER ROLE "developer" WITH NOLOGIN CONNECTION LIMIT 5"#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_alter_role_rename_to() {
		use crate::dcl::AlterRoleStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = AlterRoleStatement::new()
			.role("old_name")
			.rename_to("new_name");

		let (sql, values) = builder.build_alter_role(&stmt);
		assert_eq!(sql, r#"ALTER ROLE "old_name" RENAME TO "new_name""#);
		assert!(values.is_empty());
	}

	// CREATE USER tests
	#[test]
	fn test_create_user_basic() {
		use crate::dcl::CreateUserStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = CreateUserStatement::new().user("app_user");

		let (sql, values) = builder.build_create_user(&stmt);
		assert_eq!(sql, r#"CREATE ROLE "app_user" WITH LOGIN"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_create_user_with_password() {
		use crate::dcl::{CreateUserStatement, RoleAttribute};
		use crate::value::Value;

		let builder = PostgresQueryBuilder::new();
		let stmt = CreateUserStatement::new()
			.user("app_user")
			.attribute(RoleAttribute::Password("secret".to_string()));

		let (sql, values) = builder.build_create_user(&stmt);
		assert_eq!(sql, r#"CREATE ROLE "app_user" WITH LOGIN PASSWORD $1"#);
		assert_eq!(values.len(), 1);
		assert_eq!(
			values[0],
			Value::String(Some(Box::new("secret".to_string())))
		);
	}

	// DROP USER tests
	#[test]
	fn test_drop_user_basic() {
		use crate::dcl::DropUserStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = DropUserStatement::new().user("app_user");

		let (sql, values) = builder.build_drop_user(&stmt);
		assert_eq!(sql, r#"DROP ROLE "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_drop_user_if_exists() {
		use crate::dcl::DropUserStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = DropUserStatement::new().user("app_user").if_exists(true);

		let (sql, values) = builder.build_drop_user(&stmt);
		assert_eq!(sql, r#"DROP ROLE IF EXISTS "app_user""#);
		assert!(values.is_empty());
	}

	// ALTER USER tests
	#[test]
	fn test_alter_user_basic() {
		use crate::dcl::{AlterUserStatement, RoleAttribute};
		use crate::value::Value;

		let builder = PostgresQueryBuilder::new();
		let stmt = AlterUserStatement::new()
			.user("app_user")
			.attribute(RoleAttribute::Password("new_secret".to_string()));

		let (sql, values) = builder.build_alter_user(&stmt);
		assert_eq!(sql, r#"ALTER ROLE "app_user" WITH PASSWORD $1"#);
		assert_eq!(values.len(), 1);
		assert_eq!(
			values[0],
			Value::String(Some(Box::new("new_secret".to_string())))
		);
	}

	// RENAME USER panic test
	#[test]
	#[should_panic(expected = "RENAME USER is not supported by PostgreSQL")]
	fn test_rename_user_panics() {
		use crate::dcl::RenameUserStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = RenameUserStatement::new().rename("old", "new");

		builder.build_rename_user(&stmt);
	}

	// SET ROLE tests
	#[test]
	fn test_set_role_named() {
		use crate::dcl::{RoleTarget, SetRoleStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = SetRoleStatement::new().role(RoleTarget::Named("admin".to_string()));

		let (sql, values) = builder.build_set_role(&stmt);
		assert_eq!(sql, r#"SET ROLE "admin""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_set_role_none() {
		use crate::dcl::{RoleTarget, SetRoleStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = SetRoleStatement::new().role(RoleTarget::None);

		let (sql, values) = builder.build_set_role(&stmt);
		assert_eq!(sql, "SET ROLE NONE");
		assert!(values.is_empty());
	}

	#[test]
	#[should_panic(expected = "SET ROLE ALL is not supported by PostgreSQL")]
	fn test_set_role_all_panics() {
		use crate::dcl::{RoleTarget, SetRoleStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = SetRoleStatement::new().role(RoleTarget::All);

		builder.build_set_role(&stmt);
	}

	// RESET ROLE test
	#[test]
	fn test_reset_role() {
		use crate::dcl::ResetRoleStatement;

		let builder = PostgresQueryBuilder::new();
		let stmt = ResetRoleStatement::new();

		let (sql, values) = builder.build_reset_role(&stmt);
		assert_eq!(sql, "RESET ROLE");
		assert!(values.is_empty());
	}

	// SET DEFAULT ROLE panic test
	#[test]
	#[should_panic(expected = "SET DEFAULT ROLE is not supported by PostgreSQL")]
	fn test_set_default_role_panics() {
		use crate::dcl::{DefaultRoleSpec, SetDefaultRoleStatement};

		let builder = PostgresQueryBuilder::new();
		let stmt = SetDefaultRoleStatement::new()
			.roles(DefaultRoleSpec::All)
			.user("app_user");

		builder.build_set_default_role(&stmt);
	}

	// ==================== SQL identifier escaping tests ====================

	#[rstest]
	fn test_as_enum_escapes_type_name_with_special_characters() {
		// Arrange
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::val("active").as_enum(Alias::new("user\"status")))
			.from("users");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert: enum type name must be quoted and inner quotes doubled
		assert!(sql.contains("::\"user\"\"status\""));
	}

	#[rstest]
	fn test_cast_escapes_type_name_with_special_characters() {
		// Arrange
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::col("age").cast_as(Alias::new("my\"type")))
			.from("users");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert: cast type name must be quoted and inner quotes doubled
		assert!(sql.contains("CAST(\"age\" AS \"my\"\"type\")"));
	}

	#[rstest]
	fn test_trigger_function_name_is_escaped() {
		// Arrange
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("test_trigger")
			.timing(crate::types::TriggerTiming::After)
			.event(crate::types::TriggerEvent::Insert)
			.on_table("users")
			.for_each(crate::types::TriggerScope::Row)
			.execute_function("my\"func");

		// Act
		let (sql, _) = builder.build_create_trigger(&stmt);

		// Assert: function name must be quoted and inner quotes doubled
		assert!(sql.contains("EXECUTE FUNCTION \"my\"\"func\"()"));
	}

	#[rstest]
	fn test_as_enum_normal_type_name_is_quoted() {
		// Arrange
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::val("active").as_enum(Alias::new("status")))
			.from("users");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert: even normal identifiers should be quoted
		assert!(sql.contains("::\"status\""));
	}

	#[rstest]
	fn test_cast_normal_type_name_is_quoted() {
		// Arrange
		let builder = PostgresQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::col("age").cast_as(Alias::new("INTEGER")))
			.from("users");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert: cast type name should be quoted
		assert!(sql.contains("CAST(\"age\" AS \"INTEGER\")"));
	}

	// ==================== Dollar-quote delimiter safety tests ====================

	#[rstest]
	fn test_safe_delimiter_default_when_body_has_no_dollar_quotes() {
		// Arrange
		let body = "BEGIN RETURN 1; END;";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert
		assert_eq!(delimiter, "$$");
	}

	#[rstest]
	fn test_safe_delimiter_avoids_collision_with_dollar_dollar() {
		// Arrange
		let body = "BEGIN $$ nested $$ END;";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert
		assert_ne!(
			delimiter, "$$",
			"Delimiter must not be $$ when body contains $$"
		);
		assert_eq!(delimiter, "$body_0$");
	}

	#[rstest]
	fn test_safe_delimiter_injection_attempt_with_dollar_quotes() {
		// Arrange: attacker tries to break out of dollar-quoting
		let body = "$$ ; DROP TABLE users; --";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert
		assert_ne!(delimiter, "$$");
		let delimiters = collect_dollar_quote_delimiters(body);
		assert!(
			!delimiters.contains(&delimiter),
			"Generated delimiter must not conflict with any delimiter in body"
		);
	}

	#[rstest]
	fn test_safe_delimiter_skips_collision_with_body_0() {
		// Arrange: body contains both $$ and $body_0$
		let body = "BEGIN $$ test $body_0$ END;";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert
		assert_eq!(delimiter, "$body_1$");
	}

	#[rstest]
	fn test_safe_delimiter_multiple_collisions() {
		// Arrange: body contains $$, $body_0$, $body_1$
		let body = "$$ $body_0$ $body_1$";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert
		assert_eq!(delimiter, "$body_2$");
	}

	#[rstest]
	fn test_safe_delimiter_ignores_dollar_amount_not_delimiter() {
		// Arrange: $100 is not a dollar-quote delimiter (digit after $)
		let body = "SELECT $100 + $200";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert: $$ is safe because $100 is not a delimiter
		assert_eq!(delimiter, "$$");
	}

	#[rstest]
	fn test_safe_delimiter_empty_body() {
		// Arrange
		let body = "";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert
		assert_eq!(delimiter, "$$");
	}

	#[rstest]
	fn test_safe_delimiter_whitespace_only_body() {
		// Arrange
		let body = "   \t\n  ";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert
		assert_eq!(delimiter, "$$");
	}

	#[rstest]
	fn test_safe_delimiter_nested_dollar_quotes() {
		// Arrange: body contains nested dollar-quoted strings
		let body = "$inner$ SELECT 1 $inner$ $$ nested $$";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert: must avoid both $$ and $inner$
		assert_ne!(delimiter, "$$");
		assert_ne!(delimiter, "$inner$");
		assert_eq!(delimiter, "$body_0$");
	}

	#[rstest]
	fn test_safe_delimiter_tag_style_delimiters() {
		// Arrange: body contains $tag$ style delimiters
		let body = "$func$ BEGIN RETURN 1; END; $func$";

		// Act
		let delimiter = generate_safe_dollar_quote_delimiter(body);

		// Assert: $$ is safe because body only contains $func$
		assert_eq!(delimiter, "$$");
	}

	// ==================== Dollar-quote delimiter collection tests ====================

	#[rstest]
	fn test_collect_delimiters_empty_body() {
		// Arrange
		let body = "";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert
		assert!(delimiters.is_empty());
	}

	#[rstest]
	fn test_collect_delimiters_no_dollar_signs() {
		// Arrange
		let body = "SELECT 1 + 2";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert
		assert!(delimiters.is_empty());
	}

	#[rstest]
	fn test_collect_delimiters_dollar_amounts_are_not_delimiters() {
		// Arrange: $1, $2 are PostgreSQL parameter placeholders, not delimiters
		let body = "SELECT $1 + $2";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert
		assert!(delimiters.is_empty());
	}

	#[rstest]
	fn test_collect_delimiters_finds_empty_tag() {
		// Arrange
		let body = "$$ body content $$";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert
		assert_eq!(delimiters.len(), 1);
		assert!(delimiters.contains("$$"));
	}

	#[rstest]
	fn test_collect_delimiters_finds_named_tag() {
		// Arrange
		let body = "$func$ body $func$";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert
		assert_eq!(delimiters.len(), 1);
		assert!(delimiters.contains("$func$"));
	}

	#[rstest]
	fn test_collect_delimiters_finds_multiple_tags() {
		// Arrange
		let body = "$$ outer $inner$ nested $inner$ outer $$";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert
		assert_eq!(delimiters.len(), 2);
		assert!(delimiters.contains("$$"));
		assert!(delimiters.contains("$inner$"));
	}

	#[rstest]
	fn test_collect_delimiters_underscore_in_tag() {
		// Arrange
		let body = "$my_tag$ content $my_tag$";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert
		assert!(delimiters.contains("$my_tag$"));
	}

	#[rstest]
	fn test_collect_delimiters_rejects_digit_start_tag() {
		// Arrange: $1tag$ is not valid because tag starts with digit
		let body = "$1tag$ content";

		// Act
		let delimiters = collect_dollar_quote_delimiters(body);

		// Assert: $1tag$ is not a valid delimiter
		assert!(!delimiters.contains("$1tag$"));
	}
}

/// Collect all dollar-quote delimiters present in the body text.
///
/// A dollar-quote delimiter in PostgreSQL has the form `$tag$` where `tag` is
/// either empty or consists of `[a-zA-Z0-9_]` characters not starting with a
/// digit. This function scans the body and returns the set of all such
/// delimiters (including the surrounding `$` signs).
///
/// Using exact delimiter boundary detection instead of substring matching
/// prevents false positives (e.g. `$100` is not a delimiter) and false
/// negatives (e.g. partial overlap with candidate delimiter tags).
fn collect_dollar_quote_delimiters(body: &str) -> std::collections::HashSet<String> {
	let mut delimiters = std::collections::HashSet::new();
	let bytes = body.as_bytes();
	let len = bytes.len();
	let mut i = 0;

	while i < len {
		if bytes[i] == b'$' {
			// Found a '$', try to parse a dollar-quote delimiter
			let start = i;
			i += 1;

			// Empty tag: `$$`
			if i < len && bytes[i] == b'$' {
				delimiters.insert("$$".to_string());
				i += 1;
				continue;
			}

			// Non-empty tag: tag must match [a-zA-Z_][a-zA-Z0-9_]*
			if i < len && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
				let tag_start = i;
				i += 1;
				while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
					i += 1;
				}
				// Check for closing '$'
				if i < len && bytes[i] == b'$' {
					let delimiter = &body[start..=i];
					delimiters.insert(delimiter.to_string());
					i += 1;
					continue;
				}
				// Not a valid delimiter, continue from after the initial '$'
				i = tag_start;
				continue;
			}

			// '$' followed by a digit or other non-tag character -- not a delimiter
			continue;
		}
		i += 1;
	}

	delimiters
}

/// Generate a safe dollar-quote delimiter that does not appear in the body.
///
/// PostgreSQL dollar-quoting uses `$$` as the default delimiter. If the function
/// body contains `$$`, an attacker could break out of the dollar-quoted string.
/// This function scans for all dollar-quote delimiter patterns in the body and
/// generates a unique delimiter that does not conflict with any of them.
fn generate_safe_dollar_quote_delimiter(body: &str) -> String {
	let existing = collect_dollar_quote_delimiters(body);

	if !existing.contains("$$") {
		return "$$".to_string();
	}

	// Try numbered delimiters: $body_0$, $body_1$, ...
	for i in 0u64.. {
		let candidate = format!("$body_{}$", i);
		if !existing.contains(&candidate) {
			return candidate;
		}
	}

	// Unreachable in practice, but satisfy the compiler
	"$$".to_string()
}

impl crate::query::QueryBuilderTrait for PostgresQueryBuilder {
	fn placeholder(&self) -> (&str, bool) {
		("$", true)
	}

	fn quote_char(&self) -> char {
		'"'
	}
}
