//! WASM regression test for issues #4075 and #4088 — verifies that
//! `ClientLauncher::launch()` registers a `Router::on_navigate`
//! listener that re-mounts the root view on every `Router::push`.
//!
//! Without the fix, the SPA renders only the route mounted at boot;
//! every subsequent `Router::push` updates the path Signal but the
//! root view is never re-mounted.
//!
//! Refs #4101: the launcher migrated from reactive `Effect`/`Signal`
//! auto-tracking to explicit `Router::on_navigate` callbacks, so the
//! historical `debug_subscribers(path_signal_id)` diagnostic is no
//! longer meaningful — the user-observable HTML assertions below are
//! the authoritative regression check.
//!
//! **Run with** (from the workspace root):
//!   `wasm-pack test --headless --chrome crates/reinhardt-pages -- --test client_launcher_navigation_test`
//!
//! Cargo args (such as `--test ...`) must follow `--`; `wasm-pack` does not
//! accept Cargo flags before the path argument.

#![cfg(wasm)]

use reinhardt_core::page::Outlet;
use reinhardt_core::reactive::{Effect, ReactiveScope};
use reinhardt_pages::app::{ClientLauncher, with_spa_router};
use reinhardt_pages::component::{Head, IntoPage, Page, PageElement};
use reinhardt_pages::deps;
use reinhardt_pages::deps_auto;
use reinhardt_pages::dom::{Element, EventHandle};
use reinhardt_pages::reactive::hooks::{use_effect, use_retained_effect};
use reinhardt_pages::reactive::{
	QueryFamily, QueryOptions, Signal, queries, use_query, with_runtime,
};
use reinhardt_urls::routers::{ClientRouter, RouteMetadata};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

thread_local! {
	static RETAINED_ROUTE_TICK: RefCell<Option<Signal<i32>>> = const { RefCell::new(None) };
	static RETAINED_ROUTE_LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
	static RETAINED_REACTIVE_RENDER_TICK: RefCell<Option<Signal<i32>>> = const { RefCell::new(None) };
	static RETAINED_REACTIVE_EFFECT_TICK: RefCell<Option<Signal<i32>>> = const { RefCell::new(None) };
	static RETAINED_REACTIVE_LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
	static QUERY_OWNER_EVENT_HANDLES: RefCell<Vec<EventHandle>> = const { RefCell::new(Vec::new()) };
	static QUERY_OWNER_EFFECTS: RefCell<Vec<Effect>> = const { RefCell::new(Vec::new()) };
}

const LAUNCHER_OWNER_QUERY: QueryFamily<(), u32, String> = QueryFamily::new("tests.launcher-owner");

// Each route renders a div with a stable id and a unique text marker so the
// assertions can be tight regardless of how `Page::Text` serialises into
// `inner_html`.
fn page_root() -> Page {
	PageElement::new("div")
		.attr("id", "route-root")
		.child("ROUTE-ROOT-CONTENT")
		.into_page()
}

fn page_a() -> Page {
	PageElement::new("div")
		.attr("id", "route-a")
		.child("ROUTE-A-CONTENT")
		.into_page()
}

fn page_with_anchor_to_b() -> Page {
	PageElement::new("div")
		.attr("id", "route-a")
		.child("ROUTE-A-CONTENT")
		.child(
			PageElement::new("a")
				.attr("id", "link-to-b")
				.attr("href", "/b")
				.child("Go to B"),
		)
		.into_page()
}

fn page_b() -> Page {
	PageElement::new("div")
		.attr("id", "route-b")
		.child("ROUTE-B-CONTENT")
		.into_page()
}

fn reactive_page_b() -> Page {
	let content = Signal::new("ROUTE-B-CONTENT");
	Page::reactive(move || {
		PageElement::new("div")
			.attr("id", "route-b")
			.child(content.get())
			.into_page()
	})
}

fn layout_shell(outlet: Outlet) -> Page {
	PageElement::new("div")
		.attr("id", "layout-shell")
		.child("LAYOUT-SHELL")
		.child(outlet)
		.into_page()
}

fn managed_head_node(document: &web_sys::Document, selector: &str) -> web_sys::Element {
	document
		.query_selector(selector)
		.expect("head selector should be valid")
		.expect("managed head node should exist")
}

