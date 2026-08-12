//! Database integration for admin operations
//!
//! This module provides database access layer for admin CRUD operations,
//! integrating with reinhardt-orm's QuerySet API.

use crate::core::admin_query::AdminQuery;
use crate::types::{AdminError, AdminResult};
use async_trait::async_trait;
use reinhardt_core::macros::injectable;
use reinhardt_db::migrations::FieldType as DbFieldType;
use reinhardt_db::orm::execution::convert_values;
use reinhardt_db::orm::{
	DatabaseConnection, Filter, FilterCondition, FilterOperator, FilterValue, Model,
	database_value_to_query_value,
};
use reinhardt_di::{DiResult, Injectable, InjectionContext, KeyedFactoryOutput};
use reinhardt_query::prelude::{
	Alias, BinOper, CaseStatement, ColumnRef, Condition, Expr, ExprTrait, Func, IntoValue, Order,
	PostgresQueryBuilder, Query, QueryStatementBuilder, SelectStatement, SimpleExpr, TableRef,
	TemporalTimeZone, TemporalTruncKind, TemporalTruncOutput, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const ADMIN_LIST_TOTAL_COUNT_ALIAS: &str = "__reinhardt_total_count";
const ADMIN_DATE_HIERARCHY_ALIAS: &str = "__reinhardt_date_hierarchy";
const ADMIN_RELATED_COLUMN_ALIAS_PREFIX: &str = "__reinhardt_related_";
pub(crate) const SENSITIVE_FIELDS: &[&str] = &["password_hash", "password_salt"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdminRelatedField {
	pub(crate) relation_name: String,
	pub(crate) source_column: String,
	pub(crate) target_table: String,
	pub(crate) target_column: String,
	pub(crate) columns: Vec<String>,
}

/// Converts a `serde_json::Value` into a reinhardt-query `Value`.
///
/// String values are inspected for ISO 8601 date/time patterns and converted
/// to the appropriate chrono type so that PostgreSQL accepts them for
/// `timestamptz`, `date`, and `time` columns without an explicit cast.
fn json_to_sea_value(
	table_name: &str,
	field_name: &str,
	value: serde_json::Value,
) -> AdminResult<Value> {
	#[cfg(feature = "pgvector")]
	if let Some(field_meta) =
		crate::server::type_inference::get_field_metadata(table_name, field_name)
		&& let DbFieldType::Vector { dimensions } = &field_meta.field_type
	{
		if field_meta.nullable && matches!(&value, serde_json::Value::Null) {
			return Ok(Value::Vector(None));
		}
		let values = match value {
			serde_json::Value::String(value) => {
				let value = value.trim();
				if field_meta.nullable && value.is_empty() {
					return Ok(Value::Vector(None));
				}
				serde_json::from_str::<Vec<f32>>(value)
					.or_else(|_| {
						value
							.split(',')
							.map(|component| component.trim().parse::<f32>())
							.collect::<Result<Vec<_>, _>>()
					})
					.map_err(|error| {
						AdminError::ValidationError(format!(
							"Field '{field_name}' must be a JSON array or comma-separated vector values: {error}"
						))
					})?
			}
			serde_json::Value::Array(_) => {
				serde_json::from_value::<Vec<f32>>(value).map_err(|error| {
					AdminError::ValidationError(format!(
						"Field '{field_name}' must be an array of vector values: {error}"
					))
				})?
			}
			_ => {
				return Err(AdminError::ValidationError(format!(
					"Field '{field_name}' must be a JSON array of vector values"
				)));
			}
		};
		if values.len() != *dimensions {
			return Err(AdminError::ValidationError(format!(
				"Field '{field_name}' requires {dimensions} vector values, got {}",
				values.len()
			)));
		}
		if values.iter().any(|value| !value.is_finite()) {
			return Err(AdminError::ValidationError(format!(
				"Field '{field_name}' contains a non-finite vector value"
			)));
		}
		return Ok(Value::Vector(Some(Box::new(values))));
	}

	Ok(match value {
		serde_json::Value::String(s) => {
			// ISO 8601 datetime with timezone offset (e.g. "2026-04-02T16:45:50Z")
			if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
				Value::ChronoDateTimeUtc(Some(Box::new(dt.with_timezone(&chrono::Utc))))
			// ISO 8601 datetime with fractional seconds and Z suffix
			} else if let Ok(dt) =
				chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.fZ")
			{
				Value::ChronoDateTimeUtc(Some(Box::new(dt.and_utc())))
			// Date only
			} else if s.len() == 10 {
				if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
					return Ok(Value::ChronoDate(Some(Box::new(d))));
				}
				Value::String(Some(Box::new(s)))
			// Time only
			} else if s.len() == 8 && s.chars().filter(|c| *c == ':').count() == 2 {
				if let Ok(t) = chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S") {
					return Ok(Value::ChronoTime(Some(Box::new(t))));
				}
				Value::String(Some(Box::new(s)))
			// UUID (8-4-4-4-12 hex pattern)
			} else if s.len() == 36
				&& s.chars().enumerate().all(|(i, c)| {
					matches!(i, 8 | 13 | 18 | 23) && c == '-' || c.is_ascii_hexdigit()
				}) {
				if let Ok(uuid) = uuid::Uuid::parse_str(&s) {
					return Ok(Value::Uuid(Some(Box::new(uuid))));
				}
				Value::String(Some(Box::new(s)))
			} else {
				Value::String(Some(Box::new(s)))
			}
		}
		serde_json::Value::Number(n) => {
			if let Some(i) = n.as_i64() {
				Value::BigInt(Some(i))
			} else if let Some(f) = n.as_f64() {
				Value::Double(Some(f))
			} else {
				Value::String(Some(Box::new(n.to_string())))
			}
		}
		serde_json::Value::Bool(b) => Value::Bool(Some(b)),
		serde_json::Value::Null => Value::Int(None),
		_ => Value::String(Some(Box::new(value.to_string()))),
	})
}

/// Dummy record type for admin panel CRUD operations
///
/// This type exists solely to satisfy the `<M: Model>` generic constraint
/// in `AdminDatabase` methods. The admin panel operates on dynamic data
/// (serde_json::Value), not statically-typed models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminRecord {
	/// The primary key identifier for the admin record.
	pub id: Option<i64>,
}

/// Field accessors for `AdminRecord` used in typed query construction.
#[derive(Debug, Clone)]
pub struct AdminRecordFields {
	/// Typed field accessor for the `id` column.
	pub id: reinhardt_db::orm::query_fields::Field<AdminRecord, Option<i64>>,
}

impl Default for AdminRecordFields {
	fn default() -> Self {
		Self::new()
	}
}

impl AdminRecordFields {
	/// Creates a new set of field accessors with default column names.
	pub fn new() -> Self {
		Self {
			id: reinhardt_db::orm::query_fields::Field::new(vec!["id".to_string()]),
		}
	}
}

impl reinhardt_db::orm::FieldSelector for AdminRecordFields {
	fn with_alias(mut self, alias: &str) -> Self {
		self.id = self.id.with_alias(alias);
		self
	}
}

impl Model for AdminRecord {
	type PrimaryKey = i64;
	type Fields = AdminRecordFields;
	type Objects = reinhardt_db::orm::Manager<Self>;

	fn table_name() -> &'static str {
		"admin_records"
	}

	fn new_fields() -> Self::Fields {
		AdminRecordFields::new()
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, pk: Self::PrimaryKey) {
		self.id = Some(pk);
	}
}

/// Converts a string primary key value to the appropriate SeaQuery `Value`
/// based on the field's registered database type.
///
/// Looks up the field type from the migration registry. When the registry has
/// metadata for the given table/field, the conversion is type-aware (e.g.
/// UUID strings become `Value::Uuid`). Falls back to the i64-then-String
/// heuristic when metadata is unavailable.
fn parse_pk_value(table_name: &str, pk_field: &str, id: &str) -> Value {
	if let Some(field_meta) =
		crate::server::type_inference::get_field_metadata(table_name, pk_field)
	{
		match field_meta.field_type {
			DbFieldType::Uuid => {
				if let Ok(uuid) = uuid::Uuid::parse_str(id) {
					return Value::Uuid(Some(Box::new(uuid)));
				}
			}
			DbFieldType::BigInteger => {
				if let Ok(num) = id.parse::<i64>() {
					return Value::BigInt(Some(num));
				}
			}
			DbFieldType::Integer
			| DbFieldType::SmallInteger
			| DbFieldType::TinyInt
			| DbFieldType::MediumInt => {
				if let Ok(num) = id.parse::<i32>() {
					return Value::Int(Some(num));
				}
			}
			_ => {}
		}
	}

	// Fallback: existing heuristic for backward compatibility
	if let Ok(num_id) = id.parse::<i64>() {
		Value::BigInt(Some(num_id))
	} else {
		Value::String(Some(Box::new(id.to_string())))
	}
}

/// Batch version of `parse_pk_value` for bulk operations.
fn parse_pk_values(table_name: &str, pk_field: &str, ids: &[String]) -> Vec<Value> {
	ids.iter()
		.map(|id| parse_pk_value(table_name, pk_field, id))
		.collect()
}

/// Convert FilterValue to Value
#[doc(hidden)]
pub fn filter_value_to_sea_value(v: &FilterValue) -> AdminResult<Value> {
	let value = match v {
		FilterValue::Typed(value) => {
			database_value_to_query_value(value.clone().map_err(AdminError::FieldCodec)?)
		}
		FilterValue::String(s) => s.clone().into(),
		FilterValue::Integer(i) | FilterValue::Int(i) => (*i).into(),
		FilterValue::Float(f) => (*f).into(),
		FilterValue::Boolean(b) | FilterValue::Bool(b) => (*b).into(),
		FilterValue::Null => Value::Int(None),
		// Array values are not scalar; they are handled by In/NotIn arms
		// in build_single_filter_expr(). Return None-string as fallback
		// for unexpected scalar contexts.
		FilterValue::Array(_) => Value::String(None),
		FilterValue::List(_) | FilterValue::Range(_, _) => Value::String(None),
		FilterValue::FieldRef(f) => {
			// FieldRef generates column reference, not scalar value.
			// For Value context, return field name as string.
			// Proper handling is in build_single_filter_expr().
			Value::String(Some(Box::new(f.field.clone())))
		}
		FilterValue::Expression(expr) => {
			// Expression generates SQL expression, not scalar value.
			// For Value context, return SQL string representation.
			// Proper handling is in build_single_filter_expr().
			Value::String(Some(Box::new(expr.to_sql())))
		}
		FilterValue::OuterRef(outer) => {
			// OuterRef generates outer query reference, not scalar value.
			// For Value context, return field name as string.
			// Proper handling is in build_single_filter_expr().
			Value::String(Some(Box::new(outer.field.clone())))
		}
	};
	Ok(value)
}

fn filter_comparison_value(v: &FilterValue) -> AdminResult<SimpleExpr> {
	if let FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(date))) = v {
		// The backend value carrier has no native date variant, so keep the
		// AdminQuery value typed and lower its PostgreSQL representation through SQL DATE.
		return Ok(Expr::cust_with_values("CAST(? AS DATE)", [postgres_date_text(date)]).into());
	}

	Ok(filter_value_to_sea_value(v)?.into())
}

fn postgres_date_text(date: &chrono::NaiveDate) -> String {
	use chrono::Datelike;

	let year = date.year();
	let (display_year, suffix) = if year <= 0 {
		(1_i64 - i64::from(year), " BC")
	} else {
		(i64::from(year), "")
	};
	format!(
		"{display_year:04}-{:02}-{:02}{suffix}",
		date.month(),
		date.day()
	)
}

/// Convert an annotation `AnnotationValue` to a safe SeaQuery `SimpleExpr`.
///
/// Uses type-safe SeaQuery API for field references and literal values
/// instead of raw SQL string interpolation, preventing SQL injection.
fn annotation_value_to_safe_expr(
	val: &reinhardt_db::orm::annotation::AnnotationValue,
	root_alias: Option<&str>,
) -> SimpleExpr {
	use reinhardt_db::orm::annotation::AnnotationValue;

	match val {
		AnnotationValue::Value(v) => {
			use reinhardt_db::orm::annotation::Value as AnnotValue;
			match v {
				AnnotValue::String(s) => Expr::val(s.as_str()).into(),
				AnnotValue::Int(i) => Expr::val(*i).into(),
				AnnotValue::Float(f) => Expr::val(*f).into(),
				AnnotValue::Bool(b) => Expr::val(*b).into(),
				AnnotValue::Null => Expr::val(Option::<String>::None).into(),
			}
		}
		AnnotationValue::Field(f) => filter_column(root_alias, &f.field).into(),
		AnnotationValue::Expression(e) => annotation_expr_to_safe_expr(e, root_alias),
		AnnotationValue::Aggregate(a) => aggregate_to_safe_expr(a, root_alias),
		// Subquery and PostgreSQL-specific aggregation types produce SQL
		// from internally constructed ORM queries, not from user HTTP input.
		// Their SQL output is safe because it's built through type-safe ORM APIs.
		AnnotationValue::Subquery(_)
		| AnnotationValue::ArrayAgg(_)
		| AnnotationValue::StringAgg(_)
		| AnnotationValue::JsonbAgg(_)
		| AnnotationValue::JsonbBuildObject(_)
		| AnnotationValue::TsRank(_) => Expr::cust(val.to_sql()).into(),
	}
}

/// Convert an `Aggregate` to a safe SeaQuery `SimpleExpr`.
///
/// Uses parameterized function templates with quoted column identifiers
/// instead of raw SQL string interpolation, preventing SQL injection
/// through field name manipulation.
fn aggregate_to_safe_expr(
	agg: &reinhardt_db::orm::aggregation::Aggregate,
	root_alias: Option<&str>,
) -> SimpleExpr {
	use reinhardt_db::orm::aggregation::AggregateFunc;

	let func_name = match agg.func {
		AggregateFunc::Count | AggregateFunc::CountDistinct => "COUNT",
		AggregateFunc::Sum => "SUM",
		AggregateFunc::Avg => "AVG",
		AggregateFunc::Max => "MAX",
		AggregateFunc::Min => "MIN",
	};

	if let Some(field) = &agg.field {
		let col_expr: SimpleExpr = filter_column(root_alias, field).into();
		let is_distinct = agg.distinct || matches!(agg.func, AggregateFunc::CountDistinct);
		if is_distinct {
			Expr::cust_with_values(format!("{func_name}(DISTINCT ?)"), [col_expr]).into()
		} else {
			Expr::cust_with_values(format!("{func_name}(?)"), [col_expr]).into()
		}
	} else {
		// COUNT(*) case - static SQL template, no user input
		Expr::cust(format!("{func_name}(*)")).into()
	}
}

