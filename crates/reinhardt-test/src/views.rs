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
	pub id: Option<i64>,
	pub name: String,
	pub slug: String,
	pub created_at: String,
}

crate::impl_test_model!(TestModel, i64, "test_models");

/// Test model for API view tests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiTestModel {
	pub id: Option<i64>,
	pub title: String,
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
	request.path_params = path_params;
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
	pub content: String,
	pub allowed_methods: Vec<Method>,
}

impl SimpleTestView {
	pub fn new(content: &str) -> Self {
		Self {
			content: content.to_string(),
			allowed_methods: vec![Method::GET],
		}
	}

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
	pub error_message: String,
	pub error_kind: ErrorKind,
}

pub enum ErrorKind {
	NotFound,
	Validation,
	Internal,
	Authentication,
	Authorization,
}

impl ErrorTestView {
	pub fn new(error_message: String, error_kind: ErrorKind) -> Self {
		Self {
			error_message,
			error_kind,
		}
	}

	pub fn not_found(message: impl Into<String>) -> Self {
		Self::new(message.into(), ErrorKind::NotFound)
	}

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
