//! Browser coverage for initial and asynchronous navigation-guard flows.

#![cfg(wasm)]

use gloo_timers::future::TimeoutFuture;
use js_sys::{Function, Reflect};
use reinhardt_pages::app::ClientLauncher;
use reinhardt_pages::auth::{
	auth_state, clear_jwt_token, get_jwt_token, invalidate_authentication,
	observe_server_fn_status, observe_server_fn_status_for_request,
	server_fn_authentication_context, set_jwt_token,
};
use reinhardt_pages::component::{Component, IntoPage, Page, PageElement};
use reinhardt_pages::reactive::hooks::RouterHandle;
use reinhardt_pages::router::{Link, PrefetchMode};
use reinhardt_pages::server_fn::ServerFnError;
use reinhardt_pages::{
	Loader, NavigationContext, NavigationDecision, NavigationGuardError, QueryFamily, QueryOptions,
	SsrState, component, loader, navigation_guard,
};
use reinhardt_pages_macros::server_fn;
use reinhardt_urls::routers::ClientRouter;
use rstest::rstest;
use std::cell::{Cell, RefCell};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[server_fn(no_csrf = true)]
async fn anonymous_login_probe(username: String) -> Result<(), ServerFnError> {
	let _ = username;
	Ok(())
}

struct UnauthorizedFetchGuard {
	window: web_sys::Window,
	previous_fetch: JsValue,
}

impl UnauthorizedFetchGuard {
	fn install() -> Self {
		let window = web_sys::window().expect("browser window");
		let previous_fetch = Reflect::get(window.as_ref(), &JsValue::from_str("fetch"))
			.expect("window.fetch must be readable");
		let stub = Function::new_with_args(
			"request",
			"return Promise.resolve(new Response('Invalid credentials', { status: 401 }));",
		);
		Reflect::set(window.as_ref(), &JsValue::from_str("fetch"), stub.as_ref())
			.expect("install unauthorized fetch stub");

		Self {
			window,
			previous_fetch,
		}
	}
}

impl Drop for UnauthorizedFetchGuard {
	fn drop(&mut self) {
		let _ = Reflect::set(
			self.window.as_ref(),
			&JsValue::from_str("fetch"),
			&self.previous_fetch,
		);
	}
}

thread_local! {
	static GUARD_MODE: Cell<u8> = const { Cell::new(0) };
	static GUARD_EVALUATIONS: Cell<u32> = const { Cell::new(0) };
	static PROTECTED_MOUNTS: Cell<u32> = const { Cell::new(0) };
	static PROTECTED_LOADER_CALLS: Cell<u32> = const { Cell::new(0) };
	static SESSION_FETCHES: Cell<u32> = const { Cell::new(0) };
	static GUARD_DESTINATIONS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[navigation_guard]
async fn browser_navigation_guard(
	context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	let mode = GUARD_MODE.with(Cell::get);
	GUARD_EVALUATIONS.with(|evaluations| evaluations.set(evaluations.get() + 1));
	GUARD_DESTINATIONS.with(|destinations| {
		destinations
			.borrow_mut()
			.push(context.destination().to_owned());
	});
	if mode == 3 || mode == 8 {
		let descriptor = QueryFamily::<(), String, NavigationGuardError>::new(
			"browser-navigation-guard-session",
		)
		.query((), move || async move {
			SESSION_FETCHES.with(|fetches| fetches.set(fetches.get() + 1));
			if mode == 8 {
				TimeoutFuture::new(50).await;
			}
			Ok("session".to_owned())
		});
		context.query(descriptor, QueryOptions::new()).await?;
		return Ok(NavigationDecision::Allow);
	}
	match mode {
		0 => Ok(NavigationDecision::Redirect {
			location: "/login/".to_owned(),
			replace: true,
		}),
		1 => Ok(NavigationDecision::Allow),
		2 => {
			TimeoutFuture::new(10).await;
			Ok(NavigationDecision::Allow)
		}
		4 => Ok(NavigationDecision::Forbidden),
		5 => Ok(NavigationDecision::Redirect {
			location: context.destination().to_owned(),
			replace: true,
		}),
		6 => {
			let location = if context.destination() == "/guard-a/" {
				"/guard-b/"
			} else {
				"/guard-a/"
			};
			Ok(NavigationDecision::Redirect {
				location: location.to_owned(),
				replace: false,
			})
		}
		7 => {
			observe_server_fn_status(401);
			Ok(NavigationDecision::Allow)
		}
		_ => Ok(NavigationDecision::Forbidden),
	}
}

#[loader]
async fn browser_protected_loader() -> Result<String, String> {
	PROTECTED_LOADER_CALLS.with(|calls| calls.set(calls.get() + 1));
	Ok("PROTECTED LOADED".to_owned())
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

#[component(
	"/protected-loaded/",
	name = "browser-navigation-guard-protected-loaded",
	loader = browser_protected_loader,
	navigation_guard = browser_navigation_guard,
)]
fn browser_protected_loaded(Loader(data): Loader<String>) -> Page {
	PROTECTED_MOUNTS.with(|mounts| mounts.set(mounts.get() + 1));
	PageElement::new("div")
		.attr("id", "guard-protected-loaded")
		.child(data)
		.into_page()
}

