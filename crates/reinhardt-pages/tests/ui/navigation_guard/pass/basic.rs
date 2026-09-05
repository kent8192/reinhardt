use reinhardt_pages::{NavigationContext, NavigationDecision, NavigationGuard, NavigationGuardError, navigation_guard};

#[navigation_guard]
pub async fn require_authenticated(
	_context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	Ok(NavigationDecision::Allow)
}

async fn invoke() {
	let _ = require_authenticated(panic!("fixture context")).await;
}

fn main() {
	let _id = <require_authenticated::marker as NavigationGuard>::ID;
}
