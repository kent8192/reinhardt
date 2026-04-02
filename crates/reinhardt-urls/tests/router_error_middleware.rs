//! Integration tests for router-level middleware on error responses (#3234)
//!
//! Verifies that framework-level 404/405 responses pass through
//! the middleware chain so security headers are applied.

use hyper::Method;
use reinhardt_http::{Handler, Request, Response};
use reinhardt_middleware::xframe::{XFrameOptions, XFrameOptionsMiddleware};
use reinhardt_urls::routers::ServerRouter;
use rstest::rstest;

fn create_test_request(method: Method, path: &str) -> Request {
	Request::builder()
		.method(method)
		.uri(path)
		.version(hyper::Version::HTTP_11)
		.headers(hyper::HeaderMap::new())
		.body(bytes::Bytes::new())
		.build()
		.unwrap()
}

async fn ok_handler(_req: Request) -> reinhardt_core::exception::Result<Response> {
	Ok(Response::ok())
}

// ============================================================================
// ServerRouter 404/405 with XFrameOptionsMiddleware
// ============================================================================

/// Test: XFrameOptions header is applied to router-level 404 responses
#[rstest]
#[tokio::test]
async fn test_router_404_gets_xframe_header() {
	// Arrange
	let router = ServerRouter::new()
		.with_middleware(XFrameOptionsMiddleware::new(XFrameOptions::Deny))
		.route("/api/users/", Method::GET, ok_handler);

	// Act
	let request = create_test_request(Method::GET, "/nonexistent");
	let response = Handler::handle(&router, request).await.unwrap();

	// Assert
	assert_eq!(response.status, hyper::StatusCode::NOT_FOUND);
	assert_eq!(
		response
			.headers
			.get("X-Frame-Options")
			.map(|v| v.to_str().unwrap()),
		Some("DENY"),
		"404 response should have X-Frame-Options: DENY"
	);
}

/// Test: XFrameOptions header is applied to router-level 405 responses
#[rstest]
#[tokio::test]
async fn test_router_405_gets_xframe_header() {
	// Arrange
	let router = ServerRouter::new()
		.with_middleware(XFrameOptionsMiddleware::new(XFrameOptions::Deny))
		.route("/api/users/", Method::GET, ok_handler);

	// Act: POST to a GET-only route
	let request = create_test_request(Method::POST, "/api/users/");
	let response = Handler::handle(&router, request).await.unwrap();

	// Assert
	assert_eq!(response.status, hyper::StatusCode::METHOD_NOT_ALLOWED);
	assert_eq!(
		response
			.headers
			.get("X-Frame-Options")
			.map(|v| v.to_str().unwrap()),
		Some("DENY"),
		"405 response should have X-Frame-Options: DENY"
	);
}
