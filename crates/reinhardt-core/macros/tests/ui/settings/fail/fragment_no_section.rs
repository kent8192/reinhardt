use reinhardt_macros::settings;

#[settings(fragment = true)]
pub struct BadSettings {
	pub value: String,
}

fn main() {}
