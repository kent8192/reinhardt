//! End-to-end coverage for generated model-backed forms.

use reinhardt_core::exception::{DatabaseErrorKind, Error};
use reinhardt_core::macros::model;
use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormPayload, ModelFormPolicy, ModelFormUpdatingPayload,
	ModelFormValidatingPayload,
};
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
use reinhardt_db::orm::{DatabaseConnection, DatabaseConnectionLease, Model};
use reinhardt_forms::{ModelForm, ModelFormError};
use reinhardt_pages::form::ModelFormState;
use reinhardt_pages::server_fn::{ServerFnError, ServerFnErrorKind};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

const DEFAULT_AUDIT_TOKEN: &str = "model-form-created";
static MISSING_CLUSTER_SYNC_VALIDATOR_CALLS: AtomicUsize = AtomicUsize::new(0);
static MISSING_CLUSTER_ASYNC_VALIDATOR_CALLS: AtomicUsize = AtomicUsize::new(0);

#[model(app_label = "forms_test", form = true)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Article {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 120, unique = true)]
	title: String,
	#[field(max_length = 240)]
	nullable_note: Option<String>,
	owner_id: i64,
	#[field(max_length = 64, editable = false, default = "model-form-created")]
	audit_token: String,
}

fn validate_cluster<P: ModelFormPolicy>(
	payload: &CleanedClusterModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if payload.name().is_none() || payload.api_url().is_none() {
		MISSING_CLUSTER_SYNC_VALIDATOR_CALLS.fetch_add(1, Ordering::SeqCst);
	}
	let mut errors = ValidationErrors::new();
	if payload.name() == payload.api_url() {
		errors.add(
			"_all",
			ValidationError::Custom("Name and API URL must differ".to_owned()),
		);
	}
	if errors.is_empty() {
		Ok(())
	} else {
		Err(errors)
	}
}

#[model(app_label = "forms_test", form = true, info = false)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[form(validate = validate_cluster)]
struct Cluster {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(editable = false)]
	organization_id: i64,
	#[field(min_length = 1, max_length = 63)]
	#[form(trim)]
	name: String,
	#[field(url = true, max_length = 2048)]
	#[form(trim)]
	api_url: String,
	#[field(blank = true, max_length = 120)]
	notes: String,
}

struct ClusterPolicy;

impl ModelFormPolicy for ClusterPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "name" | "api_url" | "notes")
	}
}

#[model(app_label = "forms_test", form = true)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct EmailRecord {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(email = true, max_length = 200)]
	#[form(trim)]
	email: String,
}

fn reject_required_scalar_candidate<P: ModelFormPolicy>(
	_payload: &CleanedRequiredScalarRecordModelFormData<P>,
) -> Result<(), ValidationErrors> {
	let mut errors = ValidationErrors::new();
	errors.add(
		"_all",
		ValidationError::Custom("required scalar callback must not run".to_owned()),
	);
	Err(errors)
}

#[model(app_label = "forms_test", form = true, info = false)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[form(validate = reject_required_scalar_candidate)]
struct RequiredScalarRecord {
	#[field(primary_key = true)]
	id: Option<i64>,
	enabled: bool,
	replicas: i64,
}

struct ClusterNameOnlyPolicy;

impl ModelFormPolicy for ClusterNameOnlyPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "name" | "notes")
	}
}

struct ArticleFormPolicy;

impl ModelFormPolicy for ArticleFormPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "title" | "nullable_note")
	}
}

struct SqliteFixture {
	connection: DatabaseConnection,
	_lease: DatabaseConnectionLease,
	_directory: TempDir,
}

async fn sqlite_fixture() -> SqliteFixture {
	let directory = tempfile::Builder::new()
		.prefix("reinhardt-model-form-")
		.tempdir_in("/tmp")
		.expect("SQLite temporary directory should be created under /tmp");
	let database_path = directory.path().join("forms.sqlite");
	let database_url = format!("sqlite:///{}", database_path.display());
	let owner = BackendsConnection::connect_sqlite(&database_url)
		.await
		.expect("SQLite connection should open");
	let lease =
		DatabaseConnectionLease::register(owner).expect("SQLite connection should be registered");
	let connection = lease.handle();
	connection
		.execute(
			"CREATE TABLE forms_test_article (
				id INTEGER PRIMARY KEY AUTOINCREMENT,
				title TEXT NOT NULL UNIQUE,
				nullable_note TEXT,
				owner_id INTEGER NOT NULL,
				audit_token TEXT NOT NULL
			)",
			vec![],
		)
		.await
		.expect("article table should be created");

	SqliteFixture {
		connection,
		_lease: lease,
		_directory: directory,
	}
}

