use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(foo_bar, foo__bar))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	foo_bar: String,
	foo__bar: String,
}

fn main() {}
