//! Static typed expression and aggregate constructors.

use crate::orm::expressions::{FieldRef, GeneratedModelField};
use crate::orm::field_codec::{
	DatabaseField, DatabaseScalar, NumericAggregateField, NumericAggregateStorage,
};
use crate::orm::query_fields::expression::node::{
	ExpressionNode, JoinRequirements, RelatedColumnOperand,
};
use crate::orm::query_fields::expression::operand::AggregateOperation;
use crate::orm::query_fields::{
	AggregateKind, AggregateOutputKind, AnnotationExpressionKind, CaseWhen, CombineKind,
	TypedExpression, TypedPredicate,
};
use crate::orm::relations::{
	GeneratedRelatedField, GeneratedRelationPath, RelatedFieldRef, RelationJoinKind, RelationPath,
	RelationPathLike,
};
use crate::orm::{Model, RelationStep};
use reinhardt_core::exception::Error;

mod private {
	pub trait Sealed {}
}

/// A generated, type-checked operand accepted by [`count`].
///
/// This trait is sealed. Applications obtain implementations through generated
/// model fields and relation paths rather than implementing it themselves.
pub trait CountOperand<M>: private::Sealed {
	/// Typed value represented by this operand.
	type Value: DatabaseField;

	/// Convert this proven operand to its structured expression form.
	#[doc(hidden)]
	fn into_count_operand(self) -> TypedExpression<M, Self::Value>;
}

/// A generated, numeric operand accepted by [`sum`] and [`avg`].
///
/// This trait is sealed. `NumericAggregateField` is the explicit application
/// opt-in while the storage type determines the portable aggregate result.
pub trait NumericAggregateOperand<M>: private::Sealed {
	/// Typed field value accepted by the aggregate.
	type Value: NumericAggregateField;
	/// Result type produced by SQL `SUM`.
	type SumOutput: DatabaseField;
	/// Result type produced by SQL `AVG`.
	type AverageOutput: DatabaseField;

	/// Convert this proven operand to its structured expression form.
	#[doc(hidden)]
	fn into_numeric_aggregate_operand(self) -> TypedExpression<M, Self::Value>;
	/// Storage result metadata for SQL `SUM`.
	#[doc(hidden)]
	fn sum_output_kind() -> AggregateOutputKind;
	/// Storage result metadata for SQL `AVG`.
	#[doc(hidden)]
	fn average_output_kind() -> AggregateOutputKind;
}

/// A generated, portable ordered operand accepted by [`min`] and [`max`].
///
/// This trait is sealed. Its implementations are limited to storage types with
/// portable ordering semantics across PostgreSQL, MySQL, and SQLite.
pub trait OrderedAggregateOperand<M>: private::Sealed {
	/// Typed field value returned by the aggregate.
	type Value: DatabaseField;

	/// Convert this proven operand to its structured expression form.
	#[doc(hidden)]
	fn into_ordered_aggregate_operand(self) -> TypedExpression<M, Self::Value>;
}

trait OrderedAggregateStorage: DatabaseScalar {}

macro_rules! impl_ordered_aggregate_storage {
	($($type:ty),+ $(,)?) => {
		$(impl OrderedAggregateStorage for $type {})+
	};
}

impl_ordered_aggregate_storage!(
	i32,
	i64,
	f32,
	f64,
	rust_decimal::Decimal,
	String,
	uuid::Uuid,
	chrono::NaiveDate,
	chrono::NaiveTime,
	chrono::DateTime<chrono::Utc>,
	chrono::NaiveDateTime,
);

impl<S> OrderedAggregateStorage for Option<S> where S: OrderedAggregateStorage {}

impl<M, Value> private::Sealed for FieldRef<M, Value, GeneratedModelField> {}

impl<M, Value> CountOperand<M> for FieldRef<M, Value, GeneratedModelField>
where
	Value: DatabaseField,
{
	type Value = Value;

	fn into_count_operand(self) -> TypedExpression<M, Self::Value> {
		self.into()
	}
}