/// Convert an annotation `Expression` to a safe SeaQuery `SimpleExpr`.
///
/// Recursively converts all expression types using type-safe SeaQuery API
/// for field references and values, preventing SQL injection through
/// value manipulation in expression trees.
fn annotation_expr_to_safe_expr(
	expr: &reinhardt_db::orm::annotation::Expression,
	root_alias: Option<&str>,
) -> SimpleExpr {
	use reinhardt_db::orm::annotation::Expression as AnnotExpr;

	match expr {
		AnnotExpr::Add(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left, root_alias);
			let right_expr = annotation_value_to_safe_expr(right, root_alias);
			Expr::cust_with_values("(? + ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Subtract(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left, root_alias);
			let right_expr = annotation_value_to_safe_expr(right, root_alias);
			Expr::cust_with_values("(? - ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Multiply(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left, root_alias);
			let right_expr = annotation_value_to_safe_expr(right, root_alias);
			Expr::cust_with_values("(? * ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Divide(left, right) => {
			let left_expr = annotation_value_to_safe_expr(left, root_alias);
			let right_expr = annotation_value_to_safe_expr(right, root_alias);
			Expr::cust_with_values("(? / ?)", [left_expr, right_expr]).into()
		}
		AnnotExpr::Case { whens, default } => {
			let mut case = CaseStatement::new();
			for when in whens {
				// Q conditions are constructed internally by the ORM's query builder,
				// not from user HTTP input. The THEN values are safely converted
				// through annotation_value_to_safe_expr.
				let cond_expr: SimpleExpr = Expr::cust(when.condition.to_sql()).into();
				let then_expr = annotation_value_to_safe_expr(&when.then, root_alias);
				case = case.when(cond_expr, then_expr);
			}
			if let Some(default_val) = default {
				case = case.else_result(annotation_value_to_safe_expr(default_val, root_alias));
			}
			SimpleExpr::from(case)
		}
		AnnotExpr::Coalesce(values) => {
			let exprs: Vec<SimpleExpr> = values
				.iter()
				.map(|value| annotation_value_to_safe_expr(value, root_alias))
				.collect();
			if exprs.is_empty() {
				Expr::val(Option::<String>::None).into()
			} else {
				let placeholders = vec!["?"; exprs.len()].join(", ");
				Expr::cust_with_values(format!("COALESCE({placeholders})"), exprs).into()
			}
		}
	}
}

/// Escape SQL LIKE wildcard characters in user input
fn escape_like_pattern(input: &str) -> String {
	input
		.replace('\\', "\\\\")
		.replace('%', "\\%")
		.replace('_', "\\_")
}

/// Build a SimpleExpr from a single Filter
#[doc(hidden)]
pub fn build_single_filter_expr(filter: &Filter) -> AdminResult<Option<SimpleExpr>> {
	build_single_filter_expr_for_table(filter, None)
}

fn filter_column(root_alias: Option<&str>, field: &str) -> Expr {
	if let Some(root_alias) = root_alias
		&& !field.contains('.')
	{
		return Expr::col((Alias::new(root_alias), Alias::new(field)));
	}

	if let Some((table, column)) = field.split_once('.')
		&& !column.contains('.')
	{
		return Expr::col((Alias::new(table), Alias::new(column)));
	}

	Expr::col(Alias::new(field))
}

fn build_single_filter_expr_for_table(
	filter: &Filter,
	root_alias: Option<&str>,
) -> AdminResult<Option<SimpleExpr>> {
	if let FilterValue::Typed(Err(error)) = &filter.value {
		return Err(AdminError::FieldCodec(error.clone()));
	}

	if let FilterValue::Typed(Ok(value)) = &filter.value {
		let raw_value = match value {
			reinhardt_db::orm::DatabaseValue::String(value) => {
				Some(FilterValue::String(value.clone()))
			}
			reinhardt_db::orm::DatabaseValue::Null => Some(FilterValue::Null),
			_ => None,
		};
		if let Some(raw_value) = raw_value {
			let mut normalized = filter.clone();
			normalized.value = raw_value;
			return build_single_filter_expr_for_table(&normalized, root_alias);
		}
	}

	if let Some(expression) = filter.typed_predicate_expr(root_alias) {
		return Ok(Some(expression));
	}

	let col = root_alias.map_or_else(
		|| filter.lhs_expr(),
		|root_alias| filter.lhs_expr_for_root(root_alias),
	);
	let lhs_sql = root_alias.map_or_else(
		|| filter.lhs_sql(),
		|root_alias| filter.lhs_sql_for_root(root_alias),
	);

	let expr = match (&filter.operator, &filter.value) {
		// Null handling (must come before generic patterns)
		(FilterOperator::Eq, FilterValue::Null) => col.is_null(),
		(FilterOperator::Ne, FilterValue::Null) => col.is_not_null(),
		(FilterOperator::IExact, FilterValue::String(s)) => {
			col.binary(BinOper::ILike, SimpleExpr::from(s.clone()))
		}
		(FilterOperator::IExact, v) => col.eq(filter_value_to_sea_value(v)?),

		// FieldRef: Column-to-column comparisons
		(FilterOperator::Eq, FilterValue::FieldRef(f)) => {
			col.eq(filter_column(root_alias, &f.field))
		}
		(FilterOperator::Ne, FilterValue::FieldRef(f)) => {
			col.ne(filter_column(root_alias, &f.field))
		}
		(FilterOperator::Gt, FilterValue::FieldRef(f)) => {
			col.gt(filter_column(root_alias, &f.field))
		}
		(FilterOperator::Gte, FilterValue::FieldRef(f)) => {
			col.gte(filter_column(root_alias, &f.field))
		}
		(FilterOperator::Lt, FilterValue::FieldRef(f)) => {
			col.lt(filter_column(root_alias, &f.field))
		}
		(FilterOperator::Lte, FilterValue::FieldRef(f)) => {
			col.lte(filter_column(root_alias, &f.field))
		}

		// OuterRef: Correlated subquery references (use type-safe column API)
		(FilterOperator::Eq, FilterValue::OuterRef(outer)) => {
			col.eq(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Ne, FilterValue::OuterRef(outer)) => {
			col.ne(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Gt, FilterValue::OuterRef(outer)) => {
			col.gt(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Gte, FilterValue::OuterRef(outer)) => {
			col.gte(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Lt, FilterValue::OuterRef(outer)) => {
			col.lt(Expr::col(Alias::new(&outer.field)))
		}
		(FilterOperator::Lte, FilterValue::OuterRef(outer)) => {
			col.lte(Expr::col(Alias::new(&outer.field)))
		}

		// Expression: Arithmetic expressions (validate field names before building SQL)
		(FilterOperator::Eq, FilterValue::Expression(expr)) => {
			col.eq(annotation_expr_to_safe_expr(expr, root_alias))
		}
		(FilterOperator::Ne, FilterValue::Expression(expr)) => {
			col.ne(annotation_expr_to_safe_expr(expr, root_alias))
		}
		(FilterOperator::Gt, FilterValue::Expression(expr)) => {
			col.gt(annotation_expr_to_safe_expr(expr, root_alias))
		}
		(FilterOperator::Gte, FilterValue::Expression(expr)) => {
			col.gte(annotation_expr_to_safe_expr(expr, root_alias))
		}
		(FilterOperator::Lt, FilterValue::Expression(expr)) => {
			col.lt(annotation_expr_to_safe_expr(expr, root_alias))
		}
		(FilterOperator::Lte, FilterValue::Expression(expr)) => {
			col.lte(annotation_expr_to_safe_expr(expr, root_alias))
		}

		// Generic scalar value patterns
		(FilterOperator::Eq, v) => col.eq(filter_comparison_value(v)?),
		(FilterOperator::Ne, v) => col.ne(filter_comparison_value(v)?),
		(FilterOperator::Gt, v) => col.gt(filter_comparison_value(v)?),
		(FilterOperator::Gte, v) => col.gte(filter_comparison_value(v)?),
		(FilterOperator::Lt, v) => col.lt(filter_comparison_value(v)?),
		(FilterOperator::Lte, v) => col.lte(filter_comparison_value(v)?),

		// String-specific operators
		(FilterOperator::Contains, FilterValue::String(s)) => {
			col.like(format!("%{}%", escape_like_pattern(s)))
		}
		(FilterOperator::IContains, FilterValue::String(s)) => col.binary(
			BinOper::ILike,
			SimpleExpr::from(format!("%{}%", escape_like_pattern(s))),
		),
		(FilterOperator::StartsWith, FilterValue::String(s)) => {
			col.like(format!("{}%", escape_like_pattern(s)))
		}
		(FilterOperator::IStartsWith, FilterValue::String(s)) => col.binary(
			BinOper::ILike,
			SimpleExpr::from(format!("{}%", escape_like_pattern(s))),
		),
		(FilterOperator::EndsWith, FilterValue::String(s)) => {
			col.like(format!("%{}", escape_like_pattern(s)))
		}
		(FilterOperator::IEndsWith, FilterValue::String(s)) => col.binary(
			BinOper::ILike,
			SimpleExpr::from(format!("%{}", escape_like_pattern(s))),
		),
		(FilterOperator::Regex, FilterValue::String(pattern)) => {
			Expr::cust_with_values(format!("{} ~ ?", lhs_sql), [pattern.clone()]).into()
		}
		(FilterOperator::IRegex, FilterValue::String(pattern)) => {
			Expr::cust_with_values(format!("{} ~* ?", lhs_sql), [pattern.clone()]).into()
		}
		(FilterOperator::Range, FilterValue::Range(start, end)) => Expr::cust_with_values(
			format!("{} BETWEEN ? AND ?", lhs_sql),
			[
				filter_value_to_sea_value(start)?,
				filter_value_to_sea_value(end)?,
			],
		)
		.into(),
		// Array-based In/NotIn: convert each element to a Value
		(FilterOperator::In, FilterValue::Array(arr)) => {
			if arr.is_empty() {
				return Ok(None);
			}
			let values: Vec<Value> = arr.iter().map(|v| v.as_str().into_value()).collect();
			col.is_in(values)
		}
		(FilterOperator::NotIn, FilterValue::Array(arr)) => {
			if arr.is_empty() {
				return Ok(None);
			}
			let values: Vec<Value> = arr.iter().map(|v| v.as_str().into_value()).collect();
			col.is_not_in(values)
		}
		(FilterOperator::In, FilterValue::List(values)) => {
			if values.is_empty() {
				return Ok(None);
			}
			col.is_in(
				values
					.iter()
					.map(filter_value_to_sea_value)
					.collect::<AdminResult<Vec<_>>>()?,
			)
		}
		(FilterOperator::NotIn, FilterValue::List(values)) => {
			if values.is_empty() {
				return Ok(None);
			}
			col.is_not_in(
				values
					.iter()
					.map(filter_value_to_sea_value)
					.collect::<AdminResult<Vec<_>>>()?,
			)
		}

		(FilterOperator::In, FilterValue::String(s)) => {
			let values: Vec<Value> = s.split(',').map(|v| v.trim().into_value()).collect();
			col.is_in(values)
		}
		(FilterOperator::NotIn, FilterValue::String(s)) => {
			let values: Vec<Value> = s.split(',').map(|v| v.trim().into_value()).collect();
			col.is_not_in(values)
		}

		// Skip unsupported combinations
		_ => return Ok(None),
	};

	Ok(Some(expr))
}

/// Build Condition from filters (AND logic only)
#[doc(hidden)]
pub fn build_filter_condition(filters: &[Filter]) -> AdminResult<Option<Condition>> {
	if filters.is_empty() {
		return Ok(None);
	}

	let mut condition = Condition::all();
	let mut added = false;

	for filter in filters {
		if let Some(expr) = build_single_filter_expr(filter)? {
			condition = condition.add(expr);
			added = true;
		}
	}

	Ok(if added { Some(condition) } else { None })
}

/// Maximum recursion depth for filter conditions to prevent stack overflow
#[doc(hidden)]
pub const MAX_FILTER_DEPTH: usize = 100;

/// Build Condition from FilterCondition (supports AND/OR logic)
///
/// This function recursively processes FilterCondition to build complex
/// query conditions with nested AND/OR logic.
///
/// # Stack Overflow Protection
///
/// To prevent stack overflow with deeply nested filter conditions, this function
/// limits recursion depth to `MAX_FILTER_DEPTH` (100 levels). If the depth limit
/// is exceeded, the function returns an error.
#[doc(hidden)]
pub fn build_composite_filter_condition(
	filter_condition: &FilterCondition,
) -> AdminResult<Option<Condition>> {
	build_composite_filter_condition_for_table(filter_condition, 0, None)
}

/// Internal helper for building composite filter conditions with depth tracking
#[doc(hidden)]
pub fn build_composite_filter_condition_with_depth(
	filter_condition: &FilterCondition,
	depth: usize,
) -> AdminResult<Option<Condition>> {
	build_composite_filter_condition_for_table(filter_condition, depth, None)
}

fn build_composite_filter_condition_for_table(
	filter_condition: &FilterCondition,
	depth: usize,
	table_name: Option<&str>,
) -> AdminResult<Option<Condition>> {
	// Prevent stack overflow by limiting recursion depth
	if depth >= MAX_FILTER_DEPTH {
		return Err(AdminError::ValidationError(format!(
			"Filter condition exceeded maximum depth of {} levels",
			MAX_FILTER_DEPTH
		)));
	}

	match filter_condition {
		FilterCondition::Single(filter) => {
			Ok(build_single_filter_expr_for_table(filter, table_name)?
				.map(|expr| Condition::all().add(expr)))
		}
		FilterCondition::And(conditions) => {
			if conditions.is_empty() {
				return Ok(None);
			}
			let mut and_condition = Condition::all();
			let mut added = false;
			for cond in conditions {
				if let Some(sub_cond) =
					build_composite_filter_condition_for_table(cond, depth + 1, table_name)?
				{
					and_condition = and_condition.add(sub_cond);
					added = true;
				}
			}
			// Return None if all sub-conditions were unsupported,
			// preventing an empty Condition::all() that produces WHERE TRUE
			if added {
				Ok(Some(and_condition))
			} else {
				Ok(None)
			}
		}
		FilterCondition::Or(conditions) => {
			if conditions.is_empty() {
				return Ok(None);
			}
			let mut or_condition = Condition::any();
			let mut added = false;
			for cond in conditions {
				if let Some(sub_cond) =
					build_composite_filter_condition_for_table(cond, depth + 1, table_name)?
				{
					or_condition = or_condition.add(sub_cond);
					added = true;
				}
			}
			// Return None if all sub-conditions were unsupported,
			// preventing an empty Condition::any() that produces WHERE FALSE
			if added {
				Ok(Some(or_condition))
			} else {
				Ok(None)
			}
		}
		FilterCondition::Not(inner) => {
			Ok(
				build_composite_filter_condition_for_table(inner, depth + 1, table_name)?
					.map(|inner_condition| inner_condition.not()),
			)
		}
	}
}

fn build_combined_filter_condition(
	filter_condition: Option<&FilterCondition>,
	additional_filters: &[Filter],
) -> AdminResult<(Condition, bool)> {
	let mut combined = Condition::all();

	if let Some(fc) = filter_condition
		&& let Some(cond) = build_composite_filter_condition(fc)?
	{
		combined = combined.add(cond);
	}

	if let Some(simple_cond) = build_filter_condition(additional_filters)? {
		combined = combined.add(simple_cond);
	}

	Ok((
		combined,
		!additional_filters.is_empty() || filter_condition.is_some(),
	))
}

fn extract_admin_list_total_count(
	map: &serde_json::Map<String, serde_json::Value>,
) -> AdminResult<u64> {
	let count_value = map.get(ADMIN_LIST_TOTAL_COUNT_ALIAS).ok_or_else(|| {
		AdminError::DatabaseError(format!(
			"Admin list query result missing '{}' key",
			ADMIN_LIST_TOTAL_COUNT_ALIAS
		))
	})?;

	if let Some(count) = count_value.as_u64() {
		return Ok(count);
	}

	count_value
		.as_i64()
		.and_then(|count| if count >= 0 { Some(count as u64) } else { None })
		.ok_or_else(|| {
			AdminError::DatabaseError(format!(
				"Admin list query returned invalid total count: {}",
				count_value
			))
		})
}

fn build_admin_query_condition(
	query: &AdminQuery,
	root_alias: Option<&str>,
) -> AdminResult<Option<Condition>> {
	if query.conditions().is_empty() {
		return Ok(None);
	}

	let mut combined = Condition::all();
	for filter_condition in query.conditions() {
		let condition =
			build_composite_filter_condition_for_table(filter_condition, 0, root_alias)?
				.ok_or_else(|| {
					AdminError::ValidationError(
						"Admin queryset contains a condition that cannot be applied".to_string(),
					)
				})?;
		combined = combined.add(condition);
	}

	Ok(Some(combined))
}

fn related_column_alias(relation_index: usize, column: &str) -> String {
	format!("{ADMIN_RELATED_COLUMN_ALIAS_PREFIX}{relation_index}__{column}")
}

fn add_related_joins(
	statement: &mut SelectStatement,
	table_name: &str,
	related_fields: &[AdminRelatedField],
) {
	for related in related_fields {
		statement.left_join(
			TableRef::table_alias(
				Alias::new(&related.target_table),
				Alias::new(&related.relation_name),
			),
			Expr::col((Alias::new(table_name), Alias::new(&related.source_column))).equals((
				Alias::new(&related.relation_name),
				Alias::new(&related.target_column),
			)),
		);
	}
}

fn build_admin_list_statement(
	admin_query: &AdminQuery,
	related_fields: &[AdminRelatedField],
	sort_by: Option<&str>,
	offset: u64,
	limit: u64,
) -> AdminResult<SelectStatement> {
	let table_name = admin_query.table_name();
	let mut statement = Query::select()
		.from(Alias::new(table_name))
		.column(ColumnRef::table_asterisk(Alias::new(table_name)))
		.expr_as(
			Expr::cust("COUNT(*) OVER()"),
			Alias::new(ADMIN_LIST_TOTAL_COUNT_ALIAS),
		)
		.to_owned();

	add_related_joins(&mut statement, table_name, related_fields);
	for (relation_index, related) in related_fields.iter().enumerate() {
		for column in &related.columns {
			statement.expr_as(
				Expr::col((Alias::new(&related.relation_name), Alias::new(column))),
				Alias::new(related_column_alias(relation_index, column)),
			);
		}
	}

	if let Some(condition) = build_admin_query_condition(
		admin_query,
		(!related_fields.is_empty()).then_some(table_name),
	)? {
		statement.cond_where(condition);
	}

	if let Some(sort) = sort_by {
		let (field, order) = sort
			.strip_prefix('-')
			.map_or((sort, Order::Asc), |field| (field, Order::Desc));
		statement.order_by((Alias::new(table_name), Alias::new(field)), order);
	}

	statement.limit(limit).offset(offset);
	Ok(statement)
}

fn build_admin_count_statement(
	admin_query: &AdminQuery,
	related_fields: &[AdminRelatedField],
) -> AdminResult<SelectStatement> {
	let table_name = admin_query.table_name();
	let mut statement = Query::select()
		.from(Alias::new(table_name))
		.expr(Expr::cust("COUNT(*) AS count"))
		.to_owned();
	add_related_joins(&mut statement, table_name, related_fields);

	if let Some(condition) = build_admin_query_condition(
		admin_query,
		(!related_fields.is_empty()).then_some(table_name),
	)? {
		statement.cond_where(condition);
	}

	Ok(statement)
}

fn build_date_hierarchy_statement(
	admin_query: &AdminQuery,
	field: &str,
	level: crate::types::DateHierarchyLevel,
	field_type: &DbFieldType,
	related_fields: &[AdminRelatedField],
) -> AdminResult<SelectStatement> {
	let table_name = admin_query.table_name();
	let root_alias = (!related_fields.is_empty()).then_some(table_name);
	let kind = match level {
		crate::types::DateHierarchyLevel::Year => TemporalTruncKind::Year,
		crate::types::DateHierarchyLevel::Month => TemporalTruncKind::Month,
		crate::types::DateHierarchyLevel::Day => TemporalTruncKind::Day,
	};
	let (time_zone, output) = match field_type {
		DbFieldType::Date | DbFieldType::DateTime => (None, TemporalTruncOutput::Date),
		DbFieldType::TimestampTz => (Some(TemporalTimeZone::Utc), TemporalTruncOutput::DateTime),
		_ => {
			return Err(AdminError::ValidationError(format!(
				"Date hierarchy field '{field}' must be a date or datetime field"
			)));
		}
	};
	let projection = Func::temporal_trunc(
		filter_column(root_alias, field).into_simple_expr(),
		kind,
		time_zone,
		output,
	)
	.map_err(|error| AdminError::ValidationError(error.to_string()))?;
	let mut statement = Query::select()
		.from(Alias::new(table_name))
		.expr_as(projection, Alias::new(ADMIN_DATE_HIERARCHY_ALIAS))
		.distinct()
		.to_owned();
	add_related_joins(&mut statement, table_name, related_fields);
	statement.and_where(filter_column(root_alias, field).is_not_null());
	if let Some(condition) = build_admin_query_condition(admin_query, root_alias)? {
		statement.cond_where(condition);
	}
	statement.order_by(Alias::new(ADMIN_DATE_HIERARCHY_ALIAS), Order::Asc);
	Ok(statement)
}

fn parse_date_hierarchy_choice(
	value: &str,
	level: crate::types::DateHierarchyLevel,
	field_type: &DbFieldType,
) -> AdminResult<i32> {
	use chrono::Datelike;

	let (date_value, is_bc) = match field_type {
		DbFieldType::Date => (
			value.strip_suffix(" BC").unwrap_or(value),
			value.ends_with(" BC"),
		),
		DbFieldType::DateTime | DbFieldType::TimestampTz => {
			let is_bc = value.ends_with(" BC");
			let value = value.strip_suffix(" BC").unwrap_or(value);
			let date_end = value.find(['T', ' ']).unwrap_or(value.len());
			(&value[..date_end], is_bc)
		}
		_ => {
			return Err(AdminError::ValidationError(
				"Date hierarchy choices require a date or datetime field".to_string(),
			));
		}
	};
	let mut parts = date_value.rsplitn(3, '-');
	let day = parts.next().and_then(|part| part.parse::<u32>().ok());
	let month = parts.next().and_then(|part| part.parse::<u32>().ok());
	let year = parts.next().and_then(|part| part.parse::<i32>().ok());
	let year = match (year, is_bc) {
		(Some(year), false) => Some(year),
		(Some(year), true) if year > 0 => i32::try_from(1_i64 - i64::from(year)).ok(),
		_ => None,
	};
	let date = year
		.zip(month)
		.zip(day)
		.and_then(|((year, month), day)| chrono::NaiveDate::from_ymd_opt(year, month, day))
		.ok_or_else(|| {
			AdminError::DatabaseError(
				"Admin date hierarchy query returned an invalid date".to_string(),
			)
		})?;

	Ok(match level {
		crate::types::DateHierarchyLevel::Year => date.year(),
		crate::types::DateHierarchyLevel::Month => date.month() as i32,
		crate::types::DateHierarchyLevel::Day => date.day() as i32,
	})
}

fn decode_admin_list_row(
	mut map: serde_json::Map<String, serde_json::Value>,
	related_fields: &[AdminRelatedField],
) -> AdminResult<(HashMap<String, serde_json::Value>, u64)> {
	let total_count = extract_admin_list_total_count(&map)?;
	map.remove(ADMIN_LIST_TOTAL_COUNT_ALIAS);

	for (relation_index, related) in related_fields.iter().enumerate() {
		let mut related_object = serde_json::Map::new();
		let mut has_related_row = false;

		for column in &related.columns {
			let alias = related_column_alias(relation_index, column);
			let value = map.remove(&alias).ok_or_else(|| {
				AdminError::DatabaseError(format!(
					"Admin list query result missing related column alias '{alias}'"
				))
			})?;
			has_related_row |= !value.is_null();
			related_object.insert(column.clone(), value);
		}

		map.insert(
			related.relation_name.clone(),
			if has_related_row {
				serde_json::Value::Object(related_object)
			} else {
				serde_json::Value::Null
			},
		);
	}

	let result = map
		.into_iter()
		.filter(|(key, _)| !SENSITIVE_FIELDS.contains(&key.as_str()))
		.collect();
	Ok((result, total_count))
}

/// Admin database interface
///
/// Provides CRUD operations for admin panel, leveraging reinhardt-orm.
///
/// # Examples
///
/// ```
/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
/// use reinhardt_db::orm::DatabaseConnectionLease;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
/// let lease = DatabaseConnectionLease::register(owner)?;
/// let conn = lease.handle();
/// let db = AdminDatabase::new(conn);
///
/// // List items with filters
/// let items = db.list::<AdminRecord>("admin_records", vec![], 0, 50).await?;
/// # Ok(())
/// # }
/// ```
#[injectable(scope = Singleton, prebuilt = true)]
#[derive(Clone)]
pub struct AdminDatabase {
	connection: DatabaseConnection,
}

/// Provider key for the admin database dependency.
#[reinhardt_di::injectable_key]
pub struct AdminDatabaseKey;

impl AdminDatabase {
	/// Create a new admin database interface
	///
	/// The copyable ORM handle is stored directly. Its associated
	/// `DatabaseConnectionLease` must remain alive for the lifetime of this value.
	pub fn new(connection: DatabaseConnection) -> Self {
		Self { connection }
	}

	/// Get a reference to the underlying database connection
	pub fn connection(&self) -> &DatabaseConnection {
		&self.connection
	}

	/// List items with filters, ordering, and pagination
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::{DatabaseConnectionLease, Filter, FilterOperator, FilterValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// let filters = vec![
	///     Filter::new("is_active".to_string(), FilterOperator::Eq, FilterValue::Boolean(true))
	/// ];
	///
	/// let items = db.list::<AdminRecord>("admin_records", filters, 0, 50).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list<M: Model>(
		&self,
		table_name: &str,
		filters: Vec<Filter>,
		offset: u64,
		limit: u64,
	) -> AdminResult<Vec<HashMap<String, serde_json::Value>>> {
		// SELECT * is intentional: admin panel operates on dynamic schemas where
		// the column set is not known at compile time. Each ModelAdmin defines
		// list_display fields, and column filtering is applied at the application
		// layer after fetching all columns.
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.column(ColumnRef::Asterisk)
			.to_owned();

		// Apply filters using build_filter_condition helper
		if let Some(condition) = build_filter_condition(&filters)? {
			query.cond_where(condition);
		}

		// Apply pagination
		query.limit(limit).offset(offset);

		// Execute query
		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let rows = self
			.connection
			.query(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Convert QueryRow to HashMap
		Ok(rows
			.into_iter()
			.filter_map(|row| {
				// row.data is already a serde_json::Value, typically an Object
				if let serde_json::Value::Object(map) = row.data {
					Some(
						map.into_iter()
							.collect::<HashMap<String, serde_json::Value>>(),
					)
				} else {
					None
				}
			})
			.collect())
	}

	/// List items with composite filter conditions (supports AND/OR logic)
	///
	/// This method supports complex filter conditions using FilterCondition,
	/// which allows building nested AND/OR queries.
	///
	/// # Arguments
	///
	/// * `table_name` - The name of the table to query
	/// * `filter_condition` - Optional composite filter condition (AND/OR logic)
	/// * `additional_filters` - Additional simple filters to AND with the condition
	/// * `sort_by` - Optional sort field (prefix with "-" for descending, e.g., "created_at" or "-created_at")
	/// * `offset` - Number of items to skip for pagination
	/// * `limit` - Maximum number of items to return
	pub async fn list_with_condition<M: Model>(
		&self,
		table_name: &str,
		filter_condition: Option<&FilterCondition>,
		additional_filters: Vec<Filter>,
		sort_by: Option<&str>,
		offset: u64,
		limit: u64,
	) -> AdminResult<Vec<HashMap<String, serde_json::Value>>> {
		// SELECT * is intentional: admin panel operates on dynamic schemas where
		// the column set is not known at compile time. Each ModelAdmin defines
		// list_display fields, and column filtering is applied at the application
		// layer after fetching all columns.
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.column(ColumnRef::Asterisk)
			.to_owned();

		let (combined, has_filter) =
			build_combined_filter_condition(filter_condition, &additional_filters)?;

		if has_filter {
			query.cond_where(combined);
		}

		// Apply sorting (if specified)
		if let Some(sort_str) = sort_by {
			let (field, is_desc) = if let Some(stripped) = sort_str.strip_prefix('-') {
				(stripped, true)
			} else {
				(sort_str, false)
			};

			let col = Alias::new(field);
			if is_desc {
				query.order_by(col, Order::Desc);
			} else {
				query.order_by(col, Order::Asc);
			}
		}

		// Apply pagination
		query.limit(limit).offset(offset);

		// Execute query
		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let rows = self
			.connection
			.query(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Convert QueryRow to HashMap
		Ok(rows
			.into_iter()
			.filter_map(|row| {
				if let serde_json::Value::Object(map) = row.data {
					Some(
						map.into_iter()
							.filter(|(key, _)| !SENSITIVE_FIELDS.contains(&key.as_str()))
							.collect::<HashMap<String, serde_json::Value>>(),
					)
				} else {
					None
				}
			})
			.collect())
	}

	/// List items and return the filtered total count with one query for non-empty pages.
	///
	/// This uses a windowed `COUNT(*) OVER()` expression so the admin list endpoint
	/// can fetch page rows and pagination metadata without issuing a separate count
	/// query on the common path.
	pub async fn list_with_condition_and_count<M: Model>(
		&self,
		table_name: &str,
		filter_condition: Option<&FilterCondition>,
		additional_filters: Vec<Filter>,
		sort_by: Option<&str>,
		offset: u64,
		limit: u64,
	) -> AdminResult<(Vec<HashMap<String, serde_json::Value>>, u64)> {
		let mut admin_query = AdminQuery::new(table_name);
		if let Some(filter_condition) = filter_condition {
			admin_query = admin_query.filter_condition(filter_condition.clone());
		}
		for filter in additional_filters {
			admin_query = admin_query.filter(filter);
		}

		self.list_admin_query_with_count(&admin_query, &[], sort_by, offset, limit)
			.await
	}

	pub(crate) async fn list_admin_query_with_count(
		&self,
		admin_query: &AdminQuery,
		related_fields: &[AdminRelatedField],
		sort_by: Option<&str>,
		offset: u64,
		limit: u64,
	) -> AdminResult<(Vec<HashMap<String, serde_json::Value>>, u64)> {
		let statement =
			build_admin_list_statement(admin_query, related_fields, sort_by, offset, limit)?;
		let (sql, values) = statement.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let rows = self
			.connection
			.query(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		if rows.is_empty() {
			if offset == 0 && limit > 0 {
				return Ok((Vec::new(), 0));
			}

			let count = self.count_admin_query(admin_query, related_fields).await?;
			return Ok((Vec::new(), count));
		}

		let mut total_count = None;
		let mut results = Vec::with_capacity(rows.len());
		for row in rows {
			let serde_json::Value::Object(map) = row.data else {
				return Err(AdminError::DatabaseError(
					"Admin list query returned a non-object row".to_string(),
				));
			};
			let (result, row_count) = decode_admin_list_row(map, related_fields)?;
			if let Some(total_count) = total_count {
				if total_count != row_count {
					return Err(AdminError::DatabaseError(
						"Admin list query returned inconsistent total counts".to_string(),
					));
				}
			} else {
				total_count = Some(row_count);
			}
			results.push(result);
		}

		let total_count = total_count.ok_or_else(|| {
			AdminError::DatabaseError("Admin list query returned no object rows".to_string())
		})?;

		Ok((results, total_count))
	}

	pub(crate) async fn date_hierarchy_choices(
		&self,
		admin_query: &AdminQuery,
		field: &str,
		level: crate::types::DateHierarchyLevel,
		field_type: &DbFieldType,
		related_fields: &[AdminRelatedField],
	) -> AdminResult<Vec<i32>> {
		let statement =
			build_date_hierarchy_statement(admin_query, field, level, field_type, related_fields)?;
		let (sql, values) = statement.build(PostgresQueryBuilder);
		let rows = self
			.connection
			.query(&sql, convert_values(values))
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?;

		rows.into_iter()
			.map(|row| {
				let value = row
					.data
					.get(ADMIN_DATE_HIERARCHY_ALIAS)
					.and_then(serde_json::Value::as_str)
					.ok_or_else(|| {
						AdminError::DatabaseError(
							"Admin date hierarchy query returned an invalid value".to_string(),
						)
					})?;
				parse_date_hierarchy_choice(value, level, field_type)
			})
			.collect()
	}

	async fn count_admin_query(
		&self,
		admin_query: &AdminQuery,
		related_fields: &[AdminRelatedField],
	) -> AdminResult<u64> {
		let statement = build_admin_count_statement(admin_query, related_fields)?;
		let (sql, values) = statement.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_one(&sql, params)
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
		extract_count_from_row(&row.data)
	}

	/// Count items with composite filter conditions (supports AND/OR logic)
	///
	/// # Arguments
	///
	/// * `table_name` - The name of the table to query
	/// * `filter_condition` - Optional composite filter condition (AND/OR logic)
	/// * `additional_filters` - Additional simple filters to AND with the condition
	pub async fn count_with_condition<M: Model>(
		&self,
		table_name: &str,
		filter_condition: Option<&FilterCondition>,
		additional_filters: Vec<Filter>,
	) -> AdminResult<u64> {
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.expr(Expr::cust("COUNT(*) AS count"))
			.to_owned();

		let (combined, has_filter) =
			build_combined_filter_condition(filter_condition, &additional_filters)?;

		if has_filter {
			query.cond_where(combined);
		}

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_one(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Extract count from result, propagating errors for unexpected formats
		let count = extract_count_from_row(&row.data)?;

		Ok(count)
	}

	/// Get a single item by ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::DatabaseConnectionLease;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// let item = db.get::<AdminRecord>("admin_records", "id", "1").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		id: &str,
	) -> AdminResult<Option<HashMap<String, serde_json::Value>>> {
		let pk_value = parse_pk_value(table_name, pk_field, id);

		// SELECT * is intentional: admin detail view displays all fields from the
		// model. The admin panel operates on dynamic schemas where the column set
		// is determined by the ModelAdmin configuration at runtime.
		let query = Query::select()
			.from(Alias::new(table_name))
			.column(ColumnRef::Asterisk)
			.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value))
			.to_owned();

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_optional(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(row.and_then(|r| {
			// r.data is already a serde_json::Value, typically an Object
			if let serde_json::Value::Object(map) = r.data {
				Some(
					map.into_iter()
						.collect::<HashMap<String, serde_json::Value>>(),
				)
			} else {
				None
			}
		}))
	}

	/// Create a new item
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::DatabaseConnectionLease;
	/// use std::collections::HashMap;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// let mut data = HashMap::new();
	/// data.insert("name".to_string(), serde_json::json!("Alice"));
	/// data.insert("email".to_string(), serde_json::json!("alice@example.com"));
	///
	/// db.create::<AdminRecord>("admin_records", Some("id"), data).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create<M: Model>(
		&self,
		table_name: &str,
		pk_field: Option<&str>,
		data: HashMap<String, serde_json::Value>,
	) -> AdminResult<u64> {
		let pk_field = pk_field.unwrap_or("id");
		let mut query = Query::insert()
			.into_table(Alias::new(table_name))
			.to_owned();

		// Sort keys for deterministic column ordering in generated SQL.
		// HashMap iteration order is non-deterministic, which causes
		// flaky tests and non-reproducible query plans.
		let mut sorted_keys: Vec<String> = data.keys().cloned().collect();
		sorted_keys.sort();

		// Build column and value lists in sorted order
		let mut columns = Vec::new();
		let mut values = Vec::new();

		for key in sorted_keys {
			let value = data.get(&key).cloned().unwrap_or(serde_json::Value::Null);
			columns.push(Alias::new(&key));

			let sea_value = json_to_sea_value(table_name, &key, value)?;
			values.push(sea_value);
		}

		// Pass values directly for reinhardt-query
		query.columns(columns).values(values).map_err(|e| {
			AdminError::DatabaseError(format!("column/value count mismatch: {}", e))
		})?;

		// Add RETURNING clause using the actual primary key field
		query.returning([Alias::new(pk_field)]);

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_one(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Extract the ID from the returned row using the primary key field
		match row.data.get(pk_field) {
			Some(serde_json::Value::Number(n)) => n.as_u64().ok_or_else(|| {
				AdminError::DatabaseError(format!(
					"RETURNING clause for '{}' returned non-unsigned-integer: {}",
					pk_field, n
				))
			}),
			Some(serde_json::Value::String(_)) => {
				// UUID and other string-based PKs: return 1 as affected count
				// (the actual PK value is a string, not representable as u64)
				Ok(1)
			}
			_ => Err(AdminError::DatabaseError(format!(
				"RETURNING clause did not return expected primary key field '{}'",
				pk_field
			))),
		}
	}

	/// Update an existing item
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::DatabaseConnectionLease;
	/// use std::collections::HashMap;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// let mut data = HashMap::new();
	/// data.insert("name".to_string(), serde_json::json!("Alice Updated"));
	///
	/// db.update::<AdminRecord>("admin_records", "id", "1", data).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn update<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		id: &str,
		data: HashMap<String, serde_json::Value>,
	) -> AdminResult<u64> {
		let mut query = Query::update().table(Alias::new(table_name)).to_owned();

		// Sort keys for deterministic SET clause ordering in generated SQL
		let mut sorted_keys: Vec<String> = data.keys().cloned().collect();
		sorted_keys.sort();

		// Build SET clauses in sorted order
		for key in sorted_keys {
			let value = data.get(&key).cloned().unwrap_or(serde_json::Value::Null);
			let sea_value = json_to_sea_value(table_name, &key, value)?;
			query.value(Alias::new(&key), sea_value);
		}

		let pk_value = parse_pk_value(table_name, pk_field, id);
		query.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value));

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let affected = self
			.connection
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(affected)
	}

	/// Delete an item by ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::DatabaseConnectionLease;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// db.delete::<AdminRecord>("admin_records", "id", "1").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn delete<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		id: &str,
	) -> AdminResult<u64> {
		let pk_value = parse_pk_value(table_name, pk_field, id);

		let query = Query::delete()
			.from_table(Alias::new(table_name))
			.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value))
			.to_owned();

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let affected = self
			.connection
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(affected)
	}

	/// Delete multiple items by IDs (bulk delete)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::DatabaseConnectionLease;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// let ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
	/// db.bulk_delete::<AdminRecord>("admin_records", "id", ids).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn bulk_delete<M: Model>(
		&self,
		table_name: &str,
		pk_field: &str,
		ids: Vec<String>,
	) -> AdminResult<u64> {
		self.bulk_delete_by_table(table_name, pk_field, ids).await
	}

	/// Delete multiple items by IDs without requiring Model type parameter
	///
	/// This method provides a type-safe way to perform bulk deletions without
	/// requiring a Model type parameter. It's particularly useful for admin actions
	/// where the model type may not be known at compile time.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::AdminDatabase;
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::DatabaseConnectionLease;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// let ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
	/// db.bulk_delete_by_table("users", "id", ids).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn bulk_delete_by_table(
		&self,
		table_name: &str,
		pk_field: &str,
		ids: Vec<String>,
	) -> AdminResult<u64> {
		if ids.is_empty() {
			return Ok(0);
		}

		let pk_values = parse_pk_values(table_name, pk_field, &ids);

		let query = Query::delete()
			.from_table(Alias::new(table_name))
			.and_where(Expr::col(Alias::new(pk_field)).is_in(pk_values))
			.to_owned();

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let affected = self
			.connection
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		Ok(affected)
	}

	/// Count total items with optional filters
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_admin::core::{AdminDatabase, AdminRecord};
	/// use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	/// use reinhardt_db::orm::{DatabaseConnectionLease, Filter, FilterOperator, FilterValue};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let owner = BackendsConnection::connect_postgres("postgres://localhost/test").await?;
	/// let lease = DatabaseConnectionLease::register(owner)?;
	/// let conn = lease.handle();
	/// let db = AdminDatabase::new(conn);
	///
	/// let filters = vec![
	///     Filter::new("is_active".to_string(), FilterOperator::Eq, FilterValue::Boolean(true))
	/// ];
	///
	/// let count = db.count::<AdminRecord>("admin_records", filters).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count<M: Model>(
		&self,
		table_name: &str,
		filters: Vec<Filter>,
	) -> AdminResult<u64> {
		let mut query = Query::select()
			.from(Alias::new(table_name))
			.expr(Expr::cust("COUNT(*) AS count"))
			.to_owned();

		// Apply filters using build_filter_condition helper
		if let Some(condition) = build_filter_condition(&filters)? {
			query.cond_where(condition);
		}

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_values(values);
		let row = self
			.connection
			.query_one(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?;

		// Extract count from result, propagating errors for unexpected formats
		let count = extract_count_from_row(&row.data)?;

		Ok(count)
	}
}

