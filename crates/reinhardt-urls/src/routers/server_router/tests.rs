//! Unit tests for ServerRouter, ServerRouter splitting, and helpers.

use super::*;
use hyper::Method;
use reinhardt_core::endpoint::EndpointInfo;
use reinhardt_http::{Handler, Request, Response, Result, SyncHandler};
use rstest::rstest;
use std::sync::Arc;

#[cfg(feature = "viewsets")]
use reinhardt_views::viewsets::{
	Action, ActionMetadata, PermissionMiddleware, ViewSet, ViewSetBuilder, ViewSetMiddleware,
};

struct TestEndpoint<const ID: u8>;

impl<const ID: u8> EndpointInfo for TestEndpoint<ID> {
	fn path() -> &'static str {
		match ID {
			1 => "/health",
			2 => "/list",
			3 => "/users/{id}",
			4 => "/users/{id}/posts",
			5 => "/posts/{post_id}/comments/{comment_id}",
			6 => "/orgs/{org}/clusters/{cluster_id}/",
			7 => "/users",
			8 => "/posts",
			9 => "/items",
			10 => "/api/users/",
			11 => "/api/server_fn/test",
			12 => "/api/users",
			13 => "/auth/register/",
			14 => "/auth/login/",
			15 => "/users/",
			16 => "/dashboard/",
			17 => "/api/health",
			18 => "/sub/action/",
			19 => "/profile/",
			20 => "/detail/",
			21 => "/edit/",
			22 => "/catch/",
			23 => "/users",
			24 => "/items",
			25 => "/users",
			26 => "/items",
			27 => "/users",
			28 => "/profile",
			29 => "/trace",
			30 => "/webdav",
			_ => unreachable!("unsupported test endpoint"),
		}
	}

	fn method() -> Method {
		match ID {
			8 | 11 | 12 | 13 | 14 | 18 | 27 => Method::POST,
			21 => Method::PUT,
			29 => Method::TRACE,
			30 => Method::from_bytes(b"PROPFIND").unwrap(),
			_ => Method::GET,
		}
	}

	fn name() -> &'static str {
		match ID {
			1 => "health",
			2 => "list",
			3 => "users-detail",
			4 => "users-posts",
			5 => "post-comment",
			6 => "cluster-detail",
			7 => "users-list",
			8 => "posts-create",
			9 => "items-list",
			10 => "api-users",
			11 => "server-fn-test",
			12 => "api-users-create",
			13 => "auth-register",
			14 => "auth-login",
			15 => "users-index",
			16 => "dashboard",
			17 => "api-health",
			18 => "sub-action",
			19 => "profile",
			20 => "detail",
			21 => "edit",
			22 => "catch",
			23 => "list",
			24 => "list",
			25 => "users-list",
			26 => "items-list",
			27 => "users-create",
			28 => "!profile_detail",
			29 => "trace",
			30 => "webdav",
			_ => unreachable!("unsupported test endpoint"),
		}
	}
}

#[async_trait::async_trait]
impl<const ID: u8> Handler for TestEndpoint<ID> {
	async fn handle(&self, _req: Request) -> Result<Response> {
		Ok(Response::ok())
	}
}

struct ProtectedEndpoint;

impl EndpointInfo for ProtectedEndpoint {
	fn path() -> &'static str {
		"/items"
	}

	fn method() -> Method {
		Method::GET
	}

	fn name() -> &'static str {
		"!protected-items"
	}

	fn handler_identity() -> &'static str {
		"tests::ProtectedEndpoint"
	}

	fn auth_protection() -> reinhardt_core::endpoint::AuthProtection {
		reinhardt_core::endpoint::AuthProtection::Protected
	}

	fn guard_description() -> Option<&'static str> {
		Some("role=admin")
	}
}

#[async_trait::async_trait]
impl Handler for ProtectedEndpoint {
	async fn handle(&self, _req: Request) -> Result<Response> {
		Ok(Response::ok())
	}
}

struct ContractRawHandler;

#[async_trait::async_trait]
impl Handler for ContractRawHandler {
	async fn handle(&self, _req: Request) -> Result<Response> {
		Ok(Response::ok())
	}
}

struct ContractClassView;

#[async_trait::async_trait]
impl Handler for ContractClassView {
	async fn handle(&self, _req: Request) -> Result<Response> {
		Ok(Response::ok())
	}
}

#[cfg(feature = "viewsets")]
struct ContractViewSet;

#[cfg(feature = "viewsets")]
#[async_trait::async_trait]
impl ViewSet for ContractViewSet {
	fn get_basename(&self) -> &str {
		"contracts"
	}

	async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
		Ok(Response::ok())
	}

	fn get_extra_actions(&self) -> Vec<ActionMetadata> {
		vec![ActionMetadata::new("archive")]
	}

	fn requires_login(&self) -> bool {
		true
	}
}

#[cfg(feature = "viewsets")]
struct PermissionOnlyViewSet;

#[cfg(feature = "viewsets")]
#[async_trait::async_trait]
impl ViewSet for PermissionOnlyViewSet {
	fn get_basename(&self) -> &str {
		"permission-only"
	}

	async fn dispatch(&self, _request: Request, _action: Action) -> Result<Response> {
		Ok(Response::ok())
	}

	fn requires_login(&self) -> bool {
		true
	}

	fn get_middleware(&self) -> Option<Arc<dyn ViewSetMiddleware>> {
		Some(Arc::new(PermissionMiddleware::new(vec![
			"read".to_string(),
		])))
	}
}

struct PathParamCountHandler;

#[async_trait::async_trait]
impl Handler for PathParamCountHandler {
	async fn handle(&self, req: Request) -> Result<Response> {
		Ok(Response::ok().with_body(req.path_params.len().to_string()))
	}
}

struct PathParamCountSyncHandler;

impl SyncHandler for PathParamCountSyncHandler {
	fn handle_sync(&self, req: Request) -> Result<Response> {
		Ok(Response::ok().with_body(req.path_params.len().to_string()))
	}
}

#[rstest]
fn test_new_router() {
	// Arrange & Act
	let router = ServerRouter::new();

	// Assert
	assert_eq!(router.prefix(), "");
	assert_eq!(router.namespace(), None);
	assert_eq!(router.children_count(), 0);
}

#[rstest]
fn test_with_prefix() {
	// Arrange & Act
	let router = ServerRouter::new().with_prefix("/api/v1");

	// Assert
	assert_eq!(router.prefix(), "/api/v1");
}

#[rstest]
fn test_with_namespace() {
	// Arrange & Act
	let router = ServerRouter::new().with_namespace("v1");

	// Assert
	assert_eq!(router.namespace(), Some("v1"));
}

