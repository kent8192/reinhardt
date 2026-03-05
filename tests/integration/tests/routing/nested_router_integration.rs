//! Nested router implementation tests

use reinhardt_urls::routers::ServerRouter;
use reinhardt_views::viewsets::{ModelViewSet, NestedResource, NestedViewSet, nested_url};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
	id: i64,
	name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Post {
	id: i64,
	user_id: i64,
	title: String,
}

#[derive(Debug, Clone)]
struct UserSerializer;

#[derive(Debug, Clone)]
struct PostSerializer;

#[tokio::test]
async fn test_nested_router_basic_structure() {
	// Create parent router for users
	let users_router = ServerRouter::new().with_namespace("users");

	// Create child router for posts
	let posts_router = ServerRouter::new().with_namespace("posts");

	// Nest posts under users
	let api_router = ServerRouter::new()
		.with_prefix("/api")
		.mount("/users/", users_router)
		.mount("/posts/", posts_router);

	// Verify structure
	assert_eq!(api_router.prefix(), "/api");
	assert_eq!(api_router.children_count(), 2);
}

#[tokio::test]
async fn test_nested_router_with_viewsets() {
	// Create ViewSets
	let _user_viewset: ModelViewSet<User, UserSerializer> = ModelViewSet::new("users");
	let post_viewset: ModelViewSet<Post, PostSerializer> = ModelViewSet::new("posts");

	// Create nested resource configuration
	let nested = NestedResource::new("user", "user_id", "user_id");
	let nested_post_viewset = NestedViewSet::new(post_viewset, nested);

	// Verify nested config
	let config = nested_post_viewset.nested_config();
	assert_eq!(config.parent, "user");
	assert_eq!(config.parent_id_param, "user_id");
	assert_eq!(config.lookup_field, "user_id");
}

#[tokio::test]
async fn test_nested_url_helpers() {
	// Test URL helpers
	let list_url = nested_url("users", "123", "posts");
	assert_eq!(list_url, "users/123/posts/");

	let nested_resource = NestedResource::new("user", "user_id", "user_id");
	assert_eq!(nested_resource.parent, "user");
}

#[tokio::test]
async fn test_deeply_nested_router() {
	// Create deeply nested router structure:
	// /api/v1/orgs/{org_id}/teams/{team_id}/members/

	let members_router = ServerRouter::new().with_namespace("members");

	let teams_router = ServerRouter::new()
		.with_namespace("teams")
		.mount("/{team_id}/members/", members_router);

	let orgs_router = ServerRouter::new()
		.with_namespace("orgs")
		.mount("/{org_id}/teams/", teams_router);

	let api_router = ServerRouter::new()
		.with_prefix("/api/v1")
		.mount("/orgs/", orgs_router);

	// Verify deep nesting structure
	assert_eq!(api_router.prefix(), "/api/v1");
	assert_eq!(api_router.children_count(), 1);
}
