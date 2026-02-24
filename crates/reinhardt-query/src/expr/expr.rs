//! Expr - The expression builder.
//!
//! This module provides [`Expr`], a builder for creating SQL expressions.
//! It provides a fluent API for building complex expressions.

use super::simple_expr::{CaseStatement, Keyword, SimpleExpr};
use crate::types::{ColumnRef, DynIden, IntoColumnRef, IntoIden, WindowStatement};
use crate::value::{IntoValue, Value};

/// Expression builder for creating SQL expressions.
///
/// `Expr` provides static methods to create expressions and instance methods
/// to chain operations on them.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::Expr;
///
/// // Simple column reference
/// let col = Expr::col("name");
///
/// // Value expression
/// let val = Expr::val(42);
///
/// // Build complex expressions
/// let expr = Expr::col("age").gte(18).and(Expr::col("active").eq(true));
/// ```
#[derive(Debug, Clone)]
pub struct Expr(SimpleExpr);

impl Expr {
	/// Create an expression from a column reference.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// let expr = Expr::col("name");
	/// ```
	pub fn col<C>(col: C) -> Self
	where
		C: IntoColumnRef,
	{
		Self(SimpleExpr::Column(col.into_column_ref()))
	}

	/// Create a table-qualified column expression.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// let expr = Expr::tbl("users", "name");
	/// ```
	pub fn tbl<T, C>(table: T, col: C) -> Self
	where
		T: IntoIden,
		C: IntoIden,
	{
		Self(SimpleExpr::TableColumn(table.into_iden(), col.into_iden()))
	}

	/// Create a value expression.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// let expr = Expr::val(42);
	/// let expr2 = Expr::val("hello");
	/// ```
	pub fn val<V>(val: V) -> Self
	where
		V: IntoValue,
	{
		Self(SimpleExpr::Value(val.into_value()))
	}

	/// Create a custom SQL expression.
	///
	/// # Security Warning
	///
	/// **DO NOT** pass user input directly to this method. This method embeds the
	/// SQL string directly into the query without parameterization, which can lead
	/// to SQL injection vulnerabilities.
	///
	/// Use [`Expr::cust_with_values()`] for dynamic values instead.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// // ✅ SAFE: Static SQL expression
	/// let expr = Expr::cust("NOW()");
	///
	/// // ❌ UNSAFE: User input
	/// // let expr = Expr::cust(&user_input);
	///
	/// // ✅ SAFE: Parameterized custom expression
	/// // let expr = Expr::cust_with_values("? + ?", [user_input, other_value]);
	/// ```
	pub fn cust<S>(sql: S) -> Self
	where
		S: Into<String>,
	{
		Self(SimpleExpr::Custom(sql.into()))
	}

	/// Create a custom SQL expression with value placeholders.
	///
	/// Use `?` as placeholders for the values.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// let expr = Expr::cust_with_values("? + ?", [1, 2]);
	/// ```
	pub fn cust_with_values<S, I, V>(sql: S, values: I) -> Self
	where
		S: Into<String>,
		I: IntoIterator<Item = V>,
		V: Into<SimpleExpr>,
	{
		Self(SimpleExpr::CustomWithExpr(
			sql.into(),
			values.into_iter().map(|v| v.into()).collect(),
		))
	}

	/// Create a tuple expression.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// let expr = Expr::tuple([Expr::val(1), Expr::val(2), Expr::val(3)]);
	/// ```
	pub fn tuple<I>(exprs: I) -> Self
	where
		I: IntoIterator<Item = Self>,
	{
		Self(SimpleExpr::Tuple(
			exprs.into_iter().map(|e| e.into_simple_expr()).collect(),
		))
	}

	/// Create an asterisk expression (`*`).
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// let expr = Expr::asterisk();
	/// ```
	pub fn asterisk() -> Self {
		Self(SimpleExpr::Asterisk)
	}

