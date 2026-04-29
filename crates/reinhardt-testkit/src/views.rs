//! View test utilities for Reinhardt framework
//!
//! Provides test models, request builders, and test views for view testing.

use bytes::Bytes;
use hyper::{HeaderMap, Method, Uri, Version};
use reinhardt_http::{Error, Request, Response, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Test Models
// ============================================================================

/// Test model for view tests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestModel {
	/// Primary key identifier.
	pub id: Option<i64>,
	/// Display name of the test model.
	pub name: String,
	/// URL-safe slug derived from the name.
	pub slug: String,
	/// ISO 8601 timestamp of creation.
	pub created_at: String,
}

crate::impl_test_model!(TestModel, i64, "test_models");

/// Test model for API view tests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiTestModel {
	/// Primary key identifier.
	pub id: Option<i64>,
	/// Title of the API test model entry.
	pub title: String,
	/// Body content of the API test model entry.
	pub content: String,
}

crate::impl_test_model!(ApiTestModel, i64, "api_test_models");

// ============================================================================
// Request Creation Functions
// ============================================================================

/// Create a test request with the given parameters
pub fn create_request(
	method: Method,
	path: &str,
	query_params: Option<HashMap<String, String>>,
	headers: Option<HeaderMap>,
	body: Option<Bytes>,
) -> Request {
	// Fixes #880: URL-encode query parameter keys and values to prevent injection
	let uri_str = if let Some(ref params) = query_params {
		let query = params
			.iter()
			.map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
			.collect::<Vec<_>>()
			.join("&");
		format!("{}?{}", path, query)
	} else {
		path.to_string()
	};

	let uri = uri_str.parse::<Uri>().unwrap();
	Request::builder()
		.method(method)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers.unwrap_or_default())
		.body(body.unwrap_or_default())
		.build()
		.expect("Failed to build request")
}

/// Create a test request with path parameters
pub fn create_request_with_path_params(
	method: Method,
	path: &str,
	path_params: HashMap<String, String>,
	query_params: Option<HashMap<String, String>>,
	headers: Option<HeaderMap>,
	body: Option<Bytes>,
) -> Request {
	let mut request = create_request(method, path, query_params, headers, body);
	// Convert via `Into` since `Request.path_params` is a `PathParams` that
	// preserves URL declaration order. `HashMap` ordering is non-deterministic
	// — callers that rely on tuple-extractor ordering should use
	// `Request::builder().path_params(...)` with a `Vec<(String, String)>`
	// or `PathParams` directly.
	request.path_params = path_params.into();
	request
}

/// Create a test request with headers
pub fn create_request_with_headers(
	method: Method,
	path: &str,
	headers: HashMap<String, String>,
	body: Option<Bytes>,
) -> Request {
	let mut header_map = HeaderMap::new();
	for (key, value) in headers {
		if let (Ok(header_name), Ok(header_value)) = (
			hyper::header::HeaderName::from_bytes(key.as_bytes()),
			hyper::header::HeaderValue::from_str(&value),
		) {
			header_map.insert(header_name, header_value);
		}
	}

	create_request(method, path, None, Some(header_map), body)
}

/// Create a test request with JSON body
pub fn create_json_request(method: Method, path: &str, json_data: &serde_json::Value) -> Request {
	let body = Bytes::from(serde_json::to_vec(json_data).unwrap());
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		hyper::header::HeaderValue::from_static("application/json"),
	);

	create_request(method, path, None, Some(headers), Some(body))
}

// ============================================================================
// Test Data Generation
// ============================================================================

/// Create test objects for list views
pub fn create_test_objects() -> Vec<TestModel> {
	vec![
		TestModel {
			id: Some(1),
			name: "First Object".to_string(),
			slug: "first-object".to_string(),
			created_at: "2023-01-01T00:00:00Z".to_string(),
		},
		TestModel {
			id: Some(2),
			name: "Second Object".to_string(),
			slug: "second-object".to_string(),
			created_at: "2023-01-02T00:00:00Z".to_string(),
		},
		TestModel {
			id: Some(3),
			name: "Third Object".to_string(),
			slug: "third-object".to_string(),
			created_at: "2023-01-03T00:00:00Z".to_string(),
		},
	]
}

/// Create test objects for API views
pub fn create_api_test_objects() -> Vec<ApiTestModel> {
	vec![
		ApiTestModel {
			id: Some(1),
			title: "First Post".to_string(),
			content: "This is the first post content".to_string(),
		},
		ApiTestModel {
			id: Some(2),
			title: "Second Post".to_string(),
			content: "This is the second post content".to_string(),
		},
		ApiTestModel {
			id: Some(3),
			title: "Third Post".to_string(),
			content: "This is the third post content".to_string(),
		},
	]
}

