//! Permission-aware foreign-key relation lookups.

#[cfg(server)]
use super::admin_auth::AdminAuthenticatedUser;
#[cfg(server)]
use super::error::{AdminAuth, IntoServerFnError, MapServerFnError, ModelPermission};
#[cfg(server)]
use super::limits::{
	DEFAULT_RELATION_PAGE_SIZE, MAX_RELATION_LOOKUP_PAGE, MAX_RELATION_PAGE,
	MAX_RELATION_PAGE_SIZE, MAX_RELATION_QUERY_CHARS, MAX_RELATION_QUERY_LENGTH,
	RELATION_LOOKUP_PAGE_SIZE,
};
use crate::adapters::{
	AdminDatabase, AdminRecord, AdminSite, RelationLookupRequest, RelationLookupResponse,
	RelationOption,
};
#[cfg(server)]
use crate::core::{AdminDatabaseKey, AdminSiteKey, AdminUser, ModelAdmin};
#[cfg(server)]
use crate::server::type_inference::{
	ForeignKeyFieldMetadata, find_model_by_table_name, resolve_foreign_key_field_metadata,
};
use crate::types::ManyToManyLookupResponse;
#[cfg(server)]
use crate::types::RelationSelectorLayout;
#[cfg(server)]
use crate::types::{AdminError, AdminResult, RelationWidget};
#[cfg(server)]
use reinhardt_apps::{RelationshipMetadata, get_relationships_for_model};
#[cfg(server)]
use reinhardt_db::associations::ManyToManyManager;
#[cfg(server)]
use reinhardt_db::m2m_naming::{default_m2m_columns, default_through_table};
#[cfg(server)]
use reinhardt_db::migrations::FieldType as DatabaseFieldType;
#[cfg(server)]
use reinhardt_db::migrations::{FieldMetadata, ModelMetadata, ModelRegistry, global_registry};
#[cfg(server)]
use reinhardt_db::orm::execution::convert_values;
#[cfg(server)]
use reinhardt_db::orm::{DatabaseBackend, OrmExecutor, QueryRow};
#[cfg(server)]
use reinhardt_db::orm::{Filter, FilterCondition, FilterOperator, FilterValue};
#[cfg(server)]
use reinhardt_di::KeyedDepends;
#[cfg(server)]
use reinhardt_pages::server_fn::ServerFnRequest;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
#[cfg(server)]
use reinhardt_query::prelude::{
	Alias, Condition, Expr, ExprTrait, MySqlQueryBuilder, Order, PostgresQueryBuilder, Query,
	QueryBuilder, SelectStatement, SimpleExpr, SqliteQueryBuilder, Value, Values,
};
#[cfg(server)]
use std::collections::HashMap;
#[cfg(server)]
use std::collections::HashSet;
#[cfg(server)]
use std::sync::Arc;

#[cfg(server)]
pub(crate) struct ResolvedRelationField {
	pub(crate) foreign_key: ForeignKeyFieldMetadata,
	pub(crate) widget: RelationWidget,
	pub(crate) target_admin: Arc<dyn ModelAdmin>,
	pub(crate) target_field: String,
}
#[cfg(all(test, server))]
mod many_to_many_tests {

	use std::collections::HashMap;