	/// Create a subquery expression.
	///
	/// This creates a standalone subquery that can be used in FROM or SELECT clauses.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let subquery = Query::select().column("id").from("users");
	/// let expr = Expr::subquery(subquery);
	/// ```
	pub fn subquery(select: crate::query::SelectStatement) -> Self {
		Self(SimpleExpr::SubQuery(None, Box::new(select)))
	}

	/// Create an EXISTS subquery expression.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let subquery = Query::select().column("id").from("orders").and_where(
	///     Expr::col(("orders", "user_id")).eq(Expr::col(("users", "id")))
	/// );
	/// let exists = Expr::exists(subquery);
	/// ```
	pub fn exists(select: crate::query::SelectStatement) -> Self {
		Self(SimpleExpr::SubQuery(
			Some(super::simple_expr::SubQueryOper::Exists),
			Box::new(select),
		))
	}

	/// Create a NOT EXISTS subquery expression.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let subquery = Query::select().column("id").from("banned_users").and_where(
	///     Expr::col(("banned_users", "id")).eq(Expr::col(("users", "id")))
	/// );
	/// let not_exists = Expr::not_exists(subquery);
	/// ```
	pub fn not_exists(select: crate::query::SelectStatement) -> Self {
		Self(SimpleExpr::SubQuery(
			Some(super::simple_expr::SubQueryOper::NotExists),
			Box::new(select),
		))
	}

	/// Create an IN subquery expression.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let subquery = Query::select().column("user_id").from("premium_users");
	/// let in_expr = Expr::col("id").in_subquery(subquery);
	/// ```
	pub fn in_subquery(self, select: crate::query::SelectStatement) -> Self {
		Self(SimpleExpr::Binary(
			Box::new(self.0),
			crate::types::BinOper::In,
			Box::new(SimpleExpr::SubQuery(None, Box::new(select))),
		))
	}

	/// Create a NOT IN subquery expression.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let subquery = Query::select().column("user_id").from("banned_users");
	/// let not_in_expr = Expr::col("id").not_in_subquery(subquery);
	/// ```
	pub fn not_in_subquery(self, select: crate::query::SelectStatement) -> Self {
		Self(SimpleExpr::Binary(
			Box::new(self.0),
			crate::types::BinOper::NotIn,
			Box::new(SimpleExpr::SubQuery(None, Box::new(select))),
		))
	}

	/// Create a CASE expression.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::{Expr, ExprTrait};
	///
	/// let case = Expr::case()
	///     .when(Expr::col("status").eq("active"), "Active")
	///     .when(Expr::col("status").eq("pending"), "Pending")
	///     .else_result("Unknown");
	/// ```
	pub fn case() -> CaseExprBuilder {
		CaseExprBuilder {
			case: CaseStatement::new(),
		}
	}

	/// Create a value expression (alias for [`Expr::val`]).
	pub fn value<V>(val: V) -> Self
	where
		V: IntoValue,
	{
		Self::val(val)
	}

	/// Create an expression from a DynIden column.
	pub fn expr_col(col: DynIden) -> Self {
		Self(SimpleExpr::Column(ColumnRef::Column(col)))
	}

	// Constant expressions

	/// Create a NULL constant expression.
	pub fn null() -> Self {
		Self(SimpleExpr::Constant(Keyword::Null))
	}

	/// Create a TRUE constant expression.
	pub fn constant_true() -> Self {
		Self(SimpleExpr::Constant(Keyword::True))
	}

	/// Create a FALSE constant expression.
	pub fn constant_false() -> Self {
		Self(SimpleExpr::Constant(Keyword::False))
	}

	/// Create a DEFAULT constant expression.
	// Intentional factory method for SQL DEFAULT keyword, not std::default::Default
	#[allow(clippy::should_implement_trait)]
	pub fn default() -> Self {
		Self(SimpleExpr::Constant(Keyword::Default))
	}

	/// Create a CURRENT_TIMESTAMP expression.
	pub fn current_timestamp() -> Self {
		Self(SimpleExpr::Constant(Keyword::CurrentTimestamp))
	}

	/// Create a CURRENT_DATE expression.
	pub fn current_date() -> Self {
		Self(SimpleExpr::Constant(Keyword::CurrentDate))
	}

