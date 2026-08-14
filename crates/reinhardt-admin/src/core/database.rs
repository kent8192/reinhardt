//! Database integration for admin operations
//!
//! This module provides database access layer for admin CRUD operations,
//! integrating with reinhardt-orm's QuerySet API.

use crate::core::admin_query::AdminQuery;
use crate::types::{AdminError, AdminResult};
use async_trait::async_trait;
use reinhardt_core::macros::injectable;
use reinhardt_db::migrations::{FieldType as DbFieldType, global_registry};
use reinhardt_db::orm::execution::convert_values;
use reinhardt_db::orm::{
	AtomicTransaction, DatabaseBackend, DatabaseConnection, Filter, FilterCondition,
	FilterOperator, FilterValue, Model, OrmExecutor, QueryRow, QueryValue,
	database_value_to_query_value,
};
use reinhardt_di::{DiResult, Injectable, InjectionContext, KeyedFactoryOutput};
use reinhardt_query::prelude::{
	Alias, ArrayType, BinOper, CaseStatement, ColumnRef, Condition, Expr, ExprTrait, Func,
	IntoValue, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query, QueryBuilder,
	QueryStatementBuilder, SelectStatement, SimpleExpr, SqliteQueryBuilder, TableRef,
	TemporalTimeZone, TemporalTruncKind, TemporalTruncOutput, UpdateStatement, Value, Values,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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
	pub(crate) presence_column: String,
	pub(crate) columns: Vec<String>,
}

/// One validated row update in an atomic admin batch.
pub(crate) struct AdminBatchMutation {
	object_id: String,
	changed_fields: Vec<String>,
	data: HashMap<String, serde_json::Value>,
	json_null_fields: HashSet<String>,
}

impl AdminBatchMutation {
	pub(crate) fn new(object_id: String, data: HashMap<String, serde_json::Value>) -> Self {
		let mut changed_fields = data.keys().cloned().collect::<Vec<_>>();
		changed_fields.sort();
		Self {
			object_id,
			changed_fields,
			data,
			json_null_fields: HashSet::new(),
		}
	}

	pub(crate) fn new_with_json_nulls(
		object_id: String,
		data: HashMap<String, serde_json::Value>,
		json_null_fields: HashSet<String>,
	) -> Self {
		let mut mutation = Self::new(object_id, data);
		mutation.json_null_fields = json_null_fields;
		mutation
	}

	pub(crate) fn object_id(&self) -> &str {
		&self.object_id
	}

	pub(crate) fn changed_fields(&self) -> &[String] {
		&self.changed_fields
	}

	pub(crate) fn data(&self) -> &HashMap<String, serde_json::Value> {
		&self.data
	}
}

/// Failure from an atomic admin batch update.
#[derive(Debug, Error)]
pub(crate) enum AdminBatchAtomicError {
	#[error("row {row_index} with object ID '{object_id}' was not found")]
	ZeroAffected { row_index: usize, object_id: String },
	#[error(transparent)]
	Admin(#[from] AdminError),
	#[error(transparent)]
	Core(#[from] reinhardt_core::exception::Error),
}

fn parse_inline_naive_datetime(value: &str) -> Result<chrono::NaiveDateTime, chrono::ParseError> {
	chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
		.or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))
		.or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
}

fn json_to_text_value(value: serde_json::Value) -> Value {
	match value {
		serde_json::Value::Null => Value::String(None),
		serde_json::Value::String(value) => Value::String(Some(Box::new(value))),
		value => Value::String(Some(Box::new(value.to_string()))),
	}
}

fn json_to_set_value(field_name: &str, value: serde_json::Value) -> AdminResult<Value> {
	let values = match value {
		serde_json::Value::Null => return Ok(Value::String(None)),
		serde_json::Value::String(value) => value
			.split(',')
			.map(str::trim)
			.filter(|value| !value.is_empty())
			.map(str::to_string)
			.collect(),
		serde_json::Value::Array(values) => values
			.into_iter()
			.map(|value| {
				value.as_str().map(str::to_string).ok_or_else(|| {
					AdminError::ValidationError(format!(
						"Field '{field_name}' requires an array of SET values"
					))
				})
			})
			.collect::<AdminResult<Vec<_>>>()?,
		_ => {
			return Err(AdminError::ValidationError(format!(
				"Field '{field_name}' requires a SET string or array"
			)));
		}
	};

	Ok(Value::String(Some(Box::new(values.join(",")))))
}

fn array_type_for_field(field_type: &DbFieldType) -> Option<ArrayType> {
	Some(match field_type {
		DbFieldType::Char(_)
		| DbFieldType::VarChar(_)
		| DbFieldType::Text
		| DbFieldType::TinyText
		| DbFieldType::MediumText
		| DbFieldType::LongText
		| DbFieldType::CIText => ArrayType::String,
		DbFieldType::Integer
		| DbFieldType::SmallInteger
		| DbFieldType::TinyInt
		| DbFieldType::MediumInt => ArrayType::Int,
		DbFieldType::Custom(name) if matches!(name.as_str(), "u8" | "u16" | "u32") => {
			ArrayType::Int
		}
		DbFieldType::BigInteger => ArrayType::BigInt,
		DbFieldType::Boolean => ArrayType::Bool,
		DbFieldType::Float => ArrayType::Float,
		DbFieldType::Double | DbFieldType::Real => ArrayType::Double,
		DbFieldType::Uuid => ArrayType::Uuid,
		_ => return None,
	})
}

fn json_to_array_element(
	field_name: &str,
	field_type: &DbFieldType,
	value: serde_json::Value,
) -> AdminResult<Value> {
	if value.is_null() {
		return Err(AdminError::ValidationError(format!(
			"Field '{field_name}' does not support NULL array elements"
		)));
	}

	let invalid = || {
		AdminError::ValidationError(format!(
			"Field '{field_name}' contains an invalid array element"
		))
	};
	Ok(match field_type {
		DbFieldType::Char(_)
		| DbFieldType::VarChar(_)
		| DbFieldType::Text
		| DbFieldType::TinyText
		| DbFieldType::MediumText
		| DbFieldType::LongText
		| DbFieldType::CIText => Value::String(Some(Box::new(
			value.as_str().ok_or_else(invalid)?.to_string(),
		))),
		DbFieldType::Integer
		| DbFieldType::SmallInteger
		| DbFieldType::TinyInt
		| DbFieldType::MediumInt => {
			let value = value
				.as_i64()
				.and_then(|value| i32::try_from(value).ok())
				.ok_or_else(invalid)?;
			Value::Int(Some(value))
		}
		DbFieldType::Custom(name) if matches!(name.as_str(), "u8" | "u16" | "u32") => {
			let value = value
				.as_i64()
				.and_then(|value| i32::try_from(value).ok())
				.ok_or_else(invalid)?;
			Value::Int(Some(value))
		}
		DbFieldType::BigInteger => Value::BigInt(Some(value.as_i64().ok_or_else(invalid)?)),
		DbFieldType::Boolean => Value::Bool(Some(value.as_bool().ok_or_else(invalid)?)),
		DbFieldType::Float => {
			let value = value.as_f64().ok_or_else(invalid)? as f32;
			if !value.is_finite() {
				return Err(invalid());
			}
			Value::Float(Some(value))
		}
		DbFieldType::Double | DbFieldType::Real => {
			let value = value.as_f64().ok_or_else(invalid)?;
			if !value.is_finite() {
				return Err(invalid());
			}
			Value::Double(Some(value))
		}
		DbFieldType::Uuid => Value::Uuid(Some(Box::new(
			uuid::Uuid::parse_str(value.as_str().ok_or_else(invalid)?).map_err(|_| invalid())?,
		))),
		_ => {
			return Err(AdminError::ValidationError(format!(
				"Field '{field_name}' has an unsupported PostgreSQL array element type"
			)));
		}
	})
}

fn json_to_array_value(
	field_name: &str,
	inner_type: &DbFieldType,
	value: serde_json::Value,
) -> AdminResult<Value> {
	let array_type = array_type_for_field(inner_type).ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Field '{field_name}' has an unsupported PostgreSQL array element type"
		))
	})?;
	if value.is_null() {
		return Ok(Value::Array(array_type, None));
	}
	let values = value.as_array().ok_or_else(|| {
		AdminError::ValidationError(format!("Field '{field_name}' requires a JSON array value"))
	})?;
	let values = values
		.iter()
		.cloned()
		.map(|value| json_to_array_element(field_name, inner_type, value))
		.collect::<AdminResult<Vec<_>>>()?;
	Ok(Value::Array(array_type, Some(Box::new(values))))
}

fn json_to_structured_value(
	field_name: &str,
	field_type: &DbFieldType,
	value: serde_json::Value,
) -> Option<AdminResult<Value>> {
	Some(match field_type {
		DbFieldType::Json | DbFieldType::JsonBinary => {
			if value.is_null() {
				Ok(Value::Json(None))
			} else {
				Ok(Value::Json(Some(Box::new(value))))
			}
		}
		DbFieldType::Array(inner_type) => json_to_array_value(field_name, inner_type, value),
		DbFieldType::Set { .. } => json_to_set_value(field_name, value),
		DbFieldType::Char(_)
		| DbFieldType::VarChar(_)
		| DbFieldType::Text
		| DbFieldType::TinyText
		| DbFieldType::MediumText
		| DbFieldType::LongText
		| DbFieldType::CIText
		| DbFieldType::Enum { .. } => Ok(json_to_text_value(value)),
		_ => return None,
	})
}

#[cfg(server)]
fn mysql_primary_key_value(table_name: &str, pk_field: &str) -> Option<serde_json::Value> {
	let field = crate::server::type_inference::get_field_metadata(table_name, pk_field)?;
	if let Some(default) = field.params.get("default")
		&& let Some(value) = parse_sql_literal(default)
	{
		return Some(value);
	}

	let uuid_default = field
		.params
		.get("default")
		.is_some_and(|default| is_uuid_default(default));
	let can_store_uuid = match field.field_type {
		DbFieldType::Char(length) | DbFieldType::VarChar(length) => length >= 36,
		DbFieldType::Text
		| DbFieldType::TinyText
		| DbFieldType::MediumText
		| DbFieldType::LongText
		| DbFieldType::Uuid => true,
		_ => false,
	};
	if matches!(field.field_type, DbFieldType::Uuid) || uuid_default && can_store_uuid {
		return Some(serde_json::json!(uuid::Uuid::new_v4().to_string()));
	}

	None
}

#[cfg(server)]
fn is_uuid_default(value: &str) -> bool {
	matches!(
		value.trim().to_ascii_lowercase().as_str(),
		"uuid()" | "uuid_v4()" | "gen_random_uuid()"
	)
}

#[cfg(server)]
fn parse_sql_literal(value: &str) -> Option<serde_json::Value> {
	let value = value.trim();
	if value.len() >= 2 && value.starts_with("'") && value.ends_with("'") {
		return Some(serde_json::json!(
			value[1..value.len() - 1].replace("''", "'")
		));
	}
	if value.eq_ignore_ascii_case("true") {
		return Some(serde_json::Value::Bool(true));
	}
	if value.eq_ignore_ascii_case("false") {
		return Some(serde_json::Value::Bool(false));
	}
	value.parse::<i64>().ok().map(serde_json::Value::from)
}