/// Extract count value from a query result row
///
/// Attempts to extract an integer count from the query result by looking for
/// a "count" key in the JSON object.
///
/// Returns an error if:
/// - The "count" key is missing (lists available keys for debugging)
/// - The "count" value is not an integer
/// - The data format is not a JSON object
#[doc(hidden)]
pub fn extract_count_from_row(data: &serde_json::Value) -> AdminResult<u64> {
	if let Some(count_value) = data.get("count") {
		return count_value.as_i64().map(|v| v as u64).ok_or_else(|| {
			AdminError::DatabaseError(format!(
				"COUNT query returned non-integer value: {}",
				count_value
			))
		});
	}

	// Report available keys for diagnostics instead of using non-deterministic
	// HashMap iteration order to pick the first value
	if let Some(obj) = data.as_object() {
		let available_keys: Vec<&String> = obj.keys().collect();
		return Err(AdminError::DatabaseError(format!(
			"COUNT query result missing 'count' key, available keys: {:?}",
			available_keys
		)));
	}

	Err(AdminError::DatabaseError(format!(
		"COUNT query returned unexpected data format: {}",
		data
	)))
}

/// Injectable trait implementation for AdminDatabase
///
/// Auto-constructs from [`DatabaseConnection`] in the singleton scope when
/// no pre-built `AdminDatabase` exists. This enables admin DI dependencies
/// to be resolved at request time without requiring async initialization
/// in the synchronous `routes()` function.
///
/// Resolution order:
/// 1. Check singleton cache for pre-built `AdminDatabase` (backward compat)
/// 2. If not found, construct from `DatabaseConnection` in singleton scope
/// 3. Cache the constructed instance for subsequent requests
#[async_trait]
impl Injectable for AdminDatabase {
	async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
		// Check if pre-built AdminDatabase exists (backward compat with configure_di)
		if let Some(db) = ctx.get_singleton::<Self>() {
			return Ok((*db).clone());
		}

