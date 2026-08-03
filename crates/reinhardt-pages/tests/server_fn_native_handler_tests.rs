#![cfg(not(target_arch = "wasm32"))]
//! Native server function handler regression tests.

use bytes::Bytes;
use hyper::{Method, header};
use reinhardt_di::params::{FromRequest, ParamContext, ParamError, ParamResult};
use reinhardt_http::Request;
use reinhardt_pages::server_fn::{
	ServerFnError, ServerFnErrorKind, ServerFnRegistration, server_fn,
};
use rstest::rstest;

#[server_fn]
async fn echo_name(name: String) -> Result<String, ServerFnError> {
	Ok(name)
}

#[server_fn]
async fn echo_alias(name: String) -> Result<String, ServerFnError> {
	Ok(name)
}

#[server_fn]
async fn invalid_choice(choice_id: String) -> Result<(), ServerFnError> {
	if choice_id.is_empty() {
		return Err(ServerFnError::validation_with_message(
			"Validation failed",
			[("choice_id", "Select a choice")],
		));
	}

	Ok(())
}

#[derive(serde::Deserialize, serde::Serialize, reinhardt_core::validators::Validate)]
struct CreateUserRequest {
	#[validate(email)]
	email: String,
}

#[server_fn(pre_validate = true)]
async fn create_validated_user(request: CreateUserRequest) -> Result<String, ServerFnError> {
	Ok(request.email)
}

#[derive(serde::Serialize)]
struct CustomServerError(String);

impl From<ServerFnError> for CustomServerError {
	fn from(error: ServerFnError) -> Self {
		Self(error.to_string())
	}
}

#[server_fn]
async fn custom_error() -> Result<(), CustomServerError> {
	Err(CustomServerError("Custom failure".to_string()))
}

struct Authorization;

#[async_trait::async_trait]
impl FromRequest for Authorization {
	async fn from_request(_request: &Request, _context: &ParamContext) -> ParamResult<Self> {
		Err(ParamError::Authentication("token=top-secret".to_string()))
	}
}

struct SessionId;

#[async_trait::async_trait]
impl FromRequest for SessionId {
	async fn from_request(_request: &Request, _context: &ParamContext) -> ParamResult<Self> {
		Err(ParamError::Internal(
			"database password=top-secret".to_string(),
		))
	}
}

struct Header;

#[async_trait::async_trait]
impl FromRequest for Header {
	async fn from_request(_request: &Request, _context: &ParamContext) -> ParamResult<Self> {
		Err(ParamError::BodyError(
			"request token=top-secret is malformed".to_string(),
		))
	}
}

#[server_fn]
async fn authentication_extractor(_authorization: Authorization) -> Result<(), ServerFnError> {
	Ok(())
}

#[server_fn]
async fn internal_extractor(_session_id: SessionId) -> Result<(), ServerFnError> {
	Ok(())
}

#[server_fn]
async fn parameter_extractor(_header: Header) -> Result<(), ServerFnError> {
	Ok(())
}

#[tokio::test]
async fn authentication_extractor_returns_sanitized_unauthorized_error() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/authentication_extractor")
		.build()
		.expect("request should build");

	// Act
	let body = authentication_extractor::marker::handle(request)
		.await
		.expect_err("authentication extractor should reject the request");

	let error = serde_json::from_slice::<ServerFnError>(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(authentication_extractor::marker::error_status(&body), 401);
	assert_eq!(error.kind(), ServerFnErrorKind::Auth);
	assert_eq!(error.status(), Some(401));
	assert_eq!(error.message(), "Authentication required");
	assert!(
		!String::from_utf8(body.to_vec())
			.unwrap()
			.contains("top-secret")
	);
}

#[tokio::test]
async fn internal_extractor_returns_sanitized_internal_error() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/internal_extractor")
		.build()
		.expect("request should build");

	// Act
	let body = internal_extractor::marker::handle(request)
		.await
		.expect_err("internal extractor should reject the request");

	let error = serde_json::from_slice::<ServerFnError>(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(internal_extractor::marker::error_status(&body), 500);
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(500));
	assert_eq!(error.message(), "Internal server error");
	assert!(
		!String::from_utf8(body.to_vec())
			.unwrap()
			.contains("top-secret")
	);
}

#[tokio::test]
async fn parameter_extractor_returns_sanitized_bad_request_error() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/parameter_extractor")
		.build()
		.expect("request should build");

	// Act
	let body = parameter_extractor::marker::handle(request)
		.await
		.expect_err("parameter extractor should reject the request");
	let error = serde_json::from_slice::<ServerFnError>(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(parameter_extractor::marker::error_status(&body), 400);
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(400));
	assert_eq!(error.message(), "Parameter extraction failed");
	assert!(
		!String::from_utf8(body.to_vec())
			.unwrap()
			.contains("top-secret")
	);
}

