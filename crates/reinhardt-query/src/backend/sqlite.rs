//! SQLite query builder backend
//!
//! This module implements the SQL generation backend for SQLite.

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
	types::{BinOper, ColumnRef, TableRef},
	value::Values,
};

/// SQLite query builder
///
/// This struct implements SQL generation for SQLite, using the following conventions:
/// - Identifiers: Double quotes (`"table_name"`)
/// - Placeholders: Question marks (`?`)
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::backend::{SqliteQueryBuilder, QueryBuilder};
/// use reinhardt_query::prelude::*;
///
/// let builder = SqliteQueryBuilder::new();
/// let stmt = Query::select()
///     .column("id")
///     .from("users");
///
/// let (sql, values) = builder.build_select(&stmt);
/// // sql: SELECT "id" FROM "users"
/// ```
///
/// # Features
///
/// SQLite supports:
/// - RETURNING clause (SQLite 3.35+)
/// - FULL OUTER JOIN (via emulation)
/// - NULLS FIRST/LAST in ORDER BY
#[derive(Debug, Clone, Default)]
pub struct SqliteQueryBuilder;

impl SqliteQueryBuilder {
	/// Create a new SQLite query builder
	pub fn new() -> Self {
		Self
	}

	/// Escape an identifier for SQLite
	///
	/// SQLite uses double quotes for identifiers.
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

	/// Format a placeholder for SQLite
	///
	/// SQLite uses question mark placeholders (`?`).
	///
	/// # Arguments
	///
	/// * `_index` - The parameter index (ignored for SQLite)
	///
	/// # Returns
	///
	/// The placeholder string (always `?`)
	#[allow(clippy::unused_self)]
	fn placeholder(&self, _index: usize) -> String {
		"?".to_string()
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

				// SQLite uses ? placeholders, no adjustment needed
				writer.push("(");
				writer.push(&subquery_sql);
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
				writer.push_value(value.clone(), |_i| self.placeholder(0));
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

				// SQLite uses ? placeholders, no adjustment needed
				writer.push(&subquery_sql);
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
			SimpleExpr::AsEnum(_name, expr) => {
				// SQLite does not support PostgreSQL-style enum casting (::type),
				// so we render only the inner expression.
				self.write_simple_expr(writer, expr);
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

	/// Write a window statement (PARTITION BY ... ORDER BY ... frame_clause)
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

	/// Write a frame clause (ROWS/RANGE BETWEEN ... AND ...)
	fn write_frame_clause(&self, writer: &mut SqlWriter, frame: &crate::types::FrameClause) {
		use crate::types::FrameType;

		// Frame type (ROWS, RANGE)
		match frame.frame_type {
			FrameType::Rows => writer.push_keyword("ROWS"),
			FrameType::Range => writer.push_keyword("RANGE"),
			FrameType::Groups => {
				panic!("SQLite does not support GROUPS frame type. Use ROWS or RANGE instead.");
			}
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

impl QueryBuilder for SqliteQueryBuilder {
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

				// SQLite uses ? placeholders, no adjustment needed
				w.push(&cte_sql);
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
					// SELECT ALL - explicit but not required in SQLite
				}
				SelectDistinct::Distinct => {
					writer.push_keyword("DISTINCT");
					writer.push_space();
				}
				SelectDistinct::DistinctRow => {
					panic!("SQLite does not support DISTINCT ROW. Use DISTINCT instead.");
				}
				SelectDistinct::DistinctOn(_cols) => {
					panic!("SQLite does not support DISTINCT ON. Use DISTINCT instead.");
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

		// WINDOW clause (named windows)
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
			writer.push_value(limit.clone(), |_i| self.placeholder(0));
		}

		// OFFSET clause
		if let Some(offset) = &stmt.offset {
			writer.push_keyword("OFFSET");
			writer.push_space();
			writer.push_value(offset.clone(), |_i| self.placeholder(0));
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
			let (union_sql, union_values) = self.build_select(union_stmt);

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
						w2.push_value(value.clone(), |_i| self.placeholder(0));
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

		// RETURNING clause (SQLite 3.35+)
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

		// RETURNING clause (SQLite 3.35+)
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

		// RETURNING clause (SQLite 3.35+)
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

		// CREATE TABLE
		writer.push("CREATE TABLE");
		writer.push_space();

		// IF NOT EXISTS clause
		if stmt.if_not_exists {
			writer.push_keyword("IF NOT EXISTS");
			writer.push_space();
		}

		// Table name
		if let Some(table) = &stmt.table {
			self.write_table_ref(&mut writer, table);
		}

		writer.push_space();
		writer.push("(");

		// Column definitions
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
				writer.push(&self.column_type_to_sql(col_type));
			}

			// PRIMARY KEY
			if column.primary_key {
				writer.push(" PRIMARY KEY");
			}

			// AUTOINCREMENT (SQLite-specific, only for INTEGER PRIMARY KEY)
			if column.auto_increment {
				writer.push(" AUTOINCREMENT");
			}

			// NOT NULL
			if column.not_null {
				writer.push(" NOT NULL");
			}

			// UNIQUE
			if column.unique {
				writer.push(" UNIQUE");
			}

			// DEFAULT
			if let Some(default) = &column.default {
				writer.push(" DEFAULT ");
				self.write_simple_expr(&mut writer, default);
			}

			// CHECK constraint
			if let Some(check) = &column.check {
				writer.push(" CHECK (");
				self.write_simple_expr(&mut writer, check);
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

		// SQLite has very limited ALTER TABLE support
		// Only single operation per statement is allowed
		if stmt.operations.len() > 1 {
			panic!("SQLite does not support multiple ALTER TABLE operations in a single statement");
		}

		if let Some(operation) = stmt.operations.first() {
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
					if let Some(default) = &column_def.default {
						writer.push(" DEFAULT ");
						self.write_simple_expr(&mut writer, default);
					}
					if let Some(check) = &column_def.check {
						writer.push(" CHECK (");
						self.write_simple_expr(&mut writer, check);
						writer.push(")");
					}
				}
				AlterTableOperation::DropColumn { name, if_exists: _ } => {
					// SQLite 3.35.0+ only
					writer.push("DROP COLUMN");
					writer.push_space();
					writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
				}
				AlterTableOperation::RenameColumn { old, new } => {
					// SQLite 3.25.0+ only
					writer.push("RENAME COLUMN");
					writer.push_space();
					writer.push_identifier(&old.to_string(), |s| self.escape_iden(s));
					writer.push_space();
					writer.push("TO");
					writer.push_space();
					writer.push_identifier(&new.to_string(), |s| self.escape_iden(s));
				}
				AlterTableOperation::RenameTable(new_name) => {
					writer.push("RENAME TO");
					writer.push_space();
					writer.push_identifier(&new_name.to_string(), |s| self.escape_iden(s));
				}
				AlterTableOperation::ModifyColumn(_) => {
					panic!("SQLite does not support MODIFY COLUMN - table recreation required");
				}
				AlterTableOperation::AddConstraint(_) => {
					panic!(
						"SQLite does not support ADD CONSTRAINT - constraints must be defined during table creation"
					);
				}
				AlterTableOperation::DropConstraint { .. } => {
					panic!("SQLite does not support DROP CONSTRAINT - table recreation required");
				}
			}
		}

		writer.finish()
	}

	fn build_drop_table(&self, stmt: &DropTableStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push("DROP TABLE");
		writer.push_space();

		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
			writer.push_space();
		}

		writer.push_list(&stmt.tables, ", ", |w, table_ref| {
			self.write_table_ref(w, table_ref);
		});

		// Note: SQLite does not support CASCADE or RESTRICT for DROP TABLE

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

		// WHERE clause (partial index - supported in SQLite)
		if let Some(where_expr) = &stmt.r#where {
			writer.push_space();
			writer.push_keyword("WHERE");
			writer.push_space();
			self.write_simple_expr(&mut writer, where_expr);
		}

		// USING method is NOT supported in SQLite - ignore stmt.using

		writer.finish()
	}

