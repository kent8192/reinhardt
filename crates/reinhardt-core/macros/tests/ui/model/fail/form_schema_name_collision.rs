use reinhardt_macros::model;

include!("../support.rs");

#[model(app_label = "forms", form = true)]
struct GeneratedNameCollision {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 64)]
	foo: String,
	#[field(max_length = 64)]
	set_foo: String,
}

fn main() {}