	use super::{
		build_lookup_statement, build_select, relation_value, resolve_relation_with_registry,
		split_relation_values, sync_relation_ids, validate_lookup_bounds, validate_relation_ids,
		value_key,
	};
	use crate::core::{AdminSite, AdminUser, ModelAdmin};
	use crate::server::limits::MAX_RELATION_LOOKUP_PAGE;
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
		backend: DatabaseBackend,
		rows: Vec<Row>,
		fetch_calls: usize,
		fetches: Vec<(String, Vec<QueryValue>)>,
		executions: Vec<(String, Vec<QueryValue>)>,
	}

	impl RelationExecutor {
		fn with_ids(ids: &[&str]) -> Self {
			Self::with_backend_ids(DatabaseBackend::Postgres, ids)
		}

		fn with_backend_ids(backend: DatabaseBackend, ids: &[&str]) -> Self {
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
				backend,
				rows,
				fetch_calls: 0,
				fetches: Vec::new(),
				executions: Vec::new(),
			}
		}
	}

	#[async_trait::async_trait]
	impl OrmExecutor for RelationExecutor {
		fn backend(&self) -> DatabaseBackend {
			self.backend
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
			sql: &str,
			params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<Vec<Row>> {
			self.fetch_calls += 1;
			self.fetches.push((sql.to_string(), params));
			Ok(self.rows.clone())
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

	fn aliased_relation_descriptor() -> super::RelationDescriptor {
		let registry = ModelRegistry::new();
		let mut source = ModelMetadata::new("blog", "Article", "blog_articles");
		source.add_field(
			"title".to_string(),
			FieldMetadata::new(FieldType::VarChar(200)),
		);
		source.add_many_to_many(ManyToManyMetadata::new("tags", "Tag"));
		registry.register_model(source);

		let mut target = ModelMetadata::new("blog", "Tag", "taxonomy_tags");
		target.add_field(
			"object_id".to_string(),
			FieldMetadata::new(FieldType::BigInteger)
				.with_param("logical_name", "id")
				.with_param("db_column", "object_id"),
		);
		target.add_field(
			"display_name".to_string(),
			FieldMetadata::new(FieldType::VarChar(100))
				.with_param("logical_name", "name")
				.with_param("db_column", "display_name"),
		);
		registry.register_model(target);

		let site = AdminSite::new("Test");
		site.register(
			"Tag",
			TestAdmin::target("taxonomy_tags", vec!["id", "name"]),
		)
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
	fn normalize_relation_ids_accepts_existing_large_relations() {
		// Arrange
		let descriptor = normalization_descriptor();
		let values = (0..101).map(serde_json::Value::from).collect();
		let data = HashMap::from([("tags".to_string(), serde_json::Value::Array(values))]);

		// Act
		let result = split_relation_values(data, &[descriptor]);

		// Assert
		let (_, selections) = result.expect("existing relation selections must be accepted");
		assert_eq!(selections[0].ids.len(), 101);
	}

	#[rstest]
	fn relation_value_preserves_unsigned_u64_range() {
		// Arrange
		let mut metadata = ModelMetadata::new("taxonomy", "Tag", "taxonomy_tags");
		metadata.add_field(
			"object_id".to_string(),
			FieldMetadata::new(FieldType::Custom("u64".to_string()))
				.with_param("logical_name", "id")
				.with_param("db_column", "object_id"),
		);

		// Act
		let value = relation_value(&metadata, "id", &u64::MAX.to_string()).unwrap();

		// Assert
		assert_eq!(value, Value::BigUnsigned(Some(u64::MAX)));
	}

	#[rstest]
	fn relation_value_uses_unsigned_integer_metadata() {
		// Arrange
		let mut metadata = ModelMetadata::new("taxonomy", "Tag", "taxonomy_tags");
		metadata.add_field(
			"object_id".to_string(),
			FieldMetadata::new(FieldType::BigInteger)
				.with_param("logical_name", "id")
				.with_param("unsigned", "true"),
		);

		// Act
		let value = relation_value(&metadata, "id", &u64::MAX.to_string()).unwrap();

		// Assert
		assert_eq!(value, Value::BigUnsigned(Some(u64::MAX)));
	}

	#[rstest]
	fn relation_value_uses_non_integer_metadata() {
		// Arrange
		let mut metadata = ModelMetadata::new("taxonomy", "Tag", "taxonomy_tags");
		metadata.add_field("active".to_string(), FieldMetadata::new(FieldType::Boolean));
		metadata.add_field(
			"amount".to_string(),
			FieldMetadata::new(FieldType::Decimal {
				precision: 10,
				scale: 2,
			}),
		);
		metadata.add_field("ratio".to_string(), FieldMetadata::new(FieldType::Double));
		metadata.add_field("day".to_string(), FieldMetadata::new(FieldType::Date));

		// Act
		let active = relation_value(&metadata, "active", "true").unwrap();
		let amount = relation_value(&metadata, "amount", "12.50").unwrap();
		let negative_zero = relation_value(&metadata, "ratio", "-0").unwrap();
		let positive_zero = relation_value(&metadata, "ratio", "0").unwrap();
		let day = relation_value(&metadata, "day", "2026-08-13").unwrap();

		// Assert
		assert_eq!(active, Value::Bool(Some(true)));
		assert_eq!(
			amount,
			Value::BigDecimal(Some(Box::new("12.50".parse().unwrap())))
		);
		assert_eq!(
			day,
			Value::ChronoDate(Some(Box::new(
				chrono::NaiveDate::from_ymd_opt(2026, 8, 13).unwrap(),
			)))
		);
		assert_eq!(
			value_key(&amount),
			value_key(&relation_value(&metadata, "amount", "12.5").unwrap())
		);
		assert_eq!(value_key(&negative_zero), value_key(&positive_zero));
	}

	#[rstest]
	fn normalize_relation_ids_preserves_whitespace_for_text_keys() {
		// Arrange
		let mut descriptor = normalization_descriptor();
		descriptor
			.target_metadata
			.add_field("id".to_string(), FieldMetadata::new(FieldType::VarChar(32)));
		let data = HashMap::from([("tags".to_string(), serde_json::json!([" 001 "]))]);

		// Act
		let (_, selections) = split_relation_values(data, &[descriptor]).unwrap();

		// Assert
		assert_eq!(selections[0].ids, [" 001 "]);
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
	async fn relation_validation_batches_large_selections() {
		// Arrange
		let descriptor = normalization_descriptor();
		let ids: Vec<String> = (0..501).map(|id| id.to_string()).collect();
		let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
		let mut executor = RelationExecutor::with_ids(&id_refs);

		// Act
		let result = validate_relation_ids(&mut executor, &descriptor, &ids).await;

		// Assert
		assert!(result.is_ok());
		assert_eq!(executor.fetch_calls, 2);
	}

	#[rstest]
	#[tokio::test]
	async fn relation_sync_changes_only_the_set_difference() {
		// Arrange
		let descriptor = normalization_descriptor();
		let mut executor = RelationExecutor::with_ids(&["1", "2"]);

		// Act
		let changed = sync_relation_ids(
			&mut executor,
			&descriptor,
			Value::Int(Some(7)),
			&["2".to_string(), "3".to_string()],
		)
		.await
		.unwrap();

		// Assert
		assert!(changed);
		assert_eq!(executor.fetch_calls, 1);
		assert_eq!(
			executor.fetches[0].0,
			"SELECT \"blog_articles_tags\".\"taxonomy_tags_id\" AS \"id\" FROM \"blog_articles_tags\" WHERE \"blog_articles_tags\".\"blog_articles_id\" = $1"
		);
		assert_eq!(executor.executions.len(), 2);
		assert_eq!(
			executor.executions[0].0,
			"DELETE FROM \"blog_articles_tags\" WHERE \"blog_articles_id\" = $1 AND \"taxonomy_tags_id\" = $2"
		);
		assert_eq!(
			executor.executions[0].1,
			vec![QueryValue::Int(7), QueryValue::String("1".to_string())]
		);
		assert_eq!(
			executor.executions[1].0,
			"INSERT INTO \"blog_articles_tags\" (\"blog_articles_id\", \"taxonomy_tags_id\") VALUES ($1, $2) ON CONFLICT (\"blog_articles_id\", \"taxonomy_tags_id\") DO NOTHING"
		);
		assert_eq!(
			executor.executions[1].1,
			vec![QueryValue::Int(7), QueryValue::String("3".to_string())]
		);
	}

	#[rstest]
	#[tokio::test]
	async fn mysql_relation_sync_rejects_unpersisted_pairs() {
		// Arrange
		let descriptor = normalization_descriptor();
		let mut executor = RelationExecutor::with_backend_ids(DatabaseBackend::MySql, &["2"]);

		// Act
		let result = sync_relation_ids(
			&mut executor,
			&descriptor,
			Value::Int(Some(7)),
			&["2".to_string(), "3".to_string()],
		)
		.await;

		// Assert
		assert!(matches!(result, Err(AdminError::ValidationError(_))));
		assert_eq!(executor.fetch_calls, 2);
		assert_eq!(executor.executions.len(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn current_relation_options_deduplicate_without_sql_distinct() {
		// Arrange
		let descriptor = normalization_descriptor();
		let option_row = || {
			let mut row = Row::new();
			row.data.insert("id".to_string(), QueryValue::Int(1));
			row.data
				.insert("name".to_string(), QueryValue::String("Tag".to_string()));
			row
		};
		let mut executor = RelationExecutor {
			backend: DatabaseBackend::Postgres,
			rows: vec![option_row(), option_row()],
			fetch_calls: 0,
			fetches: Vec::new(),
			executions: Vec::new(),
		};

		// Act
		let options = super::current_relation_options(&descriptor, "7", &mut executor)
			.await
			.unwrap();

		// Assert
		assert_eq!(options.len(), 1);
		assert_eq!(options[0].id, "1");
		assert!(!executor.fetches[0].0.contains("DISTINCT"));
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
	fn resolver_uses_target_table_when_admin_route_is_aliased() {
		// Arrange
		let registry = relation_registry("Tag", "blog");
		let site = AdminSite::new("Test");
		site.register(
			"tag-route",
			TestAdmin::target("taxonomy_tags", vec!["name"]),
		)
		.unwrap();
		let source_admin = TestAdmin::source(vec!["tags"], Vec::new());

		// Act
		let result = resolve_relation_with_registry(&site, &source_admin, "tags", &registry);

		// Assert
		assert_eq!(result.unwrap().target_admin.table_name(), "taxonomy_tags");
	}

	#[rstest]
	fn lookup_bounds_reject_page_zero() {
		assert!(matches!(
			validate_lookup_bounds("", 0),
			Err(AdminError::ValidationError(_))
		));
	}

	#[rstest]
	fn lookup_bounds_enforce_maximum_page() {
		// Act and assert
		assert!(validate_lookup_bounds("", MAX_RELATION_LOOKUP_PAGE).is_ok());
		assert!(matches!(
			validate_lookup_bounds("", MAX_RELATION_LOOKUP_PAGE + 1),
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
		let statement =
			build_lookup_statement(&descriptor, r#"50%_\off"#, 1, DatabaseBackend::Sqlite).unwrap();
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

	#[rstest]
	fn lookup_resolves_aliased_fields_and_casts_non_text_search_fields() {
		// Arrange
		let descriptor = aliased_relation_descriptor();

		// Act
		let statement =
			build_lookup_statement(&descriptor, "7", 1, DatabaseBackend::Postgres).unwrap();
		let (postgres_sql, _) = build_select(&statement, DatabaseBackend::Postgres);
		let statement =
			build_lookup_statement(&descriptor, "7", 1, DatabaseBackend::MySql).unwrap();
		let (mysql_sql, _) = build_select(&statement, DatabaseBackend::MySql);
		let statement =
			build_lookup_statement(&descriptor, "7", 1, DatabaseBackend::Sqlite).unwrap();
		let (sqlite_sql, _) = build_select(&statement, DatabaseBackend::Sqlite);

		// Assert
		assert_eq!(
			postgres_sql,
			r#"SELECT "object_id" AS "id", "display_name" AS "name" FROM "taxonomy_tags" WHERE (CAST("object_id" AS TEXT) LIKE $1 ESCAPE '\' OR "display_name" LIKE $2 ESCAPE '\') ORDER BY "object_id" DESC LIMIT $3 OFFSET $4"#
		);
		assert_eq!(
			mysql_sql,
			r#"SELECT `object_id` AS `id`, `display_name` AS `name` FROM `taxonomy_tags` WHERE (CAST(`object_id` AS CHAR) LIKE ? ESCAPE 0x5C OR `display_name` LIKE ? ESCAPE 0x5C) ORDER BY `object_id` DESC LIMIT ? OFFSET ?"#
		);
		assert_eq!(
			sqlite_sql,
			r#"SELECT "object_id" AS "id", "display_name" AS "name" FROM "taxonomy_tags" WHERE (CAST("object_id" AS TEXT) LIKE ? ESCAPE '\' OR "display_name" LIKE ? ESCAPE '\') ORDER BY "object_id" DESC LIMIT ? OFFSET ?"#
		);
	}

	#[rstest]
	fn postgres_enum_search_fields_require_cast() {
		// Arrange
		let mut descriptor = aliased_relation_descriptor();
		descriptor
			.target_metadata
			.fields
			.get_mut("display_name")
			.unwrap()
			.field_type = FieldType::Enum {
			values: vec!["active".to_string(), "inactive".to_string()],
		};

		// Act
		let statement =
			build_lookup_statement(&descriptor, "active", 1, DatabaseBackend::Postgres).unwrap();
		let (postgres_sql, _) = build_select(&statement, DatabaseBackend::Postgres);
		let statement =
			build_lookup_statement(&descriptor, "active", 1, DatabaseBackend::MySql).unwrap();
		let (mysql_sql, _) = build_select(&statement, DatabaseBackend::MySql);

		// Assert
		assert_eq!(
			postgres_sql,
			r#"SELECT "object_id" AS "id", "display_name" AS "name" FROM "taxonomy_tags" WHERE (CAST("object_id" AS TEXT) LIKE $1 ESCAPE '\' OR CAST("display_name" AS TEXT) LIKE $2 ESCAPE '\') ORDER BY "object_id" DESC LIMIT $3 OFFSET $4"#
		);
		assert_eq!(
			mysql_sql,
			r#"SELECT `object_id` AS `id`, `display_name` AS `name` FROM `taxonomy_tags` WHERE (CAST(`object_id` AS CHAR) LIKE ? ESCAPE 0x5C OR `display_name` LIKE ? ESCAPE 0x5C) ORDER BY `object_id` DESC LIMIT ? OFFSET ?"#
		);
	}
}
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
const RELATION_VALIDATION_BATCH_SIZE: usize = 500;

#[cfg(server)]
fn validation_error(message: impl Into<String>) -> AdminError {
	AdminError::ValidationError(message.into())
}

#[cfg(server)]
pub(crate) fn validate_lookup_bounds(query: &str, page: u64) -> AdminResult<u64> {
	if page == 0 {
		return Err(validation_error("relation lookup page must be at least 1"));
	}
	if page > MAX_RELATION_LOOKUP_PAGE {
		return Err(validation_error("relation lookup page is too large"));
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
	if source_admin.readonly_fields().contains(&field_name) {
		return Err(validation_error(
			"read-only fields cannot use relation selectors",
		));
	}
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
	let target_admin = site.get_model_admin_by_table_name(&target_metadata.table_name)?;
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
	if relation.through.is_some()
		&& registry.get_models().into_iter().any(|metadata| {
			metadata.table_name == through_table || metadata.model_name == through_table
		}) {
		return Err(validation_error(
			"explicit through models must be managed separately from relation selectors",
		));
	}
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
		let mut ids = Vec::with_capacity(values.len());
		let mut seen = HashSet::with_capacity(values.len());
		let preserve_text_key = field_entry(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
		)
		.is_some_and(|(_, field)| supports_text_search(&field.field_type));
		for value in values {
			let id = match value {
				serde_json::Value::String(value) => {
					if value.trim().is_empty() {
						return Err(validation_error(format!(
							"relation field '{}' contains an empty identifier",
							descriptor.field_name
						)));
					}
					if preserve_text_key {
						value
					} else {
						value.trim().to_string()
					}
				}
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
fn field_entry<'a>(
	metadata: &'a ModelMetadata,
	field_name: &str,
) -> Option<(&'a str, &'a FieldMetadata)> {
	if let Some((column, field)) = metadata.fields.get_key_value(field_name) {
		return Some((column.as_str(), field));
	}

	metadata.fields.iter().find_map(|(column, field)| {
		(field
			.params
			.get("logical_name")
			.is_some_and(|name| name == field_name)
			|| field
				.params
				.get("rust_field_name")
				.is_some_and(|name| name == field_name)
			|| field
				.params
				.get("db_column")
				.is_some_and(|column_name| column_name == field_name))
		.then_some((column.as_str(), field))
	})
}

#[cfg(server)]
fn logical_column(metadata: &ModelMetadata, field_name: &str) -> String {
	field_entry(metadata, field_name)
		.map(|(column, field)| {
			field
				.params
				.get("logical_name")
				.cloned()
				.or_else(|| field.params.get("rust_field_name").cloned())
				.unwrap_or_else(|| column.to_string())
		})
		.unwrap_or_else(|| field_name.to_string())
}

#[cfg(server)]
fn target_pk_field(descriptor: &RelationDescriptor) -> String {
	logical_column(
		&descriptor.target_metadata,
		descriptor.target_admin.pk_field(),
	)
}

#[cfg(server)]
fn selected_columns(descriptor: &RelationDescriptor) -> Vec<String> {
	let pk_field = target_pk_field(descriptor);
	let mut columns = vec![pk_field.clone()];
	let mut fields = descriptor
		.target_metadata
		.fields
		.iter()
		.map(|(column, field)| {
			field
				.params
				.get("logical_name")
				.cloned()
				.or_else(|| field.params.get("rust_field_name").cloned())
				.unwrap_or_else(|| column.clone())
		})
		.filter(|field| field != &pk_field)
		.collect::<Vec<_>>();
	fields.sort();
	fields.dedup();
	columns.extend(fields);
	columns
}

#[cfg(server)]
fn physical_column(metadata: &ModelMetadata, field: &str) -> String {
	field_entry(metadata, field)
		.map(|(column, field_metadata)| {
			field_metadata
				.params
				.get("db_column")
				.cloned()
				.unwrap_or_else(|| column.to_string())
		})
		.unwrap_or_else(|| field.to_string())
}

#[cfg(server)]
fn supports_text_search(field_type: &DatabaseFieldType) -> bool {
	matches!(
		field_type,
		DatabaseFieldType::Char(_)
			| DatabaseFieldType::VarChar(_)
			| DatabaseFieldType::Text
			| DatabaseFieldType::TinyText
			| DatabaseFieldType::MediumText
			| DatabaseFieldType::LongText
			| DatabaseFieldType::Enum { .. }
			| DatabaseFieldType::Set { .. }
			| DatabaseFieldType::CIText
	)
}

#[cfg(server)]
fn requires_search_cast(field_type: &DatabaseFieldType, backend: DatabaseBackend) -> bool {
	!supports_text_search(field_type)
		|| (matches!(backend, DatabaseBackend::Postgres)
			&& matches!(field_type, DatabaseFieldType::Enum { .. }))
}

#[cfg(server)]
fn select_relation_columns(
	statement: &mut SelectStatement,
	descriptor: &RelationDescriptor,
	table: Option<Alias>,
) {
	for column in selected_columns(descriptor) {
		let physical = physical_column(&descriptor.target_metadata, &column);
		let expr = table
			.clone()
			.map(|table| Expr::col((table, Alias::new(&physical))))
			.unwrap_or_else(|| Expr::col(Alias::new(&physical)));
		if physical == column {
			statement.expr(expr);
		} else {
			statement.expr_as(expr, Alias::new(&column));
		}
	}
}

#[cfg(server)]
fn build_lookup_statement(
	descriptor: &RelationDescriptor,
	query: &str,
	page: u64,
	backend: DatabaseBackend,
) -> AdminResult<SelectStatement> {
	let offset = validate_lookup_bounds(query, page)?;
	let mut statement = Query::select();
	statement.from(Alias::new(&descriptor.target_metadata.table_name));
	select_relation_columns(&mut statement, descriptor, None);
	if !query.is_empty() {
		let mut condition = Condition::any();
		for field in descriptor.target_admin.search_fields() {
			let physical = physical_column(&descriptor.target_metadata, field);
			let expression = Expr::col(Alias::new(&physical));
			let expression = field_entry(&descriptor.target_metadata, field)
				.map(|(_, metadata)| &metadata.field_type)
				.filter(|field_type| requires_search_cast(field_type, backend))
				.map_or(expression.clone(), |_| {
					let cast_type = match backend {
						DatabaseBackend::MySql => "CHAR",
						DatabaseBackend::Postgres | DatabaseBackend::Sqlite => "TEXT",
					};
					SimpleExpr::CustomWithExpr(
						format!("CAST(? AS {cast_type})"),
						vec![expression.clone().into()],
					)
					.into()
				});
			condition = condition.add(expression.contains(query.to_owned()));
		}
		statement.cond_where(condition);
	}
	let mut ordering = descriptor
		.target_admin
		.ordering()
		.into_iter()
		.filter_map(|field| target_ordering_name(&descriptor.target_metadata, field))
		.collect::<Vec<_>>();
	let target_pk = physical_column(
		&descriptor.target_metadata,
		descriptor.target_admin.pk_field(),
	);
	if !ordering
		.iter()
		.any(|field| field.trim_start_matches('-') == target_pk)
	{
		ordering.push(target_pk);
	}
	for field in ordering {
		let (field, order) = field
			.strip_prefix('-')
			.map_or((field.as_str(), Order::Asc), |field| (field, Order::Desc));
		statement.order_by(Alias::new(field), order);
	}
	statement
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
	let pk_field = target_pk_field(descriptor);
	let value = record
		.get(&pk_field)
		.and_then(scalar_string)
		.ok_or_else(|| validation_error("relation target has an invalid primary key"))?;
	Ok(RelationOption::new(
		value.clone(),
		descriptor
			.target_admin
			.object_label(record)
			.unwrap_or(value),
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
) -> AdminResult<ManyToManyLookupResponse> {
	let statement = build_lookup_statement(descriptor, query, page, executor.backend())?;
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
	Ok(ManyToManyLookupResponse {
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
	let Some((_, field)) = field_entry(metadata, field_name) else {
		return Ok(Value::from(input.to_string()));
	};
	let unsigned = field
		.params
		.get("unsigned")
		.is_some_and(|value| value == "true");

	match (&field.field_type, unsigned) {
		(DatabaseFieldType::Custom(kind), _) if kind.eq_ignore_ascii_case("u64") => input
			.parse::<u64>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Custom(kind), _) if kind.eq_ignore_ascii_case("u32") => input
			.parse::<u32>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Custom(kind), _) if kind.eq_ignore_ascii_case("u16") => input
			.parse::<u16>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Custom(kind), _) if kind.eq_ignore_ascii_case("u8") => input
			.parse::<u8>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::BigInteger, true) => input
			.parse::<u64>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::BigInteger, false) => input
			.parse::<i64>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Integer | DatabaseFieldType::MediumInt, true) => input
			.parse::<u32>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::SmallInteger, true) => input
			.parse::<u16>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::TinyInt, true) => input
			.parse::<u8>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(
			DatabaseFieldType::Integer
			| DatabaseFieldType::SmallInteger
			| DatabaseFieldType::TinyInt
			| DatabaseFieldType::MediumInt,
			false,
		) => input
			.parse::<i32>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Uuid, _) => input
			.parse::<uuid::Uuid>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Boolean, _) => input
			.parse::<bool>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Year, _) => input
			.parse::<i32>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Decimal { .. }, _) => input
			.parse()
			.map(|value| Value::BigDecimal(Some(Box::new(value))))
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Float, _) => input
			.parse::<f32>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Double | DatabaseFieldType::Real, _) => input
			.parse::<f64>()
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Date, _) => chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d")
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::Time, _) => chrono::NaiveTime::parse_from_str(input, "%H:%M:%S%.f")
			.or_else(|_| chrono::NaiveTime::parse_from_str(input, "%H:%M"))
			.map(Value::from)
			.map_err(|_| validation_error("relation identifier has an invalid type")),
		(DatabaseFieldType::DateTime, _) => {
			chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S%.f")
				.or_else(|_| chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S%.f"))
				.or_else(|_| {
					chrono::DateTime::parse_from_rfc3339(input).map(|value| value.naive_utc())
				})
				.map(Value::from)
				.map_err(|_| validation_error("relation identifier has an invalid type"))
		}
		(DatabaseFieldType::TimestampTz, _) => chrono::DateTime::parse_from_rfc3339(input)
			.map(|value| value.with_timezone(&chrono::Utc))
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
	statement.from(target_table.clone());
	select_relation_columns(&mut statement, descriptor, Some(target_table.clone()));
	statement
		.inner_join(
			through_table.clone(),
			Expr::col((
				target_table.clone(),
				Alias::new(physical_column(
					&descriptor.target_metadata,
					descriptor.target_admin.pk_field(),
				)),
			))
			.equals((through_table.clone(), Alias::new(&descriptor.target_column))),
		)
		.and_where(
			Expr::col((through_table, Alias::new(&descriptor.source_column))).eq(source_value),
		)
		.order_by(
			(
				target_table,
				Alias::new(physical_column(
					&descriptor.target_metadata,
					descriptor.target_admin.pk_field(),
				)),
			),
			Order::Asc,
		);
	let (sql, values) = build_select(&statement, executor.backend());
	let rows = executor
		.fetch_all(&sql, convert_values(values))
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	let mut seen = HashSet::new();
	let mut options = Vec::new();
	for record in rows_to_records(rows) {
		let option = record_option(descriptor, &record)?;
		if seen.insert(option.id.clone()) {
			options.push(option);
		}
	}
	Ok(options)
}

#[cfg(server)]
pub(crate) async fn lock_relation_source<E: OrmExecutor>(
	executor: &mut E,
	descriptor: &RelationDescriptor,
	source_id: &str,
) -> AdminResult<()> {
	if executor.backend() == DatabaseBackend::Sqlite {
		return Ok(());
	}

	let source_value = relation_value(
		&descriptor.source_metadata,
		&descriptor.source_pk_field,
		source_id,
	)?;
	let source_pk = physical_column(&descriptor.source_metadata, &descriptor.source_pk_field);
	let mut statement = Query::select();
	statement
		.from(Alias::new(&descriptor.source_metadata.table_name))
		.column(Alias::new(&source_pk))
		.and_where(Expr::col(Alias::new(&source_pk)).eq(source_value))
		.lock_exclusive();
	let (sql, values) = build_select(&statement, executor.backend());
	let rows = executor
		.fetch_all(&sql, convert_values(values))
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	if rows.is_empty() {
		return Err(AdminError::ModelNotRegistered(format!(
			"{} not found",
			descriptor.source_metadata.model_name
		)));
	}
	Ok(())
}

#[cfg(server)]
pub(crate) async fn validate_relation_ids<E: OrmExecutor>(
	executor: &mut E,
	descriptor: &RelationDescriptor,
	ids: &[String],
) -> AdminResult<()> {
	let mut expected = Vec::with_capacity(ids.len());
	let mut expected_keys = HashSet::with_capacity(ids.len());
	for id in ids {
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			id,
		)?;
		if expected_keys.insert(value_key(&value)) {
			expected.push(value);
		}
	}
	if expected.is_empty() {
		return Ok(());
	}
	let target_pk = physical_column(
		&descriptor.target_metadata,
		descriptor.target_admin.pk_field(),
	);
	let target_pk_name = target_pk_field(descriptor);
	let mut returned = Vec::with_capacity(expected.len());
	let mut returned_keys = HashSet::with_capacity(expected.len());
	for expected_batch in expected.chunks(RELATION_VALIDATION_BATCH_SIZE) {
		let mut statement = Query::select();
		statement.from(Alias::new(&descriptor.target_metadata.table_name));
		if target_pk == target_pk_name {
			statement.column(Alias::new(&target_pk));
		} else {
			statement.expr_as(
				Expr::col(Alias::new(&target_pk)),
				Alias::new(&target_pk_name),
			);
		}
		statement.and_where(Expr::col(Alias::new(&target_pk)).is_in(expected_batch.to_vec()));
		let (sql, values) = build_select(&statement, executor.backend());
		let rows = executor
			.fetch_all(&sql, convert_values(values))
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
		for row in rows {
			let row = QueryRow::from_backend_row(row);
			let id = row
				.data
				.get(&target_pk_name)
				.and_then(scalar_string)
				.ok_or_else(|| validation_error("relation target has an invalid primary key"))?;
			let value = relation_value(
				&descriptor.target_metadata,
				descriptor.target_admin.pk_field(),
				&id,
			)?;
			if returned_keys.insert(value_key(&value)) {
				returned.push(value);
			}
		}
	}
	if returned_keys != expected_keys {
		return Err(validation_error(
			"one or more relation selections are invalid",
		));
	}
	Ok(())
}

#[cfg(server)]
async fn current_relation_ids<E: OrmExecutor>(
	executor: &mut E,
	descriptor: &RelationDescriptor,
	source_pk: &Value,
) -> AdminResult<Vec<Value>> {
	let through_table = Alias::new(&descriptor.through_table);
	let target_pk_name = target_pk_field(descriptor);
	let mut statement = Query::select();
	statement
		.from(through_table.clone())
		.expr_as(
			Expr::col((through_table.clone(), Alias::new(&descriptor.target_column))),
			Alias::new(&target_pk_name),
		)
		.and_where(
			Expr::col((through_table, Alias::new(&descriptor.source_column))).eq(source_pk.clone()),
		);
	let (sql, values) = build_select(&statement, executor.backend());
	let rows = executor
		.fetch_all(&sql, convert_values(values))
		.await
		.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	let mut current = Vec::with_capacity(rows.len());
	let mut current_keys = HashSet::with_capacity(rows.len());
	for row in rows {
		let row = QueryRow::from_backend_row(row);
		let id = row
			.data
			.get(&target_pk_name)
			.and_then(scalar_string)
			.ok_or_else(|| validation_error("relation target has an invalid primary key"))?;
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			&id,
		)?;
		if current_keys.insert(value_key(&value)) {
			current.push(value);
		}
	}
	Ok(current)
}

#[cfg(server)]
pub(crate) async fn relation_selection_is_unchanged<E: OrmExecutor>(
	executor: &mut E,
	descriptor: &RelationDescriptor,
	source_pk: &Value,
	ids: &[String],
) -> AdminResult<bool> {
	let mut desired_keys = HashSet::with_capacity(ids.len());
	for id in ids {
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			id,
		)?;
		desired_keys.insert(value_key(&value));
	}
	let current_keys = current_relation_ids(executor, descriptor, source_pk)
		.await?
		.iter()
		.map(value_key)
		.collect::<HashSet<_>>();
	Ok(current_keys == desired_keys)
}

#[cfg(server)]
pub(crate) async fn sync_relation_ids<E: OrmExecutor>(
	executor: &mut E,
	descriptor: &RelationDescriptor,
	source_pk: Value,
	ids: &[String],
) -> AdminResult<bool> {
	let mut desired = Vec::with_capacity(ids.len());
	let mut desired_keys = HashSet::with_capacity(ids.len());
	for id in ids {
		let value = relation_value(
			&descriptor.target_metadata,
			descriptor.target_admin.pk_field(),
			id,
		)?;
		if desired_keys.insert(value_key(&value)) {
			desired.push(value);
		}
	}

	let source_value = source_pk.clone();
	let manager = ManyToManyManager::<(), (), Value>::new(
		source_pk,
		descriptor.through_table.clone(),
		descriptor.source_column.clone(),
		descriptor.target_column.clone(),
	);
	let current = current_relation_ids(executor, descriptor, &source_value).await?;
	let current_keys: HashSet<String> = current.iter().map(value_key).collect();
	let changed = current_keys != desired_keys;

	for value in current
		.iter()
		.filter(|value| !desired_keys.contains(&value_key(value)))
	{
		manager
			.remove_with_db(executor, value.clone())
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	}
	for value in desired
		.iter()
		.filter(|value| !current_keys.contains(&value_key(value)))
	{
		manager
			.add_with_db(executor, value.clone())
			.await
			.map_err(|error| AdminError::DatabaseError(error.to_string()))?;
	}
	if matches!(executor.backend(), DatabaseBackend::MySql) {
		let persisted = current_relation_ids(executor, descriptor, &source_value).await?;
		let persisted_keys: HashSet<String> = persisted.iter().map(value_key).collect();
		if persisted_keys != desired_keys {
			return Err(validation_error(
				"one or more relation selections could not be persisted",
			));
		}
	}
	Ok(changed)
}

#[cfg(server)]
fn value_key(value: &Value) -> String {
	match value {
		Value::BigDecimal(Some(value)) => format!("BigDecimal:{}", value.normalized()),
		Value::Float(Some(value)) if *value == 0.0 => "Float:0".to_string(),
		Value::Double(Some(value)) if *value == 0.0 => "Double:0".to_string(),
		_ => format!("{value:?}"),
	}
}

/// Look up a bounded page of options for a configured many-to-many selector.
// The server-function transport requires explicit request, authentication, and DI inputs.
#[allow(clippy::too_many_arguments)]
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
) -> Result<ManyToManyLookupResponse, ServerFnError> {
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

#[cfg(server)]
fn target_field_entry<'a>(
	model: &'a ModelMetadata,
	field_name: &str,
) -> Option<(&'a str, &'a FieldMetadata)> {
	if let Some((column, field)) = model.fields.get_key_value(field_name) {
		return Some((column.as_str(), field));
	}

	model.fields.iter().find_map(|(column, field)| {
		(field
			.params
			.get("rust_field_name")
			.is_some_and(|name| name == field_name)
			|| field
				.params
				.get("logical_name")
				.is_some_and(|name| name == field_name)
			|| field
				.params
				.get("db_column")
				.is_some_and(|name| name == field_name))
		.then_some((column.as_str(), field))
	})
}

#[cfg(server)]
fn target_column_name(model: &ModelMetadata, field_name: &str) -> Option<String> {
	target_field_entry(model, field_name).map(|(column, field)| {
		field
			.params
			.get("db_column")
			.cloned()
			.unwrap_or_else(|| column.to_string())
	})
}

#[cfg(server)]
fn target_field_metadata<'a>(
	model: &'a ModelMetadata,
	field_name: &str,
) -> Option<&'a reinhardt_db::migrations::FieldMetadata> {
	target_field_entry(model, field_name).map(|(_, field)| field)
}

