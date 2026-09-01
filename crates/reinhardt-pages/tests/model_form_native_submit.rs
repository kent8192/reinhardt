#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

include!("ui/form/model_json_support.rs");

use reinhardt_pages::{FieldError, form, server_fn::ServerFnErrorKind, use_form};
use rstest::rstest;

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
		let error = reinhardt_pages::ServerFnError::validation([
			("title", "Title is already used"),
			("owner_id", "Owner is required"),
		]);

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
			Some("Validation failed\nowner_id: Owner is required".to_owned())
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
		}));

		// Assert
		assert!(panic.is_err());
		assert!(!runtime.get_field_state(form.title_field()).is_dirty);
		assert_eq!(form.value("title"), None);
	});
}
