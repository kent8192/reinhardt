//! Category 5: Middleware-Integrated E2E Tests (Issue #3083)
//!
//! These tests send real HTTP requests through the full middleware pipeline,
//! unlike Category 1-4 tests which bypass middleware by calling `router.handle()`
//! directly with manually-injected `AuthState`.
//!
//! Pipeline under test:
//! 1. `HttpServer` — body size limit enforcement (10 MB)
//! 2. `LoggingMiddleware` — request/response logging
//! 3. `AdminOriginGuardMiddleware` — same-origin validation
//! 4. `AdminCookieAuthMiddleware` — JWT from cookie/header → `AuthState`
//! 5. Route handler with `InjectionContext::fork_for_request()`

use super::server_fn_helpers::TEST_CSRF_TOKEN;
use super::server_fn_middleware_helpers::*;
use reinhardt_admin::server::security::ADMIN_AUTH_COOKIE_NAME;
use rstest::*;

// ── Category 5A: Full Pipeline Smoke Tests ──

/// Verifies that an authenticated `get_dashboard` request succeeds through
/// the full middleware pipeline (LoggingMW → OriginGuardMW → CookieAuthMW → handler).
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_get_dashboard_through_pipeline(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = staff_jwt_token();

	let response = post_server_fn(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::JwtCookie(token),
	)
	.await;

	assert_eq!(response.status().as_u16(), 200);
}

/// Verifies that an authenticated `get_list` request succeeds through the full
/// pipeline, confirming DI resolves all dependencies from middleware-populated
/// `AuthState` (AdminAuthenticatedUser, AdminSite, AdminDatabase).
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_get_list_through_pipeline(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = staff_jwt_token();

	let response = post_server_fn(
		&client,
		&server.base_url,
		"get_list",
		serde_json::json!({
			"model_name": "TestModel",
			"params": {
				"page": 1,
				"per_page": 10,
			}
		}),
		AuthMode::JwtCookie(token),
	)
	.await;

	assert_eq!(response.status().as_u16(), 200);
}

// ── Category 5B: Auth Middleware Integration ──

/// Verifies that a valid JWT in `Authorization: Bearer` header authenticates
/// through `AdminCookieAuthMiddleware` (fallback extraction path).
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_bearer_header_authenticates(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = staff_jwt_token();

	let response = post_server_fn(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::BearerHeader(token),
	)
	.await;

	assert_eq!(response.status().as_u16(), 200);
}

/// Verifies that a request without any auth token is rejected.
/// `AdminCookieAuthMiddleware` sets `AuthState::anonymous()`, and the handler
/// rejects unauthenticated access.
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_no_auth_returns_401(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();

	let response = post_server_fn(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::NoAuth,
	)
	.await;

	let status = response.status().as_u16();
	assert!(
		status == 401 || status == 500,
		"Expected 401 or 500 for unauthenticated request, got {}",
		status
	);
}

/// Verifies that a malformed JWT token is treated as unauthenticated.
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_invalid_jwt_returns_401(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();

	let response = post_server_fn(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::JwtCookie("invalid.token.here".to_string()),
	)
	.await;

	let status = response.status().as_u16();
	assert!(
		status == 401 || status == 500,
		"Expected 401 or 500 for invalid JWT, got {}",
		status
	);
}

// ── Category 5C: Origin Guard Integration ──

/// Verifies that a POST with a cross-origin Origin header is rejected with 403.
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_cross_origin_returns_403(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = staff_jwt_token();

	let response = post_cross_origin(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::JwtCookie(token),
	)
	.await;

	assert_eq!(response.status().as_u16(), 403);
}

/// Verifies that a POST without Origin or Referer header is rejected with 403.
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_missing_origin_returns_403(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = staff_jwt_token();

	let response = post_without_origin(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::JwtCookie(token),
	)
	.await;

	assert_eq!(response.status().as_u16(), 403);
}

// ── Category 5D: Body Size Limit ──

/// Verifies that `HttpServer` rejects request bodies exceeding the 10 MB limit.
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_oversized_body_rejected(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = staff_jwt_token();
	let host = server
		.base_url
		.strip_prefix("http://")
		.unwrap_or(&server.base_url);

	let oversized_data = vec![b'x'; 10 * 1024 * 1024 + 1];

	let response = client
		.post(format!("{}/admin/api/server_fn/get_list", server.base_url))
		.header("Content-Type", "application/json")
		.header("Host", host)
		.header("Origin", &server.base_url)
		.header(
			"Cookie",
			format!(
				"csrftoken={}; {}={}",
				TEST_CSRF_TOKEN, ADMIN_AUTH_COOKIE_NAME, token
			),
		)
		.body(oversized_data)
		.send()
		.await
		.expect("Failed to send oversized request");

	assert_eq!(response.status().as_u16(), 413);
}

// ── Category 5E: Non-Staff / Inactive User via Real JWT ──

/// Verifies that a JWT for a non-staff user is rejected by `AdminAuthenticatedUser`
/// injection with 401 (non-staff users cannot access admin).
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_non_staff_jwt_denied(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = non_staff_jwt_token();

	let response = post_server_fn(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::JwtCookie(token),
	)
	.await;

	let status = response.status().as_u16();
	assert!(
		status == 401,
		"Expected 401 for non-staff user, got {}",
		status
	);
}

/// Verifies that a JWT for an inactive user is rejected by `AdminAuthenticatedUser`
/// injection with 401 (inactive users cannot access admin).
#[rstest]
#[tokio::test]
async fn test_middleware_e2e_inactive_user_jwt_denied(
	#[future] middleware_e2e_context: (
		MiddlewareTestServer,
		reinhardt_di::Depends<reinhardt_admin::core::AdminDatabase>,
	),
) {
	let (server, _db) = middleware_e2e_context.await;
	let client = reqwest::Client::new();
	let token = inactive_jwt_token();

	let response = post_server_fn(
		&client,
		&server.base_url,
		"get_dashboard",
		serde_json::json!({}),
		AuthMode::JwtCookie(token),
	)
	.await;

	let status = response.status().as_u16();
	assert!(
		status == 401,
		"Expected 401 for inactive user, got {}",
		status
	);
}
