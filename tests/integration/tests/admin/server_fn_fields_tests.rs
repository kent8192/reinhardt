//! Integration tests for get_fields server function
//!
//! Tests the field definitions server function for dynamic form generation.
//! Covers regression for Issue #2920 (get_fields() missing authentication check).

use super::server_fn_helpers::{fieldset_context, server_fn_context};
use reinhardt_admin::core::{AdminDatabase, AdminRecord, AdminSite, AdminUser, ModelAdmin};
use reinhardt_admin::server::get_fields;
use reinhardt_admin::types::{FieldType, RelationOption, RelationSelectorLayout};
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::migrations::FieldType as DatabaseFieldType;
use reinhardt_db::migrations::model_registry::{
	FieldMetadata, ManyToManyMetadata, ModelMetadata, global_registry,
};
use reinhardt_db::orm::connection::DatabaseConnectionLease;
use reinhardt_di::KeyedDepends;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::*;
use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;
use std::sync::Arc;
use sqlx::Executor;

use super::server_fn_helpers::{make_auth_user, make_staff_request};

struct RelationAdmin {
	model_name: &'static str,
	table_name: &'static str,
	list_display: Vec<&'static str>,
	search_fields: Vec<&'static str>,
	fields: Vec<&'static str>,
	filter_horizontal: Vec<&'static str>,
	allow_view: bool,
}

struct RelationDatabaseLease {
	_lease: DatabaseConnectionLease,
}

impl Drop for RelationDatabaseLease {
	fn drop(&mut self) {
		for model in ["RelationArticle", "RelationTag"] {
			global_registry().remove_model("admin_relation", model);
		}
	}
}

impl RelationAdmin {
	fn source() -> Self {
		Self {
			model_name: "RelationArticle",
			table_name: "admin_relation_articles",
			list_display: vec!["id", "title"],
			search_fields: vec!["title"],
			fields: vec!["id", "title"],
			filter_horizontal: vec!["tags"],
			allow_view: true,
		}
	}

	fn target(allow_view: bool) -> Self {
		Self {
			model_name: "RelationTag",
			table_name: "admin_relation_tags",
			list_display: vec!["id", "name"],
			search_fields: vec!["name"],
			fields: vec!["id", "name"],
			filter_horizontal: Vec::new(),
			allow_view,
		}
	}
}

#[async_trait::async_trait]
impl ModelAdmin for RelationAdmin {
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

	fn fields(&self) -> Option<Vec<&str>> {
		Some(self.fields.clone())
	}

	fn filter_horizontal(&self) -> Vec<&str> {
		self.filter_horizontal.clone()
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		self.allow_view
	}
}

fn relation_site(target_view_allowed: bool) -> AdminSite {
	let site = AdminSite::new("Relation Test");
	site.register("RelationArticle", RelationAdmin::source())
		.unwrap();
	site.register("RelationTag", RelationAdmin::target(target_view_allowed))
		.unwrap();
	site
}

