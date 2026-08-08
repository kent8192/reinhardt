//! Model-rooted expressions for type-safe ORM queries.

mod kind;
mod node;
mod operand;

use crate::orm::expressions::FieldRef;
use crate::orm::relations::{RelatedFieldRef, RelationPathLike};
use crate::orm::{DatabaseField, DatabaseScalar, Model};
pub(crate) use kind::AggregateOutputKind;
pub use kind::{AggregateKind, AnnotationExpressionKind, CombineKind, ScalarKind};
use node::{ExpressionNode, JoinRequirements, RootColumnOperand, StoredExpression};
use operand::ArithmeticOperation;
use reinhardt_core::exception::Error;
use reinhardt_query::prelude::{Alias, ColumnRef, ExprTrait, IntoIden, Order, SimpleExpr};
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Sub};

pub(crate) fn qualify_model_root(expr: &SimpleExpr, root_alias: &str) -> SimpleExpr {
	let mut qualified = expr.clone();
	qualify_model_root_in_place(&mut qualified, root_alias);
	qualified
}

fn qualify_model_root_in_place(expr: &mut SimpleExpr, root_alias: &str) {
	if let SimpleExpr::Column(ColumnRef::Column(column)) = expr {
		let column = column.clone();
		*expr = SimpleExpr::Column(ColumnRef::TableColumn(
			Alias::new(root_alias).into_iden(),
			column,
		));
		return;
	}

	match expr {
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::TemporalTrunc {
			expr: expression, ..
		}
		| SimpleExpr::WindowNamed {
			func: expression, ..
		} => qualify_model_root_in_place(expression, root_alias),
		SimpleExpr::Binary(left, _, right) => {
			qualify_model_root_in_place(left, root_alias);
			qualify_model_root_in_place(right, root_alias);
		}
		SimpleExpr::FunctionCall(_, expressions) | SimpleExpr::Tuple(expressions) => {
			for expression in expressions {
				qualify_model_root_in_place(expression, root_alias);
			}
		}
		SimpleExpr::Case(case) => {
			for (condition, result) in &mut case.when_clauses {
				qualify_model_root_in_place(condition, root_alias);
				qualify_model_root_in_place(result, root_alias);
			}
			if let Some(result) = &mut case.else_clause {
				qualify_model_root_in_place(result, root_alias);
			}
		}
		SimpleExpr::Window { func, window } => {
			qualify_model_root_in_place(func, root_alias);
			for expression in &mut window.partition_by {
				qualify_model_root_in_place(expression, root_alias);
			}
			for ordering in &mut window.order_by {
				if let reinhardt_query::types::OrderExprKind::Expr(expression) = &mut ordering.expr
				{
					qualify_model_root_in_place(expression, root_alias);
				}
			}
		}
		_ => {}
	}
}

/// A SQL expression rooted in model `M` and producing values of type `R`.
#[derive(Debug, Clone)]
pub struct TypedExpression<M, R, K = ScalarKind> {
	node: ExpressionNode,
	joins: JoinRequirements,
	marker: PhantomData<fn() -> (M, R, K)>,
}

impl<M, R, K> TypedExpression<M, R, K> {
	#[cfg(feature = "pgvector")]
	pub(crate) fn new(expr: SimpleExpr) -> Self {
		Self {
			node: ExpressionNode::ExistingSimpleExpr(expr),
			joins: JoinRequirements::default(),
			marker: PhantomData,
		}
	}

	fn from_parts(node: ExpressionNode, joins: JoinRequirements) -> Self {
		Self {
			node,
			joins,
			marker: PhantomData,
		}
	}

	pub(crate) fn into_simple_expr(self) -> SimpleExpr {
		self.node.into_simple_expr()
	}

	/// Assign an identifier-safe label to this expression.
	pub fn label(self, label: impl AsRef<str>) -> Result<LabeledExpression<M, K>, Error> {
		let label = label.as_ref();
		validate_label(label)?;
		Ok(LabeledExpression {
			label: label.to_owned(),
			expression: StoredExpression {
				node: self.node,
				joins: self.joins,
			},
			marker: PhantomData,
		})
	}

