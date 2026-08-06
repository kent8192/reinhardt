//! Integration tests for the Model derive macro (via `#[model]` attribute)
//!
//! Tests the interaction between:
//! - reinhardt-macros (Model derive macro)
//! - reinhardt-orm (Model trait)
//! - reinhardt-migrations (model_registry)

use async_trait::async_trait;
use reinhardt_db::Json;
use reinhardt_db::associations::{ForeignKeyField, OneToOneField};
use reinhardt_db::migrations::FieldType;
use reinhardt_db::migrations::model_registry::global_registry;
use reinhardt_db::migrations::{GeneratedStorage, SchemaExpr, SchemaFunc};
use reinhardt_db::orm::Model as ModelTrait;
use reinhardt_db::orm::QuerySet;
use reinhardt_db::orm::connection::{DatabaseBackend, OrmExecutor, QueryResult, QueryValue, Row};
use reinhardt_db::orm::fields::FieldKwarg;
use reinhardt_db::orm::fixtures::global_fixture_registry;
use reinhardt_db::orm::registry::global_model_registry;
use reinhardt_db::orm::relationship::RelationshipType;
use reinhardt_macros::model;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "test_app", table_name = "test_users")]
struct TestUser {
	#[field(primary_key = true)]
	id: Option<i32>,

	#[field(max_length = 100, null = false)]
	username: String,

	#[field(max_length = 255)]
	email: String,

	#[field(null = true)]
	age: Option<i32>,

	#[field(default = true)]
	is_active: bool,
}

#[derive(Serialize, Deserialize)]
#[model(app_label = "metadata_test", table_name = "metadata_targets")]
struct MetadataTarget {
	#[field(primary_key = true)]
	id: Option<i64>,
}

#[model(app_label = "metadata_test", table_name = "metadata_writers")]
#[derive(Serialize, Deserialize)]
struct MetadataWriter {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[rel(foreign_key, db_column = "writer_pk")]
	writer: ForeignKeyField<MetadataTarget>,
}

#[model(app_label = "metadata_test", table_name = "nullable_metadata_writers")]
#[derive(Serialize, Deserialize)]
struct NullableMetadataWriter {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[rel(foreign_key, db_column = "nullable_writer_pk", null = true)]
	writer: ForeignKeyField<MetadataTarget>,
}

#[model(app_label = "metadata_test", table_name = "metadata_profiles")]
#[derive(Serialize, Deserialize)]
struct MetadataProfile {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[rel(one_to_one)]
	profile: OneToOneField<MetadataTarget>,
}

#[model(app_label = "traversal_test", table_name = "traversal_authors")]
#[derive(Serialize, Deserialize)]
struct TraversalAuthor {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 255, db_column = "email_address")]
	email: String,

	#[field(max_length = 255, db_column = "author_slug")]
	slug: String,
}

#[model(app_label = "traversal_test", table_name = "traversal_posts")]
#[derive(Serialize, Deserialize)]
struct TraversalPost {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[rel(foreign_key, db_column = "author_slug", to_field = "slug")]
	author: ForeignKeyField<TraversalAuthor>,
}

#[model(app_label = "accessor_test", table_name = "accessor_targets")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccessorTarget {
	#[field(primary_key = true, db_column = "target_pk")]
	id: Option<i64>,

	#[field(db_column = "target_external_key")]
	external_key: i64,
}

#[model(app_label = "accessor_test", table_name = "accessor_primary_sources")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccessorPrimarySource {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[rel(foreign_key, db_column = "target_fk")]
	target: ForeignKeyField<AccessorTarget>,
}

#[model(app_label = "accessor_test", table_name = "accessor_to_field_sources")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccessorToFieldSource {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[rel(
		foreign_key,
		db_column = "target_external_fk",
		to_field = "external_key"
	)]
	target: ForeignKeyField<AccessorTarget>,
}

#[derive(Debug, Clone, PartialEq)]
struct RecordedOrmCall {
	kind: &'static str,
	sql: String,
	params: Vec<QueryValue>,
}

#[derive(Debug)]
struct RecordingOrmExecutor {
	backend: DatabaseBackend,
	calls: Vec<RecordedOrmCall>,
}

impl RecordingOrmExecutor {
	fn postgres() -> Self {
		Self {
			backend: DatabaseBackend::Postgres,
			calls: Vec::new(),
		}
	}

