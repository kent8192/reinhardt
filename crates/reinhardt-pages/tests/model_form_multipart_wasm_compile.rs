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

mod generated_contract {
	use reinhardt_macros::model;
	use reinhardt_pages::form;
	use reinhardt_pages::server_fn::{ServerFnError, server_fn};

	#[model(
		app_label = "clusters",
		table_name = "clusters",
		form(name = PageClusterCreateForm, fields(name)),
		info = false
	)]
	pub(crate) struct PageCluster {
		#[field(primary_key = true)]
		pub id: i64,
		#[field(max_length = 100)]
		pub name: String,
	}

	#[derive(Debug, serde::Deserialize, serde::Serialize)]
	pub(crate) struct PageClusterResponse {
		pub token: String,
	}

	#[server_fn(model_form = true)]
	pub(crate) async fn save_page_cluster(
		payload: PageClusterCreateFormData,
	) -> Result<PageClusterResponse, ServerFnError> {
		let _ = payload;
		Ok(PageClusterResponse {
			token: "saved".to_owned(),
		})
	}

	#[test]
	fn generated_named_model_form_dispatch_compiles_for_wasm() {
		let form = form! {
			name: PageClusterForm,
			model_form: PageClusterCreateForm,
			server_fn: save_page_cluster,
		};
		let _payload: PageClusterCreateFormData = form.data().expect("empty payload is valid");
		let _typed_submit = async {
			let response = form
				.submit_response()
				.await
				.expect("the generated response type is concrete");
			let _: PageClusterResponse = response;
		};
	}
}
