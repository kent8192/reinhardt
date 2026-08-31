#[path = "../model_contract_support.rs"]
mod support;

use reinhardt_pages::form;
use support::{QuestionCreateForm, save_question};

fn main() {
	let _form = form! {
		name: QuestionForm,
		model_form: QuestionCreateForm,
		server_fn: save_question,
		overrides: {
			organization_id: { label: "Organization" },
		},
	};
}
