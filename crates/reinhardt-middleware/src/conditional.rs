//! Conditional GET Middleware
//!
//! Handles ETags and Last-Modified headers for conditional GET requests.

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use hyper::header::{
	ETAG, IF_MATCH, IF_MODIFIED_SINCE, IF_NONE_MATCH, IF_UNMODIFIED_SINCE, LAST_MODIFIED,
};
use hyper::{Method, StatusCode};
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Conditional GET middleware
///
/// Implements HTTP conditional requests using ETags and Last-Modified headers.
/// - Supports If-None-Match (ETag-based)
/// - Supports If-Modified-Since (Last-Modified-based)
/// - Supports If-Match and If-Unmodified-Since for safe methods
pub struct ConditionalGetMiddleware {
	/// Whether to generate ETags automatically
	generate_etag: bool,
}

impl ConditionalGetMiddleware {
	/// Create a new ConditionalGetMiddleware
	///
	/// By default, automatic ETag generation is enabled for responses.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::ConditionalGetMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("content")))
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = ConditionalGetMiddleware::new();
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/resource")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.status, StatusCode::OK);
	/// assert!(response.headers.contains_key(hyper::header::ETAG));
	/// # });
	/// ```
	pub fn new() -> Self {
		Self {
			generate_etag: true,
		}
	}
	/// Create middleware without automatic ETag generation
	///
	/// Use this when you want to handle ETags manually or rely only on Last-Modified headers.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::ConditionalGetMiddleware;
	/// use reinhardt_http::{Handler, Middleware, Request, Response};
	/// use hyper::{StatusCode, Method, Version, HeaderMap};
	/// use bytes::Bytes;
	///
	/// struct TestHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for TestHandler {
	///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
	///         let mut response = Response::new(StatusCode::OK).with_body(Bytes::from("content"));
	///         response.headers.insert(
	///             hyper::header::LAST_MODIFIED,
	///             "Wed, 21 Oct 2015 07:28:00 GMT".parse().unwrap()
	///         );
	///         Ok(response)
	///     }
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let middleware = ConditionalGetMiddleware::without_etag();
	/// let handler = Arc::new(TestHandler);
	///
	/// let request = Request::builder()
	///     .method(Method::GET)
	///     .uri("/api/resource")
	///     .version(Version::HTTP_11)
	///     .headers(HeaderMap::new())
	///     .body(Bytes::new())
	///     .build()
	///     .unwrap();
	///
	/// let response = middleware.process(request, handler).await.unwrap();
	/// assert_eq!(response.status, StatusCode::OK);
	/// assert!(!response.headers.contains_key(hyper::header::ETAG));
	/// assert!(response.headers.contains_key(hyper::header::LAST_MODIFIED));
	/// # });
	/// ```
	pub fn without_etag() -> Self {
		Self {
			generate_etag: false,
		}
	}

	/// Generate an ETag from response body
	fn generate_etag_from_body(&self, body: &[u8]) -> String {
		let mut hasher = Sha256::new();
		hasher.update(body);
		let result = hasher.finalize();
		format!("\"{}\"", hex::encode(&result[..16]))
	}

	/// Parse If-None-Match header
	fn parse_if_none_match(&self, value: &str) -> Vec<String> {
		value.split(',').map(|s| s.trim().to_string()).collect()
	}

	/// Check if ETag matches
	fn etag_matches(&self, etag: &str, if_none_match: &[String]) -> bool {
		if_none_match
			.iter()
			.any(|inm| inm == "*" || inm == etag || inm.trim_matches('"') == etag.trim_matches('"'))
	}

	/// Parse HTTP date
	fn parse_http_date(&self, value: &str) -> Option<DateTime<Utc>> {
		httpdate::parse_http_date(value).ok().map(DateTime::from)
	}
}

