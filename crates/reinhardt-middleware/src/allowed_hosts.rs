//! Allowed Hosts Middleware
//!
//! Validates the Host header against a configurable list of allowed hosts.
//! Prevents HTTP Host header attacks by rejecting requests with unrecognized hosts.

use async_trait::async_trait;
use hyper::StatusCode;
use reinhardt_conf::Settings;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// Configuration for allowed host validation
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct AllowedHostsConfig {
	/// List of allowed host patterns.
	///
	/// Supports exact matches (e.g., `"example.com"`) and wildcard patterns
	/// (e.g., `"*.example.com"` matches `sub.example.com`).
	/// An empty list allows all hosts (Django-compatible behavior).
	pub allowed_hosts: Vec<String>,
}

impl AllowedHostsConfig {
	/// Create a new configuration with the given allowed hosts
	pub fn new(allowed_hosts: Vec<String>) -> Self {
		Self { allowed_hosts }
	}

	/// Create from application `Settings`
	///
	/// Maps `Settings.allowed_hosts` to `AllowedHostsConfig.allowed_hosts`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::Settings;
	/// use reinhardt_middleware::allowed_hosts::AllowedHostsConfig;
	/// use std::path::PathBuf;
	///
	/// let mut settings = Settings::new(PathBuf::from("/app"), "secret".to_string());
	/// settings.allowed_hosts = vec!["example.com".to_string(), "*.example.com".to_string()];
	///
	/// let config = AllowedHostsConfig::from_settings(&settings);
	/// assert_eq!(config.allowed_hosts.len(), 2);
	/// ```
	pub fn from_settings(settings: &Settings) -> Self {
		Self {
			allowed_hosts: settings.allowed_hosts.clone(),
		}
	}

	/// Check if a host is allowed by this configuration
	fn is_host_allowed(&self, host: &str) -> bool {
		// Empty allowed_hosts means allow all (Django-compatible)
		if self.allowed_hosts.is_empty() {
			return true;
		}

		let host_lower = host.to_lowercase();
		// Strip port if present
		let host_without_port = host_lower.split(':').next().unwrap_or(&host_lower);

		for pattern in &self.allowed_hosts {
			let pattern_lower = pattern.to_lowercase();

			if pattern_lower.starts_with("*.") {
				// Wildcard pattern: *.example.com matches sub.example.com
				let suffix = &pattern_lower[1..]; // ".example.com"
				if host_without_port.ends_with(suffix) && host_without_port.len() > suffix.len() {
					return true;
				}
			} else if host_without_port == pattern_lower {
				// Exact match
				return true;
			}
		}

		false
	}
}

/// Middleware that validates the Host header against a list of allowed hosts
///
/// Prevents HTTP Host header attacks by rejecting requests whose Host header
/// does not match any allowed pattern. Returns HTTP 400 Bad Request for
/// disallowed hosts.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_middleware::AllowedHostsMiddleware;
/// use reinhardt_middleware::allowed_hosts::AllowedHostsConfig;
///
/// let config = AllowedHostsConfig::new(vec![
///     "example.com".to_string(),
///     "*.example.com".to_string(),
/// ]);
/// let middleware = AllowedHostsMiddleware::new(config);
/// ```
pub struct AllowedHostsMiddleware {
	config: AllowedHostsConfig,
}

impl AllowedHostsMiddleware {
	/// Create a new `AllowedHostsMiddleware` with the given configuration
	pub fn new(config: AllowedHostsConfig) -> Self {
		Self { config }
	}

	/// Create from application `Settings`
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_conf::Settings;
	/// use reinhardt_middleware::AllowedHostsMiddleware;
	///
	/// let settings = Settings::default();
	/// let middleware = AllowedHostsMiddleware::from_settings(&settings);
	/// ```
	pub fn from_settings(settings: &Settings) -> Self {
		Self::new(AllowedHostsConfig::from_settings(settings))
	}

	/// Extract the host from the request
	fn get_host(request: &Request) -> Option<String> {
		request
			.headers
			.get(hyper::header::HOST)
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string())
	}
}

#[async_trait]
impl Middleware for AllowedHostsMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let host = Self::get_host(&request);

		match host {
			Some(ref h) if self.config.is_host_allowed(h) => handler.handle(request).await,
			None if self.config.allowed_hosts.is_empty() => handler.handle(request).await,
			_ => Ok(Response::new(StatusCode::BAD_REQUEST)
				.with_body("Invalid HTTP_HOST header".as_bytes())),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use rstest::rstest;

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	fn build_request_with_host(host: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::HOST,
			hyper::header::HeaderValue::from_str(host).unwrap(),
		);
		Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn build_request_without_host() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[rstest]
	#[tokio::test]
	async fn test_exact_host_match_allowed() {
		// Arrange
		let config = AllowedHostsConfig::new(vec!["example.com".to_string()]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_wildcard_pattern_match_allowed() {
		// Arrange
		let config = AllowedHostsConfig::new(vec!["*.example.com".to_string()]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("sub.example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_empty_allowed_hosts_allows_all() {
		// Arrange
		let config = AllowedHostsConfig::new(vec![]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("anything.example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_invalid_host_returns_400() {
		// Arrange
		let config = AllowedHostsConfig::new(vec!["example.com".to_string()]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("evil.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
	}

	#[rstest]
	#[tokio::test]
	async fn test_from_settings_conversion() {
		// Arrange
		let mut settings = Settings::new(std::path::PathBuf::from("/app"), "secret".to_string());
		settings.allowed_hosts = vec!["example.com".to_string(), "*.example.com".to_string()];

		// Act
		let config = AllowedHostsConfig::from_settings(&settings);

		// Assert
		assert_eq!(config.allowed_hosts.len(), 2);
		assert_eq!(config.allowed_hosts[0], "example.com");
		assert_eq!(config.allowed_hosts[1], "*.example.com");
	}

	#[rstest]
	#[tokio::test]
	async fn test_case_insensitive_host_matching() {
		// Arrange
		let config = AllowedHostsConfig::new(vec!["Example.COM".to_string()]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_host_with_port_allowed() {
		// Arrange
		let config = AllowedHostsConfig::new(vec!["example.com".to_string()]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("example.com:8080");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_missing_host_with_non_empty_allowed_hosts_returns_400() {
		// Arrange
		let config = AllowedHostsConfig::new(vec!["example.com".to_string()]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_without_host();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
	}

	#[rstest]
	#[tokio::test]
	async fn test_missing_host_with_empty_allowed_hosts_allows() {
		// Arrange
		let config = AllowedHostsConfig::new(vec![]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_without_host();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}

	#[rstest]
	#[tokio::test]
	async fn test_wildcard_does_not_match_exact_domain() {
		// Arrange: *.example.com should NOT match example.com itself
		let config = AllowedHostsConfig::new(vec!["*.example.com".to_string()]);
		let middleware = AllowedHostsMiddleware::new(config);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::BAD_REQUEST);
	}

	#[rstest]
	#[tokio::test]
	async fn test_from_settings_middleware_creation() {
		// Arrange
		let mut settings = Settings::new(std::path::PathBuf::from("/app"), "secret".to_string());
		settings.allowed_hosts = vec!["example.com".to_string()];
		let middleware = AllowedHostsMiddleware::from_settings(&settings);
		let handler = Arc::new(TestHandler);
		let request = build_request_with_host("example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
	}
}