impl<M, Value> NumericAggregateOperand<M> for FieldRef<M, Value, GeneratedModelField>
where
	Value: NumericAggregateField,
	Value::Storage: NumericAggregateStorage,
{
	type Value = Value;
	type SumOutput = <Value::Storage as NumericAggregateStorage>::SumOutput;
	type AverageOutput = <Value::Storage as NumericAggregateStorage>::AverageOutput;

	fn into_numeric_aggregate_operand(self) -> TypedExpression<M, Self::Value> {
		self.into()
	}

	fn sum_output_kind() -> AggregateOutputKind {
		<Value::Storage as NumericAggregateStorage>::SUM_KIND
	}

	fn average_output_kind() -> AggregateOutputKind {
		<Value::Storage as NumericAggregateStorage>::AVERAGE_KIND
	}
}

impl<M, Value> OrderedAggregateOperand<M> for FieldRef<M, Value, GeneratedModelField>
where
	Value: DatabaseField,
	Value::Storage: OrderedAggregateStorage,
{
	type Value = Value;

	fn into_ordered_aggregate_operand(self) -> TypedExpression<M, Self::Value> {
		self.into()
	}
}

impl<Root, Target, Value> private::Sealed
	for RelatedFieldRef<Root, Target, Value, GeneratedRelatedField>
where
	Root: Model,
	Target: Model,
{
}

impl<Root, Target, Value> CountOperand<Root>
	for RelatedFieldRef<Root, Target, Value, GeneratedRelatedField>
where
	Root: Model,
	Target: Model,
	Value: DatabaseField,
{
	type Value = Value;

	fn into_count_operand(self) -> TypedExpression<Root, Self::Value> {
		self.into()
	}
}

impl<Root, Target, Value> NumericAggregateOperand<Root>
	for RelatedFieldRef<Root, Target, Value, GeneratedRelatedField>
where
	Root: Model,
	Target: Model,
	Value: NumericAggregateField,
	Value::Storage: NumericAggregateStorage,
{
	type Value = Value;
	type SumOutput = <Value::Storage as NumericAggregateStorage>::SumOutput;
	type AverageOutput = <Value::Storage as NumericAggregateStorage>::AverageOutput;

	fn into_numeric_aggregate_operand(self) -> TypedExpression<Root, Self::Value> {
		self.into()
	}

	fn sum_output_kind() -> AggregateOutputKind {
		<Value::Storage as NumericAggregateStorage>::SUM_KIND
	}

	fn average_output_kind() -> AggregateOutputKind {
		<Value::Storage as NumericAggregateStorage>::AVERAGE_KIND
	}
}

impl<Root, Target, Value> OrderedAggregateOperand<Root>
	for RelatedFieldRef<Root, Target, Value, GeneratedRelatedField>
where
	Root: Model,
	Target: Model,
	Value: DatabaseField,
	Value::Storage: OrderedAggregateStorage,
{
	type Value = Value;

	fn into_ordered_aggregate_operand(self) -> TypedExpression<Root, Self::Value> {
		self.into()
	}
}

impl<Root, Target> private::Sealed for RelationPath<Root, Target, GeneratedRelationPath>
where
	Root: Model,
	Target: Model,
{
}

impl<Root, Target> CountOperand<Root> for RelationPath<Root, Target, GeneratedRelationPath>
where
	Root: Model,
	Target: Model,
	Target::PrimaryKey: DatabaseField,
{
	type Value = Target::PrimaryKey;

	fn into_count_operand(self) -> TypedExpression<Root, Self::Value> {
		let relation_steps = left_join_steps(self.steps());
		TypedExpression::from_parts(
			ExpressionNode::RelatedColumn(RelatedColumnOperand {
				relation_steps: relation_steps.clone(),
				terminal_column: Target::primary_key_column().to_owned(),
				storage_kind:
					<<Target::PrimaryKey as DatabaseField>::Storage as DatabaseScalar>::STORAGE_KIND,
			}),
			JoinRequirements::from_relation_steps(relation_steps),
		)
	}
}

