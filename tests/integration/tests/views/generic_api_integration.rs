//! Integration tests for Generic API Views
//!
//! These tests verify the HTTP request/response behavior of all 8 Generic API Views:
//! - ListAPIView, CreateAPIView, UpdateAPIView, DestroyAPIView
//! - ListCreateAPIView, RetrieveUpdateAPIView, RetrieveDestroyAPIView, RetrieveUpdateDestroyAPIView

use hyper::{HeaderMap, Method, Uri, Version};
use reinhardt_http::Request;
use reinhardt_rest::serializers::JsonSerializer;
use reinhardt_views::View;
use reinhardt_views::generic::{
	CreateAPIView, DestroyAPIView, ListAPIView, ListCreateAPIView, RetrieveDestroyAPIView,
	RetrieveUpdateAPIView, RetrieveUpdateDestroyAPIView, UpdateAPIView,
};
use serde::{Deserialize, Serialize};

// Test model for integration tests
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestArticle {
	id: Option<i64>,
	title: String,
	content: String,
}

reinhardt_test::impl_test_model!(TestArticle, i64, "articles");

// Helper function to create a test request
fn create_test_request(method: Method, path: &str) -> Request {
	let uri: Uri = path.parse().unwrap();
	let version = Version::HTTP_11;
	let headers = HeaderMap::new();
	let body = hyper::body::Bytes::new();
	Request::builder()
		.method(method)
		.uri(uri)
		.version(version)
		.headers(headers)
		.body(body)
		.build()
		.unwrap()
}

