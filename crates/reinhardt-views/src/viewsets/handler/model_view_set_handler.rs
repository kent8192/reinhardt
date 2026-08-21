//! `ModelViewSetHandler` — Django REST Framework-style CRUD handler.
//!
//! Provides the standard list/retrieve/create/update/destroy actions with
//! permission checks, optional pagination, and serialization for `Model`
//! types. The response rendering for each action lives next to the action
//! itself in this module.

use super::error::ViewError;
use reinhardt_auth::{Permission, PermissionContext};
use reinhardt_db::orm::model::filter_value_from_field_type;
use reinhardt_db::orm::{
	CustomManager, Filter, FilterCondition, FilterOperator, FilterValue, Model, QuerySet,
	query_types::DbBackend,
};
use reinhardt_http::{AuthState, Request, Response};
use reinhardt_rest::filters::FilterBackend;
use reinhardt_rest::serializers::{ModelSerializer, Serializer, SerializerError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

type QuerysetFn =
	dyn Fn(&Request) -> std::result::Result<FilterCondition, ViewError> + Send + Sync + 'static;

fn map_scope_field<T: Model>(field_name: &mut String) {
	if let Some((prefix, name)) = field_name.rsplit_once('.') {
		let mut mapped_name = name.to_owned();
		map_scope_field::<T>(&mut mapped_name);
		*field_name = format!("{prefix}.{mapped_name}");
		return;
	}

	if let Some(field) = T::field_metadata()
		.into_iter()
		.find(|field| field.name == *field_name)
	{
		*field_name = field.db_column_name().to_owned();
	}
}

fn map_scope_subquery_field<T: Model>(field_name: &mut String) {
	map_scope_field::<T>(field_name);
}

fn map_scope_expression_sql<T: Model>(sql: &str) -> String {
	let fields = T::field_metadata();
	let bytes = sql.as_bytes();
	let mut mapped = String::with_capacity(sql.len());
	let mut index = 0;
	while index < bytes.len() {
		let quote = bytes[index] as char;
		if matches!(quote, '"' | '`') {
			let identifier_start = index + 1;
			let mut cursor = identifier_start;
			while cursor < bytes.len() {
				if bytes[cursor] as char == quote {
					if bytes.get(cursor + 1).map(|byte| *byte as char) == Some(quote) {
						cursor += 2;
						continue;
					}
					break;
				}
				cursor += 1;
			}
			if cursor < bytes.len() {
				let identifier = &sql[identifier_start..cursor];
				let replacement = fields
					.iter()
					.find(|field| field.name == identifier)
					.map(|field| field.db_column_name())
					.unwrap_or(identifier);
				mapped.push(quote);
				mapped.push_str(replacement);
				mapped.push(quote);
				index = cursor + 1;
				continue;
			}
		}

		let character = sql[index..]
			.chars()
			.next()
			.expect("index is within the expression");
		mapped.push(character);
		index += character.len_utf8();
	}
	mapped
}

const SQL_IDENTIFIER_KEYWORDS: &[&str] = &[
	"AND", "OR", "NOT", "NULL", "TRUE", "FALSE", "IN", "IS", "LIKE", "BETWEEN", "JOIN", "ON", "AS",
	"SELECT", "FROM", "WHERE", "INNER", "LEFT", "RIGHT", "OUTER", "CROSS", "FULL", "EXISTS",
	"CASE", "WHEN", "THEN", "ELSE", "END", "ASC", "DESC", "NULLS", "FIRST", "LAST",
];

fn sql_identifiers(sql: &str) -> Vec<String> {
	let bytes = sql.as_bytes();
	let mut identifiers = Vec::new();
	let mut index = 0;
	while index < bytes.len() {
		let quote = bytes[index] as char;
		if matches!(quote, '"' | '`' | '\'') {
			let identifier_start = index + 1;
			let mut cursor = identifier_start;
			while cursor < bytes.len() {
				if bytes[cursor] as char == quote {
					if bytes.get(cursor + 1).map(|byte| *byte as char) == Some(quote) {
						cursor += 2;
						continue;
					}
					break;
				}
				cursor += 1;
			}
			if cursor < bytes.len() && quote != '\'' {
				identifiers.push(
					sql[identifier_start..cursor]
						.replace(&format!("{quote}{quote}"), &quote.to_string()),
				);
			}
			index = cursor.saturating_add(1);
			continue;
		}

		let character = sql[index..]
			.chars()
			.next()
			.expect("index is within the expression");
		if character.is_ascii_alphabetic() || character == '_' {
			let start = index;
			index += character.len_utf8();
			while index < bytes.len() {
				let next = sql[index..]
					.chars()
					.next()
					.expect("index is within the expression");
				if next.is_ascii_alphanumeric() || next == '_' || next == '.' {
					index += next.len_utf8();
				} else {
					break;
				}
			}
			let token = &sql[start..index];
			if !SQL_IDENTIFIER_KEYWORDS
				.iter()
				.any(|keyword| token.eq_ignore_ascii_case(keyword))
			{
				identifiers.push(token.to_owned());
			}
			continue;
		}

		index += character.len_utf8();
	}
	identifiers
}

fn collect_scope_sql_fields<T: Model>(sql: &str, fields: &mut Vec<String>) {
	let metadata = T::field_metadata();
	for identifier in sql_identifiers(sql) {
		let name = identifier.rsplit('.').next().unwrap_or(identifier.as_str());
		if let Some(field) = metadata
			.iter()
			.find(|field| field.name == name || field.db_column_name() == name)
		{
			fields.push(field.name.clone());
		}
	}
}

fn join_condition_is_opaque(condition: &str) -> bool {
	let trimmed = condition.trim();
	trimmed.is_empty() || trimmed.eq_ignore_ascii_case("true") || trimmed == "1"
}

fn map_scope_query_condition<T: Model>(condition: &mut reinhardt_db::orm::expressions::Q) {
	use reinhardt_db::orm::expressions::Q;

	match condition {
		Q::Condition { field, .. } => map_scope_field::<T>(field),
		Q::Combined { conditions, .. } => {
			for condition in conditions {
				map_scope_query_condition::<T>(condition);
			}
		}
	}
}

fn map_scope_annotation_value<T: Model>(
	value: &mut reinhardt_db::orm::annotation::AnnotationValue,
) {
	use reinhardt_db::orm::annotation::AnnotationValue;

	match value {
		AnnotationValue::Field(field) => map_scope_field::<T>(&mut field.field),
		AnnotationValue::Aggregate(aggregate) => {
			if let Some(field) = &mut aggregate.field {
				map_scope_field::<T>(field);
			}
		}
		AnnotationValue::Expression(expression) => map_scope_annotation_expression::<T>(expression),
		AnnotationValue::ArrayAgg(value) => value.map_fields(map_scope_order_by_field::<T>),
		AnnotationValue::StringAgg(value) => value.map_fields(map_scope_order_by_field::<T>),
		AnnotationValue::JsonbAgg(value) => value.map_fields(map_scope_order_by_field::<T>),
		AnnotationValue::JsonbBuildObject(value) => {
			value.map_fields(map_scope_field::<T>);
		}
		AnnotationValue::TsRank(value) => value.map_fields(map_scope_field::<T>),
		AnnotationValue::Value(_) | AnnotationValue::Subquery(_) => {}
	}
}

fn map_scope_annotation_expression<T: Model>(
	expression: &mut reinhardt_db::orm::annotation::Expression,
) {
	use reinhardt_db::orm::annotation::Expression;

	match expression {
		Expression::Add(left, right)
		| Expression::Subtract(left, right)
		| Expression::Multiply(left, right)
		| Expression::Divide(left, right) => {
			map_scope_annotation_value::<T>(left);
			map_scope_annotation_value::<T>(right);
		}
		Expression::Case { whens, default } => {
			for when in whens {
				map_scope_query_condition::<T>(&mut when.condition);
				map_scope_annotation_value::<T>(&mut when.then);
			}
			if let Some(default) = default {
				map_scope_annotation_value::<T>(default);
			}
		}
		Expression::Coalesce(values) => {
			for value in values {
				map_scope_annotation_value::<T>(value);
			}
		}
	}
}

fn map_scope_filter_value<T: Model>(value: &mut FilterValue) {
	match value {
		FilterValue::FieldRef(field) => map_scope_field::<T>(&mut field.field),
		FilterValue::OuterRef(field) => map_scope_field::<T>(&mut field.field),
		FilterValue::Expression(expression) => map_scope_annotation_expression::<T>(expression),
		FilterValue::List(values) => {
			for value in values {
				map_scope_filter_value::<T>(value);
			}
		}
		FilterValue::Range(start, end) => {
			map_scope_filter_value::<T>(start);
			map_scope_filter_value::<T>(end);
		}
		FilterValue::String(_)
		| FilterValue::Timestamp(_)
		| FilterValue::Date(_)
		| FilterValue::Time(_)
		| FilterValue::NaiveDateTime(_)
		| FilterValue::Decimal(_)
		| FilterValue::Uuid(_)
		| FilterValue::Integer(_)
		| FilterValue::Int(_)
		| FilterValue::Float(_)
		| FilterValue::Boolean(_)
		| FilterValue::Bool(_)
		| FilterValue::Null
		| FilterValue::Array(_) => {}
	}
}

fn map_scope_filter_column<T: Model>(filter: &mut Filter) {
	filter.map_expression_source(map_scope_expression_sql::<T>);
	map_scope_field::<T>(&mut filter.field);
	map_scope_filter_value::<T>(&mut filter.value);
}

fn map_scope_filter_columns<T: Model>(condition: &mut FilterCondition) {
	match condition {
		FilterCondition::Single(filter) => map_scope_filter_column::<T>(filter),
		FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
			for condition in conditions {
				map_scope_filter_columns::<T>(condition);
			}
		}
		FilterCondition::Not(condition) => map_scope_filter_columns::<T>(condition),
	}
}

