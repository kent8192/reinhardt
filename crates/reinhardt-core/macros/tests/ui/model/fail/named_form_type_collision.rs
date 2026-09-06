use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(name))
)]
struct DocumentCreateFormData {
	#[field(primary_key = true)]
	id: i64,
	name: String,
}

fn main() {}
