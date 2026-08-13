#![cfg(wasm)]

mod multipart {
	include!("ui/form/model_multipart_support.rs");

	use reinhardt_pages::form;

	#[test]
	fn model_form_multipart_dispatch_compiles() {
		let _payload = UploadModelFormData::<UploadPolicy>::default();
		let _form = form! {
			name: UploadForm,
			model: Upload,
			policy: UploadPolicy,
			fields: [title, document, avatar],
			server_fn: upload,
		};
	}
}

mod json {
	include!("ui/form/model_json_support.rs");

	use reinhardt_pages::form;

	#[test]
	fn model_form_json_explicit_fields_compile() {
		let _form = form! {
			name: QuestionFieldsForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
	}

	#[test]
	fn model_form_json_exclude_compiles() {
		let _form = form! {
			name: QuestionExcludeForm,
			model: Question,
			policy: QuestionPolicy,
			exclude: [owner_id],
			server_fn: save_question,
		};
	}
}
