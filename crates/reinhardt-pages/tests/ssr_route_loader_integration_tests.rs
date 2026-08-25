#![cfg(not(target_arch = "wasm32"))]

use reinhardt_pages::router::loader::{
	loader_cache_id, loader_cache_id_with_optional_queries, route_context,
};
use reinhardt_pages::router::loader_registry::LoaderRegistry;
use reinhardt_pages::{
	HydrationContext, Loader, Outlet, Page, Path, Query, QueryClient, QueryDefaults, RouteLoader,
	SsrRenderer, component, layout, loader, page,
};
use reinhardt_urls::routers::ClientRouter;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static SHARED_SSR_LOADER_CALLS: AtomicUsize = AtomicUsize::new(0);

#[loader]
async fn shared_ssr_loader() -> Result<String, String> {
	SHARED_SSR_LOADER_CALLS.fetch_add(1, Ordering::SeqCst);
	Ok("shared loader value".to_owned())
}

#[layout(
	"/ssr-shared-loader/",
	name = "ssr-shared-loader-shell",
	loader = shared_ssr_loader,
)]
fn ssr_shared_loader_shell(Loader(value): Loader<String>, outlet: Outlet) -> Page {
	page!(|value: String, outlet: Outlet| {
		section {
			{ value }
			{ outlet }
		}
	})(value, outlet)
}

#[component("child/", name = "ssr-shared-loader-child", loader = shared_ssr_loader)]
fn ssr_shared_loader_child(Loader(value): Loader<String>) -> Page {
	page!(|value: String| {
		p { { value } }
	})(value)
}

#[loader]
async fn ssr_greeting_loader() -> Result<String, String> {
	Ok("prepared on server".to_owned())
}

#[component(
	"/greeting/",
	name = "ssr-greeting",
	loader = ssr_greeting_loader
)]
fn ssr_greeting(Loader(message): Loader<String>) -> Page {
	page!(|message: String| {
		p { { message } }
	})(message)
}

#[loader]
async fn optional_ssr_loader(Query(logs): Query<Option<i64>>) -> Result<String, String> {
	Ok(logs.map_or_else(|| "none".to_owned(), |id| id.to_string()))
}

#[component(
	"/optional-loader/",
	name = "ssr-optional-loader",
	loader = optional_ssr_loader
)]
fn ssr_optional_loader(Loader(value): Loader<String>) -> Page {
	Page::text(value)
}

#[loader]
async fn ssr_timeout_loader() -> Result<String, String> {
	tokio::time::sleep(Duration::from_millis(20)).await;
	Ok("too late".to_owned())
}

#[component("/timeout/", name = "ssr-timeout", loader = ssr_timeout_loader)]
fn ssr_timeout(Loader(message): Loader<String>) -> Page {
	page!(|message: String| {
		p { { message } }
	})(message)
}

#[loader]
async fn ssr_slow_sibling_loader() -> Result<String, String> {
	tokio::time::sleep(Duration::from_millis(20)).await;
	Ok("slow sibling".to_owned())
}

#[loader]
async fn ssr_fast_failure_loader() -> Result<String, String> {
	Err("fast loader failure".to_owned())
}

#[layout(
	"/ssr-fail-fast/",
	name = "ssr-fail-fast-shell",
	loader = ssr_slow_sibling_loader,
)]
fn ssr_fail_fast_shell(Loader(_value): Loader<String>, outlet: Outlet) -> Page {
	page!(|outlet: Outlet| { { outlet } })(outlet)
}

#[component(
	"child/",
	name = "ssr-fail-fast-child",
	loader = ssr_fast_failure_loader
)]
fn ssr_fail_fast_child(Loader(_value): Loader<String>) -> Page {
	page!(|| {
		p { "unreachable" }
	})()
}

#[loader]
async fn ssr_shell_loader(Path(workspace_id): Path<i64>) -> Result<String, String> {
	Ok(format!("shell-{workspace_id}"))
}

#[layout(
	"/ssr-workspaces/{workspace_id}/",
	name = "ssr-workspace-shell",
	loader = ssr_shell_loader,
)]
fn ssr_workspace_shell(
	Path(workspace_id): Path<i64>,
	Loader(data): Loader<String>,
	outlet: Outlet,
) -> Page {
	page!(|workspace_id: i64, data: String, outlet: Outlet| {
		section {
			id: "ssr-shell",
			{ format!("SHELL {workspace_id} {data}") }
			{ outlet }
		}
	})(workspace_id, data, outlet)
}