#[cfg(server)]
fn target_ordering_name(model: &ModelMetadata, field: &str) -> Option<String> {
	let descending = field.starts_with('-');
	let name = field.trim_start_matches('-');
	let column = target_column_name(model, name)?;
	if descending {
		Some(format!("-{column}"))
	} else {
		Some(column)
	}
}

#[cfg(server)]
pub(crate) fn validate_relation_configuration(
	site: &AdminSite,
	source_admin: &Arc<dyn ModelAdmin>,
	source_model: &ModelMetadata,
	relationships: &[&RelationshipMetadata],
	registry: &ModelRegistry,
) -> AdminResult<Vec<ResolvedRelationField>> {
	let configured_fields = source_admin
		.autocomplete_fields()
		.into_iter()
		.map(|field| (field, RelationWidget::Autocomplete))
		.chain(
			source_admin
				.raw_id_fields()
				.into_iter()
				.map(|field| (field, RelationWidget::RawId)),
		);
	let mut seen_columns = HashMap::new();
	let mut resolved_fields = Vec::new();

	for (configured_name, widget) in configured_fields {
		let foreign_key = resolve_foreign_key_field_metadata(
			source_model,
			configured_name,
			relationships,
			registry,
		)?;
		if let Some(previous_name) =
			seen_columns.insert(foreign_key.column_name.clone(), configured_name)
		{
			return Err(AdminError::ValidationError(format!(
				"Relation fields '{}' and '{}' both resolve to column '{}'",
				previous_name, configured_name, foreign_key.column_name
			)));
		}

		let target_name = foreign_key.target_model.model_name.as_str();
		let target_admin = if let Ok(admin) = site.get_model_admin(target_name) {
			if admin.table_name() != foreign_key.target_model.table_name {
				return Err(AdminError::ValidationError(format!(
					"Related admin '{}' uses table '{}', expected '{}'",
					target_name,
					admin.table_name(),
					foreign_key.target_model.table_name
				)));
			}
			admin
		} else {
			let matches = site
				.registered_models()
				.into_iter()
				.filter_map(|name| site.get_model_admin(&name).ok())
				.filter(|admin| admin.table_name() == foreign_key.target_model.table_name)
				.collect::<Vec<_>>();
			match matches.as_slice() {
				[admin] => admin.clone(),
				[] => {
					return Err(AdminError::ValidationError(format!(
						"Related admin '{}' for field '{}' is not registered",
						target_name, foreign_key.logical_name
					)));
				}
				_ => {
					return Err(AdminError::ValidationError(format!(
						"Related table '{}' has more than one registered admin",
						foreign_key.target_model.table_name
					)));
				}
			}
		};
		let target_field = foreign_key
			.target_field
			.as_deref()
			.map(|field| {
				target_column_name(&foreign_key.target_model, field).ok_or_else(|| {
					AdminError::ValidationError(format!(
						"Target field '{}' for relation '{}' is not registered",
						field, foreign_key.logical_name
					))
				})
			})
			.transpose()?
			.unwrap_or_else(|| {
				target_column_name(&foreign_key.target_model, target_admin.pk_field())
					.unwrap_or_else(|| target_admin.pk_field().to_string())
			});
		if widget == RelationWidget::Autocomplete && target_admin.search_fields().is_empty() {
			return Err(AdminError::ValidationError(format!(
				"Related admin '{}' for field '{}' must configure search_fields for autocomplete",
				target_name, foreign_key.logical_name
			)));
		}

		resolved_fields.push(ResolvedRelationField {
			foreign_key,
			widget,
			target_admin,
			target_field,
		});
	}

	Ok(resolved_fields)
}

