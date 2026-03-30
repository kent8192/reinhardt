//! Type definitions for aggregate functions and comparison expressions
//!
//! Represents aggregate functions (COUNT, SUM, AVG, MIN, MAX) used in HAVING clauses
//! and their comparison expressions.

use super::comparison::ComparisonOperator;

/// Types of aggregate functions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunction {
	/// COUNT aggregate function
	Count,
	/// SUM aggregate function
	Sum,
	/// AVG aggregate function
	Avg,
	/// MIN aggregate function
	Min,
	/// MAX aggregate function
	Max,
}

/// Aggregate expression
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::query_fields::aggregate::*;
///
/// // COUNT(*) > 5
/// let expr = AggregateExpr::count("*").gt(5);
///
/// // AVG(price) <= 100.0
/// let expr = AggregateExpr::avg("price").lte(100.0);
/// ```
#[derive(Debug, Clone)]
pub struct AggregateExpr {
	/// Type of aggregate function
	function: AggregateFunction,
	/// Target field name ("*" or specific field name)
	field: String,
}

impl AggregateExpr {
	/// Create COUNT aggregate expression
	///
	/// # Arguments
	///
	/// * `field` - Field name (usually "*")
	pub fn count(field: &str) -> Self {
		Self {
			function: AggregateFunction::Count,
			field: field.to_string(),
		}
	}

	/// Create SUM aggregate expression
	pub fn sum(field: &str) -> Self {
		Self {
			function: AggregateFunction::Sum,
			field: field.to_string(),
		}
	}

	/// Create AVG aggregate expression
	pub fn avg(field: &str) -> Self {
		Self {
			function: AggregateFunction::Avg,
			field: field.to_string(),
		}
	}

	/// Create MIN aggregate expression
	pub fn min(field: &str) -> Self {
		Self {
			function: AggregateFunction::Min,
			field: field.to_string(),
		}
	}

	/// Create MAX aggregate expression
	pub fn max(field: &str) -> Self {
		Self {
			function: AggregateFunction::Max,
			field: field.to_string(),
		}
	}

	/// Get the type of aggregate function
	pub fn function(&self) -> AggregateFunction {
		self.function
	}

	/// Get the target field name
	pub fn field(&self) -> &str {
		&self.field
	}

	// Comparison methods for method chaining

	/// Greater than (>)
	pub fn gt(self, value: impl Into<ComparisonValue>) -> ComparisonExpr {
		ComparisonExpr::new(self, ComparisonOperator::Gt, value.into())
	}

	/// Greater than or equal (>=)
	pub fn gte(self, value: impl Into<ComparisonValue>) -> ComparisonExpr {
		ComparisonExpr::new(self, ComparisonOperator::Gte, value.into())
	}

	/// Less than (<)
	pub fn lt(self, value: impl Into<ComparisonValue>) -> ComparisonExpr {
		ComparisonExpr::new(self, ComparisonOperator::Lt, value.into())
	}

	/// Less than or equal (<=)
	pub fn lte(self, value: impl Into<ComparisonValue>) -> ComparisonExpr {
		ComparisonExpr::new(self, ComparisonOperator::Lte, value.into())
	}

	/// Equal (=)
	pub fn eq(self, value: impl Into<ComparisonValue>) -> ComparisonExpr {
		ComparisonExpr::new(self, ComparisonOperator::Eq, value.into())
	}

	/// Not equal (!=)
	pub fn ne(self, value: impl Into<ComparisonValue>) -> ComparisonExpr {
		ComparisonExpr::new(self, ComparisonOperator::Ne, value.into())
	}
}

/// Comparison value (integer or floating point)
#[derive(Debug, Clone)]
pub enum ComparisonValue {
	/// Integer value
	Int(i64),
	/// Floating point value
	Float(f64),
}

impl From<i64> for ComparisonValue {
	fn from(i: i64) -> Self {
		Self::Int(i)
	}
}

impl From<i32> for ComparisonValue {
	fn from(i: i32) -> Self {
		Self::Int(i.into())
	}
}

impl From<f64> for ComparisonValue {
	fn from(f: f64) -> Self {
		Self::Float(f)
	}
}

impl From<f32> for ComparisonValue {
	fn from(f: f32) -> Self {
		Self::Float(f.into())
	}
}

/// Comparison expression for aggregate functions
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::query_fields::aggregate::*;
/// use reinhardt_db::orm::query_fields::comparison::ComparisonOperator;
///
/// // COUNT(*) > 5
/// let expr = ComparisonExpr::new(
///     AggregateExpr::count("*"),
///     ComparisonOperator::Gt,
///     ComparisonValue::Int(5),
/// );
/// ```
#[derive(Debug, Clone)]
pub struct ComparisonExpr {
	/// Aggregate expression
	pub aggregate: AggregateExpr,
	/// Comparison operator
	pub op: ComparisonOperator,
	/// Comparison value
	pub value: ComparisonValue,
}

impl ComparisonExpr {
	/// Create a comparison expression
	pub fn new(aggregate: AggregateExpr, op: ComparisonOperator, value: ComparisonValue) -> Self {
		Self {
			aggregate,
			op,
			value,
		}
	}
}