fn assert_single_managed_head_node(document: &web_sys::Document, selector: &str) {
	assert_eq!(
		document.query_selector_all(selector).unwrap().length(),
		1,
		"expected exactly one managed node for selector {selector}"
	);
}

fn reset_retained_route_state() -> Signal<i32> {
	let tick = Signal::new(0_i32);
	RETAINED_ROUTE_TICK.with(|slot| {
		*slot.borrow_mut() = Some(tick.clone());
	});
	RETAINED_ROUTE_LOG.with(|log| log.borrow_mut().clear());
	tick
}

fn retained_route_log() -> Vec<String> {
	RETAINED_ROUTE_LOG.with(|log| log.borrow().clone())
}

fn retained_route_page(label: &'static str) -> Page {
	let tick = RETAINED_ROUTE_TICK.with(|slot| {
		slot.borrow()
			.as_ref()
			.expect("retained route tick should be initialized")
			.clone()
	});

	use_retained_effect(
		{
			let tick = tick.clone();
			move || {
				let value = tick.get();
				RETAINED_ROUTE_LOG.with(|log| {
					log.borrow_mut().push(format!("run:{label}:{value}"));
				});
				Some(move || {
					RETAINED_ROUTE_LOG.with(|log| {
						log.borrow_mut().push(format!("cleanup:{label}"));
					});
				})
			}
		},
		deps![tick],
	);

	PageElement::new("div")
		.attr("id", format!("route-{label}"))
		.child(format!("ROUTE-{label}-CONTENT"))
		.into_page()
}

fn retained_route_a() -> Page {
	retained_route_page("a")
}

fn retained_route_b() -> Page {
	retained_route_page("b")
}

fn reset_retained_reactive_state() -> (Signal<i32>, Signal<i32>) {
	let render_tick = Signal::new(0_i32);
	let effect_tick = Signal::new(0_i32);
	RETAINED_REACTIVE_RENDER_TICK.with(|slot| {
		*slot.borrow_mut() = Some(render_tick.clone());
	});
	RETAINED_REACTIVE_EFFECT_TICK.with(|slot| {
		*slot.borrow_mut() = Some(effect_tick.clone());
	});
	RETAINED_REACTIVE_LOG.with(|log| log.borrow_mut().clear());
	(render_tick, effect_tick)
}

fn retained_reactive_log() -> Vec<String> {
	RETAINED_REACTIVE_LOG.with(|log| log.borrow().clone())
}

fn page_with_retained_effect_in_reactive_body() -> Page {
	Page::reactive(|| {
		let render_tick = RETAINED_REACTIVE_RENDER_TICK.with(|slot| {
			slot.borrow()
				.as_ref()
				.expect("retained reactive render tick should be initialized")
				.clone()
		});
		let effect_tick = RETAINED_REACTIVE_EFFECT_TICK.with(|slot| {
			slot.borrow()
				.as_ref()
				.expect("retained reactive effect tick should be initialized")
				.clone()
		});
		let render_value = render_tick.get();

		use_retained_effect(
			{
				let effect_tick = effect_tick.clone();
				move || {
					let value = effect_tick.get();
					RETAINED_REACTIVE_LOG.with(|log| {
						log.borrow_mut().push(format!("run:{value}"));
					});
					Some(move || {
						RETAINED_REACTIVE_LOG.with(|log| {
							log.borrow_mut().push("cleanup".to_string());
						});
					})
				}
			},
			deps![effect_tick],
		);

		PageElement::new("div")
			.attr("id", "retained-reactive")
			.child(format!("RETAINED-REACTIVE-{render_value}"))
			.into_page()
	})
}

fn page_with_reentrant_nested_reactive() -> Page {
	let trigger = Signal::new(0_i32);
	let trigger_for_outer = trigger;

	Page::reactive(move || {
		let _ = trigger_for_outer.get();
		let trigger_for_inner = trigger_for_outer;

		Page::reactive(move || {
			if trigger_for_inner.get_untracked() == 0 {
				trigger_for_inner.set(1);
			}

			PageElement::new("div")
				.attr("id", "route-reentrant")
				.child("ROUTE-REENTRANT-CONTENT")
				.into_page()
		})
	})
}

