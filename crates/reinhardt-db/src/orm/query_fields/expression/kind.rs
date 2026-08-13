//! Type-level expression kinds used to preserve aggregate provenance.

mod private {
	pub trait Sealed {}
}

/// Marks an expression evaluated once per input row.
#[derive(Debug, Clone, Copy)]
pub struct ScalarKind;

/// Marks an expression that contains an aggregate operation.
#[derive(Debug, Clone, Copy)]
pub struct AggregateKind;

impl private::Sealed for ScalarKind {}
impl private::Sealed for AggregateKind {}

/// Sealed marker for the SQL evaluation kind of an annotation expression.
pub trait AnnotationExpressionKind: private::Sealed {}

impl AnnotationExpressionKind for ScalarKind {}
impl AnnotationExpressionKind for AggregateKind {}

/// Combines the evaluation kinds of two composed expressions.
pub trait CombineKind<Right>: AnnotationExpressionKind {
	/// Evaluation kind of the composed expression.
	type Output: AnnotationExpressionKind;
}

impl CombineKind<ScalarKind> for ScalarKind {
	type Output = ScalarKind;
}

impl CombineKind<AggregateKind> for ScalarKind {
	type Output = AggregateKind;
}

impl CombineKind<ScalarKind> for AggregateKind {
	type Output = AggregateKind;
}

impl CombineKind<AggregateKind> for AggregateKind {
	type Output = AggregateKind;
}

/// Storage kind produced by a numeric aggregate.
// Aggregate constructors consume this mapping when materializing structured aggregate nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateOutputKind {
	/// Signed 64-bit integer result.
	I64,
	/// 64-bit floating-point result.
	F64,
	/// Fixed-precision decimal result.
	Decimal,
}
