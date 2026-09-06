use reinhardt_pages::{NavigationContext, navigation_guard};

#[navigation_guard]
async fn require_authenticated(_context: NavigationContext) -> Result<bool, String> {
	Ok(true)
}

fn main() {}
