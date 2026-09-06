use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(__reinhardt_range_amount))
)]
struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 64)]
	__reinhardt_range_amount: String,
}

fn main() {}
