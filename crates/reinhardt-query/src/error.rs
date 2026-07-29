//! Errors and validation for checked query building.

#[cfg(feature = "pgvector")]
use crate::types::{BinOper, PgBinOper};
use crate::{
	expr::{Condition, ConditionExpression, ConditionHolder, SimpleExpr},
	query::{
		AlterTableOperation, AlterTableStatement, CreateIndexStatement, CreateTableStatement,
		DeleteStatement, InsertSource, InsertStatement, SelectStatement, UpdateStatement,
	},
	types::{
		ColumnDef, ColumnType, OrderExpr, OrderExprKind, SchemaExpr, TableConstraint, TableRef,
		WindowStatement,
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
	pgvector_feature_from_validation(validate_select_for_backend(statement, "feature inspection"))
}

/// Returns every pgvector feature found in a select AST.
pub fn select_pgvector_features(statement: &SelectStatement) -> PgvectorFeatureSet {
	let mut features = PgvectorFeatureSet::default();
	collect_select_pgvector_features(statement, &mut features);
	features
}

/// Returns the first pgvector feature found in an insert AST.
pub fn insert_pgvector_feature(statement: &InsertStatement) -> Option<PgvectorFeature> {
	pgvector_feature_from_validation(validate_insert_for_backend(statement, "feature inspection"))
}

/// Returns every pgvector feature found in an insert AST.
pub fn insert_pgvector_features(statement: &InsertStatement) -> PgvectorFeatureSet {
	let mut features = PgvectorFeatureSet::default();
	collect_insert_pgvector_features(statement, &mut features);
	features
}

/// Returns the first pgvector feature found in an update AST.
pub fn update_pgvector_feature(statement: &UpdateStatement) -> Option<PgvectorFeature> {
	pgvector_feature_from_validation(validate_update_for_backend(statement, "feature inspection"))
}

/// Returns every pgvector feature found in an update AST.
pub fn update_pgvector_features(statement: &UpdateStatement) -> PgvectorFeatureSet {
	let mut features = PgvectorFeatureSet::default();
	collect_update_pgvector_features(statement, &mut features);
	features
}

fn pgvector_feature_from_validation(
	result: Result<(), QueryBuildError>,
) -> Option<PgvectorFeature> {
	match result {
		Err(QueryBuildError::UnsupportedBackendFeature { feature, .. }) => match feature {
			"pgvector column types" => Some(PgvectorFeature::ColumnType),
			"pgvector distance operators" => Some(PgvectorFeature::DistanceOperator),
			"approximate vector indexes" => Some(PgvectorFeature::ApproximateIndex),
			"pgvector values" => Some(PgvectorFeature::VectorValue),
			_ => None,
		},
		Err(QueryBuildError::InvalidPgvectorDimensions { .. }) => None,
		Ok(()) => None,
	}
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
	if let Some(table) = &statement.table {
		validate_table_ref(table, backend)?;
	}
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
	if let Some(table) = &statement.table {
		validate_table_ref(table, backend)?;
	}
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

#[cfg(feature = "pgvector")]
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
		SimpleExpr::Binary(left, _operator, right) => {
			#[cfg(feature = "pgvector")]
			if matches!(
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
		Value::Vector(_) => Err(unsupported("pgvector values", _backend)),
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
		| SimpleExpr::Cast(expression, _) => {
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

	use super::{PgvectorFeature, insert_pgvector_features};

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

	#[test]
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
}