	/// Order this expression in ascending order.
	pub fn asc(self) -> OrderedExpression<M> {
		OrderedExpression::new(self.into_simple_expr(), Order::Asc)
	}

	/// Order this expression in descending order.
	pub fn desc(self) -> OrderedExpression<M> {
		OrderedExpression::new(self.into_simple_expr(), Order::Desc)
	}
}

impl<M> TypedExpression<M, f64, ScalarKind> {
	/// Compare this numeric expression for equality.
	pub fn eq(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.into_simple_expr().eq(value))
	}

	/// Compare this numeric expression using less-than.
	pub fn lt(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.into_simple_expr().lt(value))
	}

	/// Compare this numeric expression using less-than-or-equal.
	pub fn le(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.into_simple_expr().lte(value))
	}

	/// Compare this numeric expression using greater-than.
	pub fn gt(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.into_simple_expr().gt(value))
	}

	/// Compare this numeric expression using greater-than-or-equal.
	pub fn ge(self, value: f64) -> TypedPredicate<M> {
		TypedPredicate::new(self.into_simple_expr().gte(value))
	}
}

impl<M, T, Origin> From<FieldRef<M, T, Origin>> for TypedExpression<M, T>
where
	T: DatabaseField,
{
	fn from(field: FieldRef<M, T, Origin>) -> Self {
		Self::from_parts(
			ExpressionNode::RootColumn(RootColumnOperand {
				logical_name: field.logical_name().to_owned(),
				physical_column: field.name().to_owned(),
				storage_kind: <T::Storage as DatabaseScalar>::STORAGE_KIND,
			}),
			JoinRequirements::default(),
		)
	}
}

impl<Root, Target, Value, Origin> From<RelatedFieldRef<Root, Target, Value, Origin>>
	for TypedExpression<Root, Value>
where
	Root: Model,
	Target: Model,
	Value: DatabaseField,
{
	fn from(field: RelatedFieldRef<Root, Target, Value, Origin>) -> Self {
		let terminal_column = Target::field_metadata()
			.into_iter()
			.find(|metadata| metadata.name == field.name())
			.map_or_else(
				|| field.name().to_owned(),
				|metadata| metadata.db_column_name().to_owned(),
			);
		let relation_steps = field.path().steps().to_vec();
		Self::from_parts(
			ExpressionNode::RelatedColumn(node::RelatedColumnOperand {
				relation_steps: relation_steps.clone(),
				terminal_column,
				storage_kind: <Value::Storage as DatabaseScalar>::STORAGE_KIND,
			}),
			JoinRequirements::from_relation_steps(relation_steps),
		)
	}
}

impl<M, R, LeftKind, RightKind> Add<TypedExpression<M, R, RightKind>>
	for TypedExpression<M, R, LeftKind>
where
	LeftKind: CombineKind<RightKind>,
	RightKind: AnnotationExpressionKind,
{
	type Output = TypedExpression<M, R, <LeftKind as CombineKind<RightKind>>::Output>;

	fn add(self, right: TypedExpression<M, R, RightKind>) -> Self::Output {
		compose_arithmetic(self, right, ArithmeticOperation::Add)
	}
}

impl<M, R, LeftKind, RightKind> Sub<TypedExpression<M, R, RightKind>>
	for TypedExpression<M, R, LeftKind>
where
	LeftKind: CombineKind<RightKind>,
	RightKind: AnnotationExpressionKind,
{
	type Output = TypedExpression<M, R, <LeftKind as CombineKind<RightKind>>::Output>;

	fn sub(self, right: TypedExpression<M, R, RightKind>) -> Self::Output {
		compose_arithmetic(self, right, ArithmeticOperation::Subtract)
	}
}

impl<M, R, LeftKind, RightKind> Mul<TypedExpression<M, R, RightKind>>
	for TypedExpression<M, R, LeftKind>
where
	LeftKind: CombineKind<RightKind>,
	RightKind: AnnotationExpressionKind,
{
	type Output = TypedExpression<M, R, <LeftKind as CombineKind<RightKind>>::Output>;

	fn mul(self, right: TypedExpression<M, R, RightKind>) -> Self::Output {
		compose_arithmetic(self, right, ArithmeticOperation::Multiply)
	}
}

