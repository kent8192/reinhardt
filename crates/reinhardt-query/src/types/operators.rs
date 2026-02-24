//! SQL operators for expressions.
//!
//! This module provides operators used in SQL expressions:
//!
//! - [`UnOper`]: Unary operators (NOT, etc.)
//! - [`BinOper`]: Binary operators (AND, OR, =, <, etc.)
//! - [`LogicalChainOper`]: Operators for chaining conditions
//! - [`SubQueryOper`]: Subquery operators (EXISTS, ANY, ALL, SOME)

/// Unary operators.
///
/// These operators take a single operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOper {
	/// Logical NOT
	Not,
}

impl UnOper {
	/// Returns the SQL representation of this operator.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Not => "NOT",
		}
	}
}

/// Binary operators.
///
/// These operators take two operands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOper {
	// Logical operators
	/// Logical AND
	And,
	/// Logical OR
	Or,

	// Comparison operators
	/// Equal (=)
	Equal,
	/// Not equal (<>)
	NotEqual,
	/// Less than (<)
	SmallerThan,
	/// Less than or equal (<=)
	SmallerThanOrEqual,
	/// Greater than (>)
	GreaterThan,
	/// Greater than or equal (>=)
	GreaterThanOrEqual,

	// Pattern matching
	/// LIKE
	Like,
	/// NOT LIKE
	NotLike,
	/// ILIKE (case-insensitive LIKE, PostgreSQL)
	ILike,
	/// NOT ILIKE (PostgreSQL)
	NotILike,
	/// SIMILAR TO (PostgreSQL)
	SimilarTo,
	/// NOT SIMILAR TO (PostgreSQL)
	NotSimilarTo,
	/// Regex match (~ in PostgreSQL)
	Matches,
	/// Regex not match (!~ in PostgreSQL)
	NotMatches,

	// Set membership
	/// IN
	In,
	/// NOT IN
	NotIn,
	/// BETWEEN
	Between,
	/// NOT BETWEEN
	NotBetween,

	// NULL checks
	/// IS
	Is,
	/// IS NOT
	IsNot,

	// Arithmetic operators
	/// Addition (+)
	Add,
	/// Subtraction (-)
	Sub,
	/// Multiplication (*)
	Mul,
	/// Division (/)
	Div,
	/// Modulo (%)
	Mod,

	// Bit operators
	/// Bitwise AND (&)
	BitAnd,
	/// Bitwise OR (|)
	BitOr,
	/// Left shift (<<)
	LShift,
	/// Right shift (>>)
	RShift,

	// Array operators (PostgreSQL)
	/// Array contains (@>)
	PgOperator(PgBinOper),
}

/// PostgreSQL-specific binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PgBinOper {
	/// Contains (@>)
	Contains,
	/// Contained by (<@)
	Contained,
	/// Overlap (&&)
	Overlap,
	/// Concatenate (||)
	Concatenate,
	/// JSONB key exists (?)
	JsonContainsKey,
	/// JSONB any key exists (?|)
	JsonContainsAnyKey,
	/// JSONB all keys exist (?&)
	JsonContainsAllKeys,
	/// Get JSON element (->)
	JsonGetByIndex,
	/// Get JSON element as text (->>)
	JsonGetAsText,
	/// Get JSON path (#>)
	JsonGetPath,
	/// Get JSON path as text (#>>)
	JsonGetPathAsText,
}

impl BinOper {
	/// Returns the SQL representation of this operator.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::And => "AND",
			Self::Or => "OR",
			Self::Equal => "=",
			Self::NotEqual => "<>",
			Self::SmallerThan => "<",
			Self::SmallerThanOrEqual => "<=",
			Self::GreaterThan => ">",
			Self::GreaterThanOrEqual => ">=",
			Self::Like => "LIKE",
			Self::NotLike => "NOT LIKE",
			Self::ILike => "ILIKE",
			Self::NotILike => "NOT ILIKE",
			Self::SimilarTo => "SIMILAR TO",
			Self::NotSimilarTo => "NOT SIMILAR TO",
			Self::Matches => "~",
			Self::NotMatches => "!~",
			Self::In => "IN",
			Self::NotIn => "NOT IN",
			Self::Between => "BETWEEN",
			Self::NotBetween => "NOT BETWEEN",
			Self::Is => "IS",
			Self::IsNot => "IS NOT",
			Self::Add => "+",
			Self::Sub => "-",
			Self::Mul => "*",
			Self::Div => "/",
			Self::Mod => "%",
			Self::BitAnd => "&",
			Self::BitOr => "|",
			Self::LShift => "<<",
			Self::RShift => ">>",
			Self::PgOperator(pg_op) => pg_op.as_str(),
		}
	}

	/// Returns the precedence of this operator.
	///
	/// Higher values indicate higher precedence (binds more tightly).
	#[must_use]
	pub fn precedence(&self) -> u8 {
		match self {
			Self::Or => 1,
			Self::And => 2,
			Self::Is | Self::IsNot => 3,
			Self::Between | Self::NotBetween | Self::In | Self::NotIn => 4,
			Self::Like
			| Self::NotLike
			| Self::ILike
			| Self::NotILike
			| Self::SimilarTo
			| Self::NotSimilarTo
			| Self::Matches
			| Self::NotMatches => 5,
			Self::Equal
			| Self::NotEqual
			| Self::SmallerThan
			| Self::SmallerThanOrEqual
			| Self::GreaterThan
			| Self::GreaterThanOrEqual => 6,
			Self::BitOr => 7,
			Self::BitAnd => 8,
			Self::LShift | Self::RShift => 9,
			Self::Add | Self::Sub => 10,
			Self::Mul | Self::Div | Self::Mod => 11,
			Self::PgOperator(_) => 6, // Same as comparison
		}
	}

	/// Returns whether this operator is left-associative.
	#[must_use]
	pub fn is_left_associative(&self) -> bool {
		// All binary operators in SQL are left-associative
		true
	}
}

