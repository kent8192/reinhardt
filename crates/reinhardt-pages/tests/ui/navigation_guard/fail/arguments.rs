use reinhardt_pages::{NavigationContext, NavigationDecision, NavigationGuardError, navigation_guard};

#[navigation_guard("unexpected")]
async fn require_authenticated(
	_context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

fn main() {}