#[cfg(server)]
fn field_type_for_value(table_name: &str, field_name: &str) -> Option<DbFieldType> {
	let field = crate::server::type_inference::get_field_metadata(table_name, field_name)?;
	let Some(target) = field.params.get("fk_target") else {
		return Some(field.field_type);
	};
	let (qualified_app, target_model_name) = target
		.split_once('.')
		.map_or((None, target.as_str()), |(app, model)| (Some(app), model));
	let target_app = field
		.params
		.get("fk_target_app")
		.map(String::as_str)
		.or(qualified_app);
	let registry = global_registry();
	let Some(target_model) = target_app
		.and_then(|app| registry.find_model_qualified(app, target_model_name))
		.or_else(|| registry.find_model_by_name(target_model_name))
	else {
		return Some(field.field_type);
	};
	let target_field = field
		.params
		.get("fk_target_field")
		.cloned()
		.or_else(|| {
			target_model
				.fields
				.iter()
				.find(|(_, metadata)| {
					metadata
						.params
						.get("primary_key")
						.is_some_and(|value| value == "true")
				})
				.map(|(name, _)| name.clone())
		})
		.unwrap_or_else(|| "id".to_string());
	target_model
		.fields
		.get(&target_field)
		.or_else(|| {
			target_model
				.fields
				.values()
				.find(|metadata| metadata.params.get("db_column") == Some(&target_field))
		})
		.or_else(|| {
			target_model.fields.values().find(|metadata| {
				metadata
					.params
					.get("rust_field_name")
					.is_some_and(|name| name == &target_field)
					|| metadata
						.params
						.get("logical_name")
						.is_some_and(|name| name == &target_field)
			})
		})
		.map(|metadata| metadata.field_type.clone())
}

fn string_value_for_field(s: String, field_type: Option<&DbFieldType>) -> Value {
	let fallback = || Value::String(Some(Box::new(s.clone())));
	match field_type {
		Some(DbFieldType::Date) => chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d")
			.map(|date| Value::ChronoDate(Some(Box::new(date))))
			.unwrap_or_else(|_| fallback()),
		Some(DbFieldType::Time) => {
			if s.len() == 8 && s.chars().filter(|character| *character == ':').count() == 2 {
				chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S")
					.map(|time| Value::ChronoTime(Some(Box::new(time))))
					.unwrap_or_else(|_| fallback())
			} else {
				fallback()
			}
		}
		Some(DbFieldType::DateTime | DbFieldType::TimestampTz) => {
			if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
				Value::ChronoDateTimeUtc(Some(Box::new(dt.with_timezone(&chrono::Utc))))
			} else if let Ok(dt) =
				chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.fZ")
			{
				Value::ChronoDateTimeUtc(Some(Box::new(dt.and_utc())))
			} else {
				fallback()
			}
		}
		Some(DbFieldType::Uuid) => uuid::Uuid::parse_str(&s)
			.map(|uuid| Value::Uuid(Some(Box::new(uuid))))
			.unwrap_or_else(|_| fallback()),
		Some(_) => fallback(),
		None => {
			if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
				Value::ChronoDateTimeUtc(Some(Box::new(dt.with_timezone(&chrono::Utc))))
			} else if let Ok(dt) =
				chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.fZ")
			{
				Value::ChronoDateTimeUtc(Some(Box::new(dt.and_utc())))
			} else if s.len() == 10 {
				if let Ok(date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
					Value::ChronoDate(Some(Box::new(date)))
				} else {
					fallback()
				}
			} else if s.len() == 8 && s.chars().filter(|character| *character == ':').count() == 2 {
				if let Ok(time) = chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S") {
					Value::ChronoTime(Some(Box::new(time)))
				} else {
					fallback()
				}
			} else if s.len() == 36
				&& s.chars().enumerate().all(|(index, character)| {
					matches!(index, 8 | 13 | 18 | 23) && character == '-'
						|| character.is_ascii_hexdigit()
				}) {
				if let Ok(uuid) = uuid::Uuid::parse_str(&s) {
					Value::Uuid(Some(Box::new(uuid)))
				} else {
					fallback()
				}
			} else {
				fallback()
			}
		}
	}
}

/// Converts a `serde_json::Value` into a reinhardt-query `Value`.
///
/// String values are converted to metadata-aware update parameters while
/// preserving exact decimal and temporal spellings.
fn json_to_update_value(
	table_name: &str,
	field_name: &str,
	value: serde_json::Value,
) -> AdminResult<Value> {
	if let Some(field_meta) =
		crate::server::type_inference::get_field_metadata(table_name, field_name)
	{
		if let Some(value) =
			json_to_structured_value(field_name, &field_meta.field_type, value.clone())
		{
			return value;
		}
		let empty = value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty());
		match field_meta.field_type {
			DbFieldType::Decimal { .. } => {
				if field_meta.nullable && empty {
					return Ok(Value::BigDecimal(None));
				}
				let decimal = match value {
					serde_json::Value::String(value) => value,
					value => value.to_string(),
				};
				let decimal = decimal.parse().map_err(|error| {
					AdminError::ValidationError(format!(
						"Field '{field_name}' requires a decimal value: {error}"
					))
				})?;
				return Ok(Value::BigDecimal(Some(Box::new(decimal))));
			}
			DbFieldType::Time => {
				if field_meta.nullable && empty {
					return Ok(Value::ChronoTime(None));
				}
				let value = value.as_str().ok_or_else(|| {
					AdminError::ValidationError(format!(
						"Field '{field_name}' requires an ISO time"
					))
				})?;
				let value = chrono::NaiveTime::parse_from_str(value, "%H:%M:%S%.f")
					.or_else(|_| chrono::NaiveTime::parse_from_str(value, "%H:%M"))
					.map_err(|_| {
						AdminError::ValidationError(format!(
							"Field '{field_name}' requires an ISO time"
						))
					})?;
				return Ok(Value::ChronoTime(Some(Box::new(value))));
			}
			DbFieldType::DateTime => {
				if field_meta.nullable && empty {
					return Ok(Value::ChronoDateTime(None));
				}
				let value = value.as_str().ok_or_else(|| {
					AdminError::ValidationError(format!(
						"Field '{field_name}' requires an ISO date-time"
					))
				})?;
				let value = parse_inline_naive_datetime(value)
					.or_else(|_| {
						chrono::DateTime::parse_from_rfc3339(value).map(|value| value.naive_utc())
					})
					.map_err(|_| {
						AdminError::ValidationError(format!(
							"Field '{field_name}' requires an ISO date-time"
						))
					})?;
				return Ok(Value::ChronoDateTime(Some(Box::new(value))));
			}
			DbFieldType::TimestampTz => {
				if field_meta.nullable && empty {
					return Ok(Value::ChronoDateTimeUtc(None));
				}
				let value = value.as_str().ok_or_else(|| {
					AdminError::ValidationError(format!(
						"Field '{field_name}' requires an ISO date-time"
					))
				})?;
				let value = chrono::DateTime::parse_from_rfc3339(value)
					.map(|value| value.with_timezone(&chrono::Utc))
					.or_else(|_| parse_inline_naive_datetime(value).map(|value| value.and_utc()))
					.map_err(|_| {
						AdminError::ValidationError(format!(
							"Field '{field_name}' requires an ISO date-time"
						))
					})?;
				return Ok(Value::ChronoDateTimeUtc(Some(Box::new(value))));
			}
			_ => {}
		}
	}

	json_to_sea_value(table_name, field_name, value)
}

fn json_to_sea_value(
	table_name: &str,
	field_name: &str,
	value: serde_json::Value,
) -> AdminResult<Value> {
	if let Some(field_meta) =
		crate::server::type_inference::get_field_metadata(table_name, field_name)
		&& let Some(value) =
			json_to_structured_value(field_name, &field_meta.field_type, value.clone())
	{
		return value;
	}

	#[cfg(server)]
	let field_type = field_type_for_value(table_name, field_name);
	#[cfg(not(server))]
	let field_type: Option<DbFieldType> = None;

	#[cfg(feature = "pgvector")]
	#[cfg(server)]
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
		serde_json::Value::String(s) => string_value_for_field(s, field_type.as_ref()),
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

pub(crate) fn validate_admin_database_value(
	table_name: &str,
	field_name: &str,
	value: &serde_json::Value,
) -> AdminResult<()> {
	json_to_update_value(table_name, field_name, value.clone()).map(drop)
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

/// Canonicalizes a primary key using its registered database type.
///
/// Returns the canonical string identity together with the typed SeaQuery
/// value used in database predicates. When registry metadata is unavailable,
/// canonical signed integer IDs retain numeric compatibility while other
/// strings remain unchanged.
pub(crate) fn canonicalize_admin_primary_key(
	table_name: &str,
	pk_field: &str,
	id: &str,
) -> AdminResult<(String, Value)> {
	if let Some(field_meta) =
		crate::server::type_inference::get_field_metadata(table_name, pk_field)
	{
		match field_meta.field_type {
			DbFieldType::Uuid => {
				let uuid = uuid::Uuid::parse_str(id).map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a UUID value"
					))
				})?;
				return Ok((uuid.to_string(), Value::Uuid(Some(Box::new(uuid)))));
			}
			DbFieldType::BigInteger => {
				let number = id.parse::<i64>().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a 64-bit integer value"
					))
				})?;
				return Ok((number.to_string(), Value::BigInt(Some(number))));
			}
			DbFieldType::Custom(type_name) => {
				let (canonical, value) = match type_name.as_str() {
					"u8" => {
						let value = id.parse::<u8>().map_err(|_| {
							AdminError::ValidationError(format!(
								"Primary key field '{pk_field}' requires an unsigned 8-bit integer value"
							))
						})?;
						(value.to_string(), Value::TinyUnsigned(Some(value)))
					}
					"u16" => {
						let value = id.parse::<u16>().map_err(|_| {
							AdminError::ValidationError(format!(
								"Primary key field '{pk_field}' requires an unsigned 16-bit integer value"
							))
						})?;
						(value.to_string(), Value::SmallUnsigned(Some(value)))
					}
					"u32" => {
						let value = id.parse::<u32>().map_err(|_| {
							AdminError::ValidationError(format!(
								"Primary key field '{pk_field}' requires an unsigned 32-bit integer value"
							))
						})?;
						(value.to_string(), Value::Unsigned(Some(value)))
					}
					"u64" => {
						let value = id.parse::<u64>().map_err(|_| {
							AdminError::ValidationError(format!(
								"Primary key field '{pk_field}' requires an unsigned 64-bit integer value"
							))
						})?;
						(value.to_string(), Value::BigUnsigned(Some(value)))
					}
					_ => (
						id.to_string(),
						Value::String(Some(Box::new(id.to_string()))),
					),
				};
				return Ok((canonical, value));
			}
			DbFieldType::Integer
			| DbFieldType::SmallInteger
			| DbFieldType::TinyInt
			| DbFieldType::MediumInt => {
				let number = id.parse::<i64>().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires an integer value"
					))
				})?;
				let value = i32::try_from(number).map_or(Value::BigInt(Some(number)), |number| {
					Value::Int(Some(number))
				});
				return Ok((number.to_string(), value));
			}
			DbFieldType::Year => {
				let number = id.parse::<i32>().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a year value"
					))
				})?;
				return Ok((number.to_string(), Value::Int(Some(number))));
			}
			DbFieldType::Boolean => {
				let value = id.parse::<bool>().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a boolean value"
					))
				})?;
				return Ok((value.to_string(), Value::Bool(Some(value))));
			}
			DbFieldType::Decimal { .. } => {
				let value = Value::BigDecimal(Some(Box::new(id.parse().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a decimal value"
					))
				})?)));
				let Value::BigDecimal(Some(decimal)) = &value else {
					unreachable!("constructed decimal value must contain a decimal")
				};
				return Ok((decimal.normalized().to_string(), value));
			}
			DbFieldType::Float => {
				let value = id.parse::<f32>().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a floating-point value"
					))
				})?;
				if !value.is_finite() {
					return Err(AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a finite value"
					)));
				}
				let canonical = if value == 0.0 {
					"0".to_string()
				} else {
					value.to_string()
				};
				return Ok((canonical, Value::Float(Some(value))));
			}
			DbFieldType::Double | DbFieldType::Real => {
				let value = id.parse::<f64>().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a floating-point value"
					))
				})?;
				if !value.is_finite() {
					return Err(AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires a finite value"
					)));
				}
				let canonical = if value == 0.0 {
					"0".to_string()
				} else {
					value.to_string()
				};
				return Ok((canonical, Value::Double(Some(value))));
			}
			DbFieldType::Date => {
				let value = chrono::NaiveDate::parse_from_str(id, "%Y-%m-%d").map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires an ISO date"
					))
				})?;
				return Ok((value.to_string(), Value::ChronoDate(Some(Box::new(value)))));
			}
			DbFieldType::Time => {
				let value = chrono::NaiveTime::parse_from_str(id, "%H:%M:%S%.f")
					.or_else(|_| chrono::NaiveTime::parse_from_str(id, "%H:%M"))
					.map_err(|_| {
						AdminError::ValidationError(format!(
							"Primary key field '{pk_field}' requires an ISO time"
						))
					})?;
				return Ok((value.to_string(), Value::ChronoTime(Some(Box::new(value)))));
			}
			DbFieldType::DateTime => {
				let value = chrono::NaiveDateTime::parse_from_str(id, "%Y-%m-%dT%H:%M:%S%.f")
					.or_else(|_| chrono::NaiveDateTime::parse_from_str(id, "%Y-%m-%d %H:%M:%S%.f"))
					.or_else(|_| {
						chrono::DateTime::parse_from_rfc3339(id).map(|value| value.naive_utc())
					})
					.map_err(|_| {
						AdminError::ValidationError(format!(
							"Primary key field '{pk_field}' requires an ISO date-time"
						))
					})?;
				return Ok((
					value.and_utc().to_rfc3339(),
					Value::ChronoDateTime(Some(Box::new(value))),
				));
			}
			DbFieldType::TimestampTz => {
				let value = chrono::DateTime::parse_from_rfc3339(id).map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires an RFC 3339 date-time"
					))
				})?;
				let value = value.with_timezone(&chrono::Utc);
				return Ok((
					value.to_rfc3339(),
					Value::ChronoDateTimeUtc(Some(Box::new(value))),
				));
			}
			DbFieldType::ForeignKey { .. } | DbFieldType::OneToOne { .. } => {
				let number = id.parse::<i64>().map_err(|_| {
					AdminError::ValidationError(format!(
						"Primary key field '{pk_field}' requires an integer relation ID"
					))
				})?;
				return Ok((number.to_string(), Value::BigInt(Some(number))));
			}
			_ => {
				return Ok((
					id.to_string(),
					Value::String(Some(Box::new(id.to_string()))),
				));
			}
		}
	}

	Ok(fallback_primary_key_identity(id))
}

