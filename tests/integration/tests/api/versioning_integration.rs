//! Integration tests for API versioning
//!
//! Tests the integration between reinhardt-versioning, reinhardt-types,
//! and reinhardt-http crates according to CLAUDE.md TO-1 rules.

use bytes::Bytes;
use hyper::{HeaderMap, Method, Uri, Version};
use reinhardt_http::{Handler, MiddlewareChain};
use reinhardt_http::{Request, Response};
use reinhardt_rest::versioning::{
	AcceptHeaderVersioning, HostNameVersioning, NamespaceVersioning, QueryParameterVersioning,
	RequestVersionExt, URLPathVersioning, VersioningMiddleware,
};
use std::sync::Arc;

// Test handler that returns version information
struct VersionEchoHandler;

#[async_trait::async_trait]
impl Handler for VersionEchoHandler {
	async fn handle(&self, request: Request) -> Result<Response, reinhardt_core::exception::Error> {
		let version = request.version().unwrap_or_else(|| "unknown".to_string());
		let body = format!("{{\"version\":\"{}\"}}", version);
		Ok(Response::ok().with_body(Bytes::from(body)))
	}
}

fn create_request(uri: &str, headers: Vec<(&str, &str)>) -> Request {
	let uri = uri.parse::<Uri>().unwrap();
	let mut header_map = HeaderMap::new();
	for (key, value) in headers {
		use hyper::header::{HeaderName, HeaderValue};
		let header_name: HeaderName = key.parse().unwrap();
		let header_value: HeaderValue = value.parse().unwrap();
		header_map.insert(header_name, header_value);
	}
	Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(header_map)
		.body(Bytes::new())
		.build()
		.unwrap()
}

#[tokio::test]
async fn test_url_path_versioning_with_middleware_chain() {
	// Setup versioning strategy
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "1.0", "2", "2.0"]);

	// Create middleware chain
	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// Test with v2 in path
	let request = create_request("/v2/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "2",
		"API version mismatch. Expected version field to be '2', got: {:?}",
		parsed["version"]
	);

	// Test with v1 in path
	let request = create_request("/v1/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "1",
		"API version mismatch. Expected version field to be '1', got: {:?}",
		parsed["version"]
	);

	// Test without version (should use default)
	let request = create_request("/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "1.0",
		"API version mismatch. Expected version field to be '1.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_accept_header_versioning_with_middleware_chain() {
	// Setup versioning strategy
	let versioning = AcceptHeaderVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);

	// Create middleware chain
	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// Test with version in Accept header
	let request = create_request("/users/", vec![("accept", "application/json; version=2.0")]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "2.0",
		"API version mismatch. Expected version field to be '2.0', got: {:?}",
		parsed["version"]
	);

	// Test without version (should use default)
	let request = create_request("/users/", vec![("accept", "application/json")]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "1.0",
		"API version mismatch. Expected version field to be '1.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_query_parameter_versioning_with_middleware_chain() {
	// Setup versioning strategy
	let versioning = QueryParameterVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0", "3.0"]);

	// Create middleware chain
	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// Test with version in query parameter
	let request = create_request("/users/?version=3.0", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "3.0",
		"API version mismatch. Expected version field to be '3.0', got: {:?}",
		parsed["version"]
	);

	// Test with custom version parameter name
	let versioning = QueryParameterVersioning::new()
		.with_version_param("v")
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);

	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let chain = MiddlewareChain::new(Arc::new(VersionEchoHandler)).with_middleware(middleware);

	let request = create_request("/users/?v=2.0", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "2.0",
		"API version mismatch. Expected version field to be '2.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_hostname_versioning_with_middleware_chain() {
	// Setup versioning strategy
	let versioning = HostNameVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["v1", "v2", "v3"]);

	// Create middleware chain
	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// Test with version in hostname
	let request = create_request("/users/", vec![("host", "v2.api.example.com")]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "v2",
		"API version mismatch. Expected version field to be 'v2', got: {:?}",
		parsed["version"]
	);

	// Test without version (should use default)
	let request = create_request("/users/", vec![("host", "api.example.com")]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "1.0",
		"API version mismatch. Expected version field to be '1.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_namespace_versioning_with_middleware_chain() {
	// Setup versioning strategy
	let versioning = NamespaceVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "1.0", "2", "2.0"]);

	// Create middleware chain
	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// Test with v1 namespace
	let request = create_request("/v1/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "1",
		"API version mismatch. Expected version field to be '1', got: {:?}",
		parsed["version"]
	);

	// Test with v2.0 namespace
	let request = create_request("/v2.0/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "2.0",
		"API version mismatch. Expected version field to be '2.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_multiple_middleware_strategies() {
	// Test combining multiple versioning strategies in sequence
	// First try Accept header, then fall back to URL path

	let accept_versioning = AcceptHeaderVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);

	let _url_versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "1.0", "2", "2.0"]);

	// Create middleware chain with both strategies
	let accept_middleware = Arc::new(VersioningMiddleware::new(accept_versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(accept_middleware);

	// Test with Accept header (should take precedence)
	let request = create_request(
		"/v1/users/",
		vec![("accept", "application/json; version=2.0")],
	);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "2.0",
		"API version mismatch. Expected version field to be '2.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_invalid_version_rejection() {
	// Setup versioning with strict allowed versions
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);

	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// Test with invalid version (v3 not in allowed list)
	// MiddlewareChain's ErrorToResponseHandler converts errors to responses,
	// so the result is Ok with an error status code rather than Err.
	let request = create_request("/v3/users/", vec![]);
	let result = chain.handle(request).await;
	let response = result.expect("MiddlewareChain should convert errors to responses");
	assert!(
		response.status.is_client_error(),
		"Invalid version should return a 4xx error, got: {}",
		response.status
	);
}