#[cfg(server)]
pub(crate) fn resolve_relation_configuration(
	site: &AdminSite,
	source_admin: &Arc<dyn ModelAdmin>,
) -> AdminResult<Vec<ResolvedRelationField>> {
	if source_admin.autocomplete_fields().is_empty() && source_admin.raw_id_fields().is_empty() {
		return Ok(Vec::new());
	}

	let source_model = find_model_by_table_name(source_admin.table_name()).ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Model metadata for admin '{}' is not registered",
			source_admin.model_name()
		))
	})?;
	let qualified_source_name = format!("{}.{}", source_model.app_label, source_model.model_name);
	let relationships = get_relationships_for_model(&qualified_source_name);

	validate_relation_configuration(
		site,
		source_admin,
		&source_model,
		&relationships,
		global_registry(),
	)
}

#[cfg(server)]
pub(crate) fn relation_field_aliases(
	site: &AdminSite,
	source_admin: &Arc<dyn ModelAdmin>,
) -> AdminResult<Vec<(String, String)>> {
	Ok(resolve_relation_configuration(site, source_admin)?
		.into_iter()
		.filter_map(|relation| {
			let logical_name = relation.foreign_key.logical_name;
			let column_name = relation.foreign_key.column_name;
			(logical_name != column_name).then_some((logical_name, column_name))
		})
		.collect())
}

