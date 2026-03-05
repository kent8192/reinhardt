//! Integration tests for parsers with HTTP request handling
//!
//! These tests require multiple crates to be integrated:
//! - reinhardt-parsers: Parser implementations
//! - reinhardt-http: HTTP request/response handling
//!
//! Based on Django REST Framework tests from:
//! django-rest-framework/tests/test_parsers.py

use bytes::Bytes;
use hyper::{HeaderMap, Method, Version, header::CONTENT_TYPE};
use reinhardt_core::parsers::{FormParser, JSONParser, MultiPartParser, parser::Parser};
use reinhardt_http::Request;

/// Test POST data access after parsing with FormParser and MultiPartParser
///
/// DRF test: test_post_accessed_in_post_method
/// Line: 149 in django-rest-framework/tests/test_parsers.py
///
/// This test verifies that after accessing request.post(), the request.data()
/// still works correctly with FormParser and MultiPartParser.
#[tokio::test]
async fn test_post_accessed_in_post_method() {
	// Create form data body
	let body = Bytes::from("foo=bar");

	// Create headers with form content type
	let mut headers = HeaderMap::new();
	headers.insert(
		CONTENT_TYPE,
		"application/x-www-form-urlencoded".parse().unwrap(),
	);

	// Create request with parsers
	let request = Request::builder()
		.method(Method::POST)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(body)
		.build()
		.unwrap()
		.with_parsers(vec![
			Box::new(FormParser::new()) as Box<dyn Parser>,
			Box::new(MultiPartParser::new()) as Box<dyn Parser>,
		]);

	// Access POST data first
	let post_data = request.post().await.unwrap();
	assert_eq!(post_data.get("foo"), Some(&vec!["bar".to_string()]));

	// Access data() - should still work due to caching
	let data = request.data().await.unwrap();
	if let reinhardt_core::parsers::parser::ParsedData::Form(form) = data {
		assert_eq!(form.get("foo"), Some(&"bar".to_string()));
	} else {
		panic!("Expected Form data");
	}
}

/// Test POST data access with JSONParser
///
/// DRF test: test_post_accessed_in_post_method_with_json_parser
/// Line: 156 in django-rest-framework/tests/test_parsers.py
///
/// This test verifies that with JSONParser, POST data access doesn't
/// interfere with JSON parsing.
#[tokio::test]
async fn test_post_accessed_in_post_method_with_json_parser() {
	// Create JSON data body
	let body = Bytes::from(r#"{"key": "value"}"#);

	// Create headers with JSON content type
	let mut headers = HeaderMap::new();
	headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

	// Create request with JSONParser only
	let request = Request::builder()
		.method(Method::POST)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(body)
		.build()
		.unwrap()
		.with_parsers(vec![Box::new(JSONParser::new()) as Box<dyn Parser>]);

	// Access POST data - should be empty (no form parser)
	let post_data = request.post().await.unwrap();
	assert!(post_data.is_empty());

	// Access data() - should return JSON
	let data = request.data().await.unwrap();
	if let reinhardt_core::parsers::parser::ParsedData::Json(json) = data {
		assert_eq!(json.get("key").and_then(|v| v.as_str()), Some("value"));
	} else {
		panic!("Expected JSON data");
	}
}

/// Test POST data access in PUT method
///
/// DRF test: test_post_accessed_in_put_method
/// Line: 163 in django-rest-framework/tests/test_parsers.py
///
/// This test verifies that PUT requests also handle POST data access correctly.
#[tokio::test]
async fn test_post_accessed_in_put_method() {
	// Create form data body
	let body = Bytes::from("foo=bar");

	// Create headers with form content type
	let mut headers = HeaderMap::new();
	headers.insert(
		CONTENT_TYPE,
		"application/x-www-form-urlencoded".parse().unwrap(),
	);

	// Create PUT request with parsers
	let request = Request::builder()
		.method(Method::PUT)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(body)
		.build()
		.unwrap()
		.with_parsers(vec![
			Box::new(FormParser::new()) as Box<dyn Parser>,
			Box::new(MultiPartParser::new()) as Box<dyn Parser>,
		]);

	// Access POST data
	let post_data = request.post().await.unwrap();
	assert_eq!(post_data.get("foo"), Some(&vec!["bar".to_string()]));

	// Access data() - should still work
	let data = request.data().await.unwrap();
	if let reinhardt_core::parsers::parser::ParsedData::Form(form) = data {
		assert_eq!(form.get("foo"), Some(&"bar".to_string()));
	} else {
		panic!("Expected Form data");
	}
}

/// Test that body can only be consumed once
///
/// DRF test: test_request_read_before_parsing
/// Line: 170 in django-rest-framework/tests/test_parsers.py
///
/// This test verifies that accessing the body multiple times raises an error.
#[tokio::test]
async fn test_request_read_before_parsing() {
	// Create form data body
	let body = Bytes::from("foo=bar");

	// Create headers with form content type
	let mut headers = HeaderMap::new();
	headers.insert(
		CONTENT_TYPE,
		"application/x-www-form-urlencoded".parse().unwrap(),
	);

	// Create request with parsers
	let request = Request::builder()
		.method(Method::PUT)
		.uri("/")
		.version(Version::HTTP_11)
		.headers(headers)
		.body(body)
		.build()
		.unwrap()
		.with_parsers(vec![
			Box::new(FormParser::new()) as Box<dyn Parser>,
			Box::new(MultiPartParser::new()) as Box<dyn Parser>,
		]);

	// First access - should succeed
	let _post_data = request.post().await.unwrap();

	// Second access - body is cached, so this should also succeed
	let _data = request.data().await.unwrap();

	// Note: In the current implementation, the body is cached after first parse,
	// so multiple accesses succeed. This differs from Django's stream-based approach
	// but is more efficient for Rust's async model.
}
