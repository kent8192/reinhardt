//! Terminal typed aggregate planning and execution.

use super::QuerySet;
use crate::orm::Model;
use crate::orm::aggregation::{AggregateDateTime, AggregateResult, AggregateValue};
use crate::orm::connection::{DatabaseBackend, OrmExecutor, QueryValue, Row, TransactionExecutor};
use crate::orm::field_codec::DatabaseStorageKind;
use crate::orm::query_fields::expression::node::{
	ExpressionNode, StoredExpression, TypedAggregateFn,
};
use crate::orm::query_fields::expression::operand::AggregateOperation;
use crate::orm::query_fields::{AggregateKind, LabeledExpression};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error, Result};
use reinhardt_query::prelude::{Alias, Expr, Query, SelectStatement, SimpleExpr};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use std::collections::BTreeSet;
use std::str::FromStr;

mod private {
	pub trait Sealed {}
}

/// Input accepted by terminal typed aggregate execution.
pub trait AggregateInput<M>: private::Sealed {
	/// Erase result types while retaining labels and compiler metadata.
	#[doc(hidden)]
	fn into_expressions(self) -> Vec<LabeledExpression<M, AggregateKind>>;
}

impl<M> private::Sealed for LabeledExpression<M, AggregateKind> {}

impl<M> AggregateInput<M> for LabeledExpression<M, AggregateKind> {
	fn into_expressions(self) -> Vec<LabeledExpression<M, AggregateKind>> {
		vec![self]
	}
}

impl<M, const N: usize> private::Sealed for [LabeledExpression<M, AggregateKind>; N] {}

impl<M, const N: usize> AggregateInput<M> for [LabeledExpression<M, AggregateKind>; N] {
	fn into_expressions(self) -> Vec<LabeledExpression<M, AggregateKind>> {
		self.into_iter().collect()
	}
}

impl<M> private::Sealed for Vec<LabeledExpression<M, AggregateKind>> {}

impl<M> AggregateInput<M> for Vec<LabeledExpression<M, AggregateKind>> {
	fn into_expressions(self) -> Vec<LabeledExpression<M, AggregateKind>> {
		self
	}
}

impl<T> QuerySet<T>
where
	T: Model,
{
	/// Executes one or more labeled typed aggregate expressions.
	pub async fn aggregate<I>(&self, input: I) -> Result<AggregateResult>
	where
		I: AggregateInput<T>,
	{
		let expressions = input.into_expressions();
		ensure_aggregate_shape(self, &expressions)?;
		if self.empty_result {
			return Ok(empty_aggregate_result(&expressions));
		}
		let mut conn = super::super::manager::get_connection().await?;
		self.aggregate_with_db_expressions(expressions, &mut conn)
			.await
	}

	/// Executes terminal aggregates through a caller-owned ORM executor.
	pub async fn aggregate_with_db<I, E>(
		&self,
		input: I,
		executor: &mut E,
	) -> Result<AggregateResult>
	where
		I: AggregateInput<T>,
		E: OrmExecutor,
	{
		self.aggregate_with_db_expressions(input.into_expressions(), executor)
			.await
	}

	/// Executes terminal aggregates through an active transaction executor.
	pub async fn aggregate_with_executor<I>(
		&self,
		input: I,
		executor: &mut dyn TransactionExecutor,
	) -> Result<AggregateResult>
	where
		I: AggregateInput<T>,
	{
		let expressions = input.into_expressions();
		ensure_aggregate_shape(self, &expressions)?;
		if self.empty_result {
			return Ok(empty_aggregate_result(&expressions));
		}
		let stmt = build_aggregate_statement(self, &expressions)?;
		let context = super::super::execution::pgvector_context_for_select(&stmt);
		let backend = Self::executor_backend(executor);
		let (sql, values) =
			Self::build_select_for_backend(&stmt, backend, executor.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::super::execution::convert_values(values);
		let started = std::time::Instant::now();
		let query_result = executor.fetch_one_with_context(&sql, params, context).await;
		let duration = started.elapsed();
		match query_result {
			Ok(row) => {
				super::super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				decode_aggregate_row(row, &expressions, backend)
			}
			Err(error) => {
				super::super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				Err(error)
			}
		}
	}

	async fn aggregate_with_db_expressions<E>(
		&self,
		expressions: Vec<LabeledExpression<T, AggregateKind>>,
		executor: &mut E,
	) -> Result<AggregateResult>
	where
		E: OrmExecutor,
	{
		ensure_aggregate_shape(self, &expressions)?;
		if self.empty_result {
			return Ok(empty_aggregate_result(&expressions));
		}
		let stmt = build_aggregate_statement(self, &expressions)?;
		let context = super::super::execution::pgvector_context_for_select(&stmt);
		let backend = executor.backend();
		let (sql, values) =
			Self::build_select_for_backend(&stmt, backend, executor.is_cockroachdb())?;
		let param_samples = values
			.iter()
			.map(|value| value.to_sql_literal())
			.collect::<Vec<_>>();
		let params = super::super::execution::convert_values(values);
		let started = std::time::Instant::now();
		let query_result = executor.fetch_one_with_context(&sql, params, context).await;
		let duration = started.elapsed();
		let row = match query_result {
			Ok(row) => {
				super::super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &param_samples, duration)
					.await;
				row
			}
			Err(error) => {
				super::super::instrumentation::instrumentation()
					.orm_query_error(&sql, &format!("{error:?}"))
					.await;
				return Err(error);
			}
		};
		decode_aggregate_row(row, &expressions, backend)
	}
}

