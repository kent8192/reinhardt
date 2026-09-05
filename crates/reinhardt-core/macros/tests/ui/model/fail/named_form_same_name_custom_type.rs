use reinhardt_macros::model;

include!("../support.rs");

struct Uuid(String);

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(external_id))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	external_id: Uuid,
}

fn main() {}
