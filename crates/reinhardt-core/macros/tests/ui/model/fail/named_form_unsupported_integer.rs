use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(count))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	count: i8,
}

fn main() {}