fn scope_annotation_value_contains_opaque_subquery(
	value: &reinhardt_db::orm::annotation::AnnotationValue,
) -> bool {
	use reinhardt_db::orm::annotation::AnnotationValue;

	match value {
		AnnotationValue::Subquery(_) => true,
		AnnotationValue::Expression(expression) => {
			scope_annotation_expression_contains_opaque_subquery(expression)
		}
		AnnotationValue::Value(_)
		| AnnotationValue::Field(_)
		| AnnotationValue::Aggregate(_)
		| AnnotationValue::ArrayAgg(_)
		| AnnotationValue::StringAgg(_)
		| AnnotationValue::JsonbAgg(_)
		| AnnotationValue::JsonbBuildObject(_)
		| AnnotationValue::TsRank(_) => false,
	}
}

fn scope_annotation_expression_contains_opaque_subquery(
	expression: &reinhardt_db::orm::annotation::Expression,
) -> bool {
	use reinhardt_db::orm::annotation::Expression;

	match expression {
		Expression::Add(left, right)
		| Expression::Subtract(left, right)
		| Expression::Multiply(left, right)
		| Expression::Divide(left, right) => {
			scope_annotation_value_contains_opaque_subquery(left)
				|| scope_annotation_value_contains_opaque_subquery(right)
		}
		Expression::Case { whens, default } => {
			whens
				.iter()
				.any(|when| scope_annotation_value_contains_opaque_subquery(&when.then))
				|| default
					.as_deref()
					.is_some_and(scope_annotation_value_contains_opaque_subquery)
		}
		Expression::Coalesce(values) => values
			.iter()
			.any(scope_annotation_value_contains_opaque_subquery),
	}
}

fn scope_filter_value_contains_opaque_subquery(value: &FilterValue) -> bool {
	match value {
		FilterValue::Expression(expression) => {
			scope_annotation_expression_contains_opaque_subquery(expression)
		}
		FilterValue::List(values) => values
			.iter()
			.any(scope_filter_value_contains_opaque_subquery),
		FilterValue::Range(start, end) => {
			scope_filter_value_contains_opaque_subquery(start)
				|| scope_filter_value_contains_opaque_subquery(end)
		}
		_ => false,
	}
}

fn scope_filter_condition_contains_opaque_subquery(condition: &FilterCondition) -> bool {
	match condition {
		FilterCondition::Single(filter) => {
			scope_filter_value_contains_opaque_subquery(&filter.value)
		}
		FilterCondition::And(conditions) | FilterCondition::Or(conditions) => conditions
			.iter()
			.any(scope_filter_condition_contains_opaque_subquery),
		FilterCondition::Not(condition) => {
			scope_filter_condition_contains_opaque_subquery(condition)
		}
	}
}

fn collect_scope_annotation_value(
	value: &reinhardt_db::orm::annotation::AnnotationValue,
	fields: &mut Vec<String>,
) {
	use reinhardt_db::orm::annotation::{AnnotationValue, Expression};

	match value {
		AnnotationValue::Field(field) => fields.push(field.field.clone()),
		AnnotationValue::Aggregate(aggregate) => {
			if let Some(field) = &aggregate.field {
				fields.push(field.clone());
			}
		}
		AnnotationValue::Expression(expression) => match expression {
			Expression::Add(left, right)
			| Expression::Subtract(left, right)
			| Expression::Multiply(left, right)
			| Expression::Divide(left, right) => {
				collect_scope_annotation_value(left, fields);
				collect_scope_annotation_value(right, fields);
			}
			Expression::Case { whens, default } => {
				for when in whens {
					collect_scope_query_condition(&when.condition, fields);
					collect_scope_annotation_value(&when.then, fields);
				}
				if let Some(default) = default {
					collect_scope_annotation_value(default, fields);
				}
			}
			Expression::Coalesce(values) => {
				for value in values {
					collect_scope_annotation_value(value, fields);
				}
			}
		},
		AnnotationValue::ArrayAgg(value) => {
			let mut value = value.clone();
			value.map_fields(|field| collect_scope_order_by_field(field, fields));
		}
		AnnotationValue::StringAgg(value) => {
			let mut value = value.clone();
			value.map_fields(|field| collect_scope_order_by_field(field, fields));
		}
		AnnotationValue::JsonbAgg(value) => {
			let mut value = value.clone();
			value.map_fields(|field| collect_scope_order_by_field(field, fields));
		}
		AnnotationValue::JsonbBuildObject(value) => {
			let mut value = value.clone();
			value.map_fields(|field| fields.push(field.clone()));
		}
		AnnotationValue::TsRank(value) => {
			let mut value = value.clone();
			value.map_fields(|field| fields.push(field.clone()));
		}
		AnnotationValue::Value(_) | AnnotationValue::Subquery(_) => {}
	}
}

fn collect_scope_query_condition(
	condition: &reinhardt_db::orm::expressions::Q,
	fields: &mut Vec<String>,
) {
	use reinhardt_db::orm::expressions::Q;

	match condition {
		Q::Condition { field, .. } => fields.push(field.clone()),
		Q::Combined { conditions, .. } => {
			for condition in conditions {
				collect_scope_query_condition(condition, fields);
			}
		}
	}
}

fn collect_scope_filter_value(value: &FilterValue, fields: &mut Vec<String>) {
	match value {
		FilterValue::FieldRef(field) => fields.push(field.field.clone()),
		FilterValue::OuterRef(field) => fields.push(field.field.clone()),
		FilterValue::Expression(expression) => {
			collect_scope_annotation_expression(expression, fields);
		}
		FilterValue::List(values) => {
			for value in values {
				collect_scope_filter_value(value, fields);
			}
		}
		FilterValue::Range(start, end) => {
			collect_scope_filter_value(start, fields);
			collect_scope_filter_value(end, fields);
		}
		FilterValue::String(_)
		| FilterValue::Timestamp(_)
		| FilterValue::Date(_)
		| FilterValue::Time(_)
		| FilterValue::NaiveDateTime(_)
		| FilterValue::Decimal(_)
		| FilterValue::Uuid(_)
		| FilterValue::Integer(_)
		| FilterValue::Int(_)
		| FilterValue::Float(_)
		| FilterValue::Boolean(_)
		| FilterValue::Bool(_)
		| FilterValue::Null
		| FilterValue::Array(_) => {}
	}
}

fn collect_scope_annotation_expression(
	expression: &reinhardt_db::orm::annotation::Expression,
	fields: &mut Vec<String>,
) {
	use reinhardt_db::orm::annotation::Expression;

	match expression {
		Expression::Add(left, right)
		| Expression::Subtract(left, right)
		| Expression::Multiply(left, right)
		| Expression::Divide(left, right) => {
			collect_scope_annotation_value(left, fields);
			collect_scope_annotation_value(right, fields);
		}
		Expression::Case { whens, default } => {
			for when in whens {
				collect_scope_query_condition(&when.condition, fields);
				collect_scope_annotation_value(&when.then, fields);
			}
			if let Some(default) = default {
				collect_scope_annotation_value(default, fields);
			}
		}
		Expression::Coalesce(values) => {
			for value in values {
				collect_scope_annotation_value(value, fields);
			}
		}
	}
}

fn collect_scope_filter_condition(condition: &FilterCondition, fields: &mut Vec<String>) {
	match condition {
		FilterCondition::Single(filter) => {
			fields.push(
				filter
					.source_field_name()
					.unwrap_or(&filter.field)
					.to_owned(),
			);
			collect_scope_filter_value(&filter.value, fields);
		}
		FilterCondition::And(conditions) | FilterCondition::Or(conditions) => {
			for condition in conditions {
				collect_scope_filter_condition(condition, fields);
			}
		}
		FilterCondition::Not(condition) => collect_scope_filter_condition(condition, fields),
	}
}

