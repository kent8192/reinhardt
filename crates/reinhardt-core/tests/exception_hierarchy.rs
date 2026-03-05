//! Tests for the Error hierarchy, status codes, ErrorKind mapping,
//! Display formatting, From conversions, and ParamErrorContext.

use reinhardt_core::exception::{Error, ErrorKind, ParamErrorContext, ParamType};
use rstest::rstest;

// ---------------------------------------------------------------------------
// 1. Parametrized status code tests for ALL Error variants
// ---------------------------------------------------------------------------

#[rstest]
#[case::http(Error::Http("bad request".to_string()), 400)]
#[case::serialization(Error::Serialization("invalid json".to_string()), 400)]
#[case::validation(Error::Validation("invalid email".to_string()), 400)]
#[case::body_already_consumed(Error::BodyAlreadyConsumed, 400)]
#[case::parse_error(Error::ParseError("not a number".to_string()), 400)]
#[case::missing_content_type(Error::MissingContentType, 400)]
#[case::invalid_page(Error::InvalidPage("must be positive".to_string()), 400)]
#[case::invalid_cursor(Error::InvalidCursor("bad base64".to_string()), 400)]
#[case::invalid_limit(Error::InvalidLimit("too large".to_string()), 400)]
#[case::missing_parameter(Error::MissingParameter("user_id".to_string()), 400)]
#[case::param_validation(
	Error::ParamValidation(Box::new(ParamErrorContext::new(ParamType::Json, "missing field"))),
	400
)]
#[case::authentication(Error::Authentication("invalid token".to_string()), 401)]
#[case::authorization(Error::Authorization("forbidden".to_string()), 403)]
#[case::not_found(Error::NotFound("resource missing".to_string()), 404)]
#[case::template_not_found(Error::TemplateNotFound("index.html".to_string()), 404)]
#[case::method_not_allowed(Error::MethodNotAllowed("PATCH not allowed".to_string()), 405)]
#[case::conflict(Error::Conflict("duplicate email".to_string()), 409)]
#[case::database(Error::Database("connection timeout".to_string()), 500)]
#[case::internal(Error::Internal("unexpected error".to_string()), 500)]
#[case::improperly_configured(Error::ImproperlyConfigured("missing DATABASE_URL".to_string()), 500)]
#[case::other(Error::Other(anyhow::anyhow!("unknown error")), 500)]
fn status_code_for_variant(#[case] error: Error, #[case] expected_status: u16) {
	// Act
	let status = error.status_code();

	// Assert
	assert_eq!(status, expected_status);
}

// ---------------------------------------------------------------------------
// 2. ErrorKind mapping verification for ALL variants
// ---------------------------------------------------------------------------

#[rstest]
#[case::http(Error::Http("x".to_string()), ErrorKind::Http)]
#[case::missing_content_type(Error::MissingContentType, ErrorKind::Http)]
#[case::database(Error::Database("x".to_string()), ErrorKind::Database)]
#[case::serialization(Error::Serialization("x".to_string()), ErrorKind::Serialization)]
#[case::validation(Error::Validation("x".to_string()), ErrorKind::Validation)]
#[case::invalid_page(Error::InvalidPage("x".to_string()), ErrorKind::Validation)]
#[case::invalid_cursor(Error::InvalidCursor("x".to_string()), ErrorKind::Validation)]
#[case::invalid_limit(Error::InvalidLimit("x".to_string()), ErrorKind::Validation)]
#[case::missing_parameter(Error::MissingParameter("x".to_string()), ErrorKind::Validation)]
#[case::authentication(Error::Authentication("x".to_string()), ErrorKind::Authentication)]
#[case::authorization(Error::Authorization("x".to_string()), ErrorKind::Authorization)]
#[case::not_found(Error::NotFound("x".to_string()), ErrorKind::NotFound)]
#[case::template_not_found(Error::TemplateNotFound("x".to_string()), ErrorKind::NotFound)]
#[case::method_not_allowed(Error::MethodNotAllowed("x".to_string()), ErrorKind::MethodNotAllowed)]
#[case::conflict(Error::Conflict("x".to_string()), ErrorKind::Conflict)]
#[case::internal(Error::Internal("x".to_string()), ErrorKind::Internal)]
#[case::improperly_configured(Error::ImproperlyConfigured("x".to_string()), ErrorKind::ImproperlyConfigured)]
#[case::body_already_consumed(Error::BodyAlreadyConsumed, ErrorKind::BodyAlreadyConsumed)]
#[case::parse_error(Error::ParseError("x".to_string()), ErrorKind::Parse)]
#[case::param_validation(
	Error::ParamValidation(Box::new(ParamErrorContext::new(ParamType::Query, "bad"))),
	ErrorKind::ParamValidation
)]
#[case::other(Error::Other(anyhow::anyhow!("x")), ErrorKind::Other)]
fn error_kind_for_variant(#[case] error: Error, #[case] expected_kind: ErrorKind) {
	// Act
	let kind = error.kind();

	// Assert
	assert_eq!(kind, expected_kind);
}

