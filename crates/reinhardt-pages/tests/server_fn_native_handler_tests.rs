#![cfg(not(target_arch = "wasm32"))]
//! Native server function handler regression tests.

use bytes::Bytes;
use hyper::{Method, header};
use reinhardt_core::parsers::UploadedFile;
use reinhardt_core::validators::{ValidationError, ValidationErrors};
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
async fn save(name: String, avatar: Option<UploadedFile>) -> Result<String, ServerFnError> {
	Ok(format!("{name}:{}", avatar.map_or(0, |file| file.size)))
}

#[server_fn]
async fn save_required(name: String, avatar: UploadedFile) -> Result<String, ServerFnError> {
	Ok(format!(
		"{name}:{}:{}",
		avatar.filename.unwrap_or_default(),
		avatar.size
	))
}

#[server_fn]
async fn inspect_optional_file(
	avatar: Option<UploadedFile>,
) -> Result<Option<(String, usize)>, ServerFnError> {
	Ok(avatar.map(|file| (file.filename.unwrap_or_default(), file.size)))
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

async fn ensure_cluster_name_available(name: &str) -> Result<(), ValidationErrors> {
	let mut errors = ValidationErrors::new();
	if name == "taken" {
		errors.add(
			"_all",
			ValidationError::Custom("Cluster creation was rejected".to_owned()),
		);
		errors.add(
			"name",
			ValidationError::Custom("A cluster with this name already exists".to_owned()),
		);
	}
	if errors.is_empty() {
		Ok(())
	} else {
		Err(errors)
	}
}

#[server_fn]
async fn validate_cluster_server_boundary(name: String) -> Result<(), ServerFnError> {
	ensure_cluster_name_available(&name).await?;
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

#[server_fn(pre_validate = true)]
async fn save_validated_user(
	request: CreateUserRequest,
	avatar: Option<UploadedFile>,
) -> Result<String, ServerFnError> {
	Ok(format!(
		"{}:{}",
		request.email,
		avatar.map_or(0, |file| file.size)
	))
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

struct CsrfToken {
	authorization: String,
	csrf: String,
}

#[async_trait::async_trait]
impl FromRequest for CsrfToken {
	async fn from_request(request: &Request, _context: &ParamContext) -> ParamResult<Self> {
		Ok(Self {
			authorization: request
				.headers
				.get(header::AUTHORIZATION)
				.and_then(|value| value.to_str().ok())
				.unwrap_or_default()
				.to_owned(),
			csrf: request
				.headers
				.get("x-csrftoken")
				.and_then(|value| value.to_str().ok())
				.unwrap_or_default()
				.to_owned(),
		})
	}
}

#[server_fn]
async fn save_with_request_headers(
	name: String,
	avatar: Option<UploadedFile>,
	headers: CsrfToken,
) -> Result<String, ServerFnError> {
	Ok(format!(
		"{name}:{}:{}:{}",
		avatar.map_or(0, |file| file.size),
		headers.authorization,
		headers.csrf
	))
}

#[server_fn]
async fn save_with_collision_names(
	arguments: String,
	__req: String,
	result: String,
	value: String,
	e: String,
	error_json: String,
	avatar: Option<UploadedFile>,
	__param_ctx: CsrfToken,
) -> Result<String, ServerFnError> {
	Ok(format!(
		"{arguments}:{__req}:{result}:{value}:{e}:{error_json}:{}:{}:{}",
		avatar.map_or(0, |file| file.size),
		__param_ctx.authorization,
		__param_ctx.csrf
	))
}

const MULTIPART_BOUNDARY: &str = "reinhardt-server-fn-boundary";
const INVALID_REQUEST_BODY: &[u8] = br#"{"version":1,"kind":"server","status":400,"message":"Invalid server function request","field_errors":[]}"#;

enum MultipartTestPart<'a> {
	Field {
		name: &'a str,
		data: &'a [u8],
	},
	File {
		name: &'a str,
		filename: &'a str,
		data: &'a [u8],
	},
}

fn multipart_body(parts: &[MultipartTestPart<'_>]) -> Bytes {
	let mut body = Vec::new();
	for part in parts {
		body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}\r\n").as_bytes());
		match part {
			MultipartTestPart::Field { name, data } => {
				body.extend_from_slice(
					format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
				);
				body.extend_from_slice(data);
			}
			MultipartTestPart::File {
				name,
				filename,
				data,
			} => {
				body.extend_from_slice(
					format!(
						"Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
					)
					.as_bytes(),
				);
				body.extend_from_slice(data);
			}
		}
		body.extend_from_slice(b"\r\n");
	}
	body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}--\r\n").as_bytes());
	Bytes::from(body)
}

fn multipart_request(path: &str, body: Bytes) -> Request {
	Request::builder()
		.method(Method::POST)
		.uri(path)
		.header(
			header::CONTENT_TYPE,
			format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
		)
		.body(body)
		.build()
		.expect("multipart request should build")
}

fn assert_invalid_request(body: Bytes) {
	assert_eq!(body, Bytes::from_static(INVALID_REQUEST_BODY));
}

#[rstest]
#[tokio::test]
async fn multipart_valid_scalar_and_file_calls_server_function() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/save",
		multipart_body(&[
			MultipartTestPart::Field {
				name: "name",
				data: br#""Ada""#,
			},
			MultipartTestPart::File {
				name: "avatar",
				filename: "avatar.txt",
				data: b"abc",
			},
		]),
	);

	// Act
	let body = save::marker::handle(request)
		.await
		.expect("valid multipart request should reach the server function");

	// Assert
	assert_eq!(body, Bytes::from_static(br#""Ada:3""#));
}

#[rstest]
#[tokio::test]
async fn multipart_required_file_and_scalar_call_server_function() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/save_required",
		multipart_body(&[
			MultipartTestPart::Field {
				name: "name",
				data: br#""Ada""#,
			},
			MultipartTestPart::File {
				name: "avatar",
				filename: "avatar.txt",
				data: b"abc",
			},
		]),
	);

	// Act
	let body = save_required::marker::handle(request)
		.await
		.expect("required multipart file should reach the server function");

	// Assert
	assert_eq!(body, Bytes::from_static(br#""Ada:avatar.txt:3""#));
}

#[rstest]
#[tokio::test]
async fn multipart_known_parts_are_accepted_in_any_order() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/save",
		multipart_body(&[
			MultipartTestPart::File {
				name: "avatar",
				filename: "avatar.txt",
				data: b"abc",
			},
			MultipartTestPart::Field {
				name: "name",
				data: br#""Ada""#,
			},
		]),
	);

	// Act
	let body = save::marker::handle(request)
		.await
		.expect("known multipart arguments should be accepted in any order");

	// Assert
	assert_eq!(body, Bytes::from_static(br#""Ada:3""#));
}

#[rstest]
#[tokio::test]
async fn multipart_absent_optional_file_becomes_none() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/save",
		multipart_body(&[MultipartTestPart::Field {
			name: "name",
			data: br#""Ada""#,
		}]),
	);

	// Act
	let body = save::marker::handle(request)
		.await
		.expect("omitted optional file should be accepted");

	// Assert
	assert_eq!(body, Bytes::from_static(br#""Ada:0""#));
}

#[rstest]
#[tokio::test]
async fn multipart_argument_mismatches_return_the_same_invalid_request_response() {
	// Arrange
	let cases = [
		(
			"unknown part",
			multipart_body(&[
				MultipartTestPart::Field {
					name: "name",
					data: br#""Ada""#,
				},
				MultipartTestPart::Field {
					name: "unknown",
					data: b"null",
				},
			]),
		),
		(
			"duplicate scalar",
			multipart_body(&[
				MultipartTestPart::Field {
					name: "name",
					data: br#""Ada""#,
				},
				MultipartTestPart::Field {
					name: "name",
					data: br#""Grace""#,
				},
			]),
		),
		(
			"duplicate file",
			multipart_body(&[
				MultipartTestPart::Field {
					name: "name",
					data: br#""Ada""#,
				},
				MultipartTestPart::File {
					name: "avatar",
					filename: "one.txt",
					data: b"one",
				},
				MultipartTestPart::File {
					name: "avatar",
					filename: "two.txt",
					data: b"two",
				},
			]),
		),
		(
			"missing scalar",
			multipart_body(&[MultipartTestPart::File {
				name: "avatar",
				filename: "avatar.txt",
				data: b"abc",
			}]),
		),
		(
			"file for scalar",
			multipart_body(&[MultipartTestPart::File {
				name: "name",
				filename: "name.txt",
				data: b"Ada",
			}]),
		),
		(
			"scalar for file",
			multipart_body(&[
				MultipartTestPart::Field {
					name: "name",
					data: br#""Ada""#,
				},
				MultipartTestPart::Field {
					name: "avatar",
					data: br#""abc""#,
				},
			]),
		),
		(
			"malformed scalar JSON",
			multipart_body(&[MultipartTestPart::Field {
				name: "name",
				data: b"Ada",
			}]),
		),
		(
			"invalid scalar UTF-8",
			multipart_body(&[MultipartTestPart::Field {
				name: "name",
				data: b"\xff",
			}]),
		),
	];

	for (case, body) in cases {
		// Act
		let error_body = save::marker::handle(multipart_request("/api/server_fn/save", body))
			.await
			.expect_err(case);

		// Assert
		assert_eq!(
			error_body,
			Bytes::from_static(INVALID_REQUEST_BODY),
			"{case}"
		);
	}
}

#[rstest]
#[tokio::test]
async fn multipart_required_empty_file_is_rejected() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/save_required",
		multipart_body(&[
			MultipartTestPart::Field {
				name: "name",
				data: br#""Ada""#,
			},
			MultipartTestPart::File {
				name: "avatar",
				filename: "",
				data: b"",
			},
		]),
	);

	// Act
	let body = save_required::marker::handle(request)
		.await
		.expect_err("required empty file should be rejected");

	// Assert
	assert_invalid_request(body);
}

