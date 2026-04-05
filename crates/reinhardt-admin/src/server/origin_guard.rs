//! Admin Origin Guard Middleware
//!
//! Restricts admin server function access to same-origin requests,
//! providing an additional security layer beyond authentication.
//!
//! # How It Works
//!
//! On state-changing requests (POST, PUT, PATCH, DELETE), the middleware
//! validates the `Origin` or `Referer` header against the `Host` header
//! to confirm that the request originates from the same domain.
//!
//! # Security Model
//!
//! This middleware works in concert with HTTP-Only cookie authentication
//! (`SameSite=Strict`). Together they form a multi-layer defense:
//!
//! 1. **`SameSite=Strict` cookie**: browsers never attach the auth cookie
//!    to cross-origin requests, so external origins fail authentication.
//! 2. **Origin guard (this middleware)**: rejects cross-origin requests
//!    early, before they reach the authentication layer. Also defends
//!    against non-browser clients that forge cookies but cannot forge
//!    matching `Origin`/`Host` headers in a browser context.
//! 3. **CSRF token validation**: existing double-submit cookie pattern
//!    provides an independent CSRF defense for mutation endpoints.
//!
//! # Skipped Methods
//!
//! GET, HEAD, and OPTIONS requests are exempt because they serve the SPA
//! shell, static assets, and CORS preflight responses respectively.

use async_trait::async_trait;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;

/// Middleware that restricts admin server function access to same-origin
/// requests by validating `Origin`/`Referer` against the `Host` header.
///
/// # Example
///
/// ```ignore
/// use reinhardt_admin::server::origin_guard::AdminOriginGuardMiddleware;
/// use reinhardt_urls::routers::ServerRouter;
///
/// let router = ServerRouter::new()
///     .with_namespace("admin")
///     .with_middleware(AdminOriginGuardMiddleware);
/// ```
pub struct AdminOriginGuardMiddleware;

#[async_trait]
impl Middleware for AdminOriginGuardMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		// Skip safe methods (SPA HTML, static assets, CORS preflight)
		if is_safe_method(&request.method) {
			return next.handle(request).await;
		}

		// Validate same-origin via Origin or Referer header
		if !is_same_origin(&request.headers) {
			tracing::warn!(
				method = %request.method,
				uri = %request.uri,
				"Admin origin guard: cross-origin or missing origin"
			);
			return Ok(Response::new(hyper::StatusCode::FORBIDDEN)
				.with_header("Content-Type", "application/json")
				.with_body(
					r#"{"error":"Forbidden: cross-origin admin requests are not allowed"}"#,
				));
		}

		next.handle(request).await
	}
}

/// Returns true for HTTP methods that don't require origin validation.
fn is_safe_method(method: &hyper::Method) -> bool {
	matches!(
		*method,
		hyper::Method::GET | hyper::Method::HEAD | hyper::Method::OPTIONS
	)
}

/// Validates that the request originates from the same origin by comparing
/// the `Origin` (or `Referer`) header against the `Host` header.
///
/// Returns `true` if:
/// - The `Origin` header's host matches the `Host` header, OR
/// - The `Referer` header's host matches the `Host` header.
///
/// Returns `false` if:
/// - Neither `Origin` nor `Referer` is present (rejects non-browser
///   clients that don't supply origin information), OR
/// - The origin doesn't match the host.
fn is_same_origin(headers: &hyper::HeaderMap) -> bool {
	let host = match headers
		.get(hyper::header::HOST)
		.and_then(|v| v.to_str().ok())
	{
		Some(h) => h,
		// No Host header — reject for safety
		None => return false,
	};

	// Try Origin header first (most reliable, sent by browsers on POST)
	if let Some(origin) = headers
		.get(hyper::header::ORIGIN)
		.and_then(|v| v.to_str().ok())
	{
		return origin_matches_host(origin, host);
	}

	// Fall back to Referer header
	if let Some(referer) = headers
		.get(hyper::header::REFERER)
		.and_then(|v| v.to_str().ok())
	{
		return referer_matches_host(referer, host);
	}

	// Neither Origin nor Referer present — reject.
	// The WASM SPA always sends Origin on POST requests.
	// Non-browser clients must provide Origin or Referer.
	false
}