#[rstest]
fn test_mount() {
	// Arrange
	let child = ServerRouter::new();

	// Act
	let router = ServerRouter::new().mount("/users/", child);

	// Assert
	assert_eq!(router.children_count(), 1);
}

#[rstest]
#[should_panic(expected = "path parameter placeholder")]
fn test_mount_panics_on_param_prefix() {
	// Arrange
	let child = ServerRouter::new();

	// Act
	// Mounting with a `{param}` placeholder in the prefix is not supported
	// and must panic at construction time.
	let _ = ServerRouter::new().mount("/orgs/{org}/clusters/", child);

	// Assert: handled by `#[should_panic]`.
}

#[rstest]
fn test_mount_inherits_di_context() {
	// Arrange
	let di_ctx =
		Arc::new(InjectionContext::builder(Arc::new(reinhardt_di::SingletonScope::new())).build());
	let child = ServerRouter::new();

	// Act
	let router = ServerRouter::new()
		.with_di_context(di_ctx.clone())
		.mount("/users/", child);

	// Assert
	assert!(router.di_context.is_some());
	assert_eq!(router.children_count(), 1);
}

#[rstest]
fn test_group() {
	// Arrange
	let users = ServerRouter::new().with_prefix("/users");
	let posts = ServerRouter::new().with_prefix("/posts");

	// Act
	let router = ServerRouter::new().group(vec![users, posts]);

	// Assert
	assert_eq!(router.children_count(), 2);
}

#[rstest]
fn test_get_all_routes() {
	// Arrange
	let router = ServerRouter::new()
		.with_prefix("/api")
		.with_namespace("api");

	// Act
	let routes = router.get_all_routes();

	// Assert
	assert_eq!(routes.len(), 0);
}

#[rstest]
fn test_get_all_routes_strips_optout_sigil_from_endpoint_name() {
	// Arrange
	let router = ServerRouter::new().endpoint(|| TestEndpoint::<28>);

	// Act
	let routes = router.get_all_routes();

	// Assert
	assert_eq!(routes.len(), 1);
	assert_eq!(routes[0].1.as_deref(), Some("profile_detail"));
}

#[rstest]
fn test_get_full_namespace_no_parent() {
	// Arrange
	let router = ServerRouter::new().with_namespace("users");

	// Act & Assert
	assert_eq!(router.get_full_namespace(None), Some("users".to_string()));
}

#[rstest]
fn test_get_full_namespace_with_parent() {
	// Arrange
	let router = ServerRouter::new().with_namespace("users");

	// Act & Assert
	assert_eq!(
		router.get_full_namespace(Some("v1")),
		Some("v1:users".to_string())
	);
}

#[rstest]
fn test_get_full_namespace_no_namespace() {
	// Arrange
	let router = ServerRouter::new();

	// Act & Assert
	assert_eq!(
		router.get_full_namespace(Some("v1")),
		Some("v1".to_string())
	);
	assert_eq!(router.get_full_namespace(None), None);
}

#[rstest]
fn test_hierarchical_namespace() {
	// Arrange
	let child = ServerRouter::new().with_namespace("users");

	// Act
	let parent = ServerRouter::new()
		.with_namespace("v1")
		.mount("/users/", child);

	// Assert
	assert_eq!(parent.namespace(), Some("v1"));
	assert_eq!(parent.children_count(), 1);
}

#[rstest]
fn test_register_all_routes_with_namespace() {
	// Arrange
	let mut router = ServerRouter::new()
		.with_namespace("api")
		.endpoint(|| TestEndpoint::<1>);

	// Act
	let errors = router.register_all_routes();
	assert!(errors.is_empty());

	// Assert
	let url = router.reverse("api:health", &[]);
	assert!(url.is_some());
	assert_eq!(url.unwrap(), "/health");
}

#[rstest]
fn test_nested_namespace_registration() {
	// Arrange
	let users = ServerRouter::new()
		.with_namespace("users")
		.endpoint(|| TestEndpoint::<2>);

	let mut api = ServerRouter::new()
		.with_namespace("v1")
		.with_prefix("/api/v1")
		.mount("/users/", users);

	// Act
	let errors = api.register_all_routes();
	assert!(errors.is_empty());

	// Assert
	let url = api.reverse("v1:users:list", &[]);
	assert!(url.is_some());
	assert_eq!(url.unwrap(), "/api/v1/users/list");
}

#[rstest]
fn test_mount_prefix_inheritance() {
	// Arrange
	let child = ServerRouter::new();

	// Act
	let parent = ServerRouter::new().with_prefix("/api").mount("/v1/", child);

	// Assert
	assert_eq!(parent.children_count(), 1);
}

#[rstest]
fn test_multiple_child_routers() {
	// Arrange
	let users = ServerRouter::new().with_namespace("users");
	let posts = ServerRouter::new().with_namespace("posts");
	let comments = ServerRouter::new().with_namespace("comments");

	// Act
	let router = ServerRouter::new()
		.mount("/users/", users)
		.mount("/posts/", posts)
		.mount("/comments/", comments);

	// Assert
	assert_eq!(router.children_count(), 3);
}

#[rstest]
fn test_deep_nesting() {
	// Arrange
	let resource = ServerRouter::new().with_namespace("resource");
	let v2 = ServerRouter::new()
		.with_namespace("v2")
		.mount("/resource/", resource);
	let v1 = ServerRouter::new().with_namespace("v1").mount("/v2/", v2);

	// Act
	let api = ServerRouter::new().with_namespace("api").mount("/v1/", v1);

	// Assert
	assert_eq!(api.children_count(), 1);
}

#[tokio::test]
async fn test_route_matching_correctness() {
	// Arrange
	let router = ServerRouter::new()
		.endpoint(|| TestEndpoint::<3>)
		.endpoint(|| TestEndpoint::<4>)
		.endpoint(|| TestEndpoint::<5>);
	router.compile_routes();

	// Act & Assert - exact path matching
	let result = router.match_own_routes("/users/123", &Method::GET);
	assert!(result.is_some());
	assert_eq!(result.unwrap().param("id"), Some("123"));

	// Act & Assert - nested path matching
	let result = router.match_own_routes("/users/456/posts", &Method::GET);
	assert!(result.is_some());
	assert_eq!(result.unwrap().param("id"), Some("456"));

	// Act & Assert - multiple parameters; verify both values AND
	// declaration order (post_id appears before comment_id in the URL).
	let route_match = router.match_own_routes("/posts/789/comments/101", &Method::GET);
	let route_match = route_match.unwrap();
	assert_eq!(route_match.param("post_id"), Some("789"));
	assert_eq!(route_match.param("comment_id"), Some("101"));
	assert_eq!(
		route_match
			.params
			.as_ref()
			.expect("parameterized route should expose path params")
			.to_vec(),
		vec![
			("post_id".to_string(), "789".to_string()),
			("comment_id".to_string(), "101".to_string()),
		],
		"path params must be stored in URL pattern declaration order (issue #4013)"
	);

	// Act & Assert - non-matching route
	let result = router.match_own_routes("/nonexistent", &Method::GET);
	assert!(result.is_none());
}