fn serialized_scope_field<'a>(
	value: &'a serde_json::Value,
	field: &reinhardt_db::orm::inspection::FieldInfo,
) -> Option<&'a serde_json::Value> {
	value
		.get(&field.name)
		.or_else(|| value.get(field.db_column_name()))
}

fn map_scope_order_by_field<T: Model>(field_name: &mut String) {
	let descending = field_name.starts_with('-');
	let order_field = field_name
		.strip_prefix('-')
		.unwrap_or(field_name)
		.to_owned();
	let Some(separator) = order_field.find(|character: char| character.is_whitespace()) else {
		map_scope_order_by_name::<T>(
			field_name,
			if descending { "-" } else { "" },
			&order_field,
			"",
		);
		return;
	};
	let (logical_name, suffix) = order_field.split_at(separator);
	map_scope_order_by_name::<T>(
		field_name,
		if descending { "-" } else { "" },
		logical_name,
		suffix,
	);
}

fn map_scope_order_by_name<T: Model>(
	field_name: &mut String,
	prefix: &str,
	qualified_name: &str,
	suffix: &str,
) {
	let (qualifier, logical_name) = qualified_name
		.rsplit_once('.')
		.map_or(("", qualified_name), |(qualifier, name)| (qualifier, name));
	let Some(field) = T::field_metadata()
		.into_iter()
		.find(|field| field.name == logical_name)
	else {
		return;
	};
	let physical_name = field.db_column_name();
	let mapped_name = if qualifier.is_empty() {
		physical_name.to_owned()
	} else {
		format!("{qualifier}.{physical_name}")
	};
	*field_name = format!("{prefix}{mapped_name}{suffix}");
}

fn collect_scope_order_by_field(field_name: &str, fields: &mut Vec<String>) {
	let field_name = field_name.strip_prefix('-').unwrap_or(field_name);
	let field_name = field_name
		.split_once(|character: char| character.is_whitespace())
		.map_or(field_name, |(field, _)| field);
	fields.push(field_name.to_owned());
}

fn parse_length_prefixed_composite_parts<'a>(
	inner: &'a str,
	fields: &[String],
) -> Option<Vec<&'a str>> {
	if fields.is_empty() {
		return None;
	}

	let mut cursor = inner;
	let mut parts = Vec::with_capacity(fields.len());
	for (index, field_name) in fields.iter().enumerate() {
		let value_start = cursor.strip_prefix(&format!("{field_name}="))?;
		let length_separator = value_start.find(':')?;
		let length = value_start[..length_separator].parse::<usize>().ok()?;
		let content_start = length_separator + 1;
		let content_end = content_start.checked_add(length)?;
		let value = value_start.get(content_start..content_end)?;
		let remainder = value_start.get(content_end..)?;

		if index + 1 == fields.len() {
			if !remainder.is_empty() {
				return None;
			}
		} else {
			cursor = remainder.strip_prefix(", ")?;
		}
		parts.push(value);
	}

	Some(parts)
}

fn parse_legacy_composite_parts<'a, F>(
	cursor: &'a str,
	fields: &[String],
	index: usize,
	is_valid_part: &F,
) -> Option<Vec<&'a str>>
where
	F: Fn(usize, &str) -> bool,
{
	let field_name = fields.get(index)?;
	let value_start = cursor.strip_prefix(&format!("{field_name}="))?;
	if index + 1 == fields.len() {
		return is_valid_part(index, value_start).then(|| vec![value_start]);
	}

	let delimiter = format!(", {}=", fields[index + 1]);
	for (position, _) in value_start.match_indices(&delimiter) {
		let part = &value_start[..position];
		if !is_valid_part(index, part) {
			continue;
		}
		let next_cursor = &value_start[position + 2..];
		if let Some(mut tail) =
			parse_legacy_composite_parts(next_cursor, fields, index + 1, is_valid_part)
		{
			tail.insert(0, part);
			return Some(tail);
		}
	}

	None
}

fn primary_key_filter_for_model<T: Model>(
	pk: &serde_json::Value,
) -> std::result::Result<FilterCondition, ViewError> {
	let pk_string = pk
		.as_str()
		.map(str::to_owned)
		.unwrap_or_else(|| pk.to_string());
	let pk_string = urlencoding::decode(&pk_string)
		.map_err(|_| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?
		.into_owned();
	let Some(composite) = T::composite_primary_key() else {
		let value = T::primary_key_filter_value_from_str(&pk_string)
			.map_err(|_| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?;
		let column = T::field_metadata()
			.into_iter()
			.find(|field| field.name == T::primary_key_field())
			.map(|field| field.db_column_name().to_owned())
			.unwrap_or_else(|| T::primary_key_field().to_owned());
		return Ok(Filter::new(column, FilterOperator::Eq, value).into());
	};

	let inner = pk_string
		.strip_prefix('(')
		.and_then(|value| value.strip_suffix(')'))
		.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?;
	let fields = composite.fields();
	let metadata = T::field_metadata();
	let is_valid_part = |index: usize, part: &str| {
		let field_name = &fields[index];
		match metadata.iter().find(|field| field.name == *field_name) {
			Some(field) => filter_value_from_field_type(&field.field_type, part).is_ok(),
			None => true,
		}
	};
	let parts = parse_length_prefixed_composite_parts(inner, fields)
		.or_else(|| parse_legacy_composite_parts(inner, fields, 0, &is_valid_part));
	let parts = parts
		.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk_string)))?;
	let filters = fields
		.iter()
		.zip(parts)
		.map(|(field_name, part)| {
			let field = metadata.iter().find(|field| field.name == *field_name);
			let filter_value = field
				.map(|field| filter_value_from_field_type(&field.field_type, part))
				.transpose()
				.map_err(|_| {
					ViewError::NotFound(format!("Object with pk={} not found", pk_string))
				})?
				.unwrap_or_else(|| FilterValue::String(part.to_owned()));
			let column = field
				.map(|field| field.db_column_name().to_owned())
				.unwrap_or_else(|| field_name.clone());
			Ok(Filter::new(column, FilterOperator::Eq, filter_value))
		})
		.collect::<std::result::Result<Vec<_>, _>>()?;

	Ok(FilterCondition::and(
		filters.into_iter().map(FilterCondition::from).collect(),
	))
}

fn assigned_primary_key_filter<T: Model>(item: &T) -> Option<FilterCondition> {
	let metadata = T::field_metadata();
	if let Some(composite) = T::composite_primary_key() {
		let values = item.get_composite_pk_values();
		let filters = composite
			.fields()
			.iter()
			.map(|field_name| {
				let value = match values.get(field_name)? {
					reinhardt_db::orm::composite_pk::PkValue::String(value) => {
						FilterValue::String(value.clone())
					}
					reinhardt_db::orm::composite_pk::PkValue::Int(value) => {
						FilterValue::Integer(*value)
					}
					reinhardt_db::orm::composite_pk::PkValue::Uint(value) => {
						FilterValue::Integer(i64::try_from(*value).ok()?)
					}
					reinhardt_db::orm::composite_pk::PkValue::Bool(value) => {
						FilterValue::Boolean(*value)
					}
				};
				let column = metadata
					.iter()
					.find(|field| field.name == *field_name)
					.map(|field| field.db_column_name().to_owned())
					.unwrap_or_else(|| field_name.clone());
				Some(Filter::new(column, FilterOperator::Eq, value).into())
			})
			.collect::<Option<Vec<FilterCondition>>>()?;
		return Some(FilterCondition::and(filters));
	}

	let column = metadata
		.iter()
		.find(|field| field.name == T::primary_key_field())
		.map(|field| field.db_column_name().to_owned())
		.unwrap_or_else(|| T::primary_key_field().to_owned());
	let serialized = serde_json::to_value(item).ok()?;
	let primary_key_value = serialized
		.get(T::primary_key_field())
		.or_else(|| serialized.get(&column))?;
	let filter = primary_key_filter_for_model::<T>(primary_key_value).ok()?;
	let FilterCondition::Single(mut filter) = filter else {
		return None;
	};
	filter.field = column;
	Some(filter.into())
}