// ---------------------------------------------------------------------------
// 3. Display / to_string() message format verification
// ---------------------------------------------------------------------------

#[rstest]
#[case::http(Error::Http("bad request".to_string()), "HTTP error: bad request")]
#[case::database(Error::Database("timeout".to_string()), "Database error: timeout")]
#[case::serialization(Error::Serialization("invalid".to_string()), "Serialization error: invalid")]
#[case::validation(Error::Validation("email".to_string()), "Validation error: email")]
#[case::authentication(Error::Authentication("expired".to_string()), "Authentication error: expired")]
#[case::authorization(Error::Authorization("denied".to_string()), "Authorization error: denied")]
#[case::not_found(Error::NotFound("user 42".to_string()), "Not found: user 42")]
#[case::template_not_found(Error::TemplateNotFound("base.html".to_string()), "Template not found: base.html")]
#[case::method_not_allowed(Error::MethodNotAllowed("DELETE".to_string()), "Method not allowed: DELETE")]
#[case::conflict(Error::Conflict("duplicate".to_string()), "Conflict: duplicate")]
#[case::internal(Error::Internal("crash".to_string()), "Internal server error: crash")]
#[case::improperly_configured(Error::ImproperlyConfigured("missing key".to_string()), "Improperly configured: missing key")]
#[case::body_already_consumed(Error::BodyAlreadyConsumed, "Body already consumed")]
#[case::parse_error(Error::ParseError("NaN".to_string()), "Parse error: NaN")]
#[case::missing_content_type(Error::MissingContentType, "Missing Content-Type header")]
#[case::invalid_page(Error::InvalidPage("negative".to_string()), "Invalid page: negative")]
#[case::invalid_cursor(Error::InvalidCursor("bad".to_string()), "Invalid cursor: bad")]
#[case::invalid_limit(Error::InvalidLimit("zero".to_string()), "Invalid limit: zero")]
#[case::missing_parameter(Error::MissingParameter("id".to_string()), "Missing parameter: id")]
fn display_format_for_variant(#[case] error: Error, #[case] expected_display: &str) {
	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, expected_display);
}

// ---------------------------------------------------------------------------
// 4. From trait conversions
// ---------------------------------------------------------------------------

#[rstest]
fn from_serde_json_error() {
	// Arrange
	let json_err = serde_json::from_str::<i32>("not_a_number").unwrap_err();

	// Act
	let error: Error = json_err.into();

	// Assert
	assert_eq!(error.status_code(), 400);
	assert_eq!(error.kind(), ErrorKind::Serialization);
	assert!(error.to_string().contains("Serialization error"));
}

#[rstest]
fn from_io_error() {
	// Arrange
	let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");

	// Act
	let error: Error = io_err.into();

	// Assert
	assert_eq!(error.status_code(), 500);
	assert_eq!(error.kind(), ErrorKind::Internal);
	assert!(error.to_string().contains("IO error"));
	assert!(error.to_string().contains("access denied"));
}

#[rstest]
fn from_string() {
	// Arrange
	let msg = String::from("something broke");

	// Act
	let error: Error = msg.into();

	// Assert
	assert_eq!(error.status_code(), 500);
	assert_eq!(error.kind(), ErrorKind::Internal);
	assert_eq!(error.to_string(), "Internal server error: something broke");
}

#[rstest]
fn from_str_ref() {
	// Arrange
	let msg: &str = "static error message";

	// Act
	let error: Error = msg.into();

	// Assert
	assert_eq!(error.status_code(), 500);
	assert_eq!(error.kind(), ErrorKind::Internal);
	assert_eq!(
		error.to_string(),
		"Internal server error: static error message"
	);
}

#[rstest]
fn from_anyhow_error() {
	// Arrange
	let anyhow_err = anyhow::anyhow!("anyhow wrapped error");

	// Act
	let error: Error = anyhow_err.into();

	// Assert
	assert_eq!(error.status_code(), 500);
	assert_eq!(error.kind(), ErrorKind::Other);
	assert!(error.to_string().contains("anyhow wrapped error"));
}

#[rstest]
fn from_http_error() {
	// Arrange
	// Build an invalid HTTP request to trigger http::Error
	let http_err = http::Request::builder()
		.method("INVALID METHOD WITH SPACES")
		.body(())
		.unwrap_err();

	// Act
	let error: Error = http_err.into();

	// Assert
	assert_eq!(error.status_code(), 400);
	assert_eq!(error.kind(), ErrorKind::Http);
	assert!(error.to_string().contains("HTTP error"));
}

// ---------------------------------------------------------------------------
// 5. ParamErrorContext construction and format_error()
// ---------------------------------------------------------------------------

#[rstest]
fn param_error_context_basic() {
	// Arrange
	let ctx = ParamErrorContext::new(ParamType::Json, "missing field 'email'");

	// Act
	let formatted = ctx.format_error();

	// Assert
	assert!(formatted.contains("Json parameter extraction failed"));
	assert!(formatted.contains("error: missing field 'email'"));
}