fn article_payload(title: &str, owner_id: i64) -> ArticleModelFormData<ArticleFormPolicy> {
	let mut payload = ArticleModelFormData::<ArticleFormPolicy>::empty();
	payload
		.set_title(title.to_owned())
		.expect("article title should be editable");
	payload.set_trusted_owner_id(owner_id);
	payload
}

fn cluster_payload(name: &str, api_url: &str, notes: &str) -> ClusterModelFormData<ClusterPolicy> {
	let mut payload = ClusterModelFormData::<ClusterPolicy>::empty();
	payload
		.set_name(name.to_owned())
		.expect("cluster name should be editable");
	payload
		.set_api_url(api_url.to_owned())
		.expect("cluster API URL should be editable");
	payload
		.set_notes(notes.to_owned())
		.expect("cluster notes should be editable");
	payload
}

fn validation_error_tuples(errors: &ValidationErrors) -> Vec<(String, String)> {
	errors
		.ordered_field_errors()
		.flat_map(|(field, errors)| {
			errors.iter().map(move |error| {
				let message = match error {
					ValidationError::Custom(message) => message.clone(),
					_ => error.to_string(),
				};
				(field.to_owned(), message)
			})
		})
		.collect()
}

fn cluster_validation_errors<P: ModelFormPolicy>(
	payload: ClusterModelFormData<P>,
) -> ValidationErrors {
	match payload.clean_and_validate() {
		Ok(_) => panic!("cluster payload should fail validation"),
		Err(errors) => errors,
	}
}

async fn ensure_cluster_name_available<P: ModelFormPolicy>(
	payload: &CleanedClusterModelFormData<P>,
) -> Result<(), ValidationErrors> {
	if payload.name().is_none() || payload.api_url().is_none() {
		MISSING_CLUSTER_ASYNC_VALIDATOR_CALLS.fetch_add(1, Ordering::SeqCst);
	}
	let mut errors = ValidationErrors::new();
	if payload.name().is_some_and(|name| name == "taken") {
		errors.add(
			"name",
			ValidationError::Custom("A cluster with this name already exists".to_owned()),
		);
	}
	if errors.is_empty() {
		Ok(())
	} else {
		Err(errors)
	}
}

async fn create_cluster(
	payload: ClusterModelFormData<ClusterPolicy>,
) -> Result<Cluster, ServerFnError> {
	let cleaned = payload.clean_and_validate()?;
	ensure_cluster_name_available(&cleaned).await?;
	cleaned
		.into_model(ClusterModelFormServerContext::new().organization_id(7))
		.map_err(|error| ServerFnError::application(error.to_string()))
}

async fn update_cluster(
	payload: ClusterModelFormData<ClusterPolicy>,
	existing: Cluster,
) -> Result<Cluster, ServerFnError> {
	let cleaned = payload.clean_and_validate_for_update(&existing)?;
	ensure_cluster_name_available(&cleaned).await?;
	cleaned
		.apply_to(existing)
		.map_err(|error| ServerFnError::application(error.to_string()))
}

async fn create_required_scalar_record(
	payload: RequiredScalarRecordModelFormData<AllEditableModelFields>,
) -> Result<RequiredScalarRecord, ServerFnError> {
	payload
		.clean_and_validate()?
		.into_model()
		.map_err(|error| ServerFnError::application(error.to_string()))
}

#[rstest]
fn generated_cluster_pipeline_normalizes_before_validation() {
	// Arrange
	let payload = cluster_payload(
		&format!(" {} ", "n".repeat(63)),
		"  https://example.com/api  ",
		"  preserve surrounding whitespace  ",
	);

	// Act
	let cleaned = payload
		.clean_and_validate()
		.expect("normalized boundary values should validate");

	// Assert
	assert_eq!(cleaned.name(), Some(&"n".repeat(63)));
	assert_eq!(
		cleaned.api_url(),
		Some(&"https://example.com/api".to_owned())
	);
	assert_eq!(
		cleaned.notes(),
		Some(&"  preserve surrounding whitespace  ".to_owned())
	);
}