impl<M, Behavior> super::SelectForUpdate<M, Behavior>
where
	M: Model,
{
	/// Rejects terminal aggregation on a locking queryset before any executor call.
	pub async fn aggregate<I>(&self, input: I) -> Result<AggregateResult>
	where
		I: AggregateInput<M>,
	{
		let expressions = input.into_expressions();
		ensure_aggregate_shape(&self.queryset, &expressions)?;
		unreachable!("locking queryset validation always rejects terminal aggregates")
	}

	/// Rejects terminal aggregation on a locking queryset with a caller executor.
	pub async fn aggregate_with_db<I, E>(
		&self,
		input: I,
		_executor: &mut E,
	) -> Result<AggregateResult>
	where
		I: AggregateInput<M>,
		E: OrmExecutor,
	{
		let expressions = input.into_expressions();
		ensure_aggregate_shape(&self.queryset, &expressions)?;
		unreachable!("locking queryset validation always rejects terminal aggregates")
	}

	/// Rejects terminal aggregation on a locking queryset with a transaction executor.
	pub async fn aggregate_with_executor<I>(
		&self,
		input: I,
		_executor: &mut dyn TransactionExecutor,
	) -> Result<AggregateResult>
	where
		I: AggregateInput<M>,
	{
		let expressions = input.into_expressions();
		ensure_aggregate_shape(&self.queryset, &expressions)?;
		unreachable!("locking queryset validation always rejects terminal aggregates")
	}
}