/// Checks if an Origin header value (e.g., `"https://example.com"`) matches the Host.
fn origin_matches_host(origin: &str, host: &str) -> bool {
	// Origin format: "scheme://host[:port]"
	let origin_host = origin.split("://").nth(1).unwrap_or(origin);
	let origin_host = origin_host.trim_end_matches('/');
	origin_host == host
}

/// Checks if a Referer header value matches the Host.
fn referer_matches_host(referer: &str, host: &str) -> bool {
	// Referer format: "scheme://host[:port]/path"
	let after_scheme = referer.split("://").nth(1).unwrap_or(referer);
	let referer_host = after_scheme.split('/').next().unwrap_or(after_scheme);
	referer_host == host
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	struct PassthroughHandler;

	#[async_trait]
	impl Handler for PassthroughHandler {
		async fn handle(&self, _request: Request) -> Result<Response> {
			Ok(Response::new(StatusCode::OK).with_body("ok"))
		}
	}

	fn make_request(method: Method, headers: HeaderMap) -> Request {
		Request::builder()
			.method(method)
			.uri("/api/server_fn/get_list")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[tokio::test]
	async fn test_get_request_passes_through() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let req = make_request(Method::GET, HeaderMap::new());
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_post_without_origin_returns_403() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "localhost:8000".parse().unwrap());
		let req = make_request(Method::POST, headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::FORBIDDEN);
	}

	#[tokio::test]
	async fn test_post_without_host_returns_403() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::ORIGIN,
			"http://localhost:8000".parse().unwrap(),
		);
		let req = make_request(Method::POST, headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::FORBIDDEN);
	}

	#[tokio::test]
	async fn test_post_same_origin_passes() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "localhost:8000".parse().unwrap());
		headers.insert(
			hyper::header::ORIGIN,
			"http://localhost:8000".parse().unwrap(),
		);
		let req = make_request(Method::POST, headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_post_different_origin_returns_403() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "localhost:8000".parse().unwrap());
		headers.insert(hyper::header::ORIGIN, "http://evil.com".parse().unwrap());
		let req = make_request(Method::POST, headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::FORBIDDEN);
	}

	#[tokio::test]
	async fn test_post_referer_same_origin_passes() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());
		headers.insert(
			hyper::header::REFERER,
			"https://example.com/admin/".parse().unwrap(),
		);
		let req = make_request(Method::POST, headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::OK);
	}

	#[tokio::test]
	async fn test_post_referer_different_origin_returns_403() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let mut headers = HeaderMap::new();
		headers.insert(hyper::header::HOST, "example.com".parse().unwrap());
		headers.insert(
			hyper::header::REFERER,
			"https://evil.com/admin/".parse().unwrap(),
		);
		let req = make_request(Method::POST, headers);
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::FORBIDDEN);
	}

	#[tokio::test]
	async fn test_options_request_passes_through() {
		let mw = AdminOriginGuardMiddleware;
		let next = Arc::new(PassthroughHandler);
		let req = make_request(Method::OPTIONS, HeaderMap::new());
		let resp = mw.process(req, next).await.unwrap();
		assert_eq!(resp.status, StatusCode::OK);
	}

	#[test]
	fn test_origin_matches_host() {
		assert!(origin_matches_host(
			"http://localhost:8000",
			"localhost:8000"
		));
		assert!(origin_matches_host("https://example.com", "example.com"));
		assert!(!origin_matches_host("http://evil.com", "example.com"));
		assert!(!origin_matches_host(
			"http://localhost:9000",
			"localhost:8000"
		));
	}

	#[test]
	fn test_referer_matches_host() {
		assert!(referer_matches_host(
			"http://localhost:8000/admin/",
			"localhost:8000"
		));
		assert!(referer_matches_host(
			"https://example.com/admin/model/",
			"example.com"
		));
		assert!(!referer_matches_host(
			"http://evil.com/admin/",
			"example.com"
		));
	}
}