#[rstest]
#[case("   ", "https://example.com", "name", "This field is required.")]
#[case(
	" nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn ",
	"https://example.com",
	"name",
	"Ensure this value has at most 63 characters (it has 64)"
)]
#[case("valid", "  not a URL  ", "api_url", "Enter a valid URL")]
fn generated_cluster_field_validation_observes_normalized_values(
	#[case] name: &str,
	#[case] api_url: &str,
	#[case] field: &str,
	#[case] message: &str,
) {
	// Arrange
	let payload = cluster_payload(name, api_url, "");

	// Act
	let errors = cluster_validation_errors(payload);

	// Assert
	assert_eq!(
		validation_error_tuples(&errors),
		vec![(field.to_owned(), message.to_owned())]
	);
}

#[rstest]
fn generated_cluster_cross_field_validation_observes_cleaned_values() {
	// Arrange
	let payload = cluster_payload("  https://example.com  ", "https://example.com  ", "");

	// Act
	let errors = cluster_validation_errors(payload);

	// Assert
	assert_eq!(
		validation_error_tuples(&errors),
		vec![("_all".to_owned(), "Name and API URL must differ".to_owned(),)]
	);
}

#[rstest]
fn generated_cluster_accumulates_field_errors_in_schema_order() {
	// Arrange
	let payload = cluster_payload(&"n".repeat(64), "not a URL", "");

	// Act
	let errors = cluster_validation_errors(payload);

	// Assert
	assert_eq!(
		validation_error_tuples(&errors),
		vec![
			(
				"name".to_owned(),
				"Ensure this value has at most 63 characters (it has 64)".to_owned(),
			),
			("api_url".to_owned(), "Enter a valid URL".to_owned()),
		]
	);
}

#[rstest]
fn generated_unannotated_text_preserves_whitespace() {
	// Arrange
	let mut payload = ArticleModelFormData::<ArticleFormPolicy>::empty();
	payload
		.set_title("  article title  ".to_owned())
		.expect("article title should be editable");

	// Act
	let cleaned = payload
		.clean_and_validate()
		.expect("unannotated text should remain valid");

	// Assert
	assert_eq!(cleaned.title(), Some(&"  article title  ".to_owned()));
}

#[rstest]
#[case("unknown")]
#[case("organization_id")]
#[case("id")]
fn generated_cluster_strict_decode_rejects_untrusted_fields(#[case] rejected_field: &str) {
	// Arrange
	let mut value = json!({"name": "cluster", "api_url": "https://example.com"});
	value
		.as_object_mut()
		.expect("cluster payload should be an object")
		.insert(rejected_field.to_owned(), json!(true));

	// Act
	let error = match serde_json::from_value::<ClusterModelFormData<ClusterPolicy>>(value) {
		Ok(_) => panic!("untrusted field should be rejected"),
		Err(error) => error,
	};

	// Assert
	assert_eq!(
		error.to_string(),
		format!("unknown field `{rejected_field}`, expected one of `name`, `api_url`, `notes`")
	);
}

#[rstest]
fn generated_cluster_policy_error_precedes_cross_field_validation() {
	// Arrange
	let payload: ClusterModelFormData<ClusterNameOnlyPolicy> = serde_json::from_value(json!({
		"name": "https://example.com",
		"api_url": "https://example.com",
	}))
	.expect("known policy-forbidden input should retain rejection evidence");

	// Act
	let errors = cluster_validation_errors(payload);
	let server_error = ServerFnError::from(errors);

	// Assert
	assert_eq!(server_error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(server_error.status(), Some(422));
	assert_eq!(server_error.field_errors().len(), 1);
	assert_eq!(server_error.field_errors()[0].field(), "api_url");
	assert_eq!(
		server_error.field_errors()[0].message(),
		"This field is not allowed."
	);
}

#[rstest]
#[case("   ", "https://example.com", "name", "This field is required.")]
#[case(
	"https://example.com",
	"https://example.com",
	"_all",
	"Name and API URL must differ"
)]
#[case(
	"taken",
	"https://example.com",
	"name",
	"A cluster with this name already exists"
)]
#[tokio::test]
async fn direct_cluster_handler_revalidates_hostile_payloads(
	#[case] name: &str,
	#[case] api_url: &str,
	#[case] field: &str,
	#[case] message: &str,
) {
	// Arrange
	let payload = cluster_payload(name, api_url, "hostile direct call");

	// Act
	let error = create_cluster(payload)
		.await
		.expect_err("hostile direct handler input should not construct a model");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(error.field_errors().len(), 1);
	assert_eq!(error.field_errors()[0].field(), field);
	assert_eq!(error.field_errors()[0].message(), message);
}

