use reinhardt_macros::model;

include!("../support.rs");

struct Slug(String);

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(slug))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	slug: Slug,
}

fn main() {}