fn ensure_aggregate_shape<T>(
	queryset: &QuerySet<T>,
	expressions: &[LabeledExpression<T, AggregateKind>],
) -> Result<()>
where
	T: Model,
{
	if expressions.is_empty() {
		return Err(Error::Validation(
			"aggregate input must contain at least one labeled expression".to_owned(),
		));
	}
	// Keep query-shape diagnostics deterministic: queryset state is checked
	// before expression metadata, in the documented annotation/group/HAVING/
	// locking order.
	if !queryset.annotations.is_empty()
		|| !queryset.backend_annotations.is_empty()
		|| !queryset.typed_annotations.is_empty()
	{
		return Err(unsupported_aggregate_shape(
			"terminal aggregate cannot run on a QuerySet containing annotations",
		));
	}
	if !queryset.group_by_fields.is_empty() {
		return Err(unsupported_aggregate_shape(
			"terminal aggregate cannot run on a QuerySet containing GROUP BY",
		));
	}
	if !queryset.typed_havings.is_empty() {
		return Err(unsupported_aggregate_shape(
			"terminal aggregate cannot run on a QuerySet containing HAVING",
		));
	}
	if queryset.select_for_update.is_some() {
		return Err(unsupported_aggregate_shape(
			"terminal aggregate cannot run on a QuerySet containing row locking",
		));
	}
	let mut labels = BTreeSet::new();
	for expression in expressions {
		if !labels.insert(expression.label()) {
			return Err(Error::Validation(format!(
				"duplicate aggregate label '{}'",
				expression.label()
			)));
		}
		let stored = expression.clone().into_stored_expression();
		if (queryset.limit.is_some() || queryset.offset.is_some())
			&& stored.joins.paths.iter().any(|path| {
				path.iter().any(|step| {
					step.multiplicity == crate::orm::relations::RelationMultiplicity::Multiple
				})
			}) {
			return Err(unsupported_aggregate_shape(
				"sliced terminal aggregates over multi-valued relations require an outer join plan",
			));
		}
		ensure_supported_terminal_expression(&stored.node)?;
	}
	if !queryset.ctes.is_empty()
		|| !queryset.lateral_joins.is_empty()
		|| queryset.from_subquery_sql.is_some()
	{
		return Err(unsupported_aggregate_shape(
			"terminal aggregate does not support CTE, LATERAL, or subquery sources",
		));
	}
	Ok(())
}

fn ensure_supported_terminal_expression(node: &ExpressionNode) -> Result<()> {
	match node {
		ExpressionNode::Aggregate {
			operation: AggregateOperation::Count,
			distinct: true,
			operand,
			..
		} if related_operand_has_composite_key(operand) => Err(unsupported_aggregate_shape(
			"COUNT(DISTINCT relation) does not support composite primary keys on PostgreSQL, MySQL, and SQLite",
		)),
		ExpressionNode::Aggregate { .. } | ExpressionNode::CountAll => Ok(()),
		_ => Err(unsupported_aggregate_shape(
			"terminal aggregate expressions must be a single aggregate function or COUNT(*)",
		)),
	}
}

fn related_operand_has_composite_key(node: &ExpressionNode) -> bool {
	match node {
		ExpressionNode::RelatedColumn(column) => column.composite_primary_key,
		_ => false,
	}
}

fn unsupported_aggregate_shape(message: &str) -> Error {
	Error::from(DatabaseError::new(DatabaseErrorKind::Unsupported, message))
}

fn build_aggregate_statement<T>(
	queryset: &QuerySet<T>,
	expressions: &[LabeledExpression<T, AggregateKind>],
) -> Result<SelectStatement>
where
	T: Model,
{
	if queryset.limit.is_some() || queryset.offset.is_some() || queryset.distinct_enabled {
		return build_sliced_aggregate_statement(queryset, expressions);
	}
	build_direct_aggregate_statement(queryset, expressions)
}

fn build_direct_aggregate_statement<T>(
	queryset: &QuerySet<T>,
	expressions: &[LabeledExpression<T, AggregateKind>],
) -> Result<SelectStatement>
where
	T: Model,
{
	let mut stmt = Query::select();
	queryset.apply_model_from(&mut stmt);

	let filter_graph = queryset.filter_relation_join_graph_for_query();
	let mut graph = filter_graph.clone();
	for expression in expressions {
		let stored = expression.clone().into_stored_expression();
		for path in &stored.joins.paths {
			graph.add_aggregate_steps(path);
		}
	}
	let graph = graph.with_root_alias_and_reserved_aliases(
		queryset.root_alias(),
		queryset.manual_join_aliases(),
	);
	for expression in expressions {
		let stored = expression.clone().into_stored_expression();
		let compiled = super::super::query_fields::expression::compiler::compile_expression(
			&stored,
			queryset.root_alias(),
			&graph,
		)?;
		stmt.expr_as(compiled, Alias::new(expression.label()));
	}
	QuerySet::<T>::apply_relation_join_graph(&mut stmt, &graph);
	queryset.apply_manual_joins(&mut stmt);

	// Build WHERE against the filter-only graph. This keeps select_related joins
	// out of terminal aggregate SQL while preserving filter relation aliases.
	let mut where_queryset = queryset.clone();
	where_queryset.relation_joins = filter_graph;
	if let Some(condition) = where_queryset.build_where_condition()? {
		stmt.cond_where(condition);
	}
	Ok(stmt.to_owned())
}

