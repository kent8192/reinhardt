//! Structured expression nodes and relation-join requirements.

use super::operand::{AggregateOperation, ArithmeticOperation};
use crate::orm::field_codec::{DatabaseStorageKind, DatabaseValue};
use crate::orm::relations::RelationStep;
use reinhardt_query::prelude::{Alias, BinOper, Expr, Func, SimpleExpr};

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
}

/// Relation joins required by an expression node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct JoinRequirements {
	pub(crate) relation_steps: Vec<RelationStep>,
}

impl JoinRequirements {
	pub(crate) fn from_relation_steps(relation_steps: Vec<RelationStep>) -> Self {
		Self { relation_steps }
	}

	pub(crate) fn combine(mut self, other: Self) -> Self {
		for step in other.relation_steps {
			if !self.relation_steps.contains(&step) {
				self.relation_steps.push(step);
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
	// Aggregate constructors materialize this variant in the subsequent annotation API layer.
	#[allow(dead_code)]
	Aggregate {
		operation: AggregateOperation,
		operand: Box<Self>,
	},
	/// An arithmetic operation applied to two operands.
	Arithmetic {
		left: Box<Self>,
		operation: ArithmeticOperation,
		right: Box<Self>,
	},
	/// A single-branch conditional expression.
	Case {
		condition: SimpleExpr,
		result: Box<Self>,
		otherwise: Option<Box<Self>>,
	},
	/// A two-operand coalescing expression.
	Coalesce { left: Box<Self>, right: Box<Self> },
	/// A pre-existing query-builder expression retained for compatibility.
	ExistingSimpleExpr(SimpleExpr),
}

impl ExpressionNode {
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
			Self::Aggregate { operation, operand } => {
				let operand = operand.into_simple_expr();
				match operation {
					AggregateOperation::Count => Func::count(operand),
					AggregateOperation::Sum => Func::sum(operand),
					AggregateOperation::Average => Func::avg(operand),
					AggregateOperation::Minimum => Func::min(operand),
					AggregateOperation::Maximum => Func::max(operand),
				}
			}
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
				result,
				otherwise,
			} => {
				let case = Expr::case().when(condition, result.into_simple_expr());
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
				},
				Self::Aggregate {
					operation: right_operation,
					operand: right_operand,
				},
			) => left_operation == right_operation && left_operand.structurally_eq(right_operand),
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
				Self::Coalesce {
					left: left_left,
					right: left_right,
				},
				Self::Coalesce {
					left: right_left,
					right: right_right,
				},
			) => left_left.structurally_eq(right_left) && left_right.structurally_eq(right_right),
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
}

impl StoredExpression {
	// Annotation projection planning consumes structural equality to deduplicate repeated expressions.
	#[allow(dead_code)]
	pub(crate) fn structurally_eq(&self, other: &Self) -> bool {
		self.joins == other.joins && self.node.structurally_eq(&other.node)
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
