use reinhardt_macros::model;

include!("../support.rs");

mod custom {
	pub type Option<T> = std::option::Option<T>;
}

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(maybe))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	maybe: custom::Option<String>,
}

fn main() {}