fn install_app_root() -> web_sys::Element {
	let document = web_sys::window().unwrap().document().unwrap();
	replace_history_path("/");
	if let Some(prev) = document.get_element_by_id("app") {
		prev.remove();
	}
	let root = document.create_element("div").unwrap();
	root.set_id("app");
	document.body().unwrap().append_child(&root).unwrap();
	root
}

fn install_named_app_root(id: &str) -> web_sys::Element {
	let document = web_sys::window().unwrap().document().unwrap();
	if let Some(previous) = document.get_element_by_id(id) {
		previous.remove();
	}
	let root = document.create_element("div").unwrap();
	root.set_id(id);
	document.body().unwrap().append_child(&root).unwrap();
	root
}

fn replace_history_path(path: &str) {
	let history = web_sys::window().unwrap().history().unwrap();
	history
		.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path))
		.expect("replace history path");
}

/// Yields control so the reactive scheduler (which uses
/// `wasm_bindgen_futures::spawn_local`) can drain queued work.
async fn yield_to_microtasks() {
	gloo_timers::future::TimeoutFuture::new(0).await;
}

fn install_launcher_query_owner_probe(
	root: &web_sys::Element,
	event_type: &'static str,
	fetch_count: Rc<Cell<u32>>,
) {
	let query = use_query(
		LAUNCHER_OWNER_QUERY.query((), move || {
			let fetch_count = Rc::clone(&fetch_count);
			async move {
				let call = fetch_count.get() + 1;
				fetch_count.set(call);
				Ok(call)
			}
		}),
		QueryOptions::default(),
	);
	let render_root = root.clone();
	let effect = use_effect(
		move || {
			let _owning_client = queries();
			if let Some(value) = query.data() {
				render_root
					.set_attribute("data-query-value", &value.to_string())
					.expect("query probe render attribute");
			}
			None::<fn()>
		},
		deps_auto!(),
	);
	let event = Element::new(root.clone()).add_event_listener(event_type, move || {
		queries().invalidate_family(LAUNCHER_OWNER_QUERY);
	});
	QUERY_OWNER_EFFECTS.with(|effects| effects.borrow_mut().push(effect));
	QUERY_OWNER_EVENT_HANDLES.with(|events| events.borrow_mut().push(event));
}

#[wasm_bindgen_test]
async fn late_event_and_render_reenter_the_first_launcher_query_client() {
	replace_history_path("/");
	QUERY_OWNER_EVENT_HANDLES.with(|events| events.borrow_mut().clear());
	QUERY_OWNER_EFFECTS.with(|effects| effects.borrow_mut().clear());
	let first_root = install_named_app_root("query-owner-app-a");
	let second_root = install_named_app_root("query-owner-app-b");
	let first_fetches = Rc::new(Cell::new(0_u32));
	let second_fetches = Rc::new(Cell::new(0_u32));

	ClientLauncher::new("#query-owner-app-a")
		.router_client(|| ClientRouter::new().route("root-a", "/", page_root))
		.after_launch({
			let first_fetches = Rc::clone(&first_fetches);
			move |ctx| {
				install_launcher_query_owner_probe(
					ctx.root_element(),
					"reinhardt-query-owner-a",
					first_fetches,
				);
			}
		})
		.launch()
		.expect("launch first app");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert_eq!(first_fetches.get(), 1);
	assert_eq!(
		first_root.get_attribute("data-query-value").as_deref(),
		Some("1")
	);

	ClientLauncher::new("#query-owner-app-b")
		.router_client(|| ClientRouter::new().route("root-b", "/", page_root))
		.after_launch({
			let second_fetches = Rc::clone(&second_fetches);
			move |ctx| {
				install_launcher_query_owner_probe(
					ctx.root_element(),
					"reinhardt-query-owner-b",
					second_fetches,
				);
			}
		})
		.launch()
		.expect("launch second app");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert_eq!(first_fetches.get(), 1);
	assert_eq!(second_fetches.get(), 1);

	first_root
		.dispatch_event(&web_sys::Event::new("reinhardt-query-owner-a").expect("owner event"))
		.expect("dispatch owner event");
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	assert_eq!(first_fetches.get(), 2);
	assert_eq!(second_fetches.get(), 1);
	assert_eq!(
		first_root.get_attribute("data-query-value").as_deref(),
		Some("2"),
		"the first launcher's late render must use its original query client"
	);

	QUERY_OWNER_EVENT_HANDLES.with(|events| events.borrow_mut().clear());
	QUERY_OWNER_EFFECTS.with(|effects| effects.borrow_mut().clear());
	first_root.remove();
	second_root.remove();
}

