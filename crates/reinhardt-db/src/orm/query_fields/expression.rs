//! Model-rooted expressions for type-safe ORM queries.

pub(crate) mod compiler;
pub(crate) mod kind;
pub(crate) mod node;
pub(crate) mod operand;

use crate::orm::expressions::{FieldRef, GeneratedModelField};
use crate::orm::query_fields::comparison::ComparisonOperator;
use crate::orm::relations::{GeneratedRelatedField, RelatedFieldRef, RelationPathLike};
use crate::orm::{DatabaseField, DatabaseScalar, Model};
pub use kind::{AggregateKind, AnnotationExpressionKind, CombineKind, ScalarKind};
use node::{ExpressionNode, JoinRequirements, RootColumnOperand, StoredExpression};
use operand::ArithmeticOperation;
use reinhardt_core::exception::Error;
use reinhardt_query::prelude::{Alias, ColumnRef, Expr, ExprTrait, IntoIden, Order, SimpleExpr};
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
	marker: PhantomData<TypedExpressionMarker<M, R, K>>,
}

type TypedExpressionMarker<M, R, K> = fn() -> (M, R, K);

impl<M, R, K> TypedExpression<M, R, K> {
	#[cfg(feature = "pgvector")]
	pub(crate) fn new(expr: SimpleExpr) -> Self {
		Self {
			node: ExpressionNode::ExistingSimpleExpr(expr),
			joins: JoinRequirements::default(),
			marker: PhantomData,
		}
	}

	pub(crate) fn from_parts(node: ExpressionNode, joins: JoinRequirements) -> Self {
		Self {
			node,
			joins,
			marker: PhantomData,
		}
	}

	pub(crate) fn into_simple_expr(self) -> SimpleExpr {
		self.node.into_simple_expr()
	}

	pub(crate) fn into_parts(self) -> (ExpressionNode, JoinRequirements) {
		(self.node, self.joins)
	}

	/// Erase the result type while retaining metadata required for SQL lowering.
	pub(crate) fn into_stored_expression(self, label: Option<String>) -> StoredExpression {
		StoredExpression::new(self.node, self.joins, label)
	}

	/// Assign an identifier-safe label to this expression.
	pub fn label(self, label: impl AsRef<str>) -> Result<LabeledExpression<M, K>, Error> {
		let label = label.as_ref();
		validate_label(label)?;
		Ok(LabeledExpression {
			label: label.to_owned(),
			expression: StoredExpression::new(self.node, self.joins, Some(label.to_owned())),
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

impl<M, R> TypedExpression<M, R, ScalarKind>
where
	R: DatabaseField,
{
	/// Compare this scalar expression for equality.
	pub fn eq<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.compare(value, SimpleExpr::eq)
			.expect("typed scalar comparison values must encode for their database field")
	}

	/// Compare this scalar expression for inequality.
	pub fn ne<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.compare(value, SimpleExpr::ne)
			.expect("typed scalar comparison values must encode for their database field")
	}

	/// Compare this scalar expression using greater-than.
	pub fn gt<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.compare(value, SimpleExpr::gt)
			.expect("typed scalar comparison values must encode for their database field")
	}

	/// Compare this scalar expression using greater-than-or-equal.
	pub fn ge<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.compare(value, SimpleExpr::gte)
			.expect("typed scalar comparison values must encode for their database field")
	}

	/// Compatibility alias for [`Self::ge`].
	pub fn gte<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.ge(value)
	}

	/// Compare this scalar expression using less-than.
	pub fn lt<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.compare(value, SimpleExpr::lt)
			.expect("typed scalar comparison values must encode for their database field")
	}

	/// Compare this scalar expression using less-than-or-equal.
	pub fn le<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.compare(value, SimpleExpr::lte)
			.expect("typed scalar comparison values must encode for their database field")
	}

	/// Compatibility alias for [`Self::le`].
	pub fn lte<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> TypedPredicate<M> {
		self.le(value)
	}

	fn compare<V: crate::orm::IntoFieldValue<R>>(
		self,
		value: V,
		operator: fn(SimpleExpr, SimpleExpr) -> SimpleExpr,
	) -> Result<TypedPredicate<M>, Error> {
		let value = value
			.into_field_value()
			.map(crate::orm::database_value_to_query_value)
			.map_err(|error| Error::Validation(error.to_string()))?;
		let joins = self.joins.clone();
		Ok(TypedPredicate {
			expr: operator(
				self.into_simple_expr(),
				Expr::value(value).into_simple_expr(),
			),
			joins,
			marker: PhantomData,
		})
	}
}

