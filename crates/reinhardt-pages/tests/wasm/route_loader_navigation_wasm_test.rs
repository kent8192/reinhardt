//! Browser-level coverage for route-loader prepare/commit navigation.
//!
//! These tests exercise the pages-owned coordinator through the public
//! `RouterHandle` API. A destination is not rendered until every matched
//! loader has prepared successfully; failures leave both the old URL and the
//! old DOM mounted.

#![cfg(wasm)]

use reinhardt_pages::app::ClientLauncher;
use reinhardt_pages::component::{Head, IntoPage, Page, PageElement};
use reinhardt_pages::reactive::hooks::RouterHandle;
use reinhardt_pages::router::request::RouteContext;
use reinhardt_pages::{
	Loader, Outlet, Query, QueryFamily, QueryOptions, RouteLoader, RouteLoaderError, component,
	layout, loader, loader_cache_id, use_query,
};
use reinhardt_urls::routers::{ClientRouter, RouteMetadata};
use std::cell::Cell;
use std::collections::HashMap;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

thread_local! {
	static LOADED_CALLS: Cell<u32> = const { Cell::new(0) };
	static INITIAL_LOADER_AFTER_LAUNCH_SAW_MOUNT: Cell<bool> = const { Cell::new(false) };
	static INITIAL_LOADER_PATH_SUBSCRIPTION_CALLS: Cell<u32> = const { Cell::new(0) };
}

fn home_page() -> Page {
	PageElement::new("div")
		.attr("id", "route-home")
		.child("HOME")
		.into_page()
}

#[loader]
async fn loaded_loader() -> Result<String, String> {
	LOADED_CALLS.with(|calls| calls.set(calls.get() + 1));
	Ok("LOADED DATA".to_string())
}

#[component("/loaded", name = "loader-navigation-loaded", loader = loaded_loader)]
fn loaded_page(Loader(data): Loader<String>) -> Page {
	let context = RouteContext::new("/loaded".to_owned(), HashMap::new(), String::new());
	let cache_id = loader_cache_id(
		<loaded_loader::marker as RouteLoader>::ID,
		&context,
		loaded_loader::INPUTS,
	)
	.expect("loaded loader cache ID");
	let query = use_query(
		QueryFamily::<String, String, RouteLoaderError>::new(
			<loaded_loader::marker as RouteLoader>::ID.as_str(),
		)
		.query_with_cancellation(cache_id, |_cancellation| async {
			loaded_loader().await.map_err(RouteLoaderError::new)
		}),
		QueryOptions::default(),
	);
	assert_eq!(query.data().as_deref(), Some(data.as_str()));
	PageElement::new("div")
		.attr("id", "route-loaded")
		.child(data)
		.into_page()
}

#[loader]
async fn query_loaded_loader(Query(tab): Query<String>) -> Result<String, String> {
	Ok(format!("TAB: {tab}"))
}

#[component(
	"/query-loaded",
	name = "loader-navigation-query-loaded",
	loader = query_loaded_loader
)]
fn query_loaded_page(Loader(data): Loader<String>) -> Page {
	PageElement::new("div")
		.attr("id", "route-query-loaded")
		.child(data)
		.into_page()
}

#[loader]
async fn persistent_layout_loader(Query(tab): Query<String>) -> Result<String, String> {
	Ok(format!("LAYOUT: {tab}"))
}

#[layout(
	"/persistent/",
	name = "loader-navigation-persistent-layout",
	loader = persistent_layout_loader
)]
fn persistent_layout(Loader(data): Loader<String>, outlet: Outlet) -> Page {
	PageElement::new("section")
		.attr("id", "persistent-layout")
		.child(data)
		.child(outlet.into_page())
		.into_page()
}

#[component("view/", name = "loader-navigation-persistent-leaf")]
fn persistent_leaf() -> Page {
	PageElement::new("div")
		.attr("id", "persistent-leaf")
		.child("LEAF")
		.into_page()
}

#[layout(
	"/query-persistent/",
	name = "loader-navigation-query-persistent-layout"
)]
fn query_persistent_layout(Query(tab): Query<String>, outlet: Outlet) -> Page {
	PageElement::new("section")
		.attr("id", "query-persistent-layout")
		.child(format!("QUERY LAYOUT: {tab}"))
		.child(outlet.into_page())
		.into_page()
}

#[component("view/", name = "loader-navigation-query-persistent-leaf")]
fn query_persistent_leaf() -> Page {
	PageElement::new("div")
		.attr("id", "query-persistent-leaf")
		.child("QUERY LEAF")
		.into_page()
}

#[loader]
async fn slow_loader() -> Result<String, String> {
	gloo_timers::future::TimeoutFuture::new(30).await;
	Ok("SLOW DATA".to_string())
}

