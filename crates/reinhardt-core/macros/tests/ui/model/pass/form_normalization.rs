use reinhardt_macros::model;

include!("../support.rs");

use model_form::ModelFormSchema;

#[model(app_label = "accounts", form = true, info = false)]
#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct Profile {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 64)]
	#[form(trim)]
	name: String,
}

fn main() {
	assert!(ProfileFormSchema::name().trim);
}