impl PgBinOper {
	/// Returns the SQL representation of this PostgreSQL operator.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Contains => "@>",
			Self::Contained => "<@",
			Self::Overlap => "&&",
			Self::Concatenate => "||",
			Self::JsonContainsKey => "?",
			Self::JsonContainsAnyKey => "?|",
			Self::JsonContainsAllKeys => "?&",
			Self::JsonGetByIndex => "->",
			Self::JsonGetAsText => "->>",
			Self::JsonGetPath => "#>",
			Self::JsonGetPathAsText => "#>>",
		}
	}
}

/// Logical operators for chaining conditions.
///
/// These operators connect conditions in WHERE clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalChainOper {
	/// Logical AND
	And,
	/// Logical OR
	Or,
}

impl LogicalChainOper {
	/// Returns the SQL representation of this operator.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::And => "AND",
			Self::Or => "OR",
		}
	}
}

impl From<LogicalChainOper> for BinOper {
	fn from(op: LogicalChainOper) -> Self {
		match op {
			LogicalChainOper::And => BinOper::And,
			LogicalChainOper::Or => BinOper::Or,
		}
	}
}

/// Subquery operators.
///
/// These operators are used with subqueries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubQueryOper {
	/// EXISTS
	Exists,
	/// ANY
	Any,
	/// SOME (alias for ANY)
	Some,
	/// ALL
	All,
}

impl SubQueryOper {
	/// Returns the SQL representation of this operator.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Exists => "EXISTS",
			Self::Any => "ANY",
			Self::Some => "SOME",
			Self::All => "ALL",
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_un_oper_as_str() {
		assert_eq!(UnOper::Not.as_str(), "NOT");
	}

	#[rstest]
	#[case(BinOper::And, "AND")]
	#[case(BinOper::Or, "OR")]
	#[case(BinOper::Equal, "=")]
	#[case(BinOper::NotEqual, "<>")]
	#[case(BinOper::SmallerThan, "<")]
	#[case(BinOper::GreaterThan, ">")]
	#[case(BinOper::Like, "LIKE")]
	#[case(BinOper::In, "IN")]
	#[case(BinOper::Between, "BETWEEN")]
	fn test_bin_oper_as_str(#[case] op: BinOper, #[case] expected: &str) {
		assert_eq!(op.as_str(), expected);
	}

	#[rstest]
	fn test_bin_oper_precedence() {
		// Multiplication has higher precedence than addition
		assert!(BinOper::Mul.precedence() > BinOper::Add.precedence());
		// AND has higher precedence than OR
		assert!(BinOper::And.precedence() > BinOper::Or.precedence());
		// Comparison has higher precedence than logical
		assert!(BinOper::Equal.precedence() > BinOper::And.precedence());
	}

	#[rstest]
	fn test_logical_chain_oper() {
		assert_eq!(LogicalChainOper::And.as_str(), "AND");
		assert_eq!(LogicalChainOper::Or.as_str(), "OR");
	}

	#[rstest]
	fn test_subquery_oper() {
		assert_eq!(SubQueryOper::Exists.as_str(), "EXISTS");
		assert_eq!(SubQueryOper::Any.as_str(), "ANY");
		assert_eq!(SubQueryOper::All.as_str(), "ALL");
	}

	#[rstest]
	fn test_pg_bin_oper() {
		assert_eq!(PgBinOper::Contains.as_str(), "@>");
		assert_eq!(PgBinOper::Contained.as_str(), "<@");
		assert_eq!(PgBinOper::JsonGetAsText.as_str(), "->>");
	}
}
