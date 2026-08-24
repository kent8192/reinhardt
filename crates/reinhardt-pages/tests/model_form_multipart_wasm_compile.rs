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

	use reinhardt_pages::{UseFormAsyncSubmitOutcome, form, use_form};

	#[test]
	fn model_form_json_explicit_fields_compile() {
		let form = form! {
			name: QuestionFieldsForm,
			model: Question,
			policy: QuestionPolicy,
			fields: [title],
			server_fn: save_question,
		};
		let runtime = use_form(&form).build();
		let _ = runtime.get_field_state(form.title_field());
		let _typed_submit = async {
			if let Ok(UseFormAsyncSubmitOutcome::Submitted(response)) =
				runtime.submit_server_fn(|| form.submit_response()).await
			{
				let _: QuestionResponse = response;
			}
		};
	}

	#[test]
	fn model_form_json_exclude_compiles() {
		let form = form! {
			name: QuestionExcludeForm,
			model: Question,
			policy: QuestionPolicy,
			exclude: [owner_id],
			server_fn: save_question,
		};
		let _ = form.field("title");
	}
}