	fn build_drop_index(&self, stmt: &DropIndexStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		writer.push("DROP INDEX");
		writer.push_space();

		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
			writer.push_space();
		}

		if let Some(name) = &stmt.name {
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		}

		// Note: SQLite does not support CASCADE or RESTRICT for DROP INDEX

		writer.finish()
	}

	fn build_create_view(&self, stmt: &CreateViewStatement) -> (String, Values) {
		let mut writer = SqlWriter::new();

		// SQLite does not support OR REPLACE for views
		if stmt.or_replace {
			panic!("SQLite does not support OR REPLACE for CREATE VIEW");
		}

		// SQLite does not support MATERIALIZED views
		if stmt.materialized {
			panic!("SQLite does not support MATERIALIZED views");
		}

		writer.push("CREATE");
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

		// SQLite does not support MATERIALIZED views
		if stmt.materialized {
			panic!("SQLite does not support MATERIALIZED views");
		}

		// SQLite only supports dropping one view at a time
		if stmt.names.len() > 1 {
			panic!("SQLite only supports dropping one view at a time");
		}

		// SQLite does not support CASCADE/RESTRICT for DROP VIEW
		if stmt.cascade || stmt.restrict {
			panic!("SQLite does not support CASCADE/RESTRICT for DROP VIEW");
		}

		writer.push("DROP");
		writer.push_keyword("VIEW");

		if stmt.if_exists {
			writer.push_keyword("IF EXISTS");
		}

		if let Some(name) = stmt.names.first() {
			writer.push_space();
			writer.push_identifier(&name.to_string(), |s| self.escape_iden(s));
		}

		writer.finish()
	}

	fn build_truncate_table(&self, stmt: &TruncateTableStatement) -> (String, Values) {
		// SQLite does not support the TRUNCATE keyword
		// We use DELETE FROM instead, which has similar effect but doesn't reset AUTO_INCREMENT

		// SQLite does not support RESTART IDENTITY, CASCADE, or RESTRICT for TRUNCATE/DELETE
		if stmt.restart_identity {
			panic!("SQLite does not support RESTART IDENTITY for TRUNCATE TABLE");
		}
		if stmt.cascade {
			panic!("SQLite does not support CASCADE for TRUNCATE TABLE");
		}
		if stmt.restrict {
			panic!("SQLite does not support RESTRICT for TRUNCATE TABLE");
		}

		// SQLite only supports truncating one table at a time
		if stmt.tables.len() > 1 {
			panic!("SQLite only supports truncating one table at a time");
		}

		let mut writer = SqlWriter::new();

		// Use DELETE FROM instead of TRUNCATE TABLE
		writer.push("DELETE FROM");
		writer.push_space();

		// Table name (single table only)
		if let Some(table_ref) = stmt.tables.first() {
			self.write_table_ref(&mut writer, table_ref);
		}

		writer.finish()
	}

	fn build_create_trigger(&self, stmt: &CreateTriggerStatement) -> (String, Values) {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		// SQLite only supports a single event per trigger
		if stmt.events.len() > 1 {
			panic!("SQLite does not support multiple events in a single trigger");
		}

		// SQLite only supports FOR EACH ROW (implicit)
		if matches!(stmt.scope, Some(TriggerScope::Statement)) {
			panic!("SQLite only supports FOR EACH ROW triggers");
		}

		// SQLite does not support FOLLOWS/PRECEDES
		if stmt.order.is_some() {
			panic!("SQLite does not support FOLLOWS/PRECEDES syntax");
		}

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

		// Event: INSERT / UPDATE [OF columns] / DELETE
		if let Some(event) = stmt.events.first() {
			writer.push_space();
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

		// ON table
		writer.push_keyword("ON");
		if let Some(table) = &stmt.table {
			writer.push_space();
			self.write_table_ref(&mut writer, table);
		}

		// FOR EACH ROW (optional in SQLite, but we can include it for clarity)
		if matches!(stmt.scope, Some(TriggerScope::Row)) {
			writer.push_keyword("FOR EACH ROW");
		}

		// WHEN (condition)
		if let Some(when_cond) = &stmt.when_condition {
			writer.push_keyword("WHEN");
			writer.push(" ");
			self.write_simple_expr(&mut writer, when_cond);
		}

		// BEGIN ... END block
		if let Some(body) = &stmt.body {
			writer.push_space();
			match body {
				TriggerBody::Single(sql) => {
					writer.push("BEGIN ");
					writer.push(sql.as_str());
					writer.push("; END");
				}
				TriggerBody::Multiple(statements) => {
					writer.push("BEGIN ");
					for (i, stmt_sql) in statements.iter().enumerate() {
						if i > 0 {
							writer.push(" ");
						}
						writer.push(stmt_sql);
						writer.push(";");
					}
					writer.push(" END");
				}
				TriggerBody::PostgresFunction(_) => {
					panic!("SQLite does not support EXECUTE FUNCTION syntax");
				}
			}
		}

		writer.finish()
	}

	fn build_drop_trigger(&self, stmt: &DropTriggerStatement) -> (String, Values) {
		// SQLite does not support CASCADE/RESTRICT
		if stmt.cascade || stmt.restrict {
			panic!("SQLite does not support CASCADE/RESTRICT for DROP TRIGGER");
		}

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

		// Note: SQLite does not require or support ON table in DROP TRIGGER

		writer.finish()
	}

	fn build_alter_index(&self, _stmt: &AlterIndexStatement) -> (String, Values) {
		panic!("SQLite does not support ALTER INDEX. Drop and recreate the index instead.");
	}

	fn build_reindex(&self, stmt: &ReindexStatement) -> (String, Values) {
		use crate::types::Iden;

		// SQLite does not support options (concurrently, verbose, tablespace)
		if stmt.concurrently {
			panic!("SQLite does not support CONCURRENTLY option for REINDEX");
		}
		if stmt.verbose {
			panic!("SQLite does not support VERBOSE option for REINDEX");
		}
		if stmt.tablespace.is_some() {
			panic!("SQLite does not support TABLESPACE option for REINDEX");
		}

		// SQLite only supports REINDEX for INDEX or TABLE (not SCHEMA, DATABASE, SYSTEM)
		if let Some(target) = stmt.target {
			use crate::query::ReindexTarget;
			match target {
				ReindexTarget::Schema | ReindexTarget::Database | ReindexTarget::System => {
					panic!("SQLite only supports REINDEX INDEX or REINDEX TABLE");
				}
				_ => {}
			}
		}

		let mut writer = SqlWriter::new();

		// REINDEX
		writer.push_keyword("REINDEX");

		// Name (optional in SQLite - reindexes all if omitted)
		if let Some(ref name) = stmt.name {
			writer.push_space();
			writer.push_identifier(&Iden::to_string(name.as_ref()), |s| self.escape_iden(s));
		}

		writer.finish()
	}

	fn build_create_function(
		&self,
		_stmt: &crate::query::CreateFunctionStatement,
	) -> (String, Values) {
		panic!(
			"SQLite does not support CREATE FUNCTION (user-defined functions are registered via API)"
		)
	}

	fn build_alter_function(
		&self,
		_stmt: &crate::query::AlterFunctionStatement,
	) -> (String, Values) {
		panic!(
			"SQLite does not support ALTER FUNCTION (user-defined functions are registered via API)"
		)
	}

	fn build_drop_function(&self, _stmt: &crate::query::DropFunctionStatement) -> (String, Values) {
		panic!(
			"SQLite does not support DROP FUNCTION (user-defined functions are registered via API)"
		)
	}

	fn build_grant(&self, _stmt: &crate::dcl::GrantStatement) -> (String, Values) {
		panic!(
			"SQLite does not support DCL (GRANT/REVOKE) statements. Use file-based permissions instead."
		);
	}

	fn build_revoke(&self, _stmt: &crate::dcl::RevokeStatement) -> (String, Values) {
		panic!(
			"SQLite does not support DCL (GRANT/REVOKE) statements. Use file-based permissions instead."
		);
	}

	fn build_grant_role(&self, _stmt: &crate::dcl::GrantRoleStatement) -> (String, Values) {
		panic!(
			"SQLite does not support DCL (GRANT role) statements. Use file-based permissions instead."
		);
	}

	fn build_revoke_role(&self, _stmt: &crate::dcl::RevokeRoleStatement) -> (String, Values) {
		panic!(
			"SQLite does not support DCL (REVOKE role) statements. Use file-based permissions instead."
		);
	}

	fn build_create_role(&self, _stmt: &crate::dcl::CreateRoleStatement) -> (String, Values) {
		panic!("SQLite does not support CREATE ROLE statement")
	}

	fn build_drop_role(&self, _stmt: &crate::dcl::DropRoleStatement) -> (String, Values) {
		panic!("SQLite does not support DROP ROLE statement")
	}

	fn build_alter_role(&self, _stmt: &crate::dcl::AlterRoleStatement) -> (String, Values) {
		panic!("SQLite does not support ALTER ROLE statement")
	}

	fn build_create_user(&self, _stmt: &crate::dcl::CreateUserStatement) -> (String, Values) {
		panic!("SQLite does not support CREATE USER statement")
	}

	fn build_drop_user(&self, _stmt: &crate::dcl::DropUserStatement) -> (String, Values) {
		panic!("SQLite does not support DROP USER statement")
	}

	fn build_alter_user(&self, _stmt: &crate::dcl::AlterUserStatement) -> (String, Values) {
		panic!("SQLite does not support ALTER USER statement")
	}

	fn build_rename_user(&self, _stmt: &crate::dcl::RenameUserStatement) -> (String, Values) {
		panic!("SQLite does not support RENAME USER statement")
	}

	fn build_set_role(&self, _stmt: &crate::dcl::SetRoleStatement) -> (String, Values) {
		panic!("SQLite does not support SET ROLE statement")
	}

	fn build_reset_role(&self, _stmt: &crate::dcl::ResetRoleStatement) -> (String, Values) {
		panic!("SQLite does not support RESET ROLE statement")
	}

	fn build_set_default_role(
		&self,
		_stmt: &crate::dcl::SetDefaultRoleStatement,
	) -> (String, Values) {
		panic!("SQLite does not support SET DEFAULT ROLE statement")
	}

	fn escape_identifier(&self, ident: &str) -> String {
		self.escape_iden(ident)
	}

	fn format_placeholder(&self, index: usize) -> String {
		self.placeholder(index)
	}

	fn build_create_schema(&self, _stmt: &crate::query::CreateSchemaStatement) -> (String, Values) {
		panic!("SQLite does not support schemas (all objects are in the main database).");
	}

	fn build_alter_schema(&self, _stmt: &crate::query::AlterSchemaStatement) -> (String, Values) {
		panic!("SQLite does not support schemas.");
	}

	fn build_drop_schema(&self, _stmt: &crate::query::DropSchemaStatement) -> (String, Values) {
		panic!("SQLite does not support schemas.");
	}

	fn build_create_sequence(
		&self,
		_stmt: &crate::query::CreateSequenceStatement,
	) -> (String, Values) {
		panic!("SQLite does not support sequences. Use AUTOINCREMENT instead.");
	}

	fn build_alter_sequence(
		&self,
		_stmt: &crate::query::AlterSequenceStatement,
	) -> (String, Values) {
		panic!("SQLite does not support sequences.");
	}

	fn build_drop_sequence(&self, _stmt: &crate::query::DropSequenceStatement) -> (String, Values) {
		panic!("SQLite does not support sequences.");
	}

	fn build_comment(&self, _stmt: &crate::query::CommentStatement) -> (String, Values) {
		panic!("SQLite does not support COMMENT ON statement.");
	}

	fn build_create_database(
		&self,
		_stmt: &crate::query::CreateDatabaseStatement,
	) -> (String, Values) {
		panic!(
			"SQLite does not support CREATE DATABASE. Databases are created as separate files via API."
		);
	}

	fn build_alter_database(
		&self,
		_stmt: &crate::query::AlterDatabaseStatement,
	) -> (String, Values) {
		panic!("SQLite does not support ALTER DATABASE.");
	}

	fn build_drop_database(&self, _stmt: &crate::query::DropDatabaseStatement) -> (String, Values) {
		panic!(
			"SQLite does not support DROP DATABASE. Database files are removed via filesystem operations."
		);
	}

	fn build_analyze(&self, _stmt: &crate::query::AnalyzeStatement) -> (String, Values) {
		panic!("SQLite ANALYZE has different syntax. Not supported via this builder.");
	}

	fn build_vacuum(&self, _stmt: &crate::query::VacuumStatement) -> (String, Values) {
		panic!(
			"SQLite VACUUM has different syntax (no table specification). Not supported via this builder."
		);
	}

	fn build_create_materialized_view(
		&self,
		_stmt: &crate::query::CreateMaterializedViewStatement,
	) -> (String, Values) {
		panic!("SQLite does not support materialized views.");
	}

	fn build_alter_materialized_view(
		&self,
		_stmt: &crate::query::AlterMaterializedViewStatement,
	) -> (String, Values) {
		panic!("SQLite does not support materialized views.");
	}

	fn build_drop_materialized_view(
		&self,
		_stmt: &crate::query::DropMaterializedViewStatement,
	) -> (String, Values) {
		panic!("SQLite does not support materialized views.");
	}

	fn build_refresh_materialized_view(
		&self,
		_stmt: &crate::query::RefreshMaterializedViewStatement,
	) -> (String, Values) {
		panic!("SQLite does not support materialized views.");
	}

	fn build_create_procedure(
		&self,
		_stmt: &crate::query::CreateProcedureStatement,
	) -> (String, Values) {
		panic!("SQLite does not support stored procedures.");
	}

	fn build_alter_procedure(
		&self,
		_stmt: &crate::query::AlterProcedureStatement,
	) -> (String, Values) {
		panic!("SQLite does not support stored procedures.");
	}

	fn build_drop_procedure(
		&self,
		_stmt: &crate::query::DropProcedureStatement,
	) -> (String, Values) {
		panic!("SQLite does not support stored procedures.");
	}

	fn build_create_type(&self, _stmt: &crate::query::CreateTypeStatement) -> (String, Values) {
		panic!("CREATE TYPE not supported.");
	}

	fn build_alter_type(&self, _stmt: &crate::query::AlterTypeStatement) -> (String, Values) {
		panic!("ALTER TYPE not supported.");
	}

	fn build_drop_type(&self, _stmt: &crate::query::DropTypeStatement) -> (String, Values) {
		panic!("DROP TYPE not supported.");
	}

	fn build_optimize_table(&self, _stmt: &OptimizeTableStatement) -> (String, Values) {
		panic!(
			"OPTIMIZE TABLE is MySQL-specific. SQLite users should use VACUUM or ANALYZE instead."
		);
	}

	fn build_repair_table(&self, _stmt: &RepairTableStatement) -> (String, Values) {
		panic!(
			"REPAIR TABLE is not supported in SQLite. SQLite databases are automatically repaired via WAL mode or PRAGMA integrity_check."
		);
	}

	fn build_check_table(&self, _stmt: &CheckTableStatement) -> (String, Values) {
		panic!(
			"CHECK TABLE is not supported in SQLite. Use PRAGMA integrity_check or PRAGMA quick_check instead."
		);
	}
}

