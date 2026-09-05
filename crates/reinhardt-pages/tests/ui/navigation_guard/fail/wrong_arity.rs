use reinhardt_pages::{NavigationContext, NavigationDecision, NavigationGuardError, navigation_guard};

#[navigation_guard]
async fn require_authenticated(
	_left: NavigationContext,
	_right: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

fn main() {}