/// Construct `COUNT(*)` for model `M`.
pub fn count_all<M>() -> TypedExpression<M, i64, AggregateKind> {
	TypedExpression::from_parts(ExpressionNode::CountAll, JoinRequirements::default())
}

/// Construct a typed `COUNT` aggregate from a generated field or relation.
pub fn count<M, Input>(input: Input) -> TypedExpression<M, i64, AggregateKind>
where
	Input: CountOperand<M>,
{
	aggregate(
		input.into_count_operand(),
		AggregateOperation::Count,
		Some(AggregateOutputKind::I64),
	)
}

/// Construct a typed `SUM` aggregate from a generated numeric field.
pub fn sum<M, Input>(input: Input) -> TypedExpression<M, Input::SumOutput, AggregateKind>
where
	Input: NumericAggregateOperand<M>,
{
	aggregate(
		input.into_numeric_aggregate_operand(),
		AggregateOperation::Sum,
		Some(Input::sum_output_kind()),
	)
}

/// Construct a typed `AVG` aggregate from a generated numeric field.
pub fn avg<M, Input>(input: Input) -> TypedExpression<M, Input::AverageOutput, AggregateKind>
where
	Input: NumericAggregateOperand<M>,
{
	aggregate(
		input.into_numeric_aggregate_operand(),
		AggregateOperation::Average,
		Some(Input::average_output_kind()),
	)
}

/// Construct a typed `MIN` aggregate from a generated ordered field.
pub fn min<M, Input>(input: Input) -> TypedExpression<M, Input::Value, AggregateKind>
where
	Input: OrderedAggregateOperand<M>,
{
	aggregate(
		input.into_ordered_aggregate_operand(),
		AggregateOperation::Minimum,
		None,
	)
}

/// Construct a typed `MAX` aggregate from a generated ordered field.
pub fn max<M, Input>(input: Input) -> TypedExpression<M, Input::Value, AggregateKind>
where
	Input: OrderedAggregateOperand<M>,
{
	aggregate(
		input.into_ordered_aggregate_operand(),
		AggregateOperation::Maximum,
		None,
	)
}

/// Create a typed database literal for an expression rooted at `M`.
pub fn literal<M, Value: DatabaseField>(value: Value) -> Result<TypedExpression<M, Value>, Error> {
	crate::orm::query_fields::literal(value)
}

/// Combine two nullable-compatible expressions with SQL `COALESCE`.
///
/// For three or more operands, nest calls from left to right.
pub fn coalesce<M, ResultType, LeftKind, RightKind>(
	left: TypedExpression<M, ResultType, LeftKind>,
	right: TypedExpression<M, ResultType, RightKind>,
) -> TypedExpression<M, ResultType, <LeftKind as CombineKind<RightKind>>::Output>
where
	LeftKind: CombineKind<RightKind>,
	RightKind: AnnotationExpressionKind,
{
	crate::orm::query_fields::coalesce(left, right)
}

/// Start a typed SQL `CASE WHEN` expression.
pub fn case_when<M, ResultType, Kind>(
	condition: TypedPredicate<M>,
	result: TypedExpression<M, ResultType, Kind>,
) -> CaseWhen<M, ResultType, Kind> {
	crate::orm::query_fields::case_when(condition, result)
}

fn aggregate<M, InputValue, Output>(
	input: TypedExpression<M, InputValue>,
	operation: AggregateOperation,
	output_kind: Option<AggregateOutputKind>,
) -> TypedExpression<M, Output, AggregateKind> {
	let (operand, joins) = input.into_parts();
	TypedExpression::from_parts(
		ExpressionNode::Aggregate {
			operation,
			operand: Box::new(operand),
			distinct: false,
			output_kind,
		},
		joins,
	)
}

fn left_join_steps(steps: &[RelationStep]) -> Vec<RelationStep> {
	steps
		.iter()
		.cloned()
		.map(|mut step| {
			step.default_join_kind = RelationJoinKind::Left;
			step
		})
		.collect()
}