/// Django REST Framework-style ViewSet handler for models.
///
/// Provides automatic CRUD operations with permission checks, filtering,
/// pagination, and serialization for Model types.
///
/// # Examples
///
/// ```no_run
/// # use reinhardt_views::viewsets::ModelViewSetHandler;
/// # use reinhardt_db::orm::Model;
/// # use serde::{Serialize, Deserialize};
/// #
/// # #[derive(Serialize, Deserialize, Clone, Debug)]
/// # struct User {
/// #     id: Option<i64>,
/// #     username: String,
/// # }
/// #
/// # #[derive(Clone)]
/// # struct UserFields;
/// #
/// # impl reinhardt_db::orm::FieldSelector for UserFields {
/// #     fn with_alias(self, _alias: &str) -> Self { self }
/// # }
/// #
/// # impl Model for User {
/// #     type PrimaryKey = i64;
/// #     type Fields = UserFields;
/// #     type Objects = reinhardt_db::orm::Manager<Self>;
/// #     fn table_name() -> &'static str { "users" }
/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
/// #     fn new_fields() -> Self::Fields { UserFields }
/// # }
/// #
/// # async fn example() {
/// let handler = ModelViewSetHandler::<User>::new();
/// # }
/// ```
pub struct ModelViewSetHandler<T>
where
	T: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
	queryset: Option<Vec<T>>,
	queryset_fn: Option<Arc<QuerysetFn>>,
	serializer_class: Option<Arc<dyn Serializer<Input = T, Output = String> + Send + Sync>>,
	permission_classes: Vec<Arc<dyn Permission>>,
	filter_backends: Vec<Arc<dyn FilterBackend>>,
	pagination_class: Option<reinhardt_core::pagination::PaginatorImpl>,
	pool: Option<Arc<sqlx::AnyPool>>,
	/// Database backend type (default: PostgreSQL)
	db_backend: DbBackend,
	_phantom: PhantomData<T>,
}