// Helper methods for CREATE TABLE
impl SqliteQueryBuilder {
	/// Convert ColumnType to SQLite SQL type string
	fn column_type_to_sql(&self, col_type: &crate::types::ColumnType) -> String {
		use crate::types::ColumnType;
		use ColumnType::*;

		match col_type {
			Char(len) => format!("CHAR({})", len.unwrap_or(1)),
			String(len) => {
				if let Some(l) = len {
					format!("VARCHAR({})", l)
				} else {
					"TEXT".to_string()
				}
			}
			Text => "TEXT".to_string(),
			TinyInteger => "INTEGER".to_string(),
			SmallInteger => "INTEGER".to_string(),
			Integer => "INTEGER".to_string(),
			BigInteger => "INTEGER".to_string(),
			Float => "REAL".to_string(),
			Double => "REAL".to_string(),
			Decimal(_) => "REAL".to_string(), // SQLite doesn't have DECIMAL, use REAL
			Boolean => "INTEGER".to_string(), // SQLite doesn't have BOOLEAN, use INTEGER (0/1)
			Date => "TEXT".to_string(),       // SQLite stores dates as TEXT or INTEGER
			Time => "TEXT".to_string(),
			DateTime => "TEXT".to_string(),
			Timestamp => "INTEGER".to_string(), // Usually stored as UNIX timestamp
			TimestampWithTimeZone => "TEXT".to_string(), // ISO 8601 format
			Binary(len) => {
				if let Some(l) = len {
					format!("BLOB({})", l)
				} else {
					"BLOB".to_string()
				}
			}
			VarBinary(len) => format!("BLOB({})", len),
			Blob => "BLOB".to_string(),
			Uuid => "TEXT".to_string(), // UUID as TEXT (36 chars)
			Json => "TEXT".to_string(), // SQLite JSON1 extension stores JSON as TEXT
			JsonBinary => "TEXT".to_string(),
			Array(_) => "TEXT".to_string(), // SQLite doesn't have ARRAY, use TEXT (JSON)
			Custom(name) => name.clone(),
		}
	}

