//! Errors and validation for checked query building.

#[cfg(feature = "pgvector")]
use crate::types::{BinOper, PgBinOper};
use crate::{
	expr::{Condition, ConditionExpression, ConditionHolder, SimpleExpr},
	query::{
		AlterTableOperation, AlterTableStatement, CreateIndexStatement, CreateTableStatement,
		DeleteStatement, InsertSource, InsertStatement, SelectDistinct, SelectStatement,
		UpdateStatement,
	},
	types::{
		ColumnDef, ColumnType, JoinType, OrderExpr, OrderExprKind, SchemaExpr, TableConstraint,
		TableRef, WindowStatement,
	},
	value::Value,
};

/// Error returned when a checked query build requests an unsupported backend feature.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum QueryBuildError {
	/// A temporal truncation unit cannot produce the requested result type.
	#[error("temporal truncation {kind} cannot produce a {output} value")]
	InvalidTemporalTruncation {
		/// The requested truncation unit.
		kind: &'static str,
		/// The requested result type.
		output: &'static str,
	},
	/// A query requires a feature unavailable in the selected backend.
	#[error("{feature} is not supported by the {backend} backend")]
	UnsupportedBackendFeature {
		/// The unsupported feature.
		feature: &'static str,
		/// The backend that does not support the feature.
		backend: &'static str,
	},
	/// A pgvector column type uses an unsupported number of dimensions.
	#[error("pgvector dimensions must be in the range 1..=2000; got {dimensions}")]
	InvalidPgvectorDimensions {
		/// The requested vector dimension count.
		dimensions: u32,
	},
}

/// A pgvector feature found through structural query inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PgvectorFeature {
	/// A PostgreSQL vector column type.
	ColumnType,
	/// A PostgreSQL vector distance operator.
	DistanceOperator,
	/// An HNSW or IVFFlat index definition.
	ApproximateIndex,
	/// A bound PostgreSQL vector value.
	VectorValue,
}

/// Set of pgvector features found through structural query inspection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PgvectorFeatureSet(u8);

impl PgvectorFeatureSet {
	const COLUMN_TYPE: u8 = 1 << 0;
	const DISTANCE_OPERATOR: u8 = 1 << 1;
	const APPROXIMATE_INDEX: u8 = 1 << 2;
	const VECTOR_VALUE: u8 = 1 << 3;

	/// Returns whether the set contains a feature.
	pub const fn contains(self, feature: PgvectorFeature) -> bool {
		let flag = match feature {
			PgvectorFeature::ColumnType => Self::COLUMN_TYPE,
			PgvectorFeature::DistanceOperator => Self::DISTANCE_OPERATOR,
			PgvectorFeature::ApproximateIndex => Self::APPROXIMATE_INDEX,
			PgvectorFeature::VectorValue => Self::VECTOR_VALUE,
		};
		self.0 & flag != 0
	}

	fn insert(&mut self, feature: PgvectorFeature) {
		self.0 |= match feature {
			PgvectorFeature::ColumnType => Self::COLUMN_TYPE,
			PgvectorFeature::DistanceOperator => Self::DISTANCE_OPERATOR,
			PgvectorFeature::ApproximateIndex => Self::APPROXIMATE_INDEX,
			PgvectorFeature::VectorValue => Self::VECTOR_VALUE,
		};
	}
}

/// Returns the first pgvector feature found in a select AST.
pub fn select_pgvector_feature(statement: &SelectStatement) -> Option<PgvectorFeature> {
	let features = select_pgvector_features(statement);
	[
		PgvectorFeature::ColumnType,
		PgvectorFeature::DistanceOperator,
		PgvectorFeature::ApproximateIndex,
		PgvectorFeature::VectorValue,
	]
	.into_iter()
	.find(|feature| features.contains(*feature))
}

/// Returns every pgvector feature found in a select AST.
pub fn select_pgvector_features(statement: &SelectStatement) -> PgvectorFeatureSet {
	let mut features = PgvectorFeatureSet::default();
	collect_select_pgvector_features(statement, &mut features);
	features
}

/// Returns the first pgvector feature found in an insert AST.
pub fn insert_pgvector_feature(statement: &InsertStatement) -> Option<PgvectorFeature> {
	let features = insert_pgvector_features(statement);
	[
		PgvectorFeature::ColumnType,
		PgvectorFeature::DistanceOperator,
		PgvectorFeature::ApproximateIndex,
		PgvectorFeature::VectorValue,
	]
	.into_iter()
	.find(|feature| features.contains(*feature))
}

/// Returns every pgvector feature found in an insert AST.
pub fn insert_pgvector_features(statement: &InsertStatement) -> PgvectorFeatureSet {
	let mut features = PgvectorFeatureSet::default();
	collect_insert_pgvector_features(statement, &mut features);
	features
}

/// Returns the first pgvector feature found in an update AST.
pub fn update_pgvector_feature(statement: &UpdateStatement) -> Option<PgvectorFeature> {
	let features = update_pgvector_features(statement);
	[
		PgvectorFeature::ColumnType,
		PgvectorFeature::DistanceOperator,
		PgvectorFeature::ApproximateIndex,
		PgvectorFeature::VectorValue,
	]
	.into_iter()
	.find(|feature| features.contains(*feature))
}