#[component("/slow", name = "loader-navigation-slow", loader = slow_loader)]
fn slow_page(Loader(data): Loader<String>) -> Page {
	PageElement::new("div")
		.attr("id", "route-slow")
		.child(data)
		.into_page()
}

#[loader]
async fn failing_loader() -> Result<String, String> {
	Err("safe loader failure".to_string())
}

#[component("/failed", name = "loader-navigation-failed", loader = failing_loader)]
fn failed_page(Loader(data): Loader<String>) -> Page {
	PageElement::new("div")
		.attr("id", "route-failed")
		.child(data)
		.into_page()
}

fn install_app_root_at(path: &str) -> web_sys::Element {
	let document = web_sys::window()
		.expect("window")
		.document()
		.expect("document");
	let history = web_sys::window()
		.expect("window")
		.history()
		.expect("history");
	history
		.replace_state_with_url(&JsValue::NULL, "", Some(path))
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

fn install_app_root() -> web_sys::Element {
	install_app_root_at("/")
}

fn build_router() -> ClientRouter {
	ClientRouter::new()
		.route("home", "/", home_page)
		.component(loaded_page)
		.component(query_loaded_page)
		.component(slow_page)
		.component(failed_page)
		.with_route_metadata(
			"home",
			RouteMetadata::new().with_head(Head::new().title("Home")),
		)
		.with_route_metadata(
			"loader-navigation-loaded",
			RouteMetadata::new().with_head(Head::new().title("Loaded")),
		)
		.with_route_metadata(
			"loader-navigation-slow",
			RouteMetadata::new().with_head(Head::new().title("Slow")),
		)
		.with_route_metadata(
			"loader-navigation-failed",
			RouteMetadata::new().with_head(Head::new().title("Failed")),
		)
}

fn build_persistent_layout_router() -> ClientRouter {
	ClientRouter::new().routes(|routes| {
		routes.layout(persistent_layout, |children| {
			children.component(persistent_leaf)
		})
	})
}

fn build_query_persistent_layout_router() -> ClientRouter {
	ClientRouter::new().routes(|routes| {
		routes.layout(query_persistent_layout, |children| {
			children.component(query_persistent_leaf)
		})
	})
}

async fn yield_to_tasks() {
	gloo_timers::future::TimeoutFuture::new(0).await;
}

fn current_path() -> String {
	web_sys::window()
		.expect("window")
		.location()
		.pathname()
		.expect("pathname")
}

fn current_location() -> String {
	let window = web_sys::window().expect("window");
	let location = window.location();
	format!(
		"{}{}",
		location.pathname().expect("pathname"),
		location.search().expect("search")
	)
}

#[wasm_bindgen_test]
async fn route_loader_navigation_commits_after_prepare() {
	let root = install_app_root();
	LOADED_CALLS.with(|calls| calls.set(0));
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("launch");

	assert!(root.inner_html().contains("HOME"));
	assert_eq!(current_path(), "/");
	let initial_state = web_sys::window()
		.expect("window")
		.history()
		.expect("history")
		.state()
		.expect("history state");
	assert_eq!(
		js_sys::Reflect::get(&initial_state, &JsValue::from_str("entry_index"))
			.expect("entry_index")
			.as_f64(),
		Some(0.0)
	);
	RouterHandle
		.push("/loaded")
		.expect("start loader navigation");

	// Matching and preparation are separate from commit: the old route remains
	// visible until the loader future settles.
	assert_eq!(current_path(), "/");
	assert!(root.inner_html().contains("HOME"));
	assert!(!root.inner_html().contains("LOADED DATA"));

	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(current_path(), "/loaded");
	assert!(root.inner_html().contains("LOADED DATA"));
	assert_eq!(
		web_sys::window().unwrap().document().unwrap().title(),
		"Loaded"
	);
	let committed_state = web_sys::window()
		.expect("window")
		.history()
		.expect("history")
		.state()
		.expect("history state");
	assert_eq!(
		js_sys::Reflect::get(&committed_state, &JsValue::from_str("entry_index"))
			.expect("entry_index")
			.as_f64(),
		Some(1.0)
	);
	assert_eq!(LOADED_CALLS.with(Cell::get), 1);
}

#[wasm_bindgen_test]
async fn route_loader_navigation_retains_old_route_while_pending() {
	let root = install_app_root();
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("launch");

	RouterHandle.push("/slow").expect("start slow navigation");
	yield_to_tasks().await;
	assert_eq!(current_path(), "/");
	assert!(root.inner_html().contains("HOME"));
	assert!(!root.inner_html().contains("SLOW DATA"));

	gloo_timers::future::TimeoutFuture::new(45).await;
	yield_to_tasks().await;
	assert_eq!(current_path(), "/slow");
	assert!(root.inner_html().contains("SLOW DATA"));
}

#[wasm_bindgen_test]
async fn route_loader_navigation_failure_retains_old_route() {
	let root = install_app_root();
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("launch");

	RouterHandle
		.push("/failed")
		.expect("start failing navigation");
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(current_path(), "/");
	assert!(root.inner_html().contains("HOME"));
	assert!(!root.inner_html().contains("route-failed"));
	assert_eq!(
		web_sys::window().unwrap().document().unwrap().title(),
		"Home"
	);
}

#[wasm_bindgen_test]
async fn client_only_initial_loader_prepares_query_values_without_ssr_state() {
	let root = install_app_root_at("/query-loaded?tab=initial");
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("client-only loader launch succeeds");

	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(current_location(), "/query-loaded?tab=initial");
	assert!(root.inner_html().contains("TAB: initial"));
}

#[wasm_bindgen_test]
async fn client_only_initial_loader_failure_renders_a_safe_error_surface() {
	let root = install_app_root_at("/failed");
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("client-only loader launch succeeds before asynchronous failure");

	yield_to_tasks().await;
	yield_to_tasks().await;

	assert_eq!(current_path(), "/failed");
	assert!(root.inner_html().contains("data-route-error"));
	assert!(root.inner_html().contains("safe loader failure"));
}

#[wasm_bindgen_test]
async fn client_only_initial_loader_activates_post_mount_lifecycle_after_commit() {
	let root = install_app_root_at("/query-loaded?tab=initial");
	INITIAL_LOADER_AFTER_LAUNCH_SAW_MOUNT.with(|value| value.set(false));
	INITIAL_LOADER_PATH_SUBSCRIPTION_CALLS.with(|value| value.set(0));

	ClientLauncher::new("#app")
		.router_client(build_router)
		.after_launch(|ctx| {
			INITIAL_LOADER_AFTER_LAUNCH_SAW_MOUNT.with(|value| {
				value.set(ctx.root_element().inner_html().contains("TAB: initial"));
			});
		})
		.on_path("/query-loaded", |_ctx| {
			INITIAL_LOADER_PATH_SUBSCRIPTION_CALLS.with(|value| value.set(value.get() + 1));
		})
		.launch()
		.expect("client-only loader launch succeeds");

	yield_to_tasks().await;
	yield_to_tasks().await;

	assert!(root.inner_html().contains("TAB: initial"));
	assert!(INITIAL_LOADER_AFTER_LAUNCH_SAW_MOUNT.with(Cell::get));
	assert_eq!(INITIAL_LOADER_PATH_SUBSCRIPTION_CALLS.with(Cell::get), 1);
}

#[wasm_bindgen_test]
async fn pop_navigation_preserves_the_destination_query_for_loader_preparation() {
	let root = install_app_root();
	ClientLauncher::new("#app")
		.router_client(build_router)
		.launch()
		.expect("launch");

	RouterHandle
		.push("/query-loaded?tab=first")
		.expect("first query navigation starts");
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert!(root.inner_html().contains("TAB: first"));

	RouterHandle
		.push("/query-loaded?tab=second")
		.expect("second query navigation starts");
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert!(root.inner_html().contains("TAB: second"));

	web_sys::window()
		.expect("window")
		.history()
		.expect("history")
		.back()
		.expect("history back");
	yield_to_tasks().await;
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert_eq!(current_location(), "/query-loaded?tab=first");
	assert!(root.inner_html().contains("TAB: first"));
}

#[wasm_bindgen_test]
async fn persistent_layout_remounts_when_its_loader_query_input_changes() {
	let root = install_app_root_at("/persistent/view/?tab=one");
	ClientLauncher::new("#app")
		.router_client(build_persistent_layout_router)
		.launch()
		.expect("launch");
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert!(root.inner_html().contains("LAYOUT: one"));

	RouterHandle
		.replace("/persistent/view/?tab=two")
		.expect("query navigation starts");
	yield_to_tasks().await;
	yield_to_tasks().await;
	assert!(root.inner_html().contains("LAYOUT: two"));
}

#[wasm_bindgen_test]
async fn persistent_layout_remounts_when_its_query_prop_changes_without_a_loader() {
	let root = install_app_root_at("/query-persistent/view/?tab=one");
	ClientLauncher::new("#app")
		.router_client(build_query_persistent_layout_router)
		.launch()
		.expect("launch");
	assert!(root.inner_html().contains("QUERY LAYOUT: one"));

	RouterHandle
		.replace("/query-persistent/view/?tab=two")
		.expect("query navigation starts");
	yield_to_tasks().await;
	yield_to_tasks().await;

	assert!(root.inner_html().contains("QUERY LAYOUT: two"));
}