#[rstest]
#[tokio::test]
async fn multipart_optional_empty_file_becomes_none() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/inspect_optional_file",
		multipart_body(&[MultipartTestPart::File {
			name: "avatar",
			filename: "",
			data: b"",
		}]),
	);

	// Act
	let body = inspect_optional_file::marker::handle(request)
		.await
		.expect("optional empty file should be accepted");

	// Assert
	assert_eq!(body, Bytes::from_static(b"null"));
}

#[rstest]
#[tokio::test]
async fn multipart_named_zero_byte_file_remains_uploaded_file() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/inspect_optional_file",
		multipart_body(&[MultipartTestPart::File {
			name: "avatar",
			filename: "empty.txt",
			data: b"",
		}]),
	);

	// Act
	let body = inspect_optional_file::marker::handle(request)
		.await
		.expect("named zero-byte file should be accepted");

	// Assert
	assert_eq!(body, Bytes::from_static(br#"["empty.txt",0]"#));
}

#[rstest]
#[tokio::test]
async fn multipart_per_file_size_limit_rejects_request() {
	// Arrange
	let oversized = vec![b'a'; 10 * 1024 * 1024 + 1];
	let request = multipart_request(
		"/api/server_fn/save",
		multipart_body(&[
			MultipartTestPart::Field {
				name: "name",
				data: br#""Ada""#,
			},
			MultipartTestPart::File {
				name: "avatar",
				filename: "avatar.bin",
				data: &oversized,
			},
		]),
	);

	// Act
	let body = save::marker::handle(request)
		.await
		.expect_err("oversized file should be rejected");

	// Assert
	assert_invalid_request(body);
}

#[rstest]
#[tokio::test]
async fn multipart_total_size_limit_rejects_request() {
	// Arrange
	let chunk = vec![b'a'; 9 * 1024 * 1024];
	let request = multipart_request(
		"/api/server_fn/save",
		multipart_body(&[
			MultipartTestPart::Field {
				name: "one",
				data: &chunk,
			},
			MultipartTestPart::Field {
				name: "two",
				data: &chunk,
			},
			MultipartTestPart::Field {
				name: "three",
				data: &chunk,
			},
			MultipartTestPart::Field {
				name: "four",
				data: &chunk,
			},
			MultipartTestPart::Field {
				name: "five",
				data: &chunk,
			},
			MultipartTestPart::Field {
				name: "six",
				data: &chunk,
			},
		]),
	);

	// Act
	let body = save::marker::handle(request)
		.await
		.expect_err("oversized multipart body should be rejected");

	// Assert
	assert_invalid_request(body);
}

#[rstest]
#[tokio::test]
async fn multipart_extractors_observe_authorization_and_csrf_headers() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/save_with_request_headers")
		.header(
			header::CONTENT_TYPE,
			format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
		)
		.header(header::AUTHORIZATION, "Bearer token")
		.header("x-csrftoken", "csrf-token")
		.body(multipart_body(&[MultipartTestPart::Field {
			name: "name",
			data: br#""Ada""#,
		}]))
		.build()
		.expect("multipart request should build");

	// Act
	let body = save_with_request_headers::marker::handle(request)
		.await
		.expect("multipart extractor should observe request headers");

	// Assert
	assert_eq!(
		body,
		Bytes::from_static(br#""Ada:0:Bearer token:csrf-token""#)
	);
}

#[rstest]
#[tokio::test]
async fn multipart_generated_bindings_do_not_collide_with_parameter_names() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/save_with_collision_names")
		.header(
			header::CONTENT_TYPE,
			format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
		)
		.header(header::AUTHORIZATION, "Bearer token")
		.header("x-csrftoken", "csrf-token")
		.body(multipart_body(&[
			MultipartTestPart::Field {
				name: "arguments",
				data: br#""arguments""#,
			},
			MultipartTestPart::Field {
				name: "__req",
				data: br#""request""#,
			},
			MultipartTestPart::Field {
				name: "result",
				data: br#""result""#,
			},
			MultipartTestPart::Field {
				name: "value",
				data: br#""value""#,
			},
			MultipartTestPart::Field {
				name: "e",
				data: br#""error""#,
			},
			MultipartTestPart::Field {
				name: "error_json",
				data: br#""error-json""#,
			},
		]))
		.build()
		.expect("multipart request should build");

	// Act
	let body = save_with_collision_names::marker::handle(request)
		.await
		.expect("generated bindings should not collide with parameter names");

	// Assert
	assert_eq!(
		body,
		Bytes::from_static(
			br#""arguments:request:result:value:error:error-json:0:Bearer token:csrf-token""#,
		)
	);
}

