use crate::adapters::{AdminDatabase, AdminSite};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey, ModelAdmin};
use crate::types::RelationLookupResponse;
#[cfg(server)]
use crate::types::{AdminError, AdminResult, RelationOption, RelationSelectorLayout};
#[cfg(server)]
use reinhardt_db::associations::ManyToManyManager;
#[cfg(server)]
use reinhardt_db::m2m_naming::{default_m2m_columns, default_through_table};
#[cfg(server)]
use reinhardt_db::migrations::FieldType as DatabaseFieldType;
#[cfg(server)]
use reinhardt_db::migrations::model_registry::{ModelMetadata, ModelRegistry, global_registry};
#[cfg(server)]
use reinhardt_db::orm::execution::convert_values;
#[cfg(server)]
use reinhardt_db::orm::{DatabaseBackend, OrmExecutor, QueryRow};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use reinhardt_query::prelude::{
	Alias, Condition, Expr, ExprTrait, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query,
	QueryBuilder, SelectStatement, SqliteQueryBuilder, Value, Values,
};
#[cfg(server)]
use std::collections::{HashMap, HashSet};
#[cfg(server)]
use std::sync::Arc;

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use super::error::{AdminAuth, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::limits::{MAX_RELATION_QUERY_CHARS, RELATION_LOOKUP_PAGE_SIZE};

/// Resolved metadata needed to read one configured many-to-many relation.
#[cfg(server)]
#[derive(Clone)]
pub(crate) struct RelationDescriptor {
	pub(crate) field_name: String,
	pub(crate) source_metadata: ModelMetadata,
	pub(crate) target_metadata: ModelMetadata,
	pub(crate) target_admin: Arc<dyn ModelAdmin>,
	pub(crate) source_pk_field: String,
	pub(crate) through_table: String,
	pub(crate) source_column: String,
	pub(crate) target_column: String,
	pub(crate) layout: RelationSelectorLayout,
}

/// One normalized relation selection removed from parent mutation data.
#[cfg(server)]
#[derive(Clone)]
pub(crate) struct RelationSelection {
	pub(crate) descriptor: RelationDescriptor,
	pub(crate) ids: Vec<String>,
}

#[cfg(server)]
fn validation_error(message: impl Into<String>) -> AdminError {
	AdminError::ValidationError(message.into())
}

#[cfg(server)]
pub(crate) fn validate_lookup_bounds(query: &str, page: u64) -> AdminResult<u64> {
	if page == 0 {
		return Err(validation_error("relation lookup page must be at least 1"));
	}
	if query.chars().count() > MAX_RELATION_QUERY_CHARS {
		return Err(validation_error("relation lookup query is too long"));
	}
	(page - 1)
		.checked_mul(RELATION_LOOKUP_PAGE_SIZE)
		.ok_or_else(|| validation_error("relation lookup page is too large"))
}

#[cfg(server)]
pub(crate) fn resolve_relation(
	site: &AdminSite,
	source_admin: &dyn ModelAdmin,
	field_name: &str,
) -> AdminResult<RelationDescriptor> {
	resolve_relation_with_registry(site, source_admin, field_name, global_registry())
}

#[cfg(server)]
pub(crate) fn resolve_relations(
	site: &AdminSite,
	source_admin: &dyn ModelAdmin,
) -> AdminResult<Vec<RelationDescriptor>> {
	source_admin
		.filter_horizontal()
		.into_iter()
		.chain(source_admin.filter_vertical())
		.map(|field_name| resolve_relation(site, source_admin, field_name))
		.collect()
}

#[cfg(server)]
fn resolve_relation_with_registry(
	site: &AdminSite,
	source_admin: &dyn ModelAdmin,
	field_name: &str,
	registry: &ModelRegistry,
) -> AdminResult<RelationDescriptor> {
	let horizontal = source_admin.filter_horizontal();
	let vertical = source_admin.filter_vertical();
	let is_horizontal = horizontal.contains(&field_name);
	let is_vertical = vertical.contains(&field_name);
	let layout = match (is_horizontal, is_vertical) {
		(true, false) => RelationSelectorLayout::Horizontal,
		(false, true) => RelationSelectorLayout::Vertical,
		(true, true) => {
			return Err(validation_error(
				"relation selector cannot use both layouts",
			));
		}
		(false, false) => return Err(validation_error("relation selector is not configured")),
	};

	let source_metadata = registry
		.get_models()
		.into_iter()
		.find(|metadata| metadata.table_name == source_admin.table_name())
		.ok_or_else(|| validation_error("source model metadata is not registered"))?;
	let relation = source_metadata
		.many_to_many_fields
		.iter()
		.find(|relation| relation.field_name == field_name)
		.ok_or_else(|| validation_error("configured selector is not a many-to-many relation"))?;

	let target_metadata =
		if let Some((app_label, model_name)) = relation.to_model.split_once('.') {
			registry.find_model_qualified(app_label, model_name)
		} else {
			registry
				.find_model_qualified(&source_metadata.app_label, &relation.to_model)
				.or_else(|| registry.find_model_by_name(&relation.to_model))
		}
		.ok_or_else(|| validation_error("target model metadata is not registered"))?;
	let target_admin = site.get_model_admin(&target_metadata.model_name)?;
	if target_admin.table_name() != target_metadata.table_name {
		return Err(validation_error(
			"target model admin does not match relation metadata",
		));
	}
	if target_admin.search_fields().is_empty() {
		return Err(validation_error(
			"relation target must configure search_fields",
		));
	}

	let through_table = relation.through.clone().unwrap_or_else(|| {
		default_through_table(&source_metadata.table_name, &relation.field_name)
	});
	let (default_source_column, default_target_column) =
		default_m2m_columns(&source_metadata.table_name, &target_metadata.table_name);

	Ok(RelationDescriptor {
		field_name: field_name.to_string(),
		source_pk_field: source_admin.pk_field().to_string(),
		through_table,
		source_column: relation
			.source_field
			.clone()
			.unwrap_or(default_source_column),
		target_column: relation
			.target_field
			.clone()
			.unwrap_or(default_target_column),
		source_metadata,
		target_metadata,
		target_admin,
		layout,
	})
}

#[cfg(server)]
pub(crate) fn split_relation_values(
	mut data: HashMap<String, serde_json::Value>,
	descriptors: &[RelationDescriptor],
) -> AdminResult<(HashMap<String, serde_json::Value>, Vec<RelationSelection>)> {
	let mut selections = Vec::new();
	for descriptor in descriptors {
		let Some(value) = data.remove(&descriptor.field_name) else {
			continue;
		};
		let serde_json::Value::Array(values) = value else {
			return Err(validation_error(format!(
				"relation field '{}' must be an array",
				descriptor.field_name
			)));
		};
		if values.len() > 100 {
			return Err(validation_error(format!(
				"relation field '{}' has too many selections",
				descriptor.field_name
			)));
		}

		let mut ids = Vec::with_capacity(values.len());
		let mut seen = HashSet::with_capacity(values.len());
		for value in values {
			let id = match value {
				serde_json::Value::String(value) => value.trim().to_string(),
				serde_json::Value::Number(value) if value.is_i64() || value.is_u64() => {
					value.to_string()
				}
				_ => {
					return Err(validation_error(format!(
						"relation field '{}' contains an invalid identifier",
						descriptor.field_name
					)));
				}
			};
			if id.is_empty() {
				return Err(validation_error(format!(
					"relation field '{}' contains an empty identifier",
					descriptor.field_name
				)));
			}
			if seen.insert(id.clone()) {
				ids.push(id);
			}
		}

		selections.push(RelationSelection {
			descriptor: descriptor.clone(),
			ids,
		});
	}
	Ok((data, selections))
}

#[cfg(server)]
fn selected_columns(descriptor: &RelationDescriptor) -> Vec<String> {
	let pk_field = descriptor.target_admin.pk_field();
	let mut columns = vec![pk_field.to_string()];
	for field in descriptor.target_admin.list_display() {
		if field != pk_field
			&& descriptor.target_metadata.fields.contains_key(field)
			&& !columns.iter().any(|column| column == field)
		{
			columns.push(field.to_string());
		}
	}
	columns
}

#[cfg(server)]
fn build_lookup_statement(
	descriptor: &RelationDescriptor,
	query: &str,
	page: u64,
) -> AdminResult<SelectStatement> {
	let offset = validate_lookup_bounds(query, page)?;
	let mut statement = Query::select();
	statement.from(Alias::new(&descriptor.target_metadata.table_name));
	for column in selected_columns(descriptor) {
		statement.column(Alias::new(column));
	}
	if !query.is_empty() {
		let mut condition = Condition::any();
		for field in descriptor.target_admin.search_fields() {
			condition = condition.add(Expr::col(Alias::new(field)).contains(query.to_owned()));
		}
		statement.cond_where(condition);
	}
	statement
		.order_by(Alias::new(descriptor.target_admin.pk_field()), Order::Asc)
		.limit(RELATION_LOOKUP_PAGE_SIZE + 1)
		.offset(offset);
	Ok(statement.to_owned())
}

#[cfg(server)]
fn build_select(statement: &SelectStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_select(statement),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_select(statement),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_select(statement),
	}
}

