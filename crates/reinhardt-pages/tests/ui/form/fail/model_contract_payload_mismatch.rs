#[path = "../model_contract_support.rs"]
mod support;

use reinhardt_core::model_form::{ModelFormPayload, ModelFormPayloadError, NativeModelFormPayload};
use reinhardt_pages::form;
use reinhardt_pages::server_fn::server_fn;
use support::{QuestionCreateForm, QuestionCreateFormPolicy};

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DifferentPayload;

impl ModelFormPayload<QuestionCreateFormPolicy> for DifferentPayload {
	fn supplied_fields(&self) -> Vec<&'static str> {
		Vec::new()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, _field: &str) -> Option<serde_json::Value> {
		None
	}

	fn set_json(
		&mut self,
		field: &str,
		_value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		Err(ModelFormPayloadError::UnknownField {
			field: field.to_owned(),
		})
	}
}

impl NativeModelFormPayload for DifferentPayload {
	fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
		serde_json::from_value(value)
	}
}

#[server_fn(model_form = true)]
async fn save_different(_payload: DifferentPayload) -> Result<(), reinhardt_pages::ServerFnError> {
	Ok(())
}

fn main() {
	let _form = form! {
		name: QuestionForm,
		model_form: QuestionCreateForm,
		server_fn: save_different,
	};
}