	fn record(&mut self, kind: &'static str, sql: &str, params: Vec<QueryValue>) {
		self.calls.push(RecordedOrmCall {
			kind,
			sql: sql.to_string(),
			params,
		});
	}
}

#[async_trait]
impl OrmExecutor for RecordingOrmExecutor {
	fn backend(&self) -> DatabaseBackend {
		self.backend
	}

	async fn execute(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<QueryResult> {
		self.record("execute", sql, params);
		Ok(QueryResult {
			rows_affected: 0,
			last_insert_id: None,
		})
	}

	async fn fetch_one(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Row> {
		self.record("fetch_one", sql, params);
		Ok(Row::new())
	}

	async fn fetch_all(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Vec<Row>> {
		self.record("fetch_all", sql, params);
		Ok(Vec::new())
	}

	async fn fetch_optional(
		&mut self,
		sql: &str,
		params: Vec<QueryValue>,
	) -> reinhardt_core::exception::Result<Option<Row>> {
		self.record("fetch_optional", sql, params);
		Ok(None)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct JsonSettings {
	indent_width: u8,
	theme: String,
}

#[derive(Serialize, Deserialize)]
#[model(app_label = "test_app", table_name = "json_models")]
struct JsonModel {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field]
	settings: Json<JsonSettings>,

	#[field(null = true)]
	raw: Option<Json<serde_json::Value>>,
}

#[derive(Serialize, Deserialize)]
#[model(app_label = "generated_app", table_name = "generated_users")]
struct GeneratedUser {
	#[field(primary_key = true)]
	id: Option<i32>,

	#[field(max_length = 100)]
	first_name: String,

	#[field(max_length = 100)]
	last_name: String,

	#[field(
		max_length = 201,
		generated = SchemaExpr::concat([
			SchemaExpr::col("first_name"),
			SchemaExpr::val(" "),
			SchemaExpr::col("last_name")
		]),
		generated_stored = true
	)]
	full_name: String,
}

#[model(
	app_label = "fixture_projection",
	table_name = "fixture_projection_users"
)]
#[derive(Serialize, Deserialize)]
struct FixtureProjectionUser {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[serde(rename = "displayName")]
	#[field(max_length = 100)]
	title: String,

	#[field(max_length = 100)]
	payload: String,

	#[field(
		max_length = 100,
		generated_sql = "UPPER(payload)",
		generated_stored = true
	)]
	generated_value: String,
}

#[model(
	app_label = "fixture_projection",
	table_name = "fixture_projection_default_users"
)]
#[derive(Serialize, Deserialize)]
struct FixtureProjectionDefaultUser {
	#[field(primary_key = true)]
	id: Option<i64>,

	#[field(max_length = 100)]
	title: String,

	#[field(default = true)]
	is_active: bool,
}

mod registered_models {
	use reinhardt_macros::model;

	#[derive(serde::Serialize, serde::Deserialize)]
	#[model(app_label = "inventory", table_name = "inventory_items")]
	pub(crate) struct InventoryItem {
		#[field(primary_key = true)]
		pub id: i64,
	}
}

#[test]
fn model_registry_records_fully_qualified_type_path() {
	let _ = registered_models::InventoryItem::table_name();

	let model = global_model_registry()
		.all()
		.into_iter()
		.find(|model| model.model_name == "InventoryItem")
		.expect("InventoryItem should be registered");

	assert_eq!(
		model.type_path,
		concat!(module_path!(), "::registered_models::InventoryItem")
	);
}

#[test]
fn test_model_trait_implementation() {
	// Verify Model trait methods are correctly implemented
	assert_eq!(TestUser::table_name(), "test_users");
	assert_eq!(TestUser::app_label(), "test_app");
	assert_eq!(TestUser::primary_key_field(), "id");
}

