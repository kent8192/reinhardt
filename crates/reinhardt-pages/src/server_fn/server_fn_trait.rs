//! Server Function Trait and Error Types
//!
//! This module defines the core trait and error types for server functions.

use reinhardt_core::validators::{ValidationError, ValidationErrors};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Common trait for all server functions
///
/// This trait is implemented automatically by the `#[server_fn]` macro.
/// Users typically don't need to implement this manually.
pub trait ServerFn {
	/// The input type (function arguments)
	type Input: Serialize + for<'de> Deserialize<'de>;

	/// The output type (function return value)
	type Output: Serialize + for<'de> Deserialize<'de>;

	/// The endpoint path for this server function
	fn endpoint() -> &'static str;

	/// The codec name ("json", "url", "msgpack")
	fn codec() -> &'static str {
		"json"
	}
}

/// Unified error type for server functions
///
/// This error type covers all possible error conditions when calling
/// a server function from the client side.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServerFnError {
	payload: ServerFnErrorPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The opaque structured payload carried by a [`ServerFnError`].
///
/// Its fields remain private so callers use the error constructors and
/// accessors rather than depending on payload layout.
#[non_exhaustive]
pub struct ServerFnErrorPayload {
	kind: ServerFnErrorKind,
	status: Option<u16>,
	message: String,
	field_errors: Vec<ServerFnFieldError>,
}

/// A validation error associated with one input field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServerFnFieldError {
	field: String,
	message: String,
}

impl ServerFnFieldError {
	/// Returns the input field name.
	pub fn field(&self) -> &str {
		&self.field
	}

	/// Returns the safe message for the field.
	pub fn message(&self) -> &str {
		&self.message
	}
}

/// The category represented by a [`ServerFnError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServerFnErrorKind {
	/// Input validation failed.
	Validation,
	/// Authentication or authorization failed.
	Auth,
	/// An application-level error was returned.
	Application,
	/// A server-side HTTP error was returned.
	Server,
	/// A transport-level error occurred.
	Transport,
	/// Deserialization of a response failed.
	Deserialization,
}

impl ServerFnError {
	fn new(kind: ServerFnErrorKind, status: Option<u16>, message: impl Into<String>) -> Self {
		Self {
			payload: ServerFnErrorPayload {
				kind,
				status,
				message: message.into(),
				field_errors: Vec::new(),
			},
		}
	}

	/// Create a validation error with the default message and field-specific messages.
	pub fn validation<I, F, M>(field_errors: I) -> Self
	where
		I: IntoIterator<Item = (F, M)>,
		F: Into<String>,
		M: Into<String>,
	{
		Self::validation_with_message("Validation failed", field_errors)
	}

	/// Create a validation error with a custom message and field-specific messages.
	pub fn validation_with_message<I, F, M>(message: impl Into<String>, field_errors: I) -> Self
	where
		I: IntoIterator<Item = (F, M)>,
		F: Into<String>,
		M: Into<String>,
	{
		let mut error = Self::new(ServerFnErrorKind::Validation, Some(422), message);
		error.payload.field_errors = field_errors
			.into_iter()
			.map(|(field, message)| ServerFnFieldError {
				field: field.into(),
				message: message.into(),
			})
			.collect();
		error
	}

	/// Create an authentication error.
	pub fn auth(status: u16, message: impl Into<String>) -> Self {
		Self::new(ServerFnErrorKind::Auth, Some(status), message)
	}

	/// Create an application error.
	pub fn application(message: impl Into<String>) -> Self {
		Self::new(ServerFnErrorKind::Application, None, message)
	}

	/// Create an application error with an HTTP status.
	pub fn application_with_status(status: u16, message: impl Into<String>) -> Self {
		Self::new(ServerFnErrorKind::Application, Some(status), message)
	}

	/// Create a server-side error.
	pub fn server(status: u16, message: impl Into<String>) -> Self {
		Self::new(ServerFnErrorKind::Server, Some(status), message)
	}

