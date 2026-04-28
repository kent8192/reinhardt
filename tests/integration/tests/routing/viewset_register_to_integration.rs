//! ViewSet as_view → register_to_router integration tests

use hyper::Method;
use reinhardt_macros::model;
use reinhardt_urls::routers::ServerRouter;
use reinhardt_views::viewsets::ModelViewSet;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[model(table_name = "users")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestUser {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

#[derive(Debug, Clone)]
struct TestSerializer;

#[tokio::test]
async fn test_viewset_register_to() {
	// Create router
	let mut router = ServerRouter::new();

	// Create viewset
	let viewset: ModelViewSet<TestUser, TestSerializer> = ModelViewSet::new("users");

	// Use new register_to() API
	let result = viewset
		.as_view()
		.action(Method::GET, "list")
		.action(Method::POST, "create")
		.register_to(&mut router, "/users");

	assert!(result.is_ok(), "register_to should succeed");

	// Get all routes to verify registration
	let routes = router.get_all_routes();
	assert!(!routes.is_empty(), "Router should have registered routes");
}

#[tokio::test]
async fn test_viewset_register_to_with_actions() {
	let mut router = ServerRouter::new();
	let viewset: ModelViewSet<TestUser, TestSerializer> = ModelViewSet::new("users");

	// Register with multiple actions
	let result = viewset
		.as_view()
		.action(Method::GET, "list")
		.action(Method::POST, "create")
		.action(Method::PUT, "update")
		.action(Method::DELETE, "destroy")
		.register_to(&mut router, "/users");

	assert!(
		result.is_ok(),
		"register_to with multiple actions should succeed"
	);

	// Get all routes to verify registration
	let routes = router.get_all_routes();
	assert!(!routes.is_empty(), "Router should have registered routes");
}

#[tokio::test]
async fn test_register_to_requires_actions() {
	let mut router = ServerRouter::new();
	let viewset: ModelViewSet<TestUser, TestSerializer> = ModelViewSet::new("users");

	// Try to register without actions - should fail
	let result = viewset.as_view().register_to(&mut router, "/users");

	assert!(result.is_err(), "register_to without actions should fail");
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("actions` argument must be provided"),
		"Error message should mention missing actions"
	);
}
