use reinhardt_macros::model;

include!("../support.rs");

#[model(app_label = "accounts", form = true)]
struct Profile {
	#[field(primary_key = true)]
	id: i64,
	#[form(lowercase)]
	name: String,
}

fn main() {}
