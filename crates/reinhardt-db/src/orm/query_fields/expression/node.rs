//! Structured expression nodes and relation-join requirements.

use super::super::comparison::ComparisonOperator;
use super::kind::AggregateOutputKind;
use super::operand::{AggregateOperation, ArithmeticOperation};
use crate::orm::field_codec::{DatabaseStorageKind, DatabaseValue};
use crate::orm::relations::RelationStep;
use reinhardt_query::prelude::SimpleExpr;
#[cfg(test)]
use reinhardt_query::prelude::{Alias, BinOper, Expr, ExprTrait, Func};

/// Root-column metadata retained independently from rendered SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootColumnOperand {
	pub(crate) logical_name: String,
	pub(crate) physical_column: String,
	pub(crate) storage_kind: DatabaseStorageKind,
}

/// Related-column metadata retained independently from rendered SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelatedColumnOperand {
	pub(crate) relation_steps: Vec<RelationStep>,
	pub(crate) terminal_column: String,
	pub(crate) storage_kind: DatabaseStorageKind,
	/// Whether the terminal relation target has a composite primary key.
	pub(crate) composite_primary_key: bool,
}

/// Relation joins required by an expression node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct JoinRequirements {
	pub(crate) paths: Vec<Vec<RelationStep>>,
}

impl JoinRequirements {
	pub(crate) fn from_relation_steps(relation_steps: Vec<RelationStep>) -> Self {
		if relation_steps.is_empty() {
			Self::default()
		} else {
			Self {
				paths: vec![relation_steps],
			}
		}
	}

	pub(crate) fn combine(mut self, other: Self) -> Self {
		for path in other.paths {
			if !self.paths.contains(&path) {
				self.paths.push(path);
			}
		}
		self
	}
}

/// A typed expression represented as structured metadata rather than raw SQL.
#[derive(Debug, Clone)]
pub(crate) enum ExpressionNode {
	/// A column on the root model.
	RootColumn(RootColumnOperand),
	/// A column reached through generated relation steps.
	RelatedColumn(RelatedColumnOperand),
	/// A typed, database-bound literal.
	Literal(DatabaseValue),
	/// An aggregate operation applied to an operand.
	Aggregate {
		operation: AggregateOperation,
		operand: Box<Self>,
		distinct: bool,
		output_kind: Option<AggregateOutputKind>,
	},
	/// A `COUNT(*)` operation with no column operand.
	CountAll,
	/// An arithmetic operation applied to two operands.
	Arithmetic {
		left: Box<Self>,
		operation: ArithmeticOperation,
		right: Box<Self>,
	},
	/// A single-branch conditional expression.
	Case {
		condition: Box<Self>,
		condition_error: Option<String>,
		result: Box<Self>,
		otherwise: Option<Box<Self>>,
	},
	/// A two-operand coalescing expression.
	Coalesce { left: Box<Self>, right: Box<Self> },
	/// A comparison between two typed expressions.
	Comparison {
		left: Box<Self>,
		operator: ComparisonOperator,
		right: Box<Self>,
	},
	/// A pre-existing query-builder expression retained for compatibility.
	ExistingSimpleExpr(SimpleExpr),
}