	/// Create a CURRENT_TIME expression.
	pub fn current_time() -> Self {
		Self(SimpleExpr::Constant(Keyword::CurrentTime))
	}

	// Window function methods

	/// Apply a window specification to this expression.
	///
	/// This creates a window function call with an inline window specification.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![Expr::col("department_id").into_simple_expr()],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// let expr = Expr::row_number().over(window);
	/// ```
	#[must_use]
	pub fn over(self, window: WindowStatement) -> SimpleExpr {
		SimpleExpr::Window {
			func: Box::new(self.0),
			window,
		}
	}

	/// Apply a named window to this expression.
	///
	/// This creates a window function call that references a named window
	/// defined in the WINDOW clause.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let expr = Expr::row_number().over_named("w");
	/// ```
	#[must_use]
	pub fn over_named<T: IntoIden>(self, name: T) -> SimpleExpr {
		SimpleExpr::WindowNamed {
			func: Box::new(self.0),
			name: name.into_iden(),
		}
	}

	/// Create a ROW_NUMBER() window function.
	///
	/// Returns a sequential number for each row within a partition.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// let expr = Expr::row_number().over(window);
	/// ```
	pub fn row_number() -> Self {
		Self(SimpleExpr::FunctionCall(
			"ROW_NUMBER".into_iden(),
			Vec::new(),
		))
	}

	/// Create a RANK() window function.
	///
	/// Returns the rank of each row within a partition, with gaps in ranking
	/// for tied values.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// let expr = Expr::rank().over(window);
	/// ```
	pub fn rank() -> Self {
		Self(SimpleExpr::FunctionCall("RANK".into_iden(), Vec::new()))
	}

	/// Create a DENSE_RANK() window function.
	///
	/// Returns the rank of each row within a partition, without gaps in ranking
	/// for tied values.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// let expr = Expr::dense_rank().over(window);
	/// ```
	pub fn dense_rank() -> Self {
		Self(SimpleExpr::FunctionCall(
			"DENSE_RANK".into_iden(),
			Vec::new(),
		))
	}

	/// Create an NTILE(n) window function.
	///
	/// Divides the rows in a partition into `buckets` number of groups.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// let expr = Expr::ntile(4).over(window); // Divide into quartiles
	/// ```
	pub fn ntile(buckets: i64) -> Self {
		Self(SimpleExpr::FunctionCall(
			"NTILE".into_iden(),
			vec![SimpleExpr::Value(Value::BigInt(Some(buckets)))],
		))
	}

	/// Create a LEAD() window function.
	///
	/// Returns the value of the expression evaluated at the row that is `offset`
	/// rows after the current row within the partition.
	///
	/// # Arguments
	///
	/// * `expr` - The expression to evaluate
	/// * `offset` - Number of rows after the current row (default is 1 if `None`)
	/// * `default` - Default value if the lead row doesn't exist (default is NULL if `None`)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// // Get next salary value
	/// let expr = Expr::lead(Expr::col("salary").into_simple_expr(), Some(1), None).over(window);
	/// ```
	pub fn lead(expr: SimpleExpr, offset: Option<i64>, default: Option<Value>) -> Self {
		let mut args = vec![expr];
		if let Some(off) = offset {
			args.push(SimpleExpr::Value(Value::BigInt(Some(off))));
			if let Some(def) = default {
				args.push(SimpleExpr::Value(def));
			}
		}
		Self(SimpleExpr::FunctionCall("LEAD".into_iden(), args))
	}

	/// Create a LAG() window function.
	///
	/// Returns the value of the expression evaluated at the row that is `offset`
	/// rows before the current row within the partition.
	///
	/// # Arguments
	///
	/// * `expr` - The expression to evaluate
	/// * `offset` - Number of rows before the current row (default is 1 if `None`)
	/// * `default` - Default value if the lag row doesn't exist (default is NULL if `None`)
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// // Get previous salary value
	/// let expr = Expr::lag(Expr::col("salary").into_simple_expr(), Some(1), None).over(window);
	/// ```
	pub fn lag(expr: SimpleExpr, offset: Option<i64>, default: Option<Value>) -> Self {
		let mut args = vec![expr];
		if let Some(off) = offset {
			args.push(SimpleExpr::Value(Value::BigInt(Some(off))));
			if let Some(def) = default {
				args.push(SimpleExpr::Value(def));
			}
		}
		Self(SimpleExpr::FunctionCall("LAG".into_iden(), args))
	}