impl<M, R, LeftKind, RightKind> Div<TypedExpression<M, R, RightKind>>
	for TypedExpression<M, R, LeftKind>
where
	LeftKind: CombineKind<RightKind>,
	RightKind: AnnotationExpressionKind,
{
	type Output = TypedExpression<M, R, <LeftKind as CombineKind<RightKind>>::Output>;

	fn div(self, right: TypedExpression<M, R, RightKind>) -> Self::Output {
		compose_arithmetic(self, right, ArithmeticOperation::Divide)
	}
}

fn compose_arithmetic<M, R, LeftKind, RightKind>(
	left: TypedExpression<M, R, LeftKind>,
	right: TypedExpression<M, R, RightKind>,
	operation: ArithmeticOperation,
) -> TypedExpression<M, R, <LeftKind as CombineKind<RightKind>>::Output>
where
	LeftKind: CombineKind<RightKind>,
	RightKind: AnnotationExpressionKind,
{
	TypedExpression::from_parts(
		ExpressionNode::Arithmetic {
			left: Box::new(left.node),
			operation,
			right: Box::new(right.node),
		},
		left.joins.combine(right.joins),
	)
}

/// Create a typed database literal for an expression rooted at `M`.
pub fn literal<M, T: DatabaseField>(value: T) -> Result<TypedExpression<M, T>, Error> {
	let value = value
		.encode_database()
		.map(DatabaseScalar::into_database_value)
		.map_err(|error| Error::Validation(error.to_string()))?;
	Ok(TypedExpression::from_parts(
		ExpressionNode::Literal(value),
		JoinRequirements::default(),
	))
}

/// Combine two nullable-compatible expressions with SQL `COALESCE`.
pub fn coalesce<M, R, LeftKind, RightKind>(
	left: TypedExpression<M, R, LeftKind>,
	right: TypedExpression<M, R, RightKind>,
) -> TypedExpression<M, R, <LeftKind as CombineKind<RightKind>>::Output>
where
	LeftKind: CombineKind<RightKind>,
	RightKind: AnnotationExpressionKind,
{
	TypedExpression::from_parts(
		ExpressionNode::Coalesce {
			left: Box::new(left.node),
			right: Box::new(right.node),
		},
		left.joins.combine(right.joins),
	)
}

/// Start a typed SQL `CASE WHEN` expression.
pub fn case_when<M, R, K>(
	condition: TypedPredicate<M>,
	result: TypedExpression<M, R, K>,
) -> CaseWhen<M, R, K> {
	CaseWhen { condition, result }
}

/// A pending `CASE WHEN` expression that requires its `ELSE` branch.
#[derive(Debug, Clone)]
pub struct CaseWhen<M, R, K = ScalarKind> {
	condition: TypedPredicate<M>,
	result: TypedExpression<M, R, K>,
}

impl<M, R, LeftKind> CaseWhen<M, R, LeftKind> {
	/// Complete this case expression with a same-model, same-result-type fallback.
	pub fn otherwise<RightKind>(
		self,
		otherwise: TypedExpression<M, R, RightKind>,
	) -> TypedExpression<M, R, <LeftKind as CombineKind<RightKind>>::Output>
	where
		LeftKind: CombineKind<RightKind>,
		RightKind: AnnotationExpressionKind,
	{
		TypedExpression::from_parts(
			ExpressionNode::Case {
				condition: self.condition.expr,
				result: Box::new(self.result.node),
				otherwise: Some(Box::new(otherwise.node)),
			},
			self.result.joins.combine(otherwise.joins),
		)
	}
}

/// An expression whose result type has been erased after applying a label.
#[derive(Debug, Clone)]
pub struct LabeledExpression<M, K = ScalarKind> {
	label: String,
	expression: StoredExpression,
	marker: PhantomData<fn() -> (M, K)>,
}

impl<M, K> LabeledExpression<M, K> {
	/// Return the validated SQL label for this expression.
	pub fn label(&self) -> &str {
		&self.label
	}