	/// Create a transport error.
	pub fn transport(message: impl Into<String>) -> Self {
		Self::new(ServerFnErrorKind::Transport, None, message)
	}

	/// Create a network error.
	pub fn network(msg: impl Into<String>) -> Self {
		Self::transport(msg)
	}

	/// Create a serialization transport error.
	pub fn serialization(msg: impl Into<String>) -> Self {
		Self::transport(msg)
	}

	/// Create a deserialization error.
	pub fn deserialization(msg: impl Into<String>) -> Self {
		Self::new(ServerFnErrorKind::Deserialization, None, msg)
	}

	fn deserialization_with_status(status: u16, message: impl Into<String>) -> Self {
		Self::new(ServerFnErrorKind::Deserialization, Some(status), message)
	}

	/// Decode a server function error response without exposing raw response bodies.
	pub fn from_http_response(status: u16, body: &str) -> Self {
		let status = if (100..=599).contains(&status) {
			status
		} else {
			500
		};
		match serde_json::from_str::<Self>(body) {
			Ok(mut error) => {
				error.payload.status = error
					.payload
					.status
					.filter(|value| (100..=599).contains(value));
				if error.payload.status.is_none() {
					error.payload.status = Some(status);
				}
				error
			}
			Err(_) => {
				Self::deserialization_with_status(status, "Invalid server function error response")
			}
		}
	}

	/// Returns the error category.
	pub fn kind(&self) -> ServerFnErrorKind {
		self.payload.kind
	}

	/// Returns the optional HTTP status code.
	pub fn status(&self) -> Option<u16> {
		self.payload.status
	}

	/// Returns the safe message intended for users.
	pub fn user_message(&self) -> &str {
		&self.payload.message
	}

	/// Returns the safe message intended for users.
	pub fn message(&self) -> &str {
		self.user_message()
	}

	/// Returns field-specific validation errors.
	pub fn field_errors(&self) -> &[ServerFnFieldError] {
		&self.payload.field_errors
	}
}

impl std::fmt::Display for ServerFnError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.user_message())
	}
}

