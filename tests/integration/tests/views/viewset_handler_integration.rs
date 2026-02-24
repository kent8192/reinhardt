//! ViewSetHandler unit tests (extracted from reinhardt-views/src/viewsets/handler.rs)
//!
//! Tests ModelViewSetHandler CRUD operations, permissions, and error handling
//! using in-memory querysets (no database required).

use bytes::Bytes;
use hyper::{HeaderMap, Method, StatusCode, Version};
use reinhardt_auth::{AllowAny, IsAuthenticated};
use reinhardt_http::Request;
use reinhardt_rest::serializers::ModelSerializer;
use reinhardt_views::viewsets::{ModelViewSetHandler, ViewError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestUser {
	id: Option<i64>,
	username: String,
	email: String,
}

reinhardt_test::impl_test_model!(TestUser, i64, "users");

fn create_test_users() -> Vec<TestUser> {
	vec![
		TestUser {
			id: Some(1),
			username: "alice".to_string(),
			email: "alice@example.com".to_string(),
		},
		TestUser {
			id: Some(2),
			username: "bob".to_string(),
			email: "bob@example.com".to_string(),
		},
		TestUser {
			id: Some(3),
			username: "charlie".to_string(),
			email: "charlie@example.com".to_string(),
		},
	]
}

/// Test: ModelViewSetHandler can be constructed with new()
#[tokio::test]
async fn test_model_viewset_handler_new() {
	// Arrange
	let handler = ModelViewSetHandler::<TestUser>::new();

	// Act - list with no queryset should return empty
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = handler.list(&request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, "[]");
}

/// Test: ModelViewSetHandler with queryset returns correct items
#[tokio::test]
async fn test_model_viewset_handler_with_queryset() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	// Act
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = handler.list(&request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(body.contains("alice"));
	assert!(body.contains("bob"));
	assert!(body.contains("charlie"));
}

/// Test: ModelViewSetHandler with custom serializer
#[tokio::test]
async fn test_model_viewset_handler_with_serializer() {
	// Arrange
	let users = create_test_users();
	let serializer = Arc::new(ModelSerializer::<TestUser>::new());
	let handler = ModelViewSetHandler::<TestUser>::new()
		.with_queryset(users)
		.with_serializer(serializer);

	// Act
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = handler.list(&request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(body.contains("alice"));
}

/// Test: ModelViewSetHandler with multiple permissions
#[tokio::test]
async fn test_model_viewset_handler_add_permission() {
	// Arrange - AllowAny should pass even with IsAuthenticated also registered
	// AllowAny is checked first and should allow access
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new()
		.with_queryset(users)
		.add_permission(Arc::new(AllowAny))
		.add_permission(Arc::new(IsAuthenticated));

	// Act
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let result = handler.list(&request).await;

	// Assert - IsAuthenticated should deny unauthenticated request
	// (AllowAny passes, but IsAuthenticated fails for unauthenticated)
	assert!(result.is_err());
}

/// Test: ModelViewSetHandler list returns all users
#[tokio::test]
async fn test_model_viewset_handler_list() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	// Act
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = handler.list(&request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(body.contains("alice"));
	assert!(body.contains("bob"));
	assert!(body.contains("charlie"));
}

/// Test: ModelViewSetHandler retrieve returns single user
#[tokio::test]
async fn test_model_viewset_handler_retrieve() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let pk = serde_json::json!(1);
	let response = handler.retrieve(&request, pk).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(body.contains("alice"));
}

/// Test: ModelViewSetHandler retrieve returns error for non-existent user
#[tokio::test]
async fn test_model_viewset_handler_retrieve_not_found() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/999/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let pk = serde_json::json!(999);
	let result = handler.retrieve(&request, pk).await;

	// Assert
	assert!(result.is_err());
	if let Err(e) = result {
		assert!(matches!(e, ViewError::NotFound(_)));
	}
}

/// Test: ModelViewSetHandler create returns CREATED status
#[tokio::test]
async fn test_model_viewset_handler_create() {
	// Arrange
	let handler = ModelViewSetHandler::<TestUser>::new();

	let body = r#"{"id":4,"username":"dave","email":"dave@example.com"}"#;
	let request = Request::builder()
		.method(Method::POST)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(body))
		.build()
		.unwrap();

	// Act
	let response = handler.create(&request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::CREATED);
	let response_body = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(response_body.contains("dave"));
}

