use reinhardt_macros::model;

include!("../support.rs");

#[model(app_label = "accounts", form = true)]
#[derive(Clone)]
#[form(validate = validate_profile)]
#[form(validate = validate_profile_again)]
struct Profile {
	#[field(primary_key = true)]
	id: i64,
	name: String,
}

fn main() {}
