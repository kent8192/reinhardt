// Integration tests for reinhardt-rest API versioning strategies
//
// Tests cover:
// - URL path versioning
// - Accept header versioning
// - Query parameter versioning
// - Namespace versioning
// - Hostname versioning
// - Default version fallback behavior
// - Invalid version handling
// - VersioningMiddleware
// - ApiVersion struct
// - VersionedHandler / VersionedHandlerWrapper
// - VersionResponseBuilder
// - VersioningConfig / VersioningManager

use bytes::Bytes;
use hyper::{HeaderMap, Method, Uri, Version};
use reinhardt_http::{Handler, Middleware, Request, Response};
use reinhardt_rest::versioning::{
	AcceptHeaderVersioning, ApiVersion, BaseVersioning, ConfigurableVersionedHandler,
	HostNameVersioning, NamespaceVersioning, QueryParameterVersioning, RequestVersionExt,
	SimpleVersionedHandler, URLPathVersioning, VersionResponseBuilder, VersionedHandler,
	VersionedHandlerWrapper, VersioningMiddleware,
};
use rstest::rstest;
use std::sync::Arc;

// ── helpers ─────────────────────────────────────────────────────────────────

fn build_request(uri: &str, headers: Vec<(&str, &str)>) -> Request {
	let uri = uri.parse::<Uri>().unwrap();
	let mut header_map = HeaderMap::new();
	for (key, value) in headers {
		let name: hyper::header::HeaderName = key.parse().unwrap();
		header_map.insert(name, value.parse().unwrap());
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

/// Minimal Handler that always returns HTTP 200 OK.
struct OkHandler;

#[async_trait::async_trait]
impl Handler for OkHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::ok())
	}
}

// ── URLPathVersioning ────────────────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn url_path_versioning_extracts_v1() {
	// Arrange
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "2"]);
	let request = build_request("/v1/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "1");
}

#[rstest]
#[tokio::test]
async fn url_path_versioning_extracts_v2() {
	// Arrange
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "2"]);
	let request = build_request("/v2/items/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "2");
}

#[rstest]
#[tokio::test]
async fn url_path_versioning_returns_default_when_no_version_in_path() {
	// Arrange
	let versioning = URLPathVersioning::new().with_default_version("1.0");
	let request = build_request("/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "1.0");
}

#[rstest]
#[tokio::test]
async fn url_path_versioning_rejects_disallowed_version() {
	// Arrange
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "2"]);
	let request = build_request("/v99/users/", vec![]);

	// Act
	let result = versioning.determine_version(&request).await;

	// Assert
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn url_path_versioning_with_custom_pattern() {
	// Arrange
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_pattern("/api/v{version}/")
		.with_allowed_versions(vec!["1", "2"]);
	let request = build_request("/api/v2/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "2");
}

// ── AcceptHeaderVersioning ───────────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn accept_header_versioning_extracts_version_from_header() {
	// Arrange
	let versioning = AcceptHeaderVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);
	let request = build_request("/users/", vec![("accept", "application/json; version=2.0")]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "2.0");
}

#[rstest]
#[tokio::test]
async fn accept_header_versioning_returns_default_when_no_version_in_header() {
	// Arrange
	let versioning = AcceptHeaderVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);
	let request = build_request("/users/", vec![("accept", "application/json")]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "1.0");
}

#[rstest]
#[tokio::test]
async fn accept_header_versioning_rejects_disallowed_version() {
	// Arrange
	let versioning = AcceptHeaderVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);
	let request = build_request("/users/", vec![("accept", "application/json; version=9.0")]);

	// Act
	let result = versioning.determine_version(&request).await;

	// Assert
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn accept_header_versioning_custom_param_name() {
	// Arrange
	let versioning = AcceptHeaderVersioning::new()
		.with_default_version("1.0")
		.with_version_param("api-version")
		.with_allowed_versions(vec!["1.0", "2.0"]);
	let request = build_request(
		"/users/",
		vec![("accept", "application/json; api-version=2.0")],
	);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "2.0");
}

// ── QueryParameterVersioning ─────────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn query_param_versioning_extracts_version_from_query() {
	// Arrange
	let versioning = QueryParameterVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);
	let request = build_request("/users/?version=2.0", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "2.0");
}

