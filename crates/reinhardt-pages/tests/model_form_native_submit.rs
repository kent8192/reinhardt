#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

include!("ui/form/model_json_support.rs");

use reinhardt_pages::{FieldError, form, server_fn::ServerFnErrorKind, use_form};

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