#[cfg(server)]
fn rows_to_records(rows: Vec<reinhardt_db::orm::Row>) -> Vec<HashMap<String, serde_json::Value>> {
	rows.into_iter()
		.filter_map(|row| match QueryRow::from_backend_row(row).data {
			serde_json::Value::Object(values) => Some(values.into_iter().collect()),
			_ => None,
		})
		.collect()
}

#[cfg(server)]
fn record_option(
	descriptor: &RelationDescriptor,
	record: &HashMap<String, serde_json::Value>,
) -> AdminResult<RelationOption> {
	let value = record
		.get(descriptor.target_admin.pk_field())
		.and_then(scalar_string)
		.ok_or_else(|| validation_error("relation target has an invalid primary key"))?;
	Ok(RelationOption::new(
		value,
		descriptor.target_admin.object_label(record),
	))
}

#[cfg(server)]
fn scalar_string(value: &serde_json::Value) -> Option<String> {
	match value {
		serde_json::Value::String(value) => Some(value.clone()),
		serde_json::Value::Number(value) => Some(value.to_string()),
		serde_json::Value::Bool(value) => Some(value.to_string()),
		serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
			None
		}
	}
}

#[cfg(server)]
pub(crate) async fn relation_options_with_executor<E: OrmExecutor>(
	descriptor: &RelationDescriptor,
	query: &str,
	page: u64,
	executor: &mut E,
) -> AdminResult<RelationLookupResponse> {
	let statement = build_lookup_statement(descriptor, query, page)?;
	let (sql, values) = build_select(&statement, executor.backend());
	let rows = executor
		.fetch_all(&sql, convert_values(values))
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	let mut options = rows_to_records(rows)
		.iter()
		.map(|record| record_option(descriptor, record))
		.collect::<AdminResult<Vec<_>>>()?;
	let has_more = options.len() > RELATION_LOOKUP_PAGE_SIZE as usize;
	options.truncate(RELATION_LOOKUP_PAGE_SIZE as usize);
	Ok(RelationLookupResponse {
		options,
		page,
		has_more,
	})
}

