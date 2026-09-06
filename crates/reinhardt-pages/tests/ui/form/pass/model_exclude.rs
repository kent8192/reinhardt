//! Model-backed form with an excluded field.

use std::marker::PhantomData;

use reinhardt_core::model_form::{
	ModelFormCleanedPayload, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
	ModelFormPayloadError, ModelFormPolicy, ModelFormSchema, ModelFormValidatingPayload,
	NativeModelFormPayload,
};
use reinhardt_pages::form;

struct Question;
struct QuestionFields;

impl ModelFormPolicy for QuestionFields {
	fn allows(field: &str) -> bool {
		field == "title"
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
		trim: false,
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
		trim: false,
	},
];

impl ModelFormSchema for QuestionFormSchema {
	type Model = Question;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&QUESTION_FIELDS
	}
}

impl QuestionFormSchema {
	const fn owner_id() -> &'static ModelFormFieldDescriptor {
		&QUESTION_FIELDS[1]
	}
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
struct QuestionModelFormData<P: ModelFormPolicy> {
	title: Option<String>,
	#[serde(skip)]
	_policy: PhantomData<P>,
}

impl<P: ModelFormPolicy> QuestionModelFormData<P> {
	fn empty() -> Self {
		Self {
			title: None,
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
		if self.title.is_some() {
			vec!["title"]
		} else {
			Vec::new()
		}
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
				Ok(())
			}
			_ => Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			}),
		}
	}
}

struct CleanedQuestionModelFormData<P: ModelFormPolicy>(QuestionModelFormData<P>);

impl<P: ModelFormPolicy> ModelFormCleanedPayload for CleanedQuestionModelFormData<P> {
	type Raw = QuestionModelFormData<P>;

	fn into_raw(self) -> Self::Raw {
		self.0
	}
}

impl<P: ModelFormPolicy> ModelFormValidatingPayload for QuestionModelFormData<P> {
	type Cleaned = CleanedQuestionModelFormData<P>;

	fn clean_and_validate(
		mut self,
	) -> Result<Self::Cleaned, reinhardt_core::validators::ValidationErrors> {
		reinhardt_forms::model_form::clean_generated_payload::<QuestionFormSchema, P, _>(
			&mut self,
		)?;
		Ok(CleanedQuestionModelFormData(self))
	}
}

impl<P: ModelFormPolicy> NativeModelFormPayload for QuestionModelFormData<P> {
	fn from_native_form_value(_value: serde_json::Value) -> Result<Self, serde_json::Error> {
		Ok(Self::empty())
	}
}

#[reinhardt_pages::server_fn::server_fn(model_form = true)]
async fn save_question(
	_payload: QuestionModelFormData<QuestionFields>,
) -> Result<(), reinhardt_pages::ServerFnError> {
	Ok(())
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _form = form! {
			name: QuestionForm,
			model: Question,
			policy: QuestionFields,
			exclude: [owner_id],
			server_fn: save_question,
		};
	});
}
