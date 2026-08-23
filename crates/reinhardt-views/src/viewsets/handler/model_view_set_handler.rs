//! `ModelViewSetHandler` — Django REST Framework-style CRUD handler.
//!
//! Provides the standard list/retrieve/create/update/destroy actions with
//! permission checks, optional pagination, and serialization for `Model`
//! types. The response rendering for each action lives next to the action
//! itself in this module.

use super::error::ViewError;
use reinhardt_auth::{Permission, PermissionContext};
use reinhardt_db::orm::model::filter_value_from_field;
use reinhardt_db::orm::{
	CustomManager, Filter, FilterCondition, FilterOperator, FilterValue, Model, QuerySet,
	query_types::DbBackend,
};
use reinhardt_http::{AuthState, Request, Response};
use reinhardt_rest::filters::FilterBackend;
use reinhardt_rest::serializers::{ModelSerializer, Serializer};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

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
		AnnotationValue::Expression(expression) => map_scope_annotation_expression::<T>(expression),
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
		| FilterValue::Uuid(_)
		| FilterValue::Integer(_)
		| FilterValue::Int(_)
		| FilterValue::Float(_)
		| FilterValue::Boolean(_)
		| FilterValue::Bool(_)
		| FilterValue::Null
		| FilterValue::Array(_)
		| FilterValue::Typed(_) => {}
	}
}

fn map_scope_filter_column<T: Model>(filter: &mut Filter) {
	filter.map_expression_source(map_scope_expression_sql::<T>);
	map_scope_field::<T>(&mut filter.field);
	map_scope_filter_value::<T>(&mut filter.value);
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
		AnnotationValue::Value(_) | AnnotationValue::Field(_) => false,
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
		| FilterValue::Uuid(_)
		| FilterValue::Integer(_)
		| FilterValue::Int(_)
		| FilterValue::Float(_)
		| FilterValue::Boolean(_)
		| FilterValue::Bool(_)
		| FilterValue::Null
		| FilterValue::Array(_)
		| FilterValue::Typed(_) => {}
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

fn map_serializer_error(error: reinhardt_core::serializers::SerializerError) -> ViewError {
	match error {
		reinhardt_core::serializers::SerializerError::Database(error) => ViewError::Database(error),
		error => ViewError::Serialization(error.to_string()),
	}
}

fn execute_mysql_control_statement<'a>(
	connection: &'a mut sqlx::AnyConnection,
) -> impl std::future::Future<Output = sqlx::Result<sqlx::any::AnyQueryResult>> + Send + 'a {
	<&'a mut sqlx::AnyConnection as sqlx::Executor<'a>>::execute(
		connection,
		sqlx::raw_sql("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"),
	)
}

async fn begin_mysql_serializable(
	pool: &sqlx::AnyPool,
) -> std::result::Result<sqlx::Transaction<'static, sqlx::Any>, sqlx::Error> {
	let mut connection = pool.acquire().await?;
	execute_mysql_control_statement(&mut connection).await?;
	sqlx::Transaction::begin(
		connection,
		Some(std::borrow::Cow::Borrowed("START TRANSACTION")),
	)
	.await
}

async fn begin_mutation_transaction<T: Model>(
	pool: &sqlx::AnyPool,
	backend: DbBackend,
	queryset: &QuerySet<T>,
) -> std::result::Result<sqlx::Transaction<'static, sqlx::Any>, sqlx::Error> {
	if !queryset.requires_serializable_transaction() {
		return pool.begin().await;
	}

	if backend == DbBackend::Mysql {
		return begin_mysql_serializable(pool).await;
	}

	let statement = match backend {
		DbBackend::Postgres => "BEGIN ISOLATION LEVEL SERIALIZABLE",
		DbBackend::Mysql => unreachable!("MySQL transactions use separate control statements"),
		DbBackend::Sqlite => "BEGIN EXCLUSIVE",
	};
	pool.begin_with(statement).await
}

fn parse_length_prefixed_composite_parts<'a>(
	inner: &'a str,
	fields: &[String],
) -> Option<Vec<&'a str>> {
	if fields.is_empty() {
		return None;
	}

	let mut cursor = inner.strip_prefix("v2;")?;
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
	let mut parsed = None;
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
			if parsed.is_some() {
				return None;
			}
			parsed = Some(tail);
		}
	}

	parsed
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
		return Ok(Filter::new(T::primary_key_column(), FilterOperator::Eq, value).into());
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
			Some(field) => filter_value_from_field(field, part).is_ok(),
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
				.map(|field| filter_value_from_field(field, part))
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
		.unwrap_or_else(|| T::primary_key_column().to_owned());
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