#[test]
fn test_compile_routes_populates_exact_static_route_table() {
	// Arrange
	let router = ServerRouter::new()
		.endpoint(|| TestEndpoint::<1>)
		.endpoint(|| TestEndpoint::<3>);

	// Act
	router.compile_routes();
	let compiled = router.compiled_routes();

	// Assert
	assert!(
		compiled
			.exact_for_method(&Method::GET)
			.expect("GET exact routes should be available")
			.contains_key("/health")
	);
	assert!(
		!compiled
			.exact_for_method(&Method::GET)
			.expect("GET exact routes should be available")
			.contains_key("/users/{id}")
	);
	assert!(
		compiled
			.router_for_method(&Method::GET)
			.expect("GET router should be available")
			.at("/users/123")
			.is_ok()
	);
}

#[test]
fn test_compile_routes_skips_exact_table_for_escaped_static_routes() {
	// Arrange
	let router = ServerRouter::new().handler("/{{hello}}", PathParamCountHandler);

	// Act
	router.compile_routes();
	let compiled = router.compiled_routes();

	// Assert
	assert!(
		!compiled
			.exact_for_method(&Method::GET)
			.expect("GET exact routes should be available")
			.contains_key("/{{hello}}")
	);
	assert!(
		compiled
			.router_for_method(&Method::GET)
			.expect("GET router should be available")
			.at("/{hello}")
			.is_ok()
	);
}

#[tokio::test]
async fn test_route_matching_preserves_url_pattern_order_issue_4013() {
	// Regression test for issue #4013: path parameters must be exposed in
	// URL pattern declaration order (not alphabetical), so that tuple
	// extractors `Path<(T1, T2)>` populate fields by position.

	// Arrange: alphabetical order would put `cluster_id` before `org`,
	// but URL declaration order is `org` first, `cluster_id` second.
	let router = ServerRouter::new().endpoint(|| TestEndpoint::<6>);
	router.compile_routes();

	// Act
	let route_match = router
		.match_own_routes("/orgs/myslug/clusters/5/", &Method::GET)
		.expect("route should match");

	// Assert
	assert_eq!(
		route_match
			.params
			.as_ref()
			.expect("parameterized route should expose path params")
			.to_vec(),
		vec![
			("org".to_string(), "myslug".to_string()),
			("cluster_id".to_string(), "5".to_string()),
		],
		"matched params must follow URL declaration order (issue #4013)"
	);
}

#[tokio::test]
async fn test_route_matching_different_methods() {
	// Arrange
	let router = ServerRouter::new()
		.endpoint(|| TestEndpoint::<7>)
		.endpoint(|| TestEndpoint::<27>);
	router.compile_routes();

	// Act & Assert - GET method
	let result = router.match_own_routes("/users", &Method::GET);
	assert!(result.is_some());

	// Act & Assert - POST method
	let result = router.match_own_routes("/users", &Method::POST);
	assert!(result.is_some());

	// Act & Assert - unsupported method
	let result = router.match_own_routes("/users", &Method::DELETE);
	assert!(result.is_none());
}

#[rstest]
fn test_unsupported_methods_do_not_fall_back_to_get_routes() {
	// Arrange
	let router = ServerRouter::new().endpoint(|| TestEndpoint::<1>);
	router.compile_routes();
	let trace = Method::from_bytes(b"TRACE").unwrap();
	let custom = Method::from_bytes(b"BREW").unwrap();

	// Act & Assert
	assert!(router.match_own_routes("/health", &trace).is_none());
	assert!(router.match_own_routes("/health", &custom).is_none());
	assert!(router.path_exists_for_any_method("/health"));
}

#[rstest]
fn test_registered_non_default_methods_resolve() {
	// Arrange
	let router = ServerRouter::new()
		.endpoint(|| TestEndpoint::<29>)
		.endpoint(|| TestEndpoint::<30>);
	router.compile_routes();
	let propfind = Method::from_bytes(b"PROPFIND").unwrap();

	// Act & Assert
	assert!(router.match_own_routes("/trace", &Method::TRACE).is_some());
	assert!(router.match_own_routes("/webdav", &propfind).is_some());
	assert!(router.match_own_routes("/trace", &Method::GET).is_none());
	assert!(router.path_exists_for_any_method("/webdav"));
}

