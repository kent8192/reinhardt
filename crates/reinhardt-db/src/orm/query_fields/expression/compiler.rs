//! Backend-neutral lowering for structured typed expressions.

use super::node::{ExpressionNode, RelatedColumnOperand, StoredExpression};
use super::operand::{AggregateOperation, ArithmeticOperation};
use crate::orm::field_codec::database_value_to_query_value;
use crate::orm::relations::RelationJoinGraph;
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Result};
use reinhardt_query::prelude::{Alias, BinOper, Expr, Func, SimpleExpr};

/// Lower one erased typed expression after its relation aliases have been planned.
pub(crate) fn compile_expression(
	expression: &StoredExpression,
	root_alias: &str,
	graph: &RelationJoinGraph,
) -> Result<SimpleExpr> {
	compile_node(&expression.node, root_alias, graph)
}

fn compile_node(
	node: &ExpressionNode,
	root_alias: &str,
	graph: &RelationJoinGraph,
) -> Result<SimpleExpr> {
	match node {
		ExpressionNode::RootColumn(column) => Ok(Expr::col((
			Alias::new(root_alias),
			Alias::new(column.physical_column.clone()),
		))
		.into_simple_expr()),
		ExpressionNode::RelatedColumn(column) => compile_related_column(column, graph),
		ExpressionNode::Literal(value) => {
			Ok(Expr::val(database_value_to_query_value(value.clone())).into_simple_expr())
		}
		ExpressionNode::Aggregate {
			operation,
			operand,
			distinct,
			..
		} => {
			if *distinct && related_operand_has_composite_key(operand) {
				return Err(DatabaseError::new(
					DatabaseErrorKind::Unsupported,
					"COUNT(DISTINCT relation) does not support composite primary keys on PostgreSQL, MySQL, and SQLite",
				)
				.into());
			}
			let operand = compile_node(operand, root_alias, graph)?;
			let operand = if *distinct {
				SimpleExpr::CustomWithExpr("DISTINCT ?".to_owned(), vec![operand])
			} else {
				operand
			};
			Ok(match operation {
				AggregateOperation::Count => Func::count(operand),
				AggregateOperation::Sum => Func::sum(operand),
				AggregateOperation::Average => Func::avg(operand),
				AggregateOperation::Minimum => Func::min(operand),
				AggregateOperation::Maximum => Func::max(operand),
			})
		}
		ExpressionNode::CountAll => Ok(Expr::cust("COUNT(*)").into_simple_expr()),
		ExpressionNode::Arithmetic {
			left,
			operation,
			right,
		} => Ok(SimpleExpr::Binary(
			Box::new(compile_node(left, root_alias, graph)?),
			match operation {
				ArithmeticOperation::Add => BinOper::Add,
				ArithmeticOperation::Subtract => BinOper::Sub,
				ArithmeticOperation::Multiply => BinOper::Mul,
				ArithmeticOperation::Divide => BinOper::Div,
			},
			Box::new(compile_node(right, root_alias, graph)?),
		)),
		ExpressionNode::Case {
			condition,
			result,
			otherwise,
		} => {
			let case = Expr::case().when(
				crate::orm::query_fields::qualify_model_root(condition, root_alias),
				compile_node(result, root_alias, graph)?,
			);
			Ok(match otherwise {
				Some(otherwise) => case
					.else_result(compile_node(otherwise, root_alias, graph)?)
					.into_simple_expr(),
				None => case.build().into_simple_expr(),
			})
		}
		ExpressionNode::Coalesce { left, right } => Ok(Func::coalesce(vec![
			compile_node(left, root_alias, graph)?,
			compile_node(right, root_alias, graph)?,
		])),
		ExpressionNode::ExistingSimpleExpr(expression) => Ok(
			crate::orm::query_fields::qualify_model_root(expression, root_alias),
		),
	}
}