#[test]
fn test_field_metadata_generation() {
	// Get field metadata
	let fields = TestUser::field_metadata();

	// Should have 5 fields
	assert_eq!(fields.len(), 5, "Expected 5 fields");

	// Check id field
	let id_field = fields.iter().find(|f| f.name == "id");
	assert!(id_field.is_some(), "id field not found");
	let id_field = id_field.unwrap();
	assert_eq!(id_field.field_type, "reinhardt.orm.models.IntegerField");
	assert!(id_field.primary_key, "id should be primary key");
	assert!(id_field.nullable, "id should be nullable (Option<i32>)");

	// Check username field
	let username_field = fields.iter().find(|f| f.name == "username");
	assert!(username_field.is_some(), "username field not found");
	let username_field = username_field.unwrap();
	assert_eq!(username_field.field_type, "reinhardt.orm.models.CharField");
	assert!(!username_field.nullable, "username should not be nullable");
	assert!(
		username_field.attributes.contains_key("max_length"),
		"username should have max_length attribute"
	);

	// Check email field
	let email_field = fields.iter().find(|f| f.name == "email");
	assert!(email_field.is_some(), "email field not found");
	let email_field = email_field.unwrap();
	assert_eq!(email_field.field_type, "reinhardt.orm.models.CharField");
	assert!(
		email_field.attributes.contains_key("max_length"),
		"email should have max_length attribute"
	);

	// Check age field
	let age_field = fields.iter().find(|f| f.name == "age");
	assert!(age_field.is_some(), "age field not found");
	let age_field = age_field.unwrap();
	assert_eq!(age_field.field_type, "reinhardt.orm.models.IntegerField");
	assert!(age_field.nullable, "age should be nullable");

	// Check is_active field
	let is_active_field = fields.iter().find(|f| f.name == "is_active");
	assert!(is_active_field.is_some(), "is_active field not found");
	let is_active_field = is_active_field.unwrap();
	assert_eq!(
		is_active_field.field_type,
		"reinhardt.orm.models.BooleanField"
	);
	assert_eq!(is_active_field.default, Some(FieldKwarg::Bool(true)));
}

#[test]
fn test_relationship_metadata_uses_generated_fk_columns_and_targets() {
	assert_eq!(MetadataWriter::field_writer_id().name(), "writer_pk");
	assert_eq!(
		NullableMetadataWriter::field_writer_id().name(),
		"nullable_writer_pk"
	);

	let writer = MetadataWriter::relationship_metadata()
		.into_iter()
		.next()
		.expect("writer relationship should be present");
	assert_eq!(writer.relationship_type, RelationshipType::ManyToOne);
	assert_eq!(writer.foreign_key.as_deref(), Some("writer_pk"));
	assert_eq!(writer.related_model, "metadata_test.MetadataTarget");

	let profile = MetadataProfile::relationship_metadata()
		.into_iter()
		.next()
		.expect("profile relationship should be present");
	assert_eq!(profile.relationship_type, RelationshipType::OneToOne);
	assert_eq!(profile.foreign_key.as_deref(), Some("profile_id"));
	assert_eq!(profile.related_model, "metadata_test.MetadataTarget");
}

#[test]
fn test_related_field_accessor_uses_physical_column_in_filter() {
	let related_email = TraversalPost::rel_author().into_typed().field_email();
	assert_eq!(related_email.name(), "email");
	let sql = QuerySet::<TraversalPost>::new()
		.filter(related_email.exact("person@example.com"))
		.to_sql()
		.expect("query with a valid relationship path should generate SQL");

	assert_eq!(
		sql,
		r#"SELECT "traversal_posts".* FROM "traversal_posts" INNER JOIN "traversal_authors" AS "author" ON "traversal_posts"."author_slug" = "author"."author_slug" WHERE "author"."email_address" = 'person@example.com'"#
	);
}

#[test]
fn test_relation_descriptor_resolves_to_field_physical_column() {
	use reinhardt_db::orm::relations::RelationPathLike;

	assert_eq!(
		TraversalPost::rel_author().steps()[0].target_column,
		"author_slug"
	);
}