#[cfg(server)]
fn find_configured_relation<'a>(
	relations: &'a [ResolvedRelationField],
	field_name: &str,
) -> AdminResult<&'a ResolvedRelationField> {
	relations
		.iter()
		.find(|relation| {
			relation.foreign_key.logical_name == field_name
				|| relation.foreign_key.column_name == field_name
		})
		.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Field '{field_name}' is not configured as an admin relation"
			))
		})
}

#[cfg(server)]
async fn require_related_view_permission(
	auth: &AdminAuth,
	user: &dyn AdminUser,
	relation: &ResolvedRelationField,
) -> Result<(), ServerFnError> {
	auth.require_model_permission(relation.target_admin.as_ref(), user, ModelPermission::View)
		.await
}

#[cfg(server)]
async fn fetch_related_record(
	db: &AdminDatabase,
	relation: &ResolvedRelationField,
	id: &str,
) -> Result<HashMap<String, serde_json::Value>, ServerFnError> {
	db.get::<AdminRecord>(
		relation.target_admin.table_name(),
		&relation.target_field,
		id,
	)
	.await
	.map_server_fn_error()?
	.ok_or_else(|| {
		AdminError::ValidationError(format!(
			"Related object '{}' with id '{}' does not exist",
			relation.target_admin.model_name(),
			id
		))
	})
	.map_server_fn_error()
}

