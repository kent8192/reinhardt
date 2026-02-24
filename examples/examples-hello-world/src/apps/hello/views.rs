//! View handlers for hello app

use reinhardt::core::serde::json::json;
use reinhardt::{Request, Response, ViewResult, get};

/// Root endpoint - returns "Hello, World!"
#[get("/", name = "hello_world")]
pub async fn hello_world(_req: Request) -> ViewResult<Response> {
	Ok(Response::ok().with_body("Hello, World!"))
}

/// Health check endpoint - returns JSON status
#[get("/health", name = "health_check")]
pub async fn health_check(_req: Request) -> ViewResult<Response> {
	let body = json!({
		"status": "ok"
	});

	Response::ok().with_json(&body)
}
