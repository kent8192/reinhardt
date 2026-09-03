#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

include!("ui/form/model_json_support.rs");

#[cfg(feature = "model-server-fnset")]
use std::collections::HashMap;

use reinhardt_pages::{
	FieldError, FormRuntimeSource, form, server_fn::ServerFnErrorKind, use_form,
};
use rstest::rstest;

#[cfg(feature = "model-server-fnset")]
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
#[cfg(feature = "model-server-fnset")]
use reinhardt_db::orm::{FieldSelector, Manager, Model, inspection::FieldInfo};
#[cfg(feature = "model-server-fnset")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "model-server-fnset")]
#[derive(Clone)]
struct ConstraintQuestionFields;

#[cfg(feature = "model-server-fnset")]
impl FieldSelector for ConstraintQuestionFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

#[cfg(feature = "model-server-fnset")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ConstraintQuestion {
	id: Option<i64>,
	title: String,
}

#[cfg(feature = "model-server-fnset")]
impl Model for ConstraintQuestion {
	type PrimaryKey = i64;
	type Fields = ConstraintQuestionFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"questions"
	}

	fn new_fields() -> Self::Fields {
		ConstraintQuestionFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn field_metadata() -> Vec<FieldInfo> {
		vec![FieldInfo {
			name: "title".to_owned(),
			field_type: "CharField".to_owned(),
			storage_kind: None,
			domain: None,
			nullable: false,
			primary_key: false,
			unique: false,
			blank: false,
			editable: true,
			default: None,
			db_default: None,
			db_column: None,
			choices: None,
			attributes: HashMap::new(),
		}]
	}

	fn constraint_fields(constraint: &str) -> Option<Vec<&'static str>> {
		match constraint {
			"questions_title_key" | "questions_title_check" => Some(vec!["title"]),
			_ => None,
		}
	}
}

#[test]
fn native_submit_maps_payload_errors_to_validation() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = form! {
			name: QuestionForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		form.set_value("title", serde_json::json!("Rejected by payload mapping"))
			.expect("control state accepts a valid title");

		let payload_error = form
			.data()
			.expect_err("payload mapping must reject the title");
		assert_eq!(
			payload_error.to_string(),
			"invalid value for model form field 'title': payload mapping rejected title"
		);

		let submit_error = tokio_test::block_on(form.submit())
			.expect_err("native submit must preserve validation");
		assert_eq!(submit_error.kind(), ServerFnErrorKind::Validation);
		assert_eq!(submit_error.status(), Some(422));
		assert_eq!(submit_error.message(), payload_error.to_string());
		assert_eq!(submit_error.field_errors(), []);
	});
}

#[test]
fn model_form_routes_structured_server_errors_to_selected_fields() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = form! {
			name: QuestionForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let runtime = use_form(&form).build();
		let error = reinhardt_pages::ServerFnError::validation_with_message(
			"Please correct the submitted values",
			[
				("title", "Title is already used"),
				("owner_id", "Owner is required"),
			],
		);

		runtime.apply_server_error(&error);

		assert_eq!(
			runtime
				.get_field_state(form.title_field())
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Title is already used")
		);
		assert_eq!(
			runtime.form_state().form_error.get(),
			Some("Please correct the submitted values\nowner_id: Owner is required".to_owned())
		);
	});
}

#[cfg(feature = "model-server-fnset")]
#[test]
fn generated_model_form_routes_serialized_database_constraint_errors() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let unique_form = form! {
			name: ConstraintQuestionUniqueForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let unique_runtime = use_form(&unique_form).build();
		let unique = ServerFnError::try_from_model_error::<ConstraintQuestion>(Error::from(
			DatabaseError::new(DatabaseErrorKind::UniqueViolation, "private duplicate")
				.with_table("questions")
				.with_constraint("questions_title_key")
				.with_columns(["title"]),
		))
		.expect("known unique constraint converts");
		let serialized = serde_json::to_string(&unique).expect("safe error serializes");
		let unique: ServerFnError =
			serde_json::from_str(&serialized).expect("safe error deserializes");
		unique_runtime.apply_server_error(&unique);
		assert_eq!(
			unique_runtime
				.get_field_state(unique_form.title_field())
				.error
				.as_ref()
				.map(FieldError::message),
			Some("A record with this value already exists")
		);

		let check_form = form! {
			name: ConstraintQuestionCheckForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let check_runtime = use_form(&check_form).build();
		let check = ServerFnError::try_from_model_error::<ConstraintQuestion>(Error::from(
			DatabaseError::new(DatabaseErrorKind::CheckViolation, "private check")
				.with_table("questions")
				.with_constraint("questions_title_check"),
		))
		.expect("known CHECK constraint converts");
		check_runtime.apply_server_error(&check);
		assert_eq!(
			check_runtime.form_state().form_error.get(),
			Some("The submitted values violate a data constraint".to_owned())
		);
		assert!(
			check_runtime
				.get_field_state(check_form.title_field())
				.error
				.is_none()
		);
	});
}

#[test]
fn model_form_runtime_mutations_track_explicit_and_excluded_fields() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let explicit_form = form! {
			name: QuestionExplicitRuntimeForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let explicit_runtime = use_form(&explicit_form).build();
		explicit_runtime.set_value(explicit_form.title_field(), "changed".to_owned());
		assert!(
			explicit_runtime
				.get_field_state(explicit_form.title_field())
				.is_touched
		);
		assert!(
			explicit_runtime
				.get_field_state(explicit_form.title_field())
				.is_dirty
		);
		explicit_runtime.reset_field(explicit_form.title_field());
		assert!(
			!explicit_runtime
				.get_field_state(explicit_form.title_field())
				.is_dirty
		);

		let excluded_form = form! {
			name: QuestionExcludedRuntimeForm,
			model: Question,
			policy: QuestionPolicy,
			exclude: [owner_id],
			server_fn: save_question,
		};
		let excluded_field = excluded_form
			.field("title")
			.expect("policy-allowed excluded-form fields resolve by name");
		assert!(excluded_form.field("owner_id").is_none());
		let excluded_runtime = use_form(&excluded_form).build();
		excluded_runtime.set_value(excluded_field, "changed".to_owned());
		assert!(excluded_runtime.get_field_state(excluded_field).is_touched);
		assert!(excluded_runtime.get_field_state(excluded_field).is_dirty);
	});
}

#[rstest]
fn model_form_reset_clears_generated_submission_state() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = form! {
			name: QuestionResetForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		form.loading().set(true);
		form.error().set(Some("stale error".to_owned()));
		form.success().set(true);

		// Act
		form.runtime_reset_state();

		// Assert
		assert!(!form.loading().get());
		assert_eq!(form.error().get(), None);
		assert!(!form.success().get());
		assert_eq!(form.title_field().name(), "title");
	});
}