#[wasm_bindgen_test]
async fn client_launcher_re_renders_on_router_push() {
	let root = install_app_root();

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new()
				.route("root", "/", page_root)
				.route("a", "/a", page_a)
				.route("b", "/b", page_b)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;

	// Navigate to /a and confirm the body switches.
	with_spa_router(|r| r.push("/a")).expect("push /a");
	let pending_after_push_a = with_runtime(|rt| rt.debug_pending_updates());
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	let html_after_a = root.inner_html();
	assert!(
		html_after_a.contains("ROUTE-A-CONTENT"),
		"[DIAG #4088] expected /a view after push('/a'). \
		 pending_updates immediately after push: {:?}, actual html: {}",
		pending_after_push_a,
		html_after_a,
	);
	assert!(
		!html_after_a.contains("ROUTE-B-CONTENT"),
		"expected /b view absent after push('/a'), got: {html_after_a}"
	);

	// Navigate to /b — this is the regression-critical step.
	with_spa_router(|r| r.push("/b")).expect("push /b");
	let pending_after_push_b = with_runtime(|rt| rt.debug_pending_updates());
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	let html_after_b = root.inner_html();
	assert!(
		html_after_b.contains("ROUTE-B-CONTENT"),
		"[DIAG #4088] expected /b view after push('/b'). \
		 pending_updates immediately after push: {:?}, actual html: {}",
		pending_after_push_b,
		html_after_b,
	);
	assert!(
		!html_after_b.contains("ROUTE-A-CONTENT"),
		"expected /a view absent after push('/b'), got: {html_after_b}"
	);
}

#[wasm_bindgen_test]
async fn client_launcher_re_renders_after_intercepted_anchor_click() {
	let root = install_app_root();

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new()
				.route("root", "/", page_root)
				.route("a", "/a", page_with_anchor_to_b)
				.route("b", "/b", page_b)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;

	with_spa_router(|r| r.push("/a")).expect("push /a");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("ROUTE-A-CONTENT"),
		"setup precondition: expected /a before clicking link, got: {}",
		root.inner_html()
	);

	let document = web_sys::window().unwrap().document().unwrap();
	let anchor: web_sys::HtmlElement = document
		.get_element_by_id("link-to-b")
		.expect("link-to-b exists")
		.dyn_into()
		.expect("link-to-b is HtmlElement");
	anchor.click();
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	let path = web_sys::window()
		.unwrap()
		.location()
		.pathname()
		.expect("location pathname");
	let html_after_click = root.inner_html();
	assert_eq!(path, "/b");
	assert!(
		html_after_click.contains("ROUTE-B-CONTENT"),
		"Refs #5104: intercepted anchor click changed the URL but did not rerender /b, got: {html_after_click}"
	);
	assert!(
		!html_after_click.contains("ROUTE-A-CONTENT"),
		"Refs #5104: previous /a view should be gone after anchor navigation, got: {html_after_click}"
	);
}

#[wasm_bindgen_test]
async fn client_launcher_preserves_layout_shell_between_sibling_routes() {
	let root = install_app_root();
	replace_history_path("/a");

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new().routes(|routes| {
				routes.layout_route("shell", "/", layout_shell, |children| {
					children
						.route("a", "a", page_a)
						.route("b", "b", reactive_page_b)
				})
			})
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;

	let document = web_sys::window().unwrap().document().unwrap();
	let shell = document
		.get_element_by_id("layout-shell")
		.expect("layout shell should mount");
	shell
		.set_attribute("data-preserved", "yes")
		.expect("mark shell");
	assert!(
		root.inner_html().contains("ROUTE-A-CONTENT"),
		"setup precondition: expected /a outlet content, got: {}",
		root.inner_html()
	);

	with_spa_router(|r| r.push("/b")).expect("push /b");
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	let shell_after = document
		.get_element_by_id("layout-shell")
		.expect("layout shell should persist");
	assert_eq!(
		shell_after.get_attribute("data-preserved").as_deref(),
		Some("yes"),
		"layout shell DOM was remounted instead of being preserved"
	);
	let html = root.inner_html();
	assert!(
		html.contains("ROUTE-B-CONTENT"),
		"expected /b content, got: {html}"
	);
	assert!(
		!html.contains("ROUTE-A-CONTENT"),
		"/a outlet content should be replaced, got: {html}"
	);
	replace_history_path("/");
}

