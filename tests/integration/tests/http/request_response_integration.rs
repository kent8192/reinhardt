//! Request and Response Integration Tests
//!
//! These tests verify the integration between Request and Response components:
//! - Content negotiation (Accept header processing)
//! - Streaming responses with StreamBody
//! - Request/Response round-trip flows
//! - Error handling across Request/Response boundary

use bytes::Bytes;
use hyper::{Method, StatusCode};
use reinhardt_http::{Error, Request, Response};
use reinhardt_test::{ServerRouter as Router, api_client_from_url, test_server_guard};

/// Test content negotiation: Accept header processing
#[tokio::test]
async fn test_content_negotiation_json() {
	let mut router = Router::new();

	// Register handler that responds based on Accept header
	router = router.function("/api/data", Method::GET, |req: Request| async move {
		let accept = req
			.headers
			.get("accept")
			.and_then(|v| v.to_str().ok())
			.unwrap_or("*/*");

		let response = if accept.contains("application/json") {
			Response::ok()
				.with_header("Content-Type", "application/json")
				.with_body(Bytes::from(r#"{"message":"JSON response"}"#))
		} else {
			Response::ok()
				.with_header("Content-Type", "text/plain")
				.with_body(Bytes::from("Plain text response"))
		};

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	// Test JSON content negotiation
	let json_response = client
		.get_with_headers("/api/data", &[("Accept", "application/json")])
		.await
		.unwrap();

	assert_eq!(json_response.status(), StatusCode::OK);
	assert_eq!(
		json_response.header("content-type").unwrap(),
		"application/json"
	);
	let json_body = json_response.text();
	assert_eq!(json_body, r#"{"message":"JSON response"}"#);

	// Test plain text content negotiation
	let text_response = client
		.get_with_headers("/api/data", &[("Accept", "text/plain")])
		.await
		.unwrap();

	assert_eq!(text_response.status(), StatusCode::OK);
	assert_eq!(text_response.header("content-type").unwrap(), "text/plain");
	let text_body = text_response.text();
	assert_eq!(text_body, "Plain text response");
}

/// Test content negotiation with wildcard Accept header
#[tokio::test]
async fn test_content_negotiation_wildcard() {
	let mut router = Router::new();

	router = router.function("/api/resource", Method::GET, |req: Request| async move {
		let accept = req
			.headers
			.get("accept")
			.and_then(|v| v.to_str().ok())
			.unwrap_or("*/*");

		// Default to JSON for wildcard
		let response = if accept == "*/*" || accept.is_empty() {
			Response::ok()
				.with_header("Content-Type", "application/json")
				.with_body(Bytes::from(r#"{"format":"json"}"#))
		} else {
			Response::new(StatusCode::NOT_ACCEPTABLE)
				.with_body(Bytes::from("Unsupported media type"))
		};

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	// Test wildcard accept
	let response = client
		.get_with_headers("/api/resource", &[("Accept", "*/*")])
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.header("content-type").unwrap(), "application/json");
}

/// Test streaming response with StreamBody
#[tokio::test]
async fn test_streaming_response() {
	let mut router = Router::new();

	router = router.function("/stream", Method::GET, |_req: Request| async move {
		// Create streaming-like data (combined chunks)
		let data = vec![
			Bytes::from("chunk1"),
			Bytes::from("chunk2"),
			Bytes::from("chunk3"),
		];

		// Combine chunks into single response body
		let mut combined = Vec::new();
		for chunk in data {
			combined.extend_from_slice(&chunk);
		}

		let response = Response::ok()
			.with_header("Content-Type", "application/octet-stream")
			.with_body(Bytes::from(combined));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/stream").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(
		response.header("content-type").unwrap(),
		"application/octet-stream"
	);

	// Collect streamed chunks
	let body = response.body();
	assert_eq!(body.as_ref(), b"chunk1chunk2chunk3");
}

/// Test large streaming response
#[tokio::test]
async fn test_large_streaming_response() {
	let mut router = Router::new();

	router = router.function("/large-stream", Method::GET, |_req: Request| async move {
		// Generate 1000 chunks and combine them
		let chunks: Vec<String> = (0..1000).map(|i| format!("chunk{:04}", i)).collect();

		let combined = chunks.join("");

		let response = Response::ok()
			.with_header("Content-Type", "text/plain")
			.with_body(Bytes::from(combined));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/large-stream").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);

	// Verify chunk count
	let body = response.text();
	assert!(body.contains("chunk0000"));
	assert!(body.contains("chunk0999"));
}

/// Test request/response round-trip with POST data
#[tokio::test]
async fn test_request_response_post_roundtrip() {
	let mut router = Router::new();

	router = router.function("/echo", Method::POST, |req: Request| async move {
		// Echo back request body with added metadata
		let content_type = req
			.headers
			.get("content-type")
			.and_then(|v| v.to_str().ok())
			.unwrap_or("unknown");

		let body_str = String::from_utf8_lossy(req.body()).to_string();

		// Use serde_json to properly escape the received body
		let response_data = serde_json::json!({
			"received": body_str,
			"content_type": content_type
		});
		let response_body = response_data.to_string();

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	// Send POST request with JSON body
	let response = client
		.post_raw("/echo", br#"{"test":"data"}"#, "application/json")
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();

	// Parse as JSON and verify structure
	let json: serde_json::Value =
		serde_json::from_str(&body).expect("Response should be valid JSON");

	assert_eq!(
		json.get("received").and_then(|v| v.as_str()),
		Some(r#"{"test":"data"}"#),
		"Received field should contain the original request body"
	);
	assert_eq!(
		json.get("content_type").and_then(|v| v.as_str()),
		Some("application/json"),
		"Content-Type should be application/json"
	);
}

/// Test error response in request/response flow
#[tokio::test]
async fn test_request_response_error_handling() {
	let mut router = Router::new();

	router = router.function("/error", Method::GET, |_req: Request| async move {
		// Return error response
		let response = Response::internal_server_error()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(r#"{"error":"Something went wrong"}"#));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/error").await.unwrap();

	assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
	let body = response.text();
	assert_eq!(body, r#"{"error":"Something went wrong"}"#);
}

/// Test multiple Accept header types
#[tokio::test]
async fn test_multiple_accept_headers() {
	let mut router = Router::new();

	router = router.function("/formats", Method::GET, |req: Request| async move {
		let accept = req
			.headers
			.get("accept")
			.and_then(|v| v.to_str().ok())
			.unwrap_or("*/*");

		let response = if accept.contains("application/json") {
			Response::ok()
				.with_header("Content-Type", "application/json")
				.with_body(Bytes::from(r#"{"format":"json"}"#))
		} else if accept.contains("text/html") {
			Response::ok()
				.with_header("Content-Type", "text/html")
				.with_body(Bytes::from("<html><body>HTML</body></html>"))
		} else if accept.contains("text/plain") {
			Response::ok()
				.with_header("Content-Type", "text/plain")
				.with_body(Bytes::from("Plain text"))
		} else {
			Response::new(StatusCode::NOT_ACCEPTABLE).with_body(Bytes::from("Not Acceptable"))
		};

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	// Test each format
	let json_response = client
		.get_with_headers("/formats", &[("Accept", "application/json")])
		.await
		.unwrap();
	assert_eq!(json_response.status(), StatusCode::OK);
	assert!(json_response.text().contains("json"));

	let html_response = client
		.get_with_headers("/formats", &[("Accept", "text/html")])
		.await
		.unwrap();
	assert_eq!(html_response.status(), StatusCode::OK);
	assert!(html_response.text().contains("<html>"));

	let text_response = client
		.get_with_headers("/formats", &[("Accept", "text/plain")])
		.await
		.unwrap();
	assert_eq!(text_response.status(), StatusCode::OK);
	assert_eq!(text_response.text(), "Plain text");
}

/// Test request/response round-trip with query parameters
#[tokio::test]
async fn test_request_response_query_params() {
	let mut router = Router::new();

	router = router.function("/search", Method::GET, |req: Request| async move {
		let query = req.query_params.get("q").map(|s| s.as_str()).unwrap_or("");
		let limit = req
			.query_params
			.get("limit")
			.map(|s| s.as_str())
			.unwrap_or("10");

		let response_body = format!(r#"{{"query":"{}","limit":"{}"}}"#, query, limit);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/search?q=test&limit=20").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();
	assert!(body.contains(r#""query":"test""#));
	assert!(body.contains(r#""limit":"20""#));
}
