//! Integration tests for customized admin form mutations.

use super::server_fn_helpers::{TEST_CSRF_TOKEN, make_auth_user, make_staff_request};
use reinhardt_admin::adapters::MutationRequest;
use reinhardt_admin::core::{
	AdminDatabase, AdminForm, AdminFormData, AdminFormErrors, AdminFormMode, AdminFormResult,
	AdminRecord, AdminSite, AdminUser, ModelAdmin,
};
use reinhardt_admin::server::{create_record, update_record};
use reinhardt_db::backends::connection::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::dialect::PostgresBackend;
use reinhardt_db::migrations::{FieldMetadata, FieldType, ModelMetadata, global_registry};
use reinhardt_db::orm::DatabaseConnectionLease;
use reinhardt_di::KeyedDepends;
use reinhardt_pages::server_fn::ServerFnErrorKind;
use reinhardt_test::fixtures::shared_postgres::shared_db_pool;
use rstest::*;
use serde_json::{Value, json};
use serial_test::serial;
use sqlx::Executor;
use std::sync::Arc;

const APP_LABEL: &str = "admin_form_customization";
const MODEL_NAME: &str = "FormCustomizationModel";
const TABLE_NAME: &str = "admin_form_customization_records";

struct RegistryGuard;

impl Drop for RegistryGuard {
	fn drop(&mut self) {
		global_registry().remove_model(APP_LABEL, MODEL_NAME);
	}
}

#[derive(Debug)]
struct FormCustomizationForm;

impl AdminForm for FormCustomizationForm {
	fn normalize(
		&self,
		mode: AdminFormMode,
		mut data: AdminFormData,
	) -> AdminFormResult<AdminFormData> {
		if let Some(title) = data.get("title").and_then(Value::as_str) {
			data.insert(
				"title".to_owned(),
				Value::String(title.trim().to_lowercase()),
			);
		}
		let marker = match mode {
			AdminFormMode::Create => "created",
			AdminFormMode::Update => "updated",
		};
		data.insert("marker".to_owned(), Value::String(marker.to_owned()));
		Ok(data)
	}

	fn validate(&self, _mode: AdminFormMode, data: &AdminFormData) -> AdminFormResult<()> {
		if data.get("title") == Some(&json!("invalid")) {
			let mut errors = AdminFormErrors::field("title", "first title error");
			errors.push_field("title", "second title error");
			errors.push_global("form-wide error");
			return Err(errors);
		}
		Ok(())
	}
}

struct FormCustomizationAdmin;

#[async_trait::async_trait]
impl ModelAdmin for FormCustomizationAdmin {
	fn model_name(&self) -> &str {
		MODEL_NAME
	}

	fn table_name(&self) -> &str {
		TABLE_NAME
	}

	fn list_display(&self) -> Vec<&str> {
		vec!["id", "title", "marker"]
	}

	fn fields(&self) -> Option<Vec<&str>> {
		Some(vec!["title", "marker"])
	}

	fn form(&self) -> Option<&dyn AdminForm> {
		static FORM: FormCustomizationForm = FormCustomizationForm;
		Some(&FORM)
	}

	async fn has_view_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_add_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}

	async fn has_change_permission(&self, _user: &dyn AdminUser) -> bool {
		true
	}
}

struct FormCustomizationContext {
	site: KeyedDepends<reinhardt_admin::core::AdminSiteKey, AdminSite>,
	db: KeyedDepends<reinhardt_admin::core::AdminDatabaseKey, AdminDatabase>,
	_connection_lease: DatabaseConnectionLease,
	_registry: RegistryGuard,
}

