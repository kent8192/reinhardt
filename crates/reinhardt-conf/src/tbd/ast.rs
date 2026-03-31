//! AST definitions for the TBD DSL.
//!
//! This module defines the abstract syntax tree (AST) nodes produced by
//! parsing TBD DSL expressions. Each node carries a [`Span`] so that
//! error messages can point back to the original source location.

use crate::tbd::Span;

/// A numeric value, either integer or floating-point.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberValue {
	/// A signed 64-bit integer literal (e.g. `42`, `-7`).
	Int(i64),
	/// A 64-bit floating-point literal (e.g. `3.14`, `-0.5`).
	Float(f64),
}

/// A literal value that appears directly in the source text.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
	/// A numeric literal (integer or float).
	Number(NumberValue),
	/// A UTF-8 string literal enclosed in double quotes.
	String(String),
	/// A boolean literal (`true` or `false`).
	Boolean(bool),
}

/// A binary arithmetic operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
	/// Addition (`+`).
	Add,
	/// Subtraction (`-`).
	Sub,
	/// Multiplication (`*`).
	Mul,
	/// Division (`/`).
	Div,
}

/// An expression node in the TBD DSL AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
	/// A literal value (number, string, or boolean).
	Literal(Literal),
	/// A function call with a name and zero or more arguments.
	FunctionCall {
		/// The function name being invoked.
		name: String,
		/// Positional arguments passed to the function.
		args: Vec<SpannedExpr>,
	},
	/// A pipe expression that feeds the left-hand side into the right-hand side.
	Pipe {
		/// The expression whose result is piped.
		left: Box<SpannedExpr>,
		/// The expression that receives the piped value.
		right: Box<SpannedExpr>,
	},
	/// A binary arithmetic operation (e.g. `a + b`).
	BinaryOp {
		/// The operator being applied.
		op: BinOp,
		/// The left-hand operand.
		left: Box<SpannedExpr>,
		/// The right-hand operand.
		right: Box<SpannedExpr>,
	},
	/// A tuple expression (e.g. `(a, b, c)`).
	Tuple(Vec<SpannedExpr>),
	/// An array expression (e.g. `[a, b, c]`).
	Array(Vec<SpannedExpr>),
	/// An expansion expression that spreads a value (e.g. `...expr`).
	Expansion(Box<SpannedExpr>),
	/// A bare identifier referencing a named value or variable.
	Identifier(String),
}

/// An expression annotated with its source location.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedExpr {
	/// The expression node.
	pub expr: Expr,
	/// The byte-offset span in the original source text.
	pub span: Span,
}

impl SpannedExpr {
	/// Creates a new spanned expression from the given node and span.
	pub fn new(expr: Expr, span: Span) -> Self {
		Self { expr, span }
	}
}
