//! Integration tests for UnifiedRouter with hierarchical routing and namespace support

use async_trait::async_trait;
use reinhardt_http::Handler;
use reinhardt_http::{Request, Response, Result, ViewResult};
use reinhardt_macros::get;
use reinhardt_urls::routers::ServerRouter;
use reinhardt_views::viewsets::{Action, ActionType, ViewSet};

// Mock ViewSet for testing
#[derive(Clone)]
struct UserViewSet;

#[async_trait]
impl ViewSet for UserViewSet {
	fn get_basename(&self) -> &str {
		"users"
	}

	async fn dispatch(&self, _req: Request, action: Action) -> Result<Response> {
		match action.action_type {
			ActionType::List => Ok(Response::ok().with_body(b"User list".to_vec())),
			ActionType::Retrieve => Ok(Response::ok().with_body(b"User detail".to_vec())),
			ActionType::Create => Ok(Response::ok().with_body(b"User created".to_vec())),
			ActionType::Update => Ok(Response::ok().with_body(b"User updated".to_vec())),
			ActionType::Destroy => Ok(Response::ok().with_body(b"User deleted".to_vec())),
			_ => Ok(Response::not_found()),
		}
	}
}

// Mock handlers using HTTP Method Macro
#[get("/health", name = "health")]
async fn health_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"OK".to_vec()))
}

#[get("/list", name = "list")]
async fn list_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"List".to_vec()))
}

#[get("/action", name = "action")]
async fn action_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"Action".to_vec()))
}

#[get("/export", name = "export")]
async fn export_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"Export".to_vec()))
}

// Mock view handler
#[derive(Clone)]
struct AboutView;

#[async_trait]
impl Handler for AboutView {
	async fn handle(&self, _req: Request) -> Result<Response> {
		Ok(Response::ok().with_body(b"About page".to_vec()))
	}
}

#[tokio::test]
async fn test_unified_router_basic_structure() {
	let router = ServerRouter::new()
		.with_prefix("/api")
		.with_namespace("api");

	assert_eq!(router.prefix(), "/api");
	assert_eq!(router.namespace(), Some("api"));
}

#[tokio::test]
async fn test_unified_router_mount_child() {
	let child = ServerRouter::new().with_namespace("users");

	let router = ServerRouter::new()
		.with_prefix("/api")
		.with_namespace("api")
		.mount("/users/", child);

	assert_eq!(router.children_count(), 1);
}

#[tokio::test]
async fn test_unified_router_with_viewset() {
	let router = ServerRouter::new()
		.with_prefix("/api")
		.viewset("users", UserViewSet);

	// Check that routes are generated
	let routes = router.get_all_routes();
	assert!(!routes.is_empty());
}

#[tokio::test]
async fn test_unified_router_hierarchical_namespace() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.viewset("users", UserViewSet);

	let mut api = ServerRouter::new()
		.with_namespace("v1")
		.with_prefix("/api/v1")
		.mount("/users/", users);

	// Register all routes
	let errors = api.register_all_routes();
	assert!(errors.is_empty());

	// Check namespace resolution
	assert_eq!(api.namespace(), Some("v1"));
}

#[tokio::test]
async fn test_unified_router_url_reversal() {
	let mut router = ServerRouter::new()
		.with_namespace("api")
		.endpoint(health_handler);

	let errors = router.register_all_routes();
	assert!(errors.is_empty());

	// Reverse URL with namespace
	let url = router.reverse("api:health", &[]);
	assert!(url.is_some());
	assert_eq!(url.unwrap(), "/health");
}

#[tokio::test]
async fn test_unified_router_nested_namespace_reversal() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.endpoint(list_handler);

	let v1 = ServerRouter::new()
		.with_namespace("v1")
		.mount("/users/", users);

	let mut api = ServerRouter::new().with_namespace("api").mount("/v1/", v1);

	let errors = api.register_all_routes();
	assert!(errors.is_empty());

	// Reverse with full namespace chain
	let url = api.reverse("api:v1:users:list", &[]);
	assert!(url.is_some());
}