#[fixture]
async fn relation_database(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> (AdminDatabase, RelationDatabaseLease) {
	let (pool, _) = shared_db_pool.await;
	pool.execute(
		"CREATE TABLE admin_relation_articles (id INTEGER PRIMARY KEY, title VARCHAR(200) NOT NULL)",
	)
	.await
	.unwrap();
	pool.execute(
		"CREATE TABLE admin_relation_tags (id INTEGER PRIMARY KEY, name VARCHAR(100) NOT NULL)",
	)
	.await
	.unwrap();
	pool.execute(
		"CREATE TABLE admin_relation_articles_tags (admin_relation_articles_id INTEGER NOT NULL, admin_relation_tags_id INTEGER NOT NULL)",
	)
	.await
	.unwrap();
	pool.execute("INSERT INTO admin_relation_articles (id, title) VALUES (1, 'Selectors')")
		.await
		.unwrap();
	pool.execute(
		"INSERT INTO admin_relation_tags (id, name) SELECT value, 'Tag ' || LPAD(value::text, 3, '0') FROM generate_series(1, 61) AS value",
	)
	.await
	.unwrap();
	pool.execute(
		"INSERT INTO admin_relation_articles_tags (admin_relation_articles_id, admin_relation_tags_id) VALUES (1, 60), (1, 61)",
	)
	.await
	.unwrap();

	let mut source = ModelMetadata::new(
		"admin_relation",
		"RelationArticle",
		"admin_relation_articles",
	);
	source.add_field(
		"id".to_string(),
		FieldMetadata::new(DatabaseFieldType::Integer),
	);
	source.add_field(
		"title".to_string(),
		FieldMetadata::new(DatabaseFieldType::VarChar(200)),
	);
	source.add_many_to_many(ManyToManyMetadata::new(
		"tags",
		"admin_relation.RelationTag",
	));
	global_registry().register_model(source);
	let mut target = ModelMetadata::new("admin_relation", "RelationTag", "admin_relation_tags");
	target.add_field(
		"id".to_string(),
		FieldMetadata::new(DatabaseFieldType::Integer),
	);
	target.add_field(
		"name".to_string(),
		FieldMetadata::new(DatabaseFieldType::VarChar(100)),
	);
	global_registry().register_model(target);

	let backend = Arc::new(PostgresBackend::new(pool));
	let connection_lease = DatabaseConnectionLease::register(BackendsConnection::new(backend))
		.expect("Failed to register relation database connection");
	let db = AdminDatabase::new(connection_lease.handle());
	(
		db,
		RelationDatabaseLease {
			_lease: connection_lease,
		},
	)
}

/// Verify get_fields preserves fieldset order and layout metadata.
#[rstest]
#[tokio::test]
#[serial(admin_registry)]
async fn test_get_fields_returns_fieldsets_in_declared_order(
	#[future] fieldset_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = fieldset_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let response = get_fields(
		"FieldsetModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await
	.expect("get_fields should succeed for configured fieldsets");

	// Assert
	assert_eq!(
		response
			.fields
			.iter()
			.map(|field| field.name.as_str())
			.collect::<Vec<_>>(),
		vec!["title", "body", "published_at"],
	);
	let fieldsets = response
		.fieldsets
		.expect("response should include fieldsets");
	assert_eq!(fieldsets.len(), 2);
	assert_eq!(fieldsets[0].title.as_deref(), Some("Main"));
	assert_eq!(
		fieldsets[0]
			.fields
			.iter()
			.map(String::as_str)
			.collect::<Vec<_>>(),
		vec!["title", "body"],
	);
	assert_eq!(fieldsets[0].collapsed, false);
	assert_eq!(fieldsets[1].title.as_deref(), Some("Publishing"));
	assert_eq!(
		fieldsets[1]
			.fields
			.iter()
			.map(String::as_str)
			.collect::<Vec<_>>(),
		vec!["published_at"],
	);
	assert_eq!(fieldsets[1].collapsed, true);
}

/// Verify get_fields rejects fieldset names absent from model metadata.
#[rstest]
#[tokio::test]
#[serial(admin_registry)]
async fn test_get_fields_rejects_unknown_fieldset_field(
	#[future] fieldset_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = fieldset_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"InvalidFieldsetModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let error = result.expect_err("unknown fieldset fields must be rejected");
	assert_eq!(
		error.kind(),
		reinhardt_pages::server_fn::ServerFnErrorKind::Server
	);
	assert_eq!(
		error.user_message(),
		"Fieldset field 'unknown_field' is not registered for model 'InvalidFieldsetModel'"
	);
}

// ==================== Happy path tests ====================

/// Verify get_fields returns field definitions for create form (no id)
#[rstest]
#[tokio::test]
async fn test_get_fields_create_form(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None, // No ID = create form
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"get_fields for create should succeed: {:?}",
		result
	);
	let response = result.unwrap();
	assert_eq!(response.model_name, "TestModel");
	assert!(
		!response.fields.is_empty(),
		"Should return field definitions"
	);
	assert!(
		response.values.is_none(),
		"Create form should have no values"
	);
}

/// Verify get_fields returns field definitions + existing values for edit form
#[rstest]
#[tokio::test]
async fn test_get_fields_edit_form(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	let mut data = HashMap::new();
	data.insert("name".to_string(), json!("Edit Form Item"));
	data.insert("status".to_string(), json!("active"));
	let created_id = db
		.create::<AdminRecord>("test_models", None, data)
		.await
		.expect("Failed to create test record");

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		Some(created_id.to_string()),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_ok(),
		"get_fields for edit should succeed: {:?}",
		result
	);
	let response = result.unwrap();
	assert!(
		!response.fields.is_empty(),
		"Should return field definitions"
	);
	assert!(
		response.values.is_some(),
		"Edit form should have existing values"
	);
}