/// Builds a terminal aggregate over a sliced or distinct queryset.
///
/// SQL applies LIMIT/OFFSET and query-level DISTINCT before aggregate
/// evaluation, so these query modifiers must be retained in a derived table.
fn build_sliced_aggregate_statement<T>(
	queryset: &QuerySet<T>,
	expressions: &[LabeledExpression<T, AggregateKind>],
) -> Result<SelectStatement>
where
	T: Model,
{
	const SOURCE_ALIAS: &str = "__reinhardt_aggregate_source";
	let mut filter_graph = queryset.filter_relation_join_graph_for_query();
	let mut operands: Vec<(StoredExpression, Option<StoredExpression>)> =
		Vec::with_capacity(expressions.len());
	for (index, expression) in expressions.iter().enumerate() {
		let stored = expression.clone().into_stored_expression();
		match &stored.node {
			ExpressionNode::Aggregate { operand, .. } => {
				for path in &stored.joins.paths {
					filter_graph.add_aggregate_steps(path);
				}
				let operand = StoredExpression::new(
					(*operand.clone()).clone(),
					stored.joins.clone(),
					Some(format!("__reinhardt_aggregate_operand_{index}")),
				);
				operands.push((stored, Some(operand)));
			}
			ExpressionNode::CountAll => operands.push((stored, None)),
			_ => {
				return Err(unsupported_aggregate_shape(
					"terminal aggregate expressions must be a single aggregate function or COUNT(*)",
				));
			}
		}
	}
	let graph = filter_graph.with_root_alias_and_reserved_aliases(
		queryset.root_alias(),
		queryset.manual_join_aliases(),
	);

	let mut inner = Query::select();
	queryset.apply_model_from(&mut inner);
	if queryset.distinct_enabled {
		inner.distinct();
	}
	// Preserve root identity in the inner rowset. This also keeps DISTINCT
	// deterministic for models with composite primary keys.
	for column in queryset.root_primary_key_columns() {
		inner.column(column);
	}
	for (_, operand) in &operands {
		if let Some(operand) = operand {
			let alias = operand
				.label
				.as_deref()
				.expect("derived aggregate operands always have compiler labels");
			inner.expr_as(
				super::super::query_fields::expression::compiler::compile_expression(
					operand,
					queryset.root_alias(),
					&graph,
				)?,
				Alias::new(alias),
			);
		}
	}
	if queryset.distinct_enabled {
		// PostgreSQL requires every DISTINCT ordering expression to be present
		// in the inner projection. These columns are intentionally unaliased:
		// they are ordering support only and are not visible to the outer query.
		for order_field in &queryset.order_by_fields {
			let field = order_field.strip_prefix('-').unwrap_or(order_field);
			inner.column(queryset.root_column_reference(field));
		}
		for ordering in &queryset.order_by_expressions {
			inner.expr(
				super::super::query_fields::expression::compiler::compile_expression(
					&ordering.expression,
					queryset.root_alias(),
					&graph,
				)?,
			);
		}
	}
	QuerySet::<T>::apply_relation_join_graph(&mut inner, &graph);
	queryset.apply_manual_joins(&mut inner);

	// The filter graph is deliberately used here: eager-loading relations and
	// annotations are not part of the terminal aggregate's source rowset.
	let mut where_queryset = queryset.clone();
	where_queryset.relation_joins = queryset.filter_relation_join_graph_for_query();
	if let Some(condition) = where_queryset.build_where_condition()? {
		inner.cond_where(condition);
	}
	queryset.apply_ordering(&mut inner)?;
	if let Some(limit) = queryset.limit {
		inner.limit(limit as u64);
	}
	if let Some(offset) = queryset.offset {
		inner.offset(offset as u64);
	}

	let mut outer = Query::select();
	outer.from_subquery(inner.to_owned(), Alias::new(SOURCE_ALIAS));
	for (expression, operand) in &operands {
		let alias = expression
			.label
			.as_deref()
			.expect("aggregate expressions always retain validated labels");
		let projected = match (&expression.node, operand) {
			(ExpressionNode::CountAll, _) => Expr::cust("COUNT(*)").into_simple_expr(),
			(
				ExpressionNode::Aggregate {
					operation,
					distinct,
					..
				},
				Some(operand),
			) => {
				let operand_alias = operand
					.label
					.as_deref()
					.expect("derived aggregate operands always have compiler labels");
				let column = Expr::col((Alias::new(SOURCE_ALIAS), Alias::new(operand_alias)))
					.into_simple_expr();
				let column = if *distinct {
					SimpleExpr::CustomWithExpr("DISTINCT ?".to_owned(), vec![column])
				} else {
					column
				};
				match operation {
					crate::orm::query_fields::expression::operand::AggregateOperation::Count => {
						reinhardt_query::prelude::Func::count(column)
					}
					crate::orm::query_fields::expression::operand::AggregateOperation::Sum => {
						reinhardt_query::prelude::Func::sum(column)
					}
					crate::orm::query_fields::expression::operand::AggregateOperation::Average => {
						reinhardt_query::prelude::Func::avg(column)
					}
					crate::orm::query_fields::expression::operand::AggregateOperation::Minimum => {
						reinhardt_query::prelude::Func::min(column)
					}
					crate::orm::query_fields::expression::operand::AggregateOperation::Maximum => {
						reinhardt_query::prelude::Func::max(column)
					}
				}
			}
			_ => {
				return Err(unsupported_aggregate_shape(
					"terminal aggregate expression could not be projected through the derived table",
				));
			}
		};
		outer.expr_as(projected, Alias::new(alias));
	}
	Ok(outer.to_owned())
}

