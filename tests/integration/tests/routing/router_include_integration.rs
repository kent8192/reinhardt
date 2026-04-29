//! Router mount() API integration tests

use reinhardt_urls::routers::ServerRouter;

#[tokio::test]
async fn test_router_mount_basic() {
	// Create child router
	let users_router = ServerRouter::new().with_namespace("users");

	// Use mount() to mount child router
	let router = ServerRouter::new()
		.with_prefix("/api")
		.mount("/users/", users_router);

	// Verify router structure using public API
	assert_eq!(router.prefix(), "/api");
	assert_eq!(router.children_count(), 1);
}

#[tokio::test]
async fn test_router_mount_mut() {
	// Create parent router
	let mut router = ServerRouter::new().with_prefix("/api");

	// Create child routers
	let users_router = ServerRouter::new().with_namespace("users");
	let posts_router = ServerRouter::new().with_namespace("posts");

	// Use mount_mut() to mount child routers
	router.mount_mut("/users/", users_router);
	router.mount_mut("/posts/", posts_router);

	// Verify router structure using public API
	assert_eq!(router.prefix(), "/api");
	assert_eq!(router.children_count(), 2);
}

#[tokio::test]
async fn test_router_mount_multiple() {
	// Create routers and verify mount works correctly
	let users_router1 = ServerRouter::new().with_namespace("users");
	let router1 = ServerRouter::new()
		.with_prefix("/api")
		.mount("/users/", users_router1);

	// Create another router with same structure
	let users_router2 = ServerRouter::new().with_namespace("users");
	let router2 = ServerRouter::new()
		.with_prefix("/api")
		.mount("/users/", users_router2);

	// Verify both routers have identical structure using public API
	assert_eq!(router1.prefix(), router2.prefix());
	assert_eq!(router1.children_count(), router2.children_count());
}

#[tokio::test]
async fn test_router_mount_nested() {
	// Create deeply nested router structure with literal mount prefixes.
	// Path parameters ({id}) belong on child routes, not on mount() prefixes —
	// see ServerRouter::validate_prefix (Fixes #4025).
	let detail_router = ServerRouter::new().with_namespace("user_detail");

	let users_router = ServerRouter::new()
		.with_namespace("users")
		.mount("/detail/", detail_router);

	let api_router = ServerRouter::new()
		.with_prefix("/api")
		.mount("/users/", users_router);

	// Verify nested structure using public API
	assert_eq!(api_router.prefix(), "/api");
	assert_eq!(api_router.children_count(), 1);
}