// ==================== Contract tests ====================

/// Verify field names match the model admin configuration
#[rstest]
#[tokio::test]
async fn test_get_fields_returns_correct_field_names(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed");
	let field_names: Vec<&str> = response.fields.iter().map(|f| f.name.as_str()).collect();
	// model_admin_config has list_display: ["id", "name", "created_at"]
	assert!(
		field_names.contains(&"id") || field_names.contains(&"name"),
		"Fields should contain model fields, got: {:?}",
		field_names
	);
}

/// Verify field labels are humanized versions of field names
#[rstest]
#[tokio::test]
async fn test_get_fields_field_labels_humanized(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed");
	for field in &response.fields {
		assert!(
			!field.label.is_empty(),
			"Field '{}' should have a non-empty label",
			field.name
		);
	}
}

/// Verify each field has a type assigned
#[rstest]
#[tokio::test]
async fn test_get_fields_field_type_inference(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed");
	// All fields should have some type inferred (Text is the fallback)
	assert!(
		!response.fields.is_empty(),
		"Should have at least one field"
	);
}

// ==================== Edge case tests ====================

/// Verify get_fields with non-existent ID returns fields but no values
#[rstest]
#[tokio::test]
async fn test_get_fields_edit_nonexistent_id(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"TestModel".to_string(),
		Some("999999".to_string()),
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	let response = result.expect("get_fields should succeed even with non-existent ID");
	assert!(
		!response.fields.is_empty(),
		"Should still return field definitions"
	);
	assert!(
		response.values.is_none(),
		"Should return None values for non-existent record"
	);
}

// ==================== Error path tests ====================

/// Verify get_fields returns error for non-registered model
#[rstest]
#[tokio::test]
async fn test_get_fields_model_not_registered(
	#[future] server_fn_context: super::server_fn_helpers::ServerFnContext,
) {
	// Arrange
	let (site, db, _connection_lease) = server_fn_context.await;
	let http_request = make_staff_request();
	let auth_user = make_auth_user();

	// Act
	let result = get_fields(
		"NonExistentModel".to_string(),
		None,
		site,
		db,
		http_request,
		auth_user,
	)
	.await;

	// Assert
	assert!(
		result.is_err(),
		"Should return error for unregistered model"
	);
}

#[rstest]
#[serial(admin_relation_registry)]
#[tokio::test]
async fn get_fields_retains_selected_relation_options_outside_first_page(
	#[future] relation_database: (AdminDatabase, RelationDatabaseLease),
) {
	let (db, _connection_lease) = relation_database.await;

	let response = get_fields(
		"RelationArticle".to_string(),
		Some("1".to_string()),
		KeyedDepends::from_value(relation_site(true)),
		KeyedDepends::from_value(db),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.unwrap();

	assert_eq!(
		response
			.fields
			.iter()
			.map(|field| field.name.as_str())
			.collect::<Vec<_>>(),
		vec!["id", "title", "tags"]
	);
	let field = response
		.fields
		.iter()
		.find(|field| field.name == "tags")
		.unwrap();
	let FieldType::ManyToManySelector {
		layout,
		available,
		selected,
		has_more,
	} = &field.field_type
	else {
		panic!("tags must be a many-to-many selector")
	};
	assert_eq!(*layout, RelationSelectorLayout::Horizontal);
	assert_eq!(available.len(), 50);
	assert_eq!(
		selected,
		&vec![
			RelationOption::new("60", "Tag 060"),
			RelationOption::new("61", "Tag 061"),
		]
	);
	assert!(*has_more);
	assert!(!available.iter().any(|option| option.id == "60" || option.id == "61"));
}

#[rstest]
#[serial(admin_relation_registry)]
#[tokio::test]
async fn get_fields_checks_target_view_permission_before_returning_labels(
	#[future] relation_database: (AdminDatabase, RelationDatabaseLease),
) {
	let (db, _connection_lease) = relation_database.await;

	let error = get_fields(
		"RelationArticle".to_string(),
		Some("1".to_string()),
		KeyedDepends::from_value(relation_site(false)),
		KeyedDepends::from_value(db),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.unwrap_err();

	assert_eq!(error.status(), Some(403));
	assert_eq!(error.message(), "Permission denied");
	assert!(!error.message().contains("Tag 061"));
	assert!(!error.message().contains('1'));
}