#[rstest]
#[tokio::test]
#[serial(model_form_missing_cluster_callbacks)]
async fn direct_create_rejects_omitted_required_fields_before_callbacks() {
	// Arrange
	MISSING_CLUSTER_SYNC_VALIDATOR_CALLS.store(0, Ordering::SeqCst);
	MISSING_CLUSTER_ASYNC_VALIDATOR_CALLS.store(0, Ordering::SeqCst);
	let mut payload = ClusterModelFormData::<ClusterPolicy>::empty();
	payload
		.set_api_url("https://example.com".to_owned())
		.expect("cluster API URL should be editable");
	payload
		.set_notes("missing name".to_owned())
		.expect("cluster notes should be editable");

	// Act
	let error = create_cluster(payload)
		.await
		.expect_err("missing required create input should fail at field validation");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(error.field_errors().len(), 1);
	assert_eq!(error.field_errors()[0].field(), "name");
	assert_eq!(error.field_errors()[0].message(), "This field is required.");
	assert_eq!(
		MISSING_CLUSTER_SYNC_VALIDATOR_CALLS.load(Ordering::SeqCst),
		0
	);
	assert_eq!(
		MISSING_CLUSTER_ASYNC_VALIDATOR_CALLS.load(Ordering::SeqCst),
		0
	);
}

#[rstest]
#[tokio::test]
async fn direct_create_reports_canonical_required_scalar_errors_before_the_callback() {
	// Arrange
	let payload = RequiredScalarRecordModelFormData::<AllEditableModelFields>::empty();

	// Act
	let error = create_required_scalar_record(payload)
		.await
		.expect_err("omitted required scalars must fail before the callback");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(
		error
			.field_errors()
			.iter()
			.map(|error| (error.field().to_owned(), error.message().to_owned()))
			.collect::<Vec<_>>(),
		vec![
			("enabled".to_owned(), "This field is required.".to_owned()),
			("replicas".to_owned(), "This field is required.".to_owned()),
		]
	);
}

#[rstest]
#[tokio::test]
async fn direct_create_aggregates_omitted_required_and_supplied_invalid_fields() {
	// Arrange
	let mut payload = ClusterModelFormData::<ClusterPolicy>::empty();
	payload
		.set_api_url("not a URL".to_owned())
		.expect("cluster API URL should be editable");

	// Act
	let error = create_cluster(payload)
		.await
		.expect_err("all create field errors should be reported before the callback");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(
		error
			.field_errors()
			.iter()
			.map(|error| (error.field().to_owned(), error.message().to_owned()))
			.collect::<Vec<_>>(),
		vec![
			("name".to_owned(), "This field is required.".to_owned()),
			("api_url".to_owned(), "Enter a valid URL".to_owned()),
		]
	);
}

#[rstest]
#[case(
	serde_json::json!({"enabled": null, "replicas": 1}),
	"invalid type: null, expected a boolean"
)]
#[case(
	serde_json::json!({"enabled": true, "replicas": null}),
	"invalid type: null, expected i64"
)]
fn generated_nonnullable_scalar_nulls_fail_during_wire_decode(
	#[case] wire: serde_json::Value,
	#[case] expected: &str,
) {
	// Arrange and Act
	let error = match serde_json::from_value::<
		RequiredScalarRecordModelFormData<AllEditableModelFields>,
	>(wire)
	{
		Ok(_) => panic!("non-nullable scalar null must fail during payload decoding"),
		Err(error) => error,
	};

	// Assert
	assert_eq!(error.to_string(), expected);
}

#[rstest]
fn generated_required_email_uses_the_canonical_message() {
	let missing = EmailRecordModelFormData::<AllEditableModelFields>::empty();
	let missing_errors = match missing.clean_and_validate() {
		Ok(_) => panic!("missing required email should fail"),
		Err(errors) => errors,
	};

	assert_eq!(
		validation_error_tuples(&missing_errors),
		vec![("email".to_owned(), "This field is required.".to_owned())]
	);

	let mut whitespace = EmailRecordModelFormData::<AllEditableModelFields>::empty();
	whitespace
		.set_email("   ".to_owned())
		.expect("email should be editable");
	let whitespace_errors = match whitespace.clean_and_validate() {
		Ok(_) => panic!("trimmed empty required email should fail"),
		Err(errors) => errors,
	};
	assert_eq!(
		validation_error_tuples(&whitespace_errors),
		vec![("email".to_owned(), "This field is required.".to_owned())]
	);
}

