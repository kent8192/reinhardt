use reinhardt_macros::settings;

#[settings(fragment = true, section = "test")]
struct BadFragment {
	#[setting(require)]
	pub port: u16,
}

fn main() {}
