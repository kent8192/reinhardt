use reinhardt_macros::model;

include!("../support.rs");

#[model(app_label = "polls", form = true)]
struct Question {
	#[field(primary_key = true)]
	id: i64,
	#[field]
	tags: Vec<String>,
}

fn main() {}