#[component("/link-source/", name = "browser-navigation-guard-link-source")]
fn browser_link_source() -> Page {
	PageElement::new("div")
		.attr("id", "guard-link-source")
		.child(
			Link::new("/protected-loaded/", "Protected")
				.attr("id", "guard-protected-link")
				.prefetch(PrefetchMode::Hover)
				.render(),
		)
		.into_page()
}

#[component("/open/", name = "browser-navigation-guard-open")]
fn browser_open() -> Page {
	PageElement::new("div")
		.attr("id", "guard-open")
		.child("OPEN")
		.into_page()
}

#[component(
	"/guard-a/",
	name = "browser-navigation-guard-a",
	navigation_guard = browser_navigation_guard,
)]
fn browser_guard_a() -> Page {
	PageElement::new("div")
		.attr("id", "guard-a")
		.child("A")
		.into_page()
}

#[component(
	"/guard-b/",
	name = "browser-navigation-guard-b",
	navigation_guard = browser_navigation_guard,
)]
fn browser_guard_b() -> Page {
	PageElement::new("div")
		.attr("id", "guard-b")
		.child("B")
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
		.component(browser_protected_loaded)
		.component(browser_link_source)
		.component(browser_open)
		.component(browser_guard_a)
		.component(browser_guard_b)
}

async fn yield_to_tasks() {
	TimeoutFuture::new(0).await;
}

async fn settle_navigation() {
	for _ in 0..8 {
		TimeoutFuture::new(10).await;
		yield_to_tasks().await;
	}
}

fn current_location() -> String {
	let location = web_sys::window().expect("browser window").location();
	format!(
		"{}{}",
		location.pathname().expect("pathname"),
		location.search().expect("query string")
	)
}

#[rstest]
#[test_attr(wasm_bindgen_test)]
async fn anonymous_server_fn_401_preserves_the_login_route() {
	// Arrange
	let root = install_app_root_at("/login/");
	auth_state().logout();
	clear_jwt_token();
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("login launch");
	settle_navigation().await;
	assert_eq!(root.inner_html(), "<div id=\"guard-login\">LOGIN</div>");
	let _fetch = UnauthorizedFetchGuard::install();

	// Act
	let result = anonymous_login_probe("invalid-user".to_owned()).await;

	// Assert
	assert!(
		result.is_err(),
		"invalid credentials must remain a form error"
	);
	assert_eq!(current_location(), "/login/");
	assert_eq!(root.inner_html(), "<div id=\"guard-login\">LOGIN</div>");
}

