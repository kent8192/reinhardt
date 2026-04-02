//! Content Security Policy (CSP) Middleware
//!
//! Provides CSP header management with:
//! - Customizable CSP directives
//! - Nonce generation for inline scripts/styles
//! - Report-Only mode for testing
//! - Per-request CSP overrides

use async_trait::async_trait;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, warn};

/// Type wrapper for CSP nonce stored in Request extensions
#[derive(Debug, Clone)]
pub struct CspNonce(pub String);

/// Validate that a nonce contains only base64 characters [A-Za-z0-9+/=].
///
/// Returns `true` if the nonce is non-empty and contains only valid base64
/// characters. This prevents header injection via malicious nonce values
/// containing characters like newlines, semicolons, or other special chars.
fn is_valid_nonce(nonce: &str) -> bool {
	!nonce.is_empty()
		&& nonce
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

/// CSP directive configuration
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CspConfig {
	/// CSP directives (e.g., "default-src", "script-src")
	pub directives: HashMap<String, Vec<String>>,
	/// Enable Report-Only mode (for testing without blocking)
	pub report_only: bool,
	/// Generate nonce for inline scripts/styles
	pub include_nonce: bool,
	/// Paths exempt from CSP header insertion.
	///
	/// When a request path matches an exempt prefix (with path-segment boundary
	/// checking), the middleware skips CSP header insertion entirely, allowing
	/// the handler's own CSP to take effect without interference.
	///
	/// This is useful when certain routes (e.g., admin panel) set their own
	/// CSP headers that differ from the application-wide policy.
	pub exempt_paths: HashSet<String>,
}

impl Default for CspConfig {
	fn default() -> Self {
		let mut directives = HashMap::new();
		directives.insert("default-src".to_string(), vec!["'self'".to_string()]);

		Self {
			directives,
			report_only: false,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		}
	}
}

impl CspConfig {
	/// Create a strict CSP configuration
	///
	/// Returns a configuration with restrictive directives suitable for high-security applications.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::CspConfig;
	///
	/// let config = CspConfig::strict();
	/// assert!(config.directives.contains_key("default-src"));
	/// assert!(config.directives.contains_key("script-src"));
	/// assert!(!config.report_only);
	/// ```
	pub fn strict() -> Self {
		let mut directives = HashMap::new();
		directives.insert("default-src".to_string(), vec!["'self'".to_string()]);
		directives.insert("script-src".to_string(), vec!["'self'".to_string()]);
		directives.insert("style-src".to_string(), vec!["'self'".to_string()]);
		directives.insert(
			"img-src".to_string(),
			vec!["'self'".to_string(), "data:".to_string()],
		);
		directives.insert("font-src".to_string(), vec!["'self'".to_string()]);
		directives.insert("connect-src".to_string(), vec!["'self'".to_string()]);
		directives.insert("frame-ancestors".to_string(), vec!["'none'".to_string()]);
		directives.insert("base-uri".to_string(), vec!["'self'".to_string()]);
		directives.insert("form-action".to_string(), vec!["'self'".to_string()]);

		Self {
			directives,
			report_only: false,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		}
	}

	/// Add a path prefix exempt from CSP header insertion.
	///
	/// Requests whose path matches this prefix (with path-segment boundary
	/// checking) will not have CSP headers set by this middleware, allowing
	/// handler-set CSP to take effect without interference.
	///
	/// Uses the same boundary matching as `CsrfMiddlewareConfig::add_exempt_path`:
	/// exempting `"/admin"` matches `"/admin"` and `"/admin/dashboard"` but
	/// NOT `"/administrator"`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::CspConfig;
	///
	/// let config = CspConfig::strict()
	///     .add_exempt_path("/admin".to_string())
	///     .add_exempt_path("/static/admin".to_string());
	///
	/// assert!(config.exempt_paths.contains("/admin"));
	/// assert!(config.exempt_paths.contains("/static/admin"));
	/// ```
	pub fn add_exempt_path(mut self, path: String) -> Self {
		self.exempt_paths.insert(path);
		self
	}
}

/// Content Security Policy middleware
pub struct CspMiddleware {
	config: CspConfig,
}

impl CspMiddleware {
	/// Create a new CspMiddleware with default configuration
	///
	/// Default configuration includes `default-src 'self'` directive.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::CspMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = CspMiddleware::new();
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/page")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// let csp = response.headers.get("Content-Security-Policy").unwrap();
	/// assert!(csp.to_str().unwrap().contains("default-src 'self'"));
	/// # });
	/// ```
	pub fn new() -> Self {
		Self {
			config: CspConfig::default(),
		}
	}
	/// Create a new CspMiddleware with custom configuration
	///
	/// # Arguments
	///
	/// * `config` - Custom CSP configuration
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{CspMiddleware, CspConfig};
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	/// use std::collections::HashMap;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let mut directives = HashMap::new();
	/// directives.insert("default-src".to_string(), vec!["'self'".to_string()]);
	/// directives.insert("script-src".to_string(), vec!["'self'".to_string(), "https://cdn.example.com".to_string()]);
	///
	/// let mut config = CspConfig::default();
	/// config.directives = directives;
	/// config.report_only = false;
	/// config.include_nonce = false;
	///
	/// let middleware = CspMiddleware::with_config(config);
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/app")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// let csp = response.headers.get("Content-Security-Policy").unwrap().to_str().unwrap();
	/// assert!(csp.contains("script-src 'self' https://cdn.example.com"));
	/// # });
	/// ```
	pub fn with_config(config: CspConfig) -> Self {
		Self { config }
	}
	/// Create a strict CSP middleware
	///
	/// Uses a restrictive configuration with strong security defaults.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::CspMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = CspMiddleware::strict();
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/secure-app")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// let csp = response.headers.get("Content-Security-Policy").unwrap().to_str().unwrap();
	/// assert!(csp.contains("default-src 'self'"));
	/// assert!(csp.contains("script-src 'self'"));
	/// assert!(csp.contains("frame-ancestors 'none'"));
	/// assert!(csp.contains("base-uri 'self'"));
	/// # });
	/// ```
	pub fn strict() -> Self {
		Self {
			config: CspConfig::strict(),
		}
	}

	/// Generate a random nonce for CSP
	fn generate_nonce(&self) -> String {
		use base64::Engine;
		use rand::RngCore;

		let mut bytes = [0u8; 16];
		rand::rng().fill_bytes(&mut bytes);
		base64::engine::general_purpose::STANDARD.encode(bytes)
	}

	/// Build CSP header value with optional nonce
	///
	/// Nonce values are validated to contain only base64 characters before
	/// embedding in the header to prevent header injection attacks.
	fn build_csp_header(&self, nonce: Option<&str>) -> String {
		let mut parts = Vec::new();

		// Only use the nonce if it passes validation
		let validated_nonce = nonce.filter(|n| is_valid_nonce(n));

		for (directive, values) in &self.config.directives {
			let mut directive_values = values.clone();

			// Add nonce to script-src and style-src if enabled
			if self.config.include_nonce
				&& (directive == "script-src" || directive == "style-src")
				&& let Some(n) = validated_nonce
			{
				directive_values.push(format!("'nonce-{}'", n));
			}

			parts.push(format!("{} {}", directive, directive_values.join(" ")));
		}

		parts.join("; ")
	}

	/// Get the appropriate CSP header name
	fn get_header_name(&self) -> &'static str {
		if self.config.report_only {
			"Content-Security-Policy-Report-Only"
		} else {
			"Content-Security-Policy"
		}
	}
}

