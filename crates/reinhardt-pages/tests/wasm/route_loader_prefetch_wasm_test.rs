//! Browser-level coverage for route-loader link prefetching.

#![cfg(wasm)]

use reinhardt_pages::app::ClientLauncher;
use reinhardt_pages::component::{Component, IntoPage, Page, PageElement};
use reinhardt_pages::router::request::RouteContext;
use reinhardt_pages::router::{Link, PrefetchMode};
use reinhardt_pages::{
	Loader, QueryFamily, QueryOptions, RouteLoader, RouteLoaderError, component, loader,
	loader_cache_id, use_query,
};
use reinhardt_urls::routers::ClientRouter;
use std::cell::Cell;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

thread_local! {
	static PREFETCH_CALLS: Cell<u32> = const { Cell::new(0) };
}

#[loader]
async fn prefetched_loader() -> Result<String, String> {
	PREFETCH_CALLS.with(|calls| calls.set(calls.get() + 1));
	Ok("PREFETCHED DATA".to_string())
}

#[component("/prefetched", name = "prefetched-page", loader = prefetched_loader)]
fn prefetched_page(Loader(data): Loader<String>) -> Page {
	let context = RouteContext::new("/prefetched".to_owned(), HashMap::new(), String::new());
	let cache_id = loader_cache_id(
		<prefetched_loader::marker as RouteLoader>::ID,
		&context,
		prefetched_loader::INPUTS,
	)
	.expect("prefetched loader cache ID");
	let query = use_query(
		QueryFamily::<String, String, RouteLoaderError>::new(
			<prefetched_loader::marker as RouteLoader>::ID.as_str(),
		)
		.query_with_cancellation(cache_id, |_cancellation| async {
			prefetched_loader().await.map_err(RouteLoaderError::new)
		}),
		QueryOptions::default(),
	);
	assert_eq!(query.data().as_deref(), Some(data.as_str()));
	PageElement::new("div")
		.attr("id", "route-prefetched")
		.child(data)
		.into_page()
}

#[loader]
async fn viewport_prefetched_loader() -> Result<String, String> {
	PREFETCH_CALLS.with(|calls| calls.set(calls.get() + 1));
	Ok("VIEWPORT PREFETCHED DATA".to_string())
}

#[component(
	"/viewport-prefetched",
	name = "viewport-prefetched-page",
	loader = viewport_prefetched_loader
)]
fn viewport_prefetched_page(Loader(data): Loader<String>) -> Page {
	PageElement::new("div")
		.attr("id", "route-viewport-prefetched")
		.child(data)
		.into_page()
}

fn home_page() -> Page {
	PageElement::new("div")
		.attr("id", "route-home")
		.child(
			Link::new("/prefetched", "Prefetch destination")
				.attr("id", "prefetch-link")
				.prefetch(PrefetchMode::Hover)
				.render(),
		)
		.into_page()
}

fn viewport_home_page() -> Page {
	PageElement::new("div")
		.attr("id", "route-viewport-home")
		.child(
			Link::new("/viewport-prefetched", "Viewport prefetch destination")
				.attr("id", "viewport-prefetch-link")
				.prefetch(PrefetchMode::Viewport)
				.render(),
		)
		.into_page()
}

fn install_app_root() -> web_sys::Element {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let history = web_sys::window()
		.expect("window")
		.history()
		.expect("history");
	history
		.replace_state_with_url(&JsValue::NULL, "", Some("/"))
		.expect("reset history path");
	if let Some(previous) = document.get_element_by_id("app") {
		previous.remove();
	}
	let root = document.create_element("div").expect("create root");
	root.set_id("app");
	document
		.body()
		.expect("body")
		.append_child(&root)
		.expect("append root");
	root
}

fn build_router() -> ClientRouter {
	ClientRouter::new()
		.route("home", "/", home_page)
		.component(prefetched_page)
}

fn build_viewport_router() -> ClientRouter {
	ClientRouter::new()
		.route("home", "/", viewport_home_page)
		.component(viewport_prefetched_page)
}

fn viewport_navigation_home_page() -> Page {
	PageElement::new("div")
		.attr("id", "viewport-navigation-home")
		.child(
			Link::new("/viewport-next", "Viewport next")
				.attr("id", "viewport-navigation-link")
				.prefetch(PrefetchMode::Viewport)
				.render(),
		)
		.into_page()
}

fn viewport_navigation_next_page() -> Page {
	PageElement::new("div")
		.attr("id", "viewport-navigation-next")
		.child(
			Link::new("/", "Viewport home")
				.attr("id", "viewport-navigation-return-link")
				.prefetch(PrefetchMode::Viewport)
				.render(),
		)
		.into_page()
}

fn build_viewport_navigation_router() -> ClientRouter {
	ClientRouter::new()
		.route("home", "/", viewport_navigation_home_page)
		.route(
			"viewport-next",
			"/viewport-next",
			viewport_navigation_next_page,
		)
}

async fn yield_to_tasks() {
	gloo_timers::future::TimeoutFuture::new(0).await;
}

#[wasm_bindgen_test]
fn prefetch_modes_render_explicit_data_attributes() {
	let hover = Link::new("/hover", "Hover").prefetch(PrefetchMode::Hover);
	let viewport = Link::new("/viewport", "Viewport").prefetch(PrefetchMode::Viewport);
	let hover_html = hover.render().render_to_string();
	let viewport_html = viewport.render().render_to_string();
	assert!(hover_html.contains("data-prefetch=\"hover\""));
	assert!(viewport_html.contains("data-prefetch=\"viewport\""));
}

