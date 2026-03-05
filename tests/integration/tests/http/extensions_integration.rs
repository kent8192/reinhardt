//! Extensions Integration Tests
//!
//! These tests verify the integration of Extensions across multiple components:
//! - Data sharing between Request/Response components
//! - Type-safe Extensions insertion and retrieval
//! - Extensions lifecycle management
//! - Concurrent access to shared Extensions

use bytes::Bytes;
use hyper::{Method, StatusCode};
use reinhardt_http::{Error, Extensions, Request, Response};
use reinhardt_test::{ServerRouter as Router, api_client_from_url, test_server_guard};

#[derive(Debug, Clone, PartialEq)]
struct UserId(u64);

#[derive(Debug, Clone, PartialEq)]
struct SessionToken(String);

#[derive(Debug, Clone, PartialEq)]
struct RequestMetadata {
	client_ip: String,
	user_agent: String,
}

/// Test Extensions data sharing between Request and Response
#[tokio::test]
async fn test_extensions_request_response_sharing() {
	let mut router = Router::new();

	router = router.function("/user-info", Method::GET, |req: Request| async move {
		// Simulate middleware inserting user ID into extensions
		req.extensions.insert(UserId(12345));
		req.extensions.insert(SessionToken("abc123".to_string()));

		// Handler retrieves user ID from extensions
		let user_id = req.extensions.get::<UserId>().expect("UserId not found");
		let session = req
			.extensions
			.get::<SessionToken>()
			.expect("SessionToken not found");

		let response_body = format!(r#"{{"user_id":{},"session":"{}"}}"#, user_id.0, session.0);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/user-info").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();
	assert!(body.contains(r#""user_id":12345"#));
	assert!(body.contains(r#""session":"abc123""#));
}

/// Test type-safe Extensions insertion and retrieval
#[tokio::test]
async fn test_extensions_type_safety() {
	let mut router = Router::new();

	router = router.function("/type-check", Method::POST, |req: Request| async move {
		// Insert different types
		req.extensions.insert(42u32);
		req.extensions.insert("test string".to_string());
		req.extensions.insert(vec![1, 2, 3]);

		// Retrieve with type safety
		let num = req.extensions.get::<u32>().expect("u32 not found");
		let text = req.extensions.get::<String>().expect("String not found");
		let vec = req
			.extensions
			.get::<Vec<i32>>()
			.expect("Vec<i32> not found");

		// Wrong type should return None
		let wrong_type = req.extensions.get::<f64>();

		let response_body = format!(
			r#"{{"u32":{},"string":"{}","vec_len":{},"has_f64":{}}}"#,
			num,
			text,
			vec.len(),
			wrong_type.is_some()
		);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client
		.post_raw("/type-check", b"", "application/octet-stream")
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();
	assert!(body.contains(r#""u32":42"#));
	assert!(body.contains(r#""string":"test string""#));
	assert!(body.contains(r#""vec_len":3"#));
	assert!(body.contains(r#""has_f64":false"#));
}

/// Test Extensions lifecycle - insert, get, remove, clear
#[tokio::test]
async fn test_extensions_lifecycle() {
	let mut router = Router::new();

	router = router.function("/lifecycle", Method::GET, |_req: Request| async move {
		let extensions = Extensions::new();

		// Insert
		extensions.insert(UserId(100));
		extensions.insert(SessionToken("token1".to_string()));

		let phase1_has_user = extensions.contains::<UserId>();
		let phase1_has_session = extensions.contains::<SessionToken>();

		// Get
		let user_id = extensions.get::<UserId>();
		let session = extensions.get::<SessionToken>();

		// Remove
		let removed_user = extensions.remove::<UserId>();
		let phase3_has_user = extensions.contains::<UserId>();

		// Clear
		extensions.clear();
		let phase4_has_session = extensions.contains::<SessionToken>();

		let response_body = format!(
			r#"{{
						"phase1_has_user":{},
						"phase1_has_session":{},
						"phase2_user_id":{},
						"phase2_session":"{}",
						"phase3_removed_user":{},
						"phase3_has_user":{},
						"phase4_has_session":{}
					}}"#,
			phase1_has_user,
			phase1_has_session,
			user_id.as_ref().map(|u| u.0).unwrap_or(0),
			session.as_ref().map(|s| s.0.as_str()).unwrap_or(""),
			removed_user.is_some(),
			phase3_has_user,
			phase4_has_session
		);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/lifecycle").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();

	// Verify lifecycle behavior
	assert!(body.contains(r#""phase1_has_user":true"#));
	assert!(body.contains(r#""phase1_has_session":true"#));
	assert!(body.contains(r#""phase2_user_id":100"#));
	assert!(body.contains(r#""phase2_session":"token1""#));
	assert!(body.contains(r#""phase3_removed_user":true"#));
	assert!(body.contains(r#""phase3_has_user":false"#));
	assert!(body.contains(r#""phase4_has_session":false"#));
}

/// Test Extensions with complex custom types
#[tokio::test]
async fn test_extensions_complex_types() {
	let mut router = Router::new();

	router = router.function("/complex", Method::POST, |req: Request| async move {
		// Insert complex type
		let metadata = RequestMetadata {
			client_ip: "192.168.1.1".to_string(),
			user_agent: "TestClient/1.0".to_string(),
		};
		req.extensions.insert(metadata.clone());

		// Retrieve and verify
		let retrieved = req
			.extensions
			.get::<RequestMetadata>()
			.expect("RequestMetadata not found");

		assert_eq!(retrieved, metadata);

		let response_body = format!(
			r#"{{"client_ip":"{}","user_agent":"{}"}}"#,
			retrieved.client_ip, retrieved.user_agent
		);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client
		.post_raw("/complex", b"", "application/octet-stream")
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();
	assert!(body.contains(r#""client_ip":"192.168.1.1""#));
	assert!(body.contains(r#""user_agent":"TestClient/1.0""#));
}

/// Test Extensions cloning behavior
#[tokio::test]
async fn test_extensions_cloning() {
	let mut router = Router::new();

	router = router.function("/clone", Method::GET, |_req: Request| async move {
		let ext1 = Extensions::new();
		ext1.insert(UserId(999));

		// Clone extensions
		let ext2 = ext1.clone();

		// Both should have the same data
		let user_from_ext1 = ext1.get::<UserId>();
		let user_from_ext2 = ext2.get::<UserId>();

		// Modify ext2
		ext2.insert(SessionToken("new_token".to_string()));

		// ext1 should now also have the session (Arc<Mutex> shared state)
		let ext1_has_session = ext1.contains::<SessionToken>();

		let response_body = format!(
			r#"{{
						"ext1_user":{},
						"ext2_user":{},
						"ext1_has_session":{}
					}}"#,
			user_from_ext1.as_ref().map(|u| u.0).unwrap_or(0),
			user_from_ext2.as_ref().map(|u| u.0).unwrap_or(0),
			ext1_has_session
		);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/clone").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();

	// Both clones should see the same user ID
	assert!(body.contains(r#""ext1_user":999"#));
	assert!(body.contains(r#""ext2_user":999"#));
	// ext1 should see session added to ext2 (shared Arc<Mutex>)
	assert!(body.contains(r#""ext1_has_session":true"#));
}

/// Test Extensions across multiple middleware layers
#[tokio::test]
async fn test_extensions_middleware_chain() {
	let mut router = Router::new();

	router = router.function(
		"/middleware-chain",
		Method::GET,
		|req: Request| async move {
			// Simulate Layer 1: Authentication middleware
			req.extensions.insert(UserId(555));
			req.extensions
				.insert(SessionToken("auth_token".to_string()));

			// Simulate Layer 2: Request tracking middleware
			req.extensions.insert(RequestMetadata {
				client_ip: "10.0.0.1".to_string(),
				user_agent: "Browser/2.0".to_string(),
			});

			// Handler accesses all extensions
			let user_id = req.extensions.get::<UserId>();
			let session = req.extensions.get::<SessionToken>();
			let metadata = req.extensions.get::<RequestMetadata>();

			let response_body = format!(
				r#"{{
						"user_id":{},
						"session":"{}",
						"client_ip":"{}",
						"user_agent":"{}"
					}}"#,
				user_id.as_ref().map(|u| u.0).unwrap_or(0),
				session.as_ref().map(|s| s.0.as_str()).unwrap_or(""),
				metadata
					.as_ref()
					.map(|m| m.client_ip.as_str())
					.unwrap_or(""),
				metadata
					.as_ref()
					.map(|m| m.user_agent.as_str())
					.unwrap_or("")
			);

			let response = Response::ok()
				.with_header("Content-Type", "application/json")
				.with_body(Bytes::from(response_body));

			Ok::<Response, Error>(response)
		},
	);

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/middleware-chain").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();

	// Verify all middleware layers contributed data
	assert!(body.contains(r#""user_id":555"#));
	assert!(body.contains(r#""session":"auth_token""#));
	assert!(body.contains(r#""client_ip":"10.0.0.1""#));
	assert!(body.contains(r#""user_agent":"Browser/2.0""#));
}

/// Test Extensions with multiple requests (isolation)
#[tokio::test]
async fn test_extensions_request_isolation() {
	let mut router = Router::new();

	router = router.function("/isolated", Method::POST, |req: Request| async move {
		// Extract request-specific ID from body
		let body_str = String::from_utf8_lossy(req.body());
		let request_id: u64 = body_str.parse().unwrap_or(0);

		// Insert request-specific data
		req.extensions.insert(UserId(request_id));

		// Retrieve and echo back
		let user_id = req.extensions.get::<UserId>().expect("UserId not found");

		let response_body = format!(r#"{{"request_id":{}}}"#, user_id.0);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;

	// Send multiple concurrent requests
	let handles: Vec<_> = (1..=5)
		.map(|i| {
			let server_url = server.url.clone();
			tokio::spawn(async move {
				// Create a new APIClient for each spawned task
				let client = api_client_from_url(&server_url);
				let response = client
					.post_raw("/isolated", i.to_string().as_bytes(), "text/plain")
					.await
					.unwrap();

				let body = response.text();
				assert!(body.contains(&format!(r#""request_id":{}"#, i)));
			})
		})
		.collect();

	// Wait for all requests to complete
	for handle in handles {
		handle.await.unwrap();
	}
}

/// Test Extensions error handling when type not found
#[tokio::test]
async fn test_extensions_missing_type_handling() {
	let mut router = Router::new();

	router = router.function("/missing-type", Method::GET, |req: Request| async move {
		// Try to get a type that was never inserted
		let missing_user = req.extensions.get::<UserId>();
		let has_user = req.extensions.contains::<UserId>();

		let response_body = format!(
			r#"{{"missing_user_is_none":{},"has_user":{}}}"#,
			missing_user.is_none(),
			has_user
		);

		let response = Response::ok()
			.with_header("Content-Type", "application/json")
			.with_body(Bytes::from(response_body));

		Ok::<Response, Error>(response)
	});

	let server = test_server_guard(router).await;
	let client = api_client_from_url(&server.url);

	let response = client.get("/missing-type").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let body = response.text();
	assert!(body.contains(r#""missing_user_is_none":true"#));
	assert!(body.contains(r#""has_user":false"#));
}
