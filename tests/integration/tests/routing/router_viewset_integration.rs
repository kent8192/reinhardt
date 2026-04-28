// Router and ViewSet integration tests
// Inspired by Django REST Framework's ViewSet router registration tests

use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_http::Request;
use reinhardt_macros::model;
use reinhardt_urls::routers::{DefaultRouter, Router};
use reinhardt_views::viewsets::ModelViewSet;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[allow(dead_code)]
#[model(table_name = "test_models")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestModel {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

#[derive(Debug, Clone)]
struct TestSerializer;

// Test: ViewSet registration with router (inspired by DRF's TestSimpleRouter)
#[tokio::test]
async fn test_viewset_registration() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("users", viewset);

	let routes = router.get_routes();
	assert_eq!(routes.len(), 2); // list and detail routes

	// Check list route
	assert_eq!(routes[0].path, "/users/");
	assert_eq!(routes[0].name.as_deref(), Some("users-list"));

	// Check detail route
	assert_eq!(routes[1].path, "/users/{id}/");
	assert_eq!(routes[1].name.as_deref(), Some("users-detail"));
}

// Test: Multiple ViewSet registration
#[tokio::test]
async fn test_multiple_viewset_registration() {
	let mut router = DefaultRouter::new();

	let users_viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));
	let posts_viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("posts"));

	router.register_viewset("users", users_viewset);
	router.register_viewset("posts", posts_viewset);

	let routes = router.get_routes();
	assert_eq!(routes.len(), 4); // 2 ViewSets * 2 routes each
}

// Test: ViewSet list route matching
#[tokio::test]
async fn test_viewset_list_route_matching() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("users", viewset);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

// Test: ViewSet detail route matching.
// Provides an in-memory item with id=123 so the dispatch's retrieve resolves
// to a real row (issue #3985 — dispatch now flows through ModelViewSetHandler).
#[tokio::test]
async fn test_viewset_detail_route_matching() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users").with_queryset(vec![TestModel {
			id: 123,
			name: "alice".into(),
		}]));

	router.register_viewset("users", viewset);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/123/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

// Test: ViewSet create action (POST to list route).
// After issue #3985 the dispatch flows through the real ModelViewSetHandler,
// so the body must deserialize into the model.
#[tokio::test]
async fn test_viewset_create_action() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("users", viewset);

	let request = Request::builder()
		.method(Method::POST)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"id": 1, "name": "test"}"#))
		.build()
		.unwrap();

	let response = router.route(request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::CREATED);
}

// Test: ViewSet update action (PUT to detail route).
// Provides an in-memory item with id=123 so the dispatch resolves the pk
// before applying the update body (issue #3985).
#[tokio::test]
async fn test_viewset_update_action() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users").with_queryset(vec![TestModel {
			id: 123,
			name: "alice".into(),
		}]));

	router.register_viewset("users", viewset);

	let request = Request::builder()
		.method(Method::PUT)
		.uri("/users/123/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"id": 123, "name": "updated"}"#))
		.build()
		.unwrap();

	let response = router.route(request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::OK);
}

// Test: ViewSet destroy action (DELETE to detail route).
// Provides an in-memory item so destroy resolves the pk and returns 204
// (issue #3985).
#[tokio::test]
async fn test_viewset_destroy_action() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users").with_queryset(vec![TestModel {
			id: 123,
			name: "alice".into(),
		}]));

	router.register_viewset("users", viewset);

	let request = Request::builder()
		.method(Method::DELETE)
		.uri("/users/123/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(request).await;
	assert!(response.is_ok());
	assert_eq!(response.unwrap().status, StatusCode::NO_CONTENT);
}

// Test: ViewSet basename in routes
#[tokio::test]
async fn test_viewset_basename_in_routes() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("custom-basename"));

	router.register_viewset("items", viewset);

	let routes = router.get_routes();
	assert_eq!(routes[0].name.as_deref(), Some("custom-basename-list"));
	assert_eq!(routes[1].name.as_deref(), Some("custom-basename-detail"));
}