#[wasm_bindgen_test]
async fn hover_prefetch_is_side_effect_free_and_shared_by_navigation() {
	let root = install_app_root();
	PREFETCH_CALLS.with(|calls| calls.set(0));
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("launch");

	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let anchor: web_sys::HtmlElement = document
		.get_element_by_id("prefetch-link")
		.expect("prefetch link")
		.dyn_into()
		.expect("link is HtmlElement");
	assert_eq!(
		anchor.get_attribute("data-prefetch").as_deref(),
		Some("hover")
	);

	let pointerover_init = web_sys::PointerEventInit::new();
	pointerover_init.set_bubbles(true);
	let pointerover =
		web_sys::PointerEvent::new_with_event_init_dict("pointerover", &pointerover_init)
			.expect("pointerover event");
	anchor
		.dispatch_event(&pointerover)
		.expect("dispatch pointerover");
	yield_to_tasks().await;
	yield_to_tasks().await;

	// Prefetch must not mutate the URL or mount the destination route.
	assert_eq!(
		web_sys::window()
			.expect("window")
			.location()
			.pathname()
			.expect("pathname"),
		"/"
	);
	assert!(root.inner_html().contains("route-home"));
	assert_eq!(PREFETCH_CALLS.with(Cell::get), 1);

	anchor.click();
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(
		web_sys::window()
			.expect("window")
			.location()
			.pathname()
			.expect("pathname"),
		"/prefetched"
	);
	assert!(root.inner_html().contains("PREFETCHED DATA"));
	assert_eq!(
		PREFETCH_CALLS.with(Cell::get),
		1,
		"navigation should reuse the shared query-cache result"
	);

	// Exercise the keyboard-intent listener as well; it is idempotent after
	// the destination is already cached and must not create a second request.
	let focusin = web_sys::FocusEvent::new("focusin").expect("focusin event");
	anchor.dispatch_event(&focusin).expect("dispatch focusin");
}

#[wasm_bindgen_test]
async fn viewport_prefetch_is_observed_after_the_link_mounts() {
	let root = install_app_root();
	PREFETCH_CALLS.with(|calls| calls.set(0));
	ClientLauncher::new("#app")
		.router_client(build_viewport_router)
		.launch()
		.expect("launch");

	assert!(root.inner_html().contains("viewport-prefetch-link"));
	for _ in 0..6 {
		yield_to_tasks().await;
	}
	assert_eq!(PREFETCH_CALLS.with(Cell::get), 1);
}

#[wasm_bindgen_test]
async fn viewport_observer_disconnects_stale_targets_before_a_navigation_rescan() {
	let root = install_app_root();
	let window = web_sys::window().expect("window");
	let key = JsValue::from_str("IntersectionObserver");
	let original = js_sys::Reflect::get(window.as_ref(), &key).expect("observer value");
	let observer_constructor = js_sys::Function::new_no_args(
		r#"
			return class {
				constructor() {
					window.__reinhardtViewportObserverEvents = window.__reinhardtViewportObserverEvents || [];
					window.__reinhardtViewportObserverEvents.push("construct");
				}
				observe(target) {
					window.__reinhardtViewportObserverEvents.push(`observe:${target.id}`);
				}
				disconnect() {
					window.__reinhardtViewportObserverEvents.push("disconnect");
				}
			}
		"#,
	)
	.call0(&JsValue::NULL)
	.expect("create observer constructor");
	js_sys::Reflect::set(window.as_ref(), &key, &observer_constructor)
		.expect("install observer constructor");
	js_sys::Reflect::set(
		window.as_ref(),
		&JsValue::from_str("__reinhardtViewportObserverEvents"),
		&js_sys::Array::new(),
	)
	.expect("reset observer events");

	ClientLauncher::new("#app")
		.router_client(build_viewport_navigation_router)
		.launch()
		.expect("launch");
	assert!(root.inner_html().contains("viewport-navigation-link"));

	let document = window.document().expect("document");
	let anchor: web_sys::HtmlElement = document
		.get_element_by_id("viewport-navigation-link")
		.expect("viewport navigation link")
		.dyn_into()
		.expect("link is HtmlElement");
	anchor.click();
	yield_to_tasks().await;
	yield_to_tasks().await;

	let events: js_sys::Array = js_sys::Reflect::get(
		window.as_ref(),
		&JsValue::from_str("__reinhardtViewportObserverEvents"),
	)
	.expect("observer events")
	.dyn_into()
	.expect("observer events are an array");
	let values = events
		.iter()
		.filter_map(|event| event.as_string())
		.collect::<Vec<_>>();
	assert_eq!(
		values
			.iter()
			.filter(|event| event.as_str() == "construct")
			.count(),
		2,
		"each scan owns an observer for its current route DOM: {values:?}"
	);
	assert_eq!(
		values
			.iter()
			.filter(|event| event.as_str() == "disconnect")
			.count(),
		1,
		"the first observer must release detached route anchors before the rescan: {values:?}"
	);
	assert!(
		values
			.iter()
			.any(|event| event == "observe:viewport-navigation-link")
	);
	assert!(
		values
			.iter()
			.any(|event| event == "observe:viewport-navigation-return-link")
	);

	js_sys::Reflect::set(window.as_ref(), &key, &original).expect("restore observer constructor");
}

#[wasm_bindgen_test]
fn launch_without_intersection_observer_when_no_viewport_link_exists() {
	let _root = install_app_root();
	let window = web_sys::window().expect("window");
	let key = JsValue::from_str("IntersectionObserver");
	let original = js_sys::Reflect::get(window.as_ref(), &key).expect("observer value");
	js_sys::Reflect::set(window.as_ref(), &key, &JsValue::UNDEFINED).expect("clear observer");

	let result = ClientLauncher::new("#app")
		.router_client(build_router)
		.launch();
	js_sys::Reflect::set(window.as_ref(), &key, &original).expect("restore observer");
	result.expect("launch does not require viewport observation");
}