impl Default for ConditionalGetMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for ConditionalGetMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Store request headers for later use
		let if_none_match = request.headers.get(IF_NONE_MATCH).cloned();
		let if_modified_since = request.headers.get(IF_MODIFIED_SINCE).cloned();
		let if_match = request.headers.get(IF_MATCH).cloned();
		let if_unmodified_since = request.headers.get(IF_UNMODIFIED_SINCE).cloned();
		let method = request.method.clone();

		// Convert errors to responses so post-processing always runs,
		// even when invoked outside MiddlewareChain. (#3244)
		let mut response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Only process GET and HEAD requests
		if method != Method::GET && method != Method::HEAD {
			return Ok(response);
		}

		// Only process successful responses
		if !response.status.is_success() {
			return Ok(response);
		}

		// Generate ETag if not present and configured to do so
		let etag = if self.generate_etag && !response.headers.contains_key(ETAG) {
			let generated = self.generate_etag_from_body(&response.body);
			if let Ok(etag_value) = generated.parse() {
				response.headers.insert(ETAG, etag_value);
				Some(generated)
			} else {
				// ETag value could not be parsed as a valid header value;
				// treat as if no ETag was generated
				None
			}
		} else {
			response
				.headers
				.get(ETAG)
				.and_then(|v| v.to_str().ok())
				.map(|s| s.to_string())
		};

		// Get Last-Modified if present
		let last_modified = response
			.headers
			.get(LAST_MODIFIED)
			.and_then(|v| v.to_str().ok())
			.and_then(|s| self.parse_http_date(s));

		// Check If-None-Match (ETag)
		if let Some(if_none_match) = if_none_match
			&& let (Ok(inm_str), Some(etag_value)) = (if_none_match.to_str(), etag.as_ref())
		{
			let inm_list = self.parse_if_none_match(inm_str);
			if self.etag_matches(etag_value, &inm_list) {
				// Return 304 Not Modified
				let mut not_modified = Response::new(StatusCode::NOT_MODIFIED);

				// Copy relevant headers
				if let Some(etag_header) = response.headers.get(ETAG) {
					not_modified.headers.insert(ETAG, etag_header.clone());
				}
				if let Some(lm_header) = response.headers.get(LAST_MODIFIED) {
					not_modified
						.headers
						.insert(LAST_MODIFIED, lm_header.clone());
				}

				return Ok(not_modified);
			}
		}

		// Check If-Modified-Since (Last-Modified)
		if let Some(if_modified_since) = if_modified_since
			&& let (Ok(ims_str), Some(lm)) = (if_modified_since.to_str(), last_modified)
			&& let Some(ims) = self.parse_http_date(ims_str)
		{
			// If resource hasn't been modified since the given date
			if lm <= ims {
				// Return 304 Not Modified
				let mut not_modified = Response::new(StatusCode::NOT_MODIFIED);

				// Copy relevant headers
				if let Some(etag_header) = response.headers.get(ETAG) {
					not_modified.headers.insert(ETAG, etag_header.clone());
				}
				if let Some(lm_header) = response.headers.get(LAST_MODIFIED) {
					not_modified
						.headers
						.insert(LAST_MODIFIED, lm_header.clone());
				}

				return Ok(not_modified);
			}
		}

		// Check If-Match (for safe methods, should match)
		if let Some(if_match) = if_match
			&& let (Ok(im_str), Some(etag_value)) = (if_match.to_str(), etag.as_ref())
		{
			let im_list = self.parse_if_none_match(im_str);
			if !self.etag_matches(etag_value, &im_list) && !im_list.contains(&"*".to_string()) {
				// Return 412 Precondition Failed
				return Ok(Response::new(StatusCode::PRECONDITION_FAILED)
					.with_body(Bytes::from(&b"Precondition Failed"[..])));
			}
		}

		// Check If-Unmodified-Since
		if let Some(if_unmodified_since) = if_unmodified_since
			&& let (Ok(ius_str), Some(lm)) = (if_unmodified_since.to_str(), last_modified)
			&& let Some(ius) = self.parse_http_date(ius_str)
		{
			// If resource has been modified since the given date
			if lm > ius {
				// Return 412 Precondition Failed
				return Ok(Response::new(StatusCode::PRECONDITION_FAILED)
					.with_body(Bytes::from(&b"Precondition Failed"[..])));
			}
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hyper::{HeaderMap, Version};

	struct TestHandler {
		body: &'static str,
		with_etag: Option<String>,
		with_last_modified: Option<DateTime<Utc>>,
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			let mut response = Response::new(StatusCode::OK).with_body(self.body.as_bytes());

			if let Some(ref etag) = self.with_etag {
				response.headers.insert(ETAG, etag.parse().unwrap());
			}

			if let Some(lm) = self.with_last_modified {
				let lm_str = httpdate::fmt_http_date(lm.into());
				response
					.headers
					.insert(LAST_MODIFIED, lm_str.parse().unwrap());
			}

			Ok(response)
		}
	}

	#[tokio::test]
	async fn test_generates_etag() {
		let middleware = ConditionalGetMiddleware::new();
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: None,
			with_last_modified: None,
		});

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert!(response.headers.contains_key(ETAG));
	}

	#[tokio::test]
	async fn test_if_none_match_returns_304() {
		let middleware = ConditionalGetMiddleware::new();
		let etag = "\"abc123\"";
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: Some(etag.to_string()),
			with_last_modified: None,
		});

		let mut headers = HeaderMap::new();
		headers.insert(IF_NONE_MATCH, etag.parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_MODIFIED);
		assert_eq!(response.body.len(), 0);
	}

	#[tokio::test]
	async fn test_if_modified_since_returns_304() {
		let middleware = ConditionalGetMiddleware::new();
		let last_modified = Utc::now() - chrono::Duration::days(1);
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: None,
			with_last_modified: Some(last_modified),
		});

		let mut headers = HeaderMap::new();
		let ims_str = httpdate::fmt_http_date((last_modified + chrono::Duration::hours(1)).into());
		headers.insert(IF_MODIFIED_SINCE, ims_str.parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::NOT_MODIFIED);
	}

	#[tokio::test]
	async fn test_if_match_fails_returns_412() {
		let middleware = ConditionalGetMiddleware::new();
		let etag = "\"abc123\"";
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: Some(etag.to_string()),
			with_last_modified: None,
		});

		let mut headers = HeaderMap::new();
		headers.insert(IF_MATCH, "\"xyz789\"".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::PRECONDITION_FAILED);
	}

	#[tokio::test]
	async fn test_middleware_wont_overwrite_etag() {
		let middleware = ConditionalGetMiddleware::new();
		let custom_etag = "\"custom-etag\"";
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: Some(custom_etag.to_string()),
			with_last_modified: None,
		});

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
		assert_eq!(
			response.headers.get(ETAG).unwrap().to_str().unwrap(),
			custom_etag
		);
	}

	#[tokio::test]
	async fn test_if_none_match_and_different_etag() {
		let middleware = ConditionalGetMiddleware::new();
		let etag = "\"abc123\"";
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: Some(etag.to_string()),
			with_last_modified: None,
		});

		let mut headers = HeaderMap::new();
		headers.insert(IF_NONE_MATCH, "\"different-etag\"".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_if_modified_since_and_last_modified_in_the_future() {
		let middleware = ConditionalGetMiddleware::new();
		let last_modified = Utc::now();
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: None,
			with_last_modified: Some(last_modified),
		});

		let mut headers = HeaderMap::new();
		let ims_str = httpdate::fmt_http_date((last_modified - chrono::Duration::hours(1)).into());
		headers.insert(IF_MODIFIED_SINCE, ims_str.parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		assert_eq!(response.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_no_etag_on_post_request() {
		let middleware = ConditionalGetMiddleware::new();
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: None,
			with_last_modified: None,
		});

		let request = Request::builder()
			.method(Method::POST)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// ETag should not be generated for POST requests
		assert!(!response.headers.contains_key(ETAG));
	}

	#[tokio::test]
	async fn test_without_etag_generation() {
		let middleware = ConditionalGetMiddleware::without_etag();
		let handler = Arc::new(TestHandler {
			body: "test response",
			with_etag: None,
			with_last_modified: None,
		});

		let request = Request::builder()
			.method(Method::GET)
			.uri("/test")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();

		// ETag should not be generated when disabled
		assert!(!response.headers.contains_key(ETAG));
	}
}