#[loader]
async fn ssr_jobs_loader(Path(workspace_id): Path<i64>) -> Result<String, String> {
	Ok(format!("jobs-{workspace_id}"))
}

#[component("jobs", name = "ssr-workspace-jobs", loader = ssr_jobs_loader)]
fn ssr_workspace_jobs(Path(workspace_id): Path<i64>, Loader(data): Loader<String>) -> Page {
	page!(|workspace_id: i64, data: String| {
		p {
			id: "ssr-jobs",
			{ format!("JOBS {workspace_id} {data}") }
		}
	})(workspace_id, data)
}

#[test]
fn route_loader_is_prepared_before_ssr_render() {
	tokio_test::block_on(async {
		let router = ClientRouter::new().component(ssr_greeting);
		let mut renderer = SsrRenderer::new();

		let output = renderer.render_route_to_string(&router, "/greeting/").await;

		assert_eq!(output.status, 200);
		assert!(output.html.contains("prepared on server"));
		let loader_id = <ssr_greeting_loader::marker as RouteLoader>::ID;
		assert!(
			output
				.html
				.contains(&format!("route-loader:{}", loader_id.as_str()))
		);
		assert!(output.html.contains("prepared on server"));
		assert_eq!(
			renderer.state().get_route_loader_state(loader_id.as_str()),
			Some(&serde_json::json!("prepared on server"))
		);
		let matched = router.match_tree("/greeting/").expect("route matches");
		let key = loader_cache_id(loader_id, &route_context(&matched), &[])
			.expect("loader key is deterministic");
		assert_eq!(
			renderer.state().get_resource_state(&key),
			Some(&serde_json::json!({ "Success": "prepared on server" }))
		);
		let registry = LoaderRegistry::global().expect("loader registry is available");
		let client = QueryClient::new(QueryDefaults::default());
		let hydration = HydrationContext::from_state(renderer.state().clone());
		registry
			.seed_hydrated_query(&client, loader_id, &route_context(&matched), &hydration)
			.expect("loader value and query lease hydrate together");
	});
}

#[test]
fn optional_loader_query_reaches_ssr_and_uses_the_same_hydration_key() {
	tokio_test::block_on(async {
		let router = ClientRouter::new().component(ssr_optional_loader);
		let loader_id = <optional_ssr_loader::marker as RouteLoader>::ID;

		let mut absent_renderer = SsrRenderer::new();
		let absent = absent_renderer
			.render_route_to_string(&router, "/optional-loader/")
			.await;
		assert_eq!(absent.status, 200);
		assert_eq!(
			absent_renderer
				.state()
				.get_route_loader_state(loader_id.as_str()),
			Some(&serde_json::json!("none"))
		);
		let absent_match = router
			.match_tree("/optional-loader/")
			.expect("optional loader route matches");
		let absent_context = route_context(&absent_match);
		let absent_key = loader_cache_id_with_optional_queries(
			loader_id,
			&absent_context,
			optional_ssr_loader::INPUTS,
			optional_ssr_loader::OPTIONAL_QUERY_INPUTS,
		)
		.expect("missing optional query has a cache key");
		assert_eq!(
			absent_renderer.state().get_resource_state(&absent_key),
			Some(&serde_json::json!({ "Success": "none" }))
		);
		let registry = LoaderRegistry::global().expect("loader registry is available");
		let client = QueryClient::new(QueryDefaults::default());
		let hydration = HydrationContext::from_state(absent_renderer.state().clone());
		registry
			.seed_hydrated_query(&client, loader_id, &absent_context, &hydration)
			.expect("optional loader hydrates under the SSR key");

		let mut selected_renderer = SsrRenderer::new();
		let selected = selected_renderer
			.render_route_to_string(&router, "/optional-loader/?logs=42")
			.await;
		assert_eq!(selected.status, 200);
		assert_eq!(
			selected_renderer
				.state()
				.get_route_loader_state(loader_id.as_str()),
			Some(&serde_json::json!("42"))
		);

		let mut invalid_renderer = SsrRenderer::new();
		let invalid = invalid_renderer
			.render_route_to_string(&router, "/optional-loader/?logs=invalid")
			.await;
		assert_eq!(invalid.status, 400);
		assert_eq!(invalid_renderer.state().resource_count(), 0);
	});
}

