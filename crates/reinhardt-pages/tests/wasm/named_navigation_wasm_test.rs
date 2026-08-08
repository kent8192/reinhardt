#![cfg(wasm)]

use gloo_timers::future::TimeoutFuture;
use reinhardt_core::reactive::ReactiveScope;
use reinhardt_pages::app::{
	__clear_spa_router_for_test, __current_path_for_test, __install_client_router_for_test,
};
use reinhardt_pages::component::Page;
use reinhardt_pages::{NavigationType, navigate_named, navigate_or_reload, route_params};
use reinhardt_urls::routers::ClientRouter;
use serial_test::serial;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct BrowserStateGuard {
	href: String,
}

impl BrowserStateGuard {
	fn capture() -> Self {
		let href = web_sys::window()
			.expect("window")
			.location()
			.href()
			.expect("current href");
		Self { href }
	}
}

impl Drop for BrowserStateGuard {
	fn drop(&mut self) {
		__clear_spa_router_for_test();
		let window = web_sys::window().expect("window");
		window
			.history()
			.expect("history")
			.replace_state_with_url(&JsValue::NULL, "", Some(&self.href))
			.expect("restore browser location");
	}
}

fn replace_path(path: &str) {
	web_sys::window()
		.expect("window")
		.history()
		.expect("history")
		.replace_state_with_url(&JsValue::NULL, "", Some(path))
		.expect("replace test path");
}

fn test_router() -> ClientRouter {
	ClientRouter::new()
		.route("home", "/", || Page::text("Home"))
		.route("project", "/projects/{project_id}/", || {
			Page::text("Project")
		})
}

#[wasm_bindgen_test]
#[serial(router)]
fn named_push_and_replace_update_the_active_spa_path() {
	ReactiveScope::run(|| {
		let _state = BrowserStateGuard::capture();
		replace_path("/");
		__install_client_router_for_test(test_router());

		navigate_named("project", [("project_id", 41_i64)], NavigationType::Push)
			.expect("named Push");
		assert_eq!(__current_path_for_test().as_deref(), Some("/projects/41/"));

		navigate_named("home", route_params! {}, NavigationType::Replace).expect("named Replace");
		assert_eq!(__current_path_for_test().as_deref(), Some("/"));
	});
}

#[wasm_bindgen_test]
#[serial(router)]
fn same_origin_absolute_url_uses_spa_navigation() {
	ReactiveScope::run(|| {
		let _state = BrowserStateGuard::capture();
		replace_path("/");
		__install_client_router_for_test(test_router());
		let origin = web_sys::window()
			.expect("window")
			.location()
			.origin()
			.expect("current origin");

		navigate_or_reload(format!("{origin}/projects/52/"), NavigationType::Push)
			.expect("same-origin SPA navigation");

		assert_eq!(__current_path_for_test().as_deref(), Some("/projects/52/"));
	});
}

#[wasm_bindgen_test]
#[serial(router)]
async fn missing_router_falls_back_to_the_exact_fragment_path() {
	let _state = BrowserStateGuard::capture();
	ReactiveScope::run(|| {
		let _component = Page::text("Fragment fallback");
		__clear_spa_router_for_test();

		navigate_or_reload("#reinhardt-named-navigation-fallback", NavigationType::Push)
			.expect("fragment hard navigation");
	});
	TimeoutFuture::new(0).await;

	assert_eq!(
		web_sys::window()
			.expect("window")
			.location()
			.hash()
			.expect("location hash"),
		"#reinhardt-named-navigation-fallback"
	);
}
