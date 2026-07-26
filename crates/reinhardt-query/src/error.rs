//! Errors and validation for checked query building.

use crate::{
	expr::{Condition, ConditionExpression, ConditionHolder, SimpleExpr},
	query::{
		AlterTableOperation, AlterTableStatement, CreateIndexStatement, CreateTableStatement,
		DeleteStatement, InsertSource, InsertStatement, SelectStatement, UpdateStatement,
	},
	types::{
		BinOper, ColumnDef, ColumnType, OrderExpr, OrderExprKind, PgBinOper, SchemaExpr,
		TableConstraint, TableRef, WindowStatement,
	},
	value::Value,
};

/// Error returned when a checked query build requests an unsupported backend feature.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum QueryBuildError {
	/// A query requires a feature unavailable in the selected backend.
	#[error("{feature} is not supported by the {backend} backend")]
	UnsupportedBackendFeature {
		/// The unsupported feature.
		feature: &'static str,
		/// The backend that does not support the feature.
		backend: &'static str,
	},
}

pub(crate) fn validate_select_for_backend(
	statement: &SelectStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	for cte in &statement.ctes {
		validate_select_for_backend(&cte.query, backend)?;
	}
	for select in &statement.selects {
		validate_simple_expr(&select.expr, backend)?;
	}
	for table in &statement.from {
		validate_table_ref(table, backend)?;
	}
	for join in &statement.join {
		validate_table_ref(&join.table, backend)?;
		if let Some(crate::types::JoinOn::Condition(condition)) = &join.on {
			validate_condition(condition, backend)?;
		}
	}
	validate_condition_holder(&statement.r#where, backend)?;
	for group in &statement.groups {
		validate_simple_expr(group, backend)?;
	}
	validate_condition_holder(&statement.having, backend)?;
	for (_, union) in &statement.unions {
		validate_select_for_backend(union, backend)?;
	}
	for order in &statement.orders {
		validate_order_expr(order, backend)?;
	}
	if let Some(limit) = &statement.limit {
		validate_value(limit, backend)?;
	}
	if let Some(offset) = &statement.offset {
		validate_value(offset, backend)?;
	}
	for (_, window) in &statement.windows {
		validate_window(window, backend)?;
	}
	Ok(())
}

pub(crate) fn validate_create_table_for_backend(
	statement: &CreateTableStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	for column in &statement.columns {
		validate_column_def(column, backend)?;
	}
	for constraint in &statement.constraints {
		validate_table_constraint(constraint, backend)?;
	}
	for index in &statement.indexes {
		if let Some(condition) = &index.r#where {
			validate_simple_expr(condition, backend)?;
		}
	}
	Ok(())
}

pub(crate) fn validate_create_index_for_backend(
	statement: &CreateIndexStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let Some(condition) = &statement.r#where {
		validate_simple_expr(condition, backend)?;
	}
	Ok(())
}

pub(crate) fn validate_insert_for_backend(
	statement: &InsertStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	match &statement.source {
		InsertSource::Values(rows) => {
			for row in rows {
				for value in row {
					validate_value(value, backend)?;
				}
			}
		}
		InsertSource::Subquery(query) => validate_select_for_backend(query, backend)?,
	}
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr(expression, backend)?;
		}
	}
	Ok(())
}

pub(crate) fn validate_update_for_backend(
	statement: &UpdateStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	for (_, expression) in &statement.values {
		validate_simple_expr(expression, backend)?;
	}
	validate_condition_holder(&statement.r#where, backend)?;
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr(expression, backend)?;
		}
	}
	Ok(())
}

pub(crate) fn validate_delete_for_backend(
	statement: &DeleteStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	validate_condition_holder(&statement.r#where, backend)?;
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr(expression, backend)?;
		}
	}
	Ok(())
}

pub(crate) fn validate_alter_table_for_backend(
	statement: &AlterTableStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	for operation in &statement.operations {
		match operation {
			AlterTableOperation::AddColumn(column) | AlterTableOperation::ModifyColumn(column) => {
				validate_column_def(column, backend)?;
			}
			AlterTableOperation::AddConstraint(constraint) => {
				validate_table_constraint(constraint, backend)?;
			}
			_ => {}
		}
	}
	Ok(())
}

fn unsupported(feature: &'static str, backend: &'static str) -> QueryBuildError {
	QueryBuildError::UnsupportedBackendFeature { feature, backend }
}

fn validate_column_def(column: &ColumnDef, backend: &'static str) -> Result<(), QueryBuildError> {
	if let Some(column_type) = &column.column_type {
		validate_column_type(column_type, backend)?;
	}
	if let Some(default) = &column.default {
		validate_simple_expr(default, backend)?;
	}
	if let Some(check) = &column.check {
		validate_simple_expr(check, backend)?;
	}
	if let Some(generated) = &column.generated {
		if let Some(expr) = &generated.expr {
			validate_schema_expr(expr, backend)?;
		}
	}
	Ok(())
}