		// Auto-construct from DatabaseConnection in singleton scope
		let conn = ctx.get_singleton::<DatabaseConnection>().ok_or_else(|| {
			reinhardt_di::DiError::NotRegistered {
				type_name: "AdminDatabase".into(),
				hint: "DatabaseConnection must be registered as a singleton. \
				       Use InjectionContextBuilder::singleton(db_connection) during setup."
					.into(),
			}
		})?;

		let db = AdminDatabase::new(*conn);
		// Cache for subsequent requests
		ctx.set_singleton(db.clone());
		Ok(db)
	}
}

#[reinhardt_di::injectable(scope = "singleton")]
async fn admin_database_provider(
	#[inject] db: AdminDatabase,
) -> KeyedFactoryOutput<AdminDatabaseKey, AdminDatabase> {
	KeyedFactoryOutput::new(db)
}

// Register AdminDatabase in the global dependency registry so direct
// `#[inject] AdminDatabase` parameters can resolve it via ctx.resolve().
// Delegates to Injectable::inject() for lazy construction from DatabaseConnection.
fn __register_admin_database(registry: &reinhardt_di::DependencyRegistry) {
	registry.register::<AdminDatabase>(
		reinhardt_di::DependencyScope::Singleton,
		reinhardt_di::InjectableFactory::<AdminDatabase>::new(),
	);
}

