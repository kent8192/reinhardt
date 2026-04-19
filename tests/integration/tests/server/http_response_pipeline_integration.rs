//! Full-stack HTTP response pipeline integration tests.
//!
//! These tests exercise the complete request/response pipeline:
//! middleware → router → handler → error conversion → hyper response
//!
//! Motivated by issue #3135 where handler-level tests passed but the
//! production server returned incorrect Content-Type headers due to
//! the error conversion path never being exercised in unit tests.

use super::server_test_helpers::{shutdown_test_server, spawn_test_server};
use async_trait::async_trait;
use reinhardt_core::exception::Result;
use reinhardt_http::{Handler, MiddlewareChain, Request, Response};
use reinhardt_middleware::SecurityMiddleware;
use reinhardt_utils::staticfiles::{StaticFilesMiddleware, StaticMiddlewareConfig};
use rstest::rstest;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a temp directory populated with sample static files.
fn create_static_files_dir() -> TempDir {
	let dir = TempDir::new().expect("Failed to create temp dir");
	let root = dir.path();

	std::fs::write(root.join("style.css"), "body { margin: 0; }").unwrap();
	std::fs::write(root.join("app.js"), "console.log('hello');").unwrap();
	// Minimal valid WASM magic bytes
	std::fs::write(root.join("module.wasm"), b"\0asm\x01\0\0\0").unwrap();
	std::fs::write(root.join("index.html"), "<html><body></body></html>").unwrap();
	std::fs::write(root.join("data.json"), r#"{"key":"value"}"#).unwrap();

	dir
}

/// Build a MiddlewareChain with StaticFilesMiddleware over a fallback handler.
fn build_static_chain(root_dir: PathBuf, fallback: Arc<dyn Handler>) -> Arc<dyn Handler> {
	let config = StaticMiddlewareConfig::new(root_dir).url_prefix("/static/");
	let static_mw = Arc::new(StaticFilesMiddleware::new(config));

	let chain = MiddlewareChain::new(fallback).with_middleware(static_mw);
	Arc::new(chain)
}

/// Build a MiddlewareChain with SecurityMiddleware + StaticFilesMiddleware.
fn build_security_static_chain(root_dir: PathBuf, fallback: Arc<dyn Handler>) -> Arc<dyn Handler> {
	let config = StaticMiddlewareConfig::new(root_dir).url_prefix("/static/");
	let static_mw = Arc::new(StaticFilesMiddleware::new(config));
	let security_mw = Arc::new(SecurityMiddleware::new());

	// Security added first (inner), static added second (outer after rev()).
	// After rev() build: security wraps static wraps handler, so security
	// post-processes all responses including those from static files.
	let chain = MiddlewareChain::new(fallback)
		.with_middleware(security_mw)
		.with_middleware(static_mw);
	Arc::new(chain)
}

// ---------------------------------------------------------------------------
// Test handlers
// ---------------------------------------------------------------------------

/// Handler that always returns 404 (simulates no route matched).
struct NotFoundHandler;

#[async_trait]
impl Handler for NotFoundHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Ok(Response::not_found().with_body("Not Found"))
	}
}

/// Handler that returns a plain text response with custom Content-Type.
struct PlainTextHandler;

#[async_trait]
impl Handler for PlainTextHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Ok(Response::ok()
			.with_header("Content-Type", "text/plain")
			.with_body("Hello, World!"))
	}
}

/// Handler that returns an error to exercise the error→JSON conversion path.
struct ErrorHandler;

#[async_trait]
impl Handler for ErrorHandler {
	async fn handle(&self, _request: Request) -> Result<Response> {
		Err(reinhardt_core::exception::Error::Internal(
			"Simulated error".to_string(),
		))
	}
}

// ===========================================================================
// Group A: Static file Content-Type preservation through full pipeline
// ===========================================================================

