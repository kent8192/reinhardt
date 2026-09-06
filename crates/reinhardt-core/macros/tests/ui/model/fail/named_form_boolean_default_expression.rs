use reinhardt_macros::model;

include!("../support.rs");

const DEFAULT_ENABLED: bool = true;

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(enabled))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(default = DEFAULT_ENABLED)]
	enabled: bool,
}

fn main() {}