#[cfg(server)]
pub(crate) fn relation_id_from_value(value: &serde_json::Value) -> AdminResult<Option<String>> {
	match value {
		serde_json::Value::Null => Ok(None),
		serde_json::Value::String(value) => Ok(Some(value.clone())),
		serde_json::Value::Number(value) => Ok(Some(value.to_string())),
		serde_json::Value::Bool(_) | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
			Err(AdminError::ValidationError(
				"Relation primary keys must be scalar values".to_string(),
			))
		}
	}
}

#[cfg(server)]
fn relation_option_from_record(
	relation: &ResolvedRelationField,
	record: &HashMap<String, serde_json::Value>,
) -> AdminResult<RelationOption> {
	let target_field = relation.target_field.as_str();
	let id = record
		.get(target_field)
		.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Related object is missing relation target field '{target_field}'"
			))
		})
		.and_then(relation_id_from_value)?
		.ok_or_else(|| {
			AdminError::ValidationError(format!(
				"Related object relation target field '{target_field}' cannot be null"
			))
		})?;
	let label = relation
		.target_admin
		.object_label(record)
		.unwrap_or_else(|| id.clone());

	Ok(RelationOption { id, label })
}

#[cfg(server)]
fn relation_id_limit(relation: &ResolvedRelationField) -> usize {
	let configured_limit =
		target_field_metadata(&relation.foreign_key.target_model, &relation.target_field).and_then(
			|field| match field.field_type {
				reinhardt_db::migrations::FieldType::Char(length)
				| reinhardt_db::migrations::FieldType::VarChar(length) => usize::try_from(length).ok(),
				_ => None,
			},
		);
	configured_limit.unwrap_or(MAX_RELATION_QUERY_LENGTH)
}

#[cfg(server)]
fn validate_relation_id(relation: &ResolvedRelationField, id: &str) -> AdminResult<()> {
	if id.is_empty() {
		return Err(AdminError::ValidationError(
			"Relation id cannot be empty".to_string(),
		));
	}
	let limit = relation_id_limit(relation);
	if id.chars().count() > limit {
		return Err(AdminError::ValidationError(format!(
			"Relation id exceeds maximum length of {limit} characters"
		)));
	}
	Ok(())
}

#[cfg(server)]
pub(crate) async fn resolve_relation_option(
	auth: &AdminAuth,
	user: &dyn AdminUser,
	db: &AdminDatabase,
	relation: &ResolvedRelationField,
	id: &str,
) -> Result<RelationOption, ServerFnError> {
	require_related_view_permission(auth, user, relation).await?;
	validate_relation_id(relation, id).map_server_fn_error()?;
	let record = fetch_related_record(db, relation, id).await?;

	relation_option_from_record(relation, &record).map_server_fn_error()
}

