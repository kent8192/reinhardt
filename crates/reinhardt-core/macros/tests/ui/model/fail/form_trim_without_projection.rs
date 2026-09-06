use reinhardt_macros::model;

include!("../support.rs");

#[model(app_label = "accounts")]
struct Profile {
	#[field(primary_key = true)]
	id: i64,
	#[form(trim)]
	name: String,
}

fn main() {}
