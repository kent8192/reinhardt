//! SimpleExpr - The core expression AST.
//!
//! This module defines [`SimpleExpr`], which represents SQL expressions as an
//! abstract syntax tree (AST). All expression operations eventually produce
//! a `SimpleExpr`.

use crate::types::{BinOper, ColumnRef, DynIden, UnOper, WindowStatement};
use crate::value::Value;

/// Subquery operators used in SQL expressions
///
/// These operators are used to combine subqueries with other expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubQueryOper {
	/// EXISTS (subquery)
	Exists,
	/// NOT EXISTS (subquery)
	NotExists,
	/// IN (subquery)
	In,
	/// NOT IN (subquery)
	NotIn,
	/// ALL (subquery) - used with comparison operators
	All,
	/// ANY (subquery) - used with comparison operators
	Any,
	/// SOME (subquery) - alias for ANY
	Some,
}

/// A simple SQL expression.
///
/// This enum represents the AST for SQL expressions. Each variant corresponds
/// to a type of SQL expression that can appear in queries.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::SimpleExpr;
///
/// // Column reference
/// let col = SimpleExpr::Column(ColumnRef::column("name"));
///
/// // Value literal
/// let val = SimpleExpr::Value(Value::Int(Some(42)));
///
/// // Binary operation (column = 42)
/// let eq = SimpleExpr::Binary(
///     Box::new(col),
///     BinOper::Equal,
///     Box::new(val),
/// );
/// ```
#[derive(Debug, Clone)]
pub enum SimpleExpr {
	/// A column reference (e.g., `name`, `users.name`, `public.users.name`)
	Column(ColumnRef),

	/// A table-qualified column (legacy format)
	TableColumn(DynIden, DynIden),

	/// A literal value (e.g., `42`, `'hello'`, `TRUE`)
	Value(Value),

	/// A unary operation (e.g., `NOT x`)
	Unary(UnOper, Box<SimpleExpr>),

	/// A binary operation (e.g., `x = y`, `a AND b`, `x + y`)
	Binary(Box<SimpleExpr>, BinOper, Box<SimpleExpr>),

	/// A function call (e.g., `MAX(x)`, `LOWER(name)`)
	FunctionCall(DynIden, Vec<SimpleExpr>),

	/// A subquery (e.g., `(SELECT ...)`)
	///
	/// The optional operator indicates how the subquery is used:
	/// - `None`: Standalone subquery (e.g., in FROM clause or SELECT list)
	/// - `Some(SubQueryOper)`: Subquery with operator (e.g., IN, EXISTS, ALL)
	SubQuery(Option<SubQueryOper>, Box<crate::query::SelectStatement>),

	/// A tuple of expressions (e.g., `(1, 2, 3)`)
	Tuple(Vec<SimpleExpr>),

	/// A custom SQL expression (e.g., `NOW()`)
	///
	/// # Security Warning
	///
	/// This variant embeds raw SQL directly into the query. Only use with trusted
	/// input or static SQL strings. For dynamic values, use `CustomWithExpr` instead.
	Custom(String),

	/// A custom SQL expression with parameter placeholders
	CustomWithExpr(String, Vec<SimpleExpr>),

	/// A constant (database-specific constant like `TRUE`, `FALSE`, `NULL`)
	Constant(Keyword),

	/// An asterisk (`*`)
	Asterisk,

	/// A CASE WHEN expression
	Case(Box<CaseStatement>),

	/// An AS expression with alias (e.g., `expr AS alias`)
	AsEnum(DynIden, Box<SimpleExpr>),

	/// A CAST expression (e.g., `CAST(x AS INTEGER)`)
	Cast(Box<SimpleExpr>, DynIden),

	/// A window function with inline window specification
	///
	/// Represents expressions like:
	/// ```sql
	/// ROW_NUMBER() OVER (PARTITION BY department_id ORDER BY salary DESC)
	/// ```
	Window {
		/// The function expression
		func: Box<SimpleExpr>,
		/// The window specification
		window: WindowStatement,
	},

	/// A window function with named window reference
	///
	/// Represents expressions like:
	/// ```sql
	/// ROW_NUMBER() OVER window_name
	/// ```
	/// where `window_name` is defined in the WINDOW clause.
	WindowNamed {
		/// The function expression
		func: Box<SimpleExpr>,
		/// The window name
		name: DynIden,
	},
}

/// SQL keywords that can appear as constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
	/// SQL NULL
	Null,
	/// SQL TRUE
	True,
	/// SQL FALSE
	False,
	/// SQL DEFAULT
	Default,
	/// SQL CURRENT_TIMESTAMP
	CurrentTimestamp,
	/// SQL CURRENT_DATE
	CurrentDate,
	/// SQL CURRENT_TIME
	CurrentTime,
}

impl Keyword {
	/// Returns the SQL representation of this keyword.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Null => "NULL",
			Self::True => "TRUE",
			Self::False => "FALSE",
			Self::Default => "DEFAULT",
			Self::CurrentTimestamp => "CURRENT_TIMESTAMP",
			Self::CurrentDate => "CURRENT_DATE",
			Self::CurrentTime => "CURRENT_TIME",
		}
	}
}