	/// Create a FIRST_VALUE() window function.
	///
	/// Returns the first value in a window frame.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// let expr = Expr::first_value(Expr::col("salary").into_simple_expr()).over(window);
	/// ```
	pub fn first_value(expr: SimpleExpr) -> Self {
		Self(SimpleExpr::FunctionCall(
			"FIRST_VALUE".into_iden(),
			vec![expr],
		))
	}

	/// Create a LAST_VALUE() window function.
	///
	/// Returns the last value in a window frame.
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// let expr = Expr::last_value(Expr::col("salary").into_simple_expr()).over(window);
	/// ```
	pub fn last_value(expr: SimpleExpr) -> Self {
		Self(SimpleExpr::FunctionCall(
			"LAST_VALUE".into_iden(),
			vec![expr],
		))
	}

	/// Create an NTH_VALUE() window function.
	///
	/// Returns the value of the expression at the nth row of the window frame.
	///
	/// # Arguments
	///
	/// * `expr` - The expression to evaluate
	/// * `n` - The row number (1-based) within the frame
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::window::WindowStatement;
	///
	/// let window = WindowStatement {
	///     partition_by: vec![],
	///     order_by: vec![],
	///     frame: None,
	/// };
	///
	/// // Get the 3rd salary value in the frame
	/// let expr = Expr::nth_value(Expr::col("salary").into_simple_expr(), 3).over(window);
	/// ```
	pub fn nth_value(expr: SimpleExpr, n: i64) -> Self {
		Self(SimpleExpr::FunctionCall(
			"NTH_VALUE".into_iden(),
			vec![expr, SimpleExpr::Value(Value::BigInt(Some(n)))],
		))
	}

	// Conversion methods

	/// Convert this Expr into a SimpleExpr.
	#[must_use]
	pub fn into_simple_expr(self) -> SimpleExpr {
		self.0
	}

	/// Get a reference to the underlying SimpleExpr.
	#[must_use]
	pub fn as_simple_expr(&self) -> &SimpleExpr {
		&self.0
	}

	/// Create a binary operation expression.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let expr = Expr::col("age").binary(BinOper::GreaterThan, Expr::val(18).into_simple_expr());
	/// ```
	pub fn binary<R>(self, op: crate::types::BinOper, right: R) -> SimpleExpr
	where
		R: Into<SimpleExpr>,
	{
		SimpleExpr::Binary(Box::new(self.0), op, Box::new(right.into()))
	}

	/// Create an equality expression between two columns.
	///
	/// This is equivalent to `self.eq(Expr::col(col))`.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let expr = Expr::col(("orders", "user_id")).equals(("users", "id"));
	/// ```
	pub fn equals<C>(self, col: C) -> SimpleExpr
	where
		C: IntoColumnRef,
	{
		SimpleExpr::Binary(
			Box::new(self.0),
			crate::types::BinOper::Equal,
			Box::new(SimpleExpr::Column(col.into_column_ref())),
		)
	}

	/// Create an aliased expression (AS).
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::expr::Expr;
	///
	/// let expr = Expr::col("name").expr_as("alias_name");
	/// ```
	#[must_use]
	pub fn expr_as<T: IntoIden>(self, alias: T) -> SimpleExpr {
		SimpleExpr::AsEnum(alias.into_iden(), Box::new(self.0))
	}
}

// Allow Expr to be converted into SimpleExpr
impl From<Expr> for SimpleExpr {
	fn from(e: Expr) -> Self {
		e.0
	}
}

// Allow creating Expr from SimpleExpr
impl From<SimpleExpr> for Expr {
	fn from(e: SimpleExpr) -> Self {
		Self(e)
	}
}

