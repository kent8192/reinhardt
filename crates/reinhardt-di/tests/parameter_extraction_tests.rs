//! Behavioral tests for request parameter extraction.

#![cfg(feature = "params")]

use bytes::Bytes;
use http::header::{CONTENT_TYPE, COOKIE};
#[cfg(feature = "multipart")]
use reinhardt_di::params::Multipart;
use reinhardt_di::params::{
	Body, Cookie, CookieStruct, Form, FromRequest, Header, HeaderStruct, Json, ParamContext,
	ParamError, Query, Request,
};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct LoginForm {
	username: String,
	age: u32,
}

#[cfg(feature = "multipart")]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct MultipartForm {
	username: String,
	age: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct CookieValues {
	session_id: String,
	theme: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct HeaderValues {
	#[serde(rename = "x-api-key")]
	api_key: String,
	#[serde(rename = "x-request-id")]
	request_id: String,
}

#[cfg(feature = "multi-value-arrays")]
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct SearchQuery {
	page: i64,
	rating: f64,
	active: bool,
	term: String,
	tag: Vec<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct OptionalQuery {
	page: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct JsonPayload {
	name: String,
}

fn request_with_body(content_type: Option<&str>, body: impl Into<Bytes>) -> Request {
	let mut builder = Request::builder().uri("/parameters");
	if let Some(content_type) = content_type {
		builder = builder.header(CONTENT_TYPE, content_type);
	}
	builder.body(body.into()).build().unwrap()
}

#[tokio::test]
async fn cookie_extractors_distinguish_required_optional_and_structured_values() {
	// Arrange
	let request = Request::builder()
		.uri("/parameters")
		.header(COOKIE, "session_id=abc123; theme=dark")
		.body(Bytes::new())
		.build()
		.unwrap();
	let mut context = ParamContext::new();
	context.set_cookie_name::<String>("session_id");

	// Act
	let required = Cookie::<String>::from_request(&request, &context)
		.await
		.unwrap();
	let optional = Cookie::<Option<String>>::from_request(&request, &context)
		.await
		.unwrap();
	let structured = CookieStruct::<CookieValues>::from_request(&request, &context)
		.await
		.unwrap();

	// Assert
	assert_eq!(required.into_inner(), "abc123");
	assert_eq!(*optional, Some("abc123".to_owned()));
	assert_eq!(
		structured.into_inner(),
		CookieValues {
			session_id: "abc123".to_owned(),
			theme: "dark".to_owned(),
		}
	);
}

#[tokio::test]
async fn cookie_extractors_report_missing_required_values_without_failing_optional_values() {
	// Arrange
	let request = Request::builder()
		.uri("/parameters")
		.body(Bytes::new())
		.build()
		.unwrap();
	let empty_context = ParamContext::new();
	let mut named_context = ParamContext::new();
	named_context.set_cookie_name::<String>("session_id");

	// Act
	let unnamed = Cookie::<String>::from_request(&request, &empty_context).await;
	let missing = Cookie::<String>::from_request(&request, &named_context).await;
	let optional = Cookie::<Option<String>>::from_request(&request, &empty_context)
		.await
		.unwrap();

	// Assert
	assert!(
		matches!(unnamed, Err(ParamError::MissingParameter(name)) if name == "Cookie name not specified in ParamContext for this type")
	);
	assert!(matches!(missing, Err(ParamError::MissingParameter(name)) if name == "session_id"));
	assert_eq!(*optional, None);
}

#[tokio::test]
async fn cookie_struct_rejects_requests_missing_required_fields() {
	// Arrange
	let request = Request::builder()
		.uri("/parameters")
		.header(COOKIE, "session_id=abc123")
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let result = CookieStruct::<CookieValues>::from_request(&request, &ParamContext::new()).await;

	// Assert
	assert!(
		matches!(result, Err(ParamError::InvalidParameter(context)) if context.field_name.as_deref() == Some("cookies"))
	);
}

#[tokio::test]
async fn header_extractors_preserve_named_and_structured_values() {
	// Arrange
	let request = Request::builder()
		.uri("/parameters")
		.header("x-api-key", "secret")
		.header("x-request-id", "request-42")
		.body(Bytes::new())
		.build()
		.unwrap();
	let mut context = ParamContext::new();
	context.set_header_name::<String>("x-api-key");

	// Act
	let required = Header::<String>::from_request(&request, &context)
		.await
		.unwrap();
	let optional = Header::<Option<String>>::from_request(&request, &context)
		.await
		.unwrap();
	let structured = HeaderStruct::<HeaderValues>::from_request(&request, &context)
		.await
		.unwrap();

	// Assert
	assert_eq!(required.into_inner(), "secret");
	assert_eq!(*optional, Some("secret".to_owned()));
	assert_eq!(
		structured.into_inner(),
		HeaderValues {
			api_key: "secret".to_owned(),
			request_id: "request-42".to_owned(),
		}
	);
}

#[tokio::test]
async fn header_extractors_report_missing_required_values_without_failing_optional_values() {
	// Arrange
	let request = Request::builder()
		.uri("/parameters")
		.body(Bytes::new())
		.build()
		.unwrap();
	let empty_context = ParamContext::new();
	let mut named_context = ParamContext::new();
	named_context.set_header_name::<String>("x-api-key");

	// Act
	let unnamed = Header::<String>::from_request(&request, &empty_context).await;
	let missing = Header::<String>::from_request(&request, &named_context).await;
	let optional = Header::<Option<String>>::from_request(&request, &empty_context)
		.await
		.unwrap();

	// Assert
	assert!(
		matches!(unnamed, Err(ParamError::MissingParameter(name)) if name == "Header name not specified in ParamContext for this type")
	);
	assert!(matches!(missing, Err(ParamError::MissingParameter(name)) if name == "x-api-key"));
	assert_eq!(*optional, None);
}

#[tokio::test]
async fn header_struct_reports_deserialization_context_for_missing_fields() {
	// Arrange
	let request = Request::builder()
		.uri("/parameters")
		.header("x-api-key", "secret")
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let result = HeaderStruct::<HeaderValues>::from_request(&request, &ParamContext::new()).await;

	// Assert
	assert!(
		matches!(result, Err(ParamError::UrlEncodingError(context)) if context.raw_value.as_deref() == Some("x-api-key=secret"))
	);
}

#[tokio::test]
async fn urlencoded_form_extracts_values_and_exposes_wrapper_interfaces() {
	// Arrange
	let request = request_with_body(
		Some("application/x-www-form-urlencoded; charset=utf-8"),
		"username=alice&age=30",
	);

	// Act
	let form = Form::<LoginForm>::from_request(&request, &ParamContext::new())
		.await
		.unwrap();
	let debug = format!("{form:?}");

	// Assert
	assert_eq!(form.username, "alice");
	assert_eq!(debug, "LoginForm { username: \"alice\", age: 30 }");
	assert_eq!(
		form.into_inner(),
		LoginForm {
			username: "alice".to_owned(),
			age: 30,
		}
	);
}

#[tokio::test]
async fn urlencoded_form_rejects_invalid_content_and_values() {
	// Arrange
	let wrong_content_type = request_with_body(Some("application/json"), "{}");
	let malformed_value = request_with_body(
		Some("application/x-www-form-urlencoded"),
		"username=alice&age=old",
	);

	// Act
	let wrong_content_result =
		Form::<LoginForm>::from_request(&wrong_content_type, &ParamContext::new()).await;
	let malformed_result =
		Form::<LoginForm>::from_request(&malformed_value, &ParamContext::new()).await;

	// Assert
	assert!(
		matches!(wrong_content_result, Err(ParamError::InvalidParameter(context)) if context.field_name.as_deref() == Some("Content-Type"))
	);
	assert!(
		matches!(malformed_result, Err(ParamError::UrlEncodingError(context)) if context.raw_value.as_deref() == Some("username=alice&age=old"))
	);
}

#[tokio::test]
async fn urlencoded_form_enforces_the_body_size_limit() {
	// Arrange
	let oversized = vec![b'a'; 2 * 1024 * 1024 + 1];
	let request = request_with_body(Some("application/x-www-form-urlencoded"), oversized);

	// Act
	let result = Form::<LoginForm>::from_request(&request, &ParamContext::new()).await;

	// Assert
	assert!(
		matches!(result, Err(ParamError::PayloadTooLarge(message)) if message == "Form body size 2097153 bytes exceeds maximum allowed size of 2097152 bytes")
	);
}

#[tokio::test]
#[cfg(feature = "multipart")]
async fn multipart_form_extracts_text_fields_and_ignores_files() {
	// Arrange
	let boundary = "coverage-boundary";
	let body = format!(
		"--{boundary}\r\nContent-Disposition: form-data; name=\"username\"\r\n\r\nalice\r\n\
--{boundary}\r\nContent-Disposition: form-data; name=\"age\"\r\n\r\n30\r\n\
--{boundary}\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"avatar.txt\"\r\nContent-Type: text/plain\r\n\r\nignored\r\n\
--{boundary}--\r\n"
	);
	let request = request_with_body(
		Some(&format!("multipart/form-data; boundary={boundary}")),
		body,
	);

	// Act
	let form = Form::<MultipartForm>::from_request(&request, &ParamContext::new())
		.await
		.unwrap();

	// Assert
	assert_eq!(
		form.into_inner(),
		MultipartForm {
			username: "alice".to_owned(),
			age: "30".to_owned(),
		}
	);
}

#[tokio::test]
#[cfg(feature = "multipart")]
async fn multipart_form_reports_invalid_boundaries() {
	// Arrange
	let request = request_with_body(Some("multipart/form-data"), Bytes::new());

	// Act
	let result = Form::<LoginForm>::from_request(&request, &ParamContext::new()).await;

	// Assert
	assert!(
		matches!(result, Err(ParamError::InvalidParameter(context)) if context.field_name.as_deref() == Some("Content-Type") && context.raw_value.as_deref() == Some("multipart/form-data"))
	);
}

#[tokio::test]
#[cfg(feature = "multipart")]
async fn multipart_extractor_streams_named_fields() {
	// Arrange
	let boundary = "stream-boundary";
	let body = format!(
		"--{boundary}\r\nContent-Disposition: form-data; name=\"message\"\r\n\r\nhello\r\n--{boundary}--\r\n"
	);
	let request = request_with_body(
		Some(&format!("multipart/form-data; boundary={boundary}")),
		body,
	);

	// Act
	let mut multipart = Multipart::from_request(&request, &ParamContext::new())
		.await
		.unwrap();
	let field = multipart.next_field().await.unwrap().unwrap();
	let name = field.name().map(str::to_owned);
	let value = field.text().await.unwrap();
	let exhausted = multipart.next_field().await.unwrap();

	// Assert
	assert_eq!(name.as_deref(), Some("message"));
	assert_eq!(value, "hello");
	assert!(exhausted.is_none());
}

#[tokio::test]
#[cfg(feature = "multipart")]
async fn multipart_extractor_requires_a_valid_content_type() {
	// Arrange
	let missing = request_with_body(None, Bytes::new());
	let invalid = request_with_body(Some("text/plain"), Bytes::new());

	// Act
	let missing_result = Multipart::from_request(&missing, &ParamContext::new()).await;
	let invalid_result = Multipart::from_request(&invalid, &ParamContext::new()).await;

	// Assert
	assert!(
		matches!(missing_result, Err(ParamError::InvalidParameter(context)) if context.field_name.as_deref() == Some("content-type"))
	);
	assert!(
		matches!(invalid_result, Err(ParamError::InvalidParameter(context)) if context.field_name.as_deref() == Some("content-type"))
	);
}

#[tokio::test]
#[cfg(feature = "multi-value-arrays")]
async fn query_extractor_preserves_scalar_types_and_repeated_values() {
	// Arrange
	let request = Request::builder()
		.uri("/parameters?page=2&rating=4.5&active=true&term=rust&tag=5&tag=6")
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let query = Query::<SearchQuery>::from_request(&request, &ParamContext::new())
		.await
		.unwrap();
	let cloned = query.clone();
	let debug = format!("{query:?}");

	// Assert
	assert_eq!(query.page, 2);
	assert_eq!(query.tag, vec![5, 6]);
	assert_eq!(
		debug,
		"SearchQuery { page: 2, rating: 4.5, active: true, term: \"rust\", tag: [5, 6] }"
	);
	assert_eq!(cloned.into_inner(), query.into_inner());
}

#[tokio::test]
async fn query_extractor_handles_empty_and_invalid_queries() {
	// Arrange
	let empty = Request::builder()
		.uri("/parameters")
		.body(Bytes::new())
		.build()
		.unwrap();
	let invalid = Request::builder()
		.uri("/parameters?page=not-a-number")
		.body(Bytes::new())
		.build()
		.unwrap();

	// Act
	let optional = Query::<OptionalQuery>::from_request(&empty, &ParamContext::new())
		.await
		.unwrap();
	let invalid_result = Query::<OptionalQuery>::from_request(&invalid, &ParamContext::new()).await;

	// Assert
	assert_eq!(optional.into_inner(), OptionalQuery { page: None });
	#[cfg(feature = "multi-value-arrays")]
	assert!(
		matches!(invalid_result, Err(ParamError::InvalidParameter(context)) if context.raw_value.as_deref() == Some("page=not-a-number"))
	);
	#[cfg(not(feature = "multi-value-arrays"))]
	assert!(
		matches!(invalid_result, Err(ParamError::UrlEncodingError(context)) if context.raw_value.as_deref() == Some("page=not-a-number"))
	);
}

#[tokio::test]
async fn body_and_json_extractors_preserve_bytes_and_report_structured_errors() {
	// Arrange
	let body_request = request_with_body(Some("application/octet-stream"), "raw-body");
	let json_request = request_with_body(Some("application/json"), r#"{"name":42}"#);

	// Act
	let body = Body::from_request(&body_request, &ParamContext::new())
		.await
		.unwrap();
	let json_result = Json::<JsonPayload>::from_request(&json_request, &ParamContext::new()).await;

	// Assert
	assert_eq!(body.0, Bytes::from_static(b"raw-body"));
	assert!(
		matches!(json_result, Err(ParamError::DeserializationError(context)) if context.raw_value.as_deref() == Some(r#"{"name":42}"#))
	);
}
