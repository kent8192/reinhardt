#![cfg(feature = "pages")]

use reinhardt::pages::component::Page;
use reinhardt::pages::{NavigateError, NavigationType, navigate_named, route_params};

#[test]
fn pages_facade_exports_named_navigation() {
	let _component = Page::text("Facade");
	let result = navigate_named("home", route_params! {}, NavigationType::Push);
	assert!(matches!(result, Err(NavigateError::RouterNotInstalled)));
}

mod prelude_exports {
	use reinhardt::pages::component::Page;
	use reinhardt::pages::prelude::*;

	#[test]
	fn prelude_exports_named_navigation() {
		let _component = Page::text("Prelude");
		let result = navigate_named("home", route_params! {}, NavigationType::Push);
		assert!(matches!(result, Err(NavigateError::RouterNotInstalled)));
	}
}
