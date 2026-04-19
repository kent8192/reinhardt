//! Origin validation middleware for CSRF protection.
//!
//! Validates the `Origin` (or `Referer`) header on state-changing HTTP
//! requests, providing defense-in-depth CSRF protection alongside
//! `SameSite=Lax` cookies.

use async_trait::async_trait;
use std::sync::Arc;

use reinhardt_http::{Handler, Middleware, Request, Response, Result};

/// Middleware that validates the `Origin` or `Referer` header on
/// state-changing requests as a CSRF protection layer.
///
/// Safe methods (`GET`, `HEAD`, `OPTIONS`) are always passed through.
/// For state-changing methods (`POST`, `PUT`, `DELETE`, `PATCH`) the
/// middleware checks whether the request origin appears in the
/// `allowed_origins` list:
///
/// 1. Reads the `Origin` header directly.
/// 2. If absent, falls back to the `Referer` header and extracts the
///    `scheme://authority` portion.
/// 3. If the origin matches → request proceeds.
/// 4. If neither header is present, or the origin does not match →
///    **403 Forbidden** with body `"Origin validation failed"`.
///
/// # Examples
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use reinhardt_middleware::OriginGuardMiddleware;
/// use reinhardt_http::MiddlewareChain;
/// # use reinhardt_http::{Handler, Request, Response, Result};
/// # use async_trait::async_trait;
/// # struct MyHandler;
/// # #[async_trait]
/// # impl Handler for MyHandler {
/// #     async fn handle(&self, _request: Request) -> Result<Response> {
/// #         Ok(Response::ok())
/// #     }
/// # }
/// # let handler = Arc::new(MyHandler);
///
/// let middleware = OriginGuardMiddleware::new(vec![
///     "https://example.com".to_string(),
///     "https://app.example.com".to_string(),
/// ]);
///
/// let app = MiddlewareChain::new(handler)
///     .with_middleware(Arc::new(middleware));
/// ```
pub struct OriginGuardMiddleware {
	allowed_origins: Vec<String>,
}

impl OriginGuardMiddleware {
	/// Creates a new `OriginGuardMiddleware` with the given list of allowed origins.
	///
	/// Each entry should be a `scheme://authority` string such as
	/// `"https://example.com"` (no trailing slash, no path).
	///
	/// # Arguments
	///
	/// * `allowed_origins` - Origins that are permitted to make state-changing requests.
	pub fn new(allowed_origins: Vec<String>) -> Self {
		Self { allowed_origins }
	}

	/// Extracts the `scheme://authority` origin from a `Referer` URL string.
	///
	/// Returns `None` if the URL cannot be parsed or has no host.
	fn origin_from_referer(referer: &str) -> Option<String> {
		let url = url::Url::parse(referer).ok()?;
		let scheme = url.scheme();
		let host = url.host_str()?;
		let port = url.port();

		let origin = if let Some(p) = port {
			format!("{}://{}:{}", scheme, host, p)
		} else {
			format!("{}://{}", scheme, host)
		};

		Some(origin)
	}

	/// Returns true if the given origin string appears in `allowed_origins`.
	fn is_allowed(&self, origin: &str) -> bool {
		self.allowed_origins.iter().any(|o| o == origin)
	}
}

#[async_trait]
impl Middleware for OriginGuardMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let method = request.method.clone();

		// Safe methods always pass through.
		let is_safe = matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS");

		if is_safe {
			return next.handle(request).await;
		}

		// State-changing method: validate Origin / Referer.
		let origin = request
			.headers
			.get("Origin")
			.and_then(|v| v.to_str().ok())
			.map(|s| s.to_string())
			.or_else(|| {
				request
					.headers
					.get("Referer")
					.and_then(|v| v.to_str().ok())
					.and_then(Self::origin_from_referer)
			});

		match origin {
			Some(ref o) if self.is_allowed(o) => next.handle(request).await,
			_ => Ok(Response::forbidden().with_body("Origin validation failed")),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_http::{Handler, Middleware, Request, Response, Result};

	struct PassThroughHandler;

	#[async_trait::async_trait]
	impl Handler for PassThroughHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::ok().with_body("ok"))
		}
	}

	fn make_request(method: Method, origin: Option<&str>, referer: Option<&str>) -> Request {
		let mut headers = HeaderMap::new();
		if let Some(o) = origin {
			headers.insert("Origin", o.parse().unwrap());
		}
		if let Some(r) = referer {
			headers.insert("Referer", r.parse().unwrap());
		}
		Request::builder()
			.method(method)
			.uri("/submit")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn middleware() -> OriginGuardMiddleware {
		OriginGuardMiddleware::new(vec![
			"https://example.com".to_string(),
			"https://app.example.com".to_string(),
		])
	}

	fn handler() -> Arc<dyn Handler> {
		Arc::new(PassThroughHandler)
	}

	// Safe methods

	#[tokio::test]
	async fn test_get_always_passes_no_origin() {
		let mw = middleware();
		let req = make_request(Method::GET, None, None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 200);
	}

	#[tokio::test]
	async fn test_head_always_passes() {
		let mw = middleware();
		let req = make_request(Method::HEAD, None, None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 200);
	}

	#[tokio::test]
	async fn test_options_always_passes() {
		let mw = middleware();
		let req = make_request(Method::OPTIONS, None, None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 200);
	}

	// POST with valid origin

	#[tokio::test]
	async fn test_post_with_valid_origin_passes() {
		let mw = middleware();
		let req = make_request(Method::POST, Some("https://example.com"), None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 200);
	}

	// POST with invalid origin

	#[tokio::test]
	async fn test_post_with_invalid_origin_returns_403() {
		let mw = middleware();
		let req = make_request(Method::POST, Some("https://evil.com"), None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 403);
		let body = String::from_utf8(resp.body.to_vec()).unwrap();
		assert_eq!(body, "Origin validation failed");
	}

	// POST with no origin but valid referer

	#[tokio::test]
	async fn test_post_no_origin_valid_referer_passes() {
		let mw = middleware();
		let req = make_request(
			Method::POST,
			None,
			Some("https://example.com/some/path?foo=bar"),
		);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 200);
	}

	// POST with no origin and no referer

	#[tokio::test]
	async fn test_post_no_origin_no_referer_returns_403() {
		let mw = middleware();
		let req = make_request(Method::POST, None, None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 403);
		let body = String::from_utf8(resp.body.to_vec()).unwrap();
		assert_eq!(body, "Origin validation failed");
	}

	// DELETE with valid origin

	#[tokio::test]
	async fn test_delete_with_valid_origin_passes() {
		let mw = middleware();
		let req = make_request(Method::DELETE, Some("https://app.example.com"), None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 200);
	}

	// PUT with invalid origin

	#[tokio::test]
	async fn test_put_with_invalid_origin_returns_403() {
		let mw = middleware();
		let req = make_request(Method::PUT, Some("https://attacker.example.com"), None);
		let resp = mw.process(req, handler()).await.unwrap();
		assert_eq!(resp.status.as_u16(), 403);
		let body = String::from_utf8(resp.body.to_vec()).unwrap();
		assert_eq!(body, "Origin validation failed");
	}
}
