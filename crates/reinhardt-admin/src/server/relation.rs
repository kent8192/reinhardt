use crate::adapters::{AdminDatabase, AdminSite};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey, ModelAdmin};
use crate::types::RelationLookupResponse;
#[cfg(server)]
use crate::types::{AdminError, AdminResult, RelationOption, RelationSelectorLayout};
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
use std::collections::{BTreeSet, HashMap};
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
	pub(crate) source_metadata: ModelMetadata,
	pub(crate) target_metadata: ModelMetadata,
	pub(crate) target_admin: Arc<dyn ModelAdmin>,
	pub(crate) source_pk_field: String,
	pub(crate) through_table: String,
	pub(crate) source_column: String,
	pub(crate) target_column: String,
	pub(crate) layout: RelationSelectorLayout,
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
		let pattern = format!("%{}%", escape_like_pattern(query));
		let mut condition = Condition::any();
		for field in descriptor.target_admin.search_fields() {
			condition = condition.add(Expr::col(Alias::new(field)).like(pattern.clone()));
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
fn escape_like_pattern(input: &str) -> String {
	input
		.replace('\\', "\\\\")
		.replace('%', "\\%")
		.replace('_', "\\_")
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
fn relation_value(metadata: &ModelMetadata, field_name: &str, input: &str) -> AdminResult<Value> {
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
	descriptor: &RelationDescriptor,
	ids: &[String],
	executor: &mut E,
) -> AdminResult<()> {
	let ids = ids.iter().map(String::as_str).collect::<BTreeSet<_>>();
	if ids.is_empty() {
		return Ok(());
	}
	let values = ids
		.iter()
		.map(|id| {
			relation_value(
				&descriptor.target_metadata,
				descriptor.target_admin.pk_field(),
				id,
			)
		})
		.collect::<AdminResult<Vec<_>>>()?;
	let statement = Query::select()
		.from(Alias::new(&descriptor.target_metadata.table_name))
		.column(Alias::new(descriptor.target_admin.pk_field()))
		.and_where(Expr::col(Alias::new(descriptor.target_admin.pk_field())).is_in(values))
		.to_owned();
	let (sql, values) = build_select(&statement, executor.backend());
	let rows = executor
		.fetch_all(&sql, convert_values(values))
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	if rows.len() != ids.len() {
		return Err(validation_error(
			"one or more relation selections are invalid",
		));
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
	use super::{resolve_relation_with_registry, validate_lookup_bounds};
	use crate::core::{AdminSite, AdminUser, ModelAdmin};
	use crate::types::{AdminError, RelationSelectorLayout};
	use reinhardt_db::m2m_naming::{default_m2m_columns, default_through_table};
	use reinhardt_db::migrations::FieldType;
	use reinhardt_db::migrations::model_registry::{
		FieldMetadata, ManyToManyMetadata, ModelMetadata, ModelRegistry,
	};
	use rstest::rstest;

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
}
