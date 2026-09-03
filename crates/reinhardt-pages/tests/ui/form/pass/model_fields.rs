//! Model-backed form with an explicit field selection.

use std::marker::PhantomData;

use reinhardt_core::model_form::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
	ModelFormPolicy, ModelFormSchema, NativeModelFormPayload,
};
use reinhardt_pages::form;

struct Question;

struct QuestionFields;

impl ModelFormPolicy for QuestionFields {
	fn allows(field: &str) -> bool {
		matches!(field, "title" | "owner_id")
	}
}

struct QuestionSubmissionPolicy;

impl ModelFormPolicy for QuestionSubmissionPolicy {
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

struct QuestionModelFormData<P: ModelFormPolicy> {
	title: Option<String>,
	owner_id: Option<i64>,
	_policy: PhantomData<P>,
}

impl<P: ModelFormPolicy> QuestionModelFormData<P> {
	fn empty() -> Self {
		Self {
			title: None,
			owner_id: None,
			_policy: PhantomData,
		}
	}
}

impl<P: ModelFormPolicy> Default for QuestionModelFormData<P> {
	fn default() -> Self {
		Self::empty()
	}
}

impl<P: ModelFormPolicy> ModelFormPayload<P> for QuestionModelFormData<P> {
	fn supplied_fields(&self) -> Vec<&'static str> {
		let mut fields = Vec::new();
		if self.title.is_some() {
			fields.push("title");
		}
		if self.owner_id.is_some() {
			fields.push("owner_id");
		}
		fields
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		match field {
			"title" => self.title.clone().map(serde_json::Value::String),
			"owner_id" => self.owner_id.map(serde_json::Value::from),
			_ => None,
		}
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		match field {
			"title" => {
				self.title = serde_json::from_value(value).map_err(|error| {
					ModelFormPayloadError::InvalidValue {
						field: field.to_owned(),
						message: error.to_string(),
					}
				})?;
			}
			"owner_id" => {
				self.owner_id = serde_json::from_value(value).map_err(|error| {
					ModelFormPayloadError::InvalidValue {
						field: field.to_owned(),
						message: error.to_string(),
					}
				})?;
			}
			_ => {
				return Err(ModelFormPayloadError::UnknownField {
					field: field.to_owned(),
				});
			}
		}
		Ok(())
	}
}

impl<P: ModelFormPolicy> NativeModelFormPayload for QuestionModelFormData<P> {
	fn from_native_form_value(_value: serde_json::Value) -> Result<Self, serde_json::Error> {
		Ok(Self::empty())
	}
}

async fn save_question(
	_payload: QuestionModelFormData<QuestionSubmissionPolicy>,
) -> Result<(), reinhardt_pages::ServerFnError> {
	Ok(())
}

mod save_question {
	// Generated server-function markers use the lower-case `marker` name.
	#[allow(non_camel_case_types)]
	pub struct marker;

	impl reinhardt_pages::server_fn::ServerFnMetadata for marker {
		const PATH: &'static str = "/api/server_fn/save_question";
		const NAME: &'static str = "save_question";
		const IS_JSON_CODEC: bool = true;
	}

	impl<Selection, S, P> reinhardt_pages::form::ModelFormServerFn<Selection, S, P> for marker
	where
		S: reinhardt_pages::form::ModelFormSchema,
		P: reinhardt_pages::form::ModelFormPolicy,
	{
		type Response = ();
		type Error = reinhardt_pages::ServerFnError;

		fn submit(
			_state: &reinhardt_pages::form::ModelFormState<S, P>,
		) -> impl core::future::Future<Output = Result<Self::Response, Self::Error>> {
			async { ::core::unreachable!() }
		}
	}
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = form! {
			name: QuestionForm,
			model: Question,
			policy: QuestionFields,
			fields: [title],
			server_fn: save_question,
			overrides: {
				title: {
					widget: TextArea,
					label: "Question",
					help_text: "Enter the question",
				},
			},
		};
		let _: QuestionModelFormData<QuestionFields> = form
			.data()
			.expect("selected fields produce the endpoint payload type");
	});
}