impl ExpressionNode {
	#[cfg(test)]
	pub(crate) fn into_simple_expr(self) -> SimpleExpr {
		match self {
			Self::RootColumn(operand) => {
				Expr::col(Alias::new(operand.physical_column)).into_simple_expr()
			}
			Self::RelatedColumn(operand) => {
				Expr::col(Alias::new(operand.terminal_column)).into_simple_expr()
			}
			Self::Literal(value) => {
				Expr::value(crate::orm::database_value_to_query_value(value)).into_simple_expr()
			}
			Self::Aggregate {
				operation,
				operand,
				distinct,
				..
			} => {
				let operand = operand.into_simple_expr();
				let operand = if distinct {
					SimpleExpr::CustomWithExpr("DISTINCT ?".to_owned(), vec![operand])
				} else {
					operand
				};
				match operation {
					AggregateOperation::Count => Func::count(operand),
					AggregateOperation::Sum => Func::sum(operand),
					AggregateOperation::Average => Func::avg(operand),
					AggregateOperation::Minimum => Func::min(operand),
					AggregateOperation::Maximum => Func::max(operand),
				}
			}
			Self::CountAll => Func::count(Expr::asterisk().into_simple_expr()),
			Self::Arithmetic {
				left,
				operation,
				right,
			} => SimpleExpr::Binary(
				Box::new(left.into_simple_expr()),
				match operation {
					ArithmeticOperation::Add => BinOper::Add,
					ArithmeticOperation::Subtract => BinOper::Sub,
					ArithmeticOperation::Multiply => BinOper::Mul,
					ArithmeticOperation::Divide => BinOper::Div,
				},
				Box::new(right.into_simple_expr()),
			),
			Self::Case {
				condition,
				condition_error: _,
				result,
				otherwise,
			} => {
				let case =
					Expr::case().when(condition.into_simple_expr(), result.into_simple_expr());
				match otherwise {
					Some(otherwise) => case
						.else_result(otherwise.into_simple_expr())
						.into_simple_expr(),
					None => case.build().into_simple_expr(),
				}
			}
			Self::Coalesce { left, right } => {
				Func::coalesce(vec![left.into_simple_expr(), right.into_simple_expr()])
			}
			Self::Comparison {
				left,
				operator,
				right,
			} => {
				let left = left.into_simple_expr();
				let right = right.into_simple_expr();
				match operator {
					ComparisonOperator::Eq => left.eq(right),
					ComparisonOperator::Ne => left.ne(right),
					ComparisonOperator::Gt => left.gt(right),
					ComparisonOperator::Gte => left.gte(right),
					ComparisonOperator::Lt => left.lt(right),
					ComparisonOperator::Lte => left.lte(right),
				}
			}
			Self::ExistingSimpleExpr(expression) => expression,
		}
	}

	// Annotation projection planning consumes structural equality to deduplicate repeated expressions.
	#[allow(dead_code)]
	pub(crate) fn structurally_eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::RootColumn(left), Self::RootColumn(right)) => left == right,
			(Self::RelatedColumn(left), Self::RelatedColumn(right)) => left == right,
			(Self::Literal(left), Self::Literal(right)) => left == right,
			(
				Self::Aggregate {
					operation: left_operation,
					operand: left_operand,
					distinct: left_distinct,
					output_kind: left_output_kind,
				},
				Self::Aggregate {
					operation: right_operation,
					operand: right_operand,
					distinct: right_distinct,
					output_kind: right_output_kind,
				},
			) => {
				left_operation == right_operation
					&& left_distinct == right_distinct
					&& left_output_kind == right_output_kind
					&& left_operand.structurally_eq(right_operand)
			}
			(Self::CountAll, Self::CountAll) => true,
			(
				Self::Arithmetic {
					left: left_left,
					operation: left_operation,
					right: left_right,
				},
				Self::Arithmetic {
					left: right_left,
					operation: right_operation,
					right: right_right,
				},
			) => {
				left_operation == right_operation
					&& left_left.structurally_eq(right_left)
					&& left_right.structurally_eq(right_right)
			}
			(
				Self::Case {
					condition: left_condition,
					condition_error: left_condition_error,
					result: left_result,
					otherwise: left_otherwise,
				},
				Self::Case {
					condition: right_condition,
					condition_error: right_condition_error,
					result: right_result,
					otherwise: right_otherwise,
				},
			) => {
				left_condition.structurally_eq(right_condition)
					&& left_condition_error == right_condition_error
					&& left_result.structurally_eq(right_result)
					&& match (left_otherwise, right_otherwise) {
						(Some(left), Some(right)) => left.structurally_eq(right),
						(None, None) => true,
						_ => false,
					}
			}
			(
				Self::Coalesce {
					left: left_left,
					right: left_right,
				},
				Self::Coalesce {
					left: right_left,
					right: right_right,
				},
			) => left_left.structurally_eq(right_left) && left_right.structurally_eq(right_right),
			(
				Self::Comparison {
					left: left_left,
					operator: left_operator,
					right: left_right,
				},
				Self::Comparison {
					left: right_left,
					operator: right_operator,
					right: right_right,
				},
			) => {
				left_operator == right_operator
					&& left_left.structurally_eq(right_left)
					&& left_right.structurally_eq(right_right)
			}
			(Self::ExistingSimpleExpr(left), Self::ExistingSimpleExpr(right)) => {
				format!("{left:?}") == format!("{right:?}")
			}
			_ => false,
		}
	}
}

