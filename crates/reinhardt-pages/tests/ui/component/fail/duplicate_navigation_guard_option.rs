use reinhardt_pages::{
	NavigationContext, NavigationDecision, NavigationGuardError, Page, component, navigation_guard,
	page,
};

#[navigation_guard]
async fn first(_context: NavigationContext) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

#[navigation_guard]
async fn second(_context: NavigationContext) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

#[component(
	"/account/",
	name = "account",
	navigation_guard = first,
	navigation_guard = second,
)]
fn account() -> Page {
	page!(|| { p { "account" } })()
}

fn main() {}
