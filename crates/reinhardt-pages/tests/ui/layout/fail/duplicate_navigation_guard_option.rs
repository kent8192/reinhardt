use reinhardt_pages::{
	NavigationContext, NavigationDecision, NavigationGuardError, Outlet, Page, layout,
	navigation_guard, page,
};

#[navigation_guard]
async fn first(_context: NavigationContext) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

#[navigation_guard]
async fn second(_context: NavigationContext) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

#[layout(
	"/dashboard/",
	name = "dashboard",
	navigation_guard = first,
	navigation_guard = second,
)]
fn dashboard(outlet: Outlet) -> Page {
	page!(|outlet: Outlet| { main { { outlet } } })(outlet)
}

fn main() {}
