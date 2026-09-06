use reinhardt_core::model_form::{
	ModelFormCleanedPayload, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
	ModelFormPayloadError, ModelFormPolicy, ModelFormSchema, ModelFormValidatingPayload,
};
use reinhardt_pages::form;
use std::marker::PhantomData;

struct Question;
struct QuestionFields;
struct QuestionFormSchema;

impl ModelFormPolicy for QuestionFields {
	fn allows(field: &str) -> bool {
		field == "title"
	}
}

const QUESTION_FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
	name: "title",
	kind: ModelFormFieldKind::Text {
		min_length: None,
		max_length: None,
		multiline: false,
	},
	required: true,
	has_default: false,
	nullable: false,
	editable: true,
	generated_relation_id: false,
	trim: false,
}];

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
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
struct QuestionModelFormData<P: ModelFormPolicy>(#[serde(skip)] PhantomData<P>);

impl<P: ModelFormPolicy> QuestionModelFormData<P> {
	fn empty() -> Self {
		Self(PhantomData)
	}
}

impl<P: ModelFormPolicy> Default for QuestionModelFormData<P> {
	fn default() -> Self {
		Self::empty()
	}
}

impl<P: ModelFormPolicy> ModelFormPayload<P> for QuestionModelFormData<P> {
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

impl<P: ModelFormPolicy> reinhardt_core::model_form::NativeModelFormPayload
	for QuestionModelFormData<P>
{
	fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
		serde_json::from_value(value)
	}
}

#[reinhardt_pages::server_fn::server_fn(model_form = true)]
async fn save_question(
	_payload: QuestionModelFormData<QuestionFields>,
) -> Result<(), reinhardt_pages::ServerFnError> {
	Ok(())
}

fn main() {
	let _form = form! {
		name: QuestionForm,
		model: Question,
		policy: QuestionFields,
		fields: [missing],
		server_fn: save_question,
	};
}