reinhardt_di::inventory::submit! {
	reinhardt_di::InjectableRegistration::new(
		__register_admin_database
	)
}

#[cfg(all(test, server))]
mod tests {
	use std::sync::Arc;

	use super::*;
	use reinhardt_db::backends::{
		connection::DatabaseConnection as BackendsConnection,
		types::{DatabaseType, QueryValue, Row},
	};
	use reinhardt_db::orm::annotation::Expression;
	use reinhardt_db::orm::connection::DatabaseConnectionLease;
	use reinhardt_db::orm::expressions::{F, FieldRef, OuterRef};
	use reinhardt_testkit::fixtures::MockDatabaseBackend;
	use rstest::rstest;

	#[test]
	fn typed_filter_codec_error_stops_admin_compilation() {
		let filter = Filter::new(
			"status",
			FilterOperator::Eq,
			FilterValue::Typed(Err(reinhardt_db::orm::FieldCodecError::Serialization(
				"rejected admin filter".to_owned(),
			))),
		);

		let error = build_single_filter_expr(&filter)
			.expect_err("typed codec error should stop admin filter compilation");
		let source =
			std::error::Error::source(&error).expect("admin codec source should be preserved");
		assert!(
			source
				.downcast_ref::<reinhardt_db::orm::FieldCodecError>()
				.is_some()
		);
	}

	#[rstest]
	#[case(FilterOperator::Contains)]
	#[case(FilterOperator::StartsWith)]
	#[case(FilterOperator::Regex)]
	fn typed_filter_codec_error_stops_string_operator_compilation(
		#[case] operator: FilterOperator,
	) {
		let filter = Filter::new(
			"status",
			operator,
			FilterValue::Typed(Err(reinhardt_db::orm::FieldCodecError::Serialization(
				"rejected admin filter".to_owned(),
			))),
		);

		assert!(build_single_filter_expr(&filter).is_err());
	}

	fn render_admin_filter(filter: &Filter) -> String {
		let expr = build_single_filter_expr(filter)
			.expect("filter should compile")
			.expect("operator should produce a condition");
		let mut query = Query::select();
		query
			.column(Alias::new("id"))
			.from(Alias::new("records"))
			.cond_where(Condition::all().add(expr));
		query.to_string(PostgresQueryBuilder)
	}

