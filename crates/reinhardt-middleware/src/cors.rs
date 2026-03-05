use async_trait::async_trait;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// CORS middleware configuration
#[non_exhaustive]
pub struct CorsConfig {
	pub allow_origins: Vec<String>,
	pub allow_methods: Vec<String>,
	pub allow_headers: Vec<String>,
	pub allow_credentials: bool,
	pub max_age: Option<u64>,
}

impl Default for CorsConfig {
	fn default() -> Self {
		Self {
			allow_origins: vec!["*".to_string()],
			allow_methods: vec![
				"GET".to_string(),
				"POST".to_string(),
				"PUT".to_string(),
				"PATCH".to_string(),
				"DELETE".to_string(),
				"OPTIONS".to_string(),
			],
			allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
			allow_credentials: false,
			max_age: Some(3600),
		}
	}
}

/// CORS middleware
pub struct CorsMiddleware {
	config: CorsConfig,
}

impl CorsMiddleware {
	/// Create a new CORS middleware with custom configuration
	///
	/// # Arguments
	///
	/// * `config` - CORS configuration specifying allowed origins, methods, headers, etc.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::{CorsMiddleware, cors::CorsConfig};
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let mut config = CorsConfig::default();
	/// config.allow_origins = vec!["https://example.com".to_string()];
	/// config.allow_methods = vec!["GET".to_string(), "POST".to_string()];
	/// config.allow_headers = vec!["Content-Type".to_string()];
	/// config.allow_credentials = true;
	/// config.max_age = Some(3600);
	///
	/// let middleware = CorsMiddleware::new(config);
	/// let handler = Arc::new(TestHandler);
	///
	/// let mut headers = HeaderMap::new();
	/// headers.insert("origin", "https://example.com".parse().unwrap());
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/data")
	///     .version(Version::HTTP_11)
	///     .headers(headers)
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.headers.get("Access-Control-Allow-Origin").unwrap(), "https://example.com");
	/// assert_eq!(response.headers.get("Access-Control-Allow-Credentials").unwrap(), "true");
	/// # });
	/// ```
	pub fn new(config: CorsConfig) -> Self {
		Self { config }
	}
	/// Create a permissive CORS middleware that allows all origins
	///
	/// This is useful for development but should be used with caution in production.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::CorsMiddleware;
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
	/// let middleware = CorsMiddleware::permissive();
	/// let handler = Arc::new(TestHandler);
	///
	/// // Preflight request
	/// let request = Request::builder()
	///     .method(Method::OPTIONS)
	///     .uri("/api/users")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.status, StatusCode::NO_CONTENT);
	/// assert!(response.headers.contains_key("Access-Control-Allow-Origin"));
	/// assert!(response.headers.contains_key("Access-Control-Allow-Methods"));
	/// # });
	/// ```
	pub fn permissive() -> Self {
		Self::new(CorsConfig::default())
	}
}

#[async_trait]
impl Middleware for CorsMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		// Extract request Origin header for validation
		let request_origin = request
			.headers
			.get(hyper::header::ORIGIN)
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string());

		// Determine the allowed origin value for this request
		let allowed_origin = self.resolve_origin(request_origin.as_deref());

		// Handle preflight OPTIONS request
		if request.method.as_str() == "OPTIONS" {
			let mut response = Response::no_content();

			if let Some(origin) = &allowed_origin {
				response.headers.insert(
					hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
					hyper::header::HeaderValue::from_str(origin)
						.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("*")),
				);
			}

			response.headers.insert(
				hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
				hyper::header::HeaderValue::from_str(&self.config.allow_methods.join(", "))
					.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("*")),
			);

			response.headers.insert(
				hyper::header::ACCESS_CONTROL_ALLOW_HEADERS,
				hyper::header::HeaderValue::from_str(&self.config.allow_headers.join(", "))
					.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("*")),
			);

			if let Some(max_age) = self.config.max_age {
				response.headers.insert(
					hyper::header::ACCESS_CONTROL_MAX_AGE,
					hyper::header::HeaderValue::from_str(&max_age.to_string())
						.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("3600")),
				);
			}

			if self.config.allow_credentials {
				response.headers.insert(
					hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
					hyper::header::HeaderValue::from_static("true"),
				);
			}

			// Add Vary: Origin when origin depends on request
			if self.config.allow_origins.len() > 1
				|| !self.config.allow_origins.contains(&"*".to_string())
			{
				response.headers.insert(
					hyper::header::VARY,
					hyper::header::HeaderValue::from_static("Origin"),
				);
			}

			return Ok(response);
		}

		// Process request and add CORS headers to response
		let mut response = next.handle(request).await?;

		if let Some(origin) = &allowed_origin {
			response.headers.insert(
				hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
				hyper::header::HeaderValue::from_str(origin)
					.unwrap_or_else(|_| hyper::header::HeaderValue::from_static("*")),
			);
		}

		if self.config.allow_credentials {
			response.headers.insert(
				hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
				hyper::header::HeaderValue::from_static("true"),
			);
		}

		// Add Vary: Origin when origin depends on request
		if self.config.allow_origins.len() > 1
			|| !self.config.allow_origins.contains(&"*".to_string())
		{
			response.headers.insert(
				hyper::header::VARY,
				hyper::header::HeaderValue::from_static("Origin"),
			);
		}

		Ok(response)
	}
}