impl<T> ModelViewSetHandler<T>
where
	T: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
	/// Create a new ModelViewSetHandler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let handler = ModelViewSetHandler::<User>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			queryset: None,
			queryset_fn: None,
			serializer_class: None,
			permission_classes: Vec::new(),
			filter_backends: Vec::new(),
			pagination_class: None,
			pool: None,
			db_backend: DbBackend::Postgres, // Default to PostgreSQL
			_phantom: PhantomData,
		}
	}

	/// Set the queryset (in-memory data) for this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let users = vec![
	///     User { id: Some(1), username: "alice".to_string() },
	///     User { id: Some(2), username: "bob".to_string() },
	/// ];
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_queryset(users);
	/// ```
	pub fn with_queryset(mut self, queryset: Vec<T>) -> Self {
		self.queryset = Some(queryset);
		self
	}

	/// Scope database queries using the current request.
	///
	/// The synchronous, fallible hook returns one [`FilterCondition`] and requires
	/// a database pool. It applies to list, retrieve, update, and destroy; create
	/// deliberately does not call it, so create ownership belongs in the
	/// serializer, permission layer, or database. Resolve asynchronous scope data
	/// in middleware before dispatch and read its application-defined identity
	/// from request extensions in this hook. Static `Vec` data supplied through
	/// [`Self::with_queryset`] is separate and is never filtered by this hook.
	/// A scoped-out object or a malformed detail primary key is reported as 404.
	/// Custom lookup fields are outside this primary-key scope boundary and are
	/// tracked by #6091.
	pub fn with_queryset_fn<F>(mut self, queryset_fn: F) -> Self
	where
		F: Fn(&Request) -> std::result::Result<FilterCondition, ViewError> + Send + Sync + 'static,
	{
		self.queryset_fn = Some(Arc::new(queryset_fn));
		self
	}

	/// Set the serializer class for this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_rest::serializers::ModelSerializer;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use std::sync::Arc;
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let serializer = Arc::new(ModelSerializer::<User>::new());
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_serializer(serializer);
	/// ```
	pub fn with_serializer(
		mut self,
		serializer: Arc<dyn Serializer<Input = T, Output = String> + Send + Sync>,
	) -> Self {
		self.serializer_class = Some(serializer);
		self
	}

	/// Set the database connection pool for this handler
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use sqlx::AnyPool;
	/// # use std::sync::Arc;
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = Arc::new(AnyPool::connect("postgres://localhost/mydb").await?);
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_pool(pool);
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_pool(mut self, pool: Arc<sqlx::AnyPool>) -> Self {
		self.pool = Some(pool);
		self
	}

	/// Set the database backend type for this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_db::orm::{Model, query_types::DbBackend};
	/// # use serde::{Serialize, Deserialize};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .with_db_backend(DbBackend::Sqlite);
	/// ```
	pub fn with_db_backend(mut self, db_backend: DbBackend) -> Self {
		self.db_backend = db_backend;
		self
	}

	/// Add a permission class to this handler
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_auth::IsAuthenticated;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use std::sync::Arc;
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// let handler = ModelViewSetHandler::<User>::new()
	///     .add_permission(Arc::new(IsAuthenticated));
	/// ```
	pub fn add_permission(mut self, permission: Arc<dyn Permission>) -> Self {
		self.permission_classes.push(permission);
		self
	}

	/// Add a filter backend to this handler
	pub fn add_filter_backend(mut self, backend: Arc<dyn FilterBackend>) -> Self {
		self.filter_backends.push(backend);
		self
	}

	/// Set the pagination class for this handler
	pub fn with_pagination(
		mut self,
		pagination: reinhardt_core::pagination::PaginatorImpl,
	) -> Self {
		self.pagination_class = Some(pagination);
		self
	}

	/// Get the queryset for this handler
	fn get_queryset(&self) -> &[T] {
		self.queryset.as_deref().unwrap_or(&[])
	}

	fn scoped_queryset(&self, request: &Request) -> std::result::Result<QuerySet<T>, ViewError> {
		let mut queryset = T::objects().all().for_model_session();
		queryset.map_filter_columns(map_scope_filter_column::<T>);
		queryset.map_order_by_fields(map_scope_order_by_field::<T>);
		queryset.map_subquery_fields(map_scope_subquery_field::<T>);
		match &self.queryset_fn {
			Some(queryset_fn) => {
				let mut condition = queryset_fn(request)?;
				map_scope_filter_columns::<T>(&mut condition);
				Ok(queryset.filter(condition))
			}
			None => Ok(queryset),
		}
	}

	fn ensure_scope_values_unchanged(
		&self,
		request: &Request,
		before: &serde_json::Value,
		after: &serde_json::Value,
	) -> std::result::Result<(), ViewError> {
		let mut field_names = Vec::new();
		let manager_queryset = T::objects().all();
		let mut has_opaque_subquery = manager_queryset
			.filters()
			.iter()
			.any(|filter| scope_filter_value_contains_opaque_subquery(&filter.value));
		has_opaque_subquery |= manager_queryset
			.filter_conditions()
			.iter()
			.any(scope_filter_condition_contains_opaque_subquery);
		for filter in manager_queryset.filters() {
			field_names.push(
				filter
					.source_field_name()
					.unwrap_or(&filter.field)
					.to_owned(),
			);
			collect_scope_filter_value(&filter.value, &mut field_names);
		}
		for condition in manager_queryset.filter_conditions() {
			collect_scope_filter_condition(condition, &mut field_names);
		}
		field_names.extend(manager_queryset.subquery_fields().map(|field| {
			field
				.rsplit_once('.')
				.map_or_else(|| field.to_owned(), |(_, name)| name.to_owned())
		}));
		let mut has_opaque_join = false;
		if manager_queryset.has_joins() {
			for condition in manager_queryset.join_on_conditions() {
				if join_condition_is_opaque(condition) {
					has_opaque_join = true;
					continue;
				}
				let before = field_names.len();
				collect_scope_sql_fields::<T>(condition, &mut field_names);
				if field_names.len() == before {
					has_opaque_join = true;
				}
			}
		}
		if let Some(queryset_fn) = &self.queryset_fn {
			let condition = queryset_fn(request)?;
			has_opaque_subquery |= scope_filter_condition_contains_opaque_subquery(&condition);
			collect_scope_filter_condition(&condition, &mut field_names);
		}
		if has_opaque_subquery {
			return Err(ViewError::Permission(
				"opaque scalar subquery scopes cannot be mutated".to_owned(),
			));
		}
		if has_opaque_join {
			return Err(ViewError::Permission(
				"opaque join-backed scopes cannot be mutated".to_owned(),
			));
		}
		if field_names.is_empty() {
			return Ok(());
		}
		field_names.sort_unstable();
		field_names.dedup();

		for field_name in field_names {
			let field_name = field_name
				.rsplit_once('.')
				.map_or(field_name.as_str(), |(_, name)| name);
			let Some(field) = T::field_metadata()
				.into_iter()
				.find(|field| field.name == field_name || field.db_column_name() == field_name)
			else {
				return Err(ViewError::Permission(format!(
					"request scope field `{field_name}` is not a model field"
				)));
			};
			if serialized_scope_field(before, &field) != serialized_scope_field(after, &field) {
				return Err(ViewError::Permission(format!(
					"scope field `{}` cannot be changed",
					field.name
				)));
			}
		}

		Ok(())
	}

	fn ensure_scope_fields_unchanged(
		&self,
		request: &Request,
		before: &T,
		after: &T,
	) -> std::result::Result<(), ViewError> {
		let before = serde_json::to_value(before).map_err(|error| {
			ViewError::Serialization(format!("failed to serialize original scope state: {error}"))
		})?;
		let after = serde_json::to_value(after).map_err(|error| {
			ViewError::Serialization(format!("failed to serialize updated scope state: {error}"))
		})?;
		self.ensure_scope_values_unchanged(request, &before, &after)
	}

	fn primary_key_filter(
		pk: &serde_json::Value,
	) -> std::result::Result<FilterCondition, ViewError> {
		primary_key_filter_for_model::<T>(pk)
	}

	async fn get_object(
		&self,
		request: &Request,
		pk: &serde_json::Value,
	) -> std::result::Result<T, ViewError> {
		let pool = self.pool.as_ref().ok_or_else(|| {
			ViewError::Internal("with_queryset_fn requires a database pool".to_owned())
		})?;
		let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
			.await
			.map_err(|error| {
				ViewError::DatabaseError(format!("Failed to create session: {error}"))
			})?;
		let queryset = self
			.scoped_queryset(request)?
			.filter(Self::primary_key_filter(pk)?)
			.without_slicing();
		session
			.list(&queryset)
			.await
			.map_err(|error| ViewError::DatabaseError(format!("Failed to query objects: {error}")))?
			.into_iter()
			.next()
			.ok_or_else(|| ViewError::NotFound(format!("Object with pk={pk} not found")))
	}

	/// Get the serializer for this handler
	fn get_serializer(&self) -> Arc<dyn Serializer<Input = T, Output = String> + Send + Sync> {
		self.serializer_class
			.clone()
			.unwrap_or_else(|| Arc::new(ModelSerializer::<T>::new()))
	}

	/// Check permissions for the request
	async fn check_permissions(&self, request: &Request) -> std::result::Result<(), ViewError> {
		// Extract authentication information from request extensions
		// The session middleware stores authenticated user_id in extensions
		//
		// Expected usage:
		// 1. Session middleware extracts session from cookie/token
		// 2. Middleware validates session and extracts user_id
		// 3. Middleware stores user_id in request.extensions using a dedicated type
		//
		// Example middleware implementation:
		//   if let Some(user_id) = session.get::<i64>("user_id").ok().flatten() {
		//       request.extensions.insert(AuthenticatedUserId(user_id));
		//   }

		let auth_state = AuthState::from_extensions(&request.extensions);
		let is_authenticated = auth_state
			.as_ref()
			.map(|state| state.is_authenticated())
			.unwrap_or(false);
		let is_admin = auth_state
			.as_ref()
			.map(|state| state.is_admin())
			.unwrap_or(false);
		let is_active = auth_state
			.as_ref()
			.map(|state| state.is_active())
			.unwrap_or(false);
		let user_obj = None;

		let context = PermissionContext {
			request,
			is_authenticated,
			is_admin,
			is_active,
			user: user_obj,
		};

		// Check all registered permission classes
		for permission in &self.permission_classes {
			if !permission.has_permission(&context).await {
				// Permission denied - return specific error
				return Err(ViewError::Permission(format!(
					"Permission denied by {}",
					std::any::type_name_of_val(&**permission)
				)));
			}
		}

		Ok(())
	}

	/// List all objects with optional filtering and pagination
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/users/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()?;
	/// let response = handler.list(&request).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list(&self, request: &Request) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		let items = if let Some(pool) = &self.pool {
			let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|error| {
					ViewError::DatabaseError(format!("Failed to create session: {error}"))
				})?;
			let queryset = self.scoped_queryset(request)?;
			session.list(&queryset).await.map_err(|error| {
				ViewError::DatabaseError(format!("Failed to list objects: {error}"))
			})?
		} else if self.queryset_fn.is_some() {
			return Err(ViewError::Internal(
				"with_queryset_fn requires a database pool".to_owned(),
			));
		} else {
			self.get_queryset().to_vec()
		};

		// Serialize all objects
		let mut serialized_items = Vec::new();
		for item in &items {
			let json = serializer
				.serialize(item)
				.map_err(|e| ViewError::Serialization(e.to_string()))?;
			serialized_items.push(json);
		}

		// Create response body
		let response_body = format!("[{}]", serialized_items.join(","));

		Ok(Response::ok().with_body(response_body))
	}

	/// Retrieve a single object by primary key
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use serde_json::Value;
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/users/1/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()?;
	/// let pk = serde_json::json!(1);
	/// let response = handler.retrieve(&request, pk).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn retrieve(
		&self,
		request: &Request,
		pk: serde_json::Value,
	) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		let item = if self.pool.is_some() {
			self.get_object(request, &pk).await?
		} else if self.queryset_fn.is_some() {
			return Err(ViewError::Internal(
				"with_queryset_fn requires a database pool".to_owned(),
			));
		} else {
			let queryset = self.get_queryset();
			let pk_str = pk.to_string();
			let pk_str = pk_str.trim_matches('"');
			queryset
				.iter()
				.find(|item| {
					if let Some(item_pk) = item.primary_key() {
						item_pk.to_string() == pk_str
					} else {
						false
					}
				})
				.cloned()
				.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk)))?
		};

		let json = serializer
			.serialize(&item)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		Ok(Response::ok().with_body(json))
	}

	/// Create a new object
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::POST)
	///     .uri("/users/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::from(r#"{"username":"alice"}"#))
	///     .build()?;
	/// let response = handler.create(&request).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create(&self, request: &Request) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		// Parse request body
		let body_str = String::from_utf8(request.body().to_vec())
			.map_err(|e| ViewError::BadRequest(format!("Invalid UTF-8: {}", e)))?;

		// Deserialize into model
		let item = serializer
			.deserialize(&body_str)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		// Save to database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Begin transaction
			session.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Add object to session
			session
				.add_new(item.clone())
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to add object: {}", e)))?;

			// Flush changes to database (generates and executes INSERT)
			session
				.flush()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Get the generated ID from the session
			let generated_id = session.get_generated_ids().first().map(|(_, id)| *id);

			// Commit transaction
			session
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;

			// Re-fetch the created object from the database to get all auto-populated fields
			// (e.g., created_at which is set by database DEFAULT), including when the
			// primary key was supplied by the caller.
			let refresh_filter = if let Some(id) = generated_id {
				Some(Self::primary_key_filter(&serde_json::json!(id))?)
			} else {
				assigned_primary_key_filter(&item)
			};
			if let Some(refresh_filter) = refresh_filter {
				let fetch_session =
					reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
						.await
						.map_err(|e| {
							ViewError::DatabaseError(format!("Failed to create session: {}", e))
						})?;

				let queryset = QuerySet::<T>::new().filter(refresh_filter).limit(1);
				let created_item = fetch_session
					.list(&queryset)
					.await
					.map_err(|error| {
						ViewError::DatabaseError(format!(
							"Failed to refresh created object: {error}"
						))
					})?
					.into_iter()
					.next()
					.ok_or_else(|| {
						ViewError::DatabaseError("Failed to find created object".to_owned())
					})?;

				// Serialize the complete object (including auto-populated fields)
				let response_body = serializer
					.serialize(&created_item)
					.map_err(|e| ViewError::Serialization(e.to_string()))?;

				return Ok(Response::created().with_body(response_body));
			}
		}

		// Fallback: return the original item if no database pool
		let response_body = serializer
			.serialize(&item)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		Ok(Response::created().with_body(response_body))
	}

	/// Update an existing object
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use serde_json::Value;
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::PUT)
	///     .uri("/users/1/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::from(r#"{"username":"alice_updated"}"#))
	///     .build()?;
	/// let pk = serde_json::json!(1);
	/// let response = handler.update(&request, pk).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn update(
		&self,
		request: &Request,
		pk: serde_json::Value,
	) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		let serializer = self.get_serializer();

		let existing_obj = if self.pool.is_some() {
			self.get_object(request, &pk).await?
		} else if self.queryset_fn.is_some() {
			return Err(ViewError::Internal(
				"with_queryset_fn requires a database pool".to_owned(),
			));
		} else {
			// Fall back to queryset for non-database mode
			// Normalize pk: strip surrounding quotes only (consistent with retrieve()).
			let pk_str_owned = pk.to_string();
			let pk_str = pk_str_owned.trim_matches('"');
			self.get_queryset()
				.iter()
				.find(|item| {
					if let Some(item_pk) = item.primary_key() {
						item_pk.to_string() == pk_str
					} else {
						false
					}
				})
				.cloned()
				.ok_or_else(|| {
					ViewError::NotFound(format!("Object with pk {} not found", pk_str))
				})?
		};

		// Parse request body as JSON for partial update (PATCH semantics)
		let body_str = String::from_utf8(request.body().to_vec())
			.map_err(|e| ViewError::BadRequest(format!("Invalid UTF-8: {}", e)))?;

		// Parse patch data as JSON
		let patch_data: serde_json::Value = serde_json::from_str(&body_str)
			.map_err(|e| ViewError::Serialization(format!("Invalid JSON: {}", e)))?;

		// Serialize existing object to JSON and merge with patch data
		let existing_json = serializer
			.serialize(&existing_obj)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;
		let mut existing_value: serde_json::Value = serde_json::from_str(&existing_json)
			.map_err(|e| ViewError::Serialization(format!("Failed to parse existing: {}", e)))?;
		// Validate and merge patch data into existing object (only overwrites provided fields)
		crate::generic::patch_utils::merge_patch_object_into(&mut existing_value, &patch_data)
			.map_err(ViewError::BadRequest)?;

		// Deserialize merged object back to model type
		let merged_json = serde_json::to_string(&existing_value)
			.map_err(|e| ViewError::Serialization(format!("Failed to serialize merged: {}", e)))?;
		let mut updated_item: T = serializer
			.deserialize(&merged_json)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;
		self.ensure_scope_fields_unchanged(request, &existing_obj, &updated_item)?;
		let primary_key = existing_obj
			.primary_key()
			.ok_or_else(|| ViewError::Internal("Object has no primary key".to_owned()))?;
		updated_item.set_primary_key(primary_key);
		let response_json = serializer
			.serialize(&updated_item)
			.map_err(|e| ViewError::Serialization(e.to_string()))?;

		// Update database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Recheck and mutate through one dedicated transaction connection.
			let mut transaction = pool.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Recheck the request-scoped predicate and lock the row before writing.
			let mutation_queryset = self
				.scoped_queryset(request)?
				.filter(Self::primary_key_filter(&pk)?)
				.without_slicing()
				.without_distinct();
			if session
				.list_with_connection_for_update(&mutation_queryset, &mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to recheck object: {}", e)))?
				.into_iter()
				.next()
				.is_none()
			{
				return Err(ViewError::NotFound(format!(
					"Object with pk={} not found",
					pk
				)));
			}

			// Add updated object to session (marks as dirty for UPDATE)
			session
				.add(updated_item.clone())
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to add object: {}", e)))?;

			// Flush changes to database (generates and executes UPDATE)
			session
				.flush_with_connection(&mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Commit transaction
			transaction
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;
		}

		// Return the complete merged/updated object
		Ok(Response::ok().with_body(response_json))
	}

	/// Delete an object
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_views::viewsets::ModelViewSetHandler;
	/// # use reinhardt_http::Request;
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # use serde_json::Value;
	/// # use bytes::Bytes;
	/// # use hyper::{Method, Version, HeaderMap};
	/// #
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User {
	/// #     id: Option<i64>,
	/// #     username: String,
	/// # }
	/// #
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// #
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// # }
	/// #
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let handler = ModelViewSetHandler::<User>::new();
	/// let request = Request::builder()
	///     .method(Method::DELETE)
	///     .uri("/users/1/")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()?;
	/// let pk = serde_json::json!(1);
	/// let response = handler.destroy(&request, pk).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn destroy(
		&self,
		request: &Request,
		pk: serde_json::Value,
	) -> std::result::Result<Response, ViewError> {
		self.check_permissions(request).await?;

		if self.pool.is_none() {
			if self.queryset_fn.is_some() {
				return Err(ViewError::Internal(
					"with_queryset_fn requires a database pool".to_owned(),
				));
			}
			let pk_str_owned = pk.to_string();
			let pk_str = pk_str_owned.trim_matches('"');
			self.get_queryset()
				.iter()
				.find(|item| {
					item.primary_key()
						.map(|item_pk| item_pk.to_string() == pk_str)
						.unwrap_or(false)
				})
				.cloned()
				.ok_or_else(|| {
					ViewError::NotFound(format!("Object with pk {} not found", pk_str))
				})?;
		}

		// Delete from database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			// Recheck and mutate through one dedicated transaction connection.
			let mut transaction = pool.begin().await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
			})?;

			// Recheck the request-scoped predicate and lock the row before deleting.
			let mutation_queryset = self
				.scoped_queryset(request)?
				.filter(Self::primary_key_filter(&pk)?)
				.without_slicing()
				.without_distinct();
			let item = session
				.list_with_connection_for_update(&mutation_queryset, &mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to recheck object: {}", e)))?
				.into_iter()
				.next()
				.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk)))?;

			// Mark object for deletion
			session.delete(item).await.map_err(|e| {
				ViewError::DatabaseError(format!("Failed to mark object for deletion: {}", e))
			})?;

			// Flush changes to database (generates and executes DELETE)
			session
				.flush_with_connection(&mut *transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			// Commit transaction
			transaction
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;
		}

		Ok(Response::no_content())
	}
}