#[rstest]
#[case("style.css", "text/css")]
#[case("app.js", "text/javascript")]
#[case("index.html", "text/html")]
#[case("data.json", "application/json")]
#[case("module.wasm", "application/wasm")]
#[tokio::test]
async fn test_static_file_content_type_preserved_through_pipeline(
	#[case] filename: &str,
	#[case] expected_content_type: &str,
) {
	// Arrange
	let dir = create_static_files_dir();
	let handler = build_static_chain(dir.path().to_path_buf(), Arc::new(NotFoundHandler));
	let (url, handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(&format!("{}/static/{}", url, filename))
		.await
		.expect("Request failed");

	// Assert
	assert_eq!(response.status(), 200);
	let content_type = response
		.headers()
		.get("content-type")
		.expect("Missing Content-Type header")
		.to_str()
		.unwrap();
	assert!(
		content_type.contains(expected_content_type),
		"Expected Content-Type containing '{}', got '{}'",
		expected_content_type,
		content_type
	);

	shutdown_test_server(handle).await;
}

// ===========================================================================
// Group B: Error response Content-Type
// ===========================================================================

#[rstest]
#[tokio::test]
async fn test_404_error_response_from_handler() {
	// Arrange
	let handler: Arc<dyn Handler> = Arc::new(NotFoundHandler);
	let (url, handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(&format!("{}/nonexistent", url))
		.await
		.expect("Request failed");

	// Assert
	assert_eq!(response.status(), 404);

	shutdown_test_server(handle).await;
}

#[rstest]
#[tokio::test]
async fn test_handler_error_produces_json_error_response() {
	// Arrange: ErrorHandler returns Err(...) which triggers Response::from(Error)
	let handler: Arc<dyn Handler> = Arc::new(ErrorHandler);
	let (url, handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(&format!("{}/anything", url))
		.await
		.expect("Request failed");

	// Assert: error conversion produces JSON body with application/json
	assert_eq!(response.status(), 500);
	let content_type = response
		.headers()
		.get("content-type")
		.expect("Missing Content-Type header on error response")
		.to_str()
		.unwrap();
	assert!(
		content_type.contains("application/json"),
		"Expected application/json Content-Type on error response, got '{}'",
		content_type
	);

	shutdown_test_server(handle).await;
}

// ===========================================================================
// Group C: Middleware does not overwrite handler-set headers
// ===========================================================================

#[rstest]
#[tokio::test]
async fn test_security_middleware_preserves_handler_content_type() {
	// Arrange
	let security_mw = Arc::new(SecurityMiddleware::new());
	let chain = MiddlewareChain::new(Arc::new(PlainTextHandler) as Arc<dyn Handler>)
		.with_middleware(security_mw);
	let (url, handle) = spawn_test_server(Arc::new(chain)).await;

	// Act
	let response = reqwest::get(&format!("{}/test", url))
		.await
		.expect("Request failed");

	// Assert: Content-Type preserved AND security headers added
	assert_eq!(response.status(), 200);

	let content_type = response
		.headers()
		.get("content-type")
		.expect("Missing Content-Type")
		.to_str()
		.unwrap();
	assert!(
		content_type.contains("text/plain"),
		"SecurityMiddleware overwrote Content-Type: expected text/plain, got '{}'",
		content_type
	);

	assert!(
		response.headers().get("x-content-type-options").is_some(),
		"Missing X-Content-Type-Options header"
	);

	shutdown_test_server(handle).await;
}

#[rstest]
#[tokio::test]
async fn test_static_file_content_type_preserved_with_security_middleware() {
	// Arrange
	let dir = create_static_files_dir();
	let handler = build_security_static_chain(dir.path().to_path_buf(), Arc::new(NotFoundHandler));
	let (url, handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(&format!("{}/static/style.css", url))
		.await
		.expect("Request failed");

	// Assert: CSS Content-Type preserved AND security headers present
	assert_eq!(response.status(), 200);

	let content_type = response
		.headers()
		.get("content-type")
		.expect("Missing Content-Type")
		.to_str()
		.unwrap();
	assert!(
		content_type.contains("text/css"),
		"Expected text/css, got '{}'",
		content_type
	);

	assert!(
		response.headers().get("x-content-type-options").is_some(),
		"Missing X-Content-Type-Options after security middleware"
	);
	assert!(
		response.headers().get("x-frame-options").is_some(),
		"Missing X-Frame-Options after security middleware"
	);

	shutdown_test_server(handle).await;
}

// ===========================================================================
// Group D: Security headers survive full pipeline
// ===========================================================================

#[rstest]
#[tokio::test]
async fn test_security_headers_present_on_static_file_response() {
	// Arrange
	let dir = create_static_files_dir();
	let handler = build_security_static_chain(dir.path().to_path_buf(), Arc::new(NotFoundHandler));
	let (url, handle) = spawn_test_server(handler).await;

	// Act
	let response = reqwest::get(&format!("{}/static/app.js", url))
		.await
		.expect("Request failed");

	// Assert
	assert_eq!(response.status(), 200);

	let headers = response.headers();
	assert_eq!(
		headers
			.get("x-content-type-options")
			.and_then(|v| v.to_str().ok()),
		Some("nosniff"),
		"X-Content-Type-Options should be nosniff"
	);
	assert_eq!(
		headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
		Some("DENY"),
		"X-Frame-Options should be DENY"
	);
	assert_eq!(
		headers.get("referrer-policy").and_then(|v| v.to_str().ok()),
		Some("same-origin"),
		"Referrer-Policy should be same-origin"
	);

	shutdown_test_server(handle).await;
}

#[rstest]
#[tokio::test]
async fn test_security_headers_present_on_error_response() {
	// Arrange
	let security_mw = Arc::new(SecurityMiddleware::new());
	let chain = MiddlewareChain::new(Arc::new(ErrorHandler) as Arc<dyn Handler>)
		.with_middleware(security_mw);
	let (url, handle) = spawn_test_server(Arc::new(chain)).await;

	// Act
	let response = reqwest::get(&format!("{}/anything", url))
		.await
		.expect("Request failed");

	// Assert: ErrorToResponseHandler converts handler Err(...) into
	// Ok(Response::from(Error)) before middleware post-processing runs.
	// This means security headers ARE added to error responses, because
	// the response flows back through the middleware chain normally.
	assert_eq!(response.status(), 500);

	let headers = response.headers();
	assert!(
		headers.get("x-content-type-options").is_some(),
		"Security headers should be present on error responses (ErrorToResponseHandler converts errors to responses before middleware post-processing)"
	);

	shutdown_test_server(handle).await;
}