#[tokio::test]
async fn test_unified_router_multiple_children() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.viewset("users", UserViewSet);

	let posts = ServerRouter::new()
		.with_namespace("posts")
		.endpoint(list_handler);

	let router = ServerRouter::new()
		.with_prefix("/api")
		.mount("/users/", users)
		.mount("/posts/", posts);

	assert_eq!(router.children_count(), 2);

	let routes = router.get_all_routes();
	assert!(!routes.is_empty());
}

#[tokio::test]
async fn test_unified_router_mixed_api_styles() {
	let router = ServerRouter::new()
		.with_prefix("/api")
		.endpoint(health_handler)
		.viewset("users", UserViewSet)
		.view("/about", AboutView);

	let routes = router.get_all_routes();
	// Should have routes from endpoint, ViewSet, and view
	assert!(routes.len() >= 3);
}

#[tokio::test]
async fn test_unified_router_deep_nesting() {
	// Create a dedicated handler for the action endpoint with POST method
	#[reinhardt_macros::post("/action", name = "action")]
	async fn action_post_handler() -> ViewResult<Response> {
		Ok(Response::ok().with_body(b"Action".to_vec()))
	}

	let resource = ServerRouter::new()
		.with_namespace("resource")
		.endpoint(action_post_handler);

	let v2 = ServerRouter::new()
		.with_namespace("v2")
		.mount("/resource/", resource);

	let v1 = ServerRouter::new().with_namespace("v1").mount("/v2/", v2);

	let mut api = ServerRouter::new()
		.with_namespace("api")
		.with_prefix("/api")
		.mount("/v1/", v1);

	let errors = api.register_all_routes();
	assert!(errors.is_empty());

	// Test deep namespace resolution
	let url = api.reverse("api:v1:v2:resource:action", &[]);
	assert!(url.is_some());
}

#[tokio::test]
async fn test_unified_router_get_all_routes() {
	let users = ServerRouter::new()
		.with_namespace("users")
		.endpoint(export_handler);

	let router = ServerRouter::new()
		.with_prefix("/api")
		.with_namespace("api")
		.endpoint(health_handler)
		.mount("/users/", users);

	let routes = router.get_all_routes();

	// Should have routes from both parent and child
	assert!(routes.len() >= 2);

	// Check namespace combination in routes
	let has_combined_namespace = routes
		.iter()
		.any(|(_, _, ns, _)| ns.as_ref().is_some_and(|n| n.contains(':')));
	assert!(has_combined_namespace);
}

#[tokio::test]
async fn test_unified_router_viewset_url_reversal() {
	let mut router = ServerRouter::new()
		.with_namespace("api")
		.with_prefix("/api")
		.viewset("users", UserViewSet);

	let errors = router.register_all_routes();
	assert!(errors.is_empty());

	// ViewSets should register standard action names
	let list_url = router.reverse("api:users-list", &[]);
	assert!(list_url.is_some());

	let detail_url = router.reverse("api:users-detail", &[("id", "123")]);
	assert!(detail_url.is_some());
}

#[tokio::test]
async fn test_unified_router_namespace_inheritance() {
	// Create a dedicated handler for this test with POST method
	#[reinhardt_macros::post("/action", name = "action")]
	async fn action_inherit_handler() -> ViewResult<Response> {
		Ok(Response::ok().with_body(b"Action".to_vec()))
	}

	let child = ServerRouter::new().endpoint(action_inherit_handler);

	let mut parent = ServerRouter::new()
		.with_namespace("parent")
		.mount("/child/", child);

	let errors = parent.register_all_routes();
	assert!(errors.is_empty());

	// Child route should inherit parent namespace
	let url = parent.reverse("parent:action", &[]);
	assert!(url.is_some());
}
