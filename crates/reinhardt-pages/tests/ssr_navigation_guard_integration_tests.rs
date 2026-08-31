#![cfg(not(target_arch = "wasm32"))]

use reinhardt_pages::reactive::QueryFamily;
use reinhardt_pages::{
	IntoPage, Loader, NavigationContext, NavigationDecision, NavigationGuardError, Outlet, Page,
	QueryOptions, SsrRenderer, component, layout, loader, navigation_guard,
};
use reinhardt_urls::routers::ClientRouter;
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};

static LOADER_CALLS: AtomicUsize = AtomicUsize::new(0);
static GUARD_CALLS: AtomicUsize = AtomicUsize::new(0);
static QUERY_CALLS: AtomicUsize = AtomicUsize::new(0);

#[navigation_guard]
async fn ssr_guard(context: NavigationContext) -> Result<NavigationDecision, NavigationGuardError> {
	GUARD_CALLS.fetch_add(1, Ordering::SeqCst);
	match context.destination() {
		"/ssr-redirect/" => Ok(NavigationDecision::Redirect {
			location: "/login?next=%2Faccount".to_owned(),
			replace: true,
		}),
		"/ssr-not-found/" => Ok(NavigationDecision::NotFound),
		"/ssr-forbidden/" => Ok(NavigationDecision::Forbidden),
		"/ssr-error/" => Err(NavigationGuardError::with_status("safe guard failure", 418)),
		_ => Ok(NavigationDecision::Allow),
	}
}

#[navigation_guard]
async fn ssr_query_guard(
	context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	let descriptor =
		QueryFamily::<(), String, NavigationGuardError>::new("ssr.session").query((), || async {
			QUERY_CALLS.fetch_add(1, Ordering::SeqCst);
			Ok("authenticated".to_owned())
		});
	context.query(descriptor, QueryOptions::new()).await?;
	Ok(NavigationDecision::Allow)
}

#[loader]
async fn ssr_guarded_loader() -> Result<String, String> {
	LOADER_CALLS.fetch_add(1, Ordering::SeqCst);
	Ok("protected content".to_owned())
}

#[component(
	"/ssr-redirect/",
	name = "ssr-redirect",
	navigation_guard = ssr_guard,
	loader = ssr_guarded_loader,
)]
fn ssr_redirect(Loader(value): Loader<String>) -> Page {
	Page::text(value)
}

#[component(
	"/ssr-not-found/",
	name = "ssr-not-found",
	navigation_guard = ssr_guard,
)]
fn ssr_not_found() -> Page {
	Page::text("protected not-found route")
}

#[component(
	"/ssr-forbidden/",
	name = "ssr-forbidden",
	navigation_guard = ssr_guard,
	loader = ssr_guarded_loader,
)]
fn ssr_forbidden(Loader(value): Loader<String>) -> Page {
	Page::text(value)
}

#[component(
	"/ssr-error/",
	name = "ssr-error",
	navigation_guard = ssr_guard,
)]
fn ssr_error() -> Page {
	Page::text("protected error route")
}

#[component(
	"/ssr-allowed/",
	name = "ssr-allowed",
	navigation_guard = ssr_query_guard,
	loader = ssr_guarded_loader,
)]
fn ssr_allowed(Loader(value): Loader<String>) -> Page {
	Page::text(value)
}

#[layout("/ssr-layout/", name = "ssr-layout", navigation_guard = ssr_guard)]
fn ssr_layout(outlet: Outlet) -> Page {
	outlet.into_page()
}

#[component("child/", name = "ssr-layout-child", navigation_guard = ssr_query_guard)]
fn ssr_layout_child() -> Page {
	Page::text("allowed layout child")
}

fn router() -> ClientRouter {
	ClientRouter::new()
		.not_found(|| Page::text("configured not found"))
		.component(ssr_redirect)
		.component(ssr_not_found)
		.component(ssr_forbidden)
		.component(ssr_error)
		.component(ssr_allowed)
		.routes(|routes| routes.layout(ssr_layout, |children| children.component(ssr_layout_child)))
}

fn reset_counts() {
	LOADER_CALLS.store(0, Ordering::SeqCst);
	GUARD_CALLS.store(0, Ordering::SeqCst);
	QUERY_CALLS.store(0, Ordering::SeqCst);
}

#[test]
#[serial(ssr_navigation_guard)]
fn redirect_is_safe_and_accessor_resets_for_next_render() {
	tokio_test::block_on(async {
		reset_counts();
		let router = router();
		let mut renderer = SsrRenderer::new();
		let redirect = renderer
			.render_route_to_string(&router, "/ssr-redirect/")
			.await;
		assert_eq!(redirect.status, 302);
		assert_eq!(
			renderer.route_redirect_location(),
			Some("/login?next=%2Faccount")
		);
		assert!(redirect.html.is_empty());
		assert_eq!(LOADER_CALLS.load(Ordering::SeqCst), 0);

		let allowed = renderer
			.render_route_to_string(&router, "/ssr-error/")
			.await;
		assert_eq!(allowed.status, 418);
		assert_eq!(renderer.route_redirect_location(), None);
	});
}

#[test]
#[serial(ssr_navigation_guard)]
fn non_allow_decisions_do_not_render_protected_content() {
	tokio_test::block_on(async {
		for (path, status, expected) in [
			("/ssr-not-found/", 404, "configured not found"),
			("/ssr-forbidden/", 403, "navigation forbidden"),
			("/ssr-error/", 418, "safe guard failure"),
		] {
			reset_counts();
			let router = router();
			let mut renderer = SsrRenderer::new();
			let output = renderer.render_route_to_string(&router, path).await;
			assert_eq!(output.status, status);
			assert!(output.html.contains(expected));
			assert!(!output.html.contains("protected"));
			assert_eq!(LOADER_CALLS.load(Ordering::SeqCst), 0);
		}
	});
}

#[test]
#[serial(ssr_navigation_guard)]
fn allowed_guards_run_twice_before_loader_and_query_fetches_once() {
	tokio_test::block_on(async {
		reset_counts();
		let router = router();
		let mut renderer = SsrRenderer::new();
		let output = renderer
			.render_route_to_string(&router, "/ssr-allowed/")
			.await;
		assert_eq!(output.status, 200);
		assert!(output.html.contains("protected content"));
		assert_eq!(GUARD_CALLS.load(Ordering::SeqCst), 0);
		assert_eq!(QUERY_CALLS.load(Ordering::SeqCst), 1);
		assert_eq!(LOADER_CALLS.load(Ordering::SeqCst), 1);
		assert!(renderer.state().to_json().unwrap().contains("ssr.session"));
	});
}

#[test]
#[serial(ssr_navigation_guard)]
fn allowed_parent_and_leaf_guards_run_before_loader() {
	tokio_test::block_on(async {
		reset_counts();
		let router = router();
		let mut renderer = SsrRenderer::new();
		let output = renderer
			.render_route_to_string(&router, "/ssr-layout/child/")
			.await;
		assert_eq!(output.status, 200);
		assert!(output.html.contains("allowed layout child"));
		assert_eq!(GUARD_CALLS.load(Ordering::SeqCst), 2);
	});
}