fn empty_aggregate_result<T>(
	expressions: &[LabeledExpression<T, AggregateKind>],
) -> AggregateResult {
	let mut result = AggregateResult::new();
	for expression in expressions {
		let stored = expression.clone().into_stored_expression();
		let value = if stored.aggregate_function == Some(TypedAggregateFn::Count) {
			AggregateValue::Integer(0)
		} else {
			AggregateValue::Null
		};
		result.insert(expression.label(), value);
	}
	result
}

fn decode_aggregate_row<T>(
	row: Row,
	expressions: &[LabeledExpression<T, AggregateKind>],
	backend: DatabaseBackend,
) -> Result<AggregateResult> {
	let mut result = AggregateResult::new();
	for expression in expressions {
		let stored = expression.clone().into_stored_expression();
		let function = stored.aggregate_function.ok_or_else(|| {
			serialization_error(
				"UNKNOWN",
				expression.label(),
				backend,
				"expression does not contain a standard aggregate",
			)
		})?;
		let raw = row.data.get(expression.label()).cloned().ok_or_else(|| {
			serialization_error(
				function_name(function),
				expression.label(),
				backend,
				"database row did not contain the projected label",
			)
		})?;
		let value = normalize_aggregate_value(&stored, raw, expression.label(), function, backend)?;
		result.insert(expression.label(), value);
	}
	Ok(result)
}

fn normalize_aggregate_value(
	stored: &StoredExpression,
	raw: QueryValue,
	label: &str,
	function: TypedAggregateFn,
	backend: DatabaseBackend,
) -> Result<AggregateValue> {
	if matches!(raw, QueryValue::Null) {
		if function == TypedAggregateFn::Count {
			return Err(serialization_error(
				function_name(function),
				label,
				backend,
				"COUNT returned SQL NULL",
			));
		}
		return Ok(AggregateValue::Null);
	}
	match function {
		TypedAggregateFn::Count => match raw {
			QueryValue::Int(value) => Ok(AggregateValue::Integer(value)),
			other => Err(unexpected_value_error(
				function_name(function),
				label,
				backend,
				other,
				"Integer",
			)),
		},
		TypedAggregateFn::Sum | TypedAggregateFn::Avg => match stored.output {
			Some(crate::orm::query_fields::AggregateOutputKind::I64) => {
				integer_sum(raw, label, function, backend)
			}
			Some(crate::orm::query_fields::AggregateOutputKind::F64) => {
				float_aggregate(raw, label, function, backend)
			}
			Some(crate::orm::query_fields::AggregateOutputKind::Decimal) => {
				decimal_aggregate(raw, label, function, backend)
			}
			None => Err(serialization_error(
				function_name(function),
				label,
				backend,
				"aggregate output storage kind is missing",
			)),
		},
		TypedAggregateFn::Min | TypedAggregateFn::Max => {
			let storage_kind = stored.aggregate_storage_kind.ok_or_else(|| {
				serialization_error(
					function_name(function),
					label,
					backend,
					"aggregate operand storage kind is missing",
				)
			})?;
			normalize_storage_value(raw, storage_kind, label, function, backend)
		}
	}
}