#[tokio::test]
async fn validation_handler_returns_versioned_error_envelope() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/invalid_choice")
		.body(Bytes::from_static(br#"{"choice_id":""}"#))
		.build()
		.expect("request should build");

	// Act
	let body = invalid_choice::marker::handle(request)
		.await
		.expect_err("validation error should reject the request");
	let error: ServerFnError = serde_json::from_slice(&body).expect("error should be valid JSON");
	let value: serde_json::Value = serde_json::from_slice(&body).expect("error should be JSON");

	// Assert
	assert_eq!(invalid_choice::marker::error_status(&body), 422);
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(value["version"], 1);
	assert_eq!(error.field_errors()[0].field(), "choice_id");
	assert_eq!(error.field_errors()[0].message(), "Select a choice");
}

#[tokio::test]
async fn pre_validate_rejects_invalid_dto_before_invocation() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/create_validated_user")
		.body(Bytes::from_static(br#"{"request":{"email":"invalid"}}"#))
		.build()
		.expect("request should build");

	// Act
	let body = create_validated_user::marker::handle(request)
		.await
		.expect_err("pre-validation should reject an invalid DTO");
	let error: ServerFnError = serde_json::from_slice(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(create_validated_user::marker::error_status(&body), 422);
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.field_errors()[0].field(), "email");
}

#[tokio::test]
async fn pre_validate_invokes_endpoint_for_valid_dto() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/create_validated_user")
		.body(Bytes::from_static(
			br#"{"request":{"email":"user@example.com"}}"#,
		))
		.build()
		.expect("request should build");

	// Act
	let body = create_validated_user::marker::handle(request)
		.await
		.expect("valid DTO should reach the endpoint");

	// Assert
	assert_eq!(
		serde_json::from_slice::<String>(&body).expect("response should be valid JSON"),
		"user@example.com"
	);
}

#[tokio::test]
async fn custom_error_handler_returns_a_versioned_error_envelope() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/custom_error")
		.build()
		.expect("request should build");

	// Act
	let body = custom_error::marker::handle(request)
		.await
		.expect_err("custom error should reject the request");
	let error: ServerFnError = serde_json::from_slice(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(custom_error::marker::error_status(&body), 500);
	assert_eq!(error.kind(), ServerFnErrorKind::Application);
	assert_eq!(error.status(), Some(500));
	assert_eq!(error.message(), "Custom failure");
}

#[tokio::test]
async fn malformed_server_fn_request_returns_bad_request_envelope() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/echo_name")
		.body(Bytes::from_static(br#"{"name":}"#))
		.build()
		.expect("request should build");

	// Act
	let body = echo_name::marker::handle(request)
		.await
		.expect_err("malformed request should be rejected");
	let error: ServerFnError = serde_json::from_slice(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(echo_name::marker::error_status(&body), 400);
	assert_eq!(error.kind(), ServerFnErrorKind::Server);
	assert_eq!(error.status(), Some(400));
	assert_eq!(error.message(), "Invalid server function request");
}

#[test]
fn http_response_error_decoding_preserves_envelopes_and_sanitizes_raw_bodies() {
	// Arrange
	let envelope = serde_json::json!({
		"version": 1,
		"kind": "validation",
		"status": null,
		"message": "Choose a value",
		"field_errors": [{ "field": "choice_id", "message": "Select a choice" }],
	})
	.to_string();

	// Act
	let structured = ServerFnError::from_http_response(422, &envelope);
	let fallback = ServerFnError::from_http_response(502, "database password=top-secret");

	// Assert
	assert_eq!(structured.kind(), ServerFnErrorKind::Validation);
	assert_eq!(structured.status(), Some(422));
	assert_eq!(structured.field_errors()[0].field(), "choice_id");
	assert_eq!(fallback.kind(), ServerFnErrorKind::Deserialization);
	assert_eq!(fallback.status(), Some(502));
	assert_eq!(fallback.message(), "Invalid server function error response");
	assert!(!fallback.message().contains("top-secret"));
}

#[test]
fn http_response_error_decoding_normalizes_invalid_outer_statuses() {
	// Arrange
	let envelope_without_status = serde_json::json!({
		"version": 1,
		"kind": "validation",
		"status": null,
		"message": "Choose a value",
		"field_errors": [],
	})
	.to_string();

	// Act
	let zero_outer_status = ServerFnError::from_http_response(0, &envelope_without_status);
	let invalid_outer_status = ServerFnError::from_http_response(700, "not an error envelope");

	// Assert
	assert_eq!(zero_outer_status.kind(), ServerFnErrorKind::Validation);
	assert_eq!(zero_outer_status.status(), Some(500));
	assert_eq!(
		invalid_outer_status.kind(),
		ServerFnErrorKind::Deserialization
	);
	assert_eq!(invalid_outer_status.status(), Some(500));
}

#[tokio::test]
async fn json_server_fn_accepts_form_content_type_without_extractors() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/echo_name")
		.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
		.body(Bytes::from_static(b"name=Alice"))
		.build()
		.expect("request should build");

	// Act
	let body = echo_name::marker::handle(request)
		.await
		.expect("server function should accept form-encoded input");
	let name: String = serde_json::from_slice(&body).expect("response should be JSON");

	// Assert
	assert_eq!(name, "Alice");
}

#[rstest]
fn generated_query_key_helper_encodes_server_fn_identity_and_args() {
	// Act
	let family = echo_name::family();
	let echo_key = echo_name::key("Alice".to_string());
	let query = echo_name::query("Alice".to_string());
	let alias_key = echo_alias::key("Alice".to_string());

	// Assert
	assert_eq!(family.id(), "server_fn:/api/server_fn/echo_name:json");
	assert_eq!(query.key(), &echo_key);
	assert_eq!(
		echo_key.id(),
		"server_fn:/api/server_fn/echo_name:json:sha256:ab576365fddb09f8b9117212e0d01bf2b8ce8202923d6cff26034af8dfd88e15"
	);
	assert_eq!(
		alias_key.id(),
		"server_fn:/api/server_fn/echo_alias:json:sha256:ab576365fddb09f8b9117212e0d01bf2b8ce8202923d6cff26034af8dfd88e15"
	);
	assert_ne!(echo_key.id(), alias_key.id());
}