impl CorsMiddleware {
	/// Resolve the origin to include in the response based on the request origin.
	///
	/// Per the CORS specification (Fetch Standard), `Access-Control-Allow-Origin`
	/// must be either `*`, a single origin, or `null`. Multiple origins in a
	/// single header value are not valid.
	fn resolve_origin(&self, request_origin: Option<&str>) -> Option<String> {
		// Wildcard: allow all origins
		if self.config.allow_origins.contains(&"*".to_string()) {
			// When credentials are enabled, wildcard is not allowed per spec;
			// reflect the request origin instead
			if self.config.allow_credentials {
				return request_origin.map(|o| o.to_string());
			}
			return Some("*".to_string());
		}

		// Check if the request origin matches any allowed origin
		if let Some(origin) = request_origin
			&& self.config.allow_origins.iter().any(|o| o == origin)
		{
			return Some(origin.to_string());
		}

		// No match: omit the CORS origin header
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct TestHandler;

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("test response")))
		}
	}

	/// Helper to create a request with an Origin header
	fn create_request_with_origin(method: Method, uri: &str, origin: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::ORIGIN,
			hyper::header::HeaderValue::from_str(origin).unwrap(),
		);

		Request::builder()
			.method(method)
			.uri(uri)
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[tokio::test]
	async fn test_preflight_request_with_matching_origin() {
		// Arrange
		let config = CorsConfig {
			allow_origins: vec!["https://example.com".to_string()],
			allow_methods: vec!["GET".to_string(), "POST".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: true,
			max_age: Some(7200),
		};
		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let request =
			create_request_with_origin(Method::OPTIONS, "/api/test", "https://example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::NO_CONTENT);

		// Origin header should reflect the matching origin (not multiple)
		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.unwrap(),
			"https://example.com"
		);

		assert!(
			response
				.headers
				.contains_key(hyper::header::ACCESS_CONTROL_ALLOW_METHODS)
		);
		assert!(
			response
				.headers
				.contains_key(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS)
		);
		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_MAX_AGE)
				.unwrap(),
			"7200"
		);
		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
				.unwrap(),
			"true"
		);
		// Vary: Origin should be present when origin list is not wildcard
		assert_eq!(response.headers.get(hyper::header::VARY).unwrap(), "Origin");
	}

	#[tokio::test]
	async fn test_regular_request_with_matching_origin() {
		// Arrange
		let config = CorsConfig {
			allow_origins: vec!["https://app.example.com".to_string()],
			allow_methods: vec!["GET".to_string()],
			allow_headers: vec!["Authorization".to_string()],
			allow_credentials: false,
			max_age: None,
		};
		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let request =
			create_request_with_origin(Method::GET, "/api/data", "https://app.example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::OK);
		assert_eq!(response.body, Bytes::from("test response"));

		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.unwrap(),
			"https://app.example.com"
		);

		assert!(
			!response
				.headers
				.contains_key(hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
		);

		// Vary: Origin should be present
		assert_eq!(response.headers.get(hyper::header::VARY).unwrap(), "Origin");
	}

	#[tokio::test]
	async fn test_request_with_non_matching_origin_omits_cors_headers() {
		// Arrange
		let config = CorsConfig {
			allow_origins: vec!["https://allowed.example.com".to_string()],
			allow_methods: vec!["GET".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: false,
			max_age: None,
		};
		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let request =
			create_request_with_origin(Method::GET, "/api/data", "https://evil.example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert: request is still processed, but no CORS origin header
		assert_eq!(response.status, StatusCode::OK);
		assert!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.is_none()
		);
	}

	#[tokio::test]
	async fn test_permissive_mode_wildcard() {
		// Arrange
		let middleware = CorsMiddleware::permissive();
		let handler = Arc::new(TestHandler);

		let request = Request::builder()
			.method(Method::OPTIONS)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = middleware.process(request, handler.clone()).await.unwrap();

		// Assert
		assert_eq!(response.status, StatusCode::NO_CONTENT);

		// Wildcard origin
		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.unwrap(),
			"*"
		);

		// Methods should be listed
		let methods_header = response
			.headers
			.get(hyper::header::ACCESS_CONTROL_ALLOW_METHODS)
			.unwrap()
			.to_str()
			.unwrap();
		assert!(methods_header.contains("GET"));
		assert!(methods_header.contains("POST"));
		assert!(methods_header.contains("PUT"));
		assert!(methods_header.contains("DELETE"));
	}

	#[tokio::test]
	async fn test_multiple_allowed_origins_reflects_matching_one() {
		// Arrange
		let config = CorsConfig {
			allow_origins: vec![
				"https://app1.example.com".to_string(),
				"https://app2.example.com".to_string(),
			],
			allow_methods: vec!["GET".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: true,
			max_age: Some(3600),
		};
		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let request =
			create_request_with_origin(Method::GET, "/api/resource", "https://app2.example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert: only the matching origin is reflected (not both joined)
		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.unwrap(),
			"https://app2.example.com"
		);

		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
				.unwrap(),
			"true"
		);

		// Vary: Origin must be present
		assert_eq!(response.headers.get(hyper::header::VARY).unwrap(), "Origin");
	}

	#[tokio::test]
	async fn test_multiple_origins_no_match_omits_origin_header() {
		// Arrange
		let config = CorsConfig {
			allow_origins: vec![
				"https://app1.example.com".to_string(),
				"https://app2.example.com".to_string(),
			],
			allow_methods: vec!["GET".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: false,
			max_age: None,
		};
		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let request = create_request_with_origin(
			Method::GET,
			"/api/resource",
			"https://attacker.example.com",
		);

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert: no origin header for non-matching origin
		assert!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.is_none()
		);
	}

	#[tokio::test]
	async fn test_wildcard_with_credentials_reflects_origin() {
		// Arrange: wildcard + credentials requires reflecting origin per spec
		let config = CorsConfig {
			allow_origins: vec!["*".to_string()],
			allow_methods: vec!["GET".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: true,
			max_age: None,
		};
		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		let request =
			create_request_with_origin(Method::GET, "/api/data", "https://any-origin.example.com");

		// Act
		let response = middleware.process(request, handler).await.unwrap();

		// Assert: should reflect the origin, not "*" (credentials mode)
		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.unwrap(),
			"https://any-origin.example.com"
		);

		assert_eq!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
				.unwrap(),
			"true"
		);
	}

	#[tokio::test]
	async fn test_request_without_origin_header() {
		// Arrange
		let config = CorsConfig {
			allow_origins: vec!["https://example.com".to_string()],
			allow_methods: vec!["GET".to_string()],
			allow_headers: vec!["Content-Type".to_string()],
			allow_credentials: false,
			max_age: None,
		};
		let middleware = CorsMiddleware::new(config);
		let handler = Arc::new(TestHandler);

		// No Origin header (same-origin request)
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

		// Assert: no origin header when no Origin in request
		assert_eq!(response.status, StatusCode::OK);
		assert!(
			response
				.headers
				.get(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN)
				.is_none()
		);
	}
}