#[cfg(server)]
pub(crate) async fn validate_relation_values(
	auth: &AdminAuth,
	user: &dyn AdminUser,
	site: &AdminSite,
	db: &AdminDatabase,
	source_admin: &Arc<dyn ModelAdmin>,
	data: &mut HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, serde_json::Value>, ServerFnError> {
	let relations = resolve_relation_configuration(site, source_admin).map_server_fn_error()?;
	let mut normalized_values = HashMap::new();

	for relation in &relations {
		let logical_name = relation.foreign_key.logical_name.as_str();
		let column_name = relation.foreign_key.column_name.as_str();
		let value = if logical_name == column_name {
			data.remove(column_name)
		} else {
			match (data.remove(logical_name), data.remove(column_name)) {
				(Some(_), Some(_)) => {
					return Err(AdminError::ValidationError(format!(
						"Relation field '{logical_name}' was submitted using both '{logical_name}' and '{column_name}'"
					))
					.into_server_fn_error());
				}
				(Some(value), None) | (None, Some(value)) => Some(value),
				(None, None) => None,
			}
		};
		let Some(value) = value else {
			continue;
		};

		require_related_view_permission(auth, user, relation).await?;
		let normalized = match relation_id_from_value(&value).map_server_fn_error()? {
			None if relation.foreign_key.field_metadata.is_nullable() => serde_json::Value::Null,
			None => {
				return Err(AdminError::ValidationError(format!(
					"Relation field '{logical_name}' cannot be null"
				))
				.into_server_fn_error());
			}
			Some(id) => {
				validate_relation_id(relation, &id).map_server_fn_error()?;
				let record = fetch_related_record(db, relation, &id).await?;
				let target_field = relation.target_field.as_str();
				let pk_value = record
					.get(target_field)
					.cloned()
					.ok_or_else(|| {
						AdminError::ValidationError(format!(
							"Related object is missing relation target field '{target_field}'"
						))
					})
					.map_server_fn_error()?;
				if relation_id_from_value(&pk_value)
					.map_server_fn_error()?
					.is_none()
				{
					return Err(AdminError::ValidationError(format!(
						"Related object relation target field '{target_field}' cannot be null"
					))
					.into_server_fn_error());
				}
				pk_value
			}
		};

		normalized_values.insert(column_name.to_string(), normalized);
	}

	Ok(normalized_values)
}

/// Search or resolve related objects for one configured foreign-key field.
#[server_fn]
pub async fn get_relation_options(
	model_name: String,
	field_name: String,
	request: RelationLookupRequest,
	#[inject] site: KeyedDepends<AdminSiteKey, AdminSite>,
	#[inject] db: KeyedDepends<AdminDatabaseKey, AdminDatabase>,
	#[inject] http_request: ServerFnRequest,
	#[inject] AdminAuthenticatedUser(user): AdminAuthenticatedUser,
) -> Result<RelationLookupResponse, ServerFnError> {
	let auth = AdminAuth::from_request(&http_request);
	let source_admin = site.get_model_admin(&model_name).map_server_fn_error()?;
	auth.require_model_permission(source_admin.as_ref(), user.as_ref(), ModelPermission::View)
		.await?;

	let relations = resolve_relation_configuration(&site, &source_admin).map_server_fn_error()?;
	let relation = find_configured_relation(&relations, &field_name).map_server_fn_error()?;

	match request {
		RelationLookupRequest::Search {
			query,
			page,
			page_size,
		} => {
			require_related_view_permission(&auth, user.as_ref(), relation).await?;
			if relation.widget != RelationWidget::Autocomplete {
				return Err(AdminError::ValidationError(format!(
					"Field '{}' does not support relation search",
					relation.foreign_key.logical_name
				))
				.into_server_fn_error());
			}
			if query.len() > MAX_RELATION_QUERY_LENGTH {
				return Err(AdminError::ValidationError(format!(
					"Relation query exceeds maximum length of {MAX_RELATION_QUERY_LENGTH} bytes"
				))
				.into_server_fn_error());
			}

			let page = page.unwrap_or(1).max(1);
			if page > MAX_RELATION_PAGE {
				return Err(AdminError::ValidationError(format!(
					"Relation page exceeds maximum of {MAX_RELATION_PAGE}"
				))
				.into_server_fn_error());
			}
			let page_size = page_size
				.unwrap_or(DEFAULT_RELATION_PAGE_SIZE)
				.clamp(1, MAX_RELATION_PAGE_SIZE);
			let offset = (page - 1) * page_size;
			let target_model = &relation.foreign_key.target_model;
			let target_field = target_column_name(target_model, &relation.target_field)
				.unwrap_or_else(|| relation.target_field.clone());
			let search_fields = relation
				.target_admin
				.search_fields()
				.into_iter()
				.map(|field| {
					target_column_name(target_model, field).unwrap_or_else(|| field.to_string())
				})
				.collect::<Vec<_>>();
			let filter_condition = if query.is_empty() {
				None
			} else {
				Some(FilterCondition::Or(
					search_fields
						.into_iter()
						.map(|field| {
							FilterCondition::Single(Filter::new(
								field.to_string(),
								FilterOperator::Contains,
								FilterValue::String(query.clone()),
							))
						})
						.collect(),
				))
			};
			let mut ordering = relation
				.target_admin
				.ordering()
				.into_iter()
				.filter_map(|field| target_ordering_name(target_model, field))
				.collect::<Vec<_>>();
			if !ordering
				.iter()
				.any(|field| field.trim_start_matches('-') == target_field)
			{
				ordering.push(target_field.clone());
			}
			let ordering_refs = ordering.iter().map(String::as_str).collect::<Vec<_>>();
			let additional_filters = vec![Filter::new(
				target_field,
				FilterOperator::IsNotNull,
				FilterValue::Null,
			)];
			let mut records = db
				.list_with_condition_ordered::<AdminRecord>(
					relation.target_admin.table_name(),
					filter_condition.as_ref(),
					additional_filters,
					&ordering_refs,
					offset,
					page_size + 1,
				)
				.await
				.map_server_fn_error()?;
			let has_next = records.len() > page_size as usize && page < MAX_RELATION_PAGE;
			records.truncate(page_size as usize);
			let results = records
				.iter()
				.map(|record| relation_option_from_record(relation, record))
				.collect::<AdminResult<Vec<_>>>()
				.map_server_fn_error()?;

			Ok(RelationLookupResponse {
				results,
				page,
				has_next,
			})
		}
		RelationLookupRequest::Resolve { id } => {
			let option = resolve_relation_option(&auth, user.as_ref(), &db, relation, &id).await?;
			Ok(RelationLookupResponse {
				results: vec![option],
				page: 1,
				has_next: false,
			})
		}
	}
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use crate::core::{AdminSite, ModelAdmin, ModelAdminConfig};
	use reinhardt_apps::{RelationshipMetadata, RelationshipType};
	use reinhardt_db::migrations::{FieldMetadata, FieldType, ModelMetadata, ModelRegistry};
	use rstest::rstest;
	use std::sync::Arc;

	fn source_metadata() -> ModelMetadata {
		let mut source = ModelMetadata::new(
			"admin_relation_config_source",
			"ResolverSource",
			"resolver_sources",
		);
		source.fields.insert(
			"author_id".to_string(),
			FieldMetadata::new(FieldType::Uuid)
				.with_param("fk_target", "ResolverTarget")
				.with_param("fk_target_app", "admin_relation_config_target"),
		);
		source
	}

	fn relationship() -> RelationshipMetadata {
		RelationshipMetadata::new(
			"admin_relation_config_source.ResolverSource",
			"ResolverTarget",
			RelationshipType::ForeignKey,
			"author",
			None,
			Some("author_id"),
			None,
		)
	}

	fn target_registry() -> ModelRegistry {
		let registry = ModelRegistry::new();
		registry.register_model(ModelMetadata::new(
			"admin_relation_config_target",
			"ResolverTarget",
			"resolver_targets",
		));
		registry
	}

	fn register_source(
		site: &AdminSite,
		autocomplete_fields: Vec<&str>,
		raw_id_fields: Vec<&str>,
	) -> Arc<dyn ModelAdmin> {
		let source = ModelAdminConfig::builder()
			.model_name("ResolverSource")
			.table_name("resolver_sources")
			.autocomplete_fields(autocomplete_fields)
			.raw_id_fields(raw_id_fields)
			.build()
			.expect("source admin should build");
		site.register("ResolverSource", source)
			.expect("source admin should register");
		site.get_model_admin("ResolverSource")
			.expect("source admin should be available")
	}

	fn register_target(site: &AdminSite, table_name: &str, search_fields: Vec<&str>) {
		let target = ModelAdminConfig::builder()
			.model_name("ResolverTarget")
			.table_name(table_name)
			.search_fields(search_fields)
			.build()
			.expect("target admin should build");
		site.register("ResolverTarget", target)
			.expect("target admin should register");
	}

	#[rstest]
	#[case(serde_json::json!(true))]
	#[case(serde_json::json!(false))]
	fn relation_id_rejects_boolean_values(#[case] value: serde_json::Value) {
		// Act
		let error = relation_id_from_value(&value)
			.expect_err("boolean relation primary keys must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Relation primary keys must be scalar values"
		);
	}

	#[rstest]
	fn relation_configuration_rejects_normalized_duplicates() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec!["author_id"]);
		register_target(&site, "resolver_targets", vec!["name"]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("logical and physical duplicates must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Relation fields 'author' and 'author_id' both resolve to column 'author_id'"
		);
	}

	#[rstest]
	fn relation_configuration_requires_related_admin() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("missing related admin must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Related admin 'ResolverTarget' for field 'author' is not registered"
		);
	}

	#[rstest]
	fn relation_configuration_resolves_registered_admin_alias_by_table() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		let target = ModelAdminConfig::builder()
			.model_name("ResolverTarget")
			.table_name("resolver_targets")
			.search_fields(vec!["name"])
			.build()
			.expect("target admin should build");
		site.register("target-alias", target)
			.expect("target admin alias should register");
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let resolved = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.expect("registered table alias should resolve");

		// Assert
		assert_eq!(resolved.len(), 1);
		assert_eq!(resolved[0].target_admin.table_name(), "resolver_targets");
	}

	#[rstest]
	fn admin_site_rejects_ambiguous_registered_admin_aliases() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		register_source(&site, vec!["author"], vec![]);
		let target = ModelAdminConfig::builder()
			.model_name("ResolverTarget")
			.table_name("resolver_targets")
			.search_fields(vec!["name"])
			.build()
			.expect("target admin should build");
		site.register("target-a", target)
			.expect("first target admin should register");
		let duplicate = ModelAdminConfig::builder()
			.model_name("ResolverTargetAlias")
			.table_name("resolver_targets")
			.search_fields(vec!["name"])
			.build()
			.expect("duplicate target admin should build");

		let error = site
			.register("target-b", duplicate)
			.expect_err("ambiguous target admin aliases must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Table 'resolver_targets' is already registered as 'target-a'"
		);
	}

	#[rstest]
	fn relation_configuration_uses_to_field_physical_column() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		register_target(&site, "resolver_targets", vec!["name"]);
		let mut source_metadata = source_metadata();
		source_metadata
			.fields
			.get_mut("author_id")
			.expect("source relation metadata should exist")
			.params
			.insert("fk_target_field".to_string(), "external_key".to_string());
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = ModelRegistry::new();
		let mut target = ModelMetadata::new(
			"admin_relation_config_target",
			"ResolverTarget",
			"resolver_targets",
		);
		target.add_field(
			"target_external_key".to_string(),
			FieldMetadata::new(FieldType::VarChar(32))
				.with_param("rust_field_name", "external_key")
				.with_param("db_column", "target_external_key"),
		);
		registry.register_model(target);

		// Act
		let resolved = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.expect("configured target field should resolve");

		// Assert
		assert_eq!(resolved[0].target_field, "target_external_key");
	}

	#[rstest]
	fn target_aliases_resolve_logical_search_and_primary_key_names() {
		// Arrange
		let mut target = ModelMetadata::new("relation_test", "Target", "targets");
		target.add_field(
			"object_id".to_string(),
			FieldMetadata::new(FieldType::Integer)
				.with_param("rust_field_name", "id")
				.with_param("db_column", "object_id")
				.with_param("primary_key", "true"),
		);
		target.add_field(
			"display_name".to_string(),
			FieldMetadata::new(FieldType::VarChar(100))
				.with_param("rust_field_name", "name")
				.with_param("db_column", "display_name"),
		);

		// Act
		let primary_key = target_column_name(&target, "id");
		let search_field = target_column_name(&target, "name");
		let ordering = target_ordering_name(&target, "-name");

		// Assert
		assert_eq!(primary_key, Some("object_id".to_string()));
		assert_eq!(search_field, Some("display_name".to_string()));
		assert_eq!(ordering, Some("-display_name".to_string()));
	}

	#[rstest]
	fn relation_id_validation_rejects_empty_and_overlong_ids() {
		// Arrange
		let mut target_model = ModelMetadata::new("relation_test", "Target", "targets");
		target_model.add_field(
			"external_key".to_string(),
			FieldMetadata::new(FieldType::VarChar(4)),
		);
		let target_admin: Arc<dyn ModelAdmin> = Arc::new(
			ModelAdminConfig::builder()
				.model_name("Target")
				.table_name("targets")
				.build()
				.expect("target admin should build"),
		);
		let mut relation = ResolvedRelationField {
			foreign_key: ForeignKeyFieldMetadata {
				logical_name: "target".to_string(),
				column_name: "target_id".to_string(),
				target_field: Some("external_key".to_string()),
				field_metadata: FieldMetadata::new(FieldType::VarChar(4)),
				target_model,
			},
			widget: RelationWidget::RawId,
			target_admin,
			target_field: "external_key".to_string(),
		};

		// Act
		let empty = validate_relation_id(&relation, "").expect_err("empty id must be rejected");
		validate_relation_id(&relation, "東京大学").expect("four Unicode characters fit the limit");
		let overlong =
			validate_relation_id(&relation, "12345").expect_err("overlong id must be rejected");

		// Assert
		assert_eq!(
			empty.to_string(),
			"Validation error: Relation id cannot be empty"
		);
		assert_eq!(
			overlong.to_string(),
			"Validation error: Relation id exceeds maximum length of 4 characters"
		);

		relation.foreign_key.target_model.fields.insert(
			"external_key".to_string(),
			FieldMetadata::new(FieldType::VarChar(255)),
		);
		validate_relation_id(&relation, &"x".repeat(255))
			.expect("relation IDs use their field limit rather than the search-query limit");
	}

	#[rstest]
	fn autocomplete_configuration_requires_related_search_fields() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		register_target(&site, "resolver_targets", vec![]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("autocomplete without search fields must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Related admin 'ResolverTarget' for field 'author' must configure search_fields for autocomplete"
		);
	}

	#[rstest]
	fn relation_configuration_rejects_related_admin_table_mismatch() {
		// Arrange
		let site = AdminSite::new("Relation configuration test");
		let source = register_source(&site, vec!["author"], vec![]);
		register_target(&site, "wrong_targets", vec!["name"]);
		let source_metadata = source_metadata();
		let relationship = relationship();
		let relationships = [&relationship];
		let registry = target_registry();

		// Act
		let error = validate_relation_configuration(
			&site,
			&source,
			&source_metadata,
			&relationships,
			&registry,
		)
		.err()
		.expect("related admin table mismatch must be rejected");

		// Assert
		assert_eq!(
			error.to_string(),
			"Validation error: Related admin 'ResolverTarget' uses table 'wrong_targets', expected 'resolver_targets'"
		);
	}
}