/// Returns every pgvector feature found in an update AST.
pub fn update_pgvector_features(statement: &UpdateStatement) -> PgvectorFeatureSet {
	let mut features = PgvectorFeatureSet::default();
	collect_update_pgvector_features(statement, &mut features);
	features
}

pub(crate) fn validate_select_for_backend(
	statement: &SelectStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if backend != "PostgreSQL"
		&& matches!(statement.distinct, Some(SelectDistinct::DistinctOn(_)))
		&& backend != "CockroachDB"
	{
		return Err(QueryBuildError::UnsupportedBackendFeature {
			feature: "DISTINCT ON",
			backend,
		});
	}
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
		if backend == "MySQL" && matches!(join.join, crate::types::JoinType::FullOuterJoin) {
			return Err(unsupported("FULL OUTER JOIN", backend));
		}
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

pub(crate) fn validate_select_lock_for_backend(
	statement: &SelectStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	validate_select_lock_for_backend_with_union_context(statement, backend, &[], false, false)
}

fn validate_select_lock_for_backend_with_union_context(
	statement: &SelectStatement,
	backend: &'static str,
	visible_cte_names: &[String],
	in_union_arm: bool,
	inherited_lock: bool,
) -> Result<(), QueryBuildError> {
	let mut visible_cte_names = visible_cte_names.to_vec();
	for cte in &statement.ctes {
		validate_select_lock_for_backend_with_union_context(
			&cte.query,
			backend,
			&visible_cte_names,
			false,
			false,
		)?;
		visible_cte_names.push(cte.name.to_string());
	}
	let lock_applies_to_derived = backend == "PostgreSQL"
		&& (inherited_lock
			|| statement
				.lock
				.as_ref()
				.is_some_and(|lock| lock.tables.is_empty()));
	for select in &statement.selects {
		validate_simple_expr_lock(&select.expr, backend, &visible_cte_names)?;
	}
	for table in &statement.from {
		validate_table_ref_lock(table, backend, &visible_cte_names, lock_applies_to_derived)?;
	}
	for join in &statement.join {
		validate_table_ref_lock(
			&join.table,
			backend,
			&visible_cte_names,
			lock_applies_to_derived,
		)?;
		if let Some(crate::types::JoinOn::Condition(condition)) = &join.on {
			validate_condition_lock(condition, backend, &visible_cte_names)?;
		}
	}
	validate_condition_holder_lock(&statement.r#where, backend, &visible_cte_names)?;
	for group in &statement.groups {
		validate_simple_expr_lock(group, backend, &visible_cte_names)?;
	}
	validate_condition_holder_lock(&statement.having, backend, &visible_cte_names)?;
	for (_, union) in &statement.unions {
		validate_select_lock_for_backend_with_union_context(
			union,
			backend,
			&visible_cte_names,
			true,
			false,
		)?;
	}
	for order in &statement.orders {
		if let OrderExprKind::Expr(expr) = &order.expr {
			validate_simple_expr_lock(expr, backend, &visible_cte_names)?;
		}
	}
	for (_, window) in &statement.windows {
		validate_window_lock(window, backend, &visible_cte_names)?;
	}

	let lock = statement.lock.as_ref();
	if lock.is_none() && !inherited_lock {
		return Ok(());
	}

	match backend {
		"PostgreSQL" => {
			let lock_tables = lock.map_or(&[][..], |lock| lock.tables.as_slice());
			if in_union_arm || !statement.unions.is_empty() {
				return Err(unsupported("row locking on UNION queries", backend));
			}
			if statement_lock_targets_cte(statement, lock_tables, &visible_cte_names) {
				return Err(unsupported("row locking on CTE-backed queries", backend));
			}
			if statement.distinct.is_some() {
				return Err(unsupported("row locking with DISTINCT queries", backend));
			}
			if !statement.groups.is_empty() || !statement.having.conditions.is_empty() {
				return Err(unsupported(
					"row locking with GROUP BY or HAVING queries",
					backend,
				));
			}
			if statement
				.selects
				.iter()
				.any(|select| contains_aggregate(&select.expr))
				|| statement.orders.iter().any(
					|order| matches!(&order.expr, OrderExprKind::Expr(expr) if contains_aggregate(expr)),
				) {
				return Err(unsupported("row locking with aggregate queries", backend));
			}
			if statement
				.selects
				.iter()
				.any(|select| contains_window(&select.expr))
				|| statement.orders.iter().any(
					|order| matches!(&order.expr, OrderExprKind::Expr(expr) if contains_window(expr)),
				) {
				return Err(unsupported(
					"row locking with window-function queries",
					backend,
				));
			}
			if (inherited_lock || lock.is_some_and(|lock| lock.tables.is_empty()))
				&& statement.join.iter().any(|join| {
					matches!(
						join.join,
						JoinType::LeftJoin | JoinType::RightJoin | JoinType::FullOuterJoin
					)
				}) {
				return Err(unsupported(
					"row locking across outer joins without explicit targets",
					backend,
				));
			}
			if let Some(lock) = lock {
				validate_lock_tables_belong_to_statement(
					statement,
					lock.tables.as_slice(),
					backend,
				)?;
				validate_lock_tables_are_not_nullable(statement, lock.tables.as_slice(), backend)?;
			}
			Ok(())
		}
		"MySQL" => {
			let Some(lock) = lock else {
				return Ok(());
			};
			if in_union_arm || !statement.unions.is_empty() {
				return Err(unsupported("row locking on UNION queries", backend));
			}
			if matches!(
				lock.r#type,
				crate::query::LockType::NoKeyUpdate | crate::query::LockType::KeyShare
			) {
				return Err(unsupported("the requested row lock strength", backend));
			}
			validate_lock_tables_belong_to_statement(statement, lock.tables.as_slice(), backend)?;
			Ok(())
		}
		"SQLite" => Err(unsupported("row locking", backend)),
		"CockroachDB" => {
			let Some(lock) = lock else {
				return Ok(());
			};
			if matches!(lock.r#type, crate::query::LockType::NoKeyUpdate) {
				return Err(unsupported("the requested row lock strength", backend));
			}
			if matches!(lock.behavior, Some(crate::query::LockBehavior::SkipLocked)) {
				return Err(unsupported("SKIP LOCKED row locking", backend));
			}
			if lock.tables.is_empty() {
				Ok(())
			} else {
				Err(unsupported("row lock table targets", backend))
			}
		}
		_ => Ok(()),
	}
}

fn statement_lock_targets_cte(
	statement: &SelectStatement,
	targets: &[TableRef],
	visible_cte_names: &[String],
) -> bool {
	let cte_relations = statement
		.from
		.iter()
		.chain(statement.join.iter().map(|join| &join.table))
		.filter(|table| table_ref_references_cte(table, visible_cte_names))
		.collect::<Vec<_>>();
	if cte_relations.is_empty() {
		return false;
	}
	if targets.is_empty() {
		return true;
	}
	targets.iter().flat_map(table_ref_lock_names).any(|target| {
		cte_relations.iter().any(|cte_relation| {
			table_ref_lock_names(cte_relation)
				.iter()
				.any(|cte_name| cte_name == &target)
		})
	})
}

fn table_ref_references_cte(table: &TableRef, cte_names: &[String]) -> bool {
	let source_name = match table {
		TableRef::Table(name) | TableRef::TableAlias(name, _) => name.to_string(),
		TableRef::SchemaTable(_, _)
		| TableRef::DatabaseSchemaTable(_, _, _)
		| TableRef::SchemaTableAlias(_, _, _)
		| TableRef::SubQuery(_, _) => return false,
	};
	cte_names.iter().any(|cte_name| cte_name == &source_name)
}

fn contains_aggregate(expr: &SimpleExpr) -> bool {
	match expr {
		SimpleExpr::FunctionCall(name, arguments) => {
			matches!(
				name.to_string().to_ascii_uppercase().as_str(),
				"COUNT"
					| "SUM" | "AVG" | "MIN"
					| "MAX" | "ARRAY_AGG"
					| "JSON_AGG" | "JSONB_AGG"
					| "STRING_AGG" | "BOOL_AND"
					| "BOOL_OR" | "EVERY"
					| "XMLAGG"
			) || arguments.iter().any(contains_aggregate)
		}
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::TemporalTrunc {
			expr: expression, ..
		} => contains_aggregate(expression),
		SimpleExpr::Binary(left, _, right) => contains_aggregate(left) || contains_aggregate(right),
		SimpleExpr::Tuple(expressions) | SimpleExpr::CustomWithExpr(_, expressions) => {
			expressions.iter().any(contains_aggregate)
		}
		SimpleExpr::Case(case) => {
			case.when_clauses.iter().any(|(condition, result)| {
				contains_aggregate(condition) || contains_aggregate(result)
			}) || case.else_clause.as_ref().is_some_and(contains_aggregate)
		}
		SimpleExpr::Window { func, window } => {
			contains_aggregate(func)
				|| window.partition_by.iter().any(contains_aggregate)
				|| window.order_by.iter().any(
					|order| matches!(&order.expr, OrderExprKind::Expr(expr) if contains_aggregate(expr)),
				)
		}
		SimpleExpr::WindowNamed { func, .. } => contains_aggregate(func),
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Value(_)
		| SimpleExpr::Custom(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk
		| SimpleExpr::SubQuery(_, _) => false,
	}
}

fn contains_window(expr: &SimpleExpr) -> bool {
	match expr {
		SimpleExpr::Window { .. } | SimpleExpr::WindowNamed { .. } => true,
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::TemporalTrunc {
			expr: expression, ..
		} => contains_window(expression),
		SimpleExpr::Binary(left, _, right) => contains_window(left) || contains_window(right),
		SimpleExpr::FunctionCall(_, arguments)
		| SimpleExpr::Tuple(arguments)
		| SimpleExpr::CustomWithExpr(_, arguments) => arguments.iter().any(contains_window),
		SimpleExpr::Case(case) => {
			case.when_clauses
				.iter()
				.any(|(condition, result)| contains_window(condition) || contains_window(result))
				|| case.else_clause.as_ref().is_some_and(contains_window)
		}
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Value(_)
		| SimpleExpr::Custom(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk
		| SimpleExpr::SubQuery(_, _) => false,
	}
}

fn validate_lock_tables_belong_to_statement(
	statement: &SelectStatement,
	targets: &[TableRef],
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	let mut relations = Vec::with_capacity(statement.from.len() + statement.join.len());
	for table in &statement.from {
		relations.extend(
			table_ref_lock_names(table)
				.into_iter()
				.map(|name| (name, table_ref_is_derived(table))),
		);
	}
	for join in &statement.join {
		relations.extend(
			table_ref_lock_names(&join.table)
				.into_iter()
				.map(|name| (name, table_ref_is_derived(&join.table))),
		);
	}
	for target in targets {
		if table_ref_is_derived(target)
			|| !table_ref_lock_names(target).iter().any(|target_name| {
				relations
					.iter()
					.any(|(relation, derived)| relation == target_name && !derived)
			}) {
			return Err(unsupported(
				"row lock target absent from the query",
				backend,
			));
		}
	}
	Ok(())
}

fn validate_lock_tables_are_not_nullable(
	statement: &SelectStatement,
	targets: &[TableRef],
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	let mut relations = Vec::new();
	for table in &statement.from {
		relations.extend(table_ref_lock_names(table));
	}
	let mut nullable_relations = Vec::new();
	for join in &statement.join {
		let joined_relations = table_ref_lock_names(&join.table);
		match join.join {
			JoinType::LeftJoin => nullable_relations.extend(joined_relations.iter().cloned()),
			JoinType::RightJoin => nullable_relations.extend(relations.iter().cloned()),
			JoinType::FullOuterJoin => {
				nullable_relations.extend(relations.iter().cloned());
				nullable_relations.extend(joined_relations.iter().cloned());
			}
			JoinType::Join | JoinType::InnerJoin | JoinType::CrossJoin => {}
		}
		relations.extend(joined_relations);
	}
	if targets.iter().flat_map(table_ref_lock_names).any(|target| {
		nullable_relations
			.iter()
			.any(|nullable_relation| nullable_relation == &target)
	}) {
		return Err(unsupported(
			"row lock target on nullable outer-join side",
			backend,
		));
	}
	Ok(())
}

fn table_ref_lock_names(table: &TableRef) -> Vec<String> {
	match table {
		TableRef::Table(table) => vec![table.to_string()],
		TableRef::SchemaTable(schema, table) => {
			vec![
				format!("{}.{}", schema.to_string(), table.to_string()),
				table.to_string(),
			]
		}
		TableRef::DatabaseSchemaTable(database, schema, table) => {
			vec![format!(
				"{}.{}.{}",
				database.to_string(),
				schema.to_string(),
				table.to_string()
			)]
		}
		TableRef::TableAlias(_, alias)
		| TableRef::SchemaTableAlias(_, _, alias)
		| TableRef::SubQuery(_, alias) => vec![alias.to_string()],
	}
}

fn table_ref_is_derived(table: &TableRef) -> bool {
	matches!(table, TableRef::SubQuery(_, _))
}

fn validate_condition_holder_lock(
	holder: &ConditionHolder,
	backend: &'static str,
	visible_cte_names: &[String],
) -> Result<(), QueryBuildError> {
	for condition in &holder.conditions {
		validate_condition_expression_lock(condition, backend, visible_cte_names)?;
	}
	Ok(())
}

fn validate_condition_lock(
	condition: &Condition,
	backend: &'static str,
	visible_cte_names: &[String],
) -> Result<(), QueryBuildError> {
	for expression in &condition.conditions {
		validate_condition_expression_lock(expression, backend, visible_cte_names)?;
	}
	Ok(())
}

fn validate_condition_expression_lock(
	expression: &ConditionExpression,
	backend: &'static str,
	visible_cte_names: &[String],
) -> Result<(), QueryBuildError> {
	match expression {
		ConditionExpression::SimpleExpr(expr) => {
			validate_simple_expr_lock(expr, backend, visible_cte_names)
		}
		ConditionExpression::Condition(condition) => {
			validate_condition_lock(condition, backend, visible_cte_names)
		}
	}
}

fn validate_simple_expr_lock(
	expr: &SimpleExpr,
	backend: &'static str,
	visible_cte_names: &[String],
) -> Result<(), QueryBuildError> {
	match expr {
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Value(_)
		| SimpleExpr::Custom(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk => Ok(()),
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::TemporalTrunc {
			expr: expression, ..
		} => validate_simple_expr_lock(expression, backend, visible_cte_names),
		SimpleExpr::Binary(left, _, right) => {
			validate_simple_expr_lock(left, backend, visible_cte_names)?;
			validate_simple_expr_lock(right, backend, visible_cte_names)
		}
		SimpleExpr::FunctionCall(_, args) | SimpleExpr::Tuple(args) => {
			for arg in args {
				validate_simple_expr_lock(arg, backend, visible_cte_names)?;
			}
			Ok(())
		}
		SimpleExpr::SubQuery(_, query) => validate_select_lock_for_backend_with_union_context(
			query,
			backend,
			visible_cte_names,
			false,
			false,
		),
		SimpleExpr::CustomWithExpr(_, expressions) => {
			for expression in expressions {
				validate_simple_expr_lock(expression, backend, visible_cte_names)?;
			}
			Ok(())
		}
		SimpleExpr::Case(case) => {
			for (condition, result) in &case.when_clauses {
				validate_simple_expr_lock(condition, backend, visible_cte_names)?;
				validate_simple_expr_lock(result, backend, visible_cte_names)?;
			}
			if let Some(result) = &case.else_clause {
				validate_simple_expr_lock(result, backend, visible_cte_names)?;
			}
			Ok(())
		}
		SimpleExpr::Window { func, window } => {
			validate_simple_expr_lock(func, backend, visible_cte_names)?;
			validate_window_lock(window, backend, visible_cte_names)
		}
		SimpleExpr::WindowNamed { func, .. } => {
			validate_simple_expr_lock(func, backend, visible_cte_names)
		}
	}
}

fn validate_table_ref_lock(
	table: &TableRef,
	backend: &'static str,
	visible_cte_names: &[String],
	inherited_lock: bool,
) -> Result<(), QueryBuildError> {
	if let TableRef::SubQuery(query, _) = table {
		validate_select_lock_for_backend_with_union_context(
			query,
			backend,
			visible_cte_names,
			false,
			inherited_lock,
		)?;
	}
	Ok(())
}

fn validate_window_lock(
	window: &WindowStatement,
	backend: &'static str,
	visible_cte_names: &[String],
) -> Result<(), QueryBuildError> {
	for partition in &window.partition_by {
		validate_simple_expr_lock(partition, backend, visible_cte_names)?;
	}
	for order in &window.order_by {
		if let OrderExprKind::Expr(expr) = &order.expr {
			validate_simple_expr_lock(expr, backend, visible_cte_names)?;
		}
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
	if let Some(table) = &statement.table {
		validate_table_ref(table, backend)?;
	}
	if let Some(condition) = &statement.r#where {
		validate_simple_expr(condition, backend)?;
	}
	Ok(())
}

pub(crate) fn validate_insert_for_backend(
	statement: &InsertStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let Some(table) = &statement.table {
		validate_table_ref(table, backend)?;
	}
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
	if let InsertSource::Subquery(query) = &statement.source {
		validate_select_lock_for_backend(query, backend)?;
	}
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr(expression, backend)?;
			validate_simple_expr_lock(expression, backend, &[])?;
		}
	}
	Ok(())
}

pub(crate) fn validate_insert_lock_for_backend(
	statement: &InsertStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let InsertSource::Subquery(query) = &statement.source {
		validate_select_lock_for_backend(query, backend)?;
	}
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr_lock(expression, backend, &[])?;
		}
	}
	Ok(())
}

pub(crate) fn validate_update_for_backend(
	statement: &UpdateStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let Some(table) = &statement.table {
		validate_table_ref(table, backend)?;
	}
	for (_, expression) in &statement.values {
		validate_simple_expr(expression, backend)?;
		validate_simple_expr_lock(expression, backend, &[])?;
	}
	validate_condition_holder(&statement.r#where, backend)?;
	validate_condition_holder_lock(&statement.r#where, backend, &[])?;
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr(expression, backend)?;
			validate_simple_expr_lock(expression, backend, &[])?;
		}
	}
	Ok(())
}

pub(crate) fn validate_update_lock_for_backend(
	statement: &UpdateStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let Some(table) = &statement.table {
		validate_table_ref_lock(table, backend, &[], false)?;
	}
	for (_, expression) in &statement.values {
		validate_simple_expr_lock(expression, backend, &[])?;
	}
	validate_condition_holder_lock(&statement.r#where, backend, &[])?;
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr_lock(expression, backend, &[])?;
		}
	}
	Ok(())
}

