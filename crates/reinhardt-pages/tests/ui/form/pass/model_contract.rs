#[path = "../model_contract_support.rs"]
mod support;

use reinhardt_pages::{FormRuntimeSource, form};
use support::{
	QuestionCreateForm, QuestionCreateFormData, QuestionCreateFormField, QuestionResponse,
	save_question,
};

fn assert_contract_field<Form>(_: &Form)
where
	Form: FormRuntimeSource<Field = QuestionCreateFormField>,
{
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = form! {
			name: QuestionForm,
			model_form: QuestionCreateForm,
			server_fn: save_question,
			overrides: {
				title: { label: "Question" },
			},
		};
		let _: QuestionCreateFormData = form.data().expect("contract payload should build");
		assert_contract_field(&form);

		#[cfg(all(target_family = "wasm", target_os = "unknown"))]
		{
			async fn assert_response(form: QuestionForm) {
				let _: QuestionResponse = form
					.submit_response()
					.await
					.expect("contract server function should return its response type");
			}
			let _ = assert_response(form);
		}
	});
}