#[cfg(server)]
pub(crate) fn relation_value(
	metadata: &ModelMetadata,
	field_name: &str,
	input: &str,
) -> AdminResult<Value> {
	match metadata
		.fields
		.get(field_name)
		.map(|field| &field.field_type)
	{
		Some(DatabaseFieldType::BigInteger) => input
			.parse::<i64>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		Some(
			DatabaseFieldType::Integer
			| DatabaseFieldType::SmallInteger
			| DatabaseFieldType::TinyInt
			| DatabaseFieldType::MediumInt,
		) => input
			.parse::<i32>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		Some(DatabaseFieldType::Uuid) => input
			.parse::<uuid::Uuid>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		_ => Ok(Value::from(input.to_string())),
	}
}

#[cfg(server)]
pub(crate) async fn current_relation_options<E: OrmExecutor>(
	descriptor: &RelationDescriptor,
	source_id: &str,
	executor: &mut E,
) -> AdminResult<Vec<RelationOption>> {
	let source_value = relation_value(
		&descriptor.source_metadata,
		&descriptor.source_pk_field,
		source_id,
	)?;
	let target_table = Alias::new(&descriptor.target_metadata.table_name);
	let through_table = Alias::new(&descriptor.through_table);
	let mut statement = Query::select();
	statement.from(target_table.clone()).distinct();
	for column in selected_columns(descriptor) {
		statement.column((target_table.clone(), Alias::new(column)));
	}
	statement
		.inner_join(
			through_table.clone(),
			Expr::col((
				target_table.clone(),
				Alias::new(descriptor.target_admin.pk_field()),
			))
			.equals((through_table.clone(), Alias::new(&descriptor.target_column))),
		)
		.and_where(
			Expr::col((through_table, Alias::new(&descriptor.source_column))).eq(source_value),
		)
		.order_by(
			(target_table, Alias::new(descriptor.target_admin.pk_field())),
			Order::Asc,
		);
	let (sql, values) = build_select(&statement, executor.backend());
	let rows = executor
		.fetch_all(&sql, convert_values(values))
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	rows_to_records(rows)
		.iter()
		.map(|record| record_option(descriptor, record))
		.collect()
}

