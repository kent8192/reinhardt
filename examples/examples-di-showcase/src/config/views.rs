//! Project-level views for examples-di-showcase
//!
//! Root and health check endpoints.

use reinhardt::core::serde::json;
use reinhardt::get;
use reinhardt::http::ViewResult;
use reinhardt::{Response, StatusCode};

/// Root endpoint — brief introduction to this example.
#[get("/", name = "root")]
pub async fn root() -> ViewResult<Response> {
	let body = json::json!({
		"example": "DI Showcase",
		"description": "FastAPI-style dependency injection with Reinhardt",
		"endpoints": [
			"GET  /di/config/              — app config (DI: AppConfig)",
			"GET  /di/greet/{name}/        — greet user (DI: GreetingService)",
			"GET  /di/counter/             — request counter (DI: RequestCounter, cached)",
			"POST /di/counter/uncached/    — uncached counter (DI: RequestCounter, cache=false)",
			"GET  /di/dashboard/{name}/    — dashboard (DI: DashboardService, nested deps)",
			"GET  /di/multiple/            — multiple deps (DI: AppConfig + GreetingService)",
			"GET  /health                  — health check",
		],
	});
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_string(&body)?.into_bytes()))
}

/// Health check endpoint.
#[get("/health", name = "health")]
pub async fn health() -> ViewResult<Response> {
	let body = json::json!({"status": "ok"});
	Ok(Response::new(StatusCode::OK)
		.with_header("Content-Type", "application/json")
		.with_body(json::to_string(&body)?.into_bytes()))
}
