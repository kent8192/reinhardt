use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(csrfmiddlewaretoken))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 64)]
	csrfmiddlewaretoken: String,
}

fn main() {}