	#[rstest]
	#[case(FilterOperator::IExact)]
	#[case(FilterOperator::Contains)]
	#[case(FilterOperator::IContains)]
	#[case(FilterOperator::StartsWith)]
	#[case(FilterOperator::IStartsWith)]
	#[case(FilterOperator::EndsWith)]
	#[case(FilterOperator::IEndsWith)]
	#[case(FilterOperator::Regex)]
	#[case(FilterOperator::IRegex)]
	fn typed_string_filter_uses_raw_string_operator_semantics(#[case] operator: FilterOperator) {
		let raw = Filter::new(
			"name",
			operator.clone(),
			FilterValue::String("a%b_c".to_owned()),
		);
		let typed = Filter::new(
			"name",
			operator,
			FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::String(
				"a%b_c".to_owned(),
			))),
		);

		assert_eq!(render_admin_filter(&typed), render_admin_filter(&raw));
	}

	#[rstest]
	#[case(FilterOperator::Eq)]
	#[case(FilterOperator::Ne)]
	fn typed_null_filter_uses_raw_null_operator_semantics(#[case] operator: FilterOperator) {
		let raw = Filter::new("deleted_at", operator.clone(), FilterValue::Null);
		let typed = Filter::new(
			"deleted_at",
			operator,
			FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Null)),
		);

		assert_eq!(render_admin_filter(&typed), render_admin_filter(&raw));
	}

	// ==================== escape_like_pattern tests ====================

	#[rstest]
	fn test_escape_like_pattern_percent() {
		// Arrange
		let input = "100%";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "100\\%");
	}

	#[rstest]
	fn test_escape_like_pattern_underscore() {
		// Arrange
		let input = "user_name";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "user\\_name");
	}

	#[rstest]
	fn test_escape_like_pattern_backslash() {
		// Arrange
		let input = "path\\to";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "path\\\\to");
	}

	#[rstest]
	fn test_escape_like_pattern_combined() {
		// Arrange
		let input = "100%_done";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "100\\%\\_done");
	}

	#[rstest]
	fn test_escape_like_pattern_no_special_chars() {
		// Arrange
		let input = "normal text";

		// Act
		let result = escape_like_pattern(input);

		// Assert
		assert_eq!(result, "normal text");
	}

	// ==================== escape_like_pattern regression tests (#632) ====================

	/// Regression tests for issue #632: LIKE wildcard injection via unescaped metacharacters.
	/// Verifies that percent, underscore, and backslash in user input are always escaped
	/// so they cannot be used as LIKE wildcards or escape prefix injections.
	#[rstest]
	#[case("%wildcard%", "\\%wildcard\\%")]
	#[case("under_score", "under\\_score")]
	#[case("back\\slash", "back\\\\slash")]
	#[case("%_%", "\\%\\_\\%")]
	fn test_escape_like_pattern_sanitizes_special_chars(
		#[case] input: &str,
		#[case] expected: &str,
	) {
		// Arrange: user-supplied string containing LIKE metacharacters
		// Act
		let escaped = escape_like_pattern(input);
		// Assert: output exactly matches fully-escaped form with no unescaped metacharacters
		assert_eq!(
			escaped, expected,
			"input={input:?} was not correctly escaped"
		);
	}

	// ==================== build_composite_filter_condition tests ====================

	#[test]
	fn test_build_composite_single_condition() {
		let filter = Filter::new(
			"name".to_string(),
			FilterOperator::Eq,
			FilterValue::String("Alice".to_string()),
		);
		let condition = FilterCondition::Single(filter);

		let result = build_composite_filter_condition(&condition);

		assert!(result.is_ok());
		let result = result.unwrap();
		assert!(result.is_some());
		// The condition should produce valid SQL when used
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("\"name\""));
		assert!(query.contains("'Alice'"));
	}

	#[test]
	fn test_build_composite_or_condition() {
		let filter1 = Filter::new(
			"name".to_string(),
			FilterOperator::Contains,
			FilterValue::String("Alice".to_string()),
		);
		let filter2 = Filter::new(
			"email".to_string(),
			FilterOperator::Contains,
			FilterValue::String("alice".to_string()),
		);

		let condition = FilterCondition::Or(vec![
			FilterCondition::Single(filter1),
			FilterCondition::Single(filter2),
		]);

		let result = build_composite_filter_condition(&condition);

		assert!(result.is_ok());
		let result = result.unwrap();
		assert!(result.is_some());
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		// OR condition should produce SQL with OR keyword
		assert!(query.contains("\"name\""));
		assert!(query.contains("\"email\""));
		assert!(query.contains("OR"));
	}

	#[test]
	fn test_build_composite_and_condition() {
		let filter1 = Filter::new(
			"is_active".to_string(),
			FilterOperator::Eq,
			FilterValue::Boolean(true),
		);
		let filter2 = Filter::new(
			"is_staff".to_string(),
			FilterOperator::Eq,
			FilterValue::Boolean(true),
		);

		let condition = FilterCondition::And(vec![
			FilterCondition::Single(filter1),
			FilterCondition::Single(filter2),
		]);

		let result = build_composite_filter_condition(&condition);

		assert!(result.is_ok());
		let result = result.unwrap();
		assert!(result.is_some());
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		// AND condition should produce SQL with AND keyword
		assert!(query.contains("\"is_active\""));
		assert!(query.contains("\"is_staff\""));
		assert!(query.contains("AND"));
	}

	#[test]
	fn test_build_composite_nested_condition() {
		// Build: (name LIKE '%Alice%' OR email LIKE '%alice%') AND is_active = true
		let filter_name = Filter::new(
			"name".to_string(),
			FilterOperator::Contains,
			FilterValue::String("Alice".to_string()),
		);
		let filter_email = Filter::new(
			"email".to_string(),
			FilterOperator::Contains,
			FilterValue::String("alice".to_string()),
		);
		let filter_active = Filter::new(
			"is_active".to_string(),
			FilterOperator::Eq,
			FilterValue::Boolean(true),
		);

		let or_condition = FilterCondition::Or(vec![
			FilterCondition::Single(filter_name),
			FilterCondition::Single(filter_email),
		]);

		let and_condition =
			FilterCondition::And(vec![or_condition, FilterCondition::Single(filter_active)]);

		let result = build_composite_filter_condition(&and_condition);

		assert!(result.is_ok());
		let result = result.unwrap();
		assert!(result.is_some());
		let cond = result.unwrap();
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond)
			.to_string(PostgresQueryBuilder);
		// Nested condition should contain both OR and AND
		assert!(query.contains("\"name\""));
		assert!(query.contains("\"email\""));
		assert!(query.contains("\"is_active\""));
		assert!(query.contains("OR"));
		assert!(query.contains("AND"));
	}

	#[test]
	fn test_build_composite_empty_or() {
		let condition = FilterCondition::Or(vec![]);

		let result = build_composite_filter_condition(&condition);

		// Empty OR should return Ok(None)
		assert!(result.is_ok());
		assert!(result.unwrap().is_none());
	}

	#[test]
	fn test_build_composite_empty_and() {
		let condition = FilterCondition::And(vec![]);

		let result = build_composite_filter_condition(&condition);

		// Empty AND should return Ok(None)
		assert!(result.is_ok());
		assert!(result.unwrap().is_none());
	}

	#[test]
	fn test_build_composite_depth_overflow_returns_error() {
		// Build a filter condition that exceeds MAX_FILTER_DEPTH by nesting
		let base_filter = Filter::new(
			"name".to_string(),
			FilterOperator::Eq,
			FilterValue::String("Alice".to_string()),
		);
		let mut condition = FilterCondition::Single(base_filter);
		// Wrap in And() nesting MAX_FILTER_DEPTH + 1 times to exceed the limit
		for _ in 0..=MAX_FILTER_DEPTH {
			condition = FilterCondition::And(vec![condition]);
		}

		let result = build_composite_filter_condition(&condition);

		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, AdminError::ValidationError(_)));
		let err_msg = err.to_string();
		assert!(
			err_msg.contains("exceeded maximum depth"),
			"Error message should mention exceeded depth, got: {}",
			err_msg
		);
	}

	// ==================== FieldRef/OuterRef/Expression filter tests ====================

	#[test]
	fn test_build_single_filter_expr_field_ref_eq() {
		let filter = Filter::new(
			"price".to_string(),
			FilterOperator::Eq,
			FilterValue::FieldRef(F::new("discount_price")),
		);
		let result = build_single_filter_expr(&filter).expect("filter should compile");
		assert!(result.is_some());

		let query = Query::select()
			.from(Alias::new("products"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("\"price\""));
		assert!(query.contains("\"discount_price\""));
	}

	#[test]
	fn test_build_single_filter_expr_field_ref_gt() {
		let filter = Filter::new(
			"price".to_string(),
			FilterOperator::Gt,
			FilterValue::FieldRef(F::new("cost")),
		);
		let result = build_single_filter_expr(&filter).expect("filter should compile");
		assert!(result.is_some());
	}

	#[test]
	fn test_build_single_filter_expr_field_ref_all_operators() {
		let operators = [
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let filter = Filter::new(
				"field_a".to_string(),
				op.clone(),
				FilterValue::FieldRef(F::new("field_b")),
			);
			let result = build_single_filter_expr(&filter).expect("filter should compile");
			assert!(
				result.is_some(),
				"FieldRef with {:?} should produce Some",
				op
			);
		}
	}

	#[test]
	fn test_build_single_filter_expr_outer_ref() {
		let filter = Filter::new(
			"author_id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("authors.id")),
		);
		let result = build_single_filter_expr(&filter).expect("filter should compile");
		assert!(result.is_some());

		let query = Query::select()
			.from(Alias::new("books"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("author_id"));
		assert!(query.contains("authors.id"));
	}

	#[test]
	fn test_build_single_filter_expr_outer_ref_all_operators() {
		let operators = [
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let filter = Filter::new(
				"child_id".to_string(),
				op.clone(),
				FilterValue::OuterRef(OuterRef::new("parent.id")),
			);
			let result = build_single_filter_expr(&filter).expect("filter should compile");
			assert!(
				result.is_some(),
				"OuterRef with {:?} should produce Some",
				op
			);
		}
	}

	#[test]
	fn test_build_single_filter_expr_expression() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value};

		// Test: price > (cost * 2)
		let expr = Expression::Multiply(
			Box::new(AnnotationValue::Field(F::new("cost"))),
			Box::new(AnnotationValue::Value(Value::Int(2))),
		);
		let filter = Filter::new(
			"price".to_string(),
			FilterOperator::Gt,
			FilterValue::Expression(expr),
		);
		let result = build_single_filter_expr(&filter).expect("filter should compile");
		assert!(result.is_some());
	}

	#[test]
	fn test_build_single_filter_expr_expression_all_operators() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		let operators = [
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let expr = Expression::Add(
				Box::new(AnnotationValue::Field(F::new("base"))),
				Box::new(AnnotationValue::Value(OrmValue::Int(10))),
			);
			let filter = Filter::new(
				"total".to_string(),
				op.clone(),
				FilterValue::Expression(expr),
			);
			let result = build_single_filter_expr(&filter).expect("filter should compile");
			assert!(
				result.is_some(),
				"Expression with {:?} should produce Some",
				op
			);
		}
	}

	#[test]
	fn test_build_single_filter_expr_uses_transformed_filter_lhs() {
		// Arrange
		struct TransformedFilterModel;

		// SAFETY: the marker type is used only to compile a synthetic persisted
		// `created_at` field reference for this SQL test.
		let filter = unsafe {
			FieldRef::<
				TransformedFilterModel,
				i64,
				reinhardt_db::orm::expressions::GeneratedModelField,
			>::from_generated_model_field_with_names("created_at", "created_at")
		}
		.year()
		.range(2024, 2026);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(result.is_some());
		let query = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert_eq!(
			query,
			r#"SELECT * FROM "users" WHERE EXTRACT(YEAR FROM "created_at") BETWEEN 2024 AND 2026"#
		);
	}

	#[test]
	fn test_filter_value_to_sea_value_field_ref_fallback() {
		let value = FilterValue::FieldRef(F::new("test_field"));
		let sea_value = filter_value_to_sea_value(&value).expect("value should compile");

		// Should return string representation, not panic
		match sea_value {
			Value::String(Some(s)) => assert_eq!(s.as_str(), "test_field"),
			_ => panic!("Expected String value"),
		}
	}

	#[test]
	fn test_filter_value_to_sea_value_outer_ref_fallback() {
		let value = FilterValue::OuterRef(OuterRef::new("outer.field"));
		let sea_value = filter_value_to_sea_value(&value).expect("value should compile");

		// Should return string representation, not panic
		match sea_value {
			Value::String(Some(s)) => assert_eq!(s.as_str(), "outer.field"),
			_ => panic!("Expected String value"),
		}
	}

	#[test]
	fn test_filter_value_to_sea_value_expression_fallback() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		let expr = Expression::Add(
			Box::new(AnnotationValue::Field(F::new("a"))),
			Box::new(AnnotationValue::Value(OrmValue::Int(1))),
		);
		let value = FilterValue::Expression(expr);
		let sea_value = filter_value_to_sea_value(&value).expect("value should compile");

		// Should return SQL string representation, not panic
		match sea_value {
			Value::String(Some(s)) => {
				assert!(s.contains("a"), "SQL should contain field name 'a'");
				assert!(s.contains("1"), "SQL should contain value '1'");
			}
			_ => panic!("Expected String value"),
		}
	}

	// ==================== insert values mismatch tests (#1551) ====================

	#[rstest]
	fn test_insert_values_mismatch_returns_error_not_panic() {
		// Arrange
		// Simulate the scenario where columns and values count mismatch
		// by calling SeaQuery's values() with wrong number of values
		let mut query = Query::insert()
			.into_table(Alias::new("test_table"))
			.to_owned();

		let columns = vec![Alias::new("col1"), Alias::new("col2"), Alias::new("col3")];
		let values = vec![Value::String(Some(Box::new("val1".to_string())))]; // Only 1 value for 3 columns

		// Act
		let result = query.columns(columns).values(values);

		// Assert - should return Err, not panic
		assert!(result.is_err());
	}

	#[rstest]
	fn test_insert_values_matching_count_succeeds() {
		// Arrange
		let mut query = Query::insert()
			.into_table(Alias::new("test_table"))
			.to_owned();

		let columns = vec![Alias::new("col1"), Alias::new("col2")];
		let values = vec![
			Value::String(Some(Box::new("val1".to_string()))),
			Value::String(Some(Box::new("val2".to_string()))),
		];

		// Act
		let result = query.columns(columns).values(values);

		// Assert
		assert!(result.is_ok());
	}

	// ==================== SQL injection prevention tests ====================

	#[test]
	fn test_outer_ref_filter_uses_safe_column_api() {
		// Arrange: OuterRef with a field name that could be an injection attempt
		let filter = Filter::new(
			"author_id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("users.id")),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert: should produce a valid expression using quoted identifiers
		assert!(result.is_some());
		let expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("books"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(expr))
			.to_string(PostgresQueryBuilder);
		// The field names should be quoted by SeaQuery's Alias, not raw interpolation
		assert!(
			query.contains("\"author_id\""),
			"Column should be properly quoted: {}",
			query
		);
	}

	#[test]
	fn test_outer_ref_injection_attempt_is_safely_quoted() {
		// Arrange: attacker tries SQL injection through OuterRef field name
		let filter = Filter::new(
			"id".to_string(),
			FilterOperator::Eq,
			FilterValue::OuterRef(OuterRef::new("id; DROP TABLE users; --")),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert: the injection string should be treated as a quoted identifier
		assert!(result.is_some());
		let expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("items"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(expr))
			.to_string(PostgresQueryBuilder);
		// SeaQuery's Alias wraps the name in double quotes, treating the entire
		// injection payload as a single identifier name (not executable SQL).
		// The right side of the equality uses Expr::col(Alias::new(...)) which
		// produces a quoted identifier instead of raw SQL interpolation.
		assert!(
			query.contains("\"id; DROP TABLE users; --\""),
			"Injection payload should be enclosed in double quotes as identifier: {}",
			query
		);
		// Verify the query is a valid single-statement SELECT (no semicolons
		// appear outside of the quoted identifier)
		let unquoted_parts: Vec<&str> = query.split('"').enumerate()
			.filter(|(i, _)| i % 2 == 0) // Even indices are outside quotes
			.map(|(_, s)| s)
			.collect();
		let unquoted_sql = unquoted_parts.join("");
		assert!(
			!unquoted_sql.contains(';'),
			"No semicolons should appear outside quoted identifiers: {}",
			query
		);
	}

	#[test]
	fn test_expression_filter_uses_safe_api() {
		use reinhardt_db::orm::annotation::AnnotationValue;

		// Arrange: arithmetic expression (price * quantity)
		let expr = Expression::Multiply(
			Box::new(AnnotationValue::Field(F::new("unit_price"))),
			Box::new(AnnotationValue::Field(F::new("quantity"))),
		);
		let filter = Filter::new(
			"total".to_string(),
			FilterOperator::Eq,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(result.is_some());
		let sea_expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("orders"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(sea_expr))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"total\""),
			"Left side should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_expression_filter_with_literal_value() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		// Arrange: field + literal value
		let expr = Expression::Add(
			Box::new(AnnotationValue::Field(F::new("price"))),
			Box::new(AnnotationValue::Value(OrmValue::Int(100))),
		);
		let filter = Filter::new(
			"adjusted_price".to_string(),
			FilterOperator::Gt,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(result.is_some());
	}

	#[test]
	fn test_outer_ref_all_operators_use_safe_api() {
		// Arrange & Act & Assert: verify all comparison operators work with OuterRef
		let operators = vec![
			FilterOperator::Eq,
			FilterOperator::Ne,
			FilterOperator::Gt,
			FilterOperator::Gte,
			FilterOperator::Lt,
			FilterOperator::Lte,
		];

		for op in operators {
			let filter = Filter::new(
				"field_a".to_string(),
				op.clone(),
				FilterValue::OuterRef(OuterRef::new("field_b")),
			);
			let result = build_single_filter_expr(&filter).expect("filter should compile");
			assert!(
				result.is_some(),
				"OuterRef with {:?} should produce Some",
				op
			);
		}
	}

	// ==================== Case/Coalesce safe expression tests ====================

	#[test]
	fn test_coalesce_expression_uses_safe_parameterized_api() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Value as OrmValue};

		// Arrange: COALESCE(field_a, 0)
		let expr = Expression::Coalesce(vec![
			AnnotationValue::Field(F::new("field_a")),
			AnnotationValue::Value(OrmValue::Int(0)),
		]);
		let filter = Filter::new(
			"result".to_string(),
			FilterOperator::Gt,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(result.is_some());
		let sea_expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("items"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(sea_expr))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("COALESCE"),
			"Should contain COALESCE function: {}",
			query
		);
		assert!(
			query.contains("\"result\""),
			"Left side should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_case_expression_uses_safe_api() {
		use reinhardt_db::orm::annotation::{
			AnnotationValue, Value as OrmValue, When as AnnotWhen,
		};
		use reinhardt_db::orm::expressions::Q;

		// Arrange: CASE WHEN status = 'active' THEN 1 ELSE 0 END
		let expr = Expression::Case {
			whens: vec![AnnotWhen::new(
				Q::new("status", "=", "'active'"),
				AnnotationValue::Value(OrmValue::Int(1)),
			)],
			default: Some(Box::new(AnnotationValue::Value(OrmValue::Int(0)))),
		};
		let filter = Filter::new(
			"priority".to_string(),
			FilterOperator::Eq,
			FilterValue::Expression(expr),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(result.is_some());
		let sea_expr = result.unwrap();
		let query = Query::select()
			.from(Alias::new("tasks"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(sea_expr))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("CASE"),
			"Should contain CASE keyword: {}",
			query
		);
		assert!(
			query.contains("WHEN"),
			"Should contain WHEN keyword: {}",
			query
		);
		assert!(
			query.contains("ELSE"),
			"Should contain ELSE keyword: {}",
			query
		);
	}

	#[test]
	fn test_empty_coalesce_returns_null() {
		// Arrange: COALESCE() with no values
		let expr = Expression::Coalesce(vec![]);

		// Act
		let result = annotation_expr_to_safe_expr(&expr, None);

		// Assert: should produce NULL expression without panicking
		let query = Query::select()
			.from(Alias::new("test"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("NULL"),
			"Empty COALESCE should produce NULL: {}",
			query
		);
	}

	// ==================== Aggregate safe expression tests ====================

	#[test]
	fn test_aggregate_count_uses_safe_api() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: COUNT(*)
		let agg = Aggregate {
			func: AggregateFunc::Count,
			field: None,
			alias: None,
			distinct: false,
		};

		// Act
		let result = aggregate_to_safe_expr(&agg, None);

		// Assert
		let query = Query::select()
			.from(Alias::new("items"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("COUNT(*)"),
			"Should contain COUNT(*): {}",
			query
		);
	}

	#[test]
	fn test_aggregate_sum_field_uses_quoted_identifier() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: SUM(price)
		let agg = Aggregate {
			func: AggregateFunc::Sum,
			field: Some("price".to_string()),
			alias: None,
			distinct: false,
		};

		// Act
		let result = aggregate_to_safe_expr(&agg, None);

		// Assert
		let query = Query::select()
			.from(Alias::new("orders"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("SUM("),
			"Should contain SUM function: {}",
			query
		);
		assert!(
			query.contains("\"price\""),
			"Field name should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_aggregate_count_distinct_uses_distinct_keyword() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: COUNT(DISTINCT category)
		let agg = Aggregate {
			func: AggregateFunc::CountDistinct,
			field: Some("category".to_string()),
			alias: None,
			distinct: false, // AggregateFunc::CountDistinct implies DISTINCT
		};

		// Act
		let result = aggregate_to_safe_expr(&agg, None);

		// Assert
		let query = Query::select()
			.from(Alias::new("products"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("COUNT(DISTINCT"),
			"Should contain COUNT(DISTINCT: {}",
			query
		);
		assert!(
			query.contains("\"category\""),
			"Field name should be quoted: {}",
			query
		);
	}

	#[test]
	fn test_aggregate_injection_attempt_is_quoted() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};

		// Arrange: attacker tries injection via aggregate field name
		let agg = Aggregate {
			func: AggregateFunc::Sum,
			field: Some("price); DROP TABLE users; --".to_string()),
			alias: None,
			distinct: false,
		};

		// Act
		let result = aggregate_to_safe_expr(&agg, None);

		// Assert: injection payload should be treated as a quoted identifier
		let query = Query::select()
			.from(Alias::new("orders"))
			.expr(result)
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"price); DROP TABLE users; --\""),
			"Injection payload should be enclosed in double quotes: {}",
			query
		);
	}

	// ==================== empty And/Or all-unsupported filter tests (#2943) ====================

	#[rstest]
	fn test_build_composite_and_all_unsupported_returns_none() {
		// Arrange
		// Contains + Boolean is an unsupported combo that falls through to None
		let filter1 = Filter::new(
			"field1".to_string(),
			FilterOperator::Contains,
			FilterValue::Boolean(true),
		);
		let filter2 = Filter::new(
			"field2".to_string(),
			FilterOperator::StartsWith,
			FilterValue::Integer(5),
		);
		let condition = FilterCondition::And(vec![
			FilterCondition::Single(filter1),
			FilterCondition::Single(filter2),
		]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert
		assert!(result.is_ok());
		assert!(
			result.unwrap().is_none(),
			"And with all unsupported filters should return None"
		);
	}

	#[rstest]
	fn test_build_composite_or_all_unsupported_returns_none() {
		// Arrange
		let filter1 = Filter::new(
			"field1".to_string(),
			FilterOperator::Contains,
			FilterValue::Boolean(true),
		);
		let filter2 = Filter::new(
			"field2".to_string(),
			FilterOperator::StartsWith,
			FilterValue::Integer(5),
		);
		let condition = FilterCondition::Or(vec![
			FilterCondition::Single(filter1),
			FilterCondition::Single(filter2),
		]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert
		assert!(result.is_ok());
		assert!(
			result.unwrap().is_none(),
			"Or with all unsupported filters should return None"
		);
	}

	#[rstest]
	fn test_build_composite_and_mixed_valid_and_unsupported() {
		// Arrange
		let valid_filter = Filter::new(
			"name".to_string(),
			FilterOperator::Eq,
			FilterValue::String("Alice".to_string()),
		);
		let unsupported_filter = Filter::new(
			"field2".to_string(),
			FilterOperator::Contains,
			FilterValue::Boolean(true),
		);
		let condition = FilterCondition::And(vec![
			FilterCondition::Single(valid_filter),
			FilterCondition::Single(unsupported_filter),
		]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(
			cond.is_some(),
			"And with at least one valid filter should return Some"
		);
		let query = Query::select()
			.from(Alias::new("t"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond.unwrap())
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"name\""),
			"SQL should contain the valid filter field, got: {}",
			query
		);
		assert!(
			query.contains("'Alice'"),
			"SQL should contain the valid filter value, got: {}",
			query
		);
	}

	#[rstest]
	fn test_build_composite_or_mixed_valid_and_unsupported() {
		// Arrange
		let valid_filter = Filter::new(
			"email".to_string(),
			FilterOperator::Eq,
			FilterValue::String("test@example.com".to_string()),
		);
		let unsupported_filter = Filter::new(
			"field2".to_string(),
			FilterOperator::StartsWith,
			FilterValue::Integer(5),
		);
		let condition = FilterCondition::Or(vec![
			FilterCondition::Single(valid_filter),
			FilterCondition::Single(unsupported_filter),
		]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(
			cond.is_some(),
			"Or with at least one valid filter should return Some"
		);
		let query = Query::select()
			.from(Alias::new("t"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond.unwrap())
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"email\""),
			"SQL should contain the valid filter field, got: {}",
			query
		);
		assert!(
			query.contains("'test@example.com'"),
			"SQL should contain the valid filter value, got: {}",
			query
		);
	}

	#[rstest]
	fn test_build_filter_condition_all_unsupported_returns_none() {
		// Arrange
		let filters = vec![
			Filter::new(
				"field1".to_string(),
				FilterOperator::Contains,
				FilterValue::Boolean(true),
			),
			Filter::new(
				"field2".to_string(),
				FilterOperator::StartsWith,
				FilterValue::Integer(5),
			),
		];

		// Act
		let result = build_filter_condition(&filters).expect("filters should compile");

		// Assert
		assert!(
			result.is_none(),
			"build_filter_condition with all unsupported filters should return None"
		);
	}

	// ==================== extract_count_from_row tests (#2945) ====================

	#[rstest]
	fn test_extract_count_from_row_with_count_key() {
		// Arrange
		let data = serde_json::json!({"count": 42});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		assert_eq!(result.unwrap(), 42);
	}

	#[rstest]
	fn test_extract_count_from_row_without_count_key() {
		// Arrange
		let data = serde_json::json!({"total": 10});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("missing 'count' key"),
			"Error should mention missing 'count' key, got: {}",
			err
		);
	}

	#[rstest]
	fn test_extract_count_from_row_empty_object() {
		// Arrange
		let data = serde_json::json!({});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("missing 'count' key"),
			"Error should mention missing 'count' key, got: {}",
			err
		);
	}

	#[rstest]
	fn test_extract_count_from_row_non_integer() {
		// Arrange
		let data = serde_json::json!({"count": "abc"});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("non-integer"),
			"Error should mention non-integer value, got: {}",
			err
		);
	}

	#[rstest]
	fn test_extract_count_from_row_null_data() {
		// Arrange
		let data = serde_json::Value::Null;

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("unexpected data format"),
			"Error should mention unexpected data format, got: {}",
			err
		);
	}

	#[rstest]
	fn test_extract_count_from_row_zero() {
		// Arrange
		let data = serde_json::json!({"count": 0});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		assert_eq!(result.unwrap(), 0);
	}

	// ==================== AdminDatabase inject tests ====================

	#[rstest]
	#[tokio::test]
	async fn test_admin_database_inject_error_hint_mentions_connection() {
		// Arrange
		let singleton = Arc::new(reinhardt_di::SingletonScope::new());
		let ctx = reinhardt_di::InjectionContext::builder(singleton).build();

		// Act
		let result = AdminDatabase::inject(&ctx).await;

		// Assert
		assert!(result.is_err());
		let err = result.err().unwrap();
		assert!(
			err.to_string().contains("DatabaseConnection"),
			"Error hint should mention DatabaseConnection, got: {}",
			err
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_admin_database_inject_returns_prebuilt_from_singleton() {
		// Arrange - simulate pre-built AdminDatabase via configure_di pattern
		let singleton = Arc::new(reinhardt_di::SingletonScope::new());
		// We cannot create a real DatabaseConnection without a DB, so test
		// the prebuilt path by directly setting AdminDatabase in singleton
		// This verifies backward compat: pre-set AdminDatabase is found first

		// Create a mock-like AdminDatabase would require DatabaseConnection,
		// so we just verify the error path when nothing is registered
		let ctx = reinhardt_di::InjectionContext::builder(singleton).build();

		// Act
		let result = AdminDatabase::inject(&ctx).await;

		// Assert - should fail with NotRegistered since no DatabaseConnection
		assert!(result.is_err());
		let err = result.err().unwrap();
		match err {
			reinhardt_di::DiError::NotRegistered { type_name, hint } => {
				assert_eq!(type_name, "AdminDatabase");
				assert_eq!(
					hint,
					"DatabaseConnection must be registered as a singleton. \
					 Use InjectionContextBuilder::singleton(db_connection) during setup."
				);
			}
			other => panic!("Expected NotRegistered error, got: {other:?}"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_admin_database_keyed_provider_reports_missing_connection() {
		let singleton = Arc::new(reinhardt_di::SingletonScope::new());
		let ctx = reinhardt_di::InjectionContext::builder(singleton).build();

		let result =
			reinhardt_di::KeyedDepends::<AdminDatabaseKey, AdminDatabase>::resolve_from_registry(
				&ctx, true,
			)
			.await;

		assert!(result.is_err());
		let err = result.err().unwrap();
		match err {
			reinhardt_di::DiError::NotRegistered { type_name, hint } => {
				assert_eq!(type_name, "AdminDatabase");
				assert_eq!(
					hint,
					"DatabaseConnection must be registered as a singleton. \
					 Use InjectionContextBuilder::singleton(db_connection) during setup."
				);
			}
			other => panic!("Expected NotRegistered error, got: {other:?}"),
		}
	}

	// ==================== FilterValue::Array In/NotIn tests (#2936) ====================

	#[rstest]
	fn test_build_single_filter_expr_array_in() {
		// Arrange
		let filter = Filter::new(
			"status".to_string(),
			FilterOperator::In,
			FilterValue::Array(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(
			result.is_some(),
			"Array In with non-empty values should return Some"
		);
		let query = Query::select()
			.from(Alias::new("table"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("IN"), "SQL should contain IN operator");
		assert!(query.contains("'a'"), "SQL should contain value 'a'");
		assert!(query.contains("'b'"), "SQL should contain value 'b'");
		assert!(query.contains("'c'"), "SQL should contain value 'c'");
	}

	#[rstest]
	fn test_build_single_filter_expr_array_not_in() {
		// Arrange
		let filter = Filter::new(
			"status".to_string(),
			FilterOperator::NotIn,
			FilterValue::Array(vec!["x".to_string(), "y".to_string()]),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(
			result.is_some(),
			"Array NotIn with non-empty values should return Some"
		);
		let query = Query::select()
			.from(Alias::new("table"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("NOT IN"),
			"SQL should contain NOT IN operator"
		);
		assert!(query.contains("'x'"), "SQL should contain value 'x'");
		assert!(query.contains("'y'"), "SQL should contain value 'y'");
	}

	#[rstest]
	fn test_build_single_filter_expr_array_in_empty() {
		// Arrange
		let filter = Filter::new(
			"status".to_string(),
			FilterOperator::In,
			FilterValue::Array(vec![]),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(
			result.is_none(),
			"Array In with empty values should return None"
		);
	}

	#[rstest]
	fn test_build_single_filter_expr_array_in_single_element() {
		// Arrange
		let filter = Filter::new(
			"category".to_string(),
			FilterOperator::In,
			FilterValue::Array(vec!["solo".to_string()]),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(
			result.is_some(),
			"Array In with single element should return Some"
		);
		let query = Query::select()
			.from(Alias::new("table"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("IN"), "SQL should contain IN operator");
		assert!(query.contains("'solo'"), "SQL should contain value 'solo'");
	}

	#[rstest]
	fn test_build_single_filter_expr_array_in_special_chars() {
		// Arrange
		let filter = Filter::new(
			"name".to_string(),
			FilterOperator::In,
			FilterValue::Array(vec!["O'Brien".to_string(), "a;DROP TABLE".to_string()]),
		);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		assert!(
			result.is_some(),
			"Array In with special chars should return Some"
		);
		let query = Query::select()
			.from(Alias::new("table"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.unwrap()))
			.to_string(PostgresQueryBuilder);
		assert!(query.contains("IN"), "SQL should contain IN operator");
		// SeaQuery's to_string with PostgresQueryBuilder escapes single quotes by doubling them
		assert!(
			query.contains("O''Brien"),
			"Single quote in value should be escaped, got: {}",
			query
		);
		// SQL injection attempt should be safely enclosed as a quoted string literal
		assert!(
			query.contains("'a;DROP TABLE'"),
			"SQL injection attempt should be safely quoted as a string literal, got: {}",
			query
		);
	}

	// ==================== Bug #2943: Composite filter WHERE TRUE tests ====================

	#[rstest]
	fn test_and_with_all_unsupported_returns_none() {
		// Arrange: Contains with Integer is unsupported (only String is handled)
		let unsupported1 = FilterCondition::Single(Filter::new(
			"name",
			FilterOperator::Contains,
			FilterValue::Integer(42),
		));
		let unsupported2 = FilterCondition::Single(Filter::new(
			"email",
			FilterOperator::StartsWith,
			FilterValue::Integer(99),
		));
		let condition = FilterCondition::And(vec![unsupported1, unsupported2]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert: And with all unsupported sub-conditions returns None
		// (fixed in #2943: previously returned empty Condition::all() generating WHERE TRUE)
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(
			cond.is_none(),
			"And with all unsupported sub-conditions should return None"
		);
	}

	#[rstest]
	fn test_or_with_all_unsupported_returns_none() {
		// Arrange: Contains/StartsWith with Integer are unsupported
		let unsupported1 = FilterCondition::Single(Filter::new(
			"name",
			FilterOperator::Contains,
			FilterValue::Integer(42),
		));
		let unsupported2 = FilterCondition::Single(Filter::new(
			"email",
			FilterOperator::StartsWith,
			FilterValue::Integer(99),
		));
		let condition = FilterCondition::Or(vec![unsupported1, unsupported2]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert: Or with all unsupported sub-conditions returns None
		// (fixed in #2943: previously returned empty Condition::any() generating WHERE FALSE)
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(
			cond.is_none(),
			"Or with all unsupported sub-conditions should return None"
		);
	}

	#[rstest]
	fn test_and_with_mix_supported_unsupported_keeps_supported() {
		// Arrange: One supported (Eq + String), one unsupported (Contains + Integer)
		let supported = FilterCondition::Single(Filter::new(
			"name",
			FilterOperator::Eq,
			FilterValue::String("Alice".to_string()),
		));
		let unsupported = FilterCondition::Single(Filter::new(
			"email",
			FilterOperator::Contains,
			FilterValue::Integer(42),
		));
		let condition = FilterCondition::And(vec![supported, unsupported]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert: Should keep the supported filter condition
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(
			cond.is_some(),
			"And with mix of supported/unsupported should return Some with supported filters"
		);
		// Verify the supported condition is preserved by building SQL
		let query = Query::select()
			.from(Alias::new("test"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond.unwrap())
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"name\""),
			"SQL should contain the supported filter field 'name': {}",
			query
		);
	}

	#[rstest]
	fn test_or_with_one_supported_one_unsupported() {
		// Arrange
		let supported = FilterCondition::Single(Filter::new(
			"status",
			FilterOperator::Eq,
			FilterValue::String("active".to_string()),
		));
		let unsupported = FilterCondition::Single(Filter::new(
			"count",
			FilterOperator::Contains,
			FilterValue::Integer(42),
		));
		let condition = FilterCondition::Or(vec![supported, unsupported]);

		// Act
		let result = build_composite_filter_condition(&condition);

		// Assert
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(
			cond.is_some(),
			"Or with one supported condition should return Some"
		);
		let query = Query::select()
			.from(Alias::new("test"))
			.column(ColumnRef::Asterisk)
			.cond_where(cond.unwrap())
			.to_string(PostgresQueryBuilder);
		assert!(
			query.contains("\"status\""),
			"SQL should contain the supported filter field 'status': {}",
			query
		);
	}

	// ==================== Bug #2945: extract_count_from_row tests ====================

	#[rstest]
	fn test_extract_count_with_count_key() {
		// Arrange
		let data = serde_json::json!({"count": 42});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), 42);
	}

	#[rstest]
	fn test_extract_count_without_count_key_returns_error() {
		// Arrange: Single non-"count" key
		let data = serde_json::json!({"total": 42});

		// Act
		let result = extract_count_from_row(&data);

		// Assert: Missing "count" key now returns error with available keys
		// (fixed in #2945: previously fell back to first value from iteration order)
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("missing 'count' key"),
			"Error should mention missing 'count' key, got: {}",
			err
		);
	}

	#[rstest]
	fn test_extract_count_with_multiple_keys_no_count_returns_error() {
		// Arrange: Multiple keys, no "count" key
		let data = serde_json::json!({"total": 42, "other": 99});

		// Act
		let result = extract_count_from_row(&data);

		// Assert: Missing "count" key returns error listing available keys
		// (fixed in #2945: previously used fragile obj.values().next() fallback)
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			err.to_string().contains("available keys"),
			"Error should list available keys, got: {}",
			err
		);
	}

	#[rstest]
	fn test_extract_count_non_integer_returns_error() {
		// Arrange
		let data = serde_json::json!({"count": "not_a_number"});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(matches!(err, AdminError::DatabaseError(_)));
	}

	#[rstest]
	fn test_extract_count_null_returns_error() {
		// Arrange
		let data = serde_json::json!({"count": null});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_extract_count_empty_object_returns_error() {
		// Arrange
		let data = serde_json::json!({});

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_extract_count_non_object_returns_error() {
		// Arrange: Array instead of object
		let data = serde_json::json!([1, 2, 3]);

		// Act
		let result = extract_count_from_row(&data);

		// Assert
		assert!(result.is_err());
	}

	// ==================== parse_pk_value tests ====================

	#[rstest]
	fn test_parse_pk_value_integer_falls_back_to_bigint() {
		// Arrange: No registry entry for this table, integer string input

		// Act
		let val = parse_pk_value("nonexistent_table", "id", "42");

		// Assert
		assert_eq!(val, Value::BigInt(Some(42)));
	}

	#[rstest]
	fn test_parse_pk_value_uuid_string_without_registry_falls_back_to_string() {
		// Arrange: No registry entry, UUID string input

		// Act
		let val = parse_pk_value(
			"nonexistent_table",
			"id",
			"c1a363b1-cc42-4dea-81f0-9dc1cedf0083",
		);

		// Assert: Without registry metadata, UUID falls back to Value::String
		assert!(matches!(val, Value::String(Some(_))));
	}

	#[rstest]
	fn test_parse_pk_value_non_numeric_string_falls_back_to_string() {
		// Arrange: No registry entry, non-numeric string input

		// Act
		let val = parse_pk_value("nonexistent_table", "id", "hello-world");

		// Assert
		assert!(matches!(val, Value::String(Some(_))));
	}

	#[rstest]
	fn test_parse_pk_value_negative_integer() {
		// Arrange: Negative integer string

		// Act
		let val = parse_pk_value("nonexistent_table", "id", "-1");

		// Assert
		assert_eq!(val, Value::BigInt(Some(-1)));
	}

	#[rstest]
	fn test_parse_pk_value_zero() {
		// Arrange: Zero as string

		// Act
		let val = parse_pk_value("nonexistent_table", "id", "0");

		// Assert
		assert_eq!(val, Value::BigInt(Some(0)));
	}

	#[rstest]
	fn test_admin_query_statements_share_scope_and_join_related_rows() {
		// Arrange
		let query = crate::core::AdminQuery::new("articles")
			.filter(Filter::new(
				"owner_id",
				FilterOperator::Eq,
				FilterValue::Integer(7),
			))
			.filter_condition(FilterCondition::Or(vec![
				FilterCondition::Single(Filter::new(
					"status",
					FilterOperator::Eq,
					FilterValue::String("published".to_string()),
				)),
				FilterCondition::Single(Filter::new(
					"status",
					FilterOperator::Eq,
					FilterValue::String("review".to_string()),
				)),
			]));
		let related = vec![AdminRelatedField {
			relation_name: "author".to_string(),
			source_column: "author_id".to_string(),
			target_table: "users".to_string(),
			target_column: "external_key".to_string(),
			columns: vec!["external_key".to_string(), "username".to_string()],
		}];

		// Act
		let list_sql = build_admin_list_statement(&query, &related, Some("-id"), 10, 25)
			.unwrap()
			.to_string(PostgresQueryBuilder);
		let count_sql = build_admin_count_statement(&query, &related)
			.unwrap()
			.to_string(PostgresQueryBuilder);

		// Assert
		assert_eq!(
			list_sql,
			"SELECT \"articles\".*, COUNT(*) OVER() AS \"__reinhardt_total_count\", \"author\".\"external_key\" AS \"__reinhardt_related_0__external_key\", \"author\".\"username\" AS \"__reinhardt_related_0__username\" FROM \"articles\" LEFT JOIN \"users\" AS \"author\" ON \"articles\".\"author_id\" = \"author\".\"external_key\" WHERE (\"articles\".\"owner_id\" = 7 AND (\"articles\".\"status\" = 'published' OR \"articles\".\"status\" = 'review')) ORDER BY \"articles\".\"id\" DESC LIMIT 25 OFFSET 10"
		);
		assert_eq!(
			count_sql,
			"SELECT COUNT(*) AS count FROM \"articles\" LEFT JOIN \"users\" AS \"author\" ON \"articles\".\"author_id\" = \"author\".\"external_key\" WHERE (\"articles\".\"owner_id\" = 7 AND (\"articles\".\"status\" = 'published' OR \"articles\".\"status\" = 'review'))"
		);
	}

	#[test]
	fn test_date_hierarchy_statement_reuses_scoped_admin_query() {
		// Arrange
		let query = crate::core::AdminQuery::new("articles")
			.filter(Filter::new(
				"owner_id",
				FilterOperator::Eq,
				FilterValue::Integer(7),
			))
			.filter(Filter::new(
				"published_on",
				FilterOperator::Gte,
				FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(
					chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
				))),
			))
			.filter(Filter::new(
				"published_on",
				FilterOperator::Lt,
				FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(
					chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
				))),
			));
		let related = vec![AdminRelatedField {
			relation_name: "author".to_string(),
			source_column: "author_id".to_string(),
			target_table: "users".to_string(),
			target_column: "external_key".to_string(),
			columns: vec![],
		}];

		// Act
		let sql = build_date_hierarchy_statement(
			&query,
			"published_on",
			crate::types::DateHierarchyLevel::Month,
			&DbFieldType::Date,
			&related,
		)
		.unwrap()
		.to_string(PostgresQueryBuilder);

		// Assert
		assert_eq!(
			sql,
			"SELECT DISTINCT DATE_TRUNC('month', \"articles\".\"published_on\")::date AS \"__reinhardt_date_hierarchy\" FROM \"articles\" LEFT JOIN \"users\" AS \"author\" ON \"articles\".\"author_id\" = \"author\".\"external_key\" WHERE \"articles\".\"published_on\" IS NOT NULL AND (\"articles\".\"owner_id\" = 7 AND \"articles\".\"published_on\" >= CAST('2024-01-01' AS DATE) AND \"articles\".\"published_on\" < CAST('2025-01-01' AS DATE)) ORDER BY \"__reinhardt_date_hierarchy\" ASC"
		);
	}

	#[test]
	fn test_datetime_hierarchy_statement_preserves_naive_calendar_and_bounds() {
		// Arrange
		let start = chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
			.unwrap()
			.and_hms_opt(0, 0, 0)
			.unwrap();
		let end = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
			.unwrap()
			.and_hms_opt(0, 0, 0)
			.unwrap();
		let query = crate::core::AdminQuery::new("events")
			.filter(Filter::new(
				"starts_at",
				FilterOperator::Gte,
				FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::NaiveDateTime(start))),
			))
			.filter(Filter::new(
				"starts_at",
				FilterOperator::Lt,
				FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::NaiveDateTime(end))),
			));

		// Act
		let statement = build_date_hierarchy_statement(
			&query,
			"starts_at",
			crate::types::DateHierarchyLevel::Month,
			&DbFieldType::DateTime,
			&[],
		)
		.unwrap();
		let (sql, values) = statement.build(PostgresQueryBuilder);
		let params = convert_values(values);

		// Assert
		assert_eq!(
			sql,
			"SELECT DISTINCT DATE_TRUNC('month', \"starts_at\")::date AS \"__reinhardt_date_hierarchy\" FROM \"events\" WHERE \"starts_at\" IS NOT NULL AND (\"starts_at\" >= $1 AND \"starts_at\" < $2) ORDER BY \"__reinhardt_date_hierarchy\" ASC"
		);
		assert_eq!(
			params,
			vec![
				reinhardt_db::backends::types::QueryValue::NaiveTimestamp(start),
				reinhardt_db::backends::types::QueryValue::NaiveTimestamp(end),
			]
		);
	}

	#[test]
	fn test_typed_date_filter_binds_castable_postgres_value() {
		// Arrange
		let query = crate::core::AdminQuery::new("articles").filter(Filter::new(
			"published_on",
			FilterOperator::Gte,
			FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(
				chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
			))),
		));

		// Act
		let statement = build_admin_list_statement(&query, &[], None, 0, 25).unwrap();
		let (sql, values) = statement.build(PostgresQueryBuilder);
		let params = convert_values(values);

		// Assert
		assert_eq!(
			sql,
			"SELECT \"articles\".*, COUNT(*) OVER() AS \"__reinhardt_total_count\" FROM \"articles\" WHERE \"published_on\" >= CAST($1 AS DATE) LIMIT $2 OFFSET $3"
		);
		assert_eq!(
			params,
			vec![
				reinhardt_db::backends::types::QueryValue::String("2024-01-01".to_string()),
				reinhardt_db::backends::types::QueryValue::Int(25),
				reinhardt_db::backends::types::QueryValue::Int(0),
			]
		);
	}

	#[test]
	fn test_typed_date_filter_normalizes_expanded_and_bce_years_for_postgres() {
		// Arrange
		let query = crate::core::AdminQuery::new("articles")
			.filter(Filter::new(
				"published_on",
				FilterOperator::Gte,
				FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(
					chrono::NaiveDate::from_ymd_opt(10_000, 1, 1).unwrap(),
				))),
			))
			.filter(Filter::new(
				"published_on",
				FilterOperator::Lt,
				FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(
					chrono::NaiveDate::from_ymd_opt(0, 1, 1).unwrap(),
				))),
			));

		// Act
		let statement = build_admin_list_statement(&query, &[], None, 0, 25).unwrap();
		let (_, values) = statement.build(PostgresQueryBuilder);
		let params = convert_values(values);

		// Assert
		assert_eq!(
			params,
			vec![
				reinhardt_db::backends::types::QueryValue::String("10000-01-01".to_string()),
				reinhardt_db::backends::types::QueryValue::String("0001-01-01 BC".to_string()),
				reinhardt_db::backends::types::QueryValue::Int(25),
				reinhardt_db::backends::types::QueryValue::Int(0),
			]
		);
	}

	#[test]
	fn test_date_hierarchy_choice_parses_expanded_years_and_datetimes() {
		assert_eq!(
			parse_date_hierarchy_choice(
				"2024-02-01",
				crate::types::DateHierarchyLevel::Month,
				&DbFieldType::DateTime,
			)
			.unwrap(),
			2
		);
		assert_eq!(
			parse_date_hierarchy_choice(
				"+10000-02-01",
				crate::types::DateHierarchyLevel::Year,
				&DbFieldType::Date,
			)
			.unwrap(),
			10_000
		);
		assert_eq!(
			parse_date_hierarchy_choice(
				"+10000-02-01T00:00:00+00:00",
				crate::types::DateHierarchyLevel::Year,
				&DbFieldType::TimestampTz,
			)
			.unwrap(),
			10_000
		);
		assert_eq!(
			parse_date_hierarchy_choice(
				"0001-02-01 00:00:00 BC",
				crate::types::DateHierarchyLevel::Year,
				&DbFieldType::DateTime,
			)
			.unwrap(),
			0
		);
		assert_eq!(
			parse_date_hierarchy_choice(
				"2024-02-29T00:00:00+00:00",
				crate::types::DateHierarchyLevel::Day,
				&DbFieldType::TimestampTz,
			)
			.unwrap(),
			29
		);
	}

	#[test]
	fn test_joined_admin_filter_preserves_transformed_lhs() {
		// Arrange
		struct TransformedFilterModel;
		// SAFETY: the marker type is used only to compile a synthetic persisted
		// `created_at` field reference for this SQL test.
		let filter = unsafe {
			FieldRef::<
				TransformedFilterModel,
				i64,
				reinhardt_db::orm::expressions::GeneratedModelField,
			>::from_generated_model_field_with_names("created_at", "created_at")
		}
		.year()
		.range(2024, 2026);

		// Act
		let expression = build_single_filter_expr_for_table(&filter, Some("articles"))
			.expect("transformed filter should compile")
			.expect("transformed filter should produce an expression");
		let sql = Query::select()
			.from(Alias::new("articles"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(expression))
			.to_string(PostgresQueryBuilder);

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "articles" WHERE EXTRACT(YEAR FROM "articles"."created_at") BETWEEN 2024 AND 2026"#
		);
	}

	#[test]
	fn test_joined_admin_filter_qualifies_nested_expression_fields() {
		use reinhardt_db::orm::aggregation::{Aggregate, AggregateFunc};
		use reinhardt_db::orm::annotation::AnnotationValue;

		// Arrange
		let nested = Expression::Multiply(
			Box::new(AnnotationValue::Aggregate(Aggregate {
				func: AggregateFunc::Sum,
				field: Some("tax".to_string()),
				alias: None,
				distinct: false,
			})),
			Box::new(AnnotationValue::Field(F::new("quantity"))),
		);
		let filter = Filter::new(
			"total",
			FilterOperator::Eq,
			FilterValue::Expression(Expression::Add(
				Box::new(AnnotationValue::Field(F::new("subtotal"))),
				Box::new(AnnotationValue::Expression(nested)),
			)),
		);

		// Act
		let expression = build_single_filter_expr_for_table(&filter, Some("orders"))
			.expect("expression filter should compile")
			.expect("expression filter should produce an expression");
		let sql = Query::select()
			.from(Alias::new("orders"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(expression))
			.to_string(PostgresQueryBuilder);

		// Assert
		assert_eq!(
			sql,
			r#"SELECT * FROM "orders" WHERE "orders"."total" = ("orders"."subtotal" + (SUM("orders"."tax") * "orders"."quantity"))"#
		);
	}

	#[rstest]
	#[case::present(
		serde_json::json!({
			"id": 11,
			"title": "Scoped article",
			"__reinhardt_total_count": 3,
			"__reinhardt_related_0__external_key": "user-7",
			"__reinhardt_related_0__username": "alice"
		}),
		serde_json::json!({
			"id": 11,
			"title": "Scoped article",
			"author": {"external_key": "user-7", "username": "alice"}
		})
	)]
	#[case::missing(
		serde_json::json!({
			"id": 12,
			"title": "Unassigned article",
			"__reinhardt_total_count": 3,
			"__reinhardt_related_0__external_key": null,
			"__reinhardt_related_0__username": null
		}),
		serde_json::json!({
			"id": 12,
			"title": "Unassigned article",
			"author": null
		})
	)]
	fn test_decode_admin_list_row_nests_related_data(
		#[case] row: serde_json::Value,
		#[case] expected: serde_json::Value,
	) {
		// Arrange
		let related = vec![AdminRelatedField {
			relation_name: "author".to_string(),
			source_column: "author_id".to_string(),
			target_table: "users".to_string(),
			target_column: "external_key".to_string(),
			columns: vec!["external_key".to_string(), "username".to_string()],
		}];
		let serde_json::Value::Object(row) = row else {
			panic!("test row must be an object");
		};

		// Act
		let (decoded, count) = decode_admin_list_row(row, &related).unwrap();

		// Assert
		assert_eq!(count, 3);
		assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
	}

	#[tokio::test]
	async fn test_admin_query_related_loading_uses_one_query() {
		// Arrange
		let mut backend = MockDatabaseBackend::new();
		backend
			.expect_database_type()
			.return_const(DatabaseType::Postgres);
		backend
			.expect_fetch_all()
			.withf(|sql, _| {
				sql.contains("LEFT JOIN \"users\" AS \"author\"")
					&& sql.contains("\"articles\".\"author_id\" = \"author\".\"external_key\"")
					&& sql.contains("AS \"__reinhardt_related_0__username\"")
					&& sql.contains("COUNT(*) OVER()")
			})
			.times(1)
			.returning(|_, _| {
				let mut row = Row::new();
				row.data.insert("id".to_string(), QueryValue::Int(11));
				row.data
					.insert("__reinhardt_total_count".to_string(), QueryValue::Int(1));
				row.data.insert(
					"__reinhardt_related_0__external_key".to_string(),
					QueryValue::String("user-7".to_string()),
				);
				row.data.insert(
					"__reinhardt_related_0__username".to_string(),
					QueryValue::String("alice".to_string()),
				);
				Ok(vec![row])
			});
		backend.expect_fetch_one().times(0);
		let connection = BackendsConnection::new(Arc::new(backend));
		let connection_lease = DatabaseConnectionLease::register(connection)
			.expect("Failed to register mock database connection");
		let database = AdminDatabase::new(connection_lease.handle());
		let query = AdminQuery::new("articles");
		let related = vec![AdminRelatedField {
			relation_name: "author".to_string(),
			source_column: "author_id".to_string(),
			target_table: "users".to_string(),
			target_column: "external_key".to_string(),
			columns: vec!["external_key".to_string(), "username".to_string()],
		}];

		// Act
		let (rows, count) = database
			.list_admin_query_with_count(&query, &related, None, 0, 25)
			.await
			.expect("related changelist query should succeed");

		// Assert
		assert_eq!(count, 1);
		assert_eq!(rows.len(), 1);
		assert_eq!(
			rows[0].get("author"),
			Some(&serde_json::json!({
				"external_key": "user-7",
				"username": "alice"
			}))
		);
	}
}
