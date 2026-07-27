//! Raw request and typed extractor route integration tests.

use bytes::Bytes;
use hyper::{Method, header};
use reinhardt_di::params::{Json, Path};
use reinhardt_http::{Handler, Request, Response, ViewResult};
use reinhardt_macros::{get, post};
use reinhardt_urls::routers::ServerRouter;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct ImportRequest {
	title: String,
}

#[get("/books/import/{job_id}", name = "raw-request-with-path")]
async fn get_import_job(req: Request, Path(job_id): Path<String>) -> ViewResult<Response> {
	let cookie = req.get_header("cookie").unwrap_or_default();
	Ok(Response::ok().with_body(format!("{job_id}:{cookie}")))
}

#[post("/books/import", name = "raw-request-with-json")]
async fn create_import_job(
	Json(payload): Json<ImportRequest>,
	req: Request,
) -> ViewResult<Response> {
	let content_type = req.get_header("content-type").unwrap_or_default();
	Ok(Response::ok().with_body(format!("{}:{content_type}", payload.title)))
}

#[tokio::test]
async fn raw_request_can_be_combined_with_path_extractor() {
	let router = ServerRouter::new().endpoint(get_import_job);
	let request = Request::builder()
		.method(Method::GET)
		.uri("/books/import/job-42")
		.header(header::COOKIE, "session=abc123")
		.build()
		.expect("request should be valid");

	let response = router
		.handle(request)
		.await
		.expect("request should dispatch");

	assert_eq!(response.body, Bytes::from_static(b"job-42:session=abc123"));
}

#[tokio::test]
async fn raw_request_can_follow_json_extractor() {
	let router = ServerRouter::new().endpoint(create_import_job);
	let payload = ImportRequest {
		title: "Rust Patterns".to_string(),
	};
	let request = Request::builder()
		.method(Method::POST)
		.uri("/books/import")
		.header(header::CONTENT_TYPE, "application/json")
		.body(Bytes::from(
			serde_json::to_vec(&payload).expect("payload should serialize"),
		))
		.build()
		.expect("request should be valid");

	let response = router
		.handle(request)
		.await
		.expect("request should dispatch");

	assert_eq!(
		response.body,
		Bytes::from_static(b"Rust Patterns:application/json")
	);
}