#[rstest]
fn test_validate_routes_success() {
	// Arrange
	let router = ServerRouter::new()
		.endpoint(|| TestEndpoint::<3>)
		.endpoint(|| TestEndpoint::<8>);

	// Act
	let result = router.validate_routes();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn test_compile_routes_returns_errors_for_duplicate_routes() {
	// Arrange - register duplicate paths for the same method
	let router = ServerRouter::new()
		.endpoint(|| TestEndpoint::<7>)
		.endpoint(|| TestEndpoint::<25>);

	// Act
	let errors = router.compile_routes();

	// Assert - matchit should report a conflict for duplicate routes
	assert!(!errors.is_empty());
	assert!(errors[0].contains("Failed to compile route"));
}

#[rstest]
fn test_validate_routes_returns_errors_for_invalid_patterns() {
	// Arrange - duplicate routes cause matchit compilation errors
	let router = ServerRouter::new()
		.endpoint(|| TestEndpoint::<9>)
		.endpoint(|| TestEndpoint::<26>);

	// Act
	let result = router.validate_routes();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(!errors.is_empty());
}

#[test]
fn test_validate_routes_checks_mounted_child_routes() {
	let child = ServerRouter::new()
		.endpoint(|| TestEndpoint::<7>)
		.endpoint(|| TestEndpoint::<25>);
	let router = ServerRouter::new().mount("/nested/", child);

	let errors = router
		.validate_routes()
		.expect_err("child route should fail validation");

	assert!(
		errors
			.iter()
			.any(|error| error.contains("Failed to compile route"))
	);
}

#[rstest]
fn test_router_reuses_compiled_routes() {
	// Arrange
	let router = ServerRouter::new().endpoint(|| TestEndpoint::<1>);

	// Act
	let first_errors = router.compile_routes();
	let second_errors = router.compile_routes();
	let first_table = router.compiled_routes() as *const _;
	let second_table = router.compiled_routes() as *const _;

	// Assert
	assert!(first_errors.is_empty());
	assert!(second_errors.is_empty());
	assert_eq!(first_table, second_table);
	let result = router.match_own_routes("/health", &Method::GET);
	assert!(result.is_some());
}

#[rstest]
fn test_router_rebuilds_compiled_routes_after_endpoint_registration() {
	// Arrange
	let router = ServerRouter::new().endpoint(|| TestEndpoint::<1>);
	router.compile_routes();
	assert!(router.match_own_routes("/list", &Method::GET).is_none());

	// Act
	let router = router.endpoint(|| TestEndpoint::<2>);

	// Assert
	assert!(router.match_own_routes("/health", &Method::GET).is_some());
	assert!(router.match_own_routes("/list", &Method::GET).is_some());
}

// --- ServerRouter::exclude() tests ---

// Simple no-op middleware for testing exclude()
struct NoopMiddleware;

#[async_trait::async_trait]
impl Middleware for NoopMiddleware {
	async fn process(
		&self,
		request: reinhardt_http::Request,
		next: std::sync::Arc<dyn reinhardt_http::Handler>,
	) -> reinhardt_http::Result<reinhardt_http::Response> {
		next.handle(request).await
	}
}

fn create_test_request(path: &str) -> reinhardt_http::Request {
	reinhardt_http::Request::builder()
		.method(Method::GET)
		.uri(path)
		.version(hyper::Version::HTTP_11)
		.headers(hyper::HeaderMap::new())
		.body(bytes::Bytes::new())
		.build()
		.unwrap()
}

fn create_test_request_with_path_params(path: &str) -> reinhardt_http::Request {
	reinhardt_http::Request::builder()
		.method(Method::GET)
		.uri(path)
		.version(hyper::Version::HTTP_11)
		.headers(hyper::HeaderMap::new())
		.body(bytes::Bytes::new())
		.path_params(vec![("id".to_string(), "stale".to_string())])
		.build()
		.unwrap()
}

#[rstest]
#[tokio::test]
async fn static_route_dispatch_clears_existing_path_params() {
	// Arrange
	let router = ServerRouter::new().handler("/health", PathParamCountHandler);
	let request = create_test_request_with_path_params("/health");

	// Act
	let response = Handler::handle(&router, request).await.unwrap();

	// Assert
	assert_eq!(response.body, bytes::Bytes::from_static(b"0"));
}

#[rstest]
fn static_route_sync_dispatch_clears_existing_path_params() {
	// Arrange
	let router = ServerRouter::new().handler_sync("/health", PathParamCountSyncHandler);
	let request = create_test_request_with_path_params("/health");

	// Act
	let response = router
		.try_dispatch_sync(request)
		.expect("static sync route should use sync fast path")
		.unwrap();

	// Assert
	assert_eq!(response.body, bytes::Bytes::from_static(b"0"));
}

#[rstest]
fn escaped_literal_brace_route_resolves_without_path_params() {
	// Arrange
	let router = ServerRouter::new().handler("/{{hello}}", PathParamCountHandler);

	// Act
	let route_match = router
		.resolve("/{hello}", &Method::GET)
		.expect("literal brace route should resolve");

	// Assert
	assert!(route_match.params.is_none());
	assert!(router.resolve("/{{hello}}", &Method::GET).is_none());
}

#[rstest]
fn test_server_router_exclude_stores_exclusion() {
	// Arrange & Act
	let router = ServerRouter::new()
		.with_middleware(NoopMiddleware)
		.exclude("/api/auth/")
		.exclude("/health");

	// Assert
	assert_eq!(router.middleware_exclusions.len(), 1);
	assert_eq!(router.middleware_exclusions[0].len(), 2);
	assert_eq!(router.middleware_exclusions[0][0], "/api/auth/");
	assert_eq!(router.middleware_exclusions[0][1], "/health");
}

#[rstest]
fn test_server_router_exclude_only_affects_last_middleware() {
	// Arrange & Act
	let router = ServerRouter::new()
		.with_middleware(NoopMiddleware)
		.exclude("/admin/")
		.with_middleware(NoopMiddleware)
		.exclude("/api/auth/");

	// Assert
	assert_eq!(router.middleware_exclusions.len(), 2);
	assert_eq!(router.middleware_exclusions[0], vec!["/admin/"]);
	assert_eq!(router.middleware_exclusions[1], vec!["/api/auth/"]);
}

#[rstest]
#[should_panic(expected = "exclude() called with no middleware")]
fn test_server_router_exclude_panics_without_middleware() {
	// Arrange & Act & Assert
	let _router = ServerRouter::new().exclude("/api/auth/");
}

#[rstest]
fn test_server_router_build_middleware_with_exclusions() {
	// Arrange
	let router = ServerRouter::new()
		.with_middleware(NoopMiddleware)
		.exclude("/admin/")
		.with_middleware(NoopMiddleware);

	// Act
	let built = router.build_middleware_with_exclusions();

	// Assert
	assert_eq!(built.len(), 2);

	let request_admin = create_test_request("/admin/dashboard");
	let request_public = create_test_request("/public");

	// First middleware (with exclusion) skips /admin/
	assert!(!built[0].should_continue(&request_admin));
	assert!(built[0].should_continue(&request_public));
	// Second middleware (no exclusion) runs for all
	assert!(built[1].should_continue(&request_admin));
	assert!(built[1].should_continue(&request_public));
}

// --- Framework-level 404/405 middleware tests (#3234) ---

// Middleware that adds a security header to responses
struct SecurityHeaderTestMiddleware;

#[async_trait::async_trait]
impl Middleware for SecurityHeaderTestMiddleware {
	async fn process(
		&self,
		request: reinhardt_http::Request,
		next: std::sync::Arc<dyn reinhardt_http::Handler>,
	) -> reinhardt_http::Result<reinhardt_http::Response> {
		let mut response = next.handle(request).await?;
		response.headers.insert(
			hyper::header::HeaderName::from_static("x-security-test"),
			hyper::header::HeaderValue::from_static("applied"),
		);
		Ok(response)
	}
}

#[rstest]
#[tokio::test]
async fn test_404_response_gets_middleware_headers() {
	// Arrange: router with middleware and a registered route
	let router = ServerRouter::new()
		.with_middleware(SecurityHeaderTestMiddleware)
		.endpoint(|| TestEndpoint::<10>);

	// Act: request a non-existent path
	let request = create_test_request("/nonexistent");
	let response = Handler::handle(&router, request).await.unwrap();

	// Assert: 404 response has security header from middleware
	assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
	assert_eq!(
		response
			.headers
			.get("x-security-test")
			.map(|v| v.to_str().unwrap()),
		Some("applied"),
		"Framework-level 404 response should have middleware security header"
	);
}

#[rstest]
#[tokio::test]
async fn test_405_response_gets_middleware_headers() {
	// Arrange: router with middleware and a GET-only route
	let router = ServerRouter::new()
		.with_middleware(SecurityHeaderTestMiddleware)
		.endpoint(|| TestEndpoint::<10>);

	// Act: send POST to a GET-only route
	let request = reinhardt_http::Request::builder()
		.method(Method::POST)
		.uri("/api/users/")
		.version(hyper::Version::HTTP_11)
		.headers(hyper::HeaderMap::new())
		.body(bytes::Bytes::new())
		.build()
		.unwrap();
	let response = Handler::handle(&router, request).await.unwrap();

	// Assert: 405 response has security header from middleware
	assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
	assert_eq!(
		response
			.headers
			.get("x-security-test")
			.map(|v| v.to_str().unwrap()),
		Some("applied"),
		"Framework-level 405 response should have middleware security header"
	);
}

#[rstest]
#[tokio::test]
async fn test_404_without_middleware_returns_error() {
	// Arrange: router with no middleware
	let router = ServerRouter::new().endpoint(|| TestEndpoint::<10>);

	// Act: request a non-existent path
	let request = create_test_request("/nonexistent");
	let result = Handler::handle(&router, request).await;

	// Assert: returns Err (not wrapped in middleware chain)
	assert!(result.is_err(), "404 without middleware should return Err");
}

#[rstest]
#[tokio::test]
async fn test_404_respects_middleware_exclusions() {
	// Arrange: router with middleware excluded for /admin/
	let router = ServerRouter::new()
		.with_middleware(SecurityHeaderTestMiddleware)
		.exclude("/admin/")
		.endpoint(|| TestEndpoint::<10>);

	// Act: request non-existent path under excluded prefix
	let request = create_test_request("/admin/nonexistent");
	let response = Handler::handle(&router, request).await.unwrap();

	// Assert: 404 response but security header absent (middleware excluded)
	assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
	assert!(
		response.headers.get("x-security-test").is_none(),
		"404 under excluded path should NOT have middleware security header"
	);
}

// --- Prefix double-application fix tests (#3407, #3408) ---

#[rstest]
#[tokio::test]
async fn test_endpoint_route_with_prefix_strips_prefix_during_compilation() {
	// Arrange: register a route whose path already contains the prefix,
	// simulating server function registration (e.g., ServerFnRegistration::PATH)
	let router = ServerRouter::new()
		.with_prefix("/api")
		.endpoint(|| TestEndpoint::<11>);

	// Act: resolve the full path (resolve() strips "/api" before matchit lookup)
	let result = router.resolve("/api/server_fn/test", &Method::POST);

	// Assert: route matches without double-prefix issue
	assert!(
		result.is_some(),
		"POST /api/server_fn/test should match when router has prefix /api"
	);
}

#[rstest]
#[tokio::test]
async fn test_endpoint_route_post_with_prefix_no_405() {
	// Arrange: register a POST route with a path that includes the prefix
	let router = ServerRouter::new()
		.with_prefix("/api")
		.endpoint(|| TestEndpoint::<12>);

	// Act: resolve POST request (verifies no 405 Method Not Allowed)
	let result = router.resolve("/api/users", &Method::POST);

	// Assert: POST route is reachable
	assert!(
		result.is_some(),
		"POST /api/users should match when router has prefix /api (no 405)"
	);

	// Also verify GET returns None (route is POST-only)
	let get_result = router.resolve("/api/users", &Method::GET);
	assert!(
		get_result.is_none(),
		"GET /api/users should not match a POST-only route"
	);
}

#[rstest]
#[tokio::test]
async fn test_endpoint_route_without_prefix_overlap_still_works() {
	// Arrange: route path does not start with the prefix
	let router = ServerRouter::new()
		.with_prefix("/api")
		.endpoint(|| TestEndpoint::<1>);

	// Act: resolve a path under the prefix
	let result = router.resolve("/api/health", &Method::GET);

	// Assert: route matches (path kept as-is since it does not start with prefix)
	assert!(
		result.is_some(),
		"/api/health should match /health route under /api prefix"
	);
}

// --- Leading slash normalization fix tests (#3419) ---
//
// strip_prefix_normalized: unit tests (normal / edge / error)

#[rstest]
// Normal: trailing-slash prefix strips correctly
#[case("/api/", "/api/auth/register/", "/auth/register/")]
// Normal: non-trailing-slash prefix strips correctly
#[case("/api", "/api/auth/register/", "/auth/register/")]
// Normal: prefix equals full path → root "/"
#[case("/api/", "/api/", "/")]
#[case("/api", "/api", "/")]
// Normal: single-segment after strip
#[case("/api/", "/api/health", "/health")]
#[case("/v1/", "/v1/users/", "/users/")]
// Edge: empty prefix returns path as-is
#[case("", "/anything", "/anything")]
#[case("", "/", "/")]
#[case("", "/a/b/c", "/a/b/c")]
// Edge: prefix is "/" — remainder loses leading slash, must be restored
#[case("/", "/health", "/health")]
#[case("/", "/a/b/c", "/a/b/c")]
// Edge: long multi-segment prefix
#[case("/api/v2/internal/", "/api/v2/internal/metrics", "/metrics")]
// Edge: path with URL-encoded segments
#[case("/api/", "/api/users%2F123/", "/users%2F123/")]
// Edge: path with hyphens and underscores
#[case("/api/", "/api/my-resource/sub_path/", "/my-resource/sub_path/")]
fn test_strip_prefix_normalized(#[case] prefix: &str, #[case] path: &str, #[case] expected: &str) {
	// Act
	let result = ServerRouter::strip_prefix_normalized(prefix, path);

	// Assert
	assert!(
		result.is_some(),
		"strip_prefix_normalized({prefix:?}, {path:?}) should return Some"
	);
	let normalized = result.unwrap();
	assert_eq!(
		normalized.as_ref(),
		expected,
		"strip_prefix_normalized({prefix:?}, {path:?})"
	);
}

#[rstest]
// Error: path doesn't start with prefix at all
#[case("/api/", "/web/page")]
#[case("/api", "/web/page")]
// Error: partial prefix match (not a real prefix)
#[case("/api/", "/ap")]
#[case("/api", "/ap")]
// Error: path is empty
#[case("/api/", "")]
#[case("/", "")]
// Error: prefix longer than path
#[case("/api/v2/", "/api/")]
fn test_strip_prefix_normalized_returns_none(#[case] prefix: &str, #[case] path: &str) {
	// Act
	let result = ServerRouter::strip_prefix_normalized(prefix, path);

	// Assert
	assert!(
		result.is_none(),
		"strip_prefix_normalized({prefix:?}, {path:?}) should return None"
	);
}

#[rstest]
fn test_strip_prefix_normalized_result_always_starts_with_slash() {
	// Arrange: various prefix/path combos that should succeed
	let cases = [
		("/api/", "/api/x"),
		("/a/b/c/", "/a/b/c/d"),
		("/", "/x"),
		("", "/x"),
		("/prefix/", "/prefix/rest/of/path"),
	];

	for (prefix, path) in cases {
		// Act
		let result = ServerRouter::strip_prefix_normalized(prefix, path);

		// Assert
		let normalized = result.unwrap();
		assert!(
			normalized.starts_with('/'),
			"result for ({prefix:?}, {path:?}) should start with '/' but got {normalized:?}"
		);
	}
}

// resolve(): normal cases with child routers

#[rstest]
#[tokio::test]
async fn test_resolve_trailing_slash_prefix_child_router_matches() {
	// Arrange: parent with trailing-slash prefix, child with its own prefix
	let child = ServerRouter::new()
		.with_prefix("/auth/")
		.endpoint(|| TestEndpoint::<13>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/auth/", child);

	// Act
	let result = parent.resolve("/api/auth/register/", &Method::POST);

	// Assert
	assert!(
		result.is_some(),
		"POST /api/auth/register/ should match child route through trailing-slash prefix"
	);
}

#[rstest]
#[tokio::test]
async fn test_resolve_no_trailing_slash_with_prefix_child_router_matches() {
	// Arrange: parent with_prefix (no trailing slash) + child mounted with trailing slash
	// Note: mount() requires trailing-slash prefix (Django convention),
	// but with_prefix() allows non-trailing-slash prefix
	let child = ServerRouter::new()
		.with_prefix("/auth/")
		.endpoint(|| TestEndpoint::<14>);
	let parent = ServerRouter::new()
		.with_prefix("/api")
		.mount("/auth/", child);

	// Act
	let result = parent.resolve("/api/auth/login/", &Method::POST);

	// Assert
	assert!(
		result.is_some(),
		"POST /api/auth/login/ should match child route with non-trailing-slash parent prefix"
	);
}

#[rstest]
#[tokio::test]
async fn test_resolve_multiple_children_with_trailing_slash_prefix() {
	// Arrange: parent with trailing-slash prefix, multiple children
	let auth = ServerRouter::new()
		.with_prefix("/auth/")
		.endpoint(|| TestEndpoint::<14>);
	let users = ServerRouter::new()
		.with_prefix("/users/")
		.endpoint(|| TestEndpoint::<15>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/auth/", auth)
		.mount("/users/", users);

	// Act & Assert: both children should be reachable
	assert!(
		parent.resolve("/api/auth/login/", &Method::POST).is_some(),
		"POST /api/auth/login/ should match auth child"
	);
	assert!(
		parent.resolve("/api/users/", &Method::GET).is_some(),
		"GET /api/users/ should match users child"
	);
}

#[rstest]
#[tokio::test]
async fn test_resolve_child_root_route_with_trailing_slash_prefix() {
	// Arrange: child's own root route (prefix stripped → "/")
	let child = ServerRouter::new()
		.with_prefix("/dashboard/")
		.endpoint(|| TestEndpoint::<16>);
	let parent = ServerRouter::new()
		.with_prefix("/app/")
		.mount("/dashboard/", child);

	// Act
	let result = parent.resolve("/app/dashboard/", &Method::GET);

	// Assert
	assert!(
		result.is_some(),
		"GET /app/dashboard/ should match child root route"
	);
}

#[rstest]
#[tokio::test]
async fn test_resolve_parent_own_route_still_works_with_trailing_slash_prefix() {
	// Arrange: parent has both own routes and children
	let child = ServerRouter::new()
		.with_prefix("/sub/")
		.endpoint(|| TestEndpoint::<18>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.endpoint(|| TestEndpoint::<17>)
		.mount("/sub/", child);

	// Act & Assert
	assert!(
		parent.resolve("/api/health", &Method::GET).is_some(),
		"Parent's own route should still work"
	);
	assert!(
		parent.resolve("/api/sub/action/", &Method::POST).is_some(),
		"Child route should also work"
	);
}

// resolve(): deep nesting

#[rstest]
#[tokio::test]
async fn test_resolve_deeply_nested_trailing_slash_prefixes() {
	// Arrange: 3 levels of trailing-slash prefixes
	let grandchild = ServerRouter::new()
		.with_prefix("/profile/")
		.endpoint(|| TestEndpoint::<19>);
	let child = ServerRouter::new()
		.with_prefix("/users/")
		.mount("/profile/", grandchild);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/users/", child);

	// Act
	let result = parent.resolve("/api/users/profile/", &Method::GET);

	// Assert
	assert!(
		result.is_some(),
		"GET /api/users/profile/ should match through 3 levels of trailing-slash prefix stripping"
	);
}

#[rstest]
#[tokio::test]
async fn test_resolve_mixed_trailing_and_non_trailing_slash_nesting() {
	// Arrange: with_prefix uses non-trailing slash, mount uses trailing slash
	let grandchild = ServerRouter::new()
		.with_prefix("/detail")
		.endpoint(|| TestEndpoint::<20>);
	let child = ServerRouter::new()
		.with_prefix("/items/")
		.mount("/detail/", grandchild);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/items/", child);

	// Act
	let result = parent.resolve("/api/items/detail/", &Method::GET);

	// Assert
	assert!(
		result.is_some(),
		"Mixed trailing/non-trailing prefix nesting should resolve correctly"
	);
}

// resolve(): error cases (should return None)

#[rstest]
#[tokio::test]
async fn test_resolve_path_not_matching_parent_prefix() {
	// Arrange
	let child = ServerRouter::new()
		.with_prefix("/auth/")
		.endpoint(|| TestEndpoint::<14>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/auth/", child);

	// Act
	let result = parent.resolve("/web/auth/login/", &Method::POST);

	// Assert
	assert!(
		result.is_none(),
		"Path not matching parent prefix should return None"
	);
}

#[rstest]
#[tokio::test]
async fn test_resolve_path_matches_parent_but_not_child() {
	// Arrange
	let child = ServerRouter::new()
		.with_prefix("/auth/")
		.endpoint(|| TestEndpoint::<14>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/auth/", child);

	// Act: path under parent prefix but doesn't match any child
	let result = parent.resolve("/api/unknown/path/", &Method::GET);

	// Assert
	assert!(
		result.is_none(),
		"Path matching parent but not child should return None"
	);
}

#[rstest]
#[tokio::test]
async fn test_resolve_wrong_method_through_child_with_trailing_slash_prefix() {
	// Arrange: child only has POST route
	let child = ServerRouter::new()
		.with_prefix("/auth/")
		.endpoint(|| TestEndpoint::<14>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/auth/", child);

	// Act: try GET instead of POST
	let result = parent.resolve("/api/auth/login/", &Method::GET);

	// Assert
	assert!(
		result.is_none(),
		"Wrong HTTP method through child router should return None"
	);
}

// path_exists_for_any_method(): normal / error / edge

#[rstest]
#[tokio::test]
async fn test_path_exists_with_trailing_slash_prefix_and_child() {
	// Arrange
	let child = ServerRouter::new()
		.with_prefix("/users/")
		.endpoint(|| TestEndpoint::<15>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/users/", child);

	// Act
	let exists = parent.path_exists_for_any_method("/api/users/");

	// Assert
	assert!(
		exists,
		"path_exists_for_any_method should find path in child router after prefix normalization"
	);
}

#[rstest]
#[tokio::test]
async fn test_path_exists_nonexistent_path_with_trailing_slash_prefix() {
	// Arrange
	let child = ServerRouter::new()
		.with_prefix("/users/")
		.endpoint(|| TestEndpoint::<15>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/users/", child);

	// Act
	let exists = parent.path_exists_for_any_method("/api/nonexistent/");

	// Assert
	assert!(
		!exists,
		"path_exists_for_any_method should return false for nonexistent path"
	);
}

#[rstest]
#[tokio::test]
async fn test_path_exists_wrong_prefix_returns_false() {
	// Arrange
	let child = ServerRouter::new()
		.with_prefix("/users/")
		.endpoint(|| TestEndpoint::<15>);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/users/", child);

	// Act
	let exists = parent.path_exists_for_any_method("/web/users/");

	// Assert
	assert!(
		!exists,
		"path_exists_for_any_method with wrong parent prefix should return false"
	);
}

#[rstest]
#[tokio::test]
async fn test_path_exists_deeply_nested_with_trailing_slash_prefix() {
	// Arrange: 3-level nesting
	let grandchild = ServerRouter::new()
		.with_prefix("/edit/")
		.endpoint(|| TestEndpoint::<21>);
	let child = ServerRouter::new()
		.with_prefix("/items/")
		.mount("/edit/", grandchild);
	let parent = ServerRouter::new()
		.with_prefix("/api/")
		.mount("/items/", child);

	// Act
	let exists = parent.path_exists_for_any_method("/api/items/edit/");

	// Assert
	assert!(
		exists,
		"path_exists_for_any_method should find deeply nested path through trailing-slash prefixes"
	);
}

// Edge cases: compile_routes with trailing-slash prefix

#[rstest]
#[tokio::test]
async fn test_endpoint_route_with_trailing_slash_prefix_compiles_correctly() {
	// Arrange: route path includes prefix with trailing slash
	let router = ServerRouter::new()
		.with_prefix("/api/")
		.endpoint(|| TestEndpoint::<11>);

	// Act
	let result = router.resolve("/api/server_fn/test", &Method::POST);

	// Assert
	assert!(
		result.is_some(),
		"Route with trailing-slash prefix should compile and resolve correctly"
	);
}

#[rstest]
#[should_panic(expected = "URL route prefix cannot be an empty string")]
fn test_mount_with_empty_prefix_panics() {
	// Arrange & Act: mounting with empty prefix should panic
	let child = ServerRouter::new().endpoint(|| TestEndpoint::<22>);
	let _parent = ServerRouter::new().with_prefix("/api/").mount("", child);
}

#[rstest]
#[tokio::test]
async fn test_resolve_child_with_slash_prefix_under_trailing_slash_parent() {
	// Arrange: child router with "/" prefix under parent with trailing-slash prefix
	let child = ServerRouter::new()
		.with_prefix("/")
		.endpoint(|| TestEndpoint::<22>);
	let parent = ServerRouter::new().with_prefix("/api/").mount("/", child);

	// Act
	let result = parent.resolve("/api/catch/", &Method::GET);

	// Assert
	assert!(
		result.is_some(),
		"Child with '/' prefix under trailing-slash parent should match"
	);
}

// ===================================================================
// Duplicate route name detection tests (Issue #3462)
// ===================================================================

#[rstest]
fn test_register_all_routes_detects_duplicate_names() {
	// Arrange — two routes with the same name in the same router
	let mut router = ServerRouter::new()
		.with_namespace("api")
		.endpoint(|| TestEndpoint::<23>)
		.endpoint(|| TestEndpoint::<24>);

	// Act
	let errors = router.register_all_routes();

	// Assert
	assert_eq!(errors.len(), 1);
	assert!(errors[0].contains("Duplicate route name 'api:list'"));
}

#[rstest]
fn test_validate_route_names_succeeds_with_unique_names() {
	// Arrange
	let router = ServerRouter::new()
		.with_namespace("api")
		.endpoint(|| TestEndpoint::<25>)
		.endpoint(|| TestEndpoint::<26>);

	// Act
	let result = router.validate_route_names();

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn test_validate_routes_includes_name_errors() {
	// Arrange — duplicate name
	let router = ServerRouter::new()
		.with_namespace("api")
		.endpoint(|| TestEndpoint::<23>)
		.endpoint(|| TestEndpoint::<24>);

	// Act
	let result = router.validate_routes();

	// Assert
	assert!(result.is_err());
	let errors = result.unwrap_err();
	assert!(errors.iter().any(|e| e.contains("Duplicate route name")));
}

#[test]
fn mounted_contract_includes_each_mount_with_endpoint_metadata() {
	let api =
		ServerRouter::new().mount("/api/", ServerRouter::new().endpoint(|| ProtectedEndpoint));
	let router = api.mount(
		"/internal/",
		ServerRouter::new().endpoint(|| ProtectedEndpoint),
	);

	let contracts = router.get_mounted_route_contracts().unwrap();
	let routes: Vec<_> = contracts
		.into_iter()
		.map(|contract| {
			(
				contract.path,
				contract.method,
				contract.metadata.handler,
				contract.metadata.authentication,
				contract.metadata.guard,
			)
		})
		.collect();

	assert_eq!(
		routes,
		vec![
			(
				"/api/items".to_string(),
				Method::GET,
				"tests::ProtectedEndpoint".to_string(),
				reinhardt_core::endpoint::AuthProtection::Protected,
				Some("role=admin".to_string()),
			),
			(
				"/internal/items".to_string(),
				Method::GET,
				"tests::ProtectedEndpoint".to_string(),
				reinhardt_core::endpoint::AuthProtection::Protected,
				Some("role=admin".to_string()),
			),
		]
	);
}

#[test]
fn mounted_contract_normalizes_endpoint_path_that_includes_router_prefix() {
	let router = ServerRouter::new()
		.with_prefix("/api")
		.endpoint(|| TestEndpoint::<10>);

	let contracts = router.get_mounted_route_contracts().unwrap();
	let paths: Vec<_> = contracts
		.into_iter()
		.map(|contract| contract.path)
		.collect();

	assert_eq!(paths, vec!["/api/users/".to_string()]);
}

#[test]
fn mounted_contract_rejects_collisions_after_prefix_flattening() {
	let router = ServerRouter::new()
		.handler("/nested/health", ContractRawHandler)
		.mount(
			"/nested/",
			ServerRouter::new().endpoint(|| TestEndpoint::<1>),
		);

	let error = router.get_mounted_route_contracts().unwrap_err();

	assert_eq!(error, "mounted route collision for `/nested/health` GET");
}

#[test]
fn mounted_contract_expands_typed_raw_handlers_and_class_views() {
	let router = ServerRouter::new()
		.handler("/raw", ContractRawHandler)
		.view("/class", ContractClassView);

	let contracts = router.get_mounted_route_contracts().unwrap();
	let raw_methods: Vec<_> = contracts
		.iter()
		.filter(|contract| contract.path == "/raw")
		.map(|contract| contract.method.clone())
		.collect();
	let class_methods: Vec<_> = contracts
		.iter()
		.filter(|contract| contract.path == "/class")
		.map(|contract| contract.method.clone())
		.collect();
	let raw_handler = contracts
		.iter()
		.find(|contract| contract.path == "/raw")
		.map(|contract| contract.metadata.handler.as_str());
	let class_handler = contracts
		.iter()
		.find(|contract| contract.path == "/class")
		.map(|contract| contract.metadata.handler.as_str());

	assert_eq!(
		raw_methods,
		vec![
			Method::GET,
			Method::POST,
			Method::PUT,
			Method::DELETE,
			Method::PATCH,
		]
	);
	assert_eq!(class_methods, raw_methods);
	assert_eq!(raw_handler, Some("route:/raw"));
	assert_eq!(class_handler, Some("view:/class"));
}

#[test]
fn mounted_contract_uses_declared_class_view_authentication() {
	#[allow(deprecated)]
	let router = ServerRouter::new()
		.view_with_authentication(
			"/class",
			ContractClassView,
			reinhardt_core::endpoint::AuthProtection::Protected,
		)
		.view_named_with_authentication(
			"/named-class",
			"named-class",
			ContractClassView,
			reinhardt_core::endpoint::AuthProtection::Protected,
		);

	let contracts = router.get_mounted_route_contracts().unwrap();

	assert_eq!(contracts.len(), 10);
	assert!(contracts.iter().all(|contract| {
		contract.metadata.authentication == reinhardt_core::endpoint::AuthProtection::Protected
	}));
}

#[cfg(feature = "viewsets")]
#[test]
fn mounted_contract_omits_viewset_extra_actions() {
	let router = ServerRouter::new().viewset("/contracts", ContractViewSet);

	let contracts = router.get_mounted_route_contracts().unwrap();
	assert!(contracts.iter().all(|contract| {
		contract.metadata.authentication == reinhardt_core::endpoint::AuthProtection::Public
	}));
	let handlers: Vec<_> = contracts
		.into_iter()
		.map(|contract| contract.metadata.handler)
		.collect();
	let viewset_name = "viewset:contracts";

	assert_eq!(
		handlers,
		vec![
			format!("{viewset_name}::list"),
			format!("{viewset_name}::create"),
			format!("{viewset_name}::retrieve"),
			format!("{viewset_name}::update"),
			format!("{viewset_name}::destroy"),
		]
	);
	assert!(
		!handlers
			.iter()
			.any(|handler| handler.ends_with("::archive"))
	);
	assert!(!handlers.iter().any(|handler| handler == "<erased handler>"));
}

#[cfg(feature = "viewsets")]
#[test]
fn mounted_contract_does_not_treat_permission_middleware_as_authentication() {
	let router = ServerRouter::new().viewset("/permission-only", PermissionOnlyViewSet);

	let contracts = router.get_mounted_route_contracts().unwrap();

	assert!(contracts.iter().all(|contract| {
		contract.metadata.authentication == reinhardt_core::endpoint::AuthProtection::None
	}));
}

#[cfg(feature = "viewsets")]
#[test]
fn mounted_contract_rejects_viewset_builder_erased_handler_metadata() {
	let mut router = ServerRouter::new();
	ViewSetBuilder::new(ContractViewSet)
		.action(Method::GET, "list")
		.register_to(&mut router, "/builder")
		.unwrap();

	let error = router.get_mounted_route_contracts().unwrap_err();

	assert_eq!(
		error,
		"mounted route `/builder` has no application-contract metadata; use a typed registration method or handler_arc_with_contract_metadata"
	);
}

#[cfg(feature = "viewsets")]
#[test]
fn mounted_contract_qualifies_standard_viewset_route_names() {
	let router = ServerRouter::new()
		.with_namespace("api")
		.viewset("/contracts", ContractViewSet);

	let contracts = router.get_mounted_route_contracts().unwrap();
	let names: Vec<_> = contracts
		.into_iter()
		.map(|contract| contract.name)
		.collect();

	assert_eq!(
		names,
		vec![
			Some("api:contracts-list".to_string()),
			Some("api:contracts-list".to_string()),
			Some("api:contracts-detail".to_string()),
			Some("api:contracts-detail".to_string()),
			Some("api:contracts-detail".to_string()),
		]
	);
}

#[test]
fn mounted_contract_rejects_erased_handler_without_metadata() {
	let router = ServerRouter::new().handler_arc("/opaque", Arc::new(TestEndpoint::<1>));

	let error = router.get_mounted_route_contracts().unwrap_err();

	assert_eq!(
		error,
		"mounted route `/opaque` has no application-contract metadata; use a typed registration method or handler_arc_with_contract_metadata"
	);
}

#[test]
fn mounted_contract_uses_explicit_erased_handler_metadata() {
	let router = ServerRouter::new().handler_arc_with_contract_metadata(
		"/opaque",
		Arc::new(TestEndpoint::<1>),
		RouteContractMetadata {
			handler: "tests::OpaqueEndpoint".to_string(),
			module_path: Some("tests".to_string()),
			function_name: Some("OpaqueEndpoint".to_string()),
			authentication: reinhardt_core::endpoint::AuthProtection::Public,
			guard: None,
		},
	);

	let contracts = router.get_mounted_route_contracts().unwrap();

	assert_eq!(contracts.len(), 5);
	assert!(contracts.iter().all(|contract| {
		contract.metadata.handler == "tests::OpaqueEndpoint"
			&& contract.metadata.authentication == reinhardt_core::endpoint::AuthProtection::Public
	}));
}