#[derive(Debug, Serialize, Deserialize)]
struct WireServerFnError {
	version: u8,
	kind: WireServerFnErrorKind,
	status: Option<u16>,
	message: String,
	field_errors: Vec<WireServerFnFieldError>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireServerFnErrorKind {
	Validation,
	Auth,
	Application,
	Server,
	Transport,
	Deserialization,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireServerFnFieldError {
	field: String,
	message: String,
}

impl Serialize for ServerFnError {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		WireServerFnError {
			version: 1,
			kind: self.payload.kind.into(),
			status: self.payload.status,
			message: self.payload.message.clone(),
			field_errors: self
				.payload
				.field_errors
				.iter()
				.map(|error| WireServerFnFieldError {
					field: error.field.clone(),
					message: error.message.clone(),
				})
				.collect(),
		}
		.serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for ServerFnError {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let wire = WireServerFnError::deserialize(deserializer)?;
		if wire.version != 1 {
			return Err(serde::de::Error::custom(format!(
				"unsupported ServerFnError version: {}",
				wire.version
			)));
		}
		Ok(Self {
			payload: ServerFnErrorPayload {
				kind: wire.kind.into(),
				status: wire.status,
				message: wire.message,
				field_errors: wire
					.field_errors
					.into_iter()
					.map(|error| ServerFnFieldError {
						field: error.field,
						message: error.message,
					})
					.collect(),
			},
		})
	}
}

impl From<ServerFnErrorKind> for WireServerFnErrorKind {
	fn from(kind: ServerFnErrorKind) -> Self {
		match kind {
			ServerFnErrorKind::Validation => Self::Validation,
			ServerFnErrorKind::Auth => Self::Auth,
			ServerFnErrorKind::Application => Self::Application,
			ServerFnErrorKind::Server => Self::Server,
			ServerFnErrorKind::Transport => Self::Transport,
			ServerFnErrorKind::Deserialization => Self::Deserialization,
		}
	}
}

impl From<WireServerFnErrorKind> for ServerFnErrorKind {
	fn from(kind: WireServerFnErrorKind) -> Self {
		match kind {
			WireServerFnErrorKind::Validation => Self::Validation,
			WireServerFnErrorKind::Auth => Self::Auth,
			WireServerFnErrorKind::Application => Self::Application,
			WireServerFnErrorKind::Server => Self::Server,
			WireServerFnErrorKind::Transport => Self::Transport,
			WireServerFnErrorKind::Deserialization => Self::Deserialization,
		}
	}
}

impl std::error::Error for ServerFnError {}

impl From<ValidationErrors> for ServerFnError {
	fn from(errors: ValidationErrors) -> Self {
		let field_errors = errors
			.ordered_field_errors()
			.flat_map(|(field, errors)| {
				errors.iter().map(move |error| match error {
					ValidationError::Custom(message) => (field, message.clone()),
					_ => (field, error.to_string()),
				})
			})
			.collect::<Vec<_>>();

		Self::validation(field_errors)
	}
}

#[cfg(test)]
mod tests {
	use reinhardt_core::validators::{ValidationError, ValidationErrors};
	use rstest::rstest;

	use super::*;

	#[rstest]
	fn test_server_fn_error_creation() {
		// Arrange & Act
		let network_err = ServerFnError::network("Connection timeout");
		let server_err = ServerFnError::server(404, "Not found");

		// Assert
		assert_eq!(network_err.kind(), ServerFnErrorKind::Transport);
		assert_eq!(server_err.status(), Some(404));
	}

	#[rstest]
	fn test_server_fn_error_display() {
		// Arrange
		let network_err = ServerFnError::network("Connection timeout");
		let server_err = ServerFnError::server(500, "Internal error");

		// Act & Assert
		assert_eq!(network_err.to_string(), "Connection timeout");
		assert_eq!(server_err.to_string(), "Internal error");
	}

	#[rstest]
	#[case::network(ServerFnError::network("timeout"), "timeout")]
	#[case::serialization(ServerFnError::serialization("bad input"), "bad input")]
	#[case::deserialization(ServerFnError::deserialization("bad json"), "bad json")]
	#[case::server(ServerFnError::server(403, "Forbidden"), "Forbidden")]
	#[case::application(ServerFnError::application("Invalid choice_id"), "Invalid choice_id")]
	fn test_message_returns_inner_text(#[case] err: ServerFnError, #[case] expected: &str) {
		// Act
		let msg = err.message();

		// Assert
		assert_eq!(msg, expected);
	}

	#[rstest]
	fn test_message_returns_empty_string_when_inner_is_empty() {
		// Arrange
		let err = ServerFnError::application("");

		// Act & Assert
		assert_eq!(err.message(), "");
	}

	#[rstest]
	fn test_message_matches_display() {
		// Arrange
		let err = ServerFnError::application("Invalid choice_id");

		// Act
		let message = err.message();
		let display = err.to_string();

		// Assert
		assert_eq!(message, display);
		assert_eq!(message, "Invalid choice_id");
		assert_eq!(display, "Invalid choice_id");
	}

	#[test]
	fn validation_error_serializes_to_version_one_envelope() {
		let error = ServerFnError::validation_with_message(
			"Please correct the submitted values",
			[("choice_id", "Select a choice")],
		);

		let value: serde_json::Value = serde_json::to_value(&error).unwrap();

		assert_eq!(value["version"], 1);
		assert_eq!(value["kind"], "validation");
		assert_eq!(value["status"], 422);
		assert_eq!(value["message"], "Please correct the submitted values");
		assert_eq!(value["field_errors"][0]["field"], "choice_id");
		assert_eq!(value["field_errors"][0]["message"], "Select a choice");
	}

	#[test]
	fn structured_error_round_trips_without_enum_tags() {
		let original = ServerFnError::auth(403, "Permission denied");
		let bytes = serde_json::to_vec(&original).unwrap();
		let decoded: ServerFnError = serde_json::from_slice(&bytes).unwrap();

		assert_eq!(decoded, original);
		assert_eq!(decoded.kind(), ServerFnErrorKind::Auth);
		assert_eq!(decoded.status(), Some(403));
		assert_eq!(decoded.user_message(), "Permission denied");
		assert!(!String::from_utf8(bytes).unwrap().contains("Auth"));
	}

	#[test]
	fn transport_aliases_use_the_transport_kind() {
		assert_eq!(
			ServerFnError::network("offline").kind(),
			ServerFnErrorKind::Transport
		);
		assert_eq!(
			ServerFnError::serialization("bad input").kind(),
			ServerFnErrorKind::Transport
		);
	}

	#[test]
	fn unknown_version_is_rejected() {
		let value = serde_json::json!({
			"version": 2,
			"kind": "application",
			"status": null,
			"message": "unsupported",
			"field_errors": [],
		});

		assert!(serde_json::from_value::<ServerFnError>(value).is_err());
	}

	#[test]
	fn validation_supports_empty_and_multiple_field_errors() {
		// Arrange
		let empty = ServerFnError::validation(std::iter::empty::<(&str, &str)>());

		// Act
		let empty_field_errors = empty.field_errors();

		// Assert
		assert_eq!(empty.status(), Some(422));
		let expected_empty_field_errors: &[ServerFnFieldError] = &[];
		assert_eq!(empty_field_errors, expected_empty_field_errors);

		// Arrange
		let multiple = ServerFnError::validation_with_message(
			"Invalid form",
			[("name", "Required"), ("email", "Invalid address")],
		);

		// Act
		let multiple_field_errors = multiple.field_errors();

		// Assert
		assert_eq!(multiple_field_errors.len(), 2);
		assert_eq!(multiple_field_errors[0].field(), "name");
		assert_eq!(multiple_field_errors[0].message(), "Required");
		assert_eq!(multiple_field_errors[1].field(), "email");
		assert_eq!(multiple_field_errors[1].message(), "Invalid address");
	}

	#[test]
	fn validation_errors_convert_to_server_fn_field_errors() {
		// Arrange
		let mut validation = ValidationErrors::new();
		validation.add("name", ValidationError::TooShort { length: 0, min: 1 });
		validation.add(
			"name",
			ValidationError::Custom("Name is required".to_string()),
		);
		validation.add(
			"email",
			ValidationError::Custom("Email is invalid".to_string()),
		);

		// Act
		let error: ServerFnError = validation.into();

		// Assert
		assert_eq!(error.kind(), ServerFnErrorKind::Validation);
		assert_eq!(error.status(), Some(422));
		assert_eq!(error.field_errors().len(), 3);
		assert_eq!(error.field_errors()[0].field(), "name");
		assert_eq!(error.field_errors()[1].field(), "name");
		assert_eq!(error.field_errors()[2].field(), "email");
		assert_eq!(
			error.field_errors()[0].message(),
			"Length too short: 0 (minimum: 1)"
		);
		assert_eq!(error.field_errors()[1].message(), "Name is required");
		assert_eq!(error.field_errors()[2].message(), "Email is invalid");
	}

	#[rstest]
	fn validation_errors_preserve_insertion_order() {
		let mut validation = ValidationErrors::new();
		validation.add("name", ValidationError::TooShort { length: 0, min: 1 });
		validation.add(
			"api_url",
			ValidationError::InvalidUrl("invalid".to_string()),
		);
		validation.add(
			"_all",
			ValidationError::Custom("Invalid combination".to_string()),
		);

		let error: ServerFnError = validation.into();
		let fields = error
			.field_errors()
			.iter()
			.map(ServerFnFieldError::field)
			.collect::<Vec<_>>();

		assert_eq!(fields, ["name", "api_url", "_all"]);
	}
}