/// Supplies a request-scoped database queryset to a model view handler.
///
/// The handler creates the base queryset from `Model::objects().all()` and
/// passes it to this provider. Implementations should return a transformed
/// queryset so manager predicates are retained. The hook is synchronous and
/// fallible; resolve asynchronous identity data before dispatch and expose it
/// through request extensions. Reinhardt does not impose a tenant model.
///
/// Providers apply to list, retrieve, update, and destroy. Create deliberately
/// bypasses the provider, and registering one without a database pool fails
/// closed instead of falling back to static in-memory data.
pub trait QuerySetProvider<M>: Send + Sync
where
	M: Model,
{
	/// Apply request-specific predicates to the supplied base queryset.
	fn get_queryset(
		&self,
		request: &Request,
		base: QuerySet<M>,
	) -> std::result::Result<QuerySet<M>, ViewError>;
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
	serializer_class: Option<Arc<dyn Serializer<Input = T, Output = String> + Send + Sync>>,
	permission_classes: Vec<Arc<dyn Permission>>,
	filter_backends: Vec<Arc<dyn FilterBackend>>,
	queryset_provider: Option<Arc<dyn QuerySetProvider<T>>>,
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
			serializer_class: None,
			permission_classes: Vec::new(),
			filter_backends: Vec::new(),
			queryset_provider: None,
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

	/// Set a request-scoped database queryset provider.
	///
	/// The provider is used for list, retrieve, update, and destroy actions.
	/// Create deliberately does not invoke it. A database pool must be
	/// configured when a provider is registered; otherwise requests fail closed.
	/// The provider receives `Model::objects().all()` and must transform that
	/// queryset rather than replacing its manager predicates.
	pub fn with_queryset_provider<P>(mut self, provider: P) -> Self
	where
		P: QuerySetProvider<T> + 'static,
	{
		self.queryset_provider = Some(Arc::new(provider));
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

	fn ensure_provider_pool(&self) -> std::result::Result<(), ViewError> {
		if self.queryset_provider.is_some() && self.pool.is_none() {
			return Err(ViewError::Internal(
				"with_queryset_provider requires a database pool".to_owned(),
			));
		}
		Ok(())
	}

	fn scoped_queryset(&self, request: &Request) -> std::result::Result<QuerySet<T>, ViewError> {
		let base = T::objects().all().for_model_session();
		let mut queryset = match &self.queryset_provider {
			Some(provider) => provider.get_queryset(request, base),
			None => Ok(base),
		}?;
		queryset.map_filter_columns(map_scope_filter_column::<T>);
		queryset.map_order_by_fields(map_scope_order_by_field::<T>);
		queryset.map_subquery_fields(map_scope_subquery_field::<T>);
		Ok(queryset)
	}

	fn database_queryset(&self, request: &Request) -> std::result::Result<QuerySet<T>, ViewError> {
		self.ensure_provider_pool()?;
		if self.pool.is_none() {
			return Err(ViewError::Internal(
				"database queryset requires a database pool".to_owned(),
			));
		}

		self.scoped_queryset(request)
	}

	fn database_detail_queryset(
		&self,
		request: &Request,
	) -> std::result::Result<QuerySet<T>, ViewError> {
		let queryset = self.scoped_queryset(request)?;
		if queryset.has_slicing() {
			return Err(ViewError::BadRequest(
				"detail actions do not support sliced provider querysets".to_owned(),
			));
		}
		Ok(queryset)
	}

	fn primary_key_filter(
		&self,
		pk: &serde_json::Value,
	) -> std::result::Result<FilterCondition, ViewError> {
		primary_key_filter_for_model::<T>(pk)
	}

	async fn database_object(
		&self,
		request: &Request,
		pk: &serde_json::Value,
	) -> std::result::Result<T, ViewError> {
		let queryset = self
			.database_detail_queryset(request)?
			.filter(self.primary_key_filter(pk)?)
			.limit(1);
		let pool = self.pool.as_ref().ok_or_else(|| {
			ViewError::Internal("database queryset requires a database pool".to_owned())
		})?;
		let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
			.await
			.map_err(|error| ViewError::DatabaseError(error.to_string()))?;
		session
			.list(&queryset)
			.await
			.map_err(|error| ViewError::DatabaseError(error.to_string()))?
			.into_iter()
			.next()
			.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk)))
	}

	fn ensure_scope_values_unchanged(
		&self,
		request: &Request,
		before: &serde_json::Value,
		after: &serde_json::Value,
	) -> std::result::Result<(), ViewError> {
		let queryset = self.scoped_queryset(request)?;
		let mut field_names = Vec::new();
		let mut has_opaque_subquery = queryset
			.filters()
			.iter()
			.any(|filter| scope_filter_value_contains_opaque_subquery(&filter.value));
		has_opaque_subquery |= queryset
			.filter_conditions()
			.iter()
			.any(scope_filter_condition_contains_opaque_subquery);
		for filter in queryset.filters() {
			field_names.push(
				filter
					.source_field_name()
					.unwrap_or(&filter.field)
					.to_owned(),
			);
			collect_scope_filter_value(&filter.value, &mut field_names);
		}
		for condition in queryset.filter_conditions() {
			collect_scope_filter_condition(condition, &mut field_names);
		}
		field_names.extend(queryset.subquery_fields().map(|field| {
			field
				.rsplit_once('.')
				.map_or_else(|| field.to_owned(), |(_, name)| name.to_owned())
		}));
		if has_opaque_subquery {
			return Err(ViewError::Permission(
				"opaque scalar subquery scopes cannot be mutated".to_owned(),
			));
		}
		if field_names.is_empty() {
			return Ok(());
		}
		field_names.sort_unstable();
		field_names.dedup();

		let metadata = T::field_metadata();
		for field_name in field_names {
			let field_name = field_name
				.rsplit_once('.')
				.map_or(field_name.as_str(), |(_, name)| name);
			let Some(field) = metadata
				.iter()
				.find(|field| field.name == field_name || field.db_column_name() == field_name)
			else {
				return Err(ViewError::Permission(format!(
					"request scope field `{field_name}` is not a model field"
				)));
			};
			if serialized_scope_field(before, field) != serialized_scope_field(after, field) {
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

	fn apply_patch_to_item(
		&self,
		serializer: &dyn Serializer<Input = T, Output = String>,
		base: &T,
		patch_data: &serde_json::Value,
	) -> std::result::Result<T, ViewError> {
		let base_json = serializer.serialize(base).map_err(map_serializer_error)?;
		let mut value: serde_json::Value = serde_json::from_str(&base_json).map_err(|error| {
			ViewError::Serialization(format!("Failed to parse existing: {error}"))
		})?;
		crate::generic::patch_utils::merge_patch_object_into(&mut value, patch_data)
			.map_err(ViewError::BadRequest)?;
		let merged_json = serde_json::to_string(&value).map_err(|error| {
			ViewError::Serialization(format!("Failed to serialize merged: {error}"))
		})?;
		let mut updated_item = serializer
			.deserialize(&merged_json)
			.map_err(map_serializer_error)?;
		let primary_key = base
			.primary_key()
			.ok_or_else(|| ViewError::Internal("Object has no primary key".to_owned()))?;
		updated_item.set_primary_key(primary_key);
		Ok(updated_item)
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
		self.ensure_provider_pool()?;

		let serializer = self.get_serializer();

		// Get items from the request-scoped database queryset when a pool is set.
		let items: Vec<T> = if let Some(pool) = &self.pool {
			let queryset = self.database_queryset(request)?;
			let session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|error| ViewError::DatabaseError(error.to_string()))?;

			session
				.list(&queryset)
				.await
				.map_err(|error| ViewError::DatabaseError(error.to_string()))?
		} else {
			// Use in-memory queryset
			self.get_queryset().to_vec()
		};

		// Serialize all objects
		let mut serialized_items = Vec::new();
		for item in &items {
			let json = serializer.serialize(item).map_err(map_serializer_error)?;
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
		self.ensure_provider_pool()?;

		let serializer = self.get_serializer();

		// Apply the typed primary-key predicate to the manager/provider queryset.
		let item: T = if self.pool.is_some() {
			self.database_object(request, &pk).await?
		} else {
			// Use in-memory queryset
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

		let json = serializer.serialize(&item).map_err(map_serializer_error)?;

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
		// Provider registration is a database configuration contract, but create
		// intentionally never invokes the provider or manager queryset.
		self.ensure_provider_pool()?;

		let serializer = self.get_serializer();

		// Parse request body
		let body_str = String::from_utf8(request.body().to_vec())
			.map_err(|e| ViewError::BadRequest(format!("Invalid UTF-8: {}", e)))?;

		// Deserialize into model
		let item = serializer
			.deserialize(&body_str)
			.map_err(map_serializer_error)?;

		// Save to database if pool is available
		if let Some(pool) = &self.pool {
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
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

			// Re-fetch the created object from the database to get all auto-populated fields
			// (e.g., created_at which is set by database DEFAULT), including when the
			// primary key was supplied by the caller.
			let refresh_filter = if let Some(id) = generated_id {
				Some(self.primary_key_filter(&serde_json::json!(id))?)
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

				// Refresh through a PK-only queryset so create remains independent of
				// request-scoped providers and custom manager predicates.
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
					.map_err(map_serializer_error)?;

				return Ok(Response::created().with_body(response_body));
			}
		}

		// Fallback: return the original item if no database pool
		let response_body = serializer.serialize(&item).map_err(map_serializer_error)?;

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
		self.ensure_provider_pool()?;

		let serializer = self.get_serializer();

		// Get existing object from database
		let existing_obj: T = if self.pool.is_some() {
			self.database_object(request, &pk).await?
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

		// Update database if pool is available
		if let Some(pool) = &self.pool {
			let pool = Arc::clone(pool);
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			let mutation_queryset = self
				.database_detail_queryset(request)?
				.filter(self.primary_key_filter(&pk)?)
				.limit(1)
				.without_distinct();
			// Recheck and mutate through one dedicated transaction connection. A
			// serializable transaction is required when the authorization predicate
			// includes a subquery, because row locks cannot protect missing rows or
			// predicate gaps from concurrent inserts.
			let mut transaction =
				begin_mutation_transaction(pool.as_ref(), self.db_backend, &mutation_queryset)
					.await
					.map_err(|e| {
						ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
					})?;
			let rechecked_items = if self.db_backend == DbBackend::Sqlite {
				session
					.list_with_connection(&mutation_queryset, &mut transaction)
					.await
			} else {
				session
					.list_with_connection_for_update(&mutation_queryset, &mut transaction)
					.await
			};
			let locked_item = rechecked_items
				.map_err(|e| ViewError::DatabaseError(format!("Failed to recheck object: {}", e)))?
				.into_iter()
				.next()
				.ok_or_else(|| ViewError::NotFound(format!("Object with pk={} not found", pk)))?;

			// Build the PATCH state from the row protected by the transaction lock.
			let updated_item =
				self.apply_patch_to_item(serializer.as_ref(), &locked_item, &patch_data)?;
			self.ensure_scope_fields_unchanged(request, &locked_item, &updated_item)?;
			let response_body = serializer
				.serialize(&updated_item)
				.map_err(map_serializer_error)?;

			// Add updated object to session (marks as dirty for UPDATE)
			session
				.add(updated_item.clone())
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to add object: {}", e)))?;

			// Flush changes to database (generates and executes UPDATE)
			session
				.flush_with_connection(&mut transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

			transaction
				.commit()
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to commit: {}", e)))?;

			return Ok(Response::ok().with_body(response_body));
		}

		let updated_item =
			self.apply_patch_to_item(serializer.as_ref(), &existing_obj, &patch_data)?;
		let response_body = serializer
			.serialize(&updated_item)
			.map_err(map_serializer_error)?;

		// Return the complete merged/updated object
		Ok(Response::ok().with_body(response_body))
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
			self.retrieve(request, pk.clone()).await?;
		}

		// Delete from database if pool is available
		if let Some(pool) = &self.pool {
			let pool = Arc::clone(pool);
			// Create a new session for this request
			let mut session = reinhardt_db::prelude::Session::new(pool.clone(), self.db_backend)
				.await
				.map_err(|e| {
					ViewError::DatabaseError(format!("Failed to create session: {}", e))
				})?;

			let mutation_queryset = self
				.database_detail_queryset(request)?
				.filter(self.primary_key_filter(&pk)?)
				.limit(1)
				.without_distinct();
			let mut transaction =
				begin_mutation_transaction(pool.as_ref(), self.db_backend, &mutation_queryset)
					.await
					.map_err(|e| {
						ViewError::DatabaseError(format!("Failed to begin transaction: {}", e))
					})?;
			let rechecked_items = if self.db_backend == DbBackend::Sqlite {
				session
					.list_with_connection(&mutation_queryset, &mut transaction)
					.await
			} else {
				session
					.list_with_connection_for_update(&mutation_queryset, &mut transaction)
					.await
			};
			let item = rechecked_items
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
				.flush_with_connection(&mut transaction)
				.await
				.map_err(|e| ViewError::DatabaseError(format!("Failed to flush: {}", e)))?;

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
	use reinhardt_http::{IsActive, IsAuthenticated as AuthenticatedMarker, Request};
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
			parse_length_prefixed_composite_parts("v2;namespace=9:a, id=999, id=3:123", &fields)
				.expect("length-prefixed composite keys should parse");

		assert_eq!(parts, vec!["a, id=999", "123"]);
	}

	#[rstest]
	fn composite_pk_parser_requires_a_version_marker_for_length_prefixed_values() {
		let fields = vec!["namespace".to_owned(), "id".to_owned()];

		assert!(
			parse_length_prefixed_composite_parts("namespace=9:a, id=999, id=3:123", &fields)
				.is_none()
		);
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

	#[rstest]
	fn legacy_composite_pk_parser_rejects_ambiguous_string_boundaries() {
		let fields = vec!["namespace".to_owned(), "slug".to_owned()];
		let is_valid = |_index: usize, _value: &str| true;

		assert!(
			parse_legacy_composite_parts("namespace=a, slug=x, slug=y", &fields, 0, &is_valid)
				.is_none()
		);
	}

	// -----------------------------------------------------------------------
	// Test model for retrieve PK tests
	// -----------------------------------------------------------------------

	#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
	struct TestItem {
		id: Option<i64>,
		name: String,
	}

	#[derive(Clone)]
	struct TestItemFields;

	impl reinhardt_db::orm::FieldSelector for TestItemFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl reinhardt_db::orm::Model for TestItem {
		type PrimaryKey = i64;
		type Fields = TestItemFields;
		type Objects = reinhardt_db::orm::Manager<Self>;

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
			let mut name = CharField::new(255);
			name.set_attributes_from_name("name");
			name.base.db_column = Some("item_name".to_owned());
			vec![FieldInfo::from_field(&name)]
		}
	}

	struct ProviderFn<F>(F);

	impl<M, F> QuerySetProvider<M> for ProviderFn<F>
	where
		M: Model,
		F: Fn(&Request, QuerySet<M>) -> std::result::Result<QuerySet<M>, ViewError> + Send + Sync,
	{
		fn get_queryset(
			&self,
			request: &Request,
			base: QuerySet<M>,
		) -> std::result::Result<QuerySet<M>, ViewError> {
			(self.0)(request, base)
		}
	}

	#[derive(Clone)]
	struct ScopeName(String);

	struct CountingProvider {
		calls: Arc<AtomicUsize>,
	}

	impl QuerySetProvider<TestItem> for CountingProvider {
		fn get_queryset(
			&self,
			_request: &Request,
			base: QuerySet<TestItem>,
		) -> std::result::Result<QuerySet<TestItem>, ViewError> {
			self.calls.fetch_add(1, Ordering::Relaxed);
			Ok(base)
		}
	}

	/// Helper to build a ModelViewSetHandler with in-memory queryset
	fn build_model_handler(items: Vec<TestItem>) -> ModelViewSetHandler<TestItem> {
		ModelViewSetHandler::<TestItem>::new().with_queryset(items)
	}

	#[test]
	fn scoped_queryset_provider_maps_model_fields_and_reads_request_extensions() {
		let request = build_request("/items/");
		request.extensions.insert(ScopeName("visible".to_owned()));
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_provider(ProviderFn(
			|request: &Request, base: QuerySet<TestItem>| {
				let name = request
					.extensions
					.get::<ScopeName>()
					.ok_or_else(|| ViewError::Permission("request scope is missing".to_owned()))?;
				Ok(base.filter(Filter::new(
					"name",
					FilterOperator::Eq,
					FilterValue::String(name.0.clone()),
				)))
			},
		));

		let queryset = handler.scoped_queryset(&request).unwrap();

		assert_eq!(queryset.filters().len(), 1);
		assert_eq!(queryset.filters()[0].field, "item_name");
		assert_eq!(queryset.filters()[0].source_field_name(), Some("name"));
	}

	#[test]
	fn scope_field_changes_are_rejected_before_update() {
		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_provider(ProviderFn(
			|_request: &Request, base: QuerySet<TestItem>| {
				Ok(base.filter(Filter::new(
					"name",
					FilterOperator::Eq,
					FilterValue::String("visible".to_owned()),
				)))
			},
		));

		let error = handler
			.ensure_scope_values_unchanged(
				&request,
				&serde_json::json!({"name": "visible"}),
				&serde_json::json!({"name": "hidden"}),
			)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message) if message == "scope field `name` cannot be changed"
		));
	}

	#[test]
	fn opaque_scalar_subquery_scope_is_rejected_before_mutation() {
		use reinhardt_db::orm::annotation::{AnnotationValue, Expression};

		let request = build_request("/items/");
		let handler = ModelViewSetHandler::<TestItem>::new().with_queryset_provider(ProviderFn(
			|_request: &Request, base: QuerySet<TestItem>| {
				Ok(base.filter(Filter::new(
					"name",
					FilterOperator::Eq,
					FilterValue::Expression(Expression::Coalesce(vec![AnnotationValue::Subquery(
						"(SELECT item_name FROM memberships)".to_owned(),
					)])),
				)))
			},
		));

		let error = handler
			.ensure_scope_values_unchanged(
				&request,
				&serde_json::json!({"name": "visible"}),
				&serde_json::json!({"name": "visible"}),
			)
			.unwrap_err();

		assert!(matches!(
			error,
			ViewError::Permission(message)
				if message == "opaque scalar subquery scopes cannot be mutated"
		));
	}

	#[test]
	fn assigned_primary_key_filter_preserves_integer_type() {
		let item = TestItem {
			id: Some(42),
			name: "visible".to_owned(),
		};
		let FilterCondition::Single(filter) = assigned_primary_key_filter(&item).unwrap() else {
			panic!("single primary key should produce one filter");
		};

		assert_eq!(filter.field, "id");
		assert!(matches!(filter.value, FilterValue::Integer(42)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_list_denies_bare_user_id_extensions_for_active_permissions() {
		// Arrange
		let handler = build_model_handler(vec![TestItem {
			id: Some(1),
			name: "first".to_string(),
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
	async fn test_list_allows_legacy_user_id_extensions_for_active_permissions() {
		// Arrange
		let handler = build_model_handler(vec![TestItem {
			id: Some(1),
			name: "first".to_string(),
		}])
		.add_permission(Arc::new(IsAuthenticated))
		.add_permission(Arc::new(IsActiveUser));
		let request = build_request("/items/");
		request.extensions.insert("legacy-user".to_string());
		request.extensions.insert(AuthenticatedMarker(true));
		request.extensions.insert(IsActive(true));

		// Act
		let result = handler.list(&request).await;

		// Assert
		let response = result.expect("legacy authenticated requests should remain authorized");
		assert_eq!(response.status, hyper::StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_retrieve_strips_quotes_from_numeric_pk() {
		// Arrange
		let items = vec![
			TestItem {
				id: Some(1),
				name: "first".to_string(),
			},
			TestItem {
				id: Some(2),
				name: "second".to_string(),
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

	#[rstest]
	#[tokio::test]
	async fn test_queryset_provider_fails_closed_without_pool() {
		// Arrange
		let calls = Arc::new(AtomicUsize::new(0));
		let handler = build_model_handler(Vec::new()).with_queryset_provider(CountingProvider {
			calls: calls.clone(),
		});
		let request = build_request("/items/");

		// Act
		let result = handler.list(&request).await;

		// Assert
		assert!(matches!(result, Err(ViewError::Internal(_))));
		assert_eq!(calls.load(Ordering::Relaxed), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_update_restores_route_primary_key_from_patch_body() {
		// Arrange
		let handler = build_model_handler(vec![TestItem {
			id: Some(1),
			name: "before".to_owned(),
		}]);
		let request = Request::builder()
			.method(Method::PATCH)
			.uri("/items/1/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::from(r#"{"id":2,"name":"after"}"#))
			.build()
			.unwrap();

		// Act
		let response = handler
			.update(&request, serde_json::json!(1))
			.await
			.expect("patch should update the scoped object");

		// Assert
		let body: TestItem = serde_json::from_slice(&response.body).unwrap();
		assert_eq!(body.id, Some(1));
		assert_eq!(body.name, "after");
	}
}