fn validate_column_type(
	column_type: &ColumnType,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	match column_type {
		ColumnType::Vector(_) => Err(unsupported("pgvector column types", backend)),
		ColumnType::Array(element_type) => validate_column_type(element_type, backend),
		_ => Ok(()),
	}
}

fn validate_table_constraint(
	constraint: &TableConstraint,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let TableConstraint::Check { expr, .. } = constraint {
		validate_simple_expr(expr, backend)?;
	}
	Ok(())
}

fn validate_schema_expr(expr: &SchemaExpr, backend: &'static str) -> Result<(), QueryBuildError> {
	match expr {
		SchemaExpr::Column(_) => Ok(()),
		SchemaExpr::Value(value) => validate_value(value, backend),
		SchemaExpr::Binary { left, right, .. } => {
			validate_schema_expr(left, backend)?;
			validate_schema_expr(right, backend)
		}
		SchemaExpr::Function { args, .. } => {
			for arg in args {
				validate_schema_expr(arg, backend)?;
			}
			Ok(())
		}
		SchemaExpr::Cast { expr, ty } => {
			validate_schema_expr(expr, backend)?;
			validate_column_type(ty, backend)
		}
	}
}

fn validate_condition_holder(
	holder: &ConditionHolder,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	for condition in &holder.conditions {
		validate_condition_expression(condition, backend)?;
	}
	Ok(())
}

fn validate_condition(condition: &Condition, backend: &'static str) -> Result<(), QueryBuildError> {
	for expression in &condition.conditions {
		validate_condition_expression(expression, backend)?;
	}
	Ok(())
}

fn validate_condition_expression(
	expression: &ConditionExpression,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	match expression {
		ConditionExpression::SimpleExpr(expr) => validate_simple_expr(expr, backend),
		ConditionExpression::Condition(condition) => validate_condition(condition, backend),
	}
}

fn validate_simple_expr(expr: &SimpleExpr, backend: &'static str) -> Result<(), QueryBuildError> {
	match expr {
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Custom(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk => Ok(()),
		SimpleExpr::Value(value) => validate_value(value, backend),
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _) => validate_simple_expr(expression, backend),
		SimpleExpr::Binary(left, operator, right) => {
			if matches!(
				operator,
				BinOper::PgOperator(
					PgBinOper::L2Distance
						| PgBinOper::NegativeInnerProduct
						| PgBinOper::CosineDistance
				)
			) {
				return Err(unsupported("pgvector distance operators", backend));
			}
			validate_simple_expr(left, backend)?;
			validate_simple_expr(right, backend)
		}
		SimpleExpr::FunctionCall(_, args) | SimpleExpr::Tuple(args) => {
			for arg in args {
				validate_simple_expr(arg, backend)?;
			}
			Ok(())
		}
		SimpleExpr::SubQuery(_, query) => validate_select_for_backend(query, backend),
		SimpleExpr::CustomWithExpr(_, expressions) => {
			for expression in expressions {
				validate_simple_expr(expression, backend)?;
			}
			Ok(())
		}
		SimpleExpr::Case(case) => {
			for (condition, result) in &case.when_clauses {
				validate_simple_expr(condition, backend)?;
				validate_simple_expr(result, backend)?;
			}
			if let Some(result) = &case.else_clause {
				validate_simple_expr(result, backend)?;
			}
			Ok(())
		}
		SimpleExpr::Window { func, window } => {
			validate_simple_expr(func, backend)?;
			validate_window(window, backend)
		}
		SimpleExpr::WindowNamed { func, .. } => validate_simple_expr(func, backend),
	}
}

fn validate_value(value: &Value, backend: &'static str) -> Result<(), QueryBuildError> {
	match value {
		Value::Vector(_) => Err(unsupported("pgvector values", backend)),
		Value::Array(_, Some(values)) => {
			for value in values.iter() {
				validate_value(value, backend)?;
			}
			Ok(())
		}
		_ => Ok(()),
	}
}

fn validate_table_ref(table: &TableRef, backend: &'static str) -> Result<(), QueryBuildError> {
	if let TableRef::SubQuery(query, _) = table {
		validate_select_for_backend(query, backend)?;
	}
	Ok(())
}

fn validate_order_expr(order: &OrderExpr, backend: &'static str) -> Result<(), QueryBuildError> {
	if let OrderExprKind::Expr(expr) = &order.expr {
		validate_simple_expr(expr, backend)?;
	}
	Ok(())
}

fn validate_window(window: &WindowStatement, backend: &'static str) -> Result<(), QueryBuildError> {
	for partition in &window.partition_by {
		validate_simple_expr(partition, backend)?;
	}
	for order in &window.order_by {
		validate_order_expr(order, backend)?;
	}
	Ok(())
}
