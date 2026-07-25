//! End-to-end coverage for generated model-backed forms.

use reinhardt_core::exception::{DatabaseErrorKind, Error};
use reinhardt_core::macros::model;
use reinhardt_core::model_form::{ModelFormPayload, ModelFormPolicy};
use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
use reinhardt_db::orm::{DatabaseConnection, DatabaseConnectionLease, Model};
use reinhardt_forms::{ModelForm, ModelFormError};
use reinhardt_pages::form::ModelFormState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tempfile::TempDir;

const DEFAULT_AUDIT_TOKEN: &str = "model-form-created";

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
	payload.set_title(title.to_owned());
	payload.set_owner_id(owner_id);
	payload
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
	update_payload.set_title("Updated article".to_owned());
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