#[wasm_bindgen_test]
async fn client_launcher_preserves_layout_head_across_sibling_navigation_and_history() {
	let root = install_app_root();
	replace_history_path("/a");

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new()
				.routes(|routes| {
					routes.layout_route("shell", "/", layout_shell, |children| {
						children.route("a", "a", page_a).route("b", "b", page_b)
					})
				})
				.with_route_metadata(
					"shell",
					RouteMetadata::new()
						.with_head(Head::new().meta_description("layout-description")),
				)
				.with_route_metadata(
					"a",
					RouteMetadata::new().with_head(Head::new().title("Route A").canonical("/a")),
				)
				.with_route_metadata(
					"b",
					RouteMetadata::new().with_head(Head::new().title("Route B").canonical("/b")),
				)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;
	let document = web_sys::window().unwrap().document().unwrap();
	assert_eq!(document.title(), "Route A");
	let layout_description = managed_head_node(
		&document,
		"meta[name='description'][content='layout-description'][data-reinhardt-head]",
	);
	assert_single_managed_head_node(
		&document,
		"meta[name='description'][content='layout-description'][data-reinhardt-head]",
	);
	managed_head_node(
		&document,
		"link[rel='canonical'][href='/a'][data-reinhardt-head]",
	);
	assert_single_managed_head_node(
		&document,
		"link[rel='canonical'][href='/a'][data-reinhardt-head]",
	);

	with_spa_router(|r| r.push("/b")).expect("push /b");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert_eq!(document.title(), "Route B");
	assert!(layout_description.is_same_node(Some(&managed_head_node(
		&document,
		"meta[name='description'][content='layout-description'][data-reinhardt-head]",
	))));
	assert_single_managed_head_node(
		&document,
		"meta[name='description'][content='layout-description'][data-reinhardt-head]",
	);
	assert!(
		document
			.query_selector("link[rel='canonical'][href='/a'][data-reinhardt-head]")
			.unwrap()
			.is_none()
	);
	managed_head_node(
		&document,
		"link[rel='canonical'][href='/b'][data-reinhardt-head]",
	);
	assert_single_managed_head_node(
		&document,
		"link[rel='canonical'][href='/b'][data-reinhardt-head]",
	);

	web_sys::window()
		.unwrap()
		.history()
		.unwrap()
		.back()
		.unwrap();
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert_eq!(document.title(), "Route A");
	assert!(layout_description.is_same_node(Some(&managed_head_node(
		&document,
		"meta[name='description'][content='layout-description'][data-reinhardt-head]",
	))));
	assert_single_managed_head_node(
		&document,
		"meta[name='description'][content='layout-description'][data-reinhardt-head]",
	);
	managed_head_node(
		&document,
		"link[rel='canonical'][href='/a'][data-reinhardt-head]",
	);
	assert!(root.inner_html().contains("ROUTE-A-CONTENT"));
	replace_history_path("/");
}

#[wasm_bindgen_test]
async fn client_launcher_clears_route_head_when_next_route_has_no_metadata() {
	let root = install_app_root();
	replace_history_path("/metadata");
	let document = web_sys::window().unwrap().document().unwrap();
	let original_title = document.title();
	document.set_title("Route head fallback");

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new()
				.route("metadata", "/metadata", page_a)
				.route("plain", "/plain", page_b)
				.with_route_metadata(
					"metadata",
					RouteMetadata::new().with_head(
						Head::new()
							.title("Metadata route")
							.meta_description("metadata-description")
							.canonical("/metadata"),
					),
				)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;
	assert_eq!(document.title(), "Metadata route");
	assert_single_managed_head_node(
		&document,
		"meta[name='description'][content='metadata-description'][data-reinhardt-head]",
	);
	assert_single_managed_head_node(
		&document,
		"link[rel='canonical'][href='/metadata'][data-reinhardt-head]",
	);

	with_spa_router(|router| router.push("/plain")).expect("push /plain");
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	assert_eq!(document.title(), "Route head fallback");
	assert!(
		document
			.query_selector(
				"meta[name='description'][content='metadata-description'][data-reinhardt-head]"
			)
			.unwrap()
			.is_none(),
		"route metadata must not leak into a route without metadata"
	);
	assert!(
		document
			.query_selector("link[rel='canonical'][href='/metadata'][data-reinhardt-head]")
			.unwrap()
			.is_none(),
		"route canonical metadata must not leak into a route without metadata"
	);
	assert!(root.inner_html().contains("ROUTE-B-CONTENT"));

	document.set_title(&original_title);
	replace_history_path("/");
}

