use reinhardt_pages::{
	NavigationContext, NavigationDecision, NavigationGuardError, Outlet, Page, layout,
	navigation_guard, page,
};

#[navigation_guard]
async fn require_authenticated(
	_context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

#[layout(
	"/dashboard/",
	navigation_guard = require_authenticated,
	name = "dashboard",
)]
fn dashboard(outlet: Outlet) -> Page {
	page!(|outlet: Outlet| { main { { outlet } } })(outlet)
}

fn main() {
	let _: Option<reinhardt_pages::NavigationGuardId> =
		<DashboardProps as reinhardt_urls::routers::client_router::LayoutInfo>::navigation_guard_id();
}
