use reinhardt_pages::{NavigationContext, NavigationDecision, NavigationGuardError, navigation_guard};

#[navigation_guard]
async fn require_authenticated<T>(
	_context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	let _ = std::marker::PhantomData::<T>;
	Ok(NavigationDecision::Allow)
}

fn main() {}
