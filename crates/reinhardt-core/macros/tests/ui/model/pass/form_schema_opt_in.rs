use reinhardt_macros::model;

include!("../support.rs");

pub mod model_form {
	pub trait ModelFormPolicy: Send + Sync + 'static {
		fn allows(field: &str) -> bool;
	}

	pub trait ModelFormSchema {
		type Model;

		fn fields() -> &'static [ModelFormFieldDescriptor];
	}

	pub trait ModelFormPayload<P: ModelFormPolicy>: Sized {
		fn supplied_fields(&self) -> Vec<&'static str>;
		fn forbidden_fields(&self) -> &[&'static str];
		fn get_json(&self, field: &str) -> Option<serde_json::Value>;
		fn set_json(
			&mut self,
			field: &str,
			value: serde_json::Value,
		) -> Result<(), ModelFormPayloadError>;
	}

	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub enum ModelFormFieldKind {
		Text {
			max_length: Option<usize>,
			multiline: bool,
		},
		Email {
			max_length: Option<usize>,
		},
		Url {
			max_length: Option<usize>,
		},
		Integer {
			min: Option<i64>,
			max: Option<i64>,
		},
		Float,
		Decimal,
		Boolean,
		Date,
		Time,
		DateTime,
		Uuid,
		Json,
	}

	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub struct ModelFormFieldDescriptor {
		pub name: &'static str,
		pub kind: ModelFormFieldKind,
		pub required: bool,
		pub has_default: bool,
		pub editable: bool,
		pub generated_relation_id: bool,
	}

	#[derive(Debug, Clone, PartialEq, Eq)]
	pub enum ModelFormPayloadError {
		UnknownField { field: String },
		ForbiddenField { field: String },
		InvalidValue { field: String, message: String },
	}
}

use model_form::{ModelFormPolicy, ModelFormSchema};

#[model(app_label = "polls", form = true)]
struct Question {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 200)]
	title: String,
	#[field(max_length = 64, editable = false)]
	audit_token: String,
}

fn assert_generated<P: ModelFormPolicy>() {
	let mut data = QuestionModelFormData::<P>::empty();
	data.set_title("Question?".to_owned());
	let _: Option<&String> = data.title();
	let fields = QuestionFormSchema::fields();
	assert_eq!(
		fields.iter().map(|field| field.name).collect::<Vec<_>>(),
		["title"]
	);
}

fn main() {
	assert_generated::<AllFields>();
}

struct AllFields;

impl ModelFormPolicy for AllFields {
	fn allows(_field: &str) -> bool {
		true
	}
}