fn integer_sum(
	raw: QueryValue,
	label: &str,
	function: TypedAggregateFn,
	backend: DatabaseBackend,
) -> Result<AggregateValue> {
	let value = match raw {
		QueryValue::Int(value) => Some(value),
		QueryValue::String(value) => rust_decimal::Decimal::from_str(&value)
			.ok()
			.and_then(|value| value.to_i64()),
		_ => None,
	};
	value.map(AggregateValue::Integer).ok_or_else(|| {
		serialization_error(
			function_name(function),
			label,
			backend,
			"integer aggregate value is out of range or malformed",
		)
	})
}

fn float_aggregate(
	raw: QueryValue,
	label: &str,
	function: TypedAggregateFn,
	backend: DatabaseBackend,
) -> Result<AggregateValue> {
	let value = match raw {
		QueryValue::Int(value) => Some(value as f64),
		QueryValue::Float(value) if value.is_finite() => Some(value),
		QueryValue::String(value) => rust_decimal::Decimal::from_str(&value)
			.ok()
			.and_then(|value| value.to_f64())
			.filter(|value| value.is_finite()),
		_ => None,
	};
	value.map(AggregateValue::Float).ok_or_else(|| {
		serialization_error(
			function_name(function),
			label,
			backend,
			"floating aggregate value is malformed or not finite",
		)
	})
}

fn decimal_aggregate(
	raw: QueryValue,
	label: &str,
	function: TypedAggregateFn,
	backend: DatabaseBackend,
) -> Result<AggregateValue> {
	let value = match raw {
		QueryValue::Int(value) => Some(rust_decimal::Decimal::from(value)),
		QueryValue::String(value) => rust_decimal::Decimal::from_str(&value).ok(),
		QueryValue::Float(value) if value.is_finite() => rust_decimal::Decimal::from_f64(value),
		_ => None,
	};
	value.map(AggregateValue::Decimal).ok_or_else(|| {
		serialization_error(
			function_name(function),
			label,
			backend,
			"decimal aggregate value is malformed",
		)
	})
}

