use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(fields))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	fields: String,
}

fn main() {}