#[cfg(server)]
pub(crate) async fn validate_relation_ids<E: OrmExecutor>(
	executor: &mut E,
	descriptor: &RelationDescriptor,
	ids: &[String],
) -> AdminResult<()> {
	let mut expected = Vec::with_capacity(ids.len());
	for id in ids {
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			id,
		)?;
		if !expected.contains(&value) {
			expected.push(value);
		}
	}
	if expected.is_empty() {
		return Ok(());
	}
	let statement = Query::select()
		.from(Alias::new(&descriptor.target_metadata.table_name))
		.column(Alias::new(descriptor.target_admin.pk_field()))
		.and_where(
			Expr::col(Alias::new(descriptor.target_admin.pk_field())).is_in(expected.clone()),
		)
		.to_owned();
	let (sql, values) = build_select(&statement, executor.backend());
	let rows = executor
		.fetch_all(&sql, convert_values(values))
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	let mut returned = Vec::with_capacity(rows.len());
	for row in rows {
		let row = QueryRow::from_backend_row(row);
		let id = row
			.data
			.get(descriptor.target_admin.pk_field())
			.and_then(scalar_string)
			.ok_or_else(|| validation_error("relation target has an invalid primary key"))?;
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			&id,
		)?;
		if !returned.contains(&value) {
			returned.push(value);
		}
	}
	if returned.len() != expected.len() || expected.iter().any(|value| !returned.contains(value)) {
		return Err(validation_error(
			"one or more relation selections are invalid",
		));
	}
	Ok(())
}

#[cfg(server)]
pub(crate) async fn sync_relation_ids<E: OrmExecutor>(
	executor: &mut E,
	descriptor: &RelationDescriptor,
	source_pk: Value,
	ids: &[String],
) -> AdminResult<()> {
	let mut desired = Vec::with_capacity(ids.len());
	for id in ids {
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			id,
		)?;
		if !desired.contains(&value) {
			desired.push(value);
		}
	}

	let manager = ManyToManyManager::<(), (), Value>::new(
		source_pk,
		descriptor.through_table.clone(),
		descriptor.source_column.clone(),
		descriptor.target_column.clone(),
	);
	let rows = manager
		.all_with_db(
			executor,
			&descriptor.target_metadata.table_name,
			descriptor.target_admin.pk_field(),
		)
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	let mut current = Vec::with_capacity(rows.len());
	for row in rows {
		let id = row
			.data
			.get(descriptor.target_admin.pk_field())
			.and_then(scalar_string)
			.ok_or_else(|| validation_error("relation target has an invalid primary key"))?;
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			&id,
		)?;
		if !current.contains(&value) {
			current.push(value);
		}
	}

	for value in current.iter().filter(|value| !desired.contains(value)) {
		manager
			.remove_with_db(executor, value.clone())
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	}
	for value in desired.iter().filter(|value| !current.contains(value)) {
		manager
			.add_with_db(executor, value.clone())
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	}
	Ok(())
}

