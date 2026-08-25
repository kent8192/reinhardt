//! Unit tests for `FromRequest`, `PathParam<T>`, `QueryParam<T>` extractors
//! (spec §4.3 of Manouche DSL v2). Refs #4668 / P7 part 2.

use rstest::rstest;
use std::collections::HashMap;

use reinhardt_urls::routers::client_router::from_request::{
	ExtractError, FromRequest, OptionalQueryParam, PathParam, QueryParam, RouteContext,
};

#[derive(Debug)]
struct StubProps {
	id: String,
}

impl FromRequest for StubProps {
	fn from_request(ctx: &RouteContext) -> Result<Self, ExtractError> {
		ctx.path_param("id")
			.ok_or_else(|| ExtractError::MissingPath("id".to_string()))
			.map(|id| StubProps { id })
	}
}

#[rstest]
fn from_request_reads_path_param() {
	// Arrange
	let mut params = HashMap::new();
	params.insert("id".to_string(), "42".to_string());
	let ctx = RouteContext::new("/users/42".to_string(), params, "".to_string());

	// Act
	let p = StubProps::from_request(&ctx).unwrap();

	// Assert
	assert_eq!(p.id, "42");
}

#[rstest]
fn from_request_returns_extract_error_when_missing() {
	// Arrange
	let ctx = RouteContext::new("/users/".to_string(), HashMap::new(), "".to_string());

	// Act
	let err = StubProps::from_request(&ctx).unwrap_err();

	// Assert
	assert!(matches!(err, ExtractError::MissingPath(s) if s == "id"));
}

#[rstest]
fn path_param_extracts_and_parses_i32() {
	// Arrange
	let mut params = HashMap::new();
	params.insert("id".to_string(), "42".to_string());
	let ctx = RouteContext::new("/users/42".to_string(), params, "".to_string());

	// Act
	let p: PathParam<i32> = PathParam::extract(&ctx, "id").unwrap();

	// Assert
	assert_eq!(p.into_inner(), 42);
}

#[rstest]
fn path_param_returns_missing_path_when_absent() {
	// Arrange
	let ctx = RouteContext::new("/".to_string(), HashMap::new(), "".to_string());

	// Act
	let err = PathParam::<i32>::extract(&ctx, "id").unwrap_err();

	// Assert
	assert!(matches!(err, ExtractError::MissingPath(n) if n == "id"));
}

#[rstest]
fn path_param_returns_parse_error_on_invalid() {
	// Arrange
	let mut params = HashMap::new();
	params.insert("id".to_string(), "abc".to_string());
	let ctx = RouteContext::new("/users/abc".to_string(), params, "".to_string());

	// Act
	let err = PathParam::<i32>::extract(&ctx, "id").unwrap_err();

	// Assert
	assert!(matches!(err, ExtractError::Parse { ref name, .. } if name == "id"));
}

#[rstest]
fn query_param_extracts_string() {
	// Arrange
	let ctx = RouteContext::new("/x".to_string(), HashMap::new(), "tab=settings".to_string());

	// Act
	let q: QueryParam<String> = QueryParam::extract(&ctx, "tab").unwrap();

	// Assert
	assert_eq!(q.into_inner(), "settings");
}

#[rstest]
fn query_param_returns_missing_query_when_absent() {
	// Arrange
	let ctx = RouteContext::new("/x".to_string(), HashMap::new(), "".to_string());

	// Act
	let err = QueryParam::<String>::extract(&ctx, "tab").unwrap_err();

	// Assert
	assert!(matches!(err, ExtractError::MissingQuery(n) if n == "tab"));
}

#[rstest]
fn query_param_returns_parse_error_on_invalid() {
	// Arrange
	let ctx = RouteContext::new(
		"/x".to_string(),
		HashMap::new(),
		"n=not-a-number".to_string(),
	);

	// Act
	let err = QueryParam::<i32>::extract(&ctx, "n").unwrap_err();

	// Assert
	assert!(matches!(err, ExtractError::Parse { ref name, .. } if name == "n"));
}

#[rstest]
fn query_param_handles_multiple_pairs() {
	// Arrange
	let ctx = RouteContext::new(
		"/x".to_string(),
		HashMap::new(),
		"a=1&b=hello&c=3".to_string(),
	);

	// Act
	let b: QueryParam<String> = QueryParam::extract(&ctx, "b").unwrap();

	// Assert
	assert_eq!(b.into_inner(), "hello");
}

#[rstest]
fn query_param_url_decodes_percent_encoded_values() {
	// Arrange
	let ctx = RouteContext::new(
		"/x".to_string(),
		HashMap::new(),
		"q=hello%20world".to_string(),
	);

	// Act
	let q: QueryParam<String> = QueryParam::extract(&ctx, "q").unwrap();

	// Assert
	assert_eq!(q.into_inner(), "hello world");
}

#[rstest]
fn query_param_decodes_plus_as_space() {
	// Arrange
	let ctx = RouteContext::new("/x".to_string(), HashMap::new(), "q=a+b".to_string());

	// Act
	let q: QueryParam<String> = QueryParam::extract(&ctx, "q").unwrap();

	// Assert
	assert_eq!(q.into_inner(), "a b");
}

#[rstest]
fn optional_query_param_returns_none_when_absent() {
	let ctx = RouteContext::new("/x".to_string(), HashMap::new(), "".to_string());

	let query = OptionalQueryParam::<i32>::extract(&ctx, "n").unwrap();

	assert_eq!(query.into_inner(), None);
}

#[rstest]
fn optional_query_param_parses_present_value() {
	let ctx = RouteContext::new("/x".to_string(), HashMap::new(), "n=42".to_string());

	let query = OptionalQueryParam::<i32>::extract(&ctx, "n").unwrap();

	assert_eq!(query.into_inner(), Some(42));
}

#[rstest]
fn optional_query_param_returns_parse_error_for_invalid_present_value() {
	let ctx = RouteContext::new(
		"/x".to_string(),
		HashMap::new(),
		"n=not-a-number".to_string(),
	);

	let error = OptionalQueryParam::<i32>::extract(&ctx, "n").unwrap_err();

	assert!(matches!(error, ExtractError::Parse { ref name, .. } if name == "n"));
}

#[rstest]
fn optional_query_param_keeps_present_empty_string() {
	let ctx = RouteContext::new("/x".to_string(), HashMap::new(), "q=".to_string());

	let query = OptionalQueryParam::<String>::extract(&ctx, "q").unwrap();

	assert_eq!(query.into_inner(), Some(String::new()));
}