#[fixture]
async fn form_customization_context(
	#[future] shared_db_pool: (sqlx::PgPool, String),
) -> FormCustomizationContext {
	let (pool, _) = shared_db_pool.await;
	pool.execute(format!("DROP TABLE IF EXISTS {TABLE_NAME}").as_str())
		.await
		.expect("form customization table should be removed");
	pool.execute(
		format!(
			"CREATE TABLE {TABLE_NAME} (\
				id SERIAL PRIMARY KEY, \
				title VARCHAR(255) NOT NULL, \
				marker VARCHAR(32) NOT NULL\
			)"
		)
		.as_str(),
	)
	.await
	.expect("form customization table should be created");

	let mut metadata = ModelMetadata::new(APP_LABEL, MODEL_NAME, TABLE_NAME);
	metadata.add_field("id".to_owned(), FieldMetadata::new(FieldType::Integer));
	metadata.add_field(
		"title".to_owned(),
		FieldMetadata::new(FieldType::VarChar(255)),
	);
	metadata.add_field(
		"marker".to_owned(),
		FieldMetadata::new(FieldType::VarChar(32)),
	);
	global_registry().register_model(metadata);

	let backend = Arc::new(PostgresBackend::new(pool));
	let connection_lease = DatabaseConnectionLease::register(BackendsConnection::new(backend))
		.expect("form customization connection should register");
	let mut connection = connection_lease.handle();
	super::server_fn_helpers::setup_admin_history_schema(&mut connection).await;
	let db = AdminDatabase::new(connection_lease.handle());
	let site = AdminSite::new("Form customization test site");
	site.register(MODEL_NAME, FormCustomizationAdmin)
		.expect("form customization admin should register");

	FormCustomizationContext {
		site: KeyedDepends::from_value(site),
		db: KeyedDepends::from_value(db),
		_connection_lease: connection_lease,
		_registry: RegistryGuard,
	}
}

#[rstest]
#[tokio::test]
#[serial(admin_form_customization)]
async fn server_fn_form_customization_normalizes_create_and_update_before_persistence(
	#[future] form_customization_context: FormCustomizationContext,
) {
	// Arrange
	let FormCustomizationContext {
		site,
		db,
		_connection_lease,
		_registry,
	} = form_customization_context.await;
	let create_request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_owned(),
		data: std::collections::HashMap::from([("title".to_owned(), json!(" Create Title "))]),
	};

	// Act
	create_record(
		MODEL_NAME.to_owned(),
		create_request,
		site.clone(),
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("create should persist normalized values");
	let created = db
		.get::<AdminRecord>(TABLE_NAME, "id", "1")
		.await
		.expect("created record lookup should succeed")
		.expect("created record should exist");

	let update_request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_owned(),
		data: std::collections::HashMap::from([("title".to_owned(), json!(" Update Title "))]),
	};
	update_record(
		MODEL_NAME.to_owned(),
		"1".to_owned(),
		update_request,
		site,
		db.clone(),
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect("update should persist normalized values");
	let updated = db
		.get::<AdminRecord>(TABLE_NAME, "id", "1")
		.await
		.expect("updated record lookup should succeed")
		.expect("updated record should exist");

	// Assert
	assert_eq!(created.get("title"), Some(&json!("create title")));
	assert_eq!(created.get("marker"), Some(&json!("created")));
	assert_eq!(updated.get("title"), Some(&json!("update title")));
	assert_eq!(updated.get("marker"), Some(&json!("updated")));
}

#[rstest]
#[tokio::test]
#[serial(admin_form_customization)]
async fn server_fn_form_customization_returns_structured_errors_in_form_order(
	#[future] form_customization_context: FormCustomizationContext,
) {
	// Arrange
	let FormCustomizationContext {
		site,
		db,
		_connection_lease,
		_registry,
	} = form_customization_context.await;
	let request = MutationRequest {
		csrf_token: TEST_CSRF_TOKEN.to_owned(),
		data: std::collections::HashMap::from([("title".to_owned(), json!("invalid"))]),
	};

	// Act
	let error = create_record(
		MODEL_NAME.to_owned(),
		request,
		site,
		db,
		make_staff_request(),
		make_auth_user(),
	)
	.await
	.expect_err("custom validation should reject invalid data");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(
		error
			.field_errors()
			.iter()
			.map(|error| (error.field(), error.message()))
			.collect::<Vec<_>>(),
		vec![
			("title", "first title error"),
			("title", "second title error"),
			("_all", "form-wide error"),
		]
	);
}
