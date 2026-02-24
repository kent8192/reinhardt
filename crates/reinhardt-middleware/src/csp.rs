//! Content Security Policy (CSP) Middleware
//!
//! Provides CSP header management with:
//! - Customizable CSP directives
//! - Nonce generation for inline scripts/styles
//! - Report-Only mode for testing
//! - Per-request CSP overrides

use async_trait::async_trait;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::collections::HashMap;
use std::sync::Arc;

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
#[derive(Debug, Clone)]
pub struct CspConfig {
	/// CSP directives (e.g., "default-src", "script-src")
	pub directives: HashMap<String, Vec<String>>,
	/// Enable Report-Only mode (for testing without blocking)
	pub report_only: bool,
	/// Generate nonce for inline scripts/styles
	pub include_nonce: bool,
}

impl Default for CspConfig {
	fn default() -> Self {
		let mut directives = HashMap::new();
		directives.insert("default-src".to_string(), vec!["'self'".to_string()]);

		Self {
			directives,
			report_only: false,
			include_nonce: false,
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
		}
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
	/// let config = CspConfig {
	///     directives,
	///     report_only: false,
	///     include_nonce: false,
	/// };
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
		rand::rngs::OsRng.fill_bytes(&mut bytes);
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
		let mut response = handler.handle(request).await?;

		// Add CSP header
		let csp_value = self.build_csp_header(nonce.as_deref());
		response
			.headers
			.insert(self.get_header_name(), csp_value.parse().unwrap());

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
}
