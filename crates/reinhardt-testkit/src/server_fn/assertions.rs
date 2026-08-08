//! Server function assertion helpers.
//!
//! This module provides assertion utilities for testing server function results,
//! including success/error checking, status code validation, and content verification.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_testkit::server_fn::assertions::{ServerFnResultAssertions, assert_server_fn_returns};
//!
//! let result = my_server_fn(input, &ctx).await;
//!
//! // Using trait methods
//! result.should_be_ok();
//! result.should_have_value(&expected);
//!
//! // Using standalone functions
//! assert_server_fn_returns(&result, &expected);
//! ```

#![cfg(native)]

use std::fmt::Debug;

use http::StatusCode;

/// Extension trait for asserting server function results.
///
/// This trait provides fluent assertion methods for `Result` types
/// returned by server functions.
pub trait ServerFnResultAssertions<T, E> {
	/// Assert that the result is `Ok` and return the value.
	fn should_be_ok(&self) -> &T;

	/// Assert that the result is `Err` and return the error.
	fn should_be_err(&self) -> &E;

	/// Assert that the result is `Ok` and the value equals the expected value.
	fn should_have_value(&self, expected: &T)
	where
		T: PartialEq + Debug;

	/// Assert that the result is `Ok` and the value satisfies a predicate.
	fn should_satisfy<F>(&self, predicate: F)
	where
		F: FnOnce(&T) -> bool;
}

impl<T: Debug, E: Debug> ServerFnResultAssertions<T, E> for Result<T, E> {
	fn should_be_ok(&self) -> &T {
		match self {
			Ok(value) => value,
			Err(e) => panic!("Expected Ok result, but got Err: {:?}", e),
		}
	}

	fn should_be_err(&self) -> &E {
		match self {
			Ok(value) => panic!("Expected Err result, but got Ok: {:?}", value),
			Err(e) => e,
		}
	}

	fn should_have_value(&self, expected: &T)
	where
		T: PartialEq + Debug,
	{
		let actual = self.should_be_ok();
		assert_eq!(
			actual, expected,
			"Expected value {:?}, but got {:?}",
			expected, actual
		);
	}

	fn should_satisfy<F>(&self, predicate: F)
	where
		F: FnOnce(&T) -> bool,
	{
		let value = self.should_be_ok();
		assert!(
			predicate(value),
			"Value {:?} did not satisfy the predicate",
			value
		);
	}
}

/// Extension trait for error assertions.
pub trait ServerFnErrorAssertions<E> {
	/// Assert that the error message contains the specified text.
	fn should_contain_message(&self, expected: &str)
	where
		E: std::fmt::Display;

	/// Assert that the error message matches exactly.
	fn should_have_message(&self, expected: &str)
	where
		E: std::fmt::Display;
}

impl<E: Debug> ServerFnErrorAssertions<E> for E {
	fn should_contain_message(&self, expected: &str)
	where
		E: std::fmt::Display,
	{
		let message = self.to_string();
		assert!(
			message.contains(expected),
			"Expected error message to contain '{}', but got '{}'",
			expected,
			message
		);
	}

	fn should_have_message(&self, expected: &str)
	where
		E: std::fmt::Display,
	{
		let message = self.to_string();
		assert_eq!(
			message, expected,
			"Expected error message '{}', but got '{}'",
			expected, message
		);
	}
}

/// Assert that a server function returns a specific value.
pub fn assert_server_fn_returns<T, E>(result: &Result<T, E>, expected: &T)
where
	T: PartialEq + Debug,
	E: Debug,
{
	result.should_have_value(expected);
}

/// Assert that a server function returns an error.
pub fn assert_server_fn_error<T, E>(result: &Result<T, E>)
where
	T: Debug,
	E: Debug,
{
	let _ = result.should_be_err();
}

/// Assert that a server function error contains the specified message.
pub fn assert_server_fn_error_contains<T, E>(result: &Result<T, E>, message: &str)
where
	T: Debug,
	E: Debug + std::fmt::Display,
{
	let error = result.should_be_err();
	error.should_contain_message(message);
}