#[wasm_bindgen_test]
async fn retained_route_effects_are_disposed_on_sibling_navigation() {
	let root = install_app_root();
	replace_history_path("/a");
	let scope = ReactiveScope::new();
	let tick = scope.enter(reset_retained_route_state);

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new().routes(|routes| {
				routes.layout_route("shell", "/", layout_shell, |children| {
					children
						.route("a", "a", retained_route_a)
						.route("b", "b", retained_route_b)
				})
			})
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("ROUTE-a-CONTENT"),
		"setup precondition: expected retained /a content, got: {}",
		root.inner_html()
	);

	with_spa_router(|r| r.push("/b")).expect("push /b");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	tick.set(1);
	with_runtime(|rt| rt.flush_updates());

	let log = retained_route_log();
	assert_eq!(
		log.iter()
			.filter(|entry| entry.starts_with("run:a:"))
			.count(),
		1,
		"previous leaf route retained effect must not re-run after sibling navigation: {log:?}"
	);
	assert!(
		log.iter().any(|entry| entry == "cleanup:a"),
		"previous leaf route retained effect cleanup should run on sibling navigation: {log:?}"
	);
	assert_eq!(
		log.iter()
			.filter(|entry| entry.starts_with("run:b:"))
			.count(),
		2,
		"current leaf route retained effect should run initially and after tick update: {log:?}"
	);
	replace_history_path("/");
}

#[wasm_bindgen_test]
async fn retained_effects_in_reactive_body_are_replaced_on_rerender() {
	let root = install_app_root();
	let scope = ReactiveScope::new();
	let (render_tick, effect_tick) = scope.enter(reset_retained_reactive_state);

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new().route(
				"retained-reactive",
				"/",
				page_with_retained_effect_in_reactive_body,
			)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("RETAINED-REACTIVE-0"),
		"setup precondition: expected retained reactive view, got: {}",
		root.inner_html()
	);

	render_tick.set(1);
	with_runtime(|rt| rt.flush_updates());
	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("RETAINED-REACTIVE-1"),
		"reactive body should rerender after render tick, got: {}",
		root.inner_html()
	);

	effect_tick.set(1);
	with_runtime(|rt| rt.flush_updates());

	let log = retained_reactive_log();
	assert_eq!(
		log.iter().filter(|entry| entry.as_str() == "run:0").count(),
		2,
		"initial effect and replacement effect should each run once before dep update: {log:?}"
	);
	assert_eq!(
		log.iter().filter(|entry| entry.as_str() == "run:1").count(),
		1,
		"only the current retained effect should re-run after dep update: {log:?}"
	);
	assert_eq!(
		log.iter()
			.filter(|entry| entry.as_str() == "cleanup")
			.count(),
		2,
		"retained effects should clean up on reactive rerender and before the dependency-driven rerun: {log:?}"
	);
	replace_history_path("/");
}