	/// Write table constraint to SQL writer
	fn write_table_constraint(
		&self,
		writer: &mut SqlWriter,
		constraint: &crate::types::TableConstraint,
	) {
		use crate::types::TableConstraint;
		use TableConstraint::*;

		match constraint {
			PrimaryKey { name, columns } => {
				if let Some(constraint_name) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&constraint_name.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("PRIMARY KEY");
				writer.push(" (");
				writer.push_list(columns, ", ", |w, col| {
					w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				});
				writer.push(")");
			}
			ForeignKey {
				name,
				columns,
				ref_table,
				ref_columns,
				on_delete,
				on_update,
			} => {
				if let Some(constraint_name) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&constraint_name.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("FOREIGN KEY");
				writer.push(" (");
				writer.push_list(columns, ", ", |w, col| {
					w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				});
				writer.push(")");
				writer.push_space();
				writer.push_keyword("REFERENCES");
				writer.push_space();
				self.write_table_ref(writer, ref_table);
				writer.push(" (");
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
			Unique { name, columns } => {
				if let Some(constraint_name) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&constraint_name.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("UNIQUE");
				writer.push(" (");
				writer.push_list(columns, ", ", |w, col| {
					w.push_identifier(&col.to_string(), |s| self.escape_iden(s));
				});
				writer.push(")");
			}
			Check { name, expr } => {
				if let Some(constraint_name) = name {
					writer.push_keyword("CONSTRAINT");
					writer.push_space();
					writer.push_identifier(&constraint_name.to_string(), |s| self.escape_iden(s));
					writer.push_space();
				}
				writer.push_keyword("CHECK");
				writer.push(" (");
				self.write_simple_expr(writer, expr);
				writer.push(")");
			}
		}
	}

	/// Convert ForeignKeyAction to SQL keyword
	fn foreign_key_action_to_sql(&self, action: &crate::types::ForeignKeyAction) -> &'static str {
		use crate::types::ForeignKeyAction;
		use ForeignKeyAction::*;

		match action {
			Cascade => "CASCADE",
			Restrict => "RESTRICT",
			SetNull => "SET NULL",
			SetDefault => "SET DEFAULT",
			NoAction => "NO ACTION",
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
		let builder = SqliteQueryBuilder::new();
		assert_eq!(builder.escape_identifier("user"), "\"user\"");
		assert_eq!(builder.escape_identifier("table_name"), "\"table_name\"");
	}

	#[test]
	fn test_escape_identifier_with_quotes() {
		let builder = SqliteQueryBuilder::new();
		assert_eq!(builder.escape_identifier("user\"name"), "\"user\"\"name\"");
	}

	#[test]
	fn test_format_placeholder() {
		let builder = SqliteQueryBuilder::new();
		assert_eq!(builder.format_placeholder(1), "?");
		assert_eq!(builder.format_placeholder(2), "?");
		assert_eq!(builder.format_placeholder(10), "?");
	}

	#[test]
	fn test_select_basic() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("id").column("name").from("users");

		let (sql, values) = builder.build_select(&stmt);
		assert_eq!(sql, "SELECT \"id\", \"name\" FROM \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_select_asterisk() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.from("users");

		let (sql, values) = builder.build_select(&stmt);
		assert_eq!(sql, "SELECT * FROM \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_select_with_where() {
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name", "email"])
			.values_panic(["Alice", "alice@example.com"]);

		let (sql, values) = builder.build_insert(&stmt);
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"name\", \"email\") VALUES (?, ?)"
		);
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_insert_multiple_rows() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::insert();
		stmt.into_table("users")
			.columns(["name", "email"])
			.values_panic(["Alice", "alice@example.com"])
			.values_panic(["Bob", "bob@example.com"]);

		let (sql, values) = builder.build_insert(&stmt);
		assert_eq!(
			sql,
			"INSERT INTO \"users\" (\"name\", \"email\") VALUES (?, ?), (?, ?)"
		);
		assert_eq!(values.len(), 4);
	}

	#[test]
	fn test_insert_with_returning() {
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();

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
		let builder = SqliteQueryBuilder::new();

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
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::update();
		stmt.table("users")
			.value("name", "Alice")
			.value("email", "alice@example.com");

		let (sql, values) = builder.build_update(&stmt);
		assert_eq!(sql, "UPDATE \"users\" SET \"name\" = ?, \"email\" = ?");
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_update_with_where() {
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::delete();
		stmt.from_table("users");

		let (sql, values) = builder.build_delete(&stmt);
		assert_eq!(sql, "DELETE FROM \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_delete_with_returning() {
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains("\"orders\".\"status\" = ?"));
		assert_eq!(values.len(), 1);
	}

	// GROUP BY / HAVING tests

	#[test]
	fn test_group_by_single_column() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("category")
			.from("products")
			.group_by("category");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("GROUP BY \"category\""));
	}