/// Assert that a validation error occurred for a specific field.
///
/// This checks if the error message mentions the specified field name,
/// which is a common pattern for validation errors.
pub fn assert_validation_error<T, E>(result: &Result<T, E>, field: &str)
where
	T: Debug,
	E: Debug + std::fmt::Display,
{
	let error = result.should_be_err();
	let message = error.to_string();
	assert!(
		message.to_lowercase().contains(&field.to_lowercase()),
		"Expected validation error for field '{}', but got: {}",
		field,
		message
	);
}

/// Assert multiple validation errors occurred for the specified fields.
pub fn assert_validation_errors<T, E>(result: &Result<T, E>, fields: &[&str])
where
	T: Debug,
	E: Debug + std::fmt::Display,
{
	let error = result.should_be_err();
	let message = error.to_string().to_lowercase();

	for field in fields {
		assert!(
			message.contains(&field.to_lowercase()),
			"Expected validation error for field '{}', but it was not found in: {}",
			field,
			error
		);
	}
}

/// Response assertion builder for server functions that return HTTP-like responses.
#[derive(Debug)]
pub struct ResponseAssertion<T> {
	value: T,
	status: Option<StatusCode>,
}

impl<T> ResponseAssertion<T> {
	/// Create a new response assertion.
	pub fn new(value: T) -> Self {
		Self {
			value,
			status: None,
		}
	}

	/// Set the expected status code.
	pub fn with_status(mut self, status: StatusCode) -> Self {
		self.status = Some(status);
		self
	}

	/// Get the underlying value.
	pub fn value(&self) -> &T {
		&self.value
	}

	/// Consume and return the value.
	pub fn into_value(self) -> T {
		self.value
	}
}

/// Trait for types that can provide a status code.
pub trait HasStatusCode {
	/// Get the status code.
	fn status_code(&self) -> StatusCode;
}

impl<T: HasStatusCode> ResponseAssertion<T> {
	/// Assert that the status code matches.
	pub fn should_have_status(&self, expected: StatusCode) {
		let actual = self.value.status_code();
		assert_eq!(
			actual, expected,
			"Expected status code {:?}, but got {:?}",
			expected, actual
		);
	}

	/// Assert that the response is successful (2xx).
	pub fn should_be_success(&self) {
		let status = self.value.status_code();
		assert!(
			status.is_success(),
			"Expected successful response, but got {:?}",
			status
		);
	}

	/// Assert that the response is a client error (4xx).
	pub fn should_be_client_error(&self) {
		let status = self.value.status_code();
		assert!(
			status.is_client_error(),
			"Expected client error response, but got {:?}",
			status
		);
	}

	/// Assert that the response is a server error (5xx).
	pub fn should_be_server_error(&self) {
		let status = self.value.status_code();
		assert!(
			status.is_server_error(),
			"Expected server error response, but got {:?}",
			status
		);
	}
}

/// Macro for creating server function test cases.
///
/// This macro provides a convenient way to define multiple test cases
/// for a server function with different inputs and expected outputs.
///
/// # Example
///
/// ```rust,ignore
/// server_fn_test_cases! {
///     my_server_fn,
///     // Test case name, input, expected pattern
///     test_success: (Input { value: 1 }, Ok(_)),
///     test_error: (Input { value: -1 }, Err(_)),
/// }
/// ```
#[macro_export]
macro_rules! server_fn_test_cases {
	($fn_name:path, $($test_name:ident: ($input:expr, $expected:pat)),* $(,)?) => {
		$(
			#[rstest::rstest]
			#[tokio::test]
			async fn $test_name(
				// Fixture injected by rstest; may not be directly used in test body
				#[allow(unused_variables)]
				server_fn_context: $crate::server_fn::ServerFnTestEnv,
			) {
				let result = $fn_name($input, &server_fn_context).await;
				assert!(
					matches!(result, $expected),
					"Expected {:?} to match pattern {}",
					result,
					stringify!($expected)
				);
			}
		)*
	};
}

/// Assertion module for common HTTP status code checks.
pub mod assert_status {
	use super::*;

