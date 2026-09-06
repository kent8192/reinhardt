use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(enabled, optional_enabled))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(default = true)]
	enabled: std::primitive::bool,
	#[field(default = false)]
	optional_enabled: Option<bool>,
}

fn main() {
	use model_form::ModelFormContractSchema;

	assert!(DocumentCreateFormSchema::contract_default_boolean_is_true("enabled"));
	assert!(!DocumentCreateFormSchema::contract_default_boolean_is_true("optional_enabled"));
}