/// Create a large set of test objects for pagination testing
pub fn create_large_test_objects(count: usize) -> Vec<TestModel> {
	(0..count)
		.map(|i| TestModel {
			id: Some(i as i64),
			name: format!("Object {}", i),
			slug: format!("object-{}", i),
			created_at: format!("2023-01-{:02}T00:00:00Z", (i % 30) + 1),
		})
		.collect()
}

// ============================================================================
// Test Views
// ============================================================================

/// Create a simple view for testing basic functionality
pub struct SimpleTestView {
	/// The response body content.
	pub content: String,
	/// HTTP methods that this view accepts.
	pub allowed_methods: Vec<Method>,
}

impl SimpleTestView {
	/// Create a new `SimpleTestView` with the given content, accepting only GET.
	pub fn new(content: &str) -> Self {
		Self {
			content: content.to_string(),
			allowed_methods: vec![Method::GET],
		}
	}

	/// Set the allowed HTTP methods for this view.
	pub fn with_methods(mut self, methods: Vec<Method>) -> Self {
		self.allowed_methods = methods;
		self
	}
}

#[async_trait::async_trait]
impl reinhardt_views::View for SimpleTestView {
	async fn dispatch(&self, request: Request) -> Result<Response> {
		if !self.allowed_methods.contains(&request.method) {
			return Err(Error::Validation(format!(
				"Method {} not allowed",
				request.method
			)));
		}

		Ok(Response::ok().with_body(self.content.clone().into_bytes()))
	}
}

/// Create a view that always returns an error for testing error handling
pub struct ErrorTestView {
	/// The error message to return.
	pub error_message: String,
	/// The kind of error to return.
	pub error_kind: ErrorKind,
}

/// Kind of error that an `ErrorTestView` will produce.
pub enum ErrorKind {
	/// HTTP 404 Not Found error.
	NotFound,
	/// Validation error (e.g., invalid input).
	Validation,
	/// Internal server error.
	Internal,
	/// Authentication required error.
	Authentication,
	/// Authorization denied error.
	Authorization,
}

impl ErrorTestView {
	/// Create a new `ErrorTestView` with the given message and error kind.
	pub fn new(error_message: String, error_kind: ErrorKind) -> Self {
		Self {
			error_message,
			error_kind,
		}
	}

	/// Create an `ErrorTestView` that returns a 404 Not Found error.
	pub fn not_found(message: impl Into<String>) -> Self {
		Self::new(message.into(), ErrorKind::NotFound)
	}

	/// Create an `ErrorTestView` that returns a validation error.
	pub fn validation(message: impl Into<String>) -> Self {
		Self::new(message.into(), ErrorKind::Validation)
	}
}