#[rstest]
fn param_error_context_with_field() {
	// Arrange
	let ctx = ParamErrorContext::new(ParamType::Query, "invalid value").with_field("page_size");

	// Act
	let formatted = ctx.format_error();

	// Assert
	assert!(formatted.contains("Query parameter extraction failed"));
	assert!(formatted.contains("field: 'page_size'"));
	assert!(formatted.contains("error: invalid value"));
}

#[rstest]
fn param_error_context_with_expected_type() {
	// Arrange
	let ctx = ParamErrorContext::new(ParamType::Path, "type mismatch")
		.with_field("id")
		.with_expected_type::<u64>();

	// Act
	let formatted = ctx.format_error();

	// Assert
	assert!(formatted.contains("Path parameter extraction failed"));
	assert!(formatted.contains("field: 'id'"));
	assert!(formatted.contains("expected type: u64"));
}

#[rstest]
fn param_error_context_all_param_types() {
	// Arrange
	let types_and_labels = [
		(ParamType::Json, "Json"),
		(ParamType::Query, "Query"),
		(ParamType::Path, "Path"),
		(ParamType::Header, "Header"),
		(ParamType::Form, "Form"),
		(ParamType::Cookie, "Cookie"),
		(ParamType::Body, "Body"),
	];

	// Act
	let results: Vec<(String, &str)> = types_and_labels
		.iter()
		.map(|(param_type, label)| {
			let ctx = ParamErrorContext::new(*param_type, "test");
			(ctx.format_error(), *label)
		})
		.collect();

	// Assert
	for (formatted, label) in &results {
		assert!(
			formatted.contains(&format!("{} parameter extraction failed", label)),
			"Expected '{}' in formatted output: {}",
			label,
			formatted
		);
	}
}

#[rstest]
fn param_validation_error_display_uses_format_error() {
	// Arrange
	let ctx = ParamErrorContext::new(ParamType::Json, "missing field 'name'")
		.with_field("name")
		.with_expected_type::<String>();
	let error = Error::ParamValidation(Box::new(ctx.clone()));

	// Act
	let display = error.to_string();

	// Assert
	assert_eq!(display, ctx.format_error());
}

// ---------------------------------------------------------------------------
// 6. Error::Conflict status code and message
// ---------------------------------------------------------------------------

#[rstest]
fn conflict_error_status_and_message() {
	// Arrange
	let error = Error::Conflict("User with this email already exists".into());

	// Act
	let status = error.status_code();
	let kind = error.kind();
	let display = error.to_string();

	// Assert
	assert_eq!(status, 409);
	assert_eq!(kind, ErrorKind::Conflict);
	assert_eq!(display, "Conflict: User with this email already exists");
}

// ---------------------------------------------------------------------------
// 7. Pagination errors: InvalidPage, InvalidCursor, InvalidLimit
// ---------------------------------------------------------------------------

#[rstest]
fn pagination_invalid_page() {
	// Arrange
	let error = Error::InvalidPage("page must be positive".into());

	// Act
	let status = error.status_code();
	let kind = error.kind();
	let display = error.to_string();

	// Assert
	assert_eq!(status, 400);
	assert_eq!(kind, ErrorKind::Validation);
	assert_eq!(display, "Invalid page: page must be positive");
}

#[rstest]
fn pagination_invalid_cursor() {
	// Arrange
	let error = Error::InvalidCursor("invalid base64 encoding".into());

	// Act
	let status = error.status_code();
	let kind = error.kind();
	let display = error.to_string();

	// Assert
	assert_eq!(status, 400);
	assert_eq!(kind, ErrorKind::Validation);
	assert_eq!(display, "Invalid cursor: invalid base64 encoding");
}

#[rstest]
fn pagination_invalid_limit() {
	// Arrange
	let error = Error::InvalidLimit("limit must be between 1 and 100".into());

	// Act
	let status = error.status_code();
	let kind = error.kind();
	let display = error.to_string();

	// Assert
	assert_eq!(status, 400);
	assert_eq!(kind, ErrorKind::Validation);
	assert_eq!(display, "Invalid limit: limit must be between 1 and 100");
}

// ---------------------------------------------------------------------------
// Additional: ParamErrorContext multiline format
// ---------------------------------------------------------------------------

#[rstest]
fn param_error_context_format_multiline() {
	// Arrange
	let ctx = ParamErrorContext::new(ParamType::Form, "deserialization failed")
		.with_field("username")
		.with_expected_type::<String>()
		.with_raw_value("12345");

	// Act
	let multiline = ctx.format_multiline(true);

	// Assert
	assert!(multiline.contains("Form parameter extraction failed"));
	assert!(multiline.contains("Error: deserialization failed"));
	assert!(multiline.contains("Field: username"));
	assert!(multiline.contains("Expected type:"));
	assert!(multiline.contains("Received: 12345"));
}

#[rstest]
fn param_error_context_format_multiline_hides_raw_value() {
	// Arrange
	let ctx =
		ParamErrorContext::new(ParamType::Header, "invalid").with_raw_value("sensitive-token");

	// Act
	let multiline = ctx.format_multiline(false);

	// Assert
	assert!(!multiline.contains("sensitive-token"));
	assert!(!multiline.contains("Received:"));
}