fn fallback_primary_key_identity(id: &str) -> (String, Value) {
	if let Ok(num_id) = id.parse::<i64>()
		&& num_id.to_string() == id
	{
		(num_id.to_string(), Value::BigInt(Some(num_id)))
	} else {
		(
			id.to_string(),
			Value::String(Some(Box::new(id.to_string()))),
		)
	}
}

fn legacy_primary_key_identity(id: &str) -> (String, Value) {
	if let Ok(num_id) = id.parse::<i64>() {
		(num_id.to_string(), Value::BigInt(Some(num_id)))
	} else {
		(
			id.to_string(),
			Value::String(Some(Box::new(id.to_string()))),
		)
	}
}

/// Converts a string primary key value to the typed query value used by CRUD.
fn parse_pk_value(table_name: &str, pk_field: &str, id: &str) -> AdminResult<Value> {
	if let Some(field_meta) =
		crate::server::type_inference::get_field_metadata(table_name, pk_field)
	{
		let invalid = |kind: &str| {
			AdminError::ValidationError(format!(
				"Invalid {kind} primary key value '{id}' for field '{pk_field}'"
			))
		};

		return match &field_meta.field_type {
			DbFieldType::Char(_)
			| DbFieldType::VarChar(_)
			| DbFieldType::Text
			| DbFieldType::TinyText
			| DbFieldType::MediumText
			| DbFieldType::LongText
			| DbFieldType::CIText
			| DbFieldType::Enum { .. }
			| DbFieldType::Set { .. } => Ok(Value::String(Some(Box::new(id.to_string())))),
			DbFieldType::Uuid => uuid::Uuid::parse_str(id)
				.map(|uuid| Value::Uuid(Some(Box::new(uuid))))
				.map_err(|_| invalid("UUID")),
			DbFieldType::BigInteger => id
				.parse::<i64>()
				.map(|value| Value::BigInt(Some(value)))
				.map_err(|_| invalid("integer")),
			DbFieldType::Integer
			| DbFieldType::SmallInteger
			| DbFieldType::TinyInt
			| DbFieldType::MediumInt => id
				.parse::<i32>()
				.map(|value| Value::Int(Some(value)))
				.map_err(|_| invalid("integer")),
			DbFieldType::Date
			| DbFieldType::Time
			| DbFieldType::DateTime
			| DbFieldType::TimestampTz => {
				let value = string_value_for_field(id.to_string(), Some(&field_meta.field_type));
				if matches!(value, Value::String(_)) {
					Err(invalid("temporal"))
				} else {
					Ok(value)
				}
			}
			_ => Ok(legacy_primary_key_identity(id).1),
		};
	}

	Ok(fallback_primary_key_identity(id).1)
}

/// Returns the canonical string form used by history and mutation identity.
pub(crate) fn canonicalize_pk_value(table_name: &str, pk_field: &str, id: &str) -> String {
	canonicalize_admin_primary_key(table_name, pk_field, id)
		.map(|(canonical_id, _)| canonical_id)
		.unwrap_or_else(|_| id.to_string())
}

/// Batch version of `parse_pk_value` for bulk operations.
fn parse_pk_values(table_name: &str, pk_field: &str, ids: &[String]) -> AdminResult<Vec<Value>> {
	ids.iter()
		.map(|id| parse_pk_value(table_name, pk_field, id))
		.collect()
}

fn build_update_statement(
	table_name: &str,
	pk_field: &str,
	id: &str,
	data: &HashMap<String, serde_json::Value>,
	backend: DatabaseBackend,
) -> AdminResult<(String, Vec<QueryValue>)> {
	let (_, pk_value) = canonicalize_admin_primary_key(table_name, pk_field, id)?;
	build_update_statement_with_pk_value(
		table_name,
		pk_field,
		pk_value,
		data,
		&HashSet::new(),
		backend,
	)
}

fn build_primary_key_exists_statement(
	table_name: &str,
	pk_field: &str,
	id: &str,
	backend: DatabaseBackend,
) -> AdminResult<(String, Vec<QueryValue>)> {
	let query = Query::select()
		.from(Alias::new(table_name))
		.column(Alias::new(pk_field))
		.and_where(Expr::col(Alias::new(pk_field)).eq(parse_pk_value(table_name, pk_field, id)?))
		.limit(1)
		.to_owned();
	let (sql, values) = match backend {
		DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
		DatabaseBackend::MySql => query.build(MySqlQueryBuilder),
		DatabaseBackend::Sqlite => query.build(SqliteQueryBuilder),
	};
	Ok((sql, convert_admin_values(values)))
}

fn build_update_statement_with_pk_value(
	table_name: &str,
	pk_field: &str,
	pk_value: Value,
	data: &HashMap<String, serde_json::Value>,
	json_null_fields: &HashSet<String>,
	backend: DatabaseBackend,
) -> AdminResult<(String, Vec<QueryValue>)> {
	let mut query = Query::update().table(Alias::new(table_name)).to_owned();
	let mut sorted_keys: Vec<&String> = data.keys().collect();
	sorted_keys.sort();

	for key in sorted_keys {
		let value = if json_null_fields.contains(key) {
			Value::Json(Some(Box::new(serde_json::Value::Null)))
		} else {
			let value = data.get(key).cloned().unwrap_or(serde_json::Value::Null);
			json_to_update_value(table_name, key, value)?
		};
		if value.is_null() {
			query.value_expr(Alias::new(key), Expr::cust("NULL"));
		} else if let Some(type_name) = postgres_parameter_cast(table_name, key, backend) {
			query.value_expr(
				Alias::new(key),
				Expr::val(value).cast_as(Alias::new(type_name)),
			);
		} else {
			query.value(Alias::new(key), value);
		}
	}
	let pk_value = if let Some(type_name) = postgres_parameter_cast(table_name, pk_field, backend) {
		Expr::val(pk_value).cast_as(Alias::new(type_name))
	} else {
		pk_value.into()
	};
	query.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value));
	let (sql, values) = build_update_for_backend(&query, backend);
	Ok((sql, convert_admin_values(values)))
}

fn postgres_parameter_cast(
	table_name: &str,
	field_name: &str,
	backend: DatabaseBackend,
) -> Option<&'static str> {
	if !matches!(backend, DatabaseBackend::Postgres) {
		return None;
	}
	match crate::server::type_inference::get_field_metadata(table_name, field_name)?.field_type {
		DbFieldType::Decimal { .. } => Some("numeric"),
		DbFieldType::Date => Some("date"),
		DbFieldType::Time => Some("time"),
		_ => None,
	}
}

fn convert_admin_array(array_type: ArrayType, values: Option<Box<Vec<Value>>>) -> QueryValue {
	let Some(values) = values else {
		return QueryValue::Null;
	};
	match array_type {
		ArrayType::String | ArrayType::Char => QueryValue::StringArray(
			values
				.into_iter()
				.filter_map(|value| match value {
					Value::String(Some(value)) => Some(*value),
					Value::Char(Some(value)) => Some(value.to_string()),
					_ => None,
				})
				.collect(),
		),
		ArrayType::TinyInt | ArrayType::SmallInt | ArrayType::Int => QueryValue::IntArray(
			values
				.into_iter()
				.filter_map(|value| match value {
					Value::TinyInt(Some(value)) => Some(i32::from(value)),
					Value::SmallInt(Some(value)) => Some(i32::from(value)),
					Value::Int(Some(value)) => Some(value),
					_ => None,
				})
				.collect(),
		),
		ArrayType::BigInt => QueryValue::BigIntArray(
			values
				.into_iter()
				.filter_map(|value| match value {
					Value::BigInt(Some(value)) => Some(value),
					_ => None,
				})
				.collect(),
		),
		ArrayType::Bool => QueryValue::BoolArray(
			values
				.into_iter()
				.filter_map(|value| match value {
					Value::Bool(Some(value)) => Some(value),
					_ => None,
				})
				.collect(),
		),
		ArrayType::Float => QueryValue::FloatArray(
			values
				.into_iter()
				.filter_map(|value| match value {
					Value::Float(Some(value)) => Some(value),
					_ => None,
				})
				.collect(),
		),
		ArrayType::Double => QueryValue::DoubleArray(
			values
				.into_iter()
				.filter_map(|value| match value {
					Value::Double(Some(value)) => Some(value),
					_ => None,
				})
				.collect(),
		),
		ArrayType::Uuid => QueryValue::UuidArray(
			values
				.into_iter()
				.filter_map(|value| match value {
					Value::Uuid(Some(value)) => Some(*value),
					_ => None,
				})
				.collect(),
		),
		_ => unreachable!("unsupported admin array type: {array_type:?}"),
	}
}

fn convert_admin_value(value: Value) -> QueryValue {
	match value {
		Value::BigUnsigned(Some(value)) => QueryValue::String(value.to_string()),
		Value::BigUnsigned(None) => QueryValue::Null,
		Value::Array(array_type, values) => convert_admin_array(array_type, values),
		Value::Decimal(Some(value)) => QueryValue::String(value.to_string()),
		Value::BigDecimal(Some(value)) => QueryValue::String(value.to_string()),
		Value::ChronoDate(Some(value)) => QueryValue::String(value.to_string()),
		Value::ChronoTime(Some(value)) => QueryValue::String(value.to_string()),
		value => convert_values(Values(vec![value]))
			.into_iter()
			.next()
			.expect("one query value produces one backend parameter"),
	}
}

fn convert_admin_values(values: Values) -> Vec<QueryValue> {
	values.0.into_iter().map(convert_admin_value).collect()
}

