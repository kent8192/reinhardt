//! Structured operands used by typed annotation expression nodes.

/// SQL aggregate operation represented by an expression node.
// Aggregate constructors materialize these operations in the subsequent annotation API layer.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AggregateOperation {
	/// Count input rows.
	Count,
	/// Sum numeric values.
	Sum,
	/// Average numeric values.
	Average,
	/// Return the smallest input value.
	Minimum,
	/// Return the largest input value.
	Maximum,
}

/// Arithmetic operation represented by an expression node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArithmeticOperation {
	/// Addition.
	Add,
	/// Subtraction.
	Subtract,
	/// Multiplication.
	Multiply,
	/// Division.
	Divide,
}