impl<M, R> TypedExpression<M, R, AggregateKind>
where
	R: DatabaseField,
{
	/// Apply SQL `DISTINCT` to this aggregate's operand.
	///
	/// `COUNT(*)` has no operand and therefore cannot be made distinct.
	/// Aggregate constructors retain the shared [`AggregateKind`] return type,
	/// so this method validates the structured node instead of using a separate
	/// public type for `COUNT(*)`.
	///
	/// # Panics
	/// Panics when called for `COUNT(*)` or an aggregate-kind composition. Use
	/// [`Self::try_distinct`] when the expression is not known to be an operand
	/// aggregate node.
	pub fn distinct(self) -> Self {
		self.try_distinct()
			.unwrap_or_else(|error| panic!("{error}"))
	}

	/// Try to apply SQL `DISTINCT` to this aggregate's operand.
	///
	/// `COUNT(*)` and aggregate-kind compositions return a validation error
	/// because they are not operand aggregate nodes.
	pub fn try_distinct(mut self) -> Result<Self, Error> {
		match &mut self.node {
			ExpressionNode::Aggregate { distinct, .. } => {
				*distinct = true;
				Ok(self)
			}
			ExpressionNode::CountAll => Err(Error::Validation(
				"COUNT(*) does not support DISTINCT because it has no operand".to_owned(),
			)),
			_ => Err(Error::Validation(
				"DISTINCT is only available on operand aggregate nodes".to_owned(),
			)),
		}
	}

	/// Compare this aggregate expression for equality in a HAVING clause.
	pub fn eq<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.compare(value, ComparisonOperator::Eq)
			.expect("typed aggregate comparison values must encode for their database field")
	}

	/// Compare this aggregate expression for inequality in a HAVING clause.
	pub fn ne<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.compare(value, ComparisonOperator::Ne)
			.expect("typed aggregate comparison values must encode for their database field")
	}

	/// Compare this aggregate expression using greater-than in a HAVING clause.
	pub fn gt<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.compare(value, ComparisonOperator::Gt)
			.expect("typed aggregate comparison values must encode for their database field")
	}

	/// Compare this aggregate expression using greater-than-or-equal in a HAVING clause.
	pub fn ge<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.compare(value, ComparisonOperator::Gte)
			.expect("typed aggregate comparison values must encode for their database field")
	}

	/// Compatibility alias for [`Self::ge`].
	pub fn gte<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.ge(value)
	}

	/// Compare this aggregate expression using less-than in a HAVING clause.
	pub fn lt<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.compare(value, ComparisonOperator::Lt)
			.expect("typed aggregate comparison values must encode for their database field")
	}

	/// Compare this aggregate expression using less-than-or-equal in a HAVING clause.
	pub fn le<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.compare(value, ComparisonOperator::Lte)
			.expect("typed aggregate comparison values must encode for their database field")
	}

	/// Compatibility alias for [`Self::le`].
	pub fn lte<V: crate::orm::IntoFieldValue<R>>(self, value: V) -> HavingPredicate<M> {
		self.le(value)
	}

	fn compare<V: crate::orm::IntoFieldValue<R>>(
		self,
		value: V,
		operator: ComparisonOperator,
	) -> Result<HavingPredicate<M>, Error> {
		let value = value
			.into_field_value()
			.map(crate::orm::database_value_to_query_value)
			.map_err(|error| Error::Validation(error.to_string()))?;
		let joins = self.joins.clone();
		let node = ExpressionNode::Comparison {
			left: Box::new(self.node),
			operator,
			right: Box::new(ExpressionNode::ExistingSimpleExpr(
				Expr::value(value).into_simple_expr(),
			)),
		};
		Ok(HavingPredicate {
			expression: StoredExpression::new(node, joins, None),
			marker: PhantomData,
		})
	}
}

