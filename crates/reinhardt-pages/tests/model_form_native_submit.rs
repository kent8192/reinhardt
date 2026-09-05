#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

include!("ui/form/model_json_support.rs");

#[cfg(feature = "model-server-fnset")]
use std::collections::HashMap;

use reinhardt_pages::{FieldError, form, server_fn::ServerFnErrorKind, use_form};
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

#[rstest]
#[case::missing_selected_title(
	None,
	Err(ServerFnError::validation([("title", "This field is required.")]))
)]
#[case::valid_subset(Some("Valid title"), Ok(()))]
#[case::selected_application_validation(
	Some("Rejected by validation"),
	Err(ServerFnError::validation([
		("title", "Title is rejected"),
		("_all", "Question is rejected"),
	]))
)]
fn native_generated_submit_validates_only_selected_fields(
	#[case] title: Option<&str>,
	#[case] expected: Result<(), ServerFnError>,
) {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let explicit_form = form! {
			name: QuestionSelectedSubmitForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let excluded_form = form! {
			name: QuestionExcludedSubmitForm,
			model: Question,
			policy: QuestionPolicy,
			exclude: [owner_id],
			server_fn: save_question,
		};
		if let Some(title) = title {
			explicit_form
				.set_value("title", serde_json::json!(title))
				.expect("selected title accepts text");
			excluded_form
				.set_value("title", serde_json::json!(title))
				.expect("non-excluded title accepts text");
		}

		// Act
		let explicit_result = tokio_test::block_on(explicit_form.submit());
		let excluded_result = tokio_test::block_on(excluded_form.submit());

		// Assert
		assert_eq!(explicit_result, expected);
		assert_eq!(excluded_result, expected);
		if expected.is_ok() {
			let payload: QuestionModelFormData<QuestionPolicy> = explicit_form
				.data()
				.expect("the public payload retains the endpoint policy");
			let errors = match payload.clean_and_validate() {
				Ok(_) => panic!("the broader endpoint policy still requires owner_id"),
				Err(errors) => errors,
			};
			assert_eq!(
				ServerFnError::from(errors),
				ServerFnError::validation([("owner_id", "This field is required.")]),
			);
		}
	});
}

#[rstest]
fn native_generated_submit_routes_snapshot_errors_without_dispatch() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = form! {
			name: QuestionForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let runtime = use_form(&form).build();
		reinhardt_core::reactive::with_runtime(|runtime| runtime.flush_updates());
		form.set_value("title", serde_json::json!("Rejected by validation"))
			.expect("control state accepts a valid title");

		let payload = form
			.data()
			.expect("raw payload assembly should precede generated validation");
		assert_eq!(
			payload.get_json("title"),
			Some(serde_json::json!("Rejected by validation"))
		);

		// Act
		let submit_error = tokio_test::block_on(form.submit())
			.expect_err("native submit must preserve validation");
		assert_eq!(
			reinhardt_pages::FormRuntimeSource::runtime_server_error(&form),
			Some(submit_error.clone())
		);
		reinhardt_core::reactive::with_runtime(|runtime| runtime.flush_updates());

		// Assert
		assert_eq!(submit_error.kind(), ServerFnErrorKind::Validation);
		assert_eq!(submit_error.status(), Some(422));
		assert_eq!(submit_error.message(), "Validation failed");
		assert_eq!(submit_error.field_errors().len(), 2);
		assert_eq!(submit_error.field_errors()[0].field(), "title");
		assert_eq!(
			submit_error.field_errors()[0].message(),
			"Title is rejected"
		);
		assert_eq!(submit_error.field_errors()[1].field(), "_all");
		assert_eq!(
			submit_error.field_errors()[1].message(),
			"Question is rejected"
		);
		assert_eq!(
			runtime
				.get_field_state(form.title_field())
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Title is rejected")
		);
		assert_eq!(
			runtime.form_state().form_error.get(),
			Some("Validation failed\n_all: Question is rejected".to_owned())
		);
	});
}

#[rstest]
fn late_use_form_subscriber_replays_generated_submit_errors_on_build() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = form! {
			name: QuestionLateSubscriberForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		form.set_value("title", serde_json::json!("Rejected by validation"))
			.expect("control state accepts a valid title");
		let submit_error = tokio_test::block_on(form.submit())
			.expect_err("native submit must preserve validation");

		// Act
		let runtime = use_form(&form).build();

		// Assert
		assert_eq!(submit_error.kind(), ServerFnErrorKind::Validation);
		assert_eq!(
			runtime
				.get_field_state(form.title_field())
				.error
				.as_ref()
				.map(FieldError::message),
			Some("Title is rejected")
		);
		assert_eq!(
			runtime.form_state().form_error.get(),
			Some("Validation failed\n_all: Question is rejected".to_owned())
		);
	});
}

#[rstest]
fn clear_errors_invalidates_generated_submit_error_cache() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = form! {
			name: QuestionClearErrorsForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let runtime = use_form(&form).build();
		form.set_value("title", serde_json::json!("Rejected by validation"))
			.expect("control state accepts a valid title");
		tokio_test::block_on(form.submit()).expect_err("native submit must preserve validation");
		assert!(reinhardt_pages::FormRuntimeSource::runtime_server_error(&form).is_some());

		// Act
		runtime.clear_errors();
		drop(runtime);
		let rebuilt = use_form(&form).build();

		// Assert
		assert!(reinhardt_pages::FormRuntimeSource::runtime_server_error(&form).is_none());
		assert!(rebuilt.get_field_state(form.title_field()).error.is_none());
		assert_eq!(rebuilt.form_state().form_error.get(), None);
	});
}

#[rstest]
fn model_form_runtime_prunes_dropped_server_error_handlers() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = form! {
			name: QuestionHandlerLifecycleForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let runtime = use_form(&form).build();
		let handler_counts = || {
			let handlers = form.__server_error_handlers.borrow();
			(
				handlers.len(),
				handlers
					.iter()
					.filter(|handler| handler.upgrade().is_some())
					.count(),
			)
		};

		assert_eq!(handler_counts(), (1, 1));
		drop(runtime);
		assert_eq!(handler_counts(), (1, 0));

		let replacement = use_form(&form).build();
		assert_eq!(handler_counts(), (1, 1));
		drop(replacement);
	});
}

#[rstest]
fn model_form_routes_structured_server_errors_to_selected_fields() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
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

		// Act
		runtime.apply_server_error(&error);

		// Assert
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

#[rstest]
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
fn generated_runtime_setter_rejects_descriptor_type_mismatch_before_storage() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = form! {
			name: QuestionTypedRuntimeForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let runtime = use_form(&form).build();

		// Act
		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			runtime.set_value(form.title_field(), 42_i64);
		}))
		.expect_err("a descriptor type mismatch must panic before storage");
		let panic_message = panic
			.downcast_ref::<String>()
			.map(String::as_str)
			.or_else(|| panic.downcast_ref::<&str>().copied());

		// Assert
		assert_eq!(
			panic_message,
			Some(
				"model form field \"title\" rejected value: invalid value for model form field 'title': expected a string"
			)
		);
		assert!(!runtime.get_field_state(form.title_field()).is_dirty);
		assert_eq!(form.value("title"), None);
	});
}
