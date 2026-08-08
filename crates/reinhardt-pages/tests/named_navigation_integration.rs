#![cfg(not(target_arch = "wasm32"))]

use reinhardt_core::reactive::ReactiveScope;
use reinhardt_pages::app::{
	__clear_spa_router_for_test, __current_path_for_test, __install_client_router_for_test,
};
use reinhardt_pages::component::Page;
use reinhardt_pages::{
	NavigateError, NavigationType, navigate_named, navigate_or_reload, route_params,
};
use reinhardt_urls::routers::ClientRouter;
use serial_test::serial;
use std::cell::Cell;

fn build_router() -> ClientRouter {
	ClientRouter::new()
		.route("home", "/", || Page::text("Home"))
		.route(
			"project-settings",
			"/projects/{project_id}/settings/",
			|| Page::text("Settings"),
		)
		.route(
			"workspace-document",
			"/workspaces/{workspace_id}/documents/{slug}/",
			|| Page::text("Document"),
		)
}

struct SpaRouterGuard;

impl SpaRouterGuard {
	fn install() -> Self {
		__install_client_router_for_test(build_router());
		Self
	}
}

impl Drop for SpaRouterGuard {
	fn drop(&mut self) {
		__clear_spa_router_for_test();
	}
}

#[test]
#[serial(router)]
fn named_push_formats_mixed_display_parameters() {
	ReactiveScope::run(|| {
		let _guard = SpaRouterGuard::install();
		let result = navigate_named(
			"workspace-document",
			route_params! {
				"workspace_id" => 42_i64,
				"slug" => "draft",
			},
			NavigationType::Push,
		);

		assert!(result.is_ok(), "named Push should succeed: {result:?}");
		assert_eq!(
			__current_path_for_test().as_deref(),
			Some("/workspaces/42/documents/draft/")
		);
	});
}

#[test]
#[serial(router)]
fn named_replace_accepts_a_homogeneous_array() {
	ReactiveScope::run(|| {
		let _guard = SpaRouterGuard::install();
		let result = navigate_named(
			"project-settings",
			[("project_id", 7_i64)],
			NavigationType::Replace,
		);

		assert!(result.is_ok(), "named Replace should succeed: {result:?}");
		assert_eq!(
			__current_path_for_test().as_deref(),
			Some("/projects/7/settings/")
		);
	});
}

#[test]
#[serial(router)]
fn named_navigation_maps_reverse_errors_exactly() {
	ReactiveScope::run(|| {
		let _guard = SpaRouterGuard::install();

		let unknown = navigate_named("missing-route", route_params! {}, NavigationType::Push)
			.expect_err("an unknown route name must fail");
		assert!(matches!(&unknown, NavigateError::RouteResolutionFailed(_)));
		assert_eq!(
			unknown.to_string(),
			"route resolution failed: Invalid route name: missing-route"
		);

		let missing = navigate_named("project-settings", route_params! {}, NavigationType::Push)
			.expect_err("a missing route parameter must fail");
		assert!(matches!(&missing, NavigateError::RouteResolutionFailed(_)));
		assert_eq!(
			missing.to_string(),
			"route resolution failed: Missing parameter: unknown"
		);
	});
}

#[test]
#[serial(router)]
fn named_navigation_requires_an_installed_router() {
	let _component = Page::text("No router");
	__clear_spa_router_for_test();

	let result = navigate_named("home", route_params! {}, NavigationType::Push);

	assert!(matches!(result, Err(NavigateError::RouterNotInstalled)));
}

#[test]
#[serial(router)]
fn route_params_evaluates_each_value_once() {
	let _component = Page::text("Route params");
	let evaluations = Cell::new(0_u8);
	let params = route_params! {
		"project_id" => {
			evaluations.set(evaluations.get() + 1);
			9_i64
		},
	};

	assert_eq!(evaluations.get(), 1);
	assert_eq!(params, vec![("project_id", "9".to_owned())]);
}

#[test]
#[serial(router)]
fn native_fallback_preserves_router_not_installed() {
	let _component = Page::text("Native fallback");
	__clear_spa_router_for_test();

	let result = navigate_or_reload("/login/", NavigationType::Push);

	assert!(matches!(result, Err(NavigateError::RouterNotInstalled)));
}

#[test]
#[serial(router)]
fn browser_originated_navigation_types_remain_noops_without_a_router() {
	let _component = Page::text("Browser navigation");
	__clear_spa_router_for_test();

	assert!(
		navigate_or_reload("/ignored/", NavigationType::Pop).is_ok(),
		"Pop must not require a router or invoke fallback"
	);
	assert!(
		navigate_or_reload("/ignored/", NavigationType::Initial).is_ok(),
		"Initial must not require a router or invoke fallback"
	);
}