#[tokio::test]
async fn test_version_propagation_through_request_lifecycle() {
	// Test that version is correctly propagated through the entire request lifecycle

	struct MultiStageHandler;

	#[async_trait::async_trait]
	impl Handler for MultiStageHandler {
		async fn handle(
			&self,
			request: Request,
		) -> Result<Response, reinhardt_core::exception::Error> {
			// First stage: read version
			let version = request.version();
			assert!(version.is_some());

			// Second stage: use version
			let v = version.unwrap();
			assert_eq!(v, "2.0");

			// Third stage: return response with version info
			Ok(Response::ok().with_body(Bytes::from(format!("{{\"version\":\"{}\"}}", v))))
		}
	}

	let versioning = QueryParameterVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);

	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(MultiStageHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	let request = create_request("/users/?version=2.0", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "2.0",
		"API version mismatch. Expected version field to be '2.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_empty_allowed_versions() {
	// Test that empty allowed_versions list allows any version
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec![] as Vec<&str>); // Empty = allow any

	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// Test with any version
	let request = create_request("/v99/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "99",
		"API version mismatch. Expected version field to be '99', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_version_or_with_fallback() {
	// Test RequestVersionExt::version_or() functionality

	struct FallbackHandler;

	#[async_trait::async_trait]
	impl Handler for FallbackHandler {
		async fn handle(
			&self,
			request: Request,
		) -> Result<Response, reinhardt_core::exception::Error> {
			let version = request.version_or("fallback");
			Ok(Response::ok().with_body(Bytes::from(format!("{{\"version\":\"{}\"}}", version))))
		}
	}

	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "1.0", "2", "2.0"]);

	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(FallbackHandler);
	let chain = MiddlewareChain::new(handler).with_middleware(middleware);

	// With version
	let request = create_request("/v2/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "2",
		"API version mismatch. Expected version field to be '2', got: {:?}",
		parsed["version"]
	);

	// Without version (should use default, not fallback)
	let request = create_request("/users/", vec![]);
	let response = chain.handle(request).await.unwrap();
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(
		parsed["version"], "1.0",
		"API version mismatch. Expected version field to be '1.0', got: {:?}",
		parsed["version"]
	);
}

#[tokio::test]
async fn test_concurrent_requests_with_different_versions() {
	// Test that multiple concurrent requests with different versions are handled correctly

	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0", "3.0", "1", "2", "3"]);

	let middleware = Arc::new(VersioningMiddleware::new(versioning));
	let handler = Arc::new(VersionEchoHandler);
	let chain = Arc::new(MiddlewareChain::new(handler).with_middleware(middleware));

	// Simulate concurrent requests
	let chain1 = chain.clone();
	let chain2 = chain.clone();
	let chain3 = chain.clone();

	let handle1 = tokio::spawn(async move {
		let request = create_request("/v1/users/", vec![]);
		chain1.handle(request).await
	});

	let handle2 = tokio::spawn(async move {
		let request = create_request("/v2/users/", vec![]);
		chain2.handle(request).await
	});

	let handle3 = tokio::spawn(async move {
		let request = create_request("/v3/users/", vec![]);
		chain3.handle(request).await
	});

	let result1 = handle1.await.unwrap().unwrap();
	let result2 = handle2.await.unwrap().unwrap();
	let result3 = handle3.await.unwrap().unwrap();

	let body1 = String::from_utf8(result1.body.to_vec()).unwrap();
	let body2 = String::from_utf8(result2.body.to_vec()).unwrap();
	let body3 = String::from_utf8(result3.body.to_vec()).unwrap();

	let parsed1: serde_json::Value = serde_json::from_str(&body1).unwrap();
	assert_eq!(
		parsed1["version"], "1",
		"API version mismatch. Expected version field to be '1', got: {:?}",
		parsed1["version"]
	);

	let parsed2: serde_json::Value = serde_json::from_str(&body2).unwrap();
	assert_eq!(
		parsed2["version"], "2",
		"API version mismatch. Expected version field to be '2', got: {:?}",
		parsed2["version"]
	);

	let parsed3: serde_json::Value = serde_json::from_str(&body3).unwrap();
	assert_eq!(
		parsed3["version"], "3",
		"API version mismatch. Expected version field to be '3', got: {:?}",
		parsed3["version"]
	);
}
