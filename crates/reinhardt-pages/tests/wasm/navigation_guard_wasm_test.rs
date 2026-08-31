//! Browser coverage for initial and asynchronous navigation-guard flows.

#![cfg(wasm)]

use gloo_timers::future::TimeoutFuture;
use reinhardt_pages::app::ClientLauncher;
use reinhardt_pages::component::{IntoPage, Page, PageElement};
use reinhardt_pages::reactive::hooks::RouterHandle;
use reinhardt_pages::{
	NavigationContext, NavigationDecision, NavigationGuardError, QueryFamily, QueryOptions,
	SsrState, component, navigation_guard,
};
use reinhardt_urls::routers::ClientRouter;
use std::cell::{Cell, RefCell};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

thread_local! {
	static GUARD_MODE: Cell<u8> = const { Cell::new(0) };
	static PROTECTED_MOUNTS: Cell<u32> = const { Cell::new(0) };
	static SESSION_FETCHES: Cell<u32> = const { Cell::new(0) };
	static GUARD_DESTINATIONS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[navigation_guard]
async fn browser_navigation_guard(
	context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	GUARD_DESTINATIONS.with(|destinations| {
		destinations
			.borrow_mut()
			.push(context.destination().to_owned());
	});
	if GUARD_MODE.with(Cell::get) == 3 {
		let descriptor = QueryFamily::<(), String, NavigationGuardError>::new(
			"browser-navigation-guard-session",
		)
		.query((), || async {
			SESSION_FETCHES.with(|fetches| fetches.set(fetches.get() + 1));
			Ok("session".to_owned())
		});
		context.query(descriptor, QueryOptions::new()).await?;
		return Ok(NavigationDecision::Allow);
	}
	match GUARD_MODE.with(Cell::get) {
		0 => Ok(NavigationDecision::Redirect {
			location: "/login/".to_owned(),
			replace: true,
		}),
		1 => Ok(NavigationDecision::Allow),
		2 => {
			TimeoutFuture::new(30).await;
			Ok(NavigationDecision::Allow)
		}
		_ => Ok(NavigationDecision::Forbidden),
	}
}

#[component("/", name = "browser-navigation-guard-home")]
fn browser_home() -> Page {
	PageElement::new("div")
		.attr("id", "guard-home")
		.child("HOME")
		.into_page()
}

#[component("/login/", name = "browser-navigation-guard-login")]
fn browser_login() -> Page {
	PageElement::new("div")
		.attr("id", "guard-login")
		.child("LOGIN")
		.into_page()
}

#[component(
	"/protected/",
	name = "browser-navigation-guard-protected",
	navigation_guard = browser_navigation_guard,
)]
fn browser_protected() -> Page {
	PROTECTED_MOUNTS.with(|mounts| mounts.set(mounts.get() + 1));
	PageElement::new("div")
		.attr("id", "guard-protected")
		.child("PROTECTED")
		.into_page()
}

fn install_app_root_at(path: &str) -> web_sys::Element {
	let window = web_sys::window().expect("browser window");
	let document = window.document().expect("browser document");
	window
		.history()
		.expect("browser history")
		.replace_state_with_url(&JsValue::NULL, "", Some(path))
		.expect("reset browser history");
	if let Some(root) = document.get_element_by_id("app") {
		root.remove();
	}
	if let Some(state) = document.get_element_by_id("ssr-state") {
		state.remove();
	}
	let root = document.create_element("div").expect("create root");
	root.set_id("app");
	document
		.body()
		.expect("document body")
		.append_child(&root)
		.expect("append root");
	root
}

fn install_ssr_state(state: SsrState) {
	let document = web_sys::window()
		.expect("browser window")
		.document()
		.expect("browser document");
	let script = document.create_element("script").expect("create SSR state");
	script.set_id("ssr-state");
	script.set_text_content(Some(&state.to_json().expect("serialize SSR state")));
	document
		.body()
		.expect("document body")
		.append_child(&script)
		.expect("append SSR state");
}