impl From<&str> for Expr {
	fn from(s: &str) -> Self {
		Expr::val(s)
	}
}

impl crate::value::IntoValue for Expr {
	fn into_value(self) -> crate::value::Value {
		match self.0 {
			SimpleExpr::Value(v) => v,
			_ => panic!("Cannot convert non-value Expr to Value"),
		}
	}
}

/// Builder for CASE expressions.
#[derive(Debug, Clone)]
pub struct CaseExprBuilder {
	case: CaseStatement,
}

impl CaseExprBuilder {
	/// Add a WHEN clause.
	#[must_use]
	pub fn when<C, R>(mut self, condition: C, result: R) -> Self
	where
		C: Into<SimpleExpr>,
		R: Into<SimpleExpr>,
	{
		self.case = self.case.when(condition, result);
		self
	}

	/// Set the ELSE clause and return the built Expr.
	#[must_use]
	pub fn else_result<E>(mut self, result: E) -> Expr
	where
		E: Into<SimpleExpr>,
	{
		self.case = self.case.else_result(result);
		Expr(SimpleExpr::Case(Box::new(self.case)))
	}

	/// Build the CASE expression without an ELSE clause.
	#[must_use]
	pub fn build(self) -> Expr {
		Expr(SimpleExpr::Case(Box::new(self.case)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::value::Value;
	use rstest::rstest;

	#[rstest]
	fn test_expr_col() {
		let expr = Expr::col("name");
		assert!(matches!(expr.0, SimpleExpr::Column(_)));
	}

	#[rstest]
	fn test_expr_tbl() {
		let expr = Expr::tbl("users", "name");
		assert!(matches!(expr.0, SimpleExpr::TableColumn(_, _)));
	}

	#[rstest]
	fn test_expr_val() {
		let expr = Expr::val(42);
		assert!(matches!(expr.0, SimpleExpr::Value(Value::Int(Some(42)))));
	}

	#[rstest]
	fn test_expr_cust() {
		let expr = Expr::cust("NOW()");
		if let SimpleExpr::Custom(s) = expr.0 {
			assert_eq!(s, "NOW()");
		} else {
			panic!("Expected Custom variant");
		}
	}

	#[rstest]
	fn test_expr_cust_with_values() {
		let expr = Expr::cust_with_values("? + ?", [1i32, 2i32]);
		assert!(matches!(expr.0, SimpleExpr::CustomWithExpr(_, _)));
	}

	#[rstest]
	fn test_expr_tuple() {
		let expr = Expr::tuple([Expr::val(1), Expr::val(2), Expr::val(3)]);
		if let SimpleExpr::Tuple(v) = expr.0 {
			assert_eq!(v.len(), 3);
		} else {
			panic!("Expected Tuple variant");
		}
	}

	#[rstest]
	fn test_expr_asterisk() {
		let expr = Expr::asterisk();
		assert!(matches!(expr.0, SimpleExpr::Asterisk));
	}

	#[rstest]
	fn test_expr_null() {
		let expr = Expr::null();
		assert!(matches!(expr.0, SimpleExpr::Constant(Keyword::Null)));
	}

	#[rstest]
	fn test_expr_current_timestamp() {
		let expr = Expr::current_timestamp();
		assert!(matches!(
			expr.0,
			SimpleExpr::Constant(Keyword::CurrentTimestamp)
		));
	}

	#[rstest]
	fn test_case_expr_builder() {
		let expr = Expr::case()
			.when(true, 1i32)
			.when(false, 0i32)
			.else_result(-1i32);

		assert!(matches!(expr.0, SimpleExpr::Case(_)));
	}

	#[rstest]
	fn test_case_expr_without_else() {
		let expr = Expr::case().when(true, 1i32).build();

		assert!(matches!(expr.0, SimpleExpr::Case(_)));
	}

	#[rstest]
	fn test_expr_into_simple_expr() {
		let expr = Expr::val(42);
		let simple = expr.into_simple_expr();
		assert!(matches!(simple, SimpleExpr::Value(Value::Int(Some(42)))));
	}
}