impl Default for CspMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for CspMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Check if path is exempt from CSP insertion.
		// Uses path-segment boundary matching: exempt "/admin" matches "/admin"
		// and "/admin/dashboard" but NOT "/administrator".
		let path = request.uri.path();
		if self
			.config
			.exempt_paths
			.iter()
			.any(|exempt| path == exempt.as_str() || path.starts_with(&format!("{}/", exempt)))
		{
			debug!(
				path = path,
				"Path is CSP-exempt, skipping CSP header insertion"
			);
			return match handler.handle(request).await {
				Ok(resp) => Ok(resp),
				Err(e) => Ok(Response::from(e)),
			};
		}

		// Generate nonce if enabled
		let nonce = if self.config.include_nonce {
			let generated_nonce = self.generate_nonce();
			// Store nonce in request extensions for template access
			request.extensions.insert(CspNonce(generated_nonce.clone()));
			Some(generated_nonce)
		} else {
			None
		};

		// Call handler
		// Convert errors to responses so post-processing (e.g., security headers)
		// always runs, even when invoked outside MiddlewareChain. (#3244)
		let mut response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Add CSP header only if handler has not already set one
		let header_name = self.get_header_name();
		if response.headers.contains_key(header_name) {
			debug!(
				header = header_name,
				"CSP header already present in response, skipping middleware insertion"
			);
		} else {
			let csp_value = self.build_csp_header(nonce.as_deref());
			match csp_value.parse() {
				Ok(value) => {
					response.headers.insert(header_name, value);
				}
				Err(e) => {
					warn!(
						error = %e,
						"Failed to parse CSP header value, skipping header insertion"
					);
				}
			}
		}

		Ok(response)
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
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("content")))
		}
	}

	#[tokio::test]
	async fn test_default_csp_header() {
		let middleware = CspMiddleware::new();
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
		let csp_header = response.headers.get("Content-Security-Policy").unwrap();
		assert!(csp_header.to_str().unwrap().contains("default-src 'self'"));
	}

	#[tokio::test]
	async fn test_custom_csp_directives() {
		let mut directives = HashMap::new();
		directives.insert("default-src".to_string(), vec!["'self'".to_string()]);
		directives.insert(
			"script-src".to_string(),
			vec!["'self'".to_string(), "https://cdn.example.com".to_string()],
		);

		let config = CspConfig {
			directives,
			report_only: false,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let csp_header = response
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(csp_header.contains("default-src 'self'"));
		assert!(csp_header.contains("script-src 'self' https://cdn.example.com"));
	}

	#[tokio::test]
	async fn test_report_only_mode() {
		let config = CspConfig {
			directives: {
				let mut d = HashMap::new();
				d.insert("default-src".to_string(), vec!["'self'".to_string()]);
				d
			},
			report_only: true,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(
			response
				.headers
				.contains_key("Content-Security-Policy-Report-Only")
		);
		assert!(!response.headers.contains_key("Content-Security-Policy"));
	}

	#[tokio::test]
	async fn test_nonce_generation() {
		let config = CspConfig {
			directives: {
				let mut d = HashMap::new();
				d.insert("script-src".to_string(), vec!["'self'".to_string()]);
				d
			},
			report_only: false,
			include_nonce: true,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let csp_header = response
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(csp_header.contains("'nonce-"));
	}

	#[tokio::test]
	async fn test_strict_csp() {
		let middleware = CspMiddleware::strict();
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let csp_header = response
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(csp_header.contains("default-src 'self'"));
		assert!(csp_header.contains("script-src 'self'"));
		assert!(csp_header.contains("style-src 'self'"));
		assert!(csp_header.contains("frame-ancestors 'none'"));
		assert!(csp_header.contains("base-uri 'self'"));
	}

	#[tokio::test]
	async fn test_multiple_directive_values() {
		let mut directives = HashMap::new();
		directives.insert(
			"img-src".to_string(),
			vec![
				"'self'".to_string(),
				"data:".to_string(),
				"https:".to_string(),
			],
		);

		let config = CspConfig {
			directives,
			report_only: false,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let csp_header = response
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(csp_header.contains("img-src 'self' data: https:"));
	}

	#[tokio::test]
	async fn test_nonce_only_added_to_script_and_style() {
		let mut directives = HashMap::new();
		directives.insert("script-src".to_string(), vec!["'self'".to_string()]);
		directives.insert("style-src".to_string(), vec!["'self'".to_string()]);
		directives.insert("img-src".to_string(), vec!["'self'".to_string()]);

		let config = CspConfig {
			directives,
			report_only: false,
			include_nonce: true,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let csp_header = response
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap();

		// Count nonce occurrences - should appear in script-src and style-src
		let nonce_count = csp_header.matches("'nonce-").count();
		assert_eq!(nonce_count, 2);
	}

	#[tokio::test]
	async fn test_empty_directives() {
		let config = CspConfig {
			directives: HashMap::new(),
			report_only: false,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// Should still have the header, just empty
		assert!(response.headers.contains_key("Content-Security-Policy"));
	}

	#[tokio::test]
	async fn test_frame_ancestors_directive() {
		let mut directives = HashMap::new();
		directives.insert(
			"frame-ancestors".to_string(),
			vec!["'self'".to_string(), "https://trusted.com".to_string()],
		);

		let config = CspConfig {
			directives,
			report_only: false,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		let csp_header = response
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(csp_header.contains("frame-ancestors 'self' https://trusted.com"));
	}

	#[tokio::test]
	async fn test_nonce_uniqueness_across_requests() {
		let config = CspConfig {
			directives: {
				let mut d = HashMap::new();
				d.insert("script-src".to_string(), vec!["'self'".to_string()]);
				d
			},
			report_only: false,
			include_nonce: true,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		// First request
		let request1 = Request::builder()
			.method(Method::GET)
			.uri("/page1")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response1 = middleware.process(request1, handler.clone()).await.unwrap();
		let csp1 = response1
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap()
			.to_string();

		// Second request
		let request2 = Request::builder()
			.method(Method::GET)
			.uri("/page2")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();
		let response2 = middleware.process(request2, handler).await.unwrap();
		let csp2 = response2
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap()
			.to_string();

		// Extract nonces
		let extract_nonce = |csp: &str| -> Option<String> {
			csp.split("'nonce-")
				.nth(1)
				.and_then(|s| s.split('\'').next())
				.map(|s| s.to_string())
		};

		let nonce1 = extract_nonce(&csp1);
		let nonce2 = extract_nonce(&csp2);

		assert!(nonce1.is_some(), "First CSP should contain nonce");
		assert!(nonce2.is_some(), "Second CSP should contain nonce");

		// Nonces should be different (uniqueness check)
		assert_ne!(nonce1, nonce2, "Nonces should be unique across requests");
	}

	#[tokio::test]
	async fn test_response_body_preserved() {
		struct TestHandlerWithBody;

		#[async_trait]
		impl Handler for TestHandlerWithBody {
			async fn handle(&self, _request: Request) -> Result<Response> {
				Ok(Response::new(StatusCode::OK).with_body(Bytes::from("custom response content")))
			}
		}

		let middleware = CspMiddleware::new();
		let handler = Arc::new(TestHandlerWithBody);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// CSP header should be present
		assert!(response.headers.contains_key("Content-Security-Policy"));

		// Response body should be preserved exactly
		assert_eq!(response.body, Bytes::from("custom response content"));
	}

	#[rstest]
	fn test_nonce_is_valid_base64() {
		// Arrange
		use base64::Engine;
		let middleware = CspMiddleware::new();

		// Act
		let nonce = middleware.generate_nonce();

		// Assert
		let decoded = base64::engine::general_purpose::STANDARD.decode(&nonce);
		assert!(
			decoded.is_ok(),
			"Nonce should be valid base64, got: {}",
			nonce
		);
	}

	#[rstest]
	fn test_nonce_length() {
		// Arrange
		use base64::Engine;
		let middleware = CspMiddleware::new();

		// Act
		let nonce = middleware.generate_nonce();
		let decoded = base64::engine::general_purpose::STANDARD
			.decode(&nonce)
			.unwrap();

		// Assert
		assert_eq!(
			decoded.len(),
			16,
			"Nonce should be exactly 16 bytes (128 bits)"
		);
	}

	#[rstest]
	fn test_is_valid_nonce_accepts_base64() {
		// Arrange & Act & Assert
		assert!(is_valid_nonce("YWJjZGVmZw=="));
		assert!(is_valid_nonce("abc123+/="));
		assert!(is_valid_nonce("ABCDEFGHIJKLMNOP"));
	}

	#[rstest]
	fn test_is_valid_nonce_rejects_invalid_chars() {
		// Arrange & Act & Assert
		assert!(!is_valid_nonce(""));
		assert!(!is_valid_nonce("abc\ndef"));
		assert!(!is_valid_nonce("abc;def"));
		assert!(!is_valid_nonce("abc def"));
		assert!(!is_valid_nonce("abc'def"));
		assert!(!is_valid_nonce("abc\rdef"));
	}

	#[rstest]
	fn test_build_csp_header_rejects_invalid_nonce() {
		// Arrange
		let mut directives = HashMap::new();
		directives.insert("script-src".to_string(), vec!["'self'".to_string()]);
		let config = CspConfig {
			directives,
			report_only: false,
			include_nonce: true,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);

		// Act - nonce with header injection attempt (newline + semicolon)
		let csp = middleware.build_csp_header(Some("abc\r\ndef;injected"));

		// Assert - invalid nonce should be silently dropped
		assert!(
			!csp.contains("nonce-"),
			"Invalid nonce should not be embedded in header"
		);
		assert!(csp.contains("script-src 'self'"));
	}

	#[rstest]
	fn test_nonce_entropy() {
		// Arrange
		let middleware = CspMiddleware::new();
		let mut nonces = std::collections::HashSet::new();

		// Act
		for _ in 0..100 {
			nonces.insert(middleware.generate_nonce());
		}

		// Assert
		assert_eq!(
			nonces.len(),
			100,
			"All 100 nonces should be unique (statistical randomness)"
		);
	}

	#[tokio::test]
	async fn test_does_not_override_existing_csp_header() {
		// Arrange
		struct HandlerWithCsp;

		#[async_trait]
		impl Handler for HandlerWithCsp {
			async fn handle(&self, _request: Request) -> Result<Response> {
				Ok(Response::new(StatusCode::OK).with_header(
					"Content-Security-Policy",
					"default-src 'self'; style-src 'self' 'unsafe-inline'",
				))
			}
		}

		let middleware = CspMiddleware::strict();
		let handler = Arc::new(HandlerWithCsp);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/admin/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - handler's CSP should be preserved, not overwritten by middleware
		let csp = response
			.headers
			.get("Content-Security-Policy")
			.unwrap()
			.to_str()
			.unwrap();
		assert!(
			csp.contains("'unsafe-inline'"),
			"Handler-set CSP should be preserved, got: {}",
			csp
		);
	}

	#[tokio::test]
	async fn test_does_not_override_existing_csp_report_only_header() {
		// Arrange
		struct HandlerWithReportOnlyCsp;

		#[async_trait]
		impl Handler for HandlerWithReportOnlyCsp {
			async fn handle(&self, _request: Request) -> Result<Response> {
				Ok(Response::new(StatusCode::OK)
					.with_header("Content-Security-Policy-Report-Only", "default-src 'none'"))
			}
		}

		let config = CspConfig {
			directives: {
				let mut d = HashMap::new();
				d.insert("default-src".to_string(), vec!["'self'".to_string()]);
				d
			},
			report_only: true,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(HandlerWithReportOnlyCsp);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - handler's report-only CSP should be preserved
		let csp = response
			.headers
			.get("Content-Security-Policy-Report-Only")
			.unwrap()
			.to_str()
			.unwrap();
		assert_eq!(
			csp, "default-src 'none'",
			"Handler-set report-only CSP should be preserved"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_exempt_path_skips_csp() {
		// Arrange
		let config = CspConfig::strict().add_exempt_path("/admin".to_string());
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/admin/dashboard")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - CSP should not be set for exempt path
		assert!(
			!response.headers.contains_key("Content-Security-Policy"),
			"CSP should not be set for exempt path"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_exempt_path_exact_match() {
		// Arrange
		let config = CspConfig::strict().add_exempt_path("/admin".to_string());
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/admin")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - exact match should also be exempt
		assert!(
			!response.headers.contains_key("Content-Security-Policy"),
			"CSP should not be set for exact exempt path match"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_non_exempt_path_gets_csp() {
		// Arrange
		let config = CspConfig::strict().add_exempt_path("/admin".to_string());
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/api/data")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - non-exempt path should still get CSP
		assert!(
			response.headers.contains_key("Content-Security-Policy"),
			"CSP should be set for non-exempt path"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_exempt_path_boundary_prevents_false_match() {
		// Arrange - exempt "/admin" should NOT exempt "/administrator"
		let config = CspConfig::strict().add_exempt_path("/admin".to_string());
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/administrator/panel")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert - /administrator should NOT be exempt
		assert!(
			response.headers.contains_key("Content-Security-Policy"),
			"/administrator should NOT be exempt when only /admin is in exempt_paths"
		);
	}

	#[rstest]
	fn test_csp_config_add_exempt_path() {
		// Arrange & Act
		let config = CspConfig::default()
			.add_exempt_path("/admin".to_string())
			.add_exempt_path("/static/admin".to_string());

		// Assert
		assert!(config.exempt_paths.contains("/admin"));
		assert!(config.exempt_paths.contains("/static/admin"));
		assert_eq!(config.exempt_paths.len(), 2);
	}

	/// Handler that always returns an error to simulate inner handler failure.
	struct ErrorHandler;

	#[async_trait]
	impl Handler for ErrorHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Err(reinhardt_http::Error::Http("handler error".to_string()))
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_csp_header_applied_on_handler_error() {
		// Arrange
		let config = CspConfig {
			directives: {
				let mut d = HashMap::new();
				d.insert("default-src".to_string(), vec!["'none'".to_string()]);
				d
			},
			report_only: false,
			include_nonce: false,
			exempt_paths: HashSet::new(),
		};
		let middleware = CspMiddleware::with_config(config);
		let handler: Arc<dyn Handler> = Arc::new(ErrorHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert — error is converted to response with CSP header applied
		assert!(response.status.is_client_error() || response.status.is_server_error());
		assert!(
			response.headers.contains_key("Content-Security-Policy"),
			"CSP header should be applied even when handler returns an error"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_csp_exempt_path_error_converted_to_response() {
		// Arrange
		let config = CspConfig::strict().add_exempt_path("/exempt".to_string());
		let middleware = CspMiddleware::with_config(config);
		let handler: Arc<dyn Handler> = Arc::new(ErrorHandler);

		let request = Request::builder()
			.method(Method::GET)
			.uri("/exempt/resource")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act — should return Ok even though handler errors, because errors are
		// converted to responses
		let result = middleware.process(request, handler).await;

		// Assert
		assert!(
			result.is_ok(),
			"Handler error should be converted to response for exempt path"
		);
		let response = result.unwrap();
		assert!(response.status.is_client_error() || response.status.is_server_error());
	}

	#[rstest]
	#[tokio::test]
	async fn test_multiple_exempt_paths() {
		// Arrange
		let config = CspConfig::strict()
			.add_exempt_path("/admin".to_string())
			.add_exempt_path("/static/admin".to_string());
		let middleware = CspMiddleware::with_config(config);
		let handler = Arc::new(TestHandler);

		// Act & Assert - both paths should be exempt
		for uri in ["/admin/dashboard", "/static/admin/style.css"] {
			let request = Request::builder()
				.method(Method::GET)
				.uri(uri)
				.version(Version::HTTP_11)
				.headers(HeaderMap::new())
				.body(Bytes::new())
				.build()
				.unwrap();

			let response = middleware.process(request, handler.clone()).await.unwrap();
			assert!(
				!response.headers.contains_key("Content-Security-Policy"),
				"Path {} should be exempt from CSP",
				uri
			);
		}
	}
}