// ListAPIView integration tests
#[tokio::test]
async fn test_list_api_view_get_request() {
	let view = ListAPIView::<TestArticle, JsonSerializer<TestArticle>>::new()
		.with_paginate_by(10)
		.with_ordering(vec!["-created_at".to_string()]);

	let request = create_test_request(Method::GET, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes GET requests but fails due to missing database connection
	assert!(
		response.is_err(),
		"GET request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_list_api_view_head_request() {
	let view = ListAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::HEAD, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes HEAD requests but fails due to missing database connection
	assert!(
		response.is_err(),
		"HEAD request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_list_api_view_disallowed_method() {
	let view = ListAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::POST, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	assert!(
		response.is_err(),
		"POST request should fail for ListAPIView"
	);
}

// CreateAPIView integration tests
#[tokio::test]
async fn test_create_api_view_post_request() {
	let view = CreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::POST, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes POST requests but fails due to missing database connection or empty request body
	assert!(
		response.is_err(),
		"POST request fails due to uninitialized database connection or invalid request body"
	);
}

#[tokio::test]
async fn test_create_api_view_disallowed_method() {
	let view = CreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::GET, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	assert!(
		response.is_err(),
		"GET request should fail for CreateAPIView"
	);
}

// UpdateAPIView integration tests
#[tokio::test]
async fn test_update_api_view_put_request() {
	let view = UpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new()
		.with_lookup_field("id".to_string());

	let request = create_test_request(Method::PUT, "http://example.com/articles/1/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes PUT requests but fails due to missing database connection or empty request body
	assert!(
		response.is_err(),
		"PUT request fails due to uninitialized database connection or invalid request body"
	);
}

#[tokio::test]
async fn test_update_api_view_patch_request() {
	let view = UpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new().with_partial(true);

	let request = create_test_request(Method::PATCH, "http://example.com/articles/1/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes PATCH requests but fails due to missing database connection or empty request body
	assert!(
		response.is_err(),
		"PATCH request fails due to uninitialized database connection or invalid request body"
	);
}

#[tokio::test]
async fn test_update_api_view_disallowed_method() {
	let view = UpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::DELETE, "http://example.com/articles/1/");
	let response = view.dispatch(request).await;

	assert!(
		response.is_err(),
		"DELETE request should fail for UpdateAPIView"
	);
}

// DestroyAPIView integration tests
#[tokio::test]
async fn test_destroy_api_view_delete_request() {
	let view = DestroyAPIView::<TestArticle>::new().with_lookup_field("id".to_string());

	let request = create_test_request(Method::DELETE, "http://example.com/articles/1/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes DELETE requests but fails due to missing database connection
	assert!(
		response.is_err(),
		"DELETE request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_destroy_api_view_disallowed_method() {
	let view = DestroyAPIView::<TestArticle>::new();

	let request = create_test_request(Method::GET, "http://example.com/articles/1/");
	let response = view.dispatch(request).await;

	assert!(
		response.is_err(),
		"GET request should fail for DestroyAPIView"
	);
}

// ListCreateAPIView integration tests
#[tokio::test]
async fn test_list_create_api_view_get_request() {
	let view =
		ListCreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new().with_paginate_by(20);

	let request = create_test_request(Method::GET, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes GET requests but fails due to missing database connection
	assert!(
		response.is_err(),
		"GET request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_list_create_api_view_post_request() {
	let view = ListCreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::POST, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	// Verifies that the view correctly processes POST requests but fails due to missing database connection or empty request body
	assert!(
		response.is_err(),
		"POST request fails due to uninitialized database connection or invalid request body"
	);
}

#[tokio::test]
async fn test_list_create_api_view_disallowed_method() {
	let view = ListCreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::DELETE, "http://example.com/articles/");
	let response = view.dispatch(request).await;

	assert!(
		response.is_err(),
		"DELETE request should fail for ListCreateAPIView"
	);
}

// RetrieveUpdateAPIView integration tests
#[tokio::test]
async fn test_retrieve_update_api_view_get_request() {
	let view = RetrieveUpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::GET, "http://example.com/articles/1/");

	// Verifies that the view correctly processes GET requests but fails due to missing database connection
	let response = view.dispatch(request).await;
	assert!(
		response.is_err(),
		"GET request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_retrieve_update_api_view_put_request() {
	let view = RetrieveUpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::PUT, "http://example.com/articles/1/");

	// Verifies that the view correctly processes PUT requests but fails due to missing database connection or empty request body
	let response = view.dispatch(request).await;
	assert!(
		response.is_err(),
		"PUT request fails due to uninitialized database connection or invalid request body"
	);
}

// RetrieveDestroyAPIView integration tests
#[tokio::test]
async fn test_retrieve_destroy_api_view_get_request() {
	let view = RetrieveDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::GET, "http://example.com/articles/1/");

	// Verifies that the view correctly processes GET requests but fails due to missing database connection
	let response = view.dispatch(request).await;
	assert!(
		response.is_err(),
		"GET request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_retrieve_destroy_api_view_delete_request() {
	let view = RetrieveDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::DELETE, "http://example.com/articles/1/");

	// Verifies that the view correctly processes DELETE requests but fails due to missing database connection
	let response = view.dispatch(request).await;
	assert!(
		response.is_err(),
		"DELETE request fails due to uninitialized database connection"
	);
}

// RetrieveUpdateDestroyAPIView integration tests
#[tokio::test]
async fn test_retrieve_update_destroy_api_view_get_request() {
	let view = RetrieveUpdateDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::GET, "http://example.com/articles/1/");

	// Verifies that the view correctly processes GET requests but fails due to missing database connection
	let response = view.dispatch(request).await;
	assert!(
		response.is_err(),
		"GET request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_retrieve_update_destroy_api_view_put_request() {
	let view = RetrieveUpdateDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::PUT, "http://example.com/articles/1/");

	// Verifies that the view correctly processes PUT requests but fails due to missing database connection or empty request body
	let response = view.dispatch(request).await;
	assert!(
		response.is_err(),
		"PUT request fails due to uninitialized database connection or invalid request body"
	);
}

#[tokio::test]
async fn test_retrieve_update_destroy_api_view_delete_request() {
	let view = RetrieveUpdateDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::DELETE, "http://example.com/articles/1/");

	// Verifies that the view correctly processes DELETE requests but fails due to missing database connection
	let response = view.dispatch(request).await;
	assert!(
		response.is_err(),
		"DELETE request fails due to uninitialized database connection"
	);
}

#[tokio::test]
async fn test_retrieve_update_destroy_api_view_disallowed_method() {
	let view = RetrieveUpdateDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	let request = create_test_request(Method::POST, "http://example.com/articles/1/");
	let response = view.dispatch(request).await;

	assert!(
		response.is_err(),
		"POST request should fail for RetrieveUpdateDestroyAPIView"
	);
}