	// Annotation projection planning consumes the erased expression representation.
	#[allow(dead_code)]
	pub(crate) fn into_stored_expression(self) -> StoredExpression {
		self.expression
	}
}

fn validate_label(label: &str) -> Result<(), Error> {
	if label.is_empty() || label.len() > 63 || !label.is_ascii() {
		return Err(Error::Validation(
			"aggregate label must be 1 to 63 ASCII bytes".to_owned(),
		));
	}
	let first = label.as_bytes()[0];
	if !first.is_ascii_alphabetic() && first != b'_' {
		return Err(Error::Validation(
			"aggregate label must start with an ASCII letter or underscore".to_owned(),
		));
	}
	if !label
		.as_bytes()
		.iter()
		.all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
	{
		return Err(Error::Validation(
			"aggregate label must contain only ASCII letters, digits, or underscores".to_owned(),
		));
	}
	Ok(())
}

/// A boolean SQL expression rooted in model `M`.
#[derive(Debug, Clone)]
pub struct TypedPredicate<M> {
	pub(crate) expr: SimpleExpr,
	marker: PhantomData<fn() -> M>,
}

impl<M> TypedPredicate<M> {
	fn new(expr: SimpleExpr) -> Self {
		Self {
			expr,
			marker: PhantomData,
		}
	}
}

/// A model-rooted expression with an ordering direction.
#[derive(Debug, Clone)]
pub struct OrderedExpression<M> {
	pub(crate) expr: SimpleExpr,
	pub(crate) order: Order,
	marker: PhantomData<fn() -> M>,
}

impl<M> OrderedExpression<M> {
	fn new(expr: SimpleExpr, order: Order) -> Self {
		Self {
			expr,
			order,
			marker: PhantomData,
		}
	}
}

impl<M> crate::orm::query::QueryFilterInput<M> for TypedPredicate<M>
where
	M: Model,
{
	fn into_filter_condition(self) -> crate::orm::query::FilterCondition {
		crate::orm::query::FilterCondition::Single(crate::orm::query::Filter::typed_predicate(
			self.expr,
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::expressions::FieldRef;
	use crate::orm::query_fields::literal;
	use reinhardt_core::exception::Error;

	struct TestModel;

	fn typed_i64_expression() -> TypedExpression<TestModel, i64> {
		literal(42_i64).expect("integer literals are valid database values")
	}

	#[test]
	fn label_accepts_a_valid_ascii_identifier() {
		assert!(typed_i64_expression().label("total_1").is_ok());
	}

	#[test]
	fn label_rejects_an_empty_identifier() {
		assert!(matches!(
			typed_i64_expression().label(""),
			Err(Error::Validation(message)) if message == "aggregate label must be 1 to 63 ASCII bytes"
		));
	}

	#[test]
	fn label_rejects_an_identifier_starting_with_a_digit() {
		assert!(matches!(
			typed_i64_expression().label("9total"),
			Err(Error::Validation(message)) if message == "aggregate label must start with an ASCII letter or underscore"
		));
	}

	#[test]
	fn label_rejects_invalid_identifier_forms() {
		assert!(typed_i64_expression().label(&"a".repeat(64)).is_err());
		assert!(typed_i64_expression().label("total-value").is_err());
		assert!(typed_i64_expression().label("合計").is_err());
	}

	#[test]
	fn identical_structured_nodes_are_deduplicated() {
		let first = typed_i64_expression()
			.label("first")
			.expect("valid label must succeed");
		let duplicate = typed_i64_expression()
			.label("duplicate")
			.expect("valid label must succeed");

		let unique = StoredExpression::deduplicate(vec![first.expression, duplicate.expression]);

		assert_eq!(unique.len(), 1);
	}

	#[test]
	fn distinct_physical_columns_are_not_deduplicated() {
		let first: TypedExpression<TestModel, i64> = FieldRef::new("first_total").into();
		let second: TypedExpression<TestModel, i64> = FieldRef::new("second_total").into();
		let first = first.label("first").expect("valid label must succeed");
		let second = second.label("second").expect("valid label must succeed");

		let unique = StoredExpression::deduplicate(vec![first.expression, second.expression]);

		assert_eq!(unique.len(), 2);
	}
}
