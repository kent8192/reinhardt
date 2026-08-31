use reinhardt_core::model_form::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
	ModelFormPolicy, ModelFormSchema,
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
	fn title() -> &'static ModelFormFieldDescriptor {
		&QUESTION_FIELDS[0]
	}
}

struct QuestionModelFormData<P: ModelFormPolicy>(PhantomData<P>);

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

async fn save_question<P: ModelFormPolicy>(
	_payload: QuestionModelFormData<P>,
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
