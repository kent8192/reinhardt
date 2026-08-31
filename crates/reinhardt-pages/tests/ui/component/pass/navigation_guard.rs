use reinhardt_pages::{
	NavigationContext, NavigationDecision, NavigationGuard, NavigationGuardError, Page, component,
	navigation_guard, page,
};

#[navigation_guard]
async fn require_authenticated(
	_context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

#[component(
	"/account/",
	navigation_guard = require_authenticated,
	name = "account",
)]
fn account() -> Page {
	page!(|| { p { "account" } })()
}

fn main() {
	let _id = <require_authenticated::marker as NavigationGuard>::ID;
	let _: Option<reinhardt_pages::NavigationGuardId> =
		<AccountProps as reinhardt_urls::routers::client_router::ComponentInfo>::navigation_guard_id();
}
