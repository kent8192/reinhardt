use reinhardt_macros::model;

include!("../support.rs");

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