fn build_router() -> ClientRouter {
	ClientRouter::new()
		.component(browser_home)
		.component(browser_login)
		.component(browser_protected)
}

async fn yield_to_tasks() {
	TimeoutFuture::new(0).await;
}

fn current_location() -> String {
	let location = web_sys::window().expect("browser window").location();
	format!(
		"{}{}",
		location.pathname().expect("pathname"),
		location.search().expect("query string")
	)
}

#[wasm_bindgen_test]
async fn denied_initial_route_does_not_mount_protected_content() {
	let root = install_app_root_at("/protected/");
	GUARD_MODE.with(|mode| mode.set(0));
	PROTECTED_MOUNTS.with(|mounts| mounts.set(0));
	GUARD_DESTINATIONS.with(|destinations| destinations.borrow_mut().clear());

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("initial guarded launch starts");
	yield_to_tasks().await;
	yield_to_tasks().await;

	assert_eq!(current_location(), "/login/");
	assert_eq!(root.inner_html(), "<div id=\"guard-login\">LOGIN</div>");
	assert_eq!(PROTECTED_MOUNTS.with(Cell::get), 0);
}

#[wasm_bindgen_test]
async fn delayed_guard_keeps_the_committed_dom_until_allow() {
	let root = install_app_root_at("/");
	GUARD_MODE.with(|mode| mode.set(2));
	PROTECTED_MOUNTS.with(|mounts| mounts.set(0));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("home launch");
	RouterHandle
		.push("/protected/")
		.expect("guarded push starts");
	assert_eq!(current_location(), "/");
	assert!(root.inner_html().contains("HOME"));
	assert_eq!(PROTECTED_MOUNTS.with(Cell::get), 0);

	TimeoutFuture::new(45).await;
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(current_location(), "/protected/");
	assert!(root.inner_html().contains("PROTECTED"));
	assert_eq!(PROTECTED_MOUNTS.with(Cell::get), 1);
}

#[wasm_bindgen_test]
async fn guard_receives_the_complete_destination_query() {
	let root = install_app_root_at("/protected/?tab=billing&next=%2Fclusters");
	GUARD_MODE.with(|mode| mode.set(0));
	GUARD_DESTINATIONS.with(|destinations| destinations.borrow_mut().clear());

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("query destination launch starts");
	yield_to_tasks().await;
	yield_to_tasks().await;

	assert_eq!(current_location(), "/login/");
	assert_eq!(root.inner_html(), "<div id=\"guard-login\">LOGIN</div>");
	assert_eq!(
		GUARD_DESTINATIONS.with(|destinations| destinations.borrow().clone()),
		vec!["/protected/?tab=billing&next=%2Fclusters".to_owned()]
	);
}

#[wasm_bindgen_test]
async fn allowed_initial_hydration_reuses_the_guard_query() {
	let root = install_app_root_at("/protected/");
	GUARD_MODE.with(|mode| mode.set(3));
	PROTECTED_MOUNTS.with(|mounts| mounts.set(0));
	SESSION_FETCHES.with(|fetches| fetches.set(0));
	let mut state = SsrState::new();
	state.add_resource_state(
		"query:browser-navigation-guard-session:sha256:74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b",
		serde_json::json!({
			"state": { "Success": "session" },
			"refetch_error": null,
			"is_fetching": false,
			"is_stale": false
		}),
	);
	install_ssr_state(state);

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("hydrated guarded launch starts");
	yield_to_tasks().await;
	yield_to_tasks().await;

	assert_eq!(
		root.inner_html(),
		"<div id=\"guard-protected\">PROTECTED</div>"
	);
	assert_eq!(SESSION_FETCHES.with(Cell::get), 0);
	assert_eq!(PROTECTED_MOUNTS.with(Cell::get), 1);
}