impl<T> Default for ModelViewSetHandler<T>
where
	T: Model + Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_auth::{IsActiveUser, IsAuthenticated};
	use reinhardt_db::orm::fields::{CharField, Field};
	use reinhardt_db::orm::inspection::FieldInfo;
	use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	use reinhardt_http::Request;
	use rstest::rstest;
	use std::sync::atomic::{AtomicUsize, Ordering};

	fn build_request(uri: &str) -> Request {
		Request::builder()
			.method(Method::GET)
			.uri(uri)
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	fn composite_pk_parser_preserves_delimiters_in_length_prefixed_values() {
		let fields = vec!["namespace".to_owned(), "id".to_owned()];
		let parts =
			parse_length_prefixed_composite_parts("namespace=9:a, id=999, id=3:123", &fields)
				.expect("length-prefixed composite keys should parse");

		assert_eq!(parts, vec!["a, id=999", "123"]);
	}

	#[rstest]
	fn legacy_composite_pk_parser_uses_typed_boundaries() {
		let fields = vec!["namespace".to_owned(), "id".to_owned()];
		let is_valid = |index: usize, value: &str| index == 0 || value.parse::<i64>().is_ok();
		let parts =
			parse_legacy_composite_parts("namespace=a, id=999, id=1", &fields, 0, &is_valid)
				.expect("legacy composite keys should parse");

		assert_eq!(parts, vec!["a, id=999", "1"]);
	}

	// -----------------------------------------------------------------------
	// Test model for retrieve PK tests
	// -----------------------------------------------------------------------

	#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
	struct TestItem {
		id: Option<i64>,
		name: String,
		organization: Option<String>,
	}

	#[derive(Clone, Copy)]
	struct AliasTestItemSerializer;

	impl Serializer for AliasTestItemSerializer {
		type Input = TestItem;
		type Output = String;

		fn serialize(&self, input: &Self::Input) -> Result<Self::Output, SerializerError> {
			serde_json::to_string(&serde_json::json!({
				"id": input.id,
				"name": input.name,
				"tenant": input.organization,
			}))
			.map_err(|error| SerializerError::Serde {
				message: error.to_string(),
			})
		}

		fn deserialize(&self, output: &Self::Output) -> Result<Self::Input, SerializerError> {
			#[derive(serde::Deserialize)]
			struct Payload {
				id: Option<i64>,
				name: String,
				tenant: Option<String>,
			}

			let payload: Payload =
				serde_json::from_str(output).map_err(|error| SerializerError::Serde {
					message: error.to_string(),
				})?;
			Ok(TestItem {
				id: payload.id,
				name: payload.name,
				organization: payload.tenant,
			})
		}
	}

	#[derive(Clone)]
	struct TestItemFields;

	#[derive(Clone, Copy)]
	struct OrganizationId(i64);

	#[derive(Default)]
	struct ScopedTestItemManager;

	impl reinhardt_db::orm::CustomManager for ScopedTestItemManager {
		type Model = TestItem;

		fn new() -> Self {
			Self
		}

		fn all(&self) -> QuerySet<Self::Model> {
			QuerySet::new()
				.filter(Filter::new(
					"organization",
					FilterOperator::Eq,
					FilterValue::Integer(99),
				))
				.filter(Filter::new(
					"organization",
					FilterOperator::Eq,
					FilterValue::FieldRef(reinhardt_db::orm::expressions::F::new("organization")),
				))
				.filter(Filter::new(
					"organization",
					FilterOperator::Eq,
					FilterValue::Expression(reinhardt_db::orm::annotation::Expression::Add(
						Box::new(reinhardt_db::orm::annotation::AnnotationValue::Field(
							reinhardt_db::orm::expressions::F::new("organization"),
						)),
						Box::new(reinhardt_db::orm::annotation::AnnotationValue::Value(
							reinhardt_db::orm::annotation::Value::Int(1),
						)),
					)),
				))
				.filter(
					reinhardt_db::orm::expressions::FieldRef::<TestItem, String>::new(
						"organization",
					)
					.year()
					.eq(2026),
				)
				.order_by(&["-organization"])
		}
	}

	impl reinhardt_db::orm::FieldSelector for TestItemFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl reinhardt_db::orm::Model for TestItem {
		type PrimaryKey = i64;
		type Fields = TestItemFields;
		type Objects = ScopedTestItemManager;

		fn table_name() -> &'static str {
			"test_items"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestItemFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut organization = CharField::new(255);
			organization.set_attributes_from_name("organization");
			organization.base.db_column = Some("organization_id".to_owned());
			vec![FieldInfo::from_field(&organization)]
		}
	}

	#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
	struct JoinScopedItem {
		id: Option<i64>,
		organization: Option<String>,
	}

	#[derive(Clone)]
	struct JoinScopedItemFields;

	#[derive(Default)]
	struct JoinScopedItemManager;

	#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
	struct JoinMembership {
		id: Option<i64>,
	}

	#[derive(Clone)]
	struct JoinMembershipFields;

	#[derive(Default)]
	struct JoinMembershipManager;

	impl reinhardt_db::orm::FieldSelector for JoinScopedItemFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl reinhardt_db::orm::FieldSelector for JoinMembershipFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl reinhardt_db::orm::CustomManager for JoinScopedItemManager {
		type Model = JoinScopedItem;

		fn new() -> Self {
			Self
		}

		fn all(&self) -> QuerySet<Self::Model> {
			QuerySet::new().inner_join_on::<JoinMembership>(
				"join_scoped_items.organization_id = memberships.organization_id",
			)
		}
	}

	impl reinhardt_db::orm::CustomManager for JoinMembershipManager {
		type Model = JoinMembership;

		fn new() -> Self {
			Self
		}
	}

	impl reinhardt_db::orm::Model for JoinScopedItem {
		type PrimaryKey = i64;
		type Fields = JoinScopedItemFields;
		type Objects = JoinScopedItemManager;

		fn table_name() -> &'static str {
			"join_scoped_items"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			JoinScopedItemFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut organization = CharField::new(255);
			organization.set_attributes_from_name("organization");
			organization.base.db_column = Some("organization_id".to_owned());
			vec![FieldInfo::from_field(&organization)]
		}
	}

	impl reinhardt_db::orm::Model for JoinMembership {
		type PrimaryKey = i64;
		type Fields = JoinMembershipFields;
		type Objects = JoinMembershipManager;

		fn table_name() -> &'static str {
			"memberships"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			JoinMembershipFields
		}
	}

	#[derive(Default)]
	struct OpaqueJoinItemManager;

	impl reinhardt_db::orm::CustomManager for OpaqueJoinItemManager {
		type Model = OpaqueJoinItem;

		fn new() -> Self {
			Self
		}

		fn all(&self) -> QuerySet<Self::Model> {
			QuerySet::new().inner_join_on::<JoinMembership>("true")
		}
	}

	#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
	struct OpaqueJoinItem {
		id: Option<i64>,
		organization: Option<String>,
	}

	#[derive(Clone)]
	struct OpaqueJoinItemFields;

	impl reinhardt_db::orm::FieldSelector for OpaqueJoinItemFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl reinhardt_db::orm::Model for OpaqueJoinItem {
		type PrimaryKey = i64;
		type Fields = OpaqueJoinItemFields;
		type Objects = OpaqueJoinItemManager;

		fn table_name() -> &'static str {
			"opaque_join_items"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			OpaqueJoinItemFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut organization = CharField::new(255);
			organization.set_attributes_from_name("organization");
			organization.base.db_column = Some("organization_id".to_owned());
			vec![FieldInfo::from_field(&organization)]
		}
	}

	/// Helper to build a ModelViewSetHandler with in-memory queryset
	fn build_model_handler(items: Vec<TestItem>) -> ModelViewSetHandler<TestItem> {
		ModelViewSetHandler::<TestItem>::new().with_queryset(items)
	}

	#[test]
	fn scoped_queryset_fn_reads_request_extensions() {
		let request = build_request("/items/");
		request.extensions.insert(OrganizationId(7));
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(|request| {
			let organization = request
				.extensions
				.get::<OrganizationId>()
				.ok_or_else(|| ViewError::Permission("organization scope is missing".to_owned()))?;
			Ok(Filter::new(
				"organization_id",
				FilterOperator::Eq,
				FilterValue::Integer(organization.0),
			)
			.into())
		});

		let queryset = handler.scoped_queryset(&request).unwrap();

		assert_eq!(queryset.filters().len(), 5);
		assert!(
			queryset
				.filters()
				.iter()
				.take(3)
				.all(|filter| filter.field == "organization_id")
		);
		assert_eq!(
			queryset.filters()[3].field,
			"EXTRACT(YEAR FROM \"organization_id\")"
		);
		assert_eq!(
			queryset.filters()[3].source_field_name(),
			Some("organization")
		);
		let FilterValue::FieldRef(field) = &queryset.filters()[1].value else {
			panic!("custom-manager field reference should be preserved");
		};
		assert_eq!(field.field, "organization_id");
		let FilterValue::Expression(expression) = &queryset.filters()[2].value else {
			panic!("custom-manager expression should be preserved");
		};
		assert_eq!(expression.to_sql(), "(\"organization_id\" + 1)");
		assert_eq!(
			queryset.to_sql(),
			"SELECT * FROM \"test_items\" WHERE (\"organization_id\" = 99 AND \"organization_id\" = \"organization_id\" AND \"organization_id\" = (\"organization_id\" + 1) AND EXTRACT(YEAR FROM \"organization_id\") = 2026 AND \"organization_id\" = 7) ORDER BY \"organization_id\" DESC"
		);
	}

	#[test]
	fn postgres_annotation_scope_fields_are_mapped_and_collected() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Expression};

		let mut value = AnnotationValue::Expression(Expression::Coalesce(vec![
			AnnotationValue::ArrayAgg(
				reinhardt_db::orm::ArrayAgg::new("organization".to_owned())
					.order_by(vec!["organization DESC".to_owned()]),
			),
			AnnotationValue::StringAgg(reinhardt_db::orm::StringAgg::new(
				"organization".to_owned(),
				", ".to_owned(),
			)),
			AnnotationValue::JsonbAgg(reinhardt_db::orm::JsonbAgg::new("organization".to_owned())),
			AnnotationValue::JsonbBuildObject(
				reinhardt_db::orm::JsonbBuildObject::new().add("tenant", "organization"),
			),
			AnnotationValue::TsRank(reinhardt_db::orm::TsRank::new(
				"organization".to_owned(),
				"tenant".to_owned(),
			)),
		]));

		let mut fields = Vec::new();
		collect_scope_annotation_value(&value, &mut fields);
		assert_eq!(fields, vec!["organization"; 6]);

		map_scope_annotation_value::<TestItem>(&mut value);
		assert_eq!(
			value.to_sql(),
			"COALESCE(ARRAY_AGG(organization_id ORDER BY organization_id DESC), STRING_AGG(organization_id, ', '), JSONB_AGG(organization_id), jsonb_build_object('tenant', organization_id), ts_rank(organization_id, to_tsquery('english', 'tenant')))"
		);
	}

	#[rstest]
	fn qualified_scope_fields_map_the_final_component() {
		let mut field = "items.organization".to_owned();

		map_scope_field::<TestItem>(&mut field);

		assert_eq!(field, "items.organization_id");
	}

	#[rstest]
	fn qualified_scope_ordering_maps_the_final_component() {
		let mut field = "items.organization DESC NULLS LAST".to_owned();

		map_scope_order_by_field::<TestItem>(&mut field);

		assert_eq!(field, "items.organization_id DESC NULLS LAST");
	}

	#[rstest]
	fn opaque_scalar_subquery_scope_is_rejected_before_mutation() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Expression};

		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(|_| {
			Ok(Filter::new(
				"organization",
				FilterOperator::Eq,
				FilterValue::Expression(Expression::Coalesce(vec![AnnotationValue::Subquery(
					"(SELECT organization_id FROM memberships WHERE memberships.item_id = items.id)"
						.to_owned(),
				)])),
			)
			.into())
		});

		let error = handler
			.ensure_scope_values_unchanged(
				&request,
				&serde_json::json!({"organization": "tenant-a"}),
				&serde_json::json!({"organization": "tenant-a"}),
			)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message)
				if message == "opaque scalar subquery scopes cannot be mutated"
		));
	}

	#[rstest]
	fn scope_field_changes_are_rejected_before_update() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(|_| {
			Ok(Filter::new("organization", FilterOperator::Eq, FilterValue::Integer(7)).into())
		});

		let error = handler
			.ensure_scope_values_unchanged(
				&request,
				&serde_json::json!({"organization": "tenant-a"}),
				&serde_json::json!({"organization": "tenant-b"}),
			)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message) if message == "scope field `organization` cannot be changed"
		));
	}

	#[test]
	fn scope_field_changes_are_rejected_after_custom_serializer_alias_round_trip() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(|_| {
			Ok(Filter::new(
				"organization",
				FilterOperator::Eq,
				FilterValue::String("tenant-a".to_owned()),
			)
			.into())
		});
		let serializer = AliasTestItemSerializer;
		let existing = TestItem {
			id: Some(1),
			name: "item".to_owned(),
			organization: Some("tenant-a".to_owned()),
		};
		let mut serialized: serde_json::Value =
			serde_json::from_str(&serializer.serialize(&existing).unwrap()).unwrap();
		serialized["tenant"] = serde_json::json!("tenant-b");
		let updated = serializer
			.deserialize(&serde_json::to_string(&serialized).unwrap())
			.unwrap();

		let error = handler
			.ensure_scope_fields_unchanged(&request, &existing, &updated)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message) if message == "scope field `organization` cannot be changed"
		));
	}

	#[rstest]
	fn custom_manager_scope_fields_are_rejected_before_update() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new();

		let error = handler
			.ensure_scope_values_unchanged(
				&request,
				&serde_json::json!({"organization": "99"}),
				&serde_json::json!({"organization": "100"}),
			)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message) if message == "scope field `organization` cannot be changed"
		));
	}

	#[test]
	fn join_backed_scope_fields_are_rejected_before_update() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<JoinScopedItem>::new();

		let error = handler
			.ensure_scope_values_unchanged(
				&request,
				&serde_json::json!({"organization": "tenant-a"}),
				&serde_json::json!({"organization": "tenant-b"}),
			)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message) if message == "scope field `organization` cannot be changed"
		));
	}

	#[test]
	fn opaque_join_backed_scopes_are_rejected_before_update() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<OpaqueJoinItem>::new();

		let error = handler
			.ensure_scope_values_unchanged(
				&request,
				&serde_json::json!({"organization": "tenant-a"}),
				&serde_json::json!({"organization": "tenant-a"}),
			)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message)
				if message == "opaque join-backed scopes cannot be mutated"
		));
	}

	#[test]
	fn scoped_queryset_propagates_hook_errors() {
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(|_| {
			Err(ViewError::Permission(
				"organization scope is missing".to_owned(),
			))
		});

		let error = match handler.scoped_queryset(&build_request("/items/")) {
			Ok(_) => panic!("queryset hook error must propagate"),
			Err(error) => error,
		};

		assert!(
			matches!(error, ViewError::Permission(message) if message == "organization scope is missing")
		);
	}

	#[test]
	fn get_object_primary_key_filter_preserves_integer_type() {
		let filter =
			ModelViewSetHandler::<TestItem>::primary_key_filter(&serde_json::json!(42)).unwrap();
		let FilterCondition::Single(filter) = filter else {
			panic!("a scalar primary key should produce one filter");
		};

		assert_eq!(filter.field, "id");
		assert!(matches!(filter.value, FilterValue::Integer(42)));
	}

	#[test]
	fn assigned_primary_key_filter_preserves_declared_key_binding() {
		let item = TestItem {
			id: Some(42),
			name: "item".to_owned(),
			organization: Some("tenant-a".to_owned()),
		};
		let FilterCondition::Single(filter) = assigned_primary_key_filter(&item).unwrap() else {
			panic!("single primary key should produce one filter");
		};

		assert_eq!(filter.field, "id");
		assert!(matches!(filter.value, FilterValue::Integer(42)));
	}

	#[tokio::test]
	async fn queryset_fn_without_pool_fails_closed() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new()
			.with_queryset(vec![TestItem {
				id: Some(1),
				name: "visible".to_owned(),
				organization: None,
			}])
			.with_queryset_fn(|_| {
				Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into())
			});

		let error = handler.list(&request).await.unwrap_err();

		assert!(matches!(error, ViewError::Internal(_)));
	}

	#[tokio::test]
	async fn permission_denial_does_not_call_queryset_fn() {
		let hook_calls = Arc::new(AtomicUsize::new(0));
		let hook_calls_for_queryset = Arc::clone(&hook_calls);
		let handler = ModelViewSetHandler::<TestItem>::new()
			.add_permission(Arc::new(IsAuthenticated))
			.with_queryset_fn(move |_| {
				hook_calls_for_queryset.fetch_add(1, Ordering::SeqCst);
				Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into())
			});

		let error = handler.list(&build_request("/items/")).await.unwrap_err();

		assert!(matches!(error, ViewError::Permission(_)));
		assert_eq!(hook_calls.load(Ordering::SeqCst), 0);
	}

	#[tokio::test]
	async fn create_does_not_call_queryset_fn() {
		let hook_calls = Arc::new(AtomicUsize::new(0));
		let hook_calls_for_queryset = Arc::clone(&hook_calls);
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_fn(move |_| {
			hook_calls_for_queryset.fetch_add(1, Ordering::SeqCst);
			Ok(Filter::new("organization_id", FilterOperator::Eq, 1_i64.into()).into())
		});
		let request = Request::builder()
			.method(Method::POST)
			.uri("/items/")
			.body(Bytes::from_static(br#"{"id":null,"name":"created"}"#))
			.build()
			.unwrap();

		let response = handler.create(&request).await.unwrap();

		assert_eq!(response.status, hyper::StatusCode::CREATED);
		assert_eq!(hook_calls.load(Ordering::SeqCst), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_list_denies_bare_user_id_extensions_for_active_permissions() {
		// Arrange
		let handler = build_model_handler(vec![TestItem {
			id: Some(1),
			name: "first".to_string(),
			organization: None,
		}])
		.add_permission(Arc::new(IsAuthenticated))
		.add_permission(Arc::new(IsActiveUser));
		let request = build_request("/items/");
		request.extensions.insert("legacy-user".to_string());

		// Act
		let result = handler.list(&request).await;

		// Assert
		let error = result.expect_err("bare user ID extensions must not grant authorization");
		assert!(matches!(error, ViewError::Permission(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_retrieve_strips_quotes_from_numeric_pk() {
		// Arrange
		let items = vec![
			TestItem {
				id: Some(1),
				name: "first".to_string(),
				organization: None,
			},
			TestItem {
				id: Some(2),
				name: "second".to_string(),
				organization: None,
			},
		];
		let handler = build_model_handler(items);
		let request = build_request("/items/1/");

		// Act - pass pk with surrounding quotes (as JSON string value)
		let pk = serde_json::json!("1");
		let result = handler.retrieve(&request, pk).await;

		// Assert - should find the item despite quotes in pk
		assert!(result.is_ok(), "retrieve should succeed with quoted pk");
		let response = result.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
		let body: TestItem =
			serde_json::from_slice(&response.body).expect("response should be valid JSON");
		assert_eq!(body.name, "first");
		assert_eq!(body.id, Some(1));
	}

	#[rstest]
	#[tokio::test]
	async fn test_retrieve_works_with_unquoted_numeric_pk() {
		// Arrange
		let items = vec![TestItem {
			id: Some(42),
			name: "answer".to_string(),
			organization: None,
		}];
		let handler = build_model_handler(items);
		let request = build_request("/items/42/");

		// Act - pass pk as JSON number (no quotes)
		let pk = serde_json::json!(42);
		let result = handler.retrieve(&request, pk).await;

		// Assert
		assert!(result.is_ok(), "retrieve should succeed with numeric pk");
		let response = result.unwrap();
		assert_eq!(response.status, hyper::StatusCode::OK);
		let body: TestItem =
			serde_json::from_slice(&response.body).expect("response should be valid JSON");
		assert_eq!(body.name, "answer");
		assert_eq!(body.id, Some(42));
	}

	#[rstest]
	#[tokio::test]
	async fn test_retrieve_returns_not_found_for_nonexistent_pk() {
		// Arrange
		let items = vec![TestItem {
			id: Some(1),
			name: "only".to_string(),
			organization: None,
		}];
		let handler = build_model_handler(items);
		let request = build_request("/items/999/");

		// Act
		let pk = serde_json::json!(999);
		let result = handler.retrieve(&request, pk).await;

		// Assert
		assert!(result.is_err(), "retrieve should fail for nonexistent pk");
		let err = result.unwrap_err();
		assert!(
			matches!(err, ViewError::NotFound(_)),
			"error should be NotFound, got: {:?}",
			err
		);
	}
}