/// Test: ModelViewSetHandler create returns error for invalid body
#[tokio::test]
async fn test_model_viewset_handler_create_invalid_body() {
	// Arrange
	let handler = ModelViewSetHandler::<TestUser>::new();

	let body = r#"{"invalid": "data"}"#;
	let request = Request::builder()
		.method(Method::POST)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(body))
		.build()
		.unwrap();

	// Act
	let result = handler.create(&request).await;

	// Assert
	assert!(result.is_err());
}

/// Test: ModelViewSetHandler update modifies existing user
#[tokio::test]
async fn test_model_viewset_handler_update() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	let body = r#"{"id":1,"username":"alice_updated","email":"alice_new@example.com"}"#;
	let request = Request::builder()
		.method(Method::PUT)
		.uri("/users/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(body))
		.build()
		.unwrap();

	// Act
	let pk = serde_json::json!(1);
	let response = handler.update(&request, pk).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let response_body = String::from_utf8(response.body.to_vec()).unwrap();
	assert!(response_body.contains("alice_updated"));
}

/// Test: ModelViewSetHandler update returns error for non-existent user
#[tokio::test]
async fn test_model_viewset_handler_update_not_found() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	let body = r#"{"id":999,"username":"nonexistent","email":"none@example.com"}"#;
	let request = Request::builder()
		.method(Method::PUT)
		.uri("/users/999/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::from(body))
		.build()
		.unwrap();

	// Act
	let pk = serde_json::json!(999);
	let result = handler.update(&request, pk).await;

	// Assert
	assert!(result.is_err());
}

/// Test: ModelViewSetHandler destroy returns NO_CONTENT
#[tokio::test]
async fn test_model_viewset_handler_destroy() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	let request = Request::builder()
		.method(Method::DELETE)
		.uri("/users/1/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let pk = serde_json::json!(1);
	let response = handler.destroy(&request, pk).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::NO_CONTENT);
}

/// Test: ModelViewSetHandler destroy returns error for non-existent user
#[tokio::test]
async fn test_model_viewset_handler_destroy_not_found() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(users);

	let request = Request::builder()
		.method(Method::DELETE)
		.uri("/users/999/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let pk = serde_json::json!(999);
	let result = handler.destroy(&request, pk).await;

	// Assert
	assert!(result.is_err());
}

/// Test: ModelViewSetHandler denies access when IsAuthenticated permission is set
#[tokio::test]
async fn test_model_viewset_handler_permission_denied() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new()
		.with_queryset(users)
		.add_permission(Arc::new(IsAuthenticated));

	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let result = handler.list(&request).await;

	// Assert
	assert!(result.is_err());
	if let Err(e) = result {
		assert!(matches!(e, ViewError::Permission(_)));
	}
}

/// Test: ModelViewSetHandler allows access with AllowAny permission
#[tokio::test]
async fn test_model_viewset_handler_allow_any_permission() {
	// Arrange
	let users = create_test_users();
	let handler = ModelViewSetHandler::<TestUser>::new()
		.with_queryset(users)
		.add_permission(Arc::new(AllowAny));

	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let result = handler.list(&request).await;

	// Assert
	assert!(result.is_ok());
}

/// Test: ViewError Display implementations
#[tokio::test]
async fn test_view_error_display() {
	// Assert
	let error = ViewError::Serialization("test".to_string());
	assert_eq!(error.to_string(), "Serialization error: test");

	let error = ViewError::Permission("denied".to_string());
	assert_eq!(error.to_string(), "Permission denied: denied");

	let error = ViewError::NotFound("missing".to_string());
	assert_eq!(error.to_string(), "Not found: missing");

	let error = ViewError::BadRequest("invalid".to_string());
	assert_eq!(error.to_string(), "Bad request: invalid");

	let error = ViewError::Internal("internal".to_string());
	assert_eq!(error.to_string(), "Internal error: internal");
}

/// Test: ModelViewSetHandler default() is equivalent to new()
#[tokio::test]
async fn test_model_viewset_handler_default() {
	// Arrange
	let handler = ModelViewSetHandler::<TestUser>::default();

	// Act
	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	let response = handler.list(&request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, "[]");
}

/// Test: ModelViewSetHandler with empty queryset returns empty list
#[tokio::test]
async fn test_model_viewset_handler_empty_queryset() {
	// Arrange
	let handler = ModelViewSetHandler::<TestUser>::new().with_queryset(vec![]);

	let request = Request::builder()
		.method(Method::GET)
		.uri("/users/")
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let response = handler.list(&request).await.unwrap();

	// Assert
	assert_eq!(response.status, StatusCode::OK);
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, "[]");
}