#[test]
fn route_loader_timeout_returns_safe_status() {
	tokio_test::block_on(async {
		let router = ClientRouter::new().component(ssr_timeout);
		let mut renderer = SsrRenderer::with_options(
			reinhardt_pages::SsrOptions::new().resource_timeout(Duration::from_millis(1)),
		);

		let output = renderer.render_route_to_string(&router, "/timeout/").await;

		assert_eq!(output.status, 504);
		assert!(output.html.contains("route loader timed out"));
		assert_eq!(renderer.state().resource_count(), 0);
	});
}

#[test]
fn route_loader_failure_returns_before_a_slow_sibling_times_out() {
	tokio_test::block_on(async {
		let router = ClientRouter::new().routes(|routes| {
			routes.layout(ssr_fail_fast_shell, |children| {
				children.component(ssr_fail_fast_child)
			})
		});
		let mut renderer = SsrRenderer::with_options(
			reinhardt_pages::SsrOptions::new().resource_timeout(Duration::from_millis(1)),
		);

		let path = "/ssr-fail-fast/child/";
		let matched = router.match_tree(path).expect("route matches");
		assert_eq!(matched.loader_ids().len(), 2);
		let output = renderer.render_route_to_string(&router, path).await;

		assert_eq!(output.status, 500);
		assert!(output.html.contains("fast loader failure"));
		assert!(!output.html.contains("route loader timed out"));
		assert_eq!(renderer.state().resource_count(), 0);
	});
}

#[test]
fn route_render_clears_previous_loader_resource_state() {
	tokio_test::block_on(async {
		let router = ClientRouter::new()
			.component(ssr_greeting)
			.not_found(|| Page::text("custom SSR not found"));
		let mut renderer = SsrRenderer::new();

		let loaded = renderer.render_route_to_string(&router, "/greeting/").await;
		assert_eq!(loaded.status, 200);
		assert!(renderer.state().resource_count() > 0);

		let missing = renderer.render_route_to_string(&router, "/missing/").await;
		assert_eq!(missing.status, 404);
		assert_eq!(missing.html, "custom SSR not found");
		assert_eq!(renderer.state().resource_count(), 0);
	});
}

#[test]
fn nested_layout_and_leaf_loaders_prepare_in_parallel_for_ssr() {
	tokio_test::block_on(async {
		let router = ClientRouter::new().routes(|routes| {
			routes.layout(ssr_workspace_shell, |children| {
				children.component(ssr_workspace_jobs)
			})
		});
		let mut renderer = SsrRenderer::new();

		let output = renderer
			.render_route_to_string(&router, "/ssr-workspaces/7/jobs")
			.await;

		assert_eq!(output.status, 200);
		assert!(output.html.contains("SHELL 7 shell-7"));
		assert!(output.html.contains("JOBS 7 jobs-7"));
		let shell_id = <ssr_shell_loader::marker as RouteLoader>::ID;
		let jobs_id = <ssr_jobs_loader::marker as RouteLoader>::ID;
		assert_eq!(
			renderer.state().get_route_loader_state(shell_id.as_str()),
			Some(&serde_json::json!("shell-7"))
		);
		assert_eq!(
			renderer.state().get_route_loader_state(jobs_id.as_str()),
			Some(&serde_json::json!("jobs-7"))
		);
	});
}

#[test]
fn repeated_loader_is_prepared_once_for_ssr() {
	tokio_test::block_on(async {
		SHARED_SSR_LOADER_CALLS.store(0, Ordering::SeqCst);
		let router = ClientRouter::new().routes(|routes| {
			routes.layout(ssr_shared_loader_shell, |children| {
				children.component(ssr_shared_loader_child)
			})
		});
		let mut renderer = SsrRenderer::new();

		let output = renderer
			.render_route_to_string(&router, "/ssr-shared-loader/child/")
			.await;

		assert_eq!(output.status, 200);
		assert_eq!(SHARED_SSR_LOADER_CALLS.load(Ordering::SeqCst), 1);
	});
}