fn compile_related_column(
	column: &RelatedColumnOperand,
	graph: &RelationJoinGraph,
) -> Result<SimpleExpr> {
	let aliases = graph
		.aliases_for_steps(&column.relation_steps)
		.ok_or_else(|| {
			DatabaseError::new(
				DatabaseErrorKind::Query,
				"typed expression relation path could not be resolved in the query join graph",
			)
		})?;
	let alias = aliases.last().ok_or_else(|| {
		DatabaseError::new(
			DatabaseErrorKind::Query,
			"typed expression relation path is empty",
		)
	})?;
	Ok(Expr::col((
		Alias::new(alias.clone()),
		Alias::new(column.terminal_column.clone()),
	))
	.into_simple_expr())
}

fn related_operand_has_composite_key(node: &ExpressionNode) -> bool {
	matches!(node, ExpressionNode::RelatedColumn(column) if column.composite_primary_key)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::field_codec::DatabaseStorageKind;
	use crate::orm::query_fields::expression::node::JoinRequirements;
	use crate::orm::relations::{RelationJoinKind, RelationMultiplicity, RelationStep};
	use reinhardt_query::prelude::{
		PostgresQueryBuilder, Query, QueryStatementBuilder, SelectStatement, SqliteQueryBuilder,
	};
	use std::borrow::Cow;

	fn expression(node: ExpressionNode, joins: JoinRequirements) -> StoredExpression {
		StoredExpression::new(node, joins, Some("value".to_owned()))
	}

	fn relation_step() -> RelationStep {
		RelationStep {
			name: Cow::Borrowed("posts"),
			source_table: Cow::Borrowed("authors"),
			target_table: Cow::Borrowed("posts"),
			source_column: Cow::Borrowed("author_pk"),
			target_column: Cow::Borrowed("author_id"),
			default_join_kind: RelationJoinKind::Inner,
			multiplicity: RelationMultiplicity::Multiple,
		}
	}

	fn render(stmt: SelectStatement, postgres: bool) -> String {
		if postgres {
			stmt.to_string(PostgresQueryBuilder)
		} else {
			stmt.to_string(SqliteQueryBuilder)
		}
	}

	#[test]
	fn lowers_count_all_without_qualifying_star() {
		let stored = expression(ExpressionNode::CountAll, JoinRequirements::default());
		let graph = RelationJoinGraph::new("authors");
		let mut stmt = Query::select();
		stmt.from(Alias::new("authors"));
		stmt.expr_as(
			compile_expression(&stored, "authors", &graph).expect("COUNT(*) compiles"),
			Alias::new("row_count"),
		);
		assert_eq!(
			render(stmt.to_owned(), true),
			"SELECT COUNT(*) AS \"row_count\" FROM \"authors\""
		);
	}

	#[test]
	fn lowers_physical_columns_and_related_count_with_left_join() {
		let step = relation_step();
		let joins = JoinRequirements::from_relation_steps(vec![step.clone()]);
		let node = ExpressionNode::Aggregate {
			operation: AggregateOperation::Count,
			operand: Box::new(ExpressionNode::RelatedColumn(RelatedColumnOperand {
				relation_steps: vec![step.clone()],
				terminal_column: "post_pk".to_owned(),
				storage_kind: DatabaseStorageKind::I64,
				composite_primary_key: false,
			})),
			distinct: false,
			output_kind: Some(super::super::kind::AggregateOutputKind::I64),
		};
		let stored = expression(node, joins);
		let mut graph = RelationJoinGraph::new("authors");
		graph.add_aggregate_steps(&stored.joins.relation_steps);
		let graph = graph.with_root_alias("authors");
		let mut stmt = Query::select();
		stmt.from(Alias::new("authors"));
		for join in graph.joins() {
			stmt.left_join(
				(
					Alias::new(join.target_table.clone()),
					Alias::new(join.alias.clone()),
				),
				Expr::col((
					Alias::new(join.source_alias.clone()),
					Alias::new(join.source_column.clone()),
				))
				.equals((
					Alias::new(join.alias.clone()),
					Alias::new(join.target_column.clone()),
				)),
			);
		}
		stmt.expr_as(
			compile_expression(&stored, "authors", &graph).expect("related count compiles"),
			Alias::new("post_count"),
		);
		let sql = render(stmt.to_owned(), true);
		assert_eq!(sql.matches("LEFT JOIN").count(), 1);
		assert!(sql.contains("COUNT(\"posts\".\"post_pk\")"));
		assert!(!sql.contains("COUNT(\"authors\".\"posts\")"));
	}
}