#[tokio::test]
async fn generated_relation_accessors_render_configured_physical_columns() {
	let primary_source = AccessorPrimarySource::build()
		.id(Some(1_i64))
		.target(7_i64)
		.finish();
	let mut primary_loader = RecordingOrmExecutor::postgres();

	let primary_result = primary_source
		.target(&mut primary_loader)
		.await
		.expect("generated primary-key loader must use the supplied executor");
	assert!(primary_result.is_none());
	assert_eq!(primary_loader.calls.len(), 1);
	assert_eq!(primary_loader.calls[0].kind, "fetch_all");
	assert!(
		primary_loader.calls[0]
			.sql
			.contains(r#"WHERE "target_pk" = $1"#),
		"generated primary-key loader must use the physical primary-key column: {}",
		primary_loader.calls[0].sql
	);
	assert!(
		!primary_loader.calls[0].sql.contains(r#"WHERE "id" = $1"#),
		"generated primary-key loader must not use the logical primary-key field name"
	);

	let target = AccessorTarget::build()
		.id(Some(7_i64))
		.external_key(7_i64)
		.finish();
	let reverse = AccessorPrimarySource::target_accessor().reverse(&target);
	let mut reverse_executor = RecordingOrmExecutor::postgres();

	let related = reverse
		.all_with_conn(&mut reverse_executor)
		.await
		.expect("generated reverse accessor must use the supplied executor");
	assert!(related.is_empty());
	assert_eq!(reverse_executor.calls.len(), 1);
	assert_eq!(reverse_executor.calls[0].kind, "fetch_all");
	assert!(
		reverse_executor.calls[0].sql.contains(r#""target_fk""#),
		"generated reverse accessor must use the configured physical foreign-key column: {}",
		reverse_executor.calls[0].sql
	);
	assert!(
		!reverse_executor.calls[0].sql.contains(r#""target_id""#),
		"generated reverse accessor must not fall back to the default foreign-key column"
	);

	let to_field_source = AccessorToFieldSource::build()
		.id(Some(2_i64))
		.target(7_i64)
		.finish();
	let mut to_field_loader = RecordingOrmExecutor::postgres();

	let to_field_result = to_field_source
		.target(&mut to_field_loader)
		.await
		.expect("generated to_field loader must use the supplied executor");
	assert!(to_field_result.is_none());
	assert_eq!(to_field_loader.calls.len(), 1);
	assert_eq!(to_field_loader.calls[0].kind, "fetch_all");
	assert!(
		to_field_loader.calls[0]
			.sql
			.contains(r#"WHERE "target_external_key" = $1"#),
		"generated to_field loader must resolve the target field's physical column: {}",
		to_field_loader.calls[0].sql
	);
	assert!(
		!to_field_loader.calls[0]
			.sql
			.contains(r#"WHERE "target_pk" = $1"#),
		"generated to_field loader must not use the target primary-key column: {}",
		to_field_loader.calls[0].sql
	);
}

#[test]
fn test_typed_generated_column_registration() {
	let _sample = GeneratedUser {
		id: None,
		first_name: "Ada".to_string(),
		last_name: "Lovelace".to_string(),
		full_name: "Ada Lovelace".to_string(),
	};
	let registry = global_registry();
	let model = registry
		.get_model("generated_app", "GeneratedUser")
		.expect("GeneratedUser should be registered in global registry");
	let field = model
		.fields
		.get("full_name")
		.expect("full_name field should be registered");
	let generated = field
		.generated
		.as_ref()
		.expect("full_name should carry generated-column metadata");

	assert_eq!(generated.storage, GeneratedStorage::Stored);
	assert!(generated.raw_sql.is_none());
	let expr_tokens = generated.expr_tokens.as_deref().unwrap_or_default();
	let compact_expr_tokens = expr_tokens
		.chars()
		.filter(|ch| !ch.is_whitespace())
		.collect::<String>();
	assert!(
		compact_expr_tokens.contains("SchemaExpr::concat"),
		"expr_tokens should retain the Rust SchemaExpr builder expression: {:?}",
		generated.expr_tokens
	);
	match generated.expr.as_deref() {
		Some(SchemaExpr::Function { func, args }) => {
			assert_eq!(*func, SchemaFunc::Concat);
			assert_eq!(args.len(), 3);
			assert_eq!(args[0], SchemaExpr::col("first_name"));
			assert_eq!(args[1], SchemaExpr::val(" "));
			assert_eq!(args[2], SchemaExpr::col("last_name"));
		}
		other => panic!("expected concat SchemaExpr, got {other:?}"),
	}
}

#[test]
fn test_fixture_projection_validates_writable_fields_without_api_serde_names() {
	let mut fields = serde_json::Map::new();
	fields.insert("id".to_string(), serde_json::json!(1));
	fields.insert("title".to_string(), serde_json::json!("Fixture title"));
	fields.insert("payload".to_string(), serde_json::json!("body"));

	assert!(
		FixtureProjectionUser::validate_fixture_fields(&fields).is_ok(),
		"generated columns must be optional and fixture field names must not use serde renames"
	);

	let mut missing_payload = fields.clone();
	missing_payload.remove("payload");
	assert!(
		FixtureProjectionUser::validate_fixture_fields(&missing_payload).is_err(),
		"non-generated fixture fields must remain required"
	);

	let mut invalid_payload = fields;
	invalid_payload.insert("payload".to_string(), serde_json::json!(42));
	assert!(
		FixtureProjectionUser::validate_fixture_fields(&invalid_payload).is_err(),
		"non-generated fixture fields must retain their Rust type validation"
	);
}

#[test]
fn test_fixture_projection_allows_missing_defaulted_fields() {
	let mut fields = serde_json::Map::new();
	fields.insert("id".to_string(), serde_json::json!(1));
	fields.insert("title".to_string(), serde_json::json!("Fixture title"));

	assert!(
		FixtureProjectionDefaultUser::validate_fixture_fields(&fields).is_ok(),
		"fixture validation must allow omitted fields that have model defaults"
	);

	fields.insert("is_active".to_string(), serde_json::json!("not-a-bool"));
	assert!(
		FixtureProjectionDefaultUser::validate_fixture_fields(&fields).is_err(),
		"provided defaulted fields must retain their Rust type validation"
	);
}

#[test]
fn test_fixture_projection_uses_custom_foreign_key_columns() {
	let mut fields = serde_json::Map::new();
	fields.insert("id".to_string(), serde_json::json!(1));
	fields.insert("writer_pk".to_string(), serde_json::json!(7));

	assert!(
		MetadataWriter::validate_fixture_fields(&fields).is_ok(),
		"fixture validation must accept the canonical custom foreign-key column"
	);

	let mut nullable_fields = serde_json::Map::new();
	nullable_fields.insert("id".to_string(), serde_json::json!(1));
	nullable_fields.insert("nullable_writer_pk".to_string(), serde_json::Value::Null);
	assert!(
		NullableMetadataWriter::validate_fixture_fields(&nullable_fields).is_ok(),
		"nullable custom foreign-key fixtures must accept explicit null"
	);

	for invalid_identifier in [serde_json::json!({ "id": 7 }), serde_json::json!([7])] {
		let mut invalid_fields = serde_json::Map::new();
		invalid_fields.insert("id".to_string(), serde_json::json!(1));
		invalid_fields.insert("nullable_writer_pk".to_string(), invalid_identifier);

		assert!(
			NullableMetadataWriter::validate_fixture_fields(&invalid_fields).is_err(),
			"nullable foreign-key fixture values must reject non-null structured identifiers"
		);
	}

	for invalid_identifier in [serde_json::json!({ "id": 7 }), serde_json::json!([7])] {
		let mut invalid_fields = serde_json::Map::new();
		invalid_fields.insert("id".to_string(), serde_json::json!(1));
		invalid_fields.insert("writer_pk".to_string(), invalid_identifier);

		assert!(
			MetadataWriter::validate_fixture_fields(&invalid_fields).is_err(),
			"required foreign-key fixture values must be scalar identifiers"
		);
	}
}

#[test]
fn test_model_registration() {
	// Verify the model was automatically registered via ctor
	let registry = global_registry();
	let models = registry.get_models();

	// Find our test model
	let test_model = models
		.iter()
		.find(|m| m.app_label == "test_app" && m.model_name == "TestUser");

	assert!(
		test_model.is_some(),
		"TestUser should be registered in global registry"
	);

	let test_model = test_model.unwrap();
	assert_eq!(test_model.table_name, "test_users");

	// Verify fields were registered
	assert_eq!(test_model.fields.len(), 5, "Expected 5 registered fields");

	// Verify field names
	assert!(test_model.fields.contains_key("id"));
	assert!(test_model.fields.contains_key("username"));
	assert!(test_model.fields.contains_key("email"));
	assert!(test_model.fields.contains_key("age"));
	assert!(test_model.fields.contains_key("is_active"));
}

#[test]
fn test_fixture_handler_registration_supports_derive_before_model() {
	let handler = global_fixture_registry().get("test_app.TestUser");

	assert!(
		handler.is_some(),
		"models that derive serde before #[model] must register a fixture handler"
	);
}

#[test]
fn test_typed_json_field_metadata_generation() {
	let fields = JsonModel::field_metadata();

	let settings_field = fields
		.iter()
		.find(|field| field.name == "settings")
		.expect("settings field should exist");
	assert_eq!(settings_field.field_type, "reinhardt.orm.models.JsonField");
	assert!(!settings_field.nullable, "settings should not be nullable");

	let raw_field = fields
		.iter()
		.find(|field| field.name == "raw")
		.expect("raw field should exist");
	assert_eq!(raw_field.field_type, "reinhardt.orm.models.JsonField");
	assert!(raw_field.nullable, "raw should be nullable");
}

#[test]
fn test_typed_json_field_registry_metadata_generation() {
	let registry = global_registry();
	let models = registry.get_models();

	let json_model = models
		.iter()
		.find(|m| m.app_label == "test_app" && m.model_name == "JsonModel")
		.expect("JsonModel should be registered in global registry");

	let settings_field = json_model
		.fields
		.get("settings")
		.expect("settings field should be registered");
	assert_eq!(settings_field.field_type, FieldType::Jsonb);
	assert!(!settings_field.nullable, "settings should not be nullable");

	let raw_field = json_model
		.fields
		.get("raw")
		.expect("raw field should be registered");
	assert_eq!(raw_field.field_type, FieldType::Jsonb);
	assert!(raw_field.nullable, "raw should be nullable");
}

#[test]
fn test_typed_json_field_serde_roundtrip() {
	let model = JsonModel {
		id: Some(1),
		settings: Json::new(JsonSettings {
			indent_width: 2,
			theme: "paper".to_string(),
		}),
		raw: Some(Json::new(serde_json::json!({
			"language": "ja",
			"draft": true
		}))),
	};

	let value = serde_json::to_value(&model).expect("Json<T> should serialize transparently");
	assert_eq!(value["settings"]["theme"], "paper");
	assert_eq!(value["raw"]["language"], "ja");

	let hydrated: JsonModel =
		serde_json::from_value(value).expect("Json<T> should deserialize transparently");
	assert_eq!(hydrated.settings.indent_width, 2);
	assert_eq!(hydrated.raw.unwrap()["draft"], true);
}

#[rstest]
fn test_typed_json_field_option_state_distinguishes_none_from_json_null() {
	// Arrange
	let settings = JsonSettings {
		indent_width: 2,
		theme: "paper".to_string(),
	};
	let absent = JsonModel {
		id: Some(1),
		settings: Json::new(settings.clone()),
		raw: None,
	};
	let json_null = JsonModel {
		id: Some(2),
		settings: Json::new(settings),
		raw: Some(Json::new(serde_json::Value::Null)),
	};

	// Act
	let absent_is_none = absent.field_is_none("raw");
	let json_null_is_none = json_null.field_is_none("raw");

	// Assert
	assert!(absent_is_none);
	assert!(!json_null_is_none);
}

#[test]
fn test_primary_key_access() {
	// Test with None primary key
	let mut user = TestUser {
		id: None,
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		age: Some(25),
		is_active: true,
	};

	// Initially no primary key
	assert!(
		user.primary_key().is_none(),
		"New user should have no primary key"
	);

	// Set primary key
	user.set_primary_key(42);
	assert_eq!(
		user.primary_key(),
		Some(42),
		"Primary key should be set to 42"
	);

	// Test with Some primary key from the start
	let user_with_id = TestUser {
		id: Some(100),
		username: "anotheruser".to_string(),
		email: "another@example.com".to_string(),
		age: None,
		is_active: false,
	};

	assert_eq!(
		user_with_id.primary_key(),
		Some(100),
		"User should have primary key 100"
	);
}

#[test]
fn test_multiple_models_registration() {
	// Define another model to ensure multiple models can be registered
	#[derive(Serialize, Deserialize)]
	#[model(app_label = "test_app", table_name = "test_posts")]
	#[allow(dead_code)]
	struct TestPost {
		#[field(primary_key = true)]
		id: Option<i64>,

		#[field(max_length = 200)]
		title: String,
	}

	// Verify both models are registered
	let registry = global_registry();
	let models = registry.get_models();

	let user_model = models
		.iter()
		.find(|m| m.model_name == "TestUser" && m.app_label == "test_app");
	let post_model = models
		.iter()
		.find(|m| m.model_name == "TestPost" && m.app_label == "test_app");

	assert!(user_model.is_some(), "TestUser should be registered");
	assert!(post_model.is_some(), "TestPost should be registered");
}