pub(crate) fn validate_delete_for_backend(
	statement: &DeleteStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let Some(table) = &statement.table {
		validate_table_ref(table, backend)?;
	}
	validate_condition_holder(&statement.r#where, backend)?;
	validate_condition_holder_lock(&statement.r#where, backend, &[])?;
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr(expression, backend)?;
			validate_simple_expr_lock(expression, backend, &[])?;
		}
	}
	Ok(())
}

pub(crate) fn validate_delete_lock_for_backend(
	statement: &DeleteStatement,
	backend: &'static str,
) -> Result<(), QueryBuildError> {
	if let Some(table) = &statement.table {
		validate_table_ref_lock(table, backend, &[], false)?;
	}
	validate_condition_holder_lock(&statement.r#where, backend, &[])?;
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			validate_simple_expr_lock(expression, backend, &[])?;
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

#[cfg(feature = "pgvector")]
pub(crate) fn validate_postgres_create_table_dimensions(
	statement: &CreateTableStatement,
) -> Result<(), QueryBuildError> {
	for column in &statement.columns {
		validate_postgres_column_dimensions(column)?;
	}
	Ok(())
}

#[cfg(feature = "pgvector")]
pub(crate) fn validate_postgres_alter_table_dimensions(
	statement: &AlterTableStatement,
) -> Result<(), QueryBuildError> {
	for operation in &statement.operations {
		match operation {
			AlterTableOperation::AddColumn(column) | AlterTableOperation::ModifyColumn(column) => {
				validate_postgres_column_dimensions(column)?;
			}
			_ => {}
		}
	}
	Ok(())
}

#[cfg(feature = "pgvector")]
fn validate_postgres_column_dimensions(column: &ColumnDef) -> Result<(), QueryBuildError> {
	if let Some(column_type) = &column.column_type {
		validate_postgres_column_type_dimensions(column_type)?;
	}
	Ok(())
}

#[cfg(feature = "pgvector")]
fn validate_postgres_column_type_dimensions(
	column_type: &ColumnType,
) -> Result<(), QueryBuildError> {
	match column_type {
		ColumnType::Vector(dimensions) if !(1..=2000).contains(dimensions) => {
			Err(QueryBuildError::InvalidPgvectorDimensions {
				dimensions: *dimensions,
			})
		}
		ColumnType::Array(element_type) => validate_postgres_column_type_dimensions(element_type),
		_ => Ok(()),
	}
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
	if let Some(generated) = &column.generated
		&& let Some(expr) = &generated.expr
	{
		validate_schema_expr(expr, backend)?;
	}
	Ok(())
}

fn validate_column_type(
	column_type: &ColumnType,
	_backend: &'static str,
) -> Result<(), QueryBuildError> {
	match column_type {
		#[cfg(feature = "pgvector")]
		ColumnType::Vector(_) => Err(unsupported("pgvector column types", _backend)),
		ColumnType::Array(element_type) => validate_column_type(element_type, _backend),
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
		SimpleExpr::TemporalTrunc {
			expr,
			kind,
			time_zone,
			output,
		} => {
			if *output == crate::expr::TemporalTruncOutput::Date
				&& matches!(
					kind,
					crate::expr::TemporalTruncKind::Hour
						| crate::expr::TemporalTruncKind::Minute
						| crate::expr::TemporalTruncKind::Second
				) {
				return Err(QueryBuildError::InvalidTemporalTruncation {
					kind: kind.as_str(),
					output: "date",
				});
			}
			if matches!(backend, "MySQL" | "SQLite")
				&& matches!(time_zone, Some(crate::expr::TemporalTimeZone::Named(_)))
			{
				return Err(unsupported("named time-zone conversion", backend));
			}
			validate_simple_expr(expr, backend)
		}
		SimpleExpr::Binary(left, _operator, right) => {
			#[cfg(feature = "pgvector")]
			if backend != "PostgreSQL"
				&& matches!(
					_operator,
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

fn validate_value(value: &Value, _backend: &'static str) -> Result<(), QueryBuildError> {
	match value {
		#[cfg(feature = "pgvector")]
		Value::Vector(_) if _backend != "PostgreSQL" => Err(unsupported("pgvector values", _backend)),
		Value::Array(_, Some(values)) => {
			for value in values.iter() {
				validate_value(value, _backend)?;
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
	if backend == "MySQL"
		&& matches!(
			window.frame.as_ref().map(|frame| &frame.frame_type),
			Some(crate::types::FrameType::Groups)
		) {
		return Err(unsupported("GROUPS window frames", backend));
	}
	for partition in &window.partition_by {
		validate_simple_expr(partition, backend)?;
	}
	for order in &window.order_by {
		validate_order_expr(order, backend)?;
	}
	Ok(())
}

fn collect_select_pgvector_features(
	statement: &SelectStatement,
	features: &mut PgvectorFeatureSet,
) {
	for cte in &statement.ctes {
		collect_select_pgvector_features(&cte.query, features);
	}
	for select in &statement.selects {
		collect_simple_expr_pgvector_features(&select.expr, features);
	}
	for table in &statement.from {
		collect_table_ref_pgvector_features(table, features);
	}
	for join in &statement.join {
		collect_table_ref_pgvector_features(&join.table, features);
		if let Some(crate::types::JoinOn::Condition(condition)) = &join.on {
			collect_condition_pgvector_features(condition, features);
		}
	}
	collect_condition_holder_pgvector_features(&statement.r#where, features);
	for group in &statement.groups {
		collect_simple_expr_pgvector_features(group, features);
	}
	collect_condition_holder_pgvector_features(&statement.having, features);
	for (_, union) in &statement.unions {
		collect_select_pgvector_features(union, features);
	}
	for order in &statement.orders {
		if let OrderExprKind::Expr(expr) = &order.expr {
			collect_simple_expr_pgvector_features(expr, features);
		}
	}
	if let Some(limit) = &statement.limit {
		collect_value_pgvector_features(limit, features);
	}
	if let Some(offset) = &statement.offset {
		collect_value_pgvector_features(offset, features);
	}
	for (_, window) in &statement.windows {
		collect_window_pgvector_features(window, features);
	}
}

fn collect_update_pgvector_features(
	statement: &UpdateStatement,
	features: &mut PgvectorFeatureSet,
) {
	if let Some(table) = &statement.table {
		collect_table_ref_pgvector_features(table, features);
	}
	for (_, expression) in &statement.values {
		collect_simple_expr_pgvector_features(expression, features);
	}
	collect_condition_holder_pgvector_features(&statement.r#where, features);
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			collect_simple_expr_pgvector_features(expression, features);
		}
	}
}

fn collect_insert_pgvector_features(
	statement: &InsertStatement,
	features: &mut PgvectorFeatureSet,
) {
	if let Some(table) = &statement.table {
		collect_table_ref_pgvector_features(table, features);
	}
	match &statement.source {
		InsertSource::Values(rows) => {
			for row in rows {
				for value in row {
					collect_value_pgvector_features(value, features);
				}
			}
		}
		InsertSource::Subquery(query) => collect_select_pgvector_features(query, features),
	}
	if let Some(expressions) = &statement.returning_exprs {
		for expression in expressions {
			collect_simple_expr_pgvector_features(expression, features);
		}
	}
}

fn collect_condition_holder_pgvector_features(
	holder: &ConditionHolder,
	features: &mut PgvectorFeatureSet,
) {
	for condition in &holder.conditions {
		collect_condition_expression_pgvector_features(condition, features);
	}
}

fn collect_condition_pgvector_features(condition: &Condition, features: &mut PgvectorFeatureSet) {
	for expression in &condition.conditions {
		collect_condition_expression_pgvector_features(expression, features);
	}
}

fn collect_condition_expression_pgvector_features(
	expression: &ConditionExpression,
	features: &mut PgvectorFeatureSet,
) {
	match expression {
		ConditionExpression::SimpleExpr(expr) => {
			collect_simple_expr_pgvector_features(expr, features);
		}
		ConditionExpression::Condition(condition) => {
			collect_condition_pgvector_features(condition, features);
		}
	}
}

fn collect_simple_expr_pgvector_features(expr: &SimpleExpr, features: &mut PgvectorFeatureSet) {
	collect_simple_expr_pgvector_features_with_values(expr, features, true);
}

fn collect_simple_expr_pgvector_features_with_values(
	expr: &SimpleExpr,
	features: &mut PgvectorFeatureSet,
	collect_vector_values: bool,
) {
	match expr {
		SimpleExpr::Column(_)
		| SimpleExpr::TableColumn(_, _)
		| SimpleExpr::Custom(_)
		| SimpleExpr::Constant(_)
		| SimpleExpr::Asterisk => {}
		SimpleExpr::Value(value) => {
			if collect_vector_values {
				collect_value_pgvector_features(value, features);
			}
		}
		SimpleExpr::Unary(_, expression)
		| SimpleExpr::AsEnum(_, expression)
		| SimpleExpr::ExprAlias(expression, _)
		| SimpleExpr::Cast(expression, _)
		| SimpleExpr::TemporalTrunc {
			expr: expression, ..
		} => {
			collect_simple_expr_pgvector_features_with_values(
				expression,
				features,
				collect_vector_values,
			);
		}
		SimpleExpr::Binary(left, _operator, right) => {
			#[cfg(feature = "pgvector")]
			let is_distance_operator = matches!(
				_operator,
				BinOper::PgOperator(
					PgBinOper::L2Distance
						| PgBinOper::NegativeInnerProduct
						| PgBinOper::CosineDistance
				)
			);
			#[cfg(not(feature = "pgvector"))]
			let is_distance_operator = false;
			if is_distance_operator {
				features.insert(PgvectorFeature::DistanceOperator);
			}
			collect_simple_expr_pgvector_features_with_values(
				left,
				features,
				collect_vector_values,
			);
			collect_simple_expr_pgvector_features_with_values(
				right,
				features,
				collect_vector_values,
			);
		}
		SimpleExpr::FunctionCall(_, args) | SimpleExpr::Tuple(args) => {
			for arg in args {
				collect_simple_expr_pgvector_features_with_values(
					arg,
					features,
					collect_vector_values,
				);
			}
		}
		SimpleExpr::SubQuery(_, query) => collect_select_pgvector_features(query, features),
		SimpleExpr::CustomWithExpr(_, expressions) => {
			for expression in expressions {
				collect_simple_expr_pgvector_features_with_values(
					expression,
					features,
					collect_vector_values,
				);
			}
		}
		SimpleExpr::Case(case) => {
			for (condition, result) in &case.when_clauses {
				collect_simple_expr_pgvector_features_with_values(
					condition,
					features,
					collect_vector_values,
				);
				collect_simple_expr_pgvector_features_with_values(
					result,
					features,
					collect_vector_values,
				);
			}
			if let Some(result) = &case.else_clause {
				collect_simple_expr_pgvector_features_with_values(
					result,
					features,
					collect_vector_values,
				);
			}
		}
		SimpleExpr::Window { func, window } => {
			collect_simple_expr_pgvector_features_with_values(
				func,
				features,
				collect_vector_values,
			);
			collect_window_pgvector_features(window, features);
		}
		SimpleExpr::WindowNamed { func, .. } => {
			collect_simple_expr_pgvector_features_with_values(
				func,
				features,
				collect_vector_values,
			);
		}
	}
}

fn collect_value_pgvector_features(value: &Value, _features: &mut PgvectorFeatureSet) {
	match value {
		#[cfg(feature = "pgvector")]
		Value::Vector(_) => _features.insert(PgvectorFeature::VectorValue),
		Value::Array(_, Some(values)) => {
			for value in values.iter() {
				collect_value_pgvector_features(value, _features);
			}
		}
		_ => {}
	}
}

fn collect_table_ref_pgvector_features(table: &TableRef, features: &mut PgvectorFeatureSet) {
	if let TableRef::SubQuery(query, _) = table {
		collect_select_pgvector_features(query, features);
	}
}

fn collect_window_pgvector_features(window: &WindowStatement, features: &mut PgvectorFeatureSet) {
	for partition in &window.partition_by {
		collect_simple_expr_pgvector_features(partition, features);
	}
	for order in &window.order_by {
		if let OrderExprKind::Expr(expr) = &order.expr {
			collect_simple_expr_pgvector_features(expr, features);
		}
	}
}

#[cfg(all(test, feature = "pgvector"))]
mod pgvector_feature_tests {
	use crate::prelude::{Alias, BinOper, Expr, Query, SimpleExpr};
	use crate::types::PgBinOper;
	use crate::value::Value;
	use rstest::rstest;

	use super::{
		PgvectorFeature, insert_pgvector_feature, insert_pgvector_features,
		select_pgvector_feature, update_pgvector_feature,
	};

	fn vector_value(values: &[f32]) -> Value {
		Value::Vector(Some(Box::new(values.to_vec())))
	}

	#[test]
	fn insert_feature_set_collects_vector_values_from_rows() {
		let statement = Query::insert()
			.into_table(Alias::new("items"))
			.columns([Alias::new("embedding")])
			.values_panic([vector_value(&[1.0, 2.0, 3.0])])
			.to_owned();

		let features = insert_pgvector_features(&statement);

		assert!(features.contains(PgvectorFeature::VectorValue));
		assert!(!features.contains(PgvectorFeature::DistanceOperator));
	}

	#[rstest]
	fn insert_feature_set_collects_nested_insert_select_features() {
		let distance = SimpleExpr::Binary(
			Box::new(Expr::col(Alias::new("embedding")).into_simple_expr()),
			BinOper::PgOperator(PgBinOper::CosineDistance),
			Box::new(SimpleExpr::Value(vector_value(&[1.0, 2.0, 3.0]))),
		);
		let select = Query::select()
			.expr(distance)
			.from(Alias::new("source_items"))
			.to_owned();
		let statement = Query::insert()
			.into_table(Alias::new("distances"))
			.columns([Alias::new("distance")])
			.from_subquery(select)
			.to_owned();

		let features = insert_pgvector_features(&statement);

		assert!(features.contains(PgvectorFeature::DistanceOperator));
		assert!(features.contains(PgvectorFeature::VectorValue));
	}

	#[rstest]
	fn insert_feature_inspection_ignores_unrelated_backend_validation() {
		let distance = SimpleExpr::Binary(
			Box::new(Expr::col(Alias::new("embedding")).into_simple_expr()),
			BinOper::PgOperator(PgBinOper::CosineDistance),
			Box::new(SimpleExpr::Value(vector_value(&[1.0, 2.0, 3.0]))),
		);
		let select = Query::select()
			.expr(distance)
			.from(Alias::new("source_items"))
			.distinct_on([Alias::new("id")])
			.to_owned();
		let statement = Query::insert()
			.into_table(Alias::new("distances"))
			.columns([Alias::new("distance")])
			.from_subquery(select)
			.to_owned();

		assert_eq!(
			insert_pgvector_feature(&statement),
			Some(PgvectorFeature::DistanceOperator)
		);
	}

	#[rstest]
	fn update_feature_inspection_ignores_unrelated_backend_validation() {
		let distance = SimpleExpr::Binary(
			Box::new(Expr::col(Alias::new("embedding")).into_simple_expr()),
			BinOper::PgOperator(PgBinOper::CosineDistance),
			Box::new(SimpleExpr::Value(vector_value(&[1.0, 2.0, 3.0]))),
		);
		let subquery = Query::select()
			.expr(distance)
			.from(Alias::new("source_items"))
			.distinct_on([Alias::new("id")])
			.to_owned();
		let statement = Query::update()
			.table(Alias::new("items"))
			.value_expr(Alias::new("distance"), Expr::subquery(subquery))
			.to_owned();

		assert_eq!(
			update_pgvector_feature(&statement),
			Some(PgvectorFeature::DistanceOperator)
		);
	}

	#[test]
	fn insert_feature_set_collects_returning_expressions() {
		let statement = Query::insert()
			.into_table(Alias::new("items"))
			.columns([Alias::new("name")])
			.values_panic([Value::String(Some(Box::new("item".to_owned())))])
			.returning_exprs([SimpleExpr::Value(vector_value(&[1.0, 2.0, 3.0]))])
			.to_owned();

		let features = insert_pgvector_features(&statement);

		assert!(features.contains(PgvectorFeature::VectorValue));
	}

	#[test]
	fn select_feature_inspection_ignores_unrelated_backend_validation() {
		let distance = SimpleExpr::Binary(
			Box::new(Expr::col(Alias::new("embedding")).into_simple_expr()),
			BinOper::PgOperator(PgBinOper::CosineDistance),
			Box::new(SimpleExpr::Value(vector_value(&[1.0, 2.0, 3.0]))),
		);
		let statement = Query::select()
			.column(Alias::new("id"))
			.from(Alias::new("items"))
			.distinct_on([Alias::new("id")])
			.and_where(distance)
			.to_owned();

		assert_eq!(
			select_pgvector_feature(&statement),
			Some(PgvectorFeature::DistanceOperator)
		);
	}
}