#[rstest]
#[tokio::test]
async fn query_param_versioning_returns_default_when_param_absent() {
	// Arrange
	let versioning = QueryParameterVersioning::new().with_default_version("1.0");
	let request = build_request("/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "1.0");
}

#[rstest]
#[tokio::test]
async fn query_param_versioning_rejects_disallowed_version() {
	// Arrange
	let versioning = QueryParameterVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1.0", "2.0"]);
	let request = build_request("/users/?version=99.0", vec![]);

	// Act
	let result = versioning.determine_version(&request).await;

	// Assert
	assert!(result.is_err());
}

#[rstest]
#[tokio::test]
async fn query_param_versioning_custom_param_name() {
	// Arrange
	let versioning = QueryParameterVersioning::new()
		.with_default_version("1.0")
		.with_version_param("v")
		.with_allowed_versions(vec!["1.0", "2.0"]);
	let request = build_request("/users/?v=2.0", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "2.0");
}

// ── NamespaceVersioning ──────────────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn namespace_versioning_extracts_version_from_path() {
	// Arrange
	let versioning = NamespaceVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "2"]);
	let request = build_request("/v1/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "1");
}

#[rstest]
#[tokio::test]
async fn namespace_versioning_returns_default_when_no_match() {
	// Arrange
	let versioning = NamespaceVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "2"]);
	let request = build_request("/api/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "1.0");
}

#[rstest]
#[tokio::test]
async fn namespace_versioning_with_custom_pattern() {
	// Arrange
	let versioning = NamespaceVersioning::new()
		.with_default_version("1.0")
		.with_pattern("/api/v{version}/")
		.with_allowed_versions(vec!["1", "2"]);
	let request = build_request("/api/v2/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "2");
}

// ── HostNameVersioning ───────────────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn hostname_versioning_extracts_version_from_host_header() {
	// Arrange
	let versioning = HostNameVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["v1", "v2"]);
	let request = build_request("/users/", vec![("host", "v2.api.example.com")]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "v2");
}

#[rstest]
#[tokio::test]
async fn hostname_versioning_returns_default_when_no_host_header() {
	// Arrange
	let versioning = HostNameVersioning::new().with_default_version("1.0");
	let request = build_request("/users/", vec![]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "1.0");
}

#[rstest]
#[tokio::test]
async fn hostname_versioning_explicit_mapping_takes_precedence() {
	// Arrange
	let versioning = HostNameVersioning::new()
		.with_default_version("1.0")
		.with_hostname_pattern("v2", "api-v2.example.com");
	let request = build_request("/users/", vec![("host", "api-v2.example.com")]);

	// Act
	let version = versioning.determine_version(&request).await.unwrap();

	// Assert
	assert_eq!(version, "v2");
}

// ── VersioningMiddleware ─────────────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn versioning_middleware_sets_version_in_extensions() {
	// Arrange
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "2"]);
	let middleware = VersioningMiddleware::new(versioning);
	let handler = Arc::new(OkHandler);
	let request = build_request("/v2/users/", vec![]);

	// Act
	let result = middleware.process(request, handler).await;

	// Assert
	assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
async fn versioning_middleware_rejects_invalid_version() {
	// Arrange
	let versioning = URLPathVersioning::new()
		.with_default_version("1.0")
		.with_allowed_versions(vec!["1", "2"]);
	let middleware = VersioningMiddleware::new(versioning);
	let handler = Arc::new(OkHandler);
	let request = build_request("/v99/users/", vec![]);

	// Act
	let result = middleware.process(request, handler).await;

	// Assert
	assert!(result.is_err());
}

// ── ApiVersion / RequestVersionExt ───────────────────────────────────────────

#[rstest]
fn api_version_as_str() {
	// Arrange
	let version = ApiVersion::new("2.0".to_string());

	// Act
	let s = version.as_str();

	// Assert
	assert_eq!(s, "2.0");
}

#[rstest]
fn api_version_display() {
	// Arrange
	let version = ApiVersion::new("3.0".to_string());

	// Act
	let display = format!("{}", version);

	// Assert
	assert_eq!(display, "3.0");
}

#[rstest]
fn request_version_ext_returns_none_when_not_set() {
	// Arrange
	let request = build_request("/users/", vec![]);

	// Act
	let version = request.version();

	// Assert
	assert!(version.is_none());
}