/// A CASE WHEN statement.
///
/// Represents SQL CASE expressions:
/// ```sql
/// CASE
///     WHEN condition1 THEN result1
///     WHEN condition2 THEN result2
///     ELSE default_result
/// END
/// ```
#[derive(Debug, Clone, Default)]
pub struct CaseStatement {
	/// The WHEN conditions and their THEN results
	pub when_clauses: Vec<(SimpleExpr, SimpleExpr)>,
	/// The ELSE result (optional)
	pub else_clause: Option<SimpleExpr>,
}

impl CaseStatement {
	/// Create a new empty CASE statement.
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a WHEN clause.
	#[must_use]
	pub fn when<C, R>(mut self, condition: C, result: R) -> Self
	where
		C: Into<SimpleExpr>,
		R: Into<SimpleExpr>,
	{
		self.when_clauses.push((condition.into(), result.into()));
		self
	}

	/// Set the ELSE clause.
	#[must_use]
	pub fn else_result<E>(mut self, result: E) -> Self
	where
		E: Into<SimpleExpr>,
	{
		self.else_clause = Some(result.into());
		self
	}
}

impl SimpleExpr {
	/// Create a binary operation expression.
	pub fn binary(self, op: crate::types::BinOper, right: SimpleExpr) -> Self {
		Self::Binary(Box::new(self), op, Box::new(right))
	}
}

// Conversion implementations

impl From<Value> for SimpleExpr {
	fn from(v: Value) -> Self {
		Self::Value(v)
	}
}

impl From<ColumnRef> for SimpleExpr {
	fn from(c: ColumnRef) -> Self {
		Self::Column(c)
	}
}

impl From<bool> for SimpleExpr {
	fn from(b: bool) -> Self {
		Self::Value(Value::Bool(Some(b)))
	}
}

impl From<i32> for SimpleExpr {
	fn from(i: i32) -> Self {
		Self::Value(Value::Int(Some(i)))
	}
}

impl From<i64> for SimpleExpr {
	fn from(i: i64) -> Self {
		Self::Value(Value::BigInt(Some(i)))
	}
}

impl From<f32> for SimpleExpr {
	fn from(f: f32) -> Self {
		Self::Value(Value::Float(Some(f)))
	}
}

impl From<f64> for SimpleExpr {
	fn from(f: f64) -> Self {
		Self::Value(Value::Double(Some(f)))
	}
}

impl From<&str> for SimpleExpr {
	fn from(s: &str) -> Self {
		Self::Value(Value::String(Some(Box::new(s.to_string()))))
	}
}

impl From<String> for SimpleExpr {
	fn from(s: String) -> Self {
		Self::Value(Value::String(Some(Box::new(s))))
	}
}

impl From<Keyword> for SimpleExpr {
	fn from(k: Keyword) -> Self {
		Self::Constant(k)
	}
}

impl From<CaseStatement> for SimpleExpr {
	fn from(case: CaseStatement) -> Self {
		Self::Case(Box::new(case))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_simple_expr_from_value() {
		let expr: SimpleExpr = Value::Int(Some(42)).into();
		assert!(matches!(expr, SimpleExpr::Value(Value::Int(Some(42)))));
	}

	#[rstest]
	fn test_simple_expr_from_bool() {
		let expr: SimpleExpr = true.into();
		assert!(matches!(expr, SimpleExpr::Value(Value::Bool(Some(true)))));
	}

	#[rstest]
	fn test_simple_expr_from_i32() {
		let expr: SimpleExpr = 42i32.into();
		assert!(matches!(expr, SimpleExpr::Value(Value::Int(Some(42)))));
	}

	#[rstest]
	fn test_simple_expr_from_str() {
		let expr: SimpleExpr = "hello".into();
		if let SimpleExpr::Value(Value::String(Some(s))) = expr {
			assert_eq!(*s, "hello");
		} else {
			panic!("Expected String value");
		}
	}

	#[rstest]
	fn test_simple_expr_column() {
		let col = ColumnRef::column("name");
		let expr: SimpleExpr = col.into();
		assert!(matches!(expr, SimpleExpr::Column(_)));
	}

	#[rstest]
	fn test_keyword_as_str() {
		assert_eq!(Keyword::Null.as_str(), "NULL");
		assert_eq!(Keyword::True.as_str(), "TRUE");
		assert_eq!(Keyword::False.as_str(), "FALSE");
		assert_eq!(Keyword::CurrentTimestamp.as_str(), "CURRENT_TIMESTAMP");
	}

	#[rstest]
	fn test_case_statement_builder() {
		let case = CaseStatement::new()
			.when(true, 1i32)
			.when(false, 0i32)
			.else_result(-1i32);

		assert_eq!(case.when_clauses.len(), 2);
		assert!(case.else_clause.is_some());
	}

	#[rstest]
	fn test_simple_expr_binary() {
		let left = SimpleExpr::Column(ColumnRef::column("age"));
		let right = SimpleExpr::Value(Value::Int(Some(18)));
		let binary =
			SimpleExpr::Binary(Box::new(left), BinOper::GreaterThanOrEqual, Box::new(right));

		assert!(matches!(
			binary,
			SimpleExpr::Binary(_, BinOper::GreaterThanOrEqual, _)
		));
	}

	#[rstest]
	fn test_simple_expr_unary() {
		let inner = SimpleExpr::Value(Value::Bool(Some(true)));
		let unary = SimpleExpr::Unary(UnOper::Not, Box::new(inner));

		assert!(matches!(unary, SimpleExpr::Unary(UnOper::Not, _)));
	}
}
