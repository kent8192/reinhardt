#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]

include!("ui/form/model_json_support.rs");

use reinhardt_pages::{form, server_fn::ServerFnErrorKind};

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
