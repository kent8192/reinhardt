use reinhardt_pages::{Page, component, page};

mod not_a_navigation_guard {
	pub struct marker;
}

#[component(
	"/account/",
	name = "account",
	navigation_guard = not_a_navigation_guard,
)]
fn account() -> Page {
	page!(|| {
		p { "account" }
	})()
}

fn main() {}