#[rstest]
#[tokio::test]
async fn direct_partial_update_validates_the_post_merge_candidate() {
	// Arrange
	let mut payload = ClusterModelFormData::<ClusterPolicy>::empty();
	payload
		.set_name("https://same.example.com".to_owned())
		.expect("cluster name should be editable");
	let existing = Cluster {
		id: Some(42),
		organization_id: 19,
		name: "original".to_owned(),
		api_url: "https://same.example.com".to_owned(),
		notes: "preserve me".to_owned(),
	};

	// Act
	let error = update_cluster(payload, existing)
		.await
		.expect_err("post-merge cross-field validation should reject the update");

	// Assert
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(error.field_errors().len(), 1);
	assert_eq!(error.field_errors()[0].field(), "_all");
	assert_eq!(
		error.field_errors()[0].message(),
		"Name and API URL must differ"
	);
}

#[rstest]
#[tokio::test]
async fn direct_cluster_handler_normalizes_and_supplies_server_context() {
	// Arrange
	let payload = cluster_payload(
		"  production  ",
		"  https://example.com/api  ",
		"  keep notes  ",
	);

	// Act
	let cluster = create_cluster(payload)
		.await
		.expect("valid direct handler input should construct a cluster");

	// Assert
	assert_eq!(cluster.id, None);
	assert_eq!(cluster.organization_id, 7);
	assert_eq!(cluster.name, "production");
	assert_eq!(cluster.api_url, "https://example.com/api");
	assert_eq!(cluster.notes, "  keep notes  ");
}

#[rstest]
fn generated_cluster_update_preserves_server_owned_values() {
	// Arrange
	let payload = cluster_payload(
		"  updated  ",
		"  https://updated.example.com/api  ",
		"updated notes",
	);
	let cleaned = payload
		.clean_and_validate()
		.expect("valid update payload should clean");
	let existing = Cluster {
		id: Some(42),
		organization_id: 19,
		name: "original".to_owned(),
		api_url: "https://original.example.com".to_owned(),
		notes: "original notes".to_owned(),
	};

	// Act
	let updated = cleaned
		.apply_to(existing)
		.expect("cleaned values should apply to an existing cluster");

	// Assert
	assert_eq!(updated.id, Some(42));
	assert_eq!(updated.organization_id, 19);
	assert_eq!(updated.name, "updated");
	assert_eq!(updated.api_url, "https://updated.example.com/api");
	assert_eq!(updated.notes, "updated notes");
}

#[tokio::test]
async fn generated_model_form_creates_and_queries_article() {
	// Arrange
	let mut fixture = sqlite_fixture().await;
	let payload = article_payload("Created article", 41);
	let mut form = ModelForm::<Article, ArticleFormPolicy>::from_payload(payload);

	// Act
	let saved = form
		.save(&mut fixture.connection)
		.await
		.expect("generated model form should create an article");
	let persisted = Article::objects()
		.get(saved.id.expect("created article should have an identifier"))
		.get_with_db(&mut fixture.connection)
		.await
		.expect("created article should be queried back");

	// Assert
	assert_eq!(
		persisted,
		Article {
			id: saved.id,
			title: "Created article".to_owned(),
			nullable_note: None,
			owner_id: 41,
			audit_token: DEFAULT_AUDIT_TOKEN.to_owned(),
		}
	);
}

#[tokio::test]
async fn generated_model_form_updates_title_and_preserves_omitted_values() {
	// Arrange
	let mut fixture = sqlite_fixture().await;
	let original = Article::objects()
		.create_with_conn(
			&mut fixture.connection,
			&Article {
				id: None,
				title: "Original article".to_owned(),
				nullable_note: None,
				owner_id: 73,
				audit_token: "preexisting-audit-token".to_owned(),
			},
		)
		.await
		.expect("preexisting article should be persisted through the ORM");
	let mut update_payload = ArticleModelFormData::<ArticleFormPolicy>::empty();
	update_payload
		.set_title("Updated article".to_owned())
		.expect("article title should be editable");
	let mut update_form = ModelForm::<Article, ArticleFormPolicy>::from_payload_and_instance(
		update_payload,
		original,
	);

	// Act
	let updated = update_form
		.save(&mut fixture.connection)
		.await
		.expect("generated model form should update the existing article");
	let persisted = Article::objects()
		.get(
			updated
				.id
				.expect("updated article should retain its identifier"),
		)
		.get_with_db(&mut fixture.connection)
		.await
		.expect("updated article should be queried back");

	// Assert
	assert_eq!(
		persisted,
		Article {
			id: updated.id,
			title: "Updated article".to_owned(),
			nullable_note: None,
			owner_id: 73,
			audit_token: "preexisting-audit-token".to_owned(),
		}
	);
}