	#[test]
	fn test_group_by_multiple_columns() {
		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
		use crate::types::{BinOper, IntoIden};

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("category").from("products").distinct();

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.starts_with("SELECT DISTINCT"));
		assert!(sql.contains("\"category\""));
		assert!(sql.contains("FROM \"products\""));
	}

	#[test]
	#[should_panic(expected = "SQLite does not support DISTINCT ROW")]
	fn test_select_distinct_row_panics() {
		use crate::query::SelectDistinct;

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("products");
		stmt.distinct = Some(SelectDistinct::DistinctRow);

		let _ = builder.build_select(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support DISTINCT ON")]
	fn test_select_distinct_on_panics() {
		use crate::query::SelectDistinct;
		use crate::types::ColumnRef;

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("products");
		stmt.distinct = Some(SelectDistinct::DistinctOn(vec![ColumnRef::column(
			"category",
		)]));

		let _ = builder.build_select(&stmt);
	}

	#[test]
	fn test_select_union() {
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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

	// --- Phase 5: Subquery Edge Case Tests ---

	#[test]
	fn test_not_in_subquery() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#"SELECT "user_id" FROM "blocked_users""#));
		assert!(sql.contains(r#""reason" = ?"#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_subquery_in_select_list() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#""name""#));
		assert!(sql.contains(r#"(SELECT "count" FROM "order_counts""#));
		assert!(sql.contains(r#""order_counts"."user_id" = "users"."id""#));
	}

	#[test]
	fn test_multiple_exists_conditions() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#"EXISTS (SELECT "id" FROM "orders""#));
		assert!(sql.contains(r#"EXISTS (SELECT "id" FROM "reviews""#));
	}

	#[test]
	fn test_nested_subquery() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#"IN (SELECT "id" FROM "employees""#));
		assert!(sql.contains(r#"IN (SELECT "department_id" FROM "top_departments""#));
		assert!(sql.contains(r#""revenue" > ?"#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_subquery_with_complex_where() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#"IN (SELECT "product_id" FROM "inventory""#));
		assert!(sql.contains(r#""quantity" > ?"#));
		assert!(sql.contains(r#""warehouse" = ?"#));
		assert!(sql.contains(r#""status" = ?"#));
		assert!(sql.contains(r#""active" = ?"#));
		assert_eq!(values.len(), 4); // 0, "main", "available", true
	}

	#[test]
	fn test_from_subquery_preserves_parameter_values() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#") AS "active_admins""#));
		// Subquery params (true, "admin") + outer param ("A%") = 3 values
		assert_eq!(values.len(), 3);
	}

	// --- Phase 5: NULL Handling Tests ---

	#[test]
	fn test_where_is_null() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("deleted_at").is_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""deleted_at" IS"#));
		assert!(sql.to_uppercase().contains("NULL"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_where_is_not_null() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("email").is_not_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""email" IS NOT"#));
		assert!(sql.to_uppercase().contains("NULL"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_is_null_combined_with_other_conditions() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("active").eq(true))
			.and_where(Expr::col("deleted_at").is_null())
			.and_where(Expr::col("email").is_not_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""active" = ?"#));
		assert!(sql.contains(r#""deleted_at" IS"#));
		assert!(sql.contains(r#""email" IS NOT"#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_is_null_with_join() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column(("users", "name"))
			.from("users")
			.left_join(
				"profiles",
				Expr::col(("users", "id")).eq(Expr::col(("profiles", "user_id"))),
			)
			.and_where(Expr::col(("profiles", "id")).is_null());

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#"LEFT JOIN "profiles""#));
		assert!(sql.contains(r#""profiles"."id" IS"#));
		assert_eq!(values.len(), 0);
	}

	// --- Phase 5: Complex WHERE Clause Tests ---
	#[test]
	fn test_where_or_condition() {
		use crate::expr::Condition;

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("users").cond_where(
			Condition::any()
				.add(Expr::col("status").eq("active"))
				.add(Expr::col("status").eq("pending")),
		);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""status" = ?"#));
		assert!(sql.contains(" OR "));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_where_between() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("products")
			.and_where(Expr::col("price").between(100, 500));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""price" BETWEEN ?"#));
		assert!(sql.contains("AND ?"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_where_not_between() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("products")
			.and_where(Expr::col("price").not_between(0, 10));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""price" NOT BETWEEN ?"#));
		assert!(sql.contains("AND ?"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_where_like() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("email").like("%@gmail.com"));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""email" LIKE ?"#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_where_in_values() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name")
			.from("users")
			.and_where(Expr::col("role").is_in(vec!["admin", "moderator", "editor"]));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""role" IN"#));
		assert_eq!(values.len(), 3);
	}

	#[test]
	fn test_insert_with_null_value() {
		use crate::value::Value;

		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains(r#"INSERT INTO "users""#));
		assert!(sql.contains(r#""name""#));
		assert!(sql.contains(r#""email""#));
		assert!(sql.contains(r#""phone""#));
		// NULL values are inlined directly, not parameterized
		assert!(sql.contains("NULL"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_select_with_single_cte() {
		// Note: CTE (WITH clause) is supported in SQLite 3.8.3+
		let builder = SqliteQueryBuilder::new();

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
		// Note: CTE (WITH clause) is supported in SQLite 3.8.3+
		let builder = SqliteQueryBuilder::new();

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
		// Both CTEs should be present
		assert!(sql.contains("\"eng_emp\" AS"));
		assert!(sql.contains("\"sales_emp\" AS"));
	}

	#[test]
	fn test_select_with_recursive_cte() {
		// Note: Recursive CTE is supported in SQLite 3.8.3+
		let builder = SqliteQueryBuilder::new();

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

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_rank_basic() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_dense_rank_with_partition() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
			r#"SELECT NTILE(?) OVER ( ORDER BY "salary" ASC ), "name" FROM "employees""#
		);
	}

	#[test]
	fn test_window_lead_basic() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_lag_with_offset_and_default() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
				Some(2),
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

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_window_row_number_order_only() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_rank_order_only() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_ntile_custom_buckets() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
			r#"SELECT NTILE(?) OVER ( PARTITION BY "department" ORDER BY "salary" DESC ), "name" FROM "employees""#
		);
	}

	#[test]
	fn test_window_lead_with_offset_and_default() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_lag_with_different_offset() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_first_value_with_partition() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
	fn test_window_last_value_with_partition() {
		use crate::types::{Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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

	// --- Phase 5: JOIN Enhancement Tests ---

	#[test]
	fn test_join_three_tables() {
		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains(r#"LEFT JOIN "customers""#));
		assert!(sql.contains(r#""orders"."customer_id" = "customers"."id""#));
		assert!(sql.contains(r#"AND "customers"."active" = ?"#));
		assert!(sql.contains(r#"AND "orders"."created_at" > "customers"."registered_at""#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_join_with_subquery_in_condition() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#"INNER JOIN "profiles""#));
		assert!(sql.contains(r#""users"."id" = "profiles"."user_id""#));
		assert!(sql.contains("IN"));
		assert!(sql.contains(r#"SELECT "max_id" FROM "user_stats""#));
	}

	#[test]
	fn test_multiple_left_joins() {
		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains(r#"LEFT JOIN "profiles""#));
		assert!(sql.contains(r#"LEFT JOIN "addresses""#));
		assert!(sql.contains(r#"LEFT JOIN "phone_numbers""#));
	}

	#[test]
	fn test_mixed_join_types() {
		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains(r#"INNER JOIN "orders""#));
		assert!(sql.contains(r#"LEFT JOIN "reviews""#));
		assert!(sql.contains(r#"RIGHT JOIN "refunds""#));
	}

	#[test]
	fn test_join_with_group_by() {
		use crate::expr::SimpleExpr;
		use crate::types::{BinOper, ColumnRef, IntoIden};

		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains(r#"INNER JOIN "orders""#));
		assert!(sql.contains(r#"GROUP BY "users"."name""#));
		assert!(sql.contains("HAVING"));
		assert!(sql.contains("COUNT(*) > ?"));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_join_with_window_function() {
		use crate::types::{IntoIden, Order, OrderExpr, OrderExprKind, WindowStatement};

		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains(r#"INNER JOIN "departments""#));
		assert!(sql.contains("ROW_NUMBER() OVER"));
		assert!(sql.contains(r#"PARTITION BY "departments"."name""#));
	}

	#[test]
	fn test_four_table_join() {
		let builder = SqliteQueryBuilder::new();
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
		assert!(sql.contains(r#"FROM "users""#));
		assert!(sql.contains(r#"INNER JOIN "orders""#));
		assert!(sql.contains(r#"INNER JOIN "products""#));
		assert!(sql.contains(r#"INNER JOIN "categories""#));
	}

	#[test]
	fn test_join_with_cte() {
		use crate::types::TableRef;

		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#"WITH "high_value_customers" AS"#));
		assert!(sql.contains(r#"INNER JOIN "high_value_customers" AS "hvc""#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_cte_with_where_and_params() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#""status" = ?"#));
		assert!(sql.contains(r#""amount" > ?"#));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_cte_used_in_join() {
		use crate::types::TableRef;

		let builder = SqliteQueryBuilder::new();

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

		let builder = SqliteQueryBuilder::new();

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
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#""status" = ?"#));
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_multiple_recursive_and_regular_ctes() {
		let builder = SqliteQueryBuilder::new();

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
		assert!(sql.contains(r#""active" = ?"#));
		assert!(sql.contains(r#"FROM "category_tree""#));
		assert_eq!(values.len(), 1);
	}

	// CASE expression tests

	#[test]
	fn test_case_simple_when_else() {
		let builder = SqliteQueryBuilder::new();

		let case_expr = Expr::case()
			.when(Expr::col("status").eq("active"), "Active")
			.else_result("Inactive");

		let mut stmt = Query::select();
		stmt.expr_as(case_expr, "status_label").from("users");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("CASE"));
		assert!(sql.contains("WHEN"));
		assert!(sql.contains(r#""status" = ?"#));
		assert!(sql.contains("THEN"));
		assert!(sql.contains("ELSE"));
		assert!(sql.contains("END"));
		assert!(sql.contains(r#"AS "status_label""#));
		assert_eq!(values.len(), 3);
	}

	#[test]
	fn test_case_multiple_when_clauses() {
		let builder = SqliteQueryBuilder::new();

		let case_expr = Expr::case()
			.when(Expr::col("score").gte(90), "A")
			.when(Expr::col("score").gte(80), "B")
			.when(Expr::col("score").gte(70), "C")
			.else_result("F");

		let mut stmt = Query::select();
		stmt.expr_as(case_expr, "grade").from("students");

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("CASE"));
		let when_count = sql.matches("WHEN").count();
		assert_eq!(when_count, 3);
		let then_count = sql.matches("THEN").count();
		assert_eq!(then_count, 3);
		assert!(sql.contains("ELSE"));
		assert!(sql.contains("END"));
		assert_eq!(values.len(), 7);
	}

	#[test]
	fn test_case_without_else() {
		let builder = SqliteQueryBuilder::new();

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
		let builder = SqliteQueryBuilder::new();

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
		let builder = SqliteQueryBuilder::new();

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
	fn test_order_by_expression_function() {
		let builder = SqliteQueryBuilder::new();

		let mut stmt = Query::select();
		stmt.column("name")
			.column("length")
			.from("items")
			.order_by_expr(
				Expr::col("name").into_simple_expr(),
				crate::types::Order::Asc,
			);

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains("ORDER BY"));
		assert!(sql.contains(r#""name" ASC"#));
	}

	#[test]
	fn test_large_limit_offset_values() {
		let builder = SqliteQueryBuilder::new();

		let mut stmt = Query::select();
		stmt.column("id")
			.from("big_table")
			.limit(1_000_000)
			.offset(5_000_000);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("LIMIT"));
		assert!(sql.contains("OFFSET"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_multiple_order_by_with_limit_offset() {
		let builder = SqliteQueryBuilder::new();

		let mut stmt = Query::select();
		stmt.column("id")
			.column("name")
			.column("score")
			.from("results")
			.order_by("score", crate::types::Order::Desc)
			.order_by("name", crate::types::Order::Asc)
			.limit(25)
			.offset(50);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains("ORDER BY"));
		assert!(sql.contains(r#""score" DESC"#));
		assert!(sql.contains(r#""name" ASC"#));
		assert!(sql.contains("LIMIT"));
		assert!(sql.contains("OFFSET"));
		assert_eq!(values.len(), 2);
	}

	// Arithmetic / string operation tests

	#[test]
	fn test_arithmetic_in_select() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::col("price").mul(Expr::col("quantity")));
		stmt.from("order_items");

		let (sql, _values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""price" * "quantity""#));
		assert!(sql.contains(r#"FROM "order_items""#));
	}

	#[test]
	fn test_modulo_operator() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("id").from("numbers");
		stmt.and_where(Expr::col("value").modulo(2i32).eq(0i32));

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""value" % ?"#));
		assert!(sql.contains("= ?"));
		assert_eq!(values.len(), 2);
	}

	#[test]
	fn test_complex_arithmetic_where() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("name").from("products");
		stmt.and_where(
			Expr::col("price")
				.mul(Expr::col("quantity"))
				.sub(Expr::col("discount"))
				.gt(500i32),
		);

		let (sql, values) = builder.build_select(&stmt);
		assert!(sql.contains(r#""price" * "quantity""#));
		assert!(sql.contains("- "));
		assert!(sql.contains(r#""discount""#));
		assert!(sql.contains("> ?"));
		assert_eq!(values.len(), 1);
	}

	// DDL Tests

	#[test]
	fn test_drop_table_basic() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users");

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_table_if_exists() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users").if_exists();

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE IF EXISTS \"users\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_table_multiple() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_table();
		stmt.table("users").table("posts");

		let (sql, values) = builder.build_drop_table(&stmt);
		assert_eq!(sql, "DROP TABLE \"users\", \"posts\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_index_basic() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_index();
		stmt.name("idx_email");

		let (sql, values) = builder.build_drop_index(&stmt);
		assert_eq!(sql, "DROP INDEX \"idx_email\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_index_if_exists() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_index();
		stmt.name("idx_email").if_exists();

		let (sql, values) = builder.build_drop_index(&stmt);
		assert_eq!(sql, "DROP INDEX IF EXISTS \"idx_email\"");
		assert_eq!(values.len(), 0);
	}

	// CREATE TABLE tests

	#[test]
	fn test_create_table_basic() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = SqliteQueryBuilder::new();
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
	fn test_create_table_with_autoincrement() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("users");
		stmt.columns.push(ColumnDef {
			name: "id".into_iden(),
			column_type: Some(ColumnType::Integer),
			not_null: false,
			unique: false,
			primary_key: true,
			auto_increment: true,
			default: None,
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		assert!(sql.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_table_with_foreign_key() {
		use crate::types::{
			ColumnDef, ColumnType, ForeignKeyAction, IntoTableRef, TableConstraint,
		};

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
	fn test_create_index_partial_with_where() {
		use crate::query::IndexColumn;

		let builder = SqliteQueryBuilder::new();
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
			r#"CREATE INDEX "idx_users_active_email" ON "users" ("email") WHERE "active" = ?"#
		);
		assert_eq!(values.len(), 1);
	}

	#[test]
	fn test_alter_table_add_column() {
		use crate::query::AlterTableOperation;
		use crate::types::{ColumnDef, ColumnType};

		let builder = SqliteQueryBuilder::new();
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

		let builder = SqliteQueryBuilder::new();
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
	fn test_alter_table_rename_column() {
		use crate::query::AlterTableOperation;

		let builder = SqliteQueryBuilder::new();
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
	fn test_alter_table_rename_table() {
		use crate::query::AlterTableOperation;

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations
			.push(AlterTableOperation::RenameTable("accounts".into_iden()));

		let (sql, values) = builder.build_alter_table(&stmt);
		assert_eq!(sql, r#"ALTER TABLE "users" RENAME TO "accounts""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support MODIFY COLUMN")]
	fn test_alter_table_modify_column_panics() {
		use crate::query::AlterTableOperation;
		use crate::types::{ColumnDef, ColumnType};

		let builder = SqliteQueryBuilder::new();
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

		let _ = builder.build_alter_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support ADD CONSTRAINT")]
	fn test_alter_table_add_constraint_panics() {
		use crate::query::AlterTableOperation;
		use crate::types::TableConstraint;

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::alter_table();
		stmt.table("users");
		stmt.operations.push(AlterTableOperation::AddConstraint(
			TableConstraint::Unique {
				name: Some("unique_email".into_iden()),
				columns: vec!["email".into_iden()],
			},
		));

		let _ = builder.build_alter_table(&stmt);
	}

	#[test]
	fn test_create_table_with_boolean_type() {
		use crate::types::{ColumnDef, ColumnType};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_table();
		stmt.table("settings");
		stmt.columns.push(ColumnDef {
			name: "active".into_iden(),
			column_type: Some(ColumnType::Boolean),
			not_null: false,
			unique: false,
			primary_key: false,
			auto_increment: false,
			default: None,
			check: None,
			comment: None,
		});

		let (sql, values) = builder.build_create_table(&stmt);
		// SQLite uses INTEGER for BOOLEAN
		assert!(sql.contains("\"active\" INTEGER"));
		assert_eq!(values.len(), 0);
	}

	// VIEW tests

	#[test]
	fn test_create_view_basic() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_view();
		stmt.name("user_view".into_iden());
		let mut select = Query::select();
		select.column("id").column("name").from("users");
		stmt.as_select(select);

		let (sql, _values) = builder.build_create_view(&stmt);
		assert!(sql.contains("CREATE VIEW"));
		assert!(sql.contains(r#""user_view""#));
		assert!(sql.contains("AS"));
		assert!(sql.contains(r#"SELECT "id", "name" FROM "users""#));
	}

	#[test]
	fn test_create_view_if_not_exists() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_view();
		stmt.name("user_view".into_iden()).if_not_exists();
		let mut select = Query::select();
		select.column("id").from("users");
		stmt.as_select(select);

		let (sql, _values) = builder.build_create_view(&stmt);
		assert!(sql.contains("CREATE VIEW IF NOT EXISTS"));
		assert!(sql.contains(r#""user_view""#));
	}

	#[test]
	#[should_panic(expected = "SQLite does not support OR REPLACE for CREATE VIEW")]
	fn test_create_view_or_replace_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_view();
		stmt.name("user_view".into_iden()).or_replace();
		let mut select = Query::select();
		select.column("id").from("users");
		stmt.as_select(select);

		let _ = builder.build_create_view(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support MATERIALIZED views")]
	fn test_create_view_materialized_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_view();
		stmt.name("user_view".into_iden()).materialized(true);
		let mut select = Query::select();
		select.column("id").from("users");
		stmt.as_select(select);

		let _ = builder.build_create_view(&stmt);
	}

	#[test]
	fn test_create_view_with_columns() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_view();
		stmt.name("user_view".into_iden())
			.columns(vec!["user_id".into_iden(), "user_name".into_iden()]);
		let mut select = Query::select();
		select.column("id").column("name").from("users");
		stmt.as_select(select);

		let (sql, _values) = builder.build_create_view(&stmt);
		assert!(sql.contains(r#""user_view" ("user_id", "user_name")"#));
	}

	#[test]
	fn test_drop_view_basic() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_view();
		stmt.names(vec!["user_view".into_iden()]);

		let (sql, values) = builder.build_drop_view(&stmt);
		assert_eq!(sql, r#"DROP VIEW "user_view""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_view_if_exists() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_view();
		stmt.names(vec!["user_view".into_iden()]).if_exists();

		let (sql, values) = builder.build_drop_view(&stmt);
		assert_eq!(sql, r#"DROP VIEW IF EXISTS "user_view""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	#[should_panic(expected = "SQLite only supports dropping one view at a time")]
	fn test_drop_view_multiple_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_view();
		stmt.names(vec!["view1".into_iden(), "view2".into_iden()]);

		let _ = builder.build_drop_view(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support CASCADE/RESTRICT for DROP VIEW")]
	fn test_drop_view_cascade_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_view();
		stmt.names(vec!["user_view".into_iden()]).cascade();

		let _ = builder.build_drop_view(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support MATERIALIZED views")]
	fn test_drop_view_materialized_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_view();
		stmt.names(vec!["user_view".into_iden()]).materialized(true);

		let _ = builder.build_drop_view(&stmt);
	}

	// TRUNCATE TABLE tests

	#[test]
	fn test_truncate_table_basic() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users");

		let (sql, values) = builder.build_truncate_table(&stmt);
		assert_eq!(sql, r#"DELETE FROM "users""#);
		assert_eq!(values.len(), 0);
	}

	#[test]
	#[should_panic(expected = "SQLite only supports truncating one table at a time")]
	fn test_truncate_table_multiple_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.tables(vec!["users", "posts"]);

		let _ = builder.build_truncate_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support RESTART IDENTITY for TRUNCATE TABLE")]
	fn test_truncate_table_restart_identity_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").restart_identity();

		let _ = builder.build_truncate_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support CASCADE for TRUNCATE TABLE")]
	fn test_truncate_table_cascade_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").cascade();

		let _ = builder.build_truncate_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support RESTRICT for TRUNCATE TABLE")]
	fn test_truncate_table_restrict_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::truncate_table();
		stmt.table("users").restrict();

		let _ = builder.build_truncate_table(&stmt);
	}

	#[test]
	fn test_create_trigger_basic() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("audit_insert")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Insert)
			.on_table("users")
			.for_each(TriggerScope::Row)
			.body(TriggerBody::single(
				"INSERT INTO audit_log (action) VALUES ('insert')",
			));

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert_eq!(
			sql,
			"CREATE TRIGGER \"audit_insert\" AFTER INSERT ON \"users\" FOR EACH ROW BEGIN INSERT INTO audit_log (action) VALUES ('insert'); END"
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_trigger_instead_of() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("view_insert")
			.timing(TriggerTiming::InsteadOf)
			.event(TriggerEvent::Insert)
			.on_table("user_view")
			.for_each(TriggerScope::Row)
			.body(TriggerBody::single(
				"INSERT INTO users (name) VALUES (NEW.name)",
			));

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert_eq!(
			sql,
			"CREATE TRIGGER \"view_insert\" INSTEAD OF INSERT ON \"user_view\" FOR EACH ROW BEGIN INSERT INTO users (name) VALUES (NEW.name); END"
		);
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_trigger_with_when() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("conditional_update")
			.timing(TriggerTiming::Before)
			.event(TriggerEvent::Update { columns: None })
			.on_table("users")
			.for_each(TriggerScope::Row)
			.when_condition(Expr::col("status").eq("active"))
			.body(TriggerBody::single("SELECT 1"));

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert!(sql.contains("CREATE TRIGGER"));
		assert!(sql.contains("BEFORE UPDATE"));
		assert!(sql.contains("WHEN"));
		assert!(sql.contains("BEGIN"));
		assert!(sql.contains("END"));
		assert_eq!(values.len(), 1); // "active" value
	}

	#[test]
	fn test_create_trigger_update_of_columns() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("status_change")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Update {
				columns: Some(vec!["status".to_string(), "updated_at".to_string()]),
			})
			.on_table("users")
			.for_each(TriggerScope::Row)
			.body(TriggerBody::single("SELECT 1"));

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert!(sql.contains("UPDATE OF"));
		assert!(sql.contains("\"status\""));
		assert!(sql.contains("\"updated_at\""));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_create_trigger_multiple_statements() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("multi_action")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Delete)
			.on_table("users")
			.for_each(TriggerScope::Row)
			.body(TriggerBody::multiple(vec![
				"INSERT INTO deleted_users SELECT *",
				"UPDATE stats SET count = count - 1",
			]));

		let (sql, values) = builder.build_create_trigger(&stmt);
		assert!(sql.contains("BEGIN"));
		assert!(sql.contains("INSERT INTO deleted_users SELECT *;"));
		assert!(sql.contains("UPDATE stats SET count = count - 1;"));
		assert!(sql.contains("END"));
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_trigger_basic() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_trigger();
		stmt.name("audit_insert");

		let (sql, values) = builder.build_drop_trigger(&stmt);
		assert_eq!(sql, "DROP TRIGGER \"audit_insert\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	fn test_drop_trigger_if_exists() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_trigger();
		stmt.name("audit_insert").if_exists();

		let (sql, values) = builder.build_drop_trigger(&stmt);
		assert_eq!(sql, "DROP TRIGGER IF EXISTS \"audit_insert\"");
		assert_eq!(values.len(), 0);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support multiple events in a single trigger")]
	fn test_create_trigger_multiple_events_panics() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("multi_event")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Insert)
			.event(TriggerEvent::Update { columns: None })
			.on_table("users")
			.for_each(TriggerScope::Row)
			.body(TriggerBody::single("SELECT 1"));

		let _ = builder.build_create_trigger(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite only supports FOR EACH ROW triggers")]
	fn test_create_trigger_for_each_statement_panics() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("statement_trigger")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Insert)
			.on_table("users")
			.for_each(TriggerScope::Statement)
			.body(TriggerBody::single("SELECT 1"));

		let _ = builder.build_create_trigger(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support FOLLOWS/PRECEDES syntax")]
	fn test_create_trigger_follows_panics() {
		use crate::types::{TriggerBody, TriggerEvent, TriggerOrder, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("ordered_trigger")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Insert)
			.on_table("users")
			.for_each(TriggerScope::Row)
			.order(TriggerOrder::Follows("other_trigger".to_string()))
			.body(TriggerBody::single("SELECT 1"));

		let _ = builder.build_create_trigger(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support EXECUTE FUNCTION syntax")]
	fn test_create_trigger_postgres_function_panics() {
		use crate::types::{TriggerEvent, TriggerScope, TriggerTiming};

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_trigger();
		stmt.name("function_trigger")
			.timing(TriggerTiming::After)
			.event(TriggerEvent::Insert)
			.on_table("users")
			.for_each(TriggerScope::Row)
			.execute_function("audit_function");

		let _ = builder.build_create_trigger(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support CASCADE/RESTRICT for DROP TRIGGER")]
	fn test_drop_trigger_cascade_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_trigger();
		stmt.name("audit_insert").cascade();

		let _ = builder.build_drop_trigger(&stmt);
	}

	// FUNCTION tests - SQLite does not support DDL for functions
	#[test]
	#[should_panic(expected = "SQLite does not support CREATE FUNCTION")]
	fn test_create_function_panics() {
		use crate::types::function::FunctionLanguage;

		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_function();
		stmt.name("my_func")
			.returns("INTEGER")
			.language(FunctionLanguage::Sql)
			.body("SELECT 1");

		let _ = builder.build_create_function(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support ALTER FUNCTION")]
	fn test_alter_function_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::alter_function();
		stmt.name("my_func").rename_to("new_func");

		let _ = builder.build_alter_function(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support DROP FUNCTION")]
	fn test_drop_function_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_function();
		stmt.name("my_func");

		let _ = builder.build_drop_function(&stmt);
	}

	// TYPE tests - SQLite does not support custom types
	#[test]
	#[should_panic(expected = "CREATE TYPE not supported.")]
	fn test_create_type_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::create_type();
		stmt.name("mood")
			.as_enum(vec!["happy".to_string(), "sad".to_string()]);

		let _ = builder.build_create_type(&stmt);
	}

	#[test]
	#[should_panic(expected = "ALTER TYPE not supported.")]
	fn test_alter_type_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::alter_type();
		stmt.name("mood").rename_to("feeling");

		let _ = builder.build_alter_type(&stmt);
	}

	#[test]
	#[should_panic(expected = "DROP TYPE not supported.")]
	fn test_drop_type_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::drop_type();
		stmt.name("mood");

		let _ = builder.build_drop_type(&stmt);
	}

	// MySQL-specific maintenance command panic tests
	#[test]
	#[should_panic(expected = "SQLite users should use VACUUM or ANALYZE")]
	fn test_optimize_table_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::optimize_table();
		stmt.table("users");

		let _ = builder.build_optimize_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "not supported in SQLite")]
	fn test_repair_table_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::repair_table();
		stmt.table("users");

		let _ = builder.build_repair_table(&stmt);
	}

	#[test]
	#[should_panic(expected = "not supported in SQLite")]
	fn test_check_table_panics() {
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::check_table();
		stmt.table("users");

		let _ = builder.build_check_table(&stmt);
	}

	// DCL (Data Control Language) Tests
	// SQLite does not support DCL - these tests verify panic behavior

	#[test]
	#[should_panic(expected = "SQLite does not support DCL")]
	fn test_grant_panics() {
		use crate::dcl::{GrantStatement, Privilege};

		let builder = SqliteQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.to("app_user");

		// Should panic with "SQLite does not support DCL" message
		let _ = builder.build_grant(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support DCL")]
	fn test_revoke_panics() {
		use crate::dcl::{Privilege, RevokeStatement};

		let builder = SqliteQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Insert)
			.from_table("users")
			.from("app_user");

		// Should panic with "SQLite does not support DCL" message
		let _ = builder.build_revoke(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support CREATE ROLE statement")]
	fn test_create_role_panics() {
		use crate::dcl::CreateRoleStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = CreateRoleStatement::new().role("developer");

		// Should panic with "SQLite does not support CREATE ROLE statement" message
		let _ = builder.build_create_role(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support DROP ROLE statement")]
	fn test_drop_role_panics() {
		use crate::dcl::DropRoleStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = DropRoleStatement::new().role("old_role");

		// Should panic with "SQLite does not support DROP ROLE statement" message
		let _ = builder.build_drop_role(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support ALTER ROLE statement")]
	fn test_alter_role_panics() {
		use crate::dcl::AlterRoleStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = AlterRoleStatement::new().role("developer");

		// Should panic with "SQLite does not support ALTER ROLE statement" message
		let _ = builder.build_alter_role(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support CREATE USER statement")]
	fn test_create_user_panics() {
		use crate::dcl::CreateUserStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = CreateUserStatement::new().user("app_user");

		let _ = builder.build_create_user(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support DROP USER statement")]
	fn test_drop_user_panics() {
		use crate::dcl::DropUserStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = DropUserStatement::new().user("app_user");

		let _ = builder.build_drop_user(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support ALTER USER statement")]
	fn test_alter_user_panics() {
		use crate::dcl::AlterUserStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = AlterUserStatement::new().user("app_user");

		let _ = builder.build_alter_user(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support RENAME USER statement")]
	fn test_rename_user_panics() {
		use crate::dcl::RenameUserStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = RenameUserStatement::new().rename("old", "new");

		let _ = builder.build_rename_user(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support SET ROLE statement")]
	fn test_set_role_panics() {
		use crate::dcl::{RoleTarget, SetRoleStatement};

		let builder = SqliteQueryBuilder::new();
		let stmt = SetRoleStatement::new().role(RoleTarget::Named("admin".to_string()));

		let _ = builder.build_set_role(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support RESET ROLE statement")]
	fn test_reset_role_panics() {
		use crate::dcl::ResetRoleStatement;

		let builder = SqliteQueryBuilder::new();
		let stmt = ResetRoleStatement::new();

		let _ = builder.build_reset_role(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support SET DEFAULT ROLE statement")]
	fn test_set_default_role_panics() {
		use crate::dcl::{DefaultRoleSpec, SetDefaultRoleStatement};

		let builder = SqliteQueryBuilder::new();
		let stmt = SetDefaultRoleStatement::new()
			.roles(DefaultRoleSpec::All)
			.user("app_user");

		let _ = builder.build_set_default_role(&stmt);
	}

	// ==================== SimpleExpr variant handling tests ====================

	#[rstest]
	fn test_table_column_expr_renders_qualified_identifier() {
		// Arrange
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::tbl("users", "name")).from("users");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert
		assert_eq!(sql, "SELECT \"users\".\"name\" FROM \"users\"");
	}

	#[rstest]
	fn test_table_column_expr_escapes_special_characters() {
		// Arrange
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::tbl(Alias::new("my\"table"), Alias::new("my\"col")))
			.from("t");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert
		assert!(sql.contains("\"my\"\"table\".\"my\"\"col\""));
	}

	#[rstest]
	fn test_as_enum_expr_renders_inner_expression_only() {
		// Arrange
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::val("active").as_enum(Alias::new("status")))
			.from("users");

		// Act
		let (sql, values) = builder.build_select(&stmt);

		// Assert: SQLite does not support ::type casting, only inner expression is rendered
		assert_eq!(sql, "SELECT ? FROM \"users\"");
		assert_eq!(values.len(), 1);
	}

	#[rstest]
	fn test_cast_expr_renders_cast_syntax() {
		// Arrange
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::col("age").cast_as(Alias::new("INTEGER")))
			.from("users");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert
		assert_eq!(sql, "SELECT CAST(\"age\" AS \"INTEGER\") FROM \"users\"");
	}

	#[rstest]
	fn test_cast_expr_escapes_type_name_with_special_characters() {
		// Arrange
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.expr(Expr::col("age").cast_as(Alias::new("my\"type")))
			.from("users");

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert
		assert!(sql.contains("CAST(\"age\" AS \"my\"\"type\")"));
	}

	#[rstest]
	fn test_table_column_in_where_clause() {
		// Arrange
		let builder = SqliteQueryBuilder::new();
		let mut stmt = Query::select();
		stmt.column("id")
			.from("orders")
			.and_where(Expr::tbl("orders", "status").eq("shipped"));

		// Act
		let (sql, _) = builder.build_select(&stmt);

		// Assert
		assert!(sql.contains("\"orders\".\"status\""));
		assert!(sql.contains("WHERE"));
	}
}

impl crate::query::QueryBuilderTrait for SqliteQueryBuilder {
	fn placeholder(&self) -> (&str, bool) {
		("?", false)
	}

	fn quote_char(&self) -> char {
		'"'
	}
}
