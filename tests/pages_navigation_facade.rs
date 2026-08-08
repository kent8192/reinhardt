#![cfg(feature = "pages")]

use reinhardt::pages::component::Page;
use reinhardt::pages::{
	NavigateError, NavigationType, navigate_named, navigate_or_reload, route_params,
};

#[test]
fn pages_facade_exports_named_navigation() {
	let _component = Page::text("Facade");
	let result = navigate_named("home", route_params! {}, NavigationType::Push);
	assert!(matches!(result, Err(NavigateError::RouterNotInstalled)));
}

#[test]
fn pages_facade_exports_hard_navigation_fallback() {
	let _component = Page::text("Fallback facade");
	let result = navigate_or_reload("/fallback/", NavigationType::Push);
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

	#[test]
	fn prelude_exports_hard_navigation_fallback() {
		let _component = Page::text("Fallback prelude");
		let result = navigate_or_reload("/fallback/", NavigationType::Push);
		assert!(matches!(result, Err(NavigateError::RouterNotInstalled)));
	}
}