	/// Assert OK (200) status.
	pub fn ok<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::OK,
			"Expected 200 OK, but got {:?}",
			status
		);
	}

	/// Assert Created (201) status.
	pub fn created<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::CREATED,
			"Expected 201 Created, but got {:?}",
			status
		);
	}

	/// Assert No Content (204) status.
	pub fn no_content<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::NO_CONTENT,
			"Expected 204 No Content, but got {:?}",
			status
		);
	}

	/// Assert Bad Request (400) status.
	pub fn bad_request<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::BAD_REQUEST,
			"Expected 400 Bad Request, but got {:?}",
			status
		);
	}

	/// Assert Unauthorized (401) status.
	pub fn unauthorized<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::UNAUTHORIZED,
			"Expected 401 Unauthorized, but got {:?}",
			status
		);
	}

	/// Assert Forbidden (403) status.
	pub fn forbidden<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::FORBIDDEN,
			"Expected 403 Forbidden, but got {:?}",
			status
		);
	}

	/// Assert Not Found (404) status.
	pub fn not_found<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::NOT_FOUND,
			"Expected 404 Not Found, but got {:?}",
			status
		);
	}

	/// Assert Conflict (409) status.
	pub fn conflict<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::CONFLICT,
			"Expected 409 Conflict, but got {:?}",
			status
		);
	}

	/// Assert Unprocessable Entity (422) status.
	pub fn unprocessable_entity<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::UNPROCESSABLE_ENTITY,
			"Expected 422 Unprocessable Entity, but got {:?}",
			status
		);
	}

	/// Assert Internal Server Error (500) status.
	pub fn internal_error<T: HasStatusCode>(response: &T) {
		let status = response.status_code();
		assert_eq!(
			status,
			StatusCode::INTERNAL_SERVER_ERROR,
			"Expected 500 Internal Server Error, but got {:?}",
			status
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[derive(Debug)]
	struct StatusResponse(StatusCode);

	impl HasStatusCode for StatusResponse {
		fn status_code(&self) -> StatusCode {
			self.0
		}
	}

	#[test]
	fn test_should_be_ok() {
		let result: Result<i32, &str> = Ok(42);
		assert_eq!(*result.should_be_ok(), 42);
	}

	#[test]
	#[should_panic(expected = "Expected Ok result")]
	fn test_should_be_ok_panics_on_err() {
		let result: Result<i32, &str> = Err("error");
		result.should_be_ok();
	}

	#[test]
	fn test_should_be_err() {
		let result: Result<i32, &str> = Err("error");
		assert_eq!(*result.should_be_err(), "error");
	}

	#[test]
	#[should_panic(expected = "Expected Err result")]
	fn test_should_be_err_panics_on_ok() {
		let result: Result<i32, &str> = Ok(42);
		result.should_be_err();
	}

	#[test]
	fn test_should_have_value() {
		let result: Result<i32, &str> = Ok(42);
		result.should_have_value(&42);
	}

	#[test]
	fn test_should_satisfy() {
		let result: Result<i32, &str> = Ok(42);
		result.should_satisfy(|v| *v > 0);
	}

	#[test]
	fn test_assert_server_fn_returns() {
		let result: Result<i32, &str> = Ok(42);
		assert_server_fn_returns(&result, &42);
	}

	#[test]
	fn test_assert_server_fn_error_contains() {
		let result: Result<i32, String> = Err("Validation failed: email is required".to_string());
		assert_server_fn_error_contains(&result, "email");
	}

	#[test]
	fn test_assert_validation_error() {
		let result: Result<i32, String> = Err("Field 'username' is required".to_string());
		assert_validation_error(&result, "username");
	}

	#[test]
	fn test_assert_validation_errors() {
		let result: Result<i32, String> =
			Err("Validation errors: username is required, email is invalid".to_string());
		assert_validation_errors(&result, &["username", "email"]);
	}

	#[rstest]
	fn server_fn_result_and_response_assertions_cover_public_success_paths() {
		// Arrange
		let successful: Result<&str, String> = Ok("created");
		let failing: Result<&str, String> = Err("email and username are invalid".to_string());
		let ok = StatusResponse(StatusCode::OK);
		let created = StatusResponse(StatusCode::CREATED);
		let empty = StatusResponse(StatusCode::NO_CONTENT);
		let bad_request = StatusResponse(StatusCode::BAD_REQUEST);
		let unauthorized = StatusResponse(StatusCode::UNAUTHORIZED);
		let forbidden = StatusResponse(StatusCode::FORBIDDEN);
		let missing = StatusResponse(StatusCode::NOT_FOUND);
		let conflict = StatusResponse(StatusCode::CONFLICT);
		let invalid = StatusResponse(StatusCode::UNPROCESSABLE_ENTITY);
		let internal = StatusResponse(StatusCode::INTERNAL_SERVER_ERROR);

		// Act
		assert_server_fn_returns(&successful, &"created");
		let return_value_mismatch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			assert_server_fn_returns(&successful, &"updated");
		}));
		assert_server_fn_error(&failing);
		let successful_error_assertion =
			std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				assert_server_fn_error(&successful);
			}));
		assert_server_fn_error_contains(&failing, "username");
		assert_validation_error(&failing, "EMAIL");
		assert_validation_errors(&failing, &["email", "username"]);
		let missing_validation_field =
			std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				assert_validation_error(&failing, "password");
			}));
		let missing_validation_member =
			std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				assert_validation_errors(&failing, &["email", "password"]);
			}));
		failing
			.should_be_err()
			.should_have_message("email and username are invalid");
		let message_mismatch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			failing
				.should_be_err()
				.should_have_message("email and password are invalid");
		}));
		let assertion = ResponseAssertion::new(created).with_status(StatusCode::CREATED);
		assert_eq!(assertion.status, Some(StatusCode::CREATED));
		assertion.should_have_status(StatusCode::CREATED);
		assertion.should_be_success();
		let extracted = assertion.into_value();

		// Assert
		assert_eq!(extracted.status_code(), StatusCode::CREATED);
		assert_eq!(
			ResponseAssertion::new(ok).value().status_code(),
			StatusCode::OK
		);
		assert_status::ok(&StatusResponse(StatusCode::OK));
		assert_status::created(&StatusResponse(StatusCode::CREATED));
		assert_status::no_content(&empty);
		assert_status::bad_request(&bad_request);
		assert_status::unauthorized(&unauthorized);
		assert_status::forbidden(&forbidden);
		assert_status::not_found(&missing);
		assert_status::conflict(&conflict);
		assert_status::unprocessable_entity(&invalid);
		assert_status::internal_error(&internal);
		ResponseAssertion::new(bad_request).should_be_client_error();
		ResponseAssertion::new(internal).should_be_server_error();
		assert!(successful_error_assertion.is_err());
		assert!(return_value_mismatch.is_err());
		assert!(message_mismatch.is_err());
		assert!(missing_validation_field.is_err());
		assert!(missing_validation_member.is_err());
	}

	fn assert_status_panics(name: &str, assertion: impl FnOnce()) {
		assert!(
			std::panic::catch_unwind(std::panic::AssertUnwindSafe(assertion)).is_err(),
			"{name} accepted a mismatched status"
		);
	}

	#[rstest]
	fn status_assertions_reject_mismatched_statuses() {
		// Arrange
		let ok = StatusResponse(StatusCode::OK);
		let created = StatusResponse(StatusCode::CREATED);
		let bad_request = StatusResponse(StatusCode::BAD_REQUEST);
		let internal = StatusResponse(StatusCode::INTERNAL_SERVER_ERROR);

		// Act and assert
		assert_status_panics("ok", || assert_status::ok(&created));
		assert_status_panics("created", || assert_status::created(&ok));
		assert_status_panics("no_content", || assert_status::no_content(&ok));
		assert_status_panics("bad_request", || assert_status::bad_request(&ok));
		assert_status_panics("unauthorized", || assert_status::unauthorized(&ok));
		assert_status_panics("forbidden", || assert_status::forbidden(&ok));
		assert_status_panics("not_found", || assert_status::not_found(&ok));
		assert_status_panics("conflict", || assert_status::conflict(&ok));
		assert_status_panics("unprocessable_entity", || {
			assert_status::unprocessable_entity(&ok)
		});
		assert_status_panics("internal_error", || assert_status::internal_error(&ok));
		assert_status_panics("client_error_category", || {
			ResponseAssertion::new(internal).should_be_client_error()
		});
		assert_status_panics("server_error_category", || {
			ResponseAssertion::new(bad_request).should_be_server_error()
		});
	}
}