#[tokio::test]
async fn generated_model_form_pages_clear_updates_nullable_column_to_null() {
	// Arrange
	let mut fixture = sqlite_fixture().await;
	let original = Article::objects()
		.create_with_conn(
			&mut fixture.connection,
			&Article {
				id: None,
				title: "Nullable article".to_owned(),
				nullable_note: Some("remove this note".to_owned()),
				owner_id: 91,
				audit_token: "nullable-audit-token".to_owned(),
			},
		)
		.await
		.expect("article with nullable text should be persisted");
	let mut state = ModelFormState::<ArticleFormSchema, ArticleFormPolicy>::new();
	state
		.set_value("nullable_note", json!(""))
		.expect("empty nullable control should be accepted");
	let payload = state
		.build_payload::<ArticleModelFormData<ArticleFormPolicy>>()
		.expect("nullable clear should assemble a generated payload");
	let mut form =
		ModelForm::<Article, ArticleFormPolicy>::from_payload_and_instance(payload, original);

	// Act
	let updated = form
		.save(&mut fixture.connection)
		.await
		.expect("nullable clear should update the article");
	let persisted = Article::objects()
		.get(
			updated
				.id
				.expect("updated article should keep its identifier"),
		)
		.get_with_db(&mut fixture.connection)
		.await
		.expect("updated article should be queried back");

	// Assert
	assert_eq!(updated.nullable_note, None);
	assert_eq!(persisted.nullable_note, None);
	assert_eq!(persisted.owner_id, 91);
	assert_eq!(persisted.audit_token, "nullable-audit-token");
}

#[test]
fn generated_model_form_rejects_forbidden_public_payload_field() {
	// Arrange
	let payload: ArticleModelFormData<ArticleFormPolicy> = serde_json::from_value(json!({
		"title": "Hostile article",
		"owner_id": 999,
	}))
	.expect("known public payload fields should deserialize");
	assert_eq!(payload.forbidden_fields(), ["owner_id"]);
	let mut form = ModelForm::<Article, ArticleFormPolicy>::from_payload(payload);

	// Act
	let error = form
		.build_instance()
		.expect_err("forbidden wire input should prevent candidate construction");

	// Assert
	assert_eq!(error, ModelFormError::ForbiddenInput { field: "owner_id" });
}

#[tokio::test]
async fn generated_model_form_retains_unique_violation_error_kind() {
	// Arrange
	let mut fixture = sqlite_fixture().await;
	let mut first_form =
		ModelForm::<Article, ArticleFormPolicy>::from_payload(article_payload("Unique title", 1));
	first_form
		.save(&mut fixture.connection)
		.await
		.expect("first unique title should be persisted");
	let mut duplicate_form =
		ModelForm::<Article, ArticleFormPolicy>::from_payload(article_payload("Unique title", 2));

	// Act
	let error = duplicate_form
		.save(&mut fixture.connection)
		.await
		.expect_err("duplicate title should be rejected");

	// Assert
	assert_eq!(
		error
			.database_error()
			.expect("persistence error should retain its database classification")
			.kind(),
		DatabaseErrorKind::UniqueViolation,
	);
}

#[tokio::test]
async fn generated_model_form_save_rolls_back_with_atomic_error() {
	// Arrange
	let mut fixture = sqlite_fixture().await;
	let mut form =
		ModelForm::<Article, ArticleFormPolicy>::from_payload(article_payload("Rolled back", 52));

	// Act
	let result: Result<(), Error> = fixture
		.connection
		.atomic(async |transaction| {
			form.save(transaction)
				.await
				.map_err(|error| Error::Internal(error.to_string()))?;
			Err(Error::Validation("rollback after save".to_owned()))
		})
		.await;

	// Assert
	match result {
		Err(Error::Validation(message)) => assert_eq!(message, "rollback after save"),
		other => panic!("expected validation error after save, got {other:?}"),
	}
	let persisted = Article::objects()
		.all()
		.all_with_db(&mut fixture.connection)
		.await
		.expect("rolled-back article query should succeed");
	assert_eq!(persisted, Vec::<Article>::new());
}