#[rstest]
fn request_version_ext_version_or_returns_default() {
	// Arrange
	let request = build_request("/users/", vec![]);

	// Act
	let version = request.version_or("fallback");

	// Assert
	assert_eq!(version, "fallback");
}

// ── SimpleVersionedHandler ───────────────────────────────────────────────────

#[rstest]
fn simple_versioned_handler_supports_version() {
	// Arrange
	let handler = SimpleVersionedHandler::new()
		.with_version_response("1.0", r#"{"ok":true}"#)
		.with_version_response("2.0", r#"{"ok":true}"#);

	// Assert
	assert!(handler.supports_version("1.0"));
	assert!(handler.supports_version("2.0"));
	assert!(!handler.supports_version("3.0"));
}

#[rstest]
#[tokio::test]
async fn simple_versioned_handler_returns_version_specific_body() {
	// Arrange
	let handler = SimpleVersionedHandler::new()
		.with_version_response("1.0", r#"{"message":"v1"}"#)
		.with_version_response("2.0", r#"{"message":"v2"}"#);
	let request = build_request("/users/", vec![]);

	// Act
	let response = handler.handle_versioned(request, "2.0").await.unwrap();

	// Assert
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, r#"{"message":"v2"}"#);
}

// ── VersionedHandlerWrapper ──────────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn versioned_handler_wrapper_routes_by_url_version() {
	// Arrange
	let versioning = Arc::new(
		URLPathVersioning::new()
			.with_default_version("1.0")
			.with_allowed_versions(vec!["1", "2"]),
	);
	let inner = SimpleVersionedHandler::new()
		.with_version_response("1", r#"{"v":"1"}"#)
		.with_version_response("2", r#"{"v":"2"}"#);
	let wrapper = VersionedHandlerWrapper::new(Arc::new(inner), versioning);
	let request = build_request("/v2/users/", vec![]);

	// Act
	let response = wrapper.handle(request).await.unwrap();

	// Assert
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, r#"{"v":"2"}"#);
}

// ── VersionResponseBuilder ───────────────────────────────────────────────────

#[rstest]
fn version_response_builder_includes_version_field() {
	// Act
	let response = VersionResponseBuilder::new("2.0")
		.with_data(serde_json::json!({"key": "value"}))
		.build();

	// Assert
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let json: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(json["version"], "2.0");
}

#[rstest]
fn version_response_builder_with_field() {
	// Act
	let response = VersionResponseBuilder::new("1.0")
		.with_field("count", serde_json::json!(42))
		.build();

	// Assert
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	let json: serde_json::Value = serde_json::from_str(&body).unwrap();
	assert_eq!(json["data"]["count"], 42);
}

#[rstest]
fn version_response_builder_version_accessor() {
	// Arrange
	let builder = VersionResponseBuilder::new("3.0");

	// Act
	let version = builder.version();

	// Assert
	assert_eq!(version, "3.0");
}

// ── ConfigurableVersionedHandler ─────────────────────────────────────────────

#[rstest]
#[tokio::test]
async fn configurable_versioned_handler_routes_to_correct_handler() {
	// Arrange
	struct EchoVersionHandler {
		body: &'static str,
	}
	#[async_trait::async_trait]
	impl Handler for EchoVersionHandler {
		async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
			Ok(Response::ok().with_body(Bytes::from(self.body)))
		}
	}

	let handler = ConfigurableVersionedHandler::new()
		.with_version_handler("1.0", Box::new(EchoVersionHandler { body: "v1-body" }))
		.with_version_handler("2.0", Box::new(EchoVersionHandler { body: "v2-body" }));

	let request = build_request("/users/", vec![]);

	// Act
	let response = handler.handle_versioned(request, "2.0").await.unwrap();

	// Assert
	let body = String::from_utf8(response.body.to_vec()).unwrap();
	assert_eq!(body, "v2-body");
}

#[rstest]
#[tokio::test]
async fn configurable_versioned_handler_unknown_version_returns_error() {
	// Arrange
	let handler = ConfigurableVersionedHandler::new();
	let request = build_request("/users/", vec![]);

	// Act
	let result = handler.handle_versioned(request, "99.0").await;

	// Assert
	assert!(result.is_err());
}