/// Expression data stored after its result type is erased by a label.
#[derive(Debug, Clone)]
pub(crate) struct StoredExpression {
	// Annotation projection planning consumes these fields after a result type is erased by a label.
	#[allow(dead_code)]
	pub(crate) node: ExpressionNode,
	// Annotation projection planning consumes these fields after a result type is erased by a label.
	#[allow(dead_code)]
	pub(crate) joins: JoinRequirements,
	/// Aggregate result storage metadata, when this expression is an aggregate.
	pub(crate) output: Option<AggregateOutputKind>,
	/// Aggregate function represented by this expression, when applicable.
	pub(crate) aggregate_function: Option<TypedAggregateFn>,
	/// Scalar storage metadata used to decode MIN/MAX aggregate results.
	pub(crate) aggregate_storage_kind: Option<DatabaseStorageKind>,
	/// Optional identifier retained with the erased expression.
	// Annotation projection planning consumes this label in the next query phase.
	#[allow(dead_code)]
	pub(crate) label: Option<String>,
}

impl StoredExpression {
	pub(crate) fn contains_aggregate(&self) -> bool {
		self.node.contains_aggregate()
	}
	pub(crate) fn new(
		node: ExpressionNode,
		joins: JoinRequirements,
		label: Option<String>,
	) -> Self {
		let output = node.aggregate_output_kind();
		let aggregate_function = node.aggregate_function();
		let aggregate_storage_kind = node.aggregate_storage_kind();
		Self {
			node,
			joins,
			output,
			aggregate_function,
			aggregate_storage_kind,
			label,
		}
	}

	// Annotation projection planning consumes structural equality to deduplicate repeated expressions.
	#[allow(dead_code)]
	pub(crate) fn structurally_eq(&self, other: &Self) -> bool {
		self.joins == other.joins
			&& self.output == other.output
			&& self.aggregate_function == other.aggregate_function
			&& self.aggregate_storage_kind == other.aggregate_storage_kind
			&& self.node.structurally_eq(&other.node)
	}

	// Annotation projection planning consumes this helper when building a SELECT list.
	#[allow(dead_code)]
	pub(crate) fn deduplicate(expressions: Vec<Self>) -> Vec<Self> {
		let mut unique = Vec::with_capacity(expressions.len());
		for expression in expressions {
			if !unique
				.iter()
				.any(|existing: &Self| existing.structurally_eq(&expression))
			{
				unique.push(expression);
			}
		}
		unique
	}
}

impl ExpressionNode {
	pub(crate) fn contains_aggregate(&self) -> bool {
		match self {
			Self::Aggregate { .. } | Self::CountAll => true,
			Self::Arithmetic { left, right, .. } | Self::Coalesce { left, right } => {
				left.contains_aggregate() || right.contains_aggregate()
			}
			Self::Comparison { left, right, .. } => {
				left.contains_aggregate() || right.contains_aggregate()
			}
			Self::Case {
				result, otherwise, ..
			} => {
				result.contains_aggregate()
					|| otherwise.as_deref().is_some_and(Self::contains_aggregate)
			}
			_ => false,
		}
	}

	pub(crate) fn scalar_grouping_nodes(&self) -> Vec<Self> {
		if !self.contains_aggregate() {
			return vec![self.clone()];
		}
		match self {
			Self::Arithmetic { left, right, .. }
			| Self::Coalesce { left, right }
			| Self::Comparison { left, right, .. } => left
				.scalar_grouping_nodes()
				.into_iter()
				.chain(right.scalar_grouping_nodes())
				.collect(),
			Self::Case {
				result, otherwise, ..
			} => result
				.scalar_grouping_nodes()
				.into_iter()
				.chain(
					otherwise
						.iter()
						.flat_map(|node| node.scalar_grouping_nodes()),
				)
				.collect(),
			Self::Aggregate { .. } | Self::CountAll => Vec::new(),
			_ => vec![self.clone()],
		}
	}

