use reinhardt_macros::settings;

#[settings(fragment = true, section = "test")]
struct BadFragment {
	#[setting(required, default = "42")]
	pub port: u16,
}

fn main() {}
