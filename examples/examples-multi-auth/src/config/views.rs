//! Root-level views

use reinhardt::{Request, Response, StatusCode, ViewResult, get};

/// Health check endpoint
#[get("/health", name = "health_check")]
pub async fn health_check(_req: Request) -> ViewResult<Response> {
	Ok(Response::new(StatusCode::OK).with_body("OK"))
}