// Test: ViewSet reverse URL lookup
#[tokio::test]
async fn test_viewset_reverse_url() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("users", viewset);

	// Reverse list URL
	let list_url = router.reverse("users-list", &std::collections::HashMap::new());
	assert!(list_url.is_ok());
	assert_eq!(list_url.unwrap(), "/users/");

	// Reverse detail URL
	let mut params = std::collections::HashMap::new();
	params.insert("id".to_string(), "42".to_string());
	let detail_url = router.reverse("users-detail", &params);
	assert!(detail_url.is_ok());
	assert_eq!(detail_url.unwrap(), "/users/42/");
}

// Test: Nested ViewSet routes with prefix
#[tokio::test]
async fn test_nested_viewset_routes() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("api/v1/users", viewset);

	let routes = router.get_routes();
	assert_eq!(routes[0].path, "/api/v1/users/");
	assert_eq!(routes[1].path, "/api/v1/users/{id}/");
}

// Test: ViewSet with custom lookup field.
// Provides an in-memory item keyed by the custom lookup field so retrieve
// resolves to a real row through the embedded ModelViewSetHandler.
#[tokio::test]
async fn test_viewset_custom_lookup_field() {
	let mut router = DefaultRouter::new();
	// Note: TestModel.primary_key() returns id, not name. We seed with id == 1
	// and use lookup_field = "username" — the ModelViewSet's lookup_field only
	// affects URL parameter resolution, not how the handler matches the pk.
	// For this test we just need the request to reach the dispatch with the
	// expected path param populated.
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> = Arc::new(
		ModelViewSet::new("users")
			.with_lookup_field("username")
			.with_queryset(vec![TestModel {
				id: 1,
				name: "alice".into(),
			}]),
	);

	router.register_viewset("users", viewset);

	// Verify that the route uses 'username' instead of 'id'
	let routes = router.get_routes();
	assert_eq!(routes.len(), 2);
	assert_eq!(routes[1].path, "/users/{username}/");

	// Test that the lookup field parameter is correctly populated by the router.
	// The handler will treat "alice" as a primary key and not find a match in
	// the queryset (which keys items by their `id`), so this confirms routing
	// reaches dispatch, not that the data is found.
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/alice/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(request).await;
	match response {
		Ok(resp) => assert_ne!(
			resp.status,
			StatusCode::OK,
			"REGRESSION GUARD (#3985): retrieve must not return placeholder 200 OK with empty body"
		),
		Err(e) => {
			let s = e.to_string();
			assert!(
				s.contains("Not found") || s.contains("not found"),
				"expected NotFound-style error, got: {s}"
			);
		}
	}
}

// Test: Multiple HTTP methods on same ViewSet route.
// Provides an in-memory item with id=1 so detail-route methods resolve the pk.
#[tokio::test]
async fn test_viewset_multiple_http_methods() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users").with_queryset(vec![TestModel {
			id: 1,
			name: "alice".into(),
		}]));

	router.register_viewset("users", viewset);

	// GET (retrieve)
	let get_request = Request::builder()
		.method(Method::GET)
		.uri("/users/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let get_response = router.route(get_request).await;
	assert!(get_response.is_ok());

	// PUT (update)
	let put_request = Request::builder()
		.method(Method::PUT)
		.uri("/users/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(r#"{"id": 1, "name": "updated"}"#))
		.build()
		.unwrap();
	let put_response = router.route(put_request).await;
	assert!(put_response.is_ok());

	// DELETE (destroy)
	let delete_request = Request::builder()
		.method(Method::DELETE)
		.uri("/users/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();
	let delete_response = router.route(delete_request).await;
	assert!(delete_response.is_ok());
}

// Test: ViewSet route not found for invalid path
#[tokio::test]
async fn test_viewset_route_not_found() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("users", viewset);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/invalid/path/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = router.route(request).await;
	assert!(response.is_err());
}

// Test: ViewSet registration with trailing slashes
#[tokio::test]
async fn test_viewset_trailing_slashes() {
	let mut router = DefaultRouter::new();
	let viewset: Arc<ModelViewSet<TestModel, TestSerializer>> =
		Arc::new(ModelViewSet::new("users"));

	router.register_viewset("users", viewset);

	let routes = router.get_routes();
	// Both routes should have trailing slashes
	assert!(routes[0].path.ends_with('/'));
	assert!(routes[1].path.ends_with('/'));
}
