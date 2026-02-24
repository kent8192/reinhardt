//! Integration tests for Server implementation

use bytes::Bytes;
use http::StatusCode;
use reinhardt_di::params::Path;
use reinhardt_http::{Request, Response, ViewResult};
use reinhardt_macros::{get, post};
use reinhardt_test::APIClient;
use reinhardt_test::fixtures::*;
use reinhardt_urls::routers::ServerRouter as Router;

// Handler for basic request test
#[get("/test", name = "test")]
async fn test_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body("Server works!"))
}

// Handler for hello endpoint
#[get("/hello", name = "hello")]
async fn hello_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body("Hello!"))
}

// Handler for goodbye endpoint
#[get("/goodbye", name = "goodbye")]
async fn goodbye_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body("Goodbye!"))
}

// Handler for submit endpoint (POST with body access)
#[post("/submit", name = "submit")]
async fn submit_handler(req: Request) -> ViewResult<Response> {
	let body_str = String::from_utf8(req.body().to_vec()).unwrap_or_default();
	Ok(Response::ok().with_body(format!("Received: {}", body_str)))
}

// Handler for JSON echo endpoint
#[post("/echo", name = "echo")]
async fn echo_handler(req: Request) -> ViewResult<Response> {
	let json: serde_json::Value = req.json()?;
	Response::ok().with_json(&json)
}

// Handler for exists endpoint
#[get("/exists", name = "exists")]
async fn exists_handler() -> ViewResult<Response> {
	Ok(Response::ok().with_body("I exist!"))
}

// Handler for concurrent test
#[get("/test", name = "concurrent_test")]
async fn concurrent_test_handler(req: Request) -> ViewResult<Response> {
	Ok(Response::ok().with_body(Bytes::from(format!("Method: {}", req.method))))
}

// Handler for headers test
#[get("/headers", name = "headers")]
async fn headers_handler(req: Request) -> ViewResult<Response> {
	let user_agent = req
		.headers
		.get(hyper::header::USER_AGENT)
		.and_then(|v| v.to_str().ok())
		.unwrap_or("Unknown");
	Ok(Response::ok().with_body(format!("User-Agent: {}", user_agent)))
}

// Handler for path parameters test
#[get("/users/{id}", name = "get_user")]
async fn get_user_handler(Path(id): Path<String>) -> ViewResult<Response> {
	Ok(Response::ok().with_body(format!("ID: {}", id)))
}

#[tokio::test]
async fn test_server_basic_request() {
	let router = Router::new().endpoint(test_handler);

	let server = test_server_guard(router).await;

	// Make HTTP request using APIClient
	let client = APIClient::with_base_url(&server.url);
	let response = client.get("/test").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "Server works!");
}

#[tokio::test]
async fn test_server_multiple_requests() {
	let router = Router::new()
		.endpoint(hello_handler)
		.endpoint(goodbye_handler);

	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);

	let response = client.get("/hello").await.unwrap();
	assert_eq!(response.text(), "Hello!");

	let response = client.get("/goodbye").await.unwrap();
	assert_eq!(response.text(), "Goodbye!");
}

#[tokio::test]
async fn test_server_post_request() {
	let router = Router::new().endpoint(submit_handler);

	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);
	let response = client
		.post_raw("/submit", b"test data", "text/plain")
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "Received: test data");
}

#[tokio::test]
async fn test_server_json_request_response() {
	let router = Router::new().endpoint(echo_handler);

	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);
	let test_data = serde_json::json!({
		"name": "Alice",
		"age": 30
	});

	let response = client.post("/echo", &test_data, "json").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	let response_json = response.json_value().unwrap();
	assert_eq!(response_json["name"], "Alice");
	assert_eq!(response_json["age"], 30);
}

#[tokio::test]
async fn test_server_404_response() {
	let router = Router::new().endpoint(exists_handler);

	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);
	let response = client.get("/notfound").await.unwrap();

	assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_server_concurrent_requests() {
	// Use a unique handler for this test to avoid name collision
	#[get("/test", name = "concurrent_test_inner")]
	async fn concurrent_inner_handler(req: Request) -> ViewResult<Response> {
		Ok(Response::ok().with_body(Bytes::from(format!("Method: {}", req.method))))
	}

	let router = Router::new().endpoint(concurrent_inner_handler);

	let server = test_server_guard(router).await;

	// Make multiple concurrent requests
	let mut handles = vec![];
	for _ in 0..10 {
		let url = server.url.clone();
		let handle = tokio::spawn(async move {
			let client = APIClient::with_base_url(&url);
			client.get("/test").await.unwrap().text()
		});
		handles.push(handle);
	}

	// Wait for all requests to complete
	for handle in handles {
		let result = handle.await.unwrap();
		assert_eq!(result, "Method: GET");
	}
}

#[tokio::test]
async fn test_server_custom_headers() {
	let router = Router::new().endpoint(headers_handler);

	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);
	let response = client
		.get_with_headers("/headers", &[("User-Agent", "TestAgent/1.0")])
		.await
		.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "User-Agent: TestAgent/1.0");
}

#[tokio::test]
async fn test_server_path_parameters() {
	let router = Router::new().endpoint(get_user_handler);

	let server = test_server_guard(router).await;

	let client = APIClient::with_base_url(&server.url);
	let response = client.get("/users/123").await.unwrap();

	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text(), "ID: 123");
}