fn normalize_storage_value(
	raw: QueryValue,
	storage_kind: DatabaseStorageKind,
	label: &str,
	function: TypedAggregateFn,
	backend: DatabaseBackend,
) -> Result<AggregateValue> {
	let unexpected = |expected: &str, raw: QueryValue| {
		unexpected_value_error(function_name(function), label, backend, raw, expected)
	};
	match storage_kind {
		DatabaseStorageKind::Bool => match raw {
			QueryValue::Bool(value) => Ok(AggregateValue::Bool(value)),
			raw => Err(unexpected("Bool", raw)),
		},
		DatabaseStorageKind::I32 | DatabaseStorageKind::I64 => match raw {
			QueryValue::Int(value) => Ok(AggregateValue::Integer(value)),
			QueryValue::String(value) => value
				.parse::<i64>()
				.map(AggregateValue::Integer)
				.map_err(|_| unexpected("Integer", QueryValue::String(value))),
			raw => Err(unexpected("Integer", raw)),
		},
		DatabaseStorageKind::F32 | DatabaseStorageKind::F64 => match raw {
			QueryValue::Int(value) => Ok(AggregateValue::Float(value as f64)),
			QueryValue::Float(value) if value.is_finite() => Ok(AggregateValue::Float(value)),
			raw => Err(unexpected("Float", raw)),
		},
		DatabaseStorageKind::Decimal => match raw {
			QueryValue::Int(value) => {
				Ok(AggregateValue::Decimal(rust_decimal::Decimal::from(value)))
			}
			QueryValue::String(value) => rust_decimal::Decimal::from_str(&value)
				.map(AggregateValue::Decimal)
				.map_err(|_| unexpected("Decimal", QueryValue::String(value))),
			raw => Err(unexpected("Decimal", raw)),
		},
		DatabaseStorageKind::String => match raw {
			QueryValue::String(value) => Ok(AggregateValue::String(value)),
			raw => Err(unexpected("String", raw)),
		},
		DatabaseStorageKind::Bytes => match raw {
			QueryValue::Bytes(value) => Ok(AggregateValue::Bytes(value)),
			raw => Err(unexpected("Bytes", raw)),
		},
		DatabaseStorageKind::Json => match raw {
			QueryValue::Json(Some(value)) => Ok(AggregateValue::Json(*value)),
			QueryValue::Json(None) => Ok(AggregateValue::Null),
			raw => Err(unexpected("Json", raw)),
		},
		DatabaseStorageKind::Uuid => match raw {
			QueryValue::Uuid(value) => Ok(AggregateValue::Uuid(value)),
			QueryValue::String(value) => uuid::Uuid::parse_str(&value)
				.map(AggregateValue::Uuid)
				.map_err(|_| unexpected("Uuid", QueryValue::String(value))),
			raw => Err(unexpected("Uuid", raw)),
		},
		DatabaseStorageKind::Date => match raw {
			QueryValue::String(value) => NaiveDate::parse_from_str(&value, "%Y-%m-%d")
				.map(AggregateValue::Date)
				.map_err(|_| unexpected("Date", QueryValue::String(value))),
			raw => Err(unexpected("Date", raw)),
		},
		DatabaseStorageKind::Time => match raw {
			QueryValue::String(value) => parse_naive_time(&value)
				.map(AggregateValue::Time)
				.map_err(|_| unexpected("Time", QueryValue::String(value))),
			raw => Err(unexpected("Time", raw)),
		},
		DatabaseStorageKind::DateTime => match raw {
			QueryValue::Timestamp(value) => {
				Ok(AggregateValue::DateTime(AggregateDateTime::Utc(value)))
			}
			QueryValue::NaiveTimestamp(value) => Ok(AggregateValue::DateTime(
				AggregateDateTime::Utc(value.and_utc()),
			)),
			QueryValue::String(value) => parse_datetime(&value)
				.map(|value| AggregateValue::DateTime(AggregateDateTime::Utc(value)))
				.map_err(|_| unexpected("DateTime", QueryValue::String(value))),
			raw => Err(unexpected("DateTime", raw)),
		},
		DatabaseStorageKind::NaiveDateTime => match raw {
			QueryValue::NaiveTimestamp(value) => {
				Ok(AggregateValue::DateTime(AggregateDateTime::Naive(value)))
			}
			QueryValue::String(value) => {
				NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S%.f")
					.or_else(|_| NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M:%S%.f"))
					.map(|value| AggregateValue::DateTime(AggregateDateTime::Naive(value)))
					.map_err(|_| unexpected("DateTime", QueryValue::String(value)))
			}
			raw => Err(unexpected("DateTime", raw)),
		},
		#[cfg(feature = "pgvector")]
		DatabaseStorageKind::Vector(_) => Err(unexpected("Vector", raw)),
	}
}

fn parse_naive_time(value: &str) -> std::result::Result<NaiveTime, chrono::ParseError> {
	NaiveTime::parse_from_str(value, "%H:%M:%S%.f")
		.or_else(|_| NaiveTime::parse_from_str(value, "%H:%M:%S"))
}

fn parse_datetime(value: &str) -> std::result::Result<DateTime<Utc>, chrono::ParseError> {
	DateTime::parse_from_rfc3339(value).map(|value| value.with_timezone(&Utc))
}

fn function_name(function: TypedAggregateFn) -> &'static str {
	match function {
		TypedAggregateFn::Count => "COUNT",
		TypedAggregateFn::Sum => "SUM",
		TypedAggregateFn::Avg => "AVG",
		TypedAggregateFn::Min => "MIN",
		TypedAggregateFn::Max => "MAX",
	}
}