/// Look up a bounded page of options for a configured many-to-many selector.
#[server_fn]
pub async fn lookup_relation_options(
	model_name: String,
	field_name: String,
	query: String,
	page: u64,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<RelationLookupResponse, ServerFnError> {
	let auth = AdminAuth::from_request(&http_request);
	let source_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(source_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;
	let descriptor =
		resolve_relation(&site, source_admin.as_ref(), &field_name).map_server_fn_error()?;
	auth.require_model_permission(
		descriptor.target_admin.as_ref(),
		user.as_ref(),
		ModelPermission::View,
	)
	.await?;
	let mut connection = *db.connection();
	relation_options_with_executor(&descriptor, &query, page, &mut connection)
		.await
		.map_server_fn_error()
}

#[cfg(all(test, server))]
mod tests {
	use std::collections::HashMap;

	use super::{
		build_lookup_statement, build_select, resolve_relation_with_registry,
		split_relation_values, sync_relation_ids, validate_lookup_bounds, validate_relation_ids,
	};
	use crate::core::{AdminSite, AdminUser, ModelAdmin};
	use crate::types::{AdminError, RelationSelectorLayout};
	use reinhardt_db::backends::types::{QueryResult, QueryValue, Row};
	use reinhardt_db::m2m_naming::{default_m2m_columns, default_through_table};
	use reinhardt_db::migrations::FieldType;
	use reinhardt_db::migrations::model_registry::{
		FieldMetadata, ManyToManyMetadata, ModelMetadata, ModelRegistry,
	};
	use reinhardt_db::orm::{DatabaseBackend, OrmExecutor};
	use reinhardt_query::prelude::{Value, Values};
	use rstest::rstest;

	struct RelationExecutor {
		rows: Option<Vec<Row>>,
		fetch_calls: usize,
		executions: Vec<(String, Vec<QueryValue>)>,
	}

	impl RelationExecutor {
		fn with_ids(ids: &[&str]) -> Self {
			let rows = ids
				.iter()
				.map(|id| {
					let mut row = Row::new();
					row.data
						.insert("id".to_string(), QueryValue::String((*id).to_string()));
					row
				})
				.collect();
			Self {
				rows: Some(rows),
				fetch_calls: 0,
				executions: Vec::new(),
			}
		}
	}

	#[async_trait::async_trait]
	impl OrmExecutor for RelationExecutor {
		fn backend(&self) -> DatabaseBackend {
			DatabaseBackend::Postgres
		}

		async fn execute(
			&mut self,
			sql: &str,
			params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<QueryResult> {
			self.executions.push((sql.to_string(), params));
			Ok(QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<Row> {
			panic!("fetch_one is not used by relation persistence")
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<Vec<Row>> {
			self.fetch_calls += 1;
			Ok(self.rows.take().unwrap_or_default())
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<Option<Row>> {
			panic!("fetch_optional is not used by relation persistence")
		}
	}

	struct TestAdmin {
		model_name: &'static str,
		table_name: &'static str,
		list_display: Vec<&'static str>,
		search_fields: Vec<&'static str>,
		filter_horizontal: Vec<&'static str>,
		filter_vertical: Vec<&'static str>,
	}

	impl TestAdmin {
		fn source(horizontal: Vec<&'static str>, vertical: Vec<&'static str>) -> Self {
			Self {
				model_name: "Article",
				table_name: "blog_articles",
				list_display: vec!["id", "title"],
				search_fields: vec!["title"],
				filter_horizontal: horizontal,
				filter_vertical: vertical,
			}
		}

		fn target(table_name: &'static str, search_fields: Vec<&'static str>) -> Self {
			Self {
				model_name: "Tag",
				table_name,
				list_display: vec!["id", "name"],
				search_fields,
				filter_horizontal: Vec::new(),
				filter_vertical: Vec::new(),
			}
		}
	}

	#[async_trait::async_trait]
	impl ModelAdmin for TestAdmin {
		fn model_name(&self) -> &str {
			self.model_name
		}

		fn table_name(&self) -> &str {
			self.table_name
		}

		fn list_display(&self) -> Vec<&str> {
			self.list_display.clone()
		}

		fn search_fields(&self) -> Vec<&str> {
			self.search_fields.clone()
		}

		fn filter_horizontal(&self) -> Vec<&str> {
			self.filter_horizontal.clone()
		}

		fn filter_vertical(&self) -> Vec<&str> {
			self.filter_vertical.clone()
		}

		async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
			true
		}
	}

	fn relation_registry(target_reference: &str, target_app: &str) -> ModelRegistry {
		let registry = ModelRegistry::new();
		let mut source = ModelMetadata::new("blog", "Article", "blog_articles");
		source.add_field(
			"title".to_string(),
			FieldMetadata::new(FieldType::VarChar(200)),
		);
		source.add_many_to_many(ManyToManyMetadata::new("tags", target_reference));
		registry.register_model(source);

		let mut target = ModelMetadata::new(target_app, "Tag", "taxonomy_tags");
		target.add_field(
			"name".to_string(),
			FieldMetadata::new(FieldType::VarChar(100)),
		);
		registry.register_model(target);
		registry
	}

	fn normalization_descriptor() -> super::RelationDescriptor {
		let registry = relation_registry("Tag", "blog");
		let site = AdminSite::new("Test");
		site.register("Tag", TestAdmin::target("taxonomy_tags", vec!["name"]))
			.unwrap();
		resolve_relation_with_registry(
			&site,
			&TestAdmin::source(vec!["tags"], Vec::new()),
			"tags",
			&registry,
		)
		.unwrap()
	}

	#[rstest]
	fn normalize_relation_ids_trims_and_deduplicates_in_insertion_order() {
		// Arrange
		let descriptor = normalization_descriptor();
		let data = HashMap::from([
			("title".to_string(), serde_json::json!("Article")),
			(
				"tags".to_string(),
				serde_json::json!([1, " 1 ", 2, "abc", "abc"]),
			),
		]);

		// Act
		let (scalar_data, selections) = split_relation_values(data, &[descriptor]).unwrap();

		// Assert
		assert_eq!(
			scalar_data,
			HashMap::from([("title".to_string(), serde_json::json!("Article"))])
		);
		assert_eq!(
			selections[0].ids,
			vec!["1".to_string(), "2".to_string(), "abc".to_string()]
		);
	}

	#[rstest]
	fn normalize_relation_ids_rejects_invalid_values() {
		// Arrange
		let descriptor = normalization_descriptor();
		let invalid_values = [
			serde_json::Value::Null,
			serde_json::json!(true),
			serde_json::json!({"id": 1}),
			serde_json::json!([[1]]),
			serde_json::json!([null]),
			serde_json::json!([true]),
			serde_json::json!([{"id": 1}]),
			serde_json::json!([""]),
			serde_json::json!(["   "]),
			serde_json::json!([1.5]),
		];

		for value in invalid_values {
			let data = HashMap::from([("tags".to_string(), value)]);

			// Act
			let result = split_relation_values(data, std::slice::from_ref(&descriptor));

			// Assert
			assert!(matches!(result, Err(AdminError::ValidationError(_))));
		}
	}

	#[rstest]
	fn normalize_relation_ids_rejects_more_than_one_hundred_values() {
		// Arrange
		let descriptor = normalization_descriptor();
		let values = (0..101).map(serde_json::Value::from).collect();
		let data = HashMap::from([("tags".to_string(), serde_json::Value::Array(values))]);

		// Act
		let result = split_relation_values(data, &[descriptor]);

		// Assert
		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	#[tokio::test]
	async fn relation_validation_rejects_an_equal_count_of_different_ids() {
		// Arrange
		let descriptor = normalization_descriptor();
		let mut executor = RelationExecutor::with_ids(&["1", "3"]);

		// Act
		let result = validate_relation_ids(
			&mut executor,
			&descriptor,
			&["1".to_string(), "2".to_string()],
		)
		.await;

		// Assert
		assert!(matches!(result, Err(AdminError::ValidationError(_))));
		assert_eq!(executor.fetch_calls, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn relation_sync_changes_only_the_set_difference() {
		// Arrange
		let descriptor = normalization_descriptor();
		let mut executor = RelationExecutor::with_ids(&["1", "2"]);

		// Act
		sync_relation_ids(
			&mut executor,
			&descriptor,
			Value::Int(Some(7)),
			&["2".to_string(), "3".to_string()],
		)
		.await
		.unwrap();

		// Assert
		assert_eq!(executor.fetch_calls, 1);
		assert_eq!(executor.executions.len(), 2);
		assert!(executor.executions[0].0.starts_with("DELETE"));
		assert_eq!(
			executor.executions[0].1,
			vec![QueryValue::Int(7), QueryValue::String("1".to_string())]
		);
		assert!(executor.executions[1].0.starts_with("INSERT"));
		assert_eq!(
			executor.executions[1].1,
			vec![QueryValue::Int(7), QueryValue::String("3".to_string())]
		);
	}

	#[rstest]
	fn resolver_rejects_configured_ordinary_field() {
		// Arrange
		let registry = ModelRegistry::new();
		let mut source = ModelMetadata::new("blog", "Article", "blog_articles");
		source.add_field(
			"tags".to_string(),
			FieldMetadata::new(FieldType::VarChar(100)),
		);
		registry.register_model(source);
		let site = AdminSite::new("Test");
		let source_admin = TestAdmin::source(vec!["tags"], Vec::new());

		// Act
		let result = resolve_relation_with_registry(&site, &source_admin, "tags", &registry);

		// Assert
		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn resolver_rejects_manual_layout_overlap() {
		// Arrange
		let registry = relation_registry("Tag", "blog");
		let site = AdminSite::new("Test");
		site.register("Tag", TestAdmin::target("taxonomy_tags", vec!["name"]))
			.unwrap();
		let source_admin = TestAdmin::source(vec!["tags"], vec!["tags"]);

		// Act
		let result = resolve_relation_with_registry(&site, &source_admin, "tags", &registry);

		// Assert
		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn resolver_uses_qualified_target_metadata_and_existing_default_names() {
		// Arrange
		let registry = relation_registry("taxonomy.Tag", "taxonomy");
		registry.register_model(ModelMetadata::new("other", "Tag", "other_tags"));
		let site = AdminSite::new("Test");
		site.register("Tag", TestAdmin::target("taxonomy_tags", vec!["name"]))
			.unwrap();
		let source_admin = TestAdmin::source(vec!["tags"], Vec::new());

		// Act
		let descriptor =
			resolve_relation_with_registry(&site, &source_admin, "tags", &registry).unwrap();

		// Assert
		assert_eq!(descriptor.target_metadata.app_label, "taxonomy");
		assert_eq!(descriptor.target_metadata.table_name, "taxonomy_tags");
		assert_eq!(descriptor.layout, RelationSelectorLayout::Horizontal);
		assert_eq!(descriptor.through_table, "blog_articles_tags");
		assert_eq!(
			descriptor.through_table,
			default_through_table("blog_articles", "tags")
		);
		assert_eq!(descriptor.source_column, "blog_articles_id");
		assert_eq!(descriptor.target_column, "taxonomy_tags_id");
		assert_eq!(
			(descriptor.source_column, descriptor.target_column),
			default_m2m_columns("blog_articles", "taxonomy_tags")
		);
	}

	#[rstest]
	fn resolver_rejects_target_without_search_fields() {
		// Arrange
		let registry = relation_registry("Tag", "blog");
		let site = AdminSite::new("Test");
		site.register("Tag", TestAdmin::target("taxonomy_tags", Vec::new()))
			.unwrap();
		let source_admin = TestAdmin::source(vec!["tags"], Vec::new());

		// Act
		let result = resolve_relation_with_registry(&site, &source_admin, "tags", &registry);

		// Assert
		assert!(matches!(result, Err(AdminError::ValidationError(_))));
	}

	#[rstest]
	fn lookup_bounds_reject_page_zero() {
		assert!(matches!(
			validate_lookup_bounds("", 0),
			Err(AdminError::ValidationError(_))
		));
	}

	#[rstest]
	fn lookup_bounds_count_unicode_scalars() {
		assert_eq!(validate_lookup_bounds(&"界".repeat(100), 1).unwrap(), 0);
		assert!(matches!(
			validate_lookup_bounds(&"界".repeat(101), 1),
			Err(AdminError::ValidationError(_))
		));
	}

	#[rstest]
	fn lookup_query_treats_like_metacharacters_as_literals_on_sqlite() {
		// Arrange
		let registry = relation_registry("Tag", "blog");
		let site = AdminSite::new("Test");
		site.register("Tag", TestAdmin::target("taxonomy_tags", vec!["name"]))
			.unwrap();
		let source_admin = TestAdmin::source(vec!["tags"], Vec::new());
		let descriptor =
			resolve_relation_with_registry(&site, &source_admin, "tags", &registry).unwrap();

		// Act
		let statement = build_lookup_statement(&descriptor, r#"50%_\off"#, 1).unwrap();
		let (sql, values) = build_select(&statement, DatabaseBackend::Sqlite);

		// Assert
		assert_eq!(
			sql,
			r#"SELECT "id", "name" FROM "taxonomy_tags" WHERE "name" LIKE ? ESCAPE '\' ORDER BY "id" ASC LIMIT ? OFFSET ?"#
		);
		assert_eq!(
			values,
			Values(vec![
				Value::String(Some(Box::new(r#"%50\%\_\\off%"#.to_string()))),
				Value::BigUnsigned(Some(51)),
				Value::BigUnsigned(Some(0)),
			])
		);
	}
}
