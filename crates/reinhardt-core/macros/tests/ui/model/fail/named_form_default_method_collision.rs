use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(r#default))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 100)]
	r#default: String,
}

fn main() {}