#[async_trait::async_trait]
impl reinhardt_views::View for ErrorTestView {
	async fn dispatch(&self, _request: Request) -> Result<Response> {
		match self.error_kind {
			ErrorKind::NotFound => Err(Error::NotFound(self.error_message.clone())),
			ErrorKind::Validation => Err(Error::Validation(self.error_message.clone())),
			ErrorKind::Internal => Err(Error::Internal(self.error_message.clone())),
			ErrorKind::Authentication => Err(Error::Authentication(self.error_message.clone())),
			ErrorKind::Authorization => Err(Error::Authorization(self.error_message.clone())),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ========================================================================
	// create_request tests
	// ========================================================================

	#[rstest]
	fn test_create_request_basic_get() {
		// Arrange
		let method = Method::GET;
		let path = "/api/items/";

		// Act
		let request = create_request(method.clone(), path, None, None, None);

		// Assert
		assert_eq!(request.method, Method::GET);
		assert_eq!(request.uri.path(), "/api/items/");
		assert!(request.uri.query().is_none());
	}

	#[rstest]
	fn test_create_request_with_query_params() {
		// Arrange
		let method = Method::GET;
		let path = "/api/items/";
		let mut params = HashMap::new();
		params.insert("page".to_string(), "2".to_string());
		params.insert("limit".to_string(), "10".to_string());

		// Act
		let request = create_request(method, path, Some(params), None, None);

		// Assert
		let query = request.uri.query().expect("query string should be present");
		assert!(query.contains("page=2"));
		assert!(query.contains("limit=10"));
	}

	#[rstest]
	fn test_create_request_with_body() {
		// Arrange
		let method = Method::POST;
		let path = "/api/items/";
		let body = Bytes::from(b"hello body".to_vec());

		// Act
		let request = create_request(method, path, None, None, Some(body.clone()));

		// Assert
		assert_eq!(request.method, Method::POST);
		assert_eq!(request.body(), &body);
	}

	#[rstest]
	fn test_create_request_with_headers_param() {
		// Arrange
		let method = Method::GET;
		let path = "/api/items/";
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::ACCEPT,
			hyper::header::HeaderValue::from_static("application/json"),
		);

		// Act
		let request = create_request(method, path, None, Some(headers), None);

		// Assert
		assert_eq!(
			request.headers.get(hyper::header::ACCEPT).unwrap(),
			"application/json"
		);
	}

	#[rstest]
	fn test_create_request_with_path_params() {
		// Arrange
		let method = Method::GET;
		let path = "/api/items/1/";
		let mut path_params = HashMap::new();
		path_params.insert("id".to_string(), "1".to_string());

		// Act
		let request =
			create_request_with_path_params(method, path, path_params.clone(), None, None, None);

		// Assert
		assert_eq!(request.path_params.get("id").unwrap(), "1");
		assert_eq!(request.path_params.len(), 1);
	}

	#[rstest]
	fn test_create_request_with_headers_fn() {
		// Arrange
		let method = Method::POST;
		let path = "/api/items/";
		let mut headers = HashMap::new();
		headers.insert("x-custom-header".to_string(), "custom-value".to_string());
		headers.insert("authorization".to_string(), "Bearer token123".to_string());

		// Act
		let request = create_request_with_headers(method, path, headers, None);

		// Assert
		assert_eq!(
			request.headers.get("x-custom-header").unwrap(),
			"custom-value"
		);
		assert_eq!(
			request.headers.get("authorization").unwrap(),
			"Bearer token123"
		);
	}

	#[rstest]
	fn test_create_json_request() {
		// Arrange
		let method = Method::POST;
		let path = "/api/items/";
		let json_data = serde_json::json!({"name": "test", "value": 42});

		// Act
		let request = create_json_request(method, path, &json_data);

		// Assert
		assert_eq!(request.method, Method::POST);
		assert_eq!(
			request.headers.get(hyper::header::CONTENT_TYPE).unwrap(),
			"application/json"
		);
		let body_bytes = request.body();
		let parsed: serde_json::Value = serde_json::from_slice(body_bytes).unwrap();
		assert_eq!(parsed, json_data);
	}

	// ========================================================================
	// Test data generation tests
	// ========================================================================

	#[rstest]
	fn test_create_test_objects_count() {
		// Arrange & Act
		let objects = create_test_objects();

		// Assert
		assert_eq!(objects.len(), 3);
	}

	#[rstest]
	fn test_create_test_objects_fields() {
		// Arrange & Act
		let objects = create_test_objects();

		// Assert
		for (i, obj) in objects.iter().enumerate() {
			assert_eq!(obj.id, Some((i + 1) as i64));
			assert!(
				!obj.name.is_empty(),
				"name should not be empty for object {}",
				i
			);
			assert!(
				!obj.slug.is_empty(),
				"slug should not be empty for object {}",
				i
			);
			assert!(
				!obj.created_at.is_empty(),
				"created_at should not be empty for object {}",
				i
			);
		}
	}

	#[rstest]
	fn test_create_api_test_objects_count() {
		// Arrange & Act
		let objects = create_api_test_objects();

		// Assert
		assert_eq!(objects.len(), 3);
	}

	#[rstest]
	fn test_create_large_test_objects_100() {
		// Arrange
		let count = 100;

		// Act
		let objects = create_large_test_objects(count);

		// Assert
		assert_eq!(objects.len(), 100);
		for (i, obj) in objects.iter().enumerate() {
			assert_eq!(obj.id, Some(i as i64));
			assert_eq!(obj.name, format!("Object {}", i));
			assert_eq!(obj.slug, format!("object-{}", i));
		}
	}

	// ========================================================================
	// SimpleTestView tests
	// ========================================================================

	#[rstest]
	#[tokio::test]
	async fn test_simple_test_view_new_dispatch_get() {
		// Arrange
		let view = SimpleTestView::new("Hello, World!");
		let request = create_request(Method::GET, "/test/", None, None, None);

		// Act
		let response = reinhardt_views::View::dispatch(&view, request).await;

		// Assert
		assert!(response.is_ok());
		let resp = response.unwrap();
		assert_eq!(resp.body.as_ref(), b"Hello, World!");
	}

	#[rstest]
	fn test_simple_test_view_with_methods() {
		// Arrange & Act
		let view = SimpleTestView::new("content").with_methods(vec![
			Method::GET,
			Method::POST,
			Method::PUT,
		]);

		// Assert
		assert_eq!(view.allowed_methods.len(), 3);
		assert!(view.allowed_methods.contains(&Method::GET));
		assert!(view.allowed_methods.contains(&Method::POST));
		assert!(view.allowed_methods.contains(&Method::PUT));
	}

	#[rstest]
	#[tokio::test]
	async fn test_simple_test_view_method_not_allowed() {
		// Arrange
		let view = SimpleTestView::new("content");
		let request = create_request(Method::POST, "/test/", None, None, None);

		// Act
		let result = reinhardt_views::View::dispatch(&view, request).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		let err_msg = err.to_string();
		assert!(
			err_msg.contains("Method POST not allowed"),
			"Expected method not allowed error, got: {}",
			err_msg
		);
	}

	// ========================================================================
	// ErrorTestView tests
	// ========================================================================

	#[rstest]
	#[tokio::test]
	async fn test_error_test_view_not_found() {
		// Arrange
		let view = ErrorTestView::not_found("Resource not found");
		let request = create_request(Method::GET, "/missing/", None, None, None);

		// Act
		let result = reinhardt_views::View::dispatch(&view, request).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("Resource not found"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_error_test_view_validation() {
		// Arrange
		let view = ErrorTestView::validation("Invalid input data");
		let request = create_request(Method::POST, "/validate/", None, None, None);

		// Act
		let result = reinhardt_views::View::dispatch(&view, request).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("Invalid input data"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_error_test_view_internal() {
		// Arrange
		let view = ErrorTestView::new("Server failure".to_string(), ErrorKind::Internal);
		let request = create_request(Method::GET, "/error/", None, None, None);

		// Act
		let result = reinhardt_views::View::dispatch(&view, request).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("Server failure"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_error_test_view_authentication() {
		// Arrange
		let view = ErrorTestView::new("Not authenticated".to_string(), ErrorKind::Authentication);
		let request = create_request(Method::GET, "/protected/", None, None, None);

		// Act
		let result = reinhardt_views::View::dispatch(&view, request).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("Not authenticated"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_error_test_view_authorization() {
		// Arrange
		let view = ErrorTestView::new("Forbidden".to_string(), ErrorKind::Authorization);
		let request = create_request(Method::GET, "/admin/", None, None, None);

		// Act
		let result = reinhardt_views::View::dispatch(&view, request).await;

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(err.to_string().contains("Forbidden"));
	}

	// ========================================================================
	// Edge case tests
	// ========================================================================

	#[rstest]
	fn test_create_request_empty_query_params() {
		// Arrange
		let params: HashMap<String, String> = HashMap::new();

		// Act
		let request = create_request(Method::GET, "/api/items/", Some(params), None, None);

		// Assert
		// Empty params still produces a "?" but no key=value pairs
		let uri_str = request.uri.to_string();
		assert!(
			uri_str == "/api/items/?" || uri_str == "/api/items/",
			"URI should be path with empty or no query: {}",
			uri_str
		);
	}

	#[rstest]
	fn test_create_request_query_special_chars() {
		// Arrange
		let mut params = HashMap::new();
		params.insert("search".to_string(), "hello world&foo=bar".to_string());

		// Act
		let request = create_request(Method::GET, "/api/search/", Some(params), None, None);

		// Assert
		let query = request.uri.query().expect("query string should be present");
		// URL-encoded: space becomes %20, & becomes %26, = becomes %3D
		assert!(
			query.contains("hello%20world%26foo%3Dbar"),
			"Special characters should be URL-encoded, got: {}",
			query
		);
	}

	#[rstest]
	fn test_create_large_test_objects_zero() {
		// Arrange & Act
		let objects = create_large_test_objects(0);

		// Assert
		assert!(objects.is_empty());
	}

	#[rstest]
	fn test_test_model_serialization() {
		// Arrange
		let model = TestModel {
			id: Some(42),
			name: "Test Item".to_string(),
			slug: "test-item".to_string(),
			created_at: "2023-06-15T12:00:00Z".to_string(),
		};

		// Act
		let json = serde_json::to_string(&model).unwrap();
		let deserialized: TestModel = serde_json::from_str(&json).unwrap();

		// Assert
		assert_eq!(model, deserialized);
	}
}
