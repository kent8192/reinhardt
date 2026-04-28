//! Integration tests for ViewSet handler behavior with Request/Response
//!
//! These tests verify the interface between reinhardt-apps (Request) and
//! reinhardt-viewsets (ViewSet handlers).

use hyper::{HeaderMap, Method, Uri, Version};
use reinhardt_http::Request;
use reinhardt_macros::model;
use reinhardt_views::viewsets::viewset::ModelViewSet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[allow(dead_code)]
#[model(table_name = "test_models")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestModel {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

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

fn create_test_request_with_body(method: Method, path: &str, body: &'static str) -> Request {
	let uri: Uri = path.parse().unwrap();
	Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(hyper::body::Bytes::from(body))
		.build()
		.unwrap()
}

#[tokio::test]
async fn test_viewset_handler_attribute_tracking() {
	let viewset = ModelViewSet::<TestModel, ()>::new("test");
	let mut actions = HashMap::new();
	actions.insert(Method::GET, "list".to_string());

	let builder = viewset.as_view();
	let handler = builder.with_actions(actions).build().unwrap();

	// Create a test request
	let request = create_test_request(Method::GET, "http://example.com/test/");

	// Handle the request - this should work without errors
	let response = handler.handle(request).await;
	assert!(response.is_ok());
}

#[tokio::test]
async fn test_viewset_handler_action_mapping() {
	let viewset = ModelViewSet::<TestModel, ()>::new("test");
	let mut actions = HashMap::new();
	actions.insert(Method::GET, "list".to_string());
	actions.insert(Method::POST, "create".to_string());

	let builder = viewset.as_view();
	let handler = builder.with_actions(actions).build().unwrap();

	// Test GET request maps to list action
	let get_request = create_test_request(Method::GET, "http://example.com/test/");
	let response = handler.handle(get_request).await;
	assert!(response.is_ok());

	// Test POST request maps to create action.
	// Provide a valid JSON body so the real CRUD handler succeeds end-to-end
	// against the in-memory queryset (no database pool configured).
	let post_request = create_test_request_with_body(
		Method::POST,
		"http://example.com/test/",
		r#"{"id":1,"name":"alpha"}"#,
	);
	let response = handler.handle(post_request).await;
	assert!(
		response.is_ok(),
		"POST should map to create and succeed; response = {:?}",
		response
	);

	// Test unsupported method returns 405 Method Not Allowed with Allow header
	let put_request = create_test_request(Method::PUT, "http://example.com/test/");
	let response = handler.handle(put_request).await;
	assert!(response.is_ok());
	let response = response.unwrap();
	assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
	let allow_header = response.headers.get(hyper::header::ALLOW).unwrap();
	let allow_value = allow_header.to_str().unwrap();
	assert!(allow_value.contains("GET") || allow_value.contains("POST"));
}