fn serialization_error(
	function: &str,
	label: &str,
	backend: DatabaseBackend,
	detail: &str,
) -> Error {
	Error::Serialization(format!(
		"aggregate function {function} for label '{label}' on backend {backend:?}: {detail}"
	))
}

fn unexpected_value_error(
	function: &str,
	label: &str,
	backend: DatabaseBackend,
	raw: QueryValue,
	expected: &str,
) -> Error {
	serialization_error(
		function,
		label,
		backend,
		&format!("database returned {raw:?}, expected {expected}"),
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn normalize(storage_kind: DatabaseStorageKind, raw: QueryValue) -> AggregateValue {
		normalize_storage_value(
			raw,
			storage_kind,
			"value",
			TypedAggregateFn::Min,
			DatabaseBackend::Postgres,
		)
		.expect("fixture value should match its storage kind")
	}

	#[test]
	fn normalize_non_numeric_storage_variants() {
		assert_eq!(
			normalize(DatabaseStorageKind::F64, QueryValue::Float(4.25)),
			AggregateValue::Float(4.25)
		);
		assert_eq!(
			normalize(
				DatabaseStorageKind::Decimal,
				QueryValue::String("42.50".to_owned()),
			),
			AggregateValue::Decimal(rust_decimal::Decimal::new(4250, 2))
		);
		assert_eq!(
			normalize(DatabaseStorageKind::Decimal, QueryValue::Int(42)),
			AggregateValue::Decimal(rust_decimal::Decimal::from(42))
		);
		assert_eq!(
			normalize(DatabaseStorageKind::Bool, QueryValue::Bool(true)),
			AggregateValue::Bool(true)
		);
		assert_eq!(
			normalize(
				DatabaseStorageKind::Bytes,
				QueryValue::Bytes(vec![1, 2, 255])
			),
			AggregateValue::Bytes(vec![1, 2, 255])
		);
		let json = serde_json::json!({"stage": "ready"});
		assert_eq!(
			normalize(
				DatabaseStorageKind::Json,
				QueryValue::Json(Some(Box::new(json.clone()))),
			),
			AggregateValue::Json(json)
		);
		assert_eq!(
			normalize(DatabaseStorageKind::Json, QueryValue::Json(None)),
			AggregateValue::Null
		);
	}

	#[test]
	fn normalize_datetime_accepts_naive_driver_timestamp() {
		let naive = NaiveDate::from_ymd_opt(2024, 1, 2)
			.expect("valid date")
			.and_hms_opt(3, 4, 5)
			.expect("valid time");

		assert_eq!(
			normalize(
				DatabaseStorageKind::DateTime,
				QueryValue::NaiveTimestamp(naive)
			),
			AggregateValue::DateTime(AggregateDateTime::Utc(naive.and_utc()))
		);
	}

	#[test]
	fn rejects_distinct_composite_relation_operand() {
		let operand = ExpressionNode::RelatedColumn(
			crate::orm::query_fields::expression::node::RelatedColumnOperand {
				relation_steps: Vec::new(),
				terminal_column: "value".to_owned(),
				storage_kind: DatabaseStorageKind::I64,
				composite_primary_key: true,
			},
		);
		let expression = ExpressionNode::Aggregate {
			operation: crate::orm::query_fields::expression::operand::AggregateOperation::Count,
			operand: Box::new(operand),
			distinct: true,
			output_kind: None,
		};
		let error = ensure_supported_terminal_expression(&expression)
			.expect_err("composite relation DISTINCT must be rejected");
		assert_eq!(
			error.database_error().expect("database error").message(),
			"COUNT(DISTINCT relation) does not support composite primary keys on PostgreSQL, MySQL, and SQLite"
		);
	}
}