	fn aggregate_output_kind(&self) -> Option<AggregateOutputKind> {
		match self {
			Self::Aggregate { output_kind, .. } => *output_kind,
			Self::CountAll => Some(AggregateOutputKind::I64),
			Self::Arithmetic { left, right, .. } | Self::Coalesce { left, right } => left
				.aggregate_output_kind()
				.or_else(|| right.aggregate_output_kind()),
			Self::Case {
				result, otherwise, ..
			} => result.aggregate_output_kind().or_else(|| {
				otherwise
					.as_deref()
					.and_then(ExpressionNode::aggregate_output_kind)
			}),
			Self::Comparison { left, right, .. } => left
				.aggregate_output_kind()
				.or_else(|| right.aggregate_output_kind()),
			_ => None,
		}
	}

	fn aggregate_function(&self) -> Option<TypedAggregateFn> {
		match self {
			Self::Aggregate { operation, .. } => Some(match operation {
				AggregateOperation::Count => TypedAggregateFn::Count,
				AggregateOperation::Sum => TypedAggregateFn::Sum,
				AggregateOperation::Average => TypedAggregateFn::Avg,
				AggregateOperation::Minimum => TypedAggregateFn::Min,
				AggregateOperation::Maximum => TypedAggregateFn::Max,
			}),
			Self::CountAll => Some(TypedAggregateFn::Count),
			Self::Arithmetic { left, right, .. } | Self::Coalesce { left, right } => left
				.aggregate_function()
				.or_else(|| right.aggregate_function()),
			Self::Case {
				result, otherwise, ..
			} => result.aggregate_function().or_else(|| {
				otherwise
					.as_deref()
					.and_then(ExpressionNode::aggregate_function)
			}),
			Self::Comparison { left, right, .. } => left
				.aggregate_function()
				.or_else(|| right.aggregate_function()),
			_ => None,
		}
	}

	fn aggregate_storage_kind(&self) -> Option<DatabaseStorageKind> {
		match self {
			Self::Aggregate { operand, .. } => scalar_storage_kind(operand),
			Self::Arithmetic { left, right, .. } | Self::Coalesce { left, right } => {
				scalar_storage_kind(left).or_else(|| scalar_storage_kind(right))
			}
			Self::Case {
				result, otherwise, ..
			} => scalar_storage_kind(result)
				.or_else(|| otherwise.as_deref().and_then(scalar_storage_kind)),
			Self::Comparison { left, right, .. } => {
				scalar_storage_kind(left).or_else(|| scalar_storage_kind(right))
			}
			_ => None,
		}
	}
}

fn scalar_storage_kind(node: &ExpressionNode) -> Option<DatabaseStorageKind> {
	match node {
		ExpressionNode::RootColumn(column) => Some(column.storage_kind),
		ExpressionNode::RelatedColumn(column) => Some(column.storage_kind),
		ExpressionNode::Aggregate { operand, .. } => scalar_storage_kind(operand),
		ExpressionNode::Arithmetic { left, right, .. }
		| ExpressionNode::Coalesce { left, right }
		| ExpressionNode::Comparison { left, right, .. } => {
			scalar_storage_kind(left).or_else(|| scalar_storage_kind(right))
		}
		ExpressionNode::Case {
			result, otherwise, ..
		} => scalar_storage_kind(result)
			.or_else(|| otherwise.as_deref().and_then(scalar_storage_kind)),
		ExpressionNode::CountAll
		| ExpressionNode::Literal(_)
		| ExpressionNode::ExistingSimpleExpr(_) => None,
	}
}

/// Aggregate operation metadata retained by stored expressions.
///
/// This is intentionally private to the expression planner. Public callers
/// construct aggregates through [`crate::orm::func`] instead of naming a
/// function or supplying a field string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypedAggregateFn {
	/// COUNT.
	Count,
	/// SUM.
	Sum,
	/// AVG.
	Avg,
	/// MIN.
	Min,
	/// MAX.
	Max,
}