fn build_update_for_backend(
	query: &UpdateStatement,
	backend: DatabaseBackend,
) -> (String, reinhardt_query::prelude::Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_update(query),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_update(query),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_update(query),
	}
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
		// Subquery and PostgreSQL-specific aggregation types produce SQL
		// from internally constructed ORM queries, not from user HTTP input.
		// Their SQL output is safe because it's built through type-safe ORM APIs.
		AnnotationValue::Subquery(_) => Expr::cust(val.to_sql()).into(),
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
		(FilterOperator::IsNull, _) => col.is_null(),
		(FilterOperator::IsNotNull, _) => col.is_not_null(),
		(FilterOperator::Eq, FilterValue::Null) => col.is_null(),
		(FilterOperator::Ne, FilterValue::Null) => col.is_not_null(),
		(FilterOperator::IExact, FilterValue::String(s)) => {
			col.binary(BinOper::ILike, SimpleExpr::from(s.clone()))
		}
		(FilterOperator::IExact, v) => col.eq(filter_comparison_value(v)?),

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
				filter_comparison_value(start)?,
				filter_comparison_value(end)?,
			],
		)
		.into(),
		// Array-based In/NotIn: convert each element to a Value
		(FilterOperator::In, FilterValue::Array(arr)) => {
			let values: Vec<Value> = arr.iter().map(|v| v.as_str().into_value()).collect();
			col.is_in(values)
		}
		(FilterOperator::NotIn, FilterValue::Array(arr)) => {
			let values: Vec<Value> = arr.iter().map(|v| v.as_str().into_value()).collect();
			col.is_not_in(values)
		}
		(FilterOperator::In, FilterValue::List(values)) => col.is_in(
			values
				.iter()
				.map(filter_comparison_value)
				.collect::<AdminResult<Vec<_>>>()?,
		),
		(FilterOperator::NotIn, FilterValue::List(values)) => col.is_not_in(
			values
				.iter()
				.map(filter_comparison_value)
				.collect::<AdminResult<Vec<_>>>()?,
		),

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
				let Some(sub_cond) =
					build_composite_filter_condition_for_table(cond, depth + 1, table_name)?
				else {
					return Ok(None);
				};
				and_condition = and_condition.add(sub_cond);
				added = true;
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
				let Some(sub_cond) =
					build_composite_filter_condition_for_table(cond, depth + 1, table_name)?
				else {
					return Ok(None);
				};
				or_condition = or_condition.add(sub_cond);
				added = true;
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

	if let Some(fc) = filter_condition {
		let cond = build_composite_filter_condition(fc)?.ok_or_else(|| {
			AdminError::ValidationError(
				"Admin queryset contains a condition that cannot be applied".to_string(),
			)
		})?;
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

fn related_column_alias(relation_index: usize, column_index: usize, column: &str) -> String {
	let alias = format!("{ADMIN_RELATED_COLUMN_ALIAS_PREFIX}{relation_index}__{column}");
	if alias.len() <= 63 {
		alias
	} else {
		format!("{ADMIN_RELATED_COLUMN_ALIAS_PREFIX}{relation_index}_{column_index}")
	}
}

fn related_table_alias(relation_index: usize) -> String {
	format!("__reinhardt_related_table_{relation_index}")
}

fn add_related_joins(
	statement: &mut SelectStatement,
	table_name: &str,
	related_fields: &[AdminRelatedField],
) {
	for (relation_index, related) in related_fields.iter().enumerate() {
		let table_alias = related_table_alias(relation_index);
		statement.left_join(
			TableRef::table_alias(Alias::new(&related.target_table), Alias::new(&table_alias)),
			Expr::col((Alias::new(table_name), Alias::new(&related.source_column)))
				.equals((Alias::new(&table_alias), Alias::new(&related.target_column))),
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
		let table_alias = related_table_alias(relation_index);
		for (column_index, column) in related.columns.iter().enumerate() {
			statement.expr_as(
				Expr::col((Alias::new(&table_alias), Alias::new(column))),
				Alias::new(related_column_alias(relation_index, column_index, column)),
			);
		}
		if !related
			.columns
			.iter()
			.any(|column| column == &related.presence_column)
		{
			statement.expr_as(
				Expr::col((
					Alias::new(&table_alias),
					Alias::new(&related.presence_column),
				)),
				Alias::new(related_column_alias(
					relation_index,
					related.columns.len(),
					&related.presence_column,
				)),
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
		let mut has_related_row = if related
			.columns
			.iter()
			.any(|column| column == &related.presence_column)
		{
			false
		} else {
			let alias = related_column_alias(
				relation_index,
				related.columns.len(),
				&related.presence_column,
			);
			let value = map.remove(&alias).ok_or_else(|| {
				AdminError::DatabaseError(format!(
					"Admin list query result missing related presence alias '{alias}'"
				))
			})?;
			!value.is_null()
		};

		for (column_index, column) in related.columns.iter().enumerate() {
			let alias = related_column_alias(relation_index, column_index, column);
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

pub(crate) struct AdminCreateResult {
	pub affected: u64,
	pub primary_key: serde_json::Value,
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

	/// List items with composite filters and multiple deterministic sort fields.
	pub async fn list_with_condition_ordered<M: Model>(
		&self,
		table_name: &str,
		filter_condition: Option<&FilterCondition>,
		additional_filters: Vec<Filter>,
		ordering: &[&str],
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

		// Apply sorting in declaration order so callers can add a stable tie-breaker.
		for &sort_str in ordering {
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

	/// List items with composite filter conditions (supports AND/OR logic).
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
		let ordering = sort_by.into_iter().collect::<Vec<_>>();
		self.list_with_condition_ordered::<M>(
			table_name,
			filter_condition,
			additional_filters,
			&ordering,
			offset,
			limit,
		)
		.await
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
		let mut connection = self.connection;
		self.get_with_executor(&mut connection, table_name, pk_field, id)
			.await
	}

	pub(crate) async fn get_with_executor<E>(
		&self,
		executor: &mut E,
		table_name: &str,
		pk_field: &str,
		id: &str,
	) -> AdminResult<Option<HashMap<String, serde_json::Value>>>
	where
		E: OrmExecutor,
	{
		let pk_value = parse_pk_value(table_name, pk_field, id)?;

		// SELECT * is intentional: admin detail view displays all fields from the
		// model. The admin panel operates on dynamic schemas where the column set
		// is determined by the ModelAdmin configuration at runtime.
		let query = Query::select()
			.from(Alias::new(table_name))
			.column(ColumnRef::Asterisk)
			.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value))
			.limit(1)
			.to_owned();

		let (sql, values) = match executor.backend() {
			reinhardt_db::orm::DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
			reinhardt_db::orm::DatabaseBackend::MySql => {
				query.build(reinhardt_query::prelude::MySqlQueryBuilder)
			}
			reinhardt_db::orm::DatabaseBackend::Sqlite => {
				query.build(reinhardt_query::prelude::SqliteQueryBuilder)
			}
		};
		let params = convert_admin_values(values);
		let row = executor
			.fetch_all(&sql, params)
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?
			.into_iter()
			.next()
			.map(QueryRow::from_backend_row);

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
		let mut connection = self.connection;
		let result = self
			.create_with_executor(&mut connection, table_name, Some(pk_field), data)
			.await?;

		match result.primary_key {
			serde_json::Value::Number(number) => number.as_u64().ok_or_else(|| {
				AdminError::DatabaseError(format!(
					"RETURNING clause for '{}' returned non-unsigned-integer: {}",
					pk_field, number
				))
			}),
			serde_json::Value::String(_) => Ok(1),
			_ => Err(AdminError::DatabaseError(format!(
				"RETURNING clause did not return expected primary key field '{}'",
				pk_field
			))),
		}
	}

	pub(crate) async fn create_with_executor<E>(
		&self,
		executor: &mut E,
		table_name: &str,
		pk_field: Option<&str>,
		data: HashMap<String, serde_json::Value>,
	) -> AdminResult<AdminCreateResult>
	where
		E: OrmExecutor,
	{
		let pk_field = pk_field.unwrap_or("id");
		let backend = executor.backend();
		let mut data = data;
		if backend == reinhardt_db::orm::DatabaseBackend::MySql
			&& !data.contains_key(pk_field)
			&& let Some(primary_key) = mysql_primary_key_value(table_name, pk_field)
		{
			data.insert(pk_field.to_string(), primary_key);
		}
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

		if columns.is_empty() {
			query.default_values();
		} else {
			// Pass values directly for reinhardt-query
			query.columns(columns).values(values).map_err(|e| {
				AdminError::DatabaseError(format!("column/value count mismatch: {}", e))
			})?;
		}

		if backend != reinhardt_db::orm::DatabaseBackend::MySql {
			query.returning([Alias::new(pk_field)]);
		}

		let (sql, values) = match backend {
			reinhardt_db::orm::DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
			reinhardt_db::orm::DatabaseBackend::MySql => {
				query.build(reinhardt_query::prelude::MySqlQueryBuilder)
			}
			reinhardt_db::orm::DatabaseBackend::Sqlite => {
				query.build(reinhardt_query::prelude::SqliteQueryBuilder)
			}
		};
		let params = convert_admin_values(values);
		let primary_key = if backend == reinhardt_db::orm::DatabaseBackend::MySql {
			let result = executor
				.execute(&sql, params)
				.await
				.map_err(|e| AdminError::DatabaseError(e.to_string()))?;
			result
				.last_insert_id
				.map(|id| serde_json::json!(id))
				.or_else(|| data.get(pk_field).cloned())
				.ok_or_else(|| {
					AdminError::DatabaseError(format!(
						"insert did not return or provide primary key field '{}'",
						pk_field
					))
				})?
		} else {
			let row = executor
				.fetch_one(&sql, params)
				.await
				.map_err(|e| AdminError::DatabaseError(e.to_string()))?;
			let row = reinhardt_db::orm::QueryRow::from_backend_row(row);
			row.data.get(pk_field).cloned().ok_or_else(|| {
				AdminError::DatabaseError(format!(
					"RETURNING clause did not return expected primary key field '{}'",
					pk_field
				))
			})?
		};

		match &primary_key {
			serde_json::Value::Number(n) => n.as_u64().ok_or_else(|| {
				AdminError::DatabaseError(format!(
					"RETURNING clause for '{}' returned non-unsigned-integer: {}",
					pk_field, n
				))
			}),
			serde_json::Value::String(_) => {
				// UUID and other string-based PKs: return 1 as affected count
				// (the actual PK value is a string, not representable as u64)
				Ok(1)
			}
			_ => Err(AdminError::DatabaseError(format!(
				"RETURNING clause did not return expected primary key field '{}'",
				pk_field
			))),
		}?;

		Ok(AdminCreateResult {
			affected: 1,
			primary_key,
		})
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
		let mut connection = self.connection;
		self.update_with_executor(&mut connection, table_name, pk_field, id, data)
			.await
	}

	pub(crate) async fn update_with_executor<E>(
		&self,
		executor: &mut E,
		table_name: &str,
		pk_field: &str,
		id: &str,
		data: HashMap<String, serde_json::Value>,
	) -> AdminResult<u64>
	where
		E: OrmExecutor,
	{
		let (sql, params) = build_update_statement_with_pk_value(
			table_name,
			pk_field,
			parse_pk_value(table_name, pk_field, id)?,
			&data,
			&HashSet::new(),
			OrmExecutor::backend(executor),
		)?;
		let affected = executor
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?
			.rows_affected;

		Ok(affected)
	}

	/// Updates a validated batch and runs follow-up work inside the same transaction.
	pub(crate) async fn update_batch_with<F>(
		&self,
		table_name: &str,
		pk_field: &str,
		mutations: Vec<AdminBatchMutation>,
		after_updates: F,
	) -> Result<u64, AdminBatchAtomicError>
	where
		F: for<'transaction> std::ops::AsyncFnOnce(
				&'transaction mut AtomicTransaction,
			) -> AdminResult<()>,
	{
		let backend = OrmExecutor::backend(&self.connection);
		let statements = mutations
			.iter()
			.map(|mutation| {
				if mutation.json_null_fields.is_empty() {
					return build_update_statement(
						table_name,
						pk_field,
						mutation.object_id(),
						&mutation.data,
						backend,
					);
				}
				let (_, pk_value) =
					canonicalize_admin_primary_key(table_name, pk_field, mutation.object_id())?;
				build_update_statement_with_pk_value(
					table_name,
					pk_field,
					pk_value,
					&mutation.data,
					&mutation.json_null_fields,
					backend,
				)
			})
			.collect::<AdminResult<Vec<_>>>()?;

		self.connection
			.atomic(async move |transaction| {
				let mut affected = 0;
				for (row_index, ((sql, params), mutation)) in
					statements.into_iter().zip(&mutations).enumerate()
				{
					let result = OrmExecutor::execute(transaction, &sql, params).await?;
					if result.rows_affected == 0 {
						if backend == DatabaseBackend::MySql {
							let (exists_sql, exists_params) = build_primary_key_exists_statement(
								table_name,
								pk_field,
								mutation.object_id(),
								backend,
							)?;
							if OrmExecutor::fetch_optional(transaction, &exists_sql, exists_params)
								.await?
								.is_some()
							{
								affected += 1;
								continue;
							}
						}
						return Err(AdminBatchAtomicError::ZeroAffected {
							row_index,
							object_id: mutation.object_id().to_string(),
						});
					}
					affected += result.rows_affected;
				}

				after_updates(transaction).await?;
				Ok(affected)
			})
			.await
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
		let mut connection = self.connection;
		self.delete_with_executor(&mut connection, table_name, pk_field, id)
			.await
	}

	pub(crate) async fn delete_with_executor<E>(
		&self,
		executor: &mut E,
		table_name: &str,
		pk_field: &str,
		id: &str,
	) -> AdminResult<u64>
	where
		E: OrmExecutor,
	{
		let pk_value = parse_pk_value(table_name, pk_field, id)?;

		let query = Query::delete()
			.from_table(Alias::new(table_name))
			.and_where(Expr::col(Alias::new(pk_field)).eq(pk_value))
			.to_owned();

		let (sql, values) = match executor.backend() {
			reinhardt_db::orm::DatabaseBackend::Postgres => query.build(PostgresQueryBuilder),
			reinhardt_db::orm::DatabaseBackend::MySql => {
				query.build(reinhardt_query::prelude::MySqlQueryBuilder)
			}
			reinhardt_db::orm::DatabaseBackend::Sqlite => {
				query.build(reinhardt_query::prelude::SqliteQueryBuilder)
			}
		};
		let params = convert_admin_values(values);
		let affected = executor
			.execute(&sql, params)
			.await
			.map_err(|e| AdminError::DatabaseError(e.to_string()))?
			.rows_affected;

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

		let pk_values = parse_pk_values(table_name, pk_field, &ids)?;

		let query = Query::delete()
			.from_table(Alias::new(table_name))
			.and_where(Expr::col(Alias::new(pk_field)).is_in(pk_values))
			.to_owned();

		let (sql, values) = query.build(PostgresQueryBuilder);
		let params = convert_admin_values(values);
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
	use std::collections::VecDeque;
	use std::sync::Arc;

	use super::*;
	use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
	use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
	use reinhardt_db::migrations::model_registry::{FieldMetadata, ModelMetadata, global_registry};
	use reinhardt_db::orm::annotation::Expression;
	use reinhardt_db::orm::expressions::{F, FieldRef, OuterRef};
	use reinhardt_db::orm::{
		DatabaseBackend, DatabaseConnectionLease, QueryResult, QueryValue, Row,
	};
	use reinhardt_query::prelude::{ColumnDef, SqliteQueryBuilder};
	use rstest::rstest;
	use serial_test::serial;
	use uuid::Uuid;

	struct DatabaseMetadataGuard {
		app_label: String,
		model_name: String,
	}

	impl Drop for DatabaseMetadataGuard {
		fn drop(&mut self) {
			global_registry().remove_model(&self.app_label, &self.model_name);
		}
	}

	fn register_database_metadata(
		fields: impl IntoIterator<Item = (&'static str, FieldMetadata)>,
	) -> (String, DatabaseMetadataGuard) {
		let suffix = Uuid::new_v4().simple().to_string();
		let app_label = format!("admin_database_{suffix}");
		let model_name = format!("AdminDatabase{suffix}");
		let table_name = format!("admin_database_{suffix}");
		let mut metadata = ModelMetadata::new(&app_label, &model_name, &table_name);
		for (name, field) in fields {
			metadata.add_field(name.to_string(), field);
		}
		global_registry().register_model(metadata);
		(
			table_name,
			DatabaseMetadataGuard {
				app_label,
				model_name,
			},
		)
	}

	#[rstest]
	#[serial(admin_database_metadata)]
	fn postgres_update_preserves_decimal_and_temporal_parameters_and_uses_literal_null() {
		let (table_name, _guard) = register_database_metadata([
			("id", FieldMetadata::new(DbFieldType::Date)),
			(
				"amount",
				FieldMetadata::new(DbFieldType::Decimal {
					precision: 40,
					scale: 9,
				})
				.with_nullable(true),
			),
		]);
		let decimal = "123456789012345678901234567890.123456789";

		let (sql, params) = build_update_statement(
			&table_name,
			"id",
			"2026-08-10",
			&HashMap::from([("amount".to_string(), serde_json::json!(decimal))]),
			DatabaseBackend::Postgres,
		)
		.expect("build exact decimal update");
		let (null_sql, null_params) = build_update_statement(
			&table_name,
			"id",
			"2026-08-10",
			&HashMap::from([("amount".to_string(), serde_json::Value::Null)]),
			DatabaseBackend::Postgres,
		)
		.expect("build nullable decimal update");

		assert_eq!(
			sql,
			format!(
				r#"UPDATE "{table_name}" SET "amount" = CAST($1 AS "numeric") WHERE "id" = CAST($2 AS "date")"#
			)
		);
		assert_eq!(
			params,
			vec![
				QueryValue::String(decimal.to_string()),
				QueryValue::String("2026-08-10".to_string()),
			]
		);
		assert_eq!(
			null_sql,
			format!(r#"UPDATE "{table_name}" SET "amount" = NULL WHERE "id" = CAST($1 AS "date")"#)
		);
		assert_eq!(
			null_params,
			vec![QueryValue::String("2026-08-10".to_string())]
		);
	}

	#[rstest]
	#[serial(admin_database_metadata)]
	fn postgres_update_uses_native_datetime_parameters() {
		let (table_name, _guard) = register_database_metadata([
			("id", FieldMetadata::new(DbFieldType::Date)),
			("starts_at", FieldMetadata::new(DbFieldType::DateTime)),
			("published_at", FieldMetadata::new(DbFieldType::TimestampTz)),
			("time_of_day", FieldMetadata::new(DbFieldType::Time)),
		]);
		let date = chrono::NaiveDate::from_ymd_opt(2026, 8, 10).expect("valid date fixture");
		let starts_at = date.and_hms_opt(9, 8, 0).expect("valid date-time fixture");
		let published_at = date.and_hms_opt(0, 8, 7).expect("valid date-time fixture");
		let time = chrono::NaiveTime::from_hms_opt(9, 8, 0).expect("valid time fixture");

		let (sql, params) = build_update_statement(
			&table_name,
			"id",
			"2026-08-10",
			&HashMap::from([
				(
					"starts_at".to_string(),
					serde_json::json!("2026-08-10T09:08"),
				),
				(
					"published_at".to_string(),
					serde_json::json!("2026-08-10T00:08:07"),
				),
				("time_of_day".to_string(), serde_json::json!("09:08")),
			]),
			DatabaseBackend::Postgres,
		)
		.expect("build temporal update");

		assert_eq!(
			sql,
			format!(
				r#"UPDATE "{table_name}" SET "published_at" = $1, "starts_at" = $2, "time_of_day" = CAST($3 AS "time") WHERE "id" = CAST($4 AS "date")"#
			)
		);
		assert_eq!(
			params,
			vec![
				QueryValue::Timestamp(published_at.and_utc()),
				QueryValue::NaiveTimestamp(starts_at),
				QueryValue::String(time.to_string()),
				QueryValue::String(date.to_string()),
			]
		);
	}

	#[rstest]
	#[serial(admin_database_metadata)]
	fn postgres_update_preserves_structured_values_and_text_spellings() {
		let (table_name, _guard) = register_database_metadata([
			("id", FieldMetadata::new(DbFieldType::Integer)),
			(
				"payload",
				FieldMetadata::new(DbFieldType::JsonBinary).with_nullable(true),
			),
			(
				"numbers",
				FieldMetadata::new(DbFieldType::Array(Box::new(DbFieldType::Integer))),
			),
			(
				"permissions",
				FieldMetadata::new(DbFieldType::Set {
					values: vec!["read".to_string(), "write".to_string()],
				}),
			),
			("title", FieldMetadata::new(DbFieldType::Text)),
		]);

		let (sql, params) = build_update_statement(
			&table_name,
			"id",
			"1",
			&HashMap::from([
				("numbers".to_string(), serde_json::json!([1, 2, 3])),
				(
					"payload".to_string(),
					serde_json::json!({"enabled": true, "labels": ["a", "b"]}),
				),
				(
					"permissions".to_string(),
					serde_json::json!(["read", "write"]),
				),
				("title".to_string(), serde_json::json!("2026-08-12")),
			]),
			DatabaseBackend::Postgres,
		)
		.expect("build structured update");

		assert_eq!(
			sql,
			format!(
				r#"UPDATE "{table_name}" SET "numbers" = $1, "payload" = $2, "permissions" = $3, "title" = $4 WHERE "id" = $5"#
			)
		);
		assert_eq!(
			params,
			vec![
				QueryValue::IntArray(vec![1, 2, 3]),
				QueryValue::Json(Some(Box::new(serde_json::json!({
					"enabled": true,
					"labels": ["a", "b"],
				})))),
				QueryValue::String("read,write".to_string()),
				QueryValue::String("2026-08-12".to_string()),
				QueryValue::Int(1),
			]
		);
	}

	#[test]
	#[serial(admin_database_metadata)]
	fn mysql_unsigned_primary_key_stays_an_exact_string_parameter() {
		let (table_name, _guard) = register_database_metadata([
			(
				"id",
				FieldMetadata::new(DbFieldType::Custom("u64".to_string())),
			),
			("title", FieldMetadata::new(DbFieldType::Text)),
		]);
		let id = "18446744073709551615";

		let (_, params) = build_update_statement(
			&table_name,
			"id",
			id,
			&HashMap::from([("title".to_string(), serde_json::json!("updated"))]),
			DatabaseBackend::MySql,
		)
		.expect("build unsigned primary-key update");

		assert_eq!(
			params,
			vec![
				QueryValue::String("updated".to_string()),
				QueryValue::String(id.to_string()),
			]
		);
	}

	#[test]
	#[serial(admin_database_metadata)]
	fn primary_key_canonicalization_covers_inline_edit_scalar_types() {
		use reinhardt_db::migrations::ForeignKeyAction;

		let uuid =
			Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("valid UUID fixture");
		let date = chrono::NaiveDate::from_ymd_opt(2026, 8, 10).expect("valid date fixture");
		let time = chrono::NaiveTime::from_hms_opt(9, 8, 0).expect("valid time fixture");
		let naive_datetime = date.and_hms_opt(0, 8, 7).expect("valid date-time fixture");
		let utc_datetime = naive_datetime.and_utc();
		let decimal = "123456789012345678901234567890.123"
			.parse()
			.expect("valid decimal fixture");
		let cases = vec![
			(
				DbFieldType::Uuid,
				"550E8400-E29B-41D4-A716-446655440000",
				uuid.to_string(),
				Value::Uuid(Some(Box::new(uuid))),
			),
			(
				DbFieldType::Boolean,
				"true",
				"true".to_string(),
				Value::Bool(Some(true)),
			),
			(
				DbFieldType::Decimal {
					precision: 40,
					scale: 3,
				},
				"123456789012345678901234567890.123000",
				"123456789012345678901234567890.123".to_string(),
				Value::BigDecimal(Some(Box::new(decimal))),
			),
			(
				DbFieldType::Float,
				"1.25",
				"1.25".to_string(),
				Value::Float(Some(1.25)),
			),
			(
				DbFieldType::Double,
				"2.5",
				"2.5".to_string(),
				Value::Double(Some(2.5)),
			),
			(
				DbFieldType::Real,
				"-0",
				"0".to_string(),
				Value::Double(Some(-0.0)),
			),
			(
				DbFieldType::Date,
				"2026-08-10",
				"2026-08-10".to_string(),
				Value::ChronoDate(Some(Box::new(date))),
			),
			(
				DbFieldType::Time,
				"09:08",
				"09:08:00".to_string(),
				Value::ChronoTime(Some(Box::new(time))),
			),
			(
				DbFieldType::DateTime,
				"2026-08-10T09:08:07+09:00",
				"2026-08-10T00:08:07+00:00".to_string(),
				Value::ChronoDateTime(Some(Box::new(naive_datetime))),
			),
			(
				DbFieldType::TimestampTz,
				"2026-08-10T09:08:07+09:00",
				"2026-08-10T00:08:07+00:00".to_string(),
				Value::ChronoDateTimeUtc(Some(Box::new(utc_datetime))),
			),
			(
				DbFieldType::Year,
				"02026",
				"2026".to_string(),
				Value::Int(Some(2026)),
			),
			(
				DbFieldType::ForeignKey {
					to_table: "related".to_string(),
					to_field: "id".to_string(),
					on_delete: ForeignKeyAction::Cascade,
				},
				"00042",
				"42".to_string(),
				Value::BigInt(Some(42)),
			),
		];

		for (field_type, input, expected_id, expected_value) in cases {
			let (table_name, _guard) =
				register_database_metadata([("id", FieldMetadata::new(field_type.clone()))]);
			let actual = canonicalize_admin_primary_key(&table_name, "id", input)
				.expect("canonicalize supported scalar primary key");
			assert_eq!(actual, (expected_id, expected_value), "{field_type:?}");
		}
	}

	struct MutationExecutor {
		backend: DatabaseBackend,
		rows: VecDeque<Row>,
		fetch_one_calls: usize,
		execute_calls: usize,
		executed_sql: Vec<String>,
		execute_result: QueryResult,
	}

	impl MutationExecutor {
		fn new(rows: impl IntoIterator<Item = Row>) -> Self {
			Self {
				backend: DatabaseBackend::Postgres,
				rows: rows.into_iter().collect(),
				fetch_one_calls: 0,
				execute_calls: 0,
				executed_sql: Vec::new(),
				execute_result: QueryResult {
					rows_affected: 1,
					last_insert_id: None,
				},
			}
		}

		fn mysql(last_insert_id: Option<u64>) -> Self {
			Self {
				backend: DatabaseBackend::MySql,
				rows: VecDeque::new(),
				fetch_one_calls: 0,
				execute_calls: 0,
				executed_sql: Vec::new(),
				execute_result: QueryResult {
					rows_affected: 1,
					last_insert_id,
				},
			}
		}
	}

	#[reinhardt_core::async_trait]
	impl OrmExecutor for MutationExecutor {
		fn backend(&self) -> DatabaseBackend {
			self.backend
		}

		async fn execute(
			&mut self,
			sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<QueryResult, Error> {
			self.execute_calls += 1;
			self.executed_sql.push(sql.to_string());
			Ok(self.execute_result.clone())
		}

		async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row, Error> {
			self.fetch_one_calls += 1;
			self.rows.pop_front().ok_or_else(|| {
				DatabaseError::new(DatabaseErrorKind::Query, "missing queued row").into()
			})
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Vec<Row>, Error> {
			unreachable!("mutation methods do not fetch all rows")
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> Result<Option<Row>, Error> {
			unreachable!("mutation methods do not fetch optional rows")
		}
	}

	async fn test_admin_database() -> (AdminDatabase, DatabaseConnectionLease) {
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("in-memory SQLite connection should initialize");
		let lease = DatabaseConnectionLease::register(owner)
			.expect("SQLite connection should register for the test lifetime");
		let database = AdminDatabase::new(lease.handle());
		(database, lease)
	}

	fn primary_key_row(value: QueryValue) -> Row {
		let mut row = Row::new();
		row.insert("id".to_owned(), value);
		row
	}

	#[rstest]
	#[tokio::test]
	async fn mutation_methods_use_supplied_executor_and_return_integer_primary_key() {
		let (database, _lease) = test_admin_database().await;
		let mut executor = MutationExecutor::new([primary_key_row(QueryValue::Int(42))]);

		let created = database
			.create_with_executor(
				&mut executor,
				"records",
				Some("id"),
				HashMap::from([("name".to_owned(), serde_json::json!("created"))]),
			)
			.await
			.expect("create should use the supplied executor");
		let updated = database
			.update_with_executor(
				&mut executor,
				"records",
				"id",
				"42",
				HashMap::from([("name".to_owned(), serde_json::json!("updated"))]),
			)
			.await
			.expect("update should use the supplied executor");
		let deleted = database
			.delete_with_executor(&mut executor, "records", "id", "42")
			.await
			.expect("delete should use the supplied executor");

		assert_eq!(created.affected, 1);
		assert_eq!(created.primary_key, serde_json::json!(42));
		assert_eq!(updated, 1);
		assert_eq!(deleted, 1);
		assert_eq!(executor.fetch_one_calls, 1);
		assert_eq!(executor.execute_calls, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn public_create_returns_numeric_primary_key_while_executor_result_reports_row_count() {
		let (database, _lease) = test_admin_database().await;
		let mut connection = *database.connection();
		OrmExecutor::execute(
			&mut connection,
			"CREATE TABLE records (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
			Vec::new(),
		)
		.await
		.expect("test table should be created");

		let primary_key = database
			.create::<AdminRecord>(
				"records",
				Some("id"),
				HashMap::from([
					("id".to_owned(), serde_json::json!(42)),
					("name".to_owned(), serde_json::json!("created")),
				]),
			)
			.await
			.expect("public create should preserve the numeric primary key result");

		assert_eq!(primary_key, 42);
	}

	#[rstest]
	#[tokio::test]
	async fn create_with_executor_returns_string_primary_key() {
		let (database, _lease) = test_admin_database().await;
		let mut executor =
			MutationExecutor::new([primary_key_row(QueryValue::String("item-42".to_owned()))]);

		let created = database
			.create_with_executor(
				&mut executor,
				"records",
				Some("id"),
				HashMap::from([("name".to_owned(), serde_json::json!("created"))]),
			)
			.await
			.expect("create should retain a string primary key");

		assert_eq!(created.affected, 1);
		assert_eq!(created.primary_key, serde_json::json!("item-42"));
		assert_eq!(executor.fetch_one_calls, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn mutation_methods_use_mysql_sql_and_last_insert_id() {
		let (database, _lease) = test_admin_database().await;
		let mut executor = MutationExecutor::mysql(Some(42));

		let created = database
			.create_with_executor(
				&mut executor,
				"records",
				Some("id"),
				HashMap::from([("name".to_owned(), serde_json::json!("created"))]),
			)
			.await
			.expect("MySQL create should use the insert result primary key");
		let updated = database
			.update_with_executor(
				&mut executor,
				"records",
				"id",
				"42",
				HashMap::from([("name".to_owned(), serde_json::json!("updated"))]),
			)
			.await
			.expect("MySQL update should use MySQL SQL");
		let deleted = database
			.delete_with_executor(&mut executor, "records", "id", "42")
			.await
			.expect("MySQL delete should use MySQL SQL");

		assert_eq!(created.primary_key, serde_json::json!(42));
		assert_eq!(updated, 1);
		assert_eq!(deleted, 1);
		assert_eq!(executor.fetch_one_calls, 0);
		assert_eq!(executor.execute_calls, 3);
		assert!(
			executor
				.executed_sql
				.iter()
				.all(|sql| sql.contains('?') && !sql.contains("RETURNING"))
		);
		assert!(executor.executed_sql[0].starts_with("INSERT INTO `records`"));
	}

	#[rstest]
	#[tokio::test]
	async fn mysql_create_preserves_submitted_string_primary_key() {
		let (database, _lease) = test_admin_database().await;
		let mut executor = MutationExecutor::mysql(None);

		let created = database
			.create_with_executor(
				&mut executor,
				"records",
				Some("id"),
				HashMap::from([
					("id".to_owned(), serde_json::json!("01")),
					("name".to_owned(), serde_json::json!("created")),
				]),
			)
			.await
			.expect("MySQL create should retain a submitted string primary key");

		assert_eq!(created.primary_key, serde_json::json!("01"));
		assert_eq!(executor.execute_calls, 1);
	}

	#[rstest]
	#[tokio::test]
	#[serial(admin_database_metadata)]
	async fn mysql_create_uses_literal_default_primary_key_without_insert_id() {
		let (table_name, _guard) = register_database_metadata([(
			"id",
			FieldMetadata::new(DbFieldType::VarChar(36))
				.with_param("primary_key", "true")
				.with_param("default", "'generated-id'"),
		)]);
		let (database, _lease) = test_admin_database().await;
		let mut executor = MutationExecutor::mysql(None);

		let created = database
			.create_with_executor(&mut executor, &table_name, Some("id"), HashMap::new())
			.await
			.expect("MySQL should use a literal default primary key");

		assert_eq!(created.primary_key, serde_json::json!("generated-id"));
		assert_eq!(
			executor.executed_sql[0],
			format!("INSERT INTO `{table_name}` (`id`) VALUES (?)")
		);
	}

	#[rstest]
	#[tokio::test]
	#[serial(admin_database_metadata)]
	async fn mysql_create_generates_uuid_primary_key_without_insert_id() {
		let (table_name, _guard) = register_database_metadata([(
			"id",
			FieldMetadata::new(DbFieldType::Uuid).with_param("primary_key", "true"),
		)]);
		let (database, _lease) = test_admin_database().await;
		let mut executor = MutationExecutor::mysql(None);

		let created = database
			.create_with_executor(&mut executor, &table_name, Some("id"), HashMap::new())
			.await
			.expect("MySQL should generate an application UUID primary key");

		let primary_key = created
			.primary_key
			.as_str()
			.and_then(|value| uuid::Uuid::parse_str(value).ok())
			.expect("generated primary key should be a UUID");
		assert_ne!(primary_key, uuid::Uuid::nil());
		assert_eq!(executor.execute_calls, 1);
	}

	#[rstest]
	fn batch_update_uses_mysql_sql() {
		let (sql, params) = build_update_statement(
			"records",
			"id",
			"42",
			&HashMap::from([("name".to_owned(), serde_json::json!("updated"))]),
			DatabaseBackend::MySql,
		)
		.expect("build MySQL batch update");

		assert_eq!(sql, "UPDATE `records` SET `name` = ? WHERE `id` = ?");
		assert_eq!(params.len(), 2);
	}

	async fn batch_sqlite_database(
		records: &[(i32, &str)],
	) -> (DatabaseConnectionLease, DatabaseConnection, AdminDatabase) {
		let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("connect SQLite batch fixture");
		let records_table = Query::create_table()
			.table(Alias::new("batch_records"))
			.col(
				ColumnDef::new(Alias::new("id"))
					.integer()
					.not_null(true)
					.primary_key(true),
			)
			.col(ColumnDef::new(Alias::new("name")).string().not_null(true))
			.col(ColumnDef::new(Alias::new("status")).string())
			.to_string(SqliteQueryBuilder);
		owner
			.execute(&records_table, vec![])
			.await
			.expect("create records fixture");
		let audit_table = Query::create_table()
			.table(Alias::new("batch_audit"))
			.col(
				ColumnDef::new(Alias::new("object_id"))
					.string()
					.not_null(true),
			)
			.to_string(SqliteQueryBuilder);
		owner
			.execute(&audit_table, vec![])
			.await
			.expect("create audit fixture");
		let mut seed = Query::insert();
		seed.into_table(Alias::new("batch_records"))
			.columns([Alias::new("id"), Alias::new("name")]);
		for (id, name) in records {
			seed.values_panic([Expr::val(*id), Expr::val(*name)]);
		}
		let seed = seed.to_string(SqliteQueryBuilder);
		owner
			.execute(&seed, vec![])
			.await
			.expect("seed records fixture");
		let lease = DatabaseConnectionLease::register(owner).expect("register SQLite lease");
		let connection = lease.handle();
		let db = AdminDatabase::new(connection);
		(lease, connection, db)
	}

	#[rstest]
	#[tokio::test]
	async fn batch_update_writes_nullable_values_as_sql_null() {
		let (_lease, connection, db) = batch_sqlite_database(&[(1, "before")]).await;
		let mutation = AdminBatchMutation::new(
			"1".to_string(),
			HashMap::from([("status".to_string(), serde_json::Value::Null)]),
		);

		let updated = db
			.update_batch_with(
				"batch_records",
				"id",
				vec![mutation],
				async |_transaction| Ok(()),
			)
			.await
			.expect("commit nullable batch update");
		let query = Query::select()
			.column(Alias::new("status"))
			.from(Alias::new("batch_records"))
			.to_string(SqliteQueryBuilder);
		let rows = connection
			.query(&query, vec![])
			.await
			.expect("read nullable batch update");

		assert_eq!(updated, 1);
		assert_eq!(rows.len(), 1);
		assert_eq!(rows[0].data.get("status"), Some(&serde_json::Value::Null));
	}

	#[rstest]
	fn batch_update_preserves_explicit_json_null_as_a_json_parameter() {
		// Arrange
		let data = HashMap::from([("payload".to_string(), serde_json::Value::Null)]);
		let json_null_fields = HashSet::from(["payload".to_string()]);

		// Act
		let (_, params) = build_update_statement_with_pk_value(
			"records",
			"id",
			Value::Int(Some(1)),
			&data,
			&json_null_fields,
			DatabaseBackend::Postgres,
		)
		.expect("build explicit JSON-null update");

		// Assert
		assert_eq!(
			params.first(),
			Some(&QueryValue::Json(Some(Box::new(serde_json::Value::Null))))
		);
	}

	#[rstest]
	#[tokio::test]
	async fn batch_callback_error_rolls_back_updates_and_callback_writes() {
		// Arrange
		let (_lease, connection, db) = batch_sqlite_database(&[(1, "before")]).await;
		let mutation = AdminBatchMutation::new(
			"1".to_string(),
			HashMap::from([("name".to_string(), serde_json::json!("after"))]),
		);
		let audit_insert = Query::insert()
			.into_table(Alias::new("batch_audit"))
			.columns([Alias::new("object_id")])
			.values_panic([Expr::val("1")])
			.to_string(SqliteQueryBuilder);
		assert_eq!(mutation.object_id(), "1");
		assert_eq!(mutation.changed_fields(), &["name".to_string()]);

		// Act
		let result = db
			.update_batch_with(
				"batch_records",
				"id",
				vec![mutation],
				async move |transaction| {
					OrmExecutor::execute(transaction, &audit_insert, vec![])
						.await
						.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
					Err(AdminError::DatabaseError("reject callback".to_string()))
				},
			)
			.await;

		// Assert
		assert!(matches!(result, Err(AdminBatchAtomicError::Admin(_))));
		let records_query = Query::select()
			.column(Alias::new("name"))
			.from(Alias::new("batch_records"))
			.to_string(SqliteQueryBuilder);
		let records = connection
			.query(&records_query, vec![])
			.await
			.expect("query records after rollback");
		assert_eq!(records.len(), 1);
		assert_eq!(
			records[0].data.get("name"),
			Some(&serde_json::json!("before"))
		);
		let audit_query = Query::select()
			.column(Alias::new("object_id"))
			.from(Alias::new("batch_audit"))
			.to_string(SqliteQueryBuilder);
		let audit_rows = connection
			.query(&audit_query, vec![])
			.await
			.expect("query audit after rollback");
		assert_eq!(audit_rows.len(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn batch_callback_observes_updates_and_commits_audit_rows() {
		// Arrange
		let (_lease, connection, db) =
			batch_sqlite_database(&[(1, "first before"), (2, "second before")]).await;
		let mutations = vec![
			AdminBatchMutation::new(
				"1".to_string(),
				HashMap::from([("name".to_string(), serde_json::json!("first after"))]),
			),
			AdminBatchMutation::new(
				"2".to_string(),
				HashMap::from([("name".to_string(), serde_json::json!("second after"))]),
			),
		];
		let updated_query = Query::select()
			.columns([Alias::new("id"), Alias::new("name")])
			.from(Alias::new("batch_records"))
			.order_by(Alias::new("id"), Order::Asc)
			.to_string(SqliteQueryBuilder);
		assert_eq!(mutations.len(), 2);
		assert_eq!(mutations[0].object_id(), "1");
		assert_eq!(mutations[1].object_id(), "2");
		assert_eq!(mutations[0].changed_fields(), &["name".to_string()]);
		assert_eq!(mutations[1].changed_fields(), &["name".to_string()]);
		let audit_ids = mutations
			.iter()
			.map(|mutation| mutation.object_id().to_owned())
			.collect::<Vec<_>>();

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
		// Act
		let updated = db
			.update_batch_with("batch_records", "id", mutations, async move |transaction| {
				let rows = OrmExecutor::fetch_all(transaction, &updated_query, vec![])
					.await
					.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
				assert_eq!(rows.len(), 2);
				assert_eq!(
					rows[0].data.get("name"),
					Some(&QueryValue::String("first after".to_string()))
				);
				assert_eq!(
					rows[1].data.get("name"),
					Some(&QueryValue::String("second after".to_string()))
				);

				let mut audit_insert = Query::insert();
				audit_insert
					.into_table(Alias::new("batch_audit"))
					.columns([Alias::new("object_id")]);
				for object_id in audit_ids {
					audit_insert.values_panic([Expr::val(object_id)]);
				}
				let audit_insert = audit_insert.to_string(SqliteQueryBuilder);
				OrmExecutor::execute(transaction, &audit_insert, vec![])
					.await
					.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
				Ok(())
			})
			.await
			.expect("commit updates and callback audit rows");

		// Assert
		assert_eq!(updated, 2);
		let records = connection
			.query(
				&Query::select()
					.columns([Alias::new("id"), Alias::new("name")])
					.from(Alias::new("batch_records"))
					.order_by(Alias::new("id"), Order::Asc)
					.to_string(SqliteQueryBuilder),
				vec![],
			)
			.await
			.expect("query committed records");
		assert_eq!(records.len(), 2);
		assert_eq!(
			records[0].data.get("name"),
			Some(&serde_json::json!("first after"))
		);
		assert_eq!(
			records[1].data.get("name"),
			Some(&serde_json::json!("second after"))
		);
		let audit_rows = connection
			.query(
				&Query::select()
					.column(Alias::new("object_id"))
					.from(Alias::new("batch_audit"))
					.order_by(Alias::new("object_id"), Order::Asc)
					.to_string(SqliteQueryBuilder),
				vec![],
			)
			.await
			.expect("query committed audit rows");
		assert_eq!(audit_rows.len(), 2);
		assert_eq!(
			audit_rows[0].data.get("object_id"),
			Some(&serde_json::json!("1"))
		);
		assert_eq!(
			audit_rows[1].data.get("object_id"),
			Some(&serde_json::json!("2"))
		);
	}

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

	#[rstest]
	#[case(FilterOperator::IsNull, "IS NULL")]
	#[case(FilterOperator::IsNotNull, "IS NOT NULL")]
	fn explicit_null_operators_render_without_values(
		#[case] operator: FilterOperator,
		#[case] expected: &str,
	) {
		let filter = Filter::new("deleted_at", operator, FilterValue::Null);

		assert_eq!(
			render_admin_filter(&filter),
			format!("SELECT \"id\" FROM \"records\" WHERE \"deleted_at\" {expected}")
		);
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
		// The `created_at` field is never constructed: this struct only serves as a
		// phantom type parameter for `FieldRef` below, documenting the field's shape.
		#[allow(
			dead_code,
			reason = "phantom type parameter for FieldRef, never constructed"
		)]
		struct TransformedFilterModel {
			created_at: i64,
		}

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
	fn test_build_composite_and_mixed_valid_and_unsupported_returns_none() {
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
		assert!(result.unwrap().is_none());
	}

	#[rstest]
	fn test_build_composite_or_mixed_valid_and_unsupported_returns_none() {
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
		assert!(result.unwrap().is_none());
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
		let query = Query::select()
			.from(Alias::new("table"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.expect("empty IN should be FALSE")))
			.to_string(PostgresQueryBuilder);
		assert_eq!(query, r#"SELECT * FROM "table" WHERE FALSE"#);
	}

	#[rstest]
	#[case::array(FilterValue::Array(vec![]))]
	#[case::list(FilterValue::List(vec![]))]
	fn test_build_single_filter_expr_empty_not_in_is_noop(#[case] value: FilterValue) {
		// Arrange
		let filter = Filter::new("status", FilterOperator::NotIn, value);

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert: TRUE is the no-op identity for an AND condition.
		let query = Query::select()
			.from(Alias::new("table"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.expect("empty NOT IN should be TRUE")))
			.to_string(PostgresQueryBuilder);
		assert_eq!(query, r#"SELECT * FROM "table" WHERE TRUE"#);
	}

	#[test]
	fn test_build_single_filter_expr_empty_list_in_is_false() {
		// Arrange
		let filter = Filter::new("status", FilterOperator::In, FilterValue::List(vec![]));

		// Act
		let result = build_single_filter_expr(&filter).expect("filter should compile");

		// Assert
		let query = Query::select()
			.from(Alias::new("table"))
			.column(ColumnRef::Asterisk)
			.cond_where(Condition::all().add(result.expect("empty IN should be FALSE")))
			.to_string(PostgresQueryBuilder);
		assert_eq!(query, r#"SELECT * FROM "table" WHERE FALSE"#);
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
	fn test_and_with_mix_supported_unsupported_returns_none() {
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

		// Assert: Reject the composite condition instead of dropping a child.
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(cond.is_none());
	}

	#[rstest]
	fn test_or_with_one_supported_one_unsupported_returns_none() {
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

		// Assert: Reject the composite condition instead of dropping a child.
		assert!(result.is_ok());
		let cond = result.unwrap();
		assert!(cond.is_none());
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

	fn register_pk_metadata(
		app_label: &str,
		model_name: &str,
		table_name: &str,
		field_type: DbFieldType,
	) {
		let mut metadata = ModelMetadata::new(app_label, model_name, table_name);
		metadata.fields.insert(
			"id".to_string(),
			FieldMetadata::new(field_type).with_param("primary_key", "true"),
		);
		global_registry().register_model(metadata);
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn parse_pk_value_registered_text_preserves_leading_zeroes() {
		// Arrange
		register_pk_metadata(
			"admin_pk_parser_text",
			"AdminPkParserText",
			"admin_pk_parser_text_records",
			DbFieldType::VarChar(32),
		);

		// Act
		let value = parse_pk_value("admin_pk_parser_text_records", "id", "001")
			.expect("registered text primary key should parse");

		// Assert
		assert_eq!(value, Value::String(Some(Box::new("001".to_string()))));
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn parse_pk_value_without_metadata_uses_canonical_numeric_identity() {
		// Arrange: no registry entry exists for the table.

		// Act
		let value = parse_pk_value("admin_pk_parser_missing_numeric_records", "id", "1")
			.expect("metadata-free canonical numeric primary key should parse");

		// Assert
		assert_eq!(value, Value::BigInt(Some(1)));
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn parse_pk_value_registered_timestamp_uses_target_metadata() {
		// Arrange
		let mut metadata = ModelMetadata::new(
			"admin_pk_parser_timestamp",
			"AdminPkParserTimestamp",
			"admin_pk_parser_timestamp_records",
		);
		metadata.fields.insert(
			"created_on".to_string(),
			FieldMetadata::new(DbFieldType::TimestampTz)
				.with_param("rust_field_name", "created_at")
				.with_param("db_column", "created_on")
				.with_param("primary_key", "true"),
		);
		global_registry().register_model(metadata);
		let expected = chrono::DateTime::parse_from_rfc3339("2026-01-01T12:34:56.123Z")
			.expect("fixture timestamp should parse")
			.with_timezone(&chrono::Utc);

		// Act
		let value = parse_pk_value(
			"admin_pk_parser_timestamp_records",
			"created_at",
			"2026-01-01T12:34:56.123Z",
		)
		.expect("registered timestamp primary key should parse");

		// Assert
		assert_eq!(value, Value::ChronoDateTimeUtc(Some(Box::new(expected))));
	}

	#[rstest]
	fn registered_text_values_do_not_use_date_or_uuid_heuristics() {
		let date_like =
			string_value_for_field("2026-01-01".to_string(), Some(&DbFieldType::VarChar(64)));
		let uuid_like = string_value_for_field(
			"550e8400-e29b-41d4-a716-446655440000".to_string(),
			Some(&DbFieldType::Text),
		);

		assert_eq!(
			date_like,
			Value::String(Some(Box::new("2026-01-01".to_string())))
		);
		assert_eq!(
			uuid_like,
			Value::String(Some(Box::new(
				"550e8400-e29b-41d4-a716-446655440000".to_string()
			)))
		);
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn parse_pk_value_without_metadata_preserves_string_identity() {
		// Arrange: no registry entry exists for the table.

		// Act
		let value = parse_pk_value("admin_pk_parser_missing_records", "id", "001")
			.expect("metadata-free primary key should preserve the submitted string");

		// Assert
		assert_eq!(value, Value::String(Some(Box::new("001".to_string()))));
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn parse_pk_value_registered_decimal_retains_numeric_compatibility_fallback() {
		// Arrange
		register_pk_metadata(
			"admin_pk_parser_decimal",
			"AdminPkParserDecimal",
			"admin_pk_parser_decimal_records",
			DbFieldType::Decimal {
				precision: 10,
				scale: 2,
			},
		);

		// Act
		let value = parse_pk_value("admin_pk_parser_decimal_records", "id", "001")
			.expect("registered decimal should retain the compatibility fallback");

		// Assert
		assert_eq!(value, Value::BigInt(Some(1)));
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn parse_pk_value_rejects_malformed_registered_uuid() {
		// Arrange
		register_pk_metadata(
			"admin_pk_parser_uuid",
			"AdminPkParserUuid",
			"admin_pk_parser_uuid_records",
			DbFieldType::Uuid,
		);

		// Act
		let error = parse_pk_value("admin_pk_parser_uuid_records", "id", "not-a-uuid")
			.expect_err("malformed registered UUID primary key must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Invalid UUID primary key value 'not-a-uuid' for field 'id'"
		);
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn parse_pk_value_rejects_malformed_registered_integer() {
		// Arrange
		register_pk_metadata(
			"admin_pk_parser_integer",
			"AdminPkParserInteger",
			"admin_pk_parser_integer_records",
			DbFieldType::Integer,
		);

		// Act
		let error = parse_pk_value("admin_pk_parser_integer_records", "id", "12x")
			.expect_err("malformed registered integer primary key must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Invalid integer primary key value '12x' for field 'id'"
		);
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn test_parse_pk_value_uuid_string_without_registry_falls_back_to_string() {
		// Arrange: No registry entry, UUID string input

		// Act
		let value = parse_pk_value(
			"nonexistent_table",
			"id",
			"c1a363b1-cc42-4dea-81f0-9dc1cedf0083",
		)
		.expect("metadata-free UUID-shaped value should use the compatibility fallback");

		// Assert: Without registry metadata, UUID falls back to Value::String
		assert_eq!(
			value,
			Value::String(Some(Box::new(
				"c1a363b1-cc42-4dea-81f0-9dc1cedf0083".to_string()
			)))
		);
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn test_parse_pk_value_non_numeric_string_falls_back_to_string() {
		// Arrange: No registry entry, non-numeric string input

		// Act
		let value = parse_pk_value("nonexistent_table", "id", "hello-world")
			.expect("metadata-free string should use the compatibility fallback");

		// Assert
		assert_eq!(
			value,
			Value::String(Some(Box::new("hello-world".to_string())))
		);
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn test_parse_pk_value_without_metadata_uses_negative_integer() {
		// Arrange: Negative integer string without registry metadata

		// Act
		let value = parse_pk_value("nonexistent_table", "id", "-1")
			.expect("metadata-free signed primary key should parse");

		// Assert
		assert_eq!(value, Value::BigInt(Some(-1)));
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn test_parse_pk_value_without_metadata_uses_zero_integer() {
		// Arrange: Zero as string without registry metadata

		// Act
		let value = parse_pk_value("nonexistent_table", "id", "0")
			.expect("metadata-free signed primary key should parse");

		// Assert
		assert_eq!(value, Value::BigInt(Some(0)));
	}

	#[rstest]
	#[serial(admin_pk_parser)]
	fn test_parse_pk_value_without_metadata_uses_signed_integer_minimum() {
		// Arrange: Minimum signed integer without registry metadata

		// Act
		let value = parse_pk_value("nonexistent_table", "id", &i64::MIN.to_string())
			.expect("metadata-free signed primary key should parse");

		// Assert
		assert_eq!(value, Value::BigInt(Some(i64::MIN)));
	}

	#[rstest]
	fn canonical_pk_value_preserves_leading_zero_string_identity() {
		assert_eq!(canonicalize_pk_value("nonexistent_table", "id", "01"), "01");
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
			presence_column: "external_key".to_string(),
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
			"SELECT \"articles\".*, COUNT(*) OVER() AS \"__reinhardt_total_count\", \"__reinhardt_related_table_0\".\"external_key\" AS \"__reinhardt_related_0__external_key\", \"__reinhardt_related_table_0\".\"username\" AS \"__reinhardt_related_0__username\" FROM \"articles\" LEFT JOIN \"users\" AS \"__reinhardt_related_table_0\" ON \"articles\".\"author_id\" = \"__reinhardt_related_table_0\".\"external_key\" WHERE (\"articles\".\"owner_id\" = 7 AND (\"articles\".\"status\" = 'published' OR \"articles\".\"status\" = 'review')) ORDER BY \"articles\".\"id\" DESC LIMIT 25 OFFSET 10"
		);
		assert_eq!(
			count_sql,
			"SELECT COUNT(*) AS count FROM \"articles\" LEFT JOIN \"users\" AS \"__reinhardt_related_table_0\" ON \"articles\".\"author_id\" = \"__reinhardt_related_table_0\".\"external_key\" WHERE (\"articles\".\"owner_id\" = 7 AND (\"articles\".\"status\" = 'published' OR \"articles\".\"status\" = 'review'))"
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
			presence_column: "external_key".to_string(),
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
			"SELECT DISTINCT DATE_TRUNC('month', \"articles\".\"published_on\")::date AS \"__reinhardt_date_hierarchy\" FROM \"articles\" LEFT JOIN \"users\" AS \"__reinhardt_related_table_0\" ON \"articles\".\"author_id\" = \"__reinhardt_related_table_0\".\"external_key\" WHERE \"articles\".\"published_on\" IS NOT NULL AND (\"articles\".\"owner_id\" = 7 AND \"articles\".\"published_on\" >= CAST('2024-01-01' AS DATE) AND \"articles\".\"published_on\" < CAST('2025-01-01' AS DATE)) ORDER BY \"__reinhardt_date_hierarchy\" ASC"
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
	fn test_typed_date_filter_normalizes_range_and_membership_values() {
		// Arrange
		let start = FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(
			chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
		)));
		let end = FilterValue::Typed(Ok(reinhardt_db::orm::DatabaseValue::Date(
			chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
		)));
		let range = Filter::new(
			"published_on",
			FilterOperator::Range,
			FilterValue::Range(Box::new(start.clone()), Box::new(end.clone())),
		);
		let membership = Filter::new(
			"published_on",
			FilterOperator::In,
			FilterValue::List(vec![start, end]),
		);

		// Act
		let range_sql = Query::select()
			.from(Alias::new("articles"))
			.column(ColumnRef::Asterisk)
			.cond_where(
				Condition::all().add(
					build_single_filter_expr(&range)
						.expect("range should compile")
						.expect("range should produce an expression"),
				),
			)
			.to_string(PostgresQueryBuilder);
		let membership_sql = Query::select()
			.from(Alias::new("articles"))
			.column(ColumnRef::Asterisk)
			.cond_where(
				Condition::all().add(
					build_single_filter_expr(&membership)
						.expect("membership should compile")
						.expect("membership should produce an expression"),
				),
			)
			.to_string(PostgresQueryBuilder);

		// Assert
		assert_eq!(
			range_sql,
			r#"SELECT * FROM "articles" WHERE "published_on" BETWEEN CAST('2024-01-01' AS DATE) AND CAST('2024-12-31' AS DATE)"#
		);
		assert_eq!(
			membership_sql,
			r#"SELECT * FROM "articles" WHERE "published_on" IN (CAST('2024-01-01' AS DATE), CAST('2024-12-31' AS DATE))"#
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
			presence_column: "external_key".to_string(),
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

	#[test]
	fn test_decode_admin_list_row_uses_hidden_non_null_presence_column() {
		// Arrange
		let related = vec![AdminRelatedField {
			relation_name: "author".to_string(),
			source_column: "author_id".to_string(),
			target_table: "users".to_string(),
			target_column: "external_key".to_string(),
			presence_column: "id".to_string(),
			columns: vec!["display_name".to_string()],
		}];
		let row = serde_json::json!({
			"id": 11,
			"__reinhardt_total_count": 1,
			"__reinhardt_related_0__id": 7,
			"__reinhardt_related_0__display_name": null
		});
		let serde_json::Value::Object(row) = row else {
			panic!("test row must be an object");
		};

		// Act
		let (decoded, count) = decode_admin_list_row(row, &related).unwrap();

		// Assert: the joined row survives even though its visible column is NULL,
		// and the non-null presence sentinel is not exposed in the response.
		assert_eq!(count, 1);
		assert_eq!(
			serde_json::to_value(decoded).unwrap(),
			serde_json::json!({
				"id": 11,
				"author": {"display_name": null}
			})
		);
	}

	#[test]
	fn test_admin_list_statement_selects_hidden_related_presence_column() {
		// Arrange
		let query = crate::core::AdminQuery::new("articles");
		let related = vec![AdminRelatedField {
			relation_name: "author".to_string(),
			source_column: "author_id".to_string(),
			target_table: "users".to_string(),
			target_column: "external_key".to_string(),
			presence_column: "id".to_string(),
			columns: vec!["display_name".to_string()],
		}];

		// Act
		let sql = build_admin_list_statement(&query, &related, None, 0, 25)
			.unwrap()
			.to_string(PostgresQueryBuilder);

		// Assert
		assert_eq!(
			sql,
			r#"SELECT "articles".*, COUNT(*) OVER() AS "__reinhardt_total_count", "__reinhardt_related_table_0"."display_name" AS "__reinhardt_related_0__display_name", "__reinhardt_related_table_0"."id" AS "__reinhardt_related_0__id" FROM "articles" LEFT JOIN "users" AS "__reinhardt_related_table_0" ON "articles"."author_id" = "__reinhardt_related_table_0"."external_key" LIMIT 25 OFFSET 0"#
		);
	}

	#[rstest]
	fn related_column_aliases_fall_back_to_bounded_indexes() {
		let long_column = "a_column_name_that_is_longer_than_the_postgresql_identifier_limit";

		assert_eq!(
			related_column_alias(3, 7, "username"),
			"__reinhardt_related_3__username"
		);
		assert_eq!(
			related_column_alias(3, 7, long_column),
			"__reinhardt_related_3_7"
		);
		assert_ne!(
			related_column_alias(3, 7, long_column),
			related_column_alias(3, 8, long_column)
		);
		assert_ne!(related_table_alias(0), related_table_alias(1));
		assert_ne!(related_table_alias(0), "articles");
		assert!(related_table_alias(0).len() <= 63);
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
				sql.contains("LEFT JOIN \"users\" AS \"__reinhardt_related_table_0\"")
					&& sql.contains(
						"\"articles\".\"author_id\" = \"__reinhardt_related_table_0\".\"external_key\"",
					) && sql.contains("AS \"__reinhardt_related_0__username\"")
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
			presence_column: "external_key".to_string(),
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