#[rstest]
#[test_attr(wasm_bindgen_test)]
fn stale_server_fn_401_preserves_a_rotated_jwt() {
	// Arrange
	auth_state().logout();
	clear_jwt_token();
	set_jwt_token("expired-token");
	let context = server_fn_authentication_context();

	// Act
	set_jwt_token("replacement-token");
	observe_server_fn_status_for_request(401, context);
	let retained_token = get_jwt_token();
	clear_jwt_token();

	// Assert
	assert_eq!(retained_token.as_deref(), Some("replacement-token"));
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

	TimeoutFuture::new(80).await;
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(current_location(), "/protected/");
	assert!(root.inner_html().contains("PROTECTED"));
	assert!(PROTECTED_MOUNTS.with(Cell::get) > 0);
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

#[wasm_bindgen_test]
async fn redirected_push_replace_and_link_skip_protected_loaders() {
	let root = install_app_root_at("/");
	GUARD_MODE.with(|mode| mode.set(0));
	PROTECTED_MOUNTS.with(|mounts| mounts.set(0));
	PROTECTED_LOADER_CALLS.with(|calls| calls.set(0));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("home launch");
	RouterHandle
		.push("/protected-loaded/")
		.expect("guarded push starts");
	settle_navigation().await;
	assert_eq!(current_location(), "/login/");
	assert_eq!(PROTECTED_MOUNTS.with(Cell::get), 0);
	assert_eq!(PROTECTED_LOADER_CALLS.with(Cell::get), 0);

	RouterHandle.replace("/").expect("reset route for replace");
	settle_navigation().await;
	RouterHandle
		.replace("/protected-loaded/")
		.expect("guarded replace starts");
	settle_navigation().await;
	assert_eq!(current_location(), "/login/");
	assert_eq!(PROTECTED_MOUNTS.with(Cell::get), 0);
	assert_eq!(PROTECTED_LOADER_CALLS.with(Cell::get), 0);

	RouterHandle
		.replace("/link-source/")
		.expect("link source navigation starts");
	settle_navigation().await;
	let document = web_sys::window()
		.expect("browser window")
		.document()
		.expect("browser document");
	let link: web_sys::HtmlElement = document
		.get_element_by_id("guard-protected-link")
		.expect("protected link")
		.dyn_into()
		.expect("protected link is an HTML element");
	link.click();
	settle_navigation().await;
	assert_eq!(current_location(), "/login/");
	assert!(root.inner_html().contains("LOGIN"));
	assert_eq!(PROTECTED_MOUNTS.with(Cell::get), 0);
	assert_eq!(PROTECTED_LOADER_CALLS.with(Cell::get), 0);
}

#[wasm_bindgen_test]
async fn denied_prefetch_preserves_route_and_click_rechecks_the_guard() {
	let root = install_app_root_at("/");
	GUARD_MODE.with(|mode| mode.set(1));
	PROTECTED_MOUNTS.with(|mounts| mounts.set(0));
	PROTECTED_LOADER_CALLS.with(|calls| calls.set(0));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("home launch");
	RouterHandle
		.push("/link-source/")
		.expect("link source navigation starts");
	settle_navigation().await;
	let document = web_sys::window()
		.expect("browser window")
		.document()
		.expect("browser document");
	let link: web_sys::HtmlElement = document
		.get_element_by_id("guard-protected-link")
		.expect("protected link")
		.dyn_into()
		.expect("protected link is an HTML element");
	GUARD_MODE.with(|mode| mode.set(0));
	let pointerover_init = web_sys::PointerEventInit::new();
	pointerover_init.set_bubbles(true);
	let pointerover =
		web_sys::PointerEvent::new_with_event_init_dict("pointerover", &pointerover_init)
			.expect("pointerover event");
	link.dispatch_event(&pointerover)
		.expect("dispatch pointerover");
	settle_navigation().await;
	assert_eq!(current_location(), "/link-source/");
	assert!(root.inner_html().contains("guard-link-source"));
	assert_eq!(PROTECTED_LOADER_CALLS.with(Cell::get), 0);

	GUARD_MODE.with(|mode| mode.set(1));
	link.click();
	settle_navigation().await;
	assert_eq!(current_location(), "/protected-loaded/");
	assert!(root.inner_html().contains("PROTECTED LOADED"));
	assert_eq!(PROTECTED_LOADER_CALLS.with(Cell::get), 1);
}

#[wasm_bindgen_test]
async fn denied_back_and_forward_pops_restore_the_committed_route() {
	let root = install_app_root_at("/");
	GUARD_MODE.with(|mode| mode.set(1));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("home launch");
	RouterHandle
		.push("/protected/")
		.expect("protected push starts");
	settle_navigation().await;
	RouterHandle.push("/open/").expect("open push starts");
	settle_navigation().await;
	assert_eq!(current_location(), "/open/");
	assert!(root.inner_html().contains("OPEN"));

	GUARD_MODE.with(|mode| mode.set(4));
	web_sys::window()
		.expect("browser window")
		.history()
		.expect("browser history")
		.back()
		.expect("back traversal starts");
	settle_navigation().await;
	assert_eq!(current_location(), "/open/");
	assert!(root.inner_html().contains("OPEN"));

	web_sys::window()
		.expect("browser window")
		.history()
		.expect("browser history")
		.forward()
		.expect("forward traversal starts");
	settle_navigation().await;
	assert_eq!(current_location(), "/open/");
	assert!(root.inner_html().contains("OPEN"));
}

#[wasm_bindgen_test]
async fn explicit_auth_invalidation_replaces_the_protected_route() {
	let root = install_app_root_at("/protected/");
	GUARD_MODE.with(|mode| mode.set(1));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("protected launch");
	settle_navigation().await;
	assert!(root.inner_html().contains("PROTECTED"));

	GUARD_MODE.with(|mode| mode.set(0));
	invalidate_authentication();
	settle_navigation().await;
	assert_eq!(current_location(), "/login/");
	assert!(root.inner_html().contains("LOGIN"));
}

#[wasm_bindgen_test]
async fn failed_auth_revalidation_unmounts_the_protected_route() {
	let root = install_app_root_at("/protected/");
	GUARD_MODE.with(|mode| mode.set(1));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("protected launch");
	settle_navigation().await;
	assert!(root.inner_html().contains("PROTECTED"));

	GUARD_MODE.with(|mode| mode.set(4));
	invalidate_authentication();
	assert!(root.inner_html().is_empty());
	settle_navigation().await;

	assert_eq!(current_location(), "/protected/");
	assert!(root.inner_html().is_empty());
}

#[wasm_bindgen_test]
async fn managed_auth_statuses_apply_401_but_ignore_403() {
	let root = install_app_root_at("/protected/");
	GUARD_MODE.with(|mode| mode.set(1));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("protected launch");
	settle_navigation().await;
	assert!(root.inner_html().contains("PROTECTED"));

	observe_server_fn_status(403);
	settle_navigation().await;
	assert_eq!(current_location(), "/protected/");
	assert!(root.inner_html().contains("PROTECTED"));

	GUARD_MODE.with(|mode| mode.set(0));
	observe_server_fn_status(401);
	settle_navigation().await;
	assert_eq!(current_location(), "/login/");
}

#[wasm_bindgen_test]
async fn repeated_managed_401_responses_coalesce_to_one_revalidation() {
	let root = install_app_root_at("/protected/");
	GUARD_MODE.with(|mode| mode.set(1));
	GUARD_EVALUATIONS.with(|evaluations| evaluations.set(0));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("protected launch");
	settle_navigation().await;
	let evaluations_before = GUARD_EVALUATIONS.with(Cell::get);
	GUARD_MODE.with(|mode| mode.set(0));
	observe_server_fn_status(401);
	observe_server_fn_status(401);
	settle_navigation().await;

	assert_eq!(root.inner_html(), "<div id=\"guard-login\">LOGIN</div>");
	assert_eq!(current_location(), "/login/");
	assert_eq!(
		GUARD_EVALUATIONS.with(Cell::get),
		evaluations_before + 1,
		"duplicate 401 responses schedule one active-branch evaluation"
	);
}

#[wasm_bindgen_test]
async fn persistent_401_during_auth_revalidation_settles_once() {
	let root = install_app_root_at("/protected/");
	GUARD_MODE.with(|mode| mode.set(1));
	GUARD_EVALUATIONS.with(|evaluations| evaluations.set(0));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("protected launch");
	settle_navigation().await;
	let evaluations_before = GUARD_EVALUATIONS.with(Cell::get);
	GUARD_MODE.with(|mode| mode.set(7));

	invalidate_authentication();
	settle_navigation().await;

	assert_eq!(current_location(), "/protected/");
	assert_eq!(
		root.inner_html(),
		"<div id=\"guard-protected\">PROTECTED</div>"
	);
	assert_eq!(
		GUARD_EVALUATIONS.with(Cell::get),
		evaluations_before + 2,
		"persistent 401 responses stay inside one two-pass revalidation"
	);
}

#[rstest]
#[test_attr(wasm_bindgen_test)]
async fn rotated_jwt_supersedes_pending_auth_revalidation() {
	// Arrange
	let root = install_app_root_at("/protected/");
	auth_state().logout();
	clear_jwt_token();
	set_jwt_token("account-a");
	GUARD_MODE.with(|mode| mode.set(3));
	SESSION_FETCHES.with(|fetches| fetches.set(0));
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("protected launch");
	settle_navigation().await;
	assert_eq!(SESSION_FETCHES.with(Cell::get), 1);

	GUARD_MODE.with(|mode| mode.set(8));
	invalidate_authentication();
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(SESSION_FETCHES.with(Cell::get), 2);

	// Act
	set_jwt_token("account-b");
	invalidate_authentication();
	settle_navigation().await;
	let fetches = SESSION_FETCHES.with(Cell::get);
	clear_jwt_token();

	// Assert
	assert_eq!(current_location(), "/protected/");
	assert_eq!(
		root.inner_html(),
		"<div id=\"guard-protected\">PROTECTED</div>"
	);
	assert_eq!(
		fetches, 3,
		"a new JWT identity must clear and replace the pending revalidation"
	);
}

#[wasm_bindgen_test]
async fn redirect_chains_stop_for_same_target_and_two_route_loops() {
	let root = install_app_root_at("/");
	GUARD_EVALUATIONS.with(|evaluations| evaluations.set(0));
	GUARD_MODE.with(|mode| mode.set(5));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("home launch");
	RouterHandle
		.push("/protected/")
		.expect("same-target redirect starts");
	settle_navigation().await;
	assert_eq!(current_location(), "/");
	assert!(root.inner_html().contains("HOME"));
	assert!(GUARD_EVALUATIONS.with(Cell::get) >= 1);

	GUARD_EVALUATIONS.with(|evaluations| evaluations.set(0));
	GUARD_MODE.with(|mode| mode.set(6));
	RouterHandle
		.push("/guard-a/")
		.expect("two-target redirect starts");
	settle_navigation().await;
	assert_eq!(current_location(), "/");
	assert!(root.inner_html().contains("HOME"));
	assert!(GUARD_EVALUATIONS.with(Cell::get) >= 2);
}
