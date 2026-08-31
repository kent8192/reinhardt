use reinhardt_pages::{Outlet, Page, layout, page};

mod not_a_navigation_guard {
	pub struct marker;
}

#[layout(
	"/dashboard/",
	name = "dashboard",
	navigation_guard = not_a_navigation_guard,
)]
fn dashboard(outlet: Outlet) -> Page {
	page!(|outlet: Outlet| { main { { outlet } } })(outlet)
}

fn main() {}
