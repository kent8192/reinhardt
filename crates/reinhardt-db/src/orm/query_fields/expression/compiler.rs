//! Backend-neutral lowering for structured typed expressions.

use super::node::{ExpressionNode, RelatedColumnOperand, StoredExpression};
use super::operand::{AggregateOperation, ArithmeticOperation};
use crate::orm::field_codec::database_value_to_query_value;
use crate::orm::relations::RelationJoinGraph;
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Result};
use reinhardt_query::prelude::{Alias, BinOper, Expr, Func, IntoIden, SimpleExpr};

/// Lower one erased typed expression after its relation aliases have been planned.
pub(crate) fn compile_expression(
	expression: &StoredExpression,
	root_alias: &str,
	graph: &RelationJoinGraph,
) -> Result<SimpleExpr> {
	compile_node(&expression.node, root_alias, graph)
}

pub(crate) fn compile_predicate(
	expression: &SimpleExpr,
	joins: &super::node::JoinRequirements,
	root_alias: &str,
	graph: &RelationJoinGraph,
) -> Result<SimpleExpr> {
	qualify_condition(expression, joins, root_alias, graph)
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
			condition_joins,
			result,
			otherwise,
		} => {
			let case = Expr::case().when(
				qualify_condition(condition, condition_joins, root_alias, graph)?,
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

fn qualify_condition(
	condition: &SimpleExpr,
	joins: &super::node::JoinRequirements,
	root_alias: &str,
	graph: &RelationJoinGraph,
) -> Result<SimpleExpr> {
	let mut qualified = if joins.paths.is_empty() {
		crate::orm::query_fields::qualify_model_root(condition, root_alias)
	} else {
		condition.clone()
	};
	if joins.paths.len() > 1 {
		return Err(DatabaseError::new(
			DatabaseErrorKind::Query,
			"typed predicate with multiple relation paths cannot be qualified safely",
		)
		.into());
	}
	if !joins.paths.is_empty() && count_unqualified_columns(condition) > 1 {
		return Err(DatabaseError::new(
			DatabaseErrorKind::Query,
			"typed predicate mixing root and related columns cannot be qualified safely",
		)
		.into());
	}
	let Some(path) = joins.paths.first() else {
		return Ok(qualified);
	};
	let Some(alias) = graph
		.aliases_for_steps(path)
		.and_then(|aliases| aliases.last().cloned())
	else {
		return Err(DatabaseError::new(
			DatabaseErrorKind::Query,
			"typed predicate relation path could not be resolved in the query join graph",
		)
		.into());
	};
	qualify_related_columns(&mut qualified, &alias);
	Ok(qualified)
}

fn count_unqualified_columns(expression: &SimpleExpr) -> usize {
	match expression {
		SimpleExpr::Column(reinhardt_query::prelude::ColumnRef::Column(_)) => 1,
		SimpleExpr::Binary(left, _, right) => {
			count_unqualified_columns(left) + count_unqualified_columns(right)
		}
		SimpleExpr::Unary(_, value)
		| SimpleExpr::ExprAlias(value, _)
		| SimpleExpr::Cast(value, _)
		| SimpleExpr::AsEnum(_, value) => count_unqualified_columns(value),
		SimpleExpr::FunctionCall(_, values) | SimpleExpr::Tuple(values) => {
			values.iter().map(count_unqualified_columns).sum()
		}
		_ => 0,
	}
}

fn qualify_related_columns(expression: &mut SimpleExpr, alias: &str) {
	match expression {
		SimpleExpr::Column(reinhardt_query::prelude::ColumnRef::Column(column)) => {
			let column = column.clone();
			*expression = SimpleExpr::Column(reinhardt_query::prelude::ColumnRef::TableColumn(
				Alias::new(alias).into_iden(),
				column,
			));
		}
		SimpleExpr::Binary(left, _, right) => {
			qualify_related_columns(left, alias);
			qualify_related_columns(right, alias);
		}
		SimpleExpr::Unary(_, value)
		| SimpleExpr::ExprAlias(value, _)
		| SimpleExpr::Cast(value, _)
		| SimpleExpr::AsEnum(_, value) => qualify_related_columns(value, alias),
		SimpleExpr::FunctionCall(_, values) | SimpleExpr::Tuple(values) => {
			for value in values {
				qualify_related_columns(value, alias);
			}
		}
		_ => {}
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
		ExprTrait, PostgresQueryBuilder, Query, QueryStatementBuilder, SelectStatement,
		SqliteQueryBuilder,
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

	fn relation_step_named(name: &'static str, target: &'static str) -> RelationStep {
		RelationStep {
			name: name.into(),
			source_table: "authors".into(),
			target_table: target.into(),
			source_column: "author_pk".into(),
			target_column: "author_id".into(),
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
		for path in &stored.joins.paths {
			graph.add_aggregate_steps(path);
		}
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

	#[test]
	fn independent_related_paths_are_not_chained() {
		let posts = relation_step_named("posts", "posts");
		let comments = relation_step_named("comments", "comments");
		let posts_column = ExpressionNode::RelatedColumn(RelatedColumnOperand {
			relation_steps: vec![posts.clone()],
			terminal_column: "amount".to_owned(),
			storage_kind: DatabaseStorageKind::I64,
			composite_primary_key: false,
		});
		let comments_column = ExpressionNode::RelatedColumn(RelatedColumnOperand {
			relation_steps: vec![comments.clone()],
			terminal_column: "amount".to_owned(),
			storage_kind: DatabaseStorageKind::I64,
			composite_primary_key: false,
		});
		let node = ExpressionNode::Arithmetic {
			left: Box::new(ExpressionNode::Aggregate {
				operation: AggregateOperation::Sum,
				operand: Box::new(posts_column),
				distinct: false,
				output_kind: Some(super::super::kind::AggregateOutputKind::I64),
			}),
			operation: ArithmeticOperation::Add,
			right: Box::new(ExpressionNode::Aggregate {
				operation: AggregateOperation::Sum,
				operand: Box::new(comments_column),
				distinct: false,
				output_kind: Some(super::super::kind::AggregateOutputKind::I64),
			}),
		};
		let joins = JoinRequirements::from_relation_steps(vec![posts])
			.combine(JoinRequirements::from_relation_steps(vec![comments]));
		let stored = expression(node, joins);
		let mut graph = RelationJoinGraph::new("authors");
		for path in &stored.joins.paths {
			graph.add_aggregate_steps(path);
		}
		let graph = graph.with_root_alias("authors");
		let compiled = compile_expression(&stored, "authors", &graph)
			.expect("independent related paths must compile");
		let mut stmt = Query::select();
		stmt.from(Alias::new("authors")).expr(compiled);
		let sql = render(stmt.to_owned(), true);
		assert!(sql.contains("\"posts\"."));
		assert!(sql.contains("\"comments\"."));
		assert!(graph.joins().iter().any(|join| join.alias == "posts"));
		assert!(graph.joins().iter().any(|join| join.alias == "comments"));
	}

	#[test]
	fn related_typed_predicate_uses_inner_join_alias() {
		let step = relation_step();
		let joins = JoinRequirements::from_relation_steps(vec![step.clone()]);
		let mut graph = RelationJoinGraph::new("authors");
		graph.add_steps(&[step], RelationJoinKind::Inner);
		let graph = graph.with_root_alias("authors");
		let predicate = Expr::col(Alias::new("amount")).eq(Expr::val(1));
		let compiled = compile_predicate(&predicate, &joins, "authors", &graph)
			.expect("related predicate path should resolve");
		let mut stmt = Query::select();
		stmt.expr(compiled);
		let sql = stmt.to_string(PostgresQueryBuilder);
		assert!(sql.contains("\"posts\".\"amount\""));
	}

	#[test]
	fn mixed_root_and_related_predicate_fails_closed() {
		let step = relation_step();
		let joins = JoinRequirements::from_relation_steps(vec![step.clone()]);
		let mut graph = RelationJoinGraph::new("authors");
		graph.add_steps(&[step], RelationJoinKind::Inner);
		let predicate = Expr::col(Alias::new("amount"))
			.add(Expr::col(Alias::new("author_pk")))
			.eq(Expr::val(1));
		let error = compile_predicate(&predicate, &joins, "authors", &graph)
			.expect_err("mixed relation predicates must fail closed");
		assert_eq!(error.database_kind(), Some(DatabaseErrorKind::Query));
	}
}