/// Direct reproduction of Issue #4088: simulates the reinhardt-cloud dashboard
/// flow with /, /login, /register, /clusters where /clusters has no route
/// (falls through to not_found).
#[wasm_bindgen_test]
async fn client_launcher_reproduces_issue_4088_navigation_flow() {
	let root = install_app_root();

	fn login_page() -> Page {
		PageElement::new("div")
			.attr("id", "login")
			.child("LOGIN-CONTENT")
			.into_page()
	}
	fn register_page() -> Page {
		PageElement::new("div")
			.attr("id", "register")
			.child("REGISTER-CONTENT")
			.into_page()
	}
	fn dashboard() -> Page {
		PageElement::new("div")
			.attr("id", "dashboard")
			.child("DASHBOARD-CONTENT")
			.into_page()
	}
	fn not_found() -> Page {
		PageElement::new("div")
			.attr("id", "not-found")
			.child("NOT-FOUND-CONTENT")
			.into_page()
	}

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new()
				.route("dashboard", "/", dashboard)
				.route("login", "/login", login_page)
				.route("register", "/register", register_page)
				.not_found(not_found)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("DASHBOARD-CONTENT"),
		"boot mount: expected DASHBOARD-CONTENT, got {}",
		root.inner_html()
	);

	// Issue #4088 row 2: /login should render login_page after Router::push
	with_spa_router(|r| r.push("/login")).expect("push /login");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("LOGIN-CONTENT"),
		"#4088 reproduction: /login must render login_page, got {}",
		root.inner_html()
	);

	// Issue #4088 row 3: /register
	with_spa_router(|r| r.push("/register")).expect("push /register");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("REGISTER-CONTENT"),
		"#4088 reproduction: /register must render register_page, got {}",
		root.inner_html()
	);

	// Issue #4088 row 4: /clusters has no route — must fall through to not_found
	with_spa_router(|r| r.push("/clusters")).expect("push /clusters");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	assert!(
		root.inner_html().contains("NOT-FOUND-CONTENT"),
		"#4088 reproduction: unmatched path /clusters must render not_found, got {}",
		root.inner_html()
	);
}