#[rstest]
#[tokio::test]
async fn multipart_pre_validate_checks_json_arguments() {
	// Arrange
	let request = multipart_request(
		"/api/server_fn/save_validated_user",
		multipart_body(&[MultipartTestPart::Field {
			name: "request",
			data: br#"{"email":"invalid"}"#,
		}]),
	);

	// Act
	let body = save_validated_user::marker::handle(request)
		.await
		.expect_err("invalid multipart JSON argument should fail validation");
	let error: ServerFnError = serde_json::from_slice(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(save_validated_user::marker::error_status(&body), 422);
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.field_errors()[0].field(), "email");
}

#[rstest]
#[tokio::test]
async fn non_multipart_json_request_and_response_remain_byte_for_byte_unchanged() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/echo_name")
		.header(header::CONTENT_TYPE, "application/json")
		.body(Bytes::from_static(br#"{"name":"Ada"}"#))
		.build()
		.expect("JSON request should build");

	// Act
	let body = echo_name::marker::handle(request)
		.await
		.expect("JSON request should remain accepted");

	// Assert
	assert_eq!(body, Bytes::from_static(br#""Ada""#));
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

#[rstest]
#[tokio::test]
async fn validation_errors_from_async_cluster_checks_route_field_and_form_errors() {
	// Arrange
	let request = Request::builder()
		.method(Method::POST)
		.uri("/api/server_fn/validate_cluster_server_boundary")
		.body(Bytes::from_static(br#"{"name":"taken"}"#))
		.build()
		.expect("cluster validation request should build");

	// Act
	let body = validate_cluster_server_boundary::marker::handle(request)
		.await
		.expect_err("async cluster validation should reject a taken name");
	let error: ServerFnError = serde_json::from_slice(&body).expect("error should be valid JSON");

	// Assert
	assert_eq!(
		validate_cluster_server_boundary::marker::error_status(&body),
		422
	);
	assert_eq!(error.kind(), ServerFnErrorKind::Validation);
	assert_eq!(error.status(), Some(422));
	assert_eq!(error.field_errors().len(), 2);
	assert_eq!(error.field_errors()[0].field(), "name");
	assert_eq!(
		error.field_errors()[0].message(),
		"A cluster with this name already exists"
	);
	assert_eq!(error.field_errors()[1].field(), "_all");
	assert_eq!(
		error.field_errors()[1].message(),
		"Cluster creation was rejected"
	);
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