impl<M, T> From<FieldRef<M, T, GeneratedModelField>> for TypedExpression<M, T>
where
	T: DatabaseField,
{
	fn from(field: FieldRef<M, T, GeneratedModelField>) -> Self {
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

impl<Root, Target, Value> From<RelatedFieldRef<Root, Target, Value, GeneratedRelatedField>>
	for TypedExpression<Root, Value>
where
	Root: Model,
	Target: Model,
	Value: DatabaseField,
{
	fn from(field: RelatedFieldRef<Root, Target, Value, GeneratedRelatedField>) -> Self {
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
				composite_primary_key: false,
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
		let condition_joins = self.condition.joins.clone();
		TypedExpression::from_parts(
			ExpressionNode::Case {
				condition: self.condition.expr,
				condition_joins,
				result: Box::new(self.result.node),
				otherwise: Some(Box::new(otherwise.node)),
			},
			self.condition
				.joins
				.combine(self.result.joins)
				.combine(otherwise.joins),
		)
	}
}

/// An expression whose result type has been erased after applying a label.
#[derive(Debug)]
pub struct LabeledExpression<M, K = ScalarKind> {
	label: String,
	expression: StoredExpression,
	marker: PhantomData<fn() -> (M, K)>,
}

impl<M, K> Clone for LabeledExpression<M, K> {
	fn clone(&self) -> Self {
		Self {
			label: self.label.clone(),
			expression: self.expression.clone(),
			marker: PhantomData,
		}
	}
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

pub(crate) fn validate_label(label: &str) -> Result<(), Error> {
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
	pub(crate) joins: JoinRequirements,
	marker: PhantomData<fn() -> M>,
}

/// A boolean aggregate comparison to be compiled as a HAVING predicate.
#[derive(Debug, Clone)]
pub struct HavingPredicate<M> {
	/// Structured aggregate comparison consumed by the query planner.
	pub(crate) expression: StoredExpression,
	marker: PhantomData<fn() -> M>,
}

impl<M> HavingPredicate<M> {
	/// Erase the model marker after the root type has been checked by `QuerySet`.
	pub(crate) fn into_stored_expression(self) -> StoredExpression {
		self.expression
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
		crate::orm::query::FilterCondition::Single(crate::orm::query::Filter::typed_predicate(self))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::expressions::{FieldRef, GeneratedModelField};
	use crate::orm::query_fields::literal;
	use reinhardt_core::exception::Error;
	use reinhardt_query::prelude::{PostgresQueryBuilder, Query, QueryStatementBuilder};

	#[derive(Clone)]
	struct TestModel;

	fn typed_i64_expression() -> TypedExpression<TestModel, i64> {
		literal(42_i64).expect("integer literals are valid database values")
	}

	fn generated_i64_field(name: &'static str) -> FieldRef<TestModel, i64, GeneratedModelField> {
		// SAFETY: the test fixture declares a distinct i64-backed model column for each name.
		unsafe { FieldRef::from_generated_model_field_with_names(name, name) }
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
		let first: TypedExpression<TestModel, i64> = generated_i64_field("first_total").into();
		let second: TypedExpression<TestModel, i64> = generated_i64_field("second_total").into();
		let first = first.label("first").expect("valid label must succeed");
		let second = second.label("second").expect("valid label must succeed");

		let unique = StoredExpression::deduplicate(vec![first.expression, second.expression]);

		assert_eq!(unique.len(), 2);
	}

	#[test]
	fn identical_case_nodes_are_deduplicated() {
		let first = case_when(
			literal(1.0_f64)
				.expect("floating point literals are valid database values")
				.eq(1.0),
			typed_i64_expression(),
		)
		.otherwise(typed_i64_expression())
		.label("first")
		.expect("valid label must succeed");
		let duplicate = case_when(
			literal(1.0_f64)
				.expect("floating point literals are valid database values")
				.eq(1.0),
			typed_i64_expression(),
		)
		.otherwise(typed_i64_expression())
		.label("duplicate")
		.expect("valid label must succeed");

		let unique = StoredExpression::deduplicate(vec![first.expression, duplicate.expression]);

		assert_eq!(unique.len(), 1);
	}

	#[test]
	fn distinct_operand_aggregate_retains_structured_state_and_renders_sql() {
		let expression = crate::orm::func::sum(generated_i64_field("total")).distinct();
		let (node, _) = expression.clone().into_parts();

		assert!(matches!(
			node,
			ExpressionNode::Aggregate { distinct: true, .. }
		));

		let mut statement = Query::select();
		statement
			.expr(expression.into_simple_expr())
			.from("test_models");
		assert_eq!(
			statement.to_string(PostgresQueryBuilder),
			r#"SELECT SUM(DISTINCT "total") FROM "test_models""#
		);
	}

	#[test]
	fn count_all_rejects_distinct_without_creating_operand_state() {
		assert!(matches!(
			crate::orm::func::count_all::<TestModel>().try_distinct(),
			Err(Error::Validation(message)) if message == "COUNT(*) does not support DISTINCT because it has no operand"
		));
	}

	#[test]
	fn composed_aggregate_rejects_distinct_without_panicking() {
		let expression = crate::orm::func::sum(generated_i64_field("total"))
			+ literal::<TestModel, _>(1_i64).expect("integer literals are valid database values");

		assert!(matches!(
			expression.try_distinct(),
			Err(Error::Validation(message)) if message == "DISTINCT is only available on operand aggregate nodes"
		));
	}

	#[test]
	#[should_panic(expected = "COUNT(*) does not support DISTINCT")]
	fn count_all_distinct_panics_to_preserve_the_operand_only_contract() {
		let _ = crate::orm::func::count_all::<TestModel>().distinct();
	}
}