/// Regression test for the popstate gap fixed in #4108: browser
/// back/forward must trigger the launcher's render path, which means
/// the underlying `on_navigate` observers must fire from popstate.
/// Before the fix, only programmatic `Router::push` / `Router::replace`
/// woke observers.
#[wasm_bindgen_test]
async fn client_launcher_re_renders_on_popstate() {
	let root = install_app_root();

	let observed_paths: std::rc::Rc<std::cell::RefCell<Vec<String>>> =
		std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
	let observed_paths_for_listener = observed_paths.clone();

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new()
				.route("root", "/", page_root)
				.route("a", "/a", page_a)
				.route("b", "/b", page_b)
		})
		.after_launch(move |_ctx| {
			// after_launch is FnOnce, so observed_paths_for_listener can
			// be moved straight into the listener body without an extra
			// clone.
			let sub = with_spa_router(move |r| {
				r.on_navigate_dyn(Box::new(move |path, _params| {
					observed_paths_for_listener
						.borrow_mut()
						.push(path.to_string());
				}))
			});
			// Leak the subscription for the lifetime of the test; it is
			// dropped naturally when the WASM module exits.
			std::mem::forget(sub);
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;

	// Arrange: navigate forward to /a then /b so popstate has somewhere
	// to pop back to.
	with_spa_router(|r| r.push("/a")).expect("push /a");
	yield_to_microtasks().await;
	yield_to_microtasks().await;
	with_spa_router(|r| r.push("/b")).expect("push /b");
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	let html_at_b = root.inner_html();
	assert!(
		html_at_b.contains("ROUTE-B-CONTENT"),
		"setup precondition: should be on /b before history.back, got: {html_at_b}"
	);

	// Act: simulate the browser back button.
	let history = web_sys::window().unwrap().history().unwrap();
	history.back().expect("history.back");
	// popstate is dispatched as a macrotask. The yield_to_microtasks
	// helper uses TimeoutFuture::new(0), which is itself a macrotask
	// boundary (setTimeout(0)), so two yields suffice to let popstate
	// fire and then for the launcher's render Effect (or, post-#4101,
	// on_navigate listener) to commit the DOM update.
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	// Assert: DOM reflects /a, and the on_navigate observer received the
	// popstate path (this is the bit that used to silently break).
	let html_after_back = root.inner_html();
	assert!(
		html_after_back.contains("ROUTE-A-CONTENT"),
		"expected /a view after history.back from /b, got: {html_after_back}"
	);
	assert!(
		!html_after_back.contains("ROUTE-B-CONTENT"),
		"/b view should be gone after history.back, got: {html_after_back}"
	);

	let paths = observed_paths.borrow();
	assert_eq!(
		paths.as_slice(),
		&["/a".to_string(), "/b".to_string(), "/a".to_string()],
		"on_navigate listener must fire for each push and the popstate, in order"
	);
}

/// Regression coverage for the structural fragility class
/// (#3348, #4075, #4088). Multiple back-to-back navigations exercise
/// the full render -> cleanup_reactive_nodes -> remount cycle
/// repeatedly. Once PR #4101 removes the launcher's render Effect,
/// the Effect/Signal auto-tracking corruption pattern is structurally
/// impossible regardless of the reactive primitives embedded in route
/// views; this test guards the navigation cycle itself.
///
/// Refs #4101, #4088, #4075, #3348.
#[wasm_bindgen_test]
async fn client_launcher_handles_back_to_back_navigations() {
	let root = install_app_root();

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new()
				.route("root", "/", page_root)
				.route("a", "/a", page_a)
				.route("b", "/b", page_b)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;

	// Act: bounce between /a and /b multiple times. The regression
	// class manifested as the second or third navigation no longer
	// re-mounting.
	for iteration in 0..3 {
		with_spa_router(|r| r.push("/a")).expect("push /a");
		yield_to_microtasks().await;
		yield_to_microtasks().await;
		assert!(
			root.inner_html().contains("ROUTE-A-CONTENT"),
			"iteration {iteration}: expected /a view after push('/a'), got: {}",
			root.inner_html()
		);
		assert!(
			!root.inner_html().contains("ROUTE-B-CONTENT"),
			"iteration {iteration}: /b view should be gone after push('/a'), got: {}",
			root.inner_html()
		);

		with_spa_router(|r| r.push("/b")).expect("push /b");
		yield_to_microtasks().await;
		yield_to_microtasks().await;
		assert!(
			root.inner_html().contains("ROUTE-B-CONTENT"),
			"iteration {iteration}: expected /b view after push('/b'), got: {}",
			root.inner_html()
		);
		assert!(
			!root.inner_html().contains("ROUTE-A-CONTENT"),
			"iteration {iteration}: /a view should be gone after push('/b'), got: {}",
			root.inner_html()
		);
	}
}

#[wasm_bindgen_test]
async fn nested_reactive_content_is_removed_with_outer_owner() {
	use reinhardt_pages::component::PageExt;
	use reinhardt_pages::dom::Element;

	let root = install_app_root();
	let scope = ReactiveScope::new();
	let (authorized, secret) = scope.enter(|| {
		let authorized = Signal::new(true);
		let secret = Signal::new("SECRET-42".to_owned());
		let authorized_for_outer = authorized.clone();
		let secret_for_inner = secret.clone();

		Page::reactive(move || {
			if authorized_for_outer.get() {
				let secret_for_render = secret_for_inner.clone();
				Page::reactive(move || Page::text(secret_for_render.get()))
			} else {
				Page::Empty
			}
		})
		.mount(&Element::new(root.clone()))
		.expect("mount nested reactive page");

		(authorized, secret)
	});

	yield_to_microtasks().await;
	assert!(
		root.text_content()
			.unwrap_or_default()
			.contains("SECRET-42"),
		"expected secret to render before authorization is revoked, got: {}",
		root.inner_html()
	);

	authorized.set(false);
	with_runtime(|rt| rt.flush_updates());
	yield_to_microtasks().await;
	assert!(
		!root
			.text_content()
			.unwrap_or_default()
			.contains("SECRET-42"),
		"secret should be removed when the outer reactive owner rerenders, got: {}",
		root.inner_html()
	);

	secret.set("SECRET-99".to_owned());
	with_runtime(|rt| rt.flush_updates());
	yield_to_microtasks().await;
	assert!(
		!root
			.text_content()
			.unwrap_or_default()
			.contains("SECRET-99"),
		"detached nested reactive effect should not reinsert secret content, got: {}",
		root.inner_html()
	);
}

#[wasm_bindgen_test]
async fn client_launcher_handles_reentrant_reactive_mount_during_navigation() {
	let root = install_app_root();

	ClientLauncher::new("#app")
		.router_client(|| {
			ClientRouter::new().route("root", "/", page_root).route(
				"reentrant",
				"/reentrant",
				page_with_reentrant_nested_reactive,
			)
		})
		.launch()
		.expect("launch");

	yield_to_microtasks().await;

	with_spa_router(|r| r.push("/reentrant")).expect("push /reentrant");
	yield_to_microtasks().await;
	yield_to_microtasks().await;

	let html = root.inner_html();
	assert!(
		html.contains("ROUTE-REENTRANT-CONTENT"),
		"expected reentrant route to render without RefCell reentry panic, got: {html}"
	);
}
