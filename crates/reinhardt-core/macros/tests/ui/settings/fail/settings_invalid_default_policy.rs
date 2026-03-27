use reinhardt_macros::settings;

#[settings(fragment = true, section = "test", default_policy = "exclude")]
struct BadFragment {
	pub port: u16,
}

fn main() {}
