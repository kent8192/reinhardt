use std::marker::PhantomData;

use reinhardt_core::model_form::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
	ModelFormPolicy, ModelFormSchema, NativeModelFormPayload,
};
use reinhardt_pages::server_fn::{ServerFnError, server_fn};

struct Question;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct QuestionResponse {
	token: String,
}

#[derive(Debug)]
struct QuestionPolicy;

impl ModelFormPolicy for QuestionPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "title" | "owner_id")
	}
}

struct QuestionFormSchema;

const QUESTION_FIELDS: [ModelFormFieldDescriptor; 2] = [
	ModelFormFieldDescriptor {
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
	},
	ModelFormFieldDescriptor {
		name: "owner_id",
		kind: ModelFormFieldKind::Integer {
			min: None,
			max: None,
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: true,
	},
];

impl ModelFormSchema for QuestionFormSchema {
	type Model = Question;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&QUESTION_FIELDS
	}
}

impl QuestionFormSchema {
	const fn title() -> &'static ModelFormFieldDescriptor {
		&QUESTION_FIELDS[0]
	}

	const fn owner_id() -> &'static ModelFormFieldDescriptor {
		&QUESTION_FIELDS[1]
	}
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(bound = "")]
struct QuestionModelFormData<P: ModelFormPolicy> {
	title: Option<String>,
	#[serde(skip)]
	_policy: PhantomData<P>,
}

impl<P: ModelFormPolicy> Default for QuestionModelFormData<P> {
	fn default() -> Self {
		Self {
			title: None,
			_policy: PhantomData,
		}
	}
}

impl<P: ModelFormPolicy> ModelFormPayload<P> for QuestionModelFormData<P> {
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
		_value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		match field {
			"title" => Err(ModelFormPayloadError::InvalidValue {
				field: field.to_owned(),
				message: "payload mapping rejected title".to_owned(),
			}),
			_ => Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			}),
		}
	}
}

impl<P: ModelFormPolicy> NativeModelFormPayload for QuestionModelFormData<P> {
	fn from_native_form_value(_value: serde_json::Value) -> Result<Self, serde_json::Error> {
		Ok(Self::default())
	}
}

#[server_fn(model_form = true)]
async fn save_question(
	payload: QuestionModelFormData<QuestionPolicy>,
) -> Result<QuestionResponse, ServerFnError> {
	let _ = payload;
	let response = QuestionResponse {
		token: "one-time-token".to_owned(),
	};
	let _ = &response.token;
	Ok(response)
}
