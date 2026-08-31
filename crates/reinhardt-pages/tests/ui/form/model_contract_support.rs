use reinhardt_core::model_form::{
	ModelFormContract, ModelFormContractField, ModelFormContractSchema, ModelFormFieldDescriptor,
	ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError, ModelFormPolicy,
	NativeModelFormPayload,
};
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

pub(crate) struct QuestionCreateForm;

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct QuestionCreateFormData {
	title: Option<String>,
}

impl QuestionCreateFormData {
	pub(crate) fn title(&self) -> Option<&String> {
		self.title.as_ref()
	}
}

#[doc(hidden)]
pub(crate) struct QuestionCreateFormPolicy;

impl ModelFormPolicy for QuestionCreateFormPolicy {
	fn allows(field: &str) -> bool {
		field == "title"
	}
}

impl ModelFormPayload<QuestionCreateFormPolicy> for QuestionCreateFormData {
	fn supplied_fields(&self) -> Vec<&'static str> {
		self.title.as_ref().map_or_else(Vec::new, |_| vec!["title"])
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		match field {
			"title" => self.title.clone().map(serde_json::Value::String),
			_ => None,
		}
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		match field {
			"title" => {
				self.title = Some(serde_json::from_value(value).map_err(|error| {
					ModelFormPayloadError::InvalidValue {
						field: field.to_owned(),
						message: error.to_string(),
					}
				})?);
				Ok(())
			}
			_ => Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			}),
		}
	}
}

impl NativeModelFormPayload for QuestionCreateFormData {
	fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
		serde_json::from_value(value)
	}
}

pub(crate) struct QuestionCreateFormSchema;

const QUESTION_CREATE_FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
	name: "title",
	kind: ModelFormFieldKind::Text {
		min_length: None,
		max_length: Some(200),
		multiline: false,
	},
	required: true,
	has_default: false,
	nullable: false,
	editable: true,
	generated_relation_id: false,
}];

impl ModelFormContractSchema for QuestionCreateFormSchema {
	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&QUESTION_CREATE_FIELDS
	}
}

// Both descriptor accessors are compile-time guards used by the contract fixtures.
#[allow(dead_code)]
impl QuestionCreateFormSchema {
	pub(crate) const fn title() -> &'static ModelFormFieldDescriptor {
		&QUESTION_CREATE_FIELDS[0]
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum QuestionCreateFormField {
	Title,
}

impl ModelFormContractField for QuestionCreateFormField {
	fn name(self) -> &'static str {
		match self {
			Self::Title => "title",
		}
	}
}

// Both descriptor accessors are compile-time guards used by the contract fixtures.
#[allow(dead_code)]
impl QuestionCreateForm {
	pub(crate) const fn title() -> &'static ModelFormFieldDescriptor {
		QuestionCreateFormSchema::title()
	}
}

impl ModelFormContract for QuestionCreateForm {
	type Data = QuestionCreateFormData;
	type Schema = QuestionCreateFormSchema;
	type Field = QuestionCreateFormField;
	type Policy = QuestionCreateFormPolicy;

	fn fields() -> &'static [Self::Field] {
		&[QuestionCreateFormField::Title]
	}
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct QuestionResponse {
	pub(crate) token: String,
}

#[server_fn(model_form = true)]
pub(crate) async fn save_question(
	payload: QuestionCreateFormData,
) -> Result<QuestionResponse, ServerFnError> {
	let _ = payload.title();
	Ok(QuestionResponse {
		token: "saved".to_owned(),
	})
}
