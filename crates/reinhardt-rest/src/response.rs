//! REST API response types
//!
//! Re-exports pagination types from reinhardt-pagination and provides
//! REST-specific response utilities compatible with Django REST Framework.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export PaginatedResponse from reinhardt-pagination (via reinhardt-core)
pub use reinhardt_core::pagination::PaginatedResponse;

/// DRF-style API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
	/// Response data
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<T>,

	/// Error message (if any)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<String>,

	/// Error details (if any)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub errors: Option<HashMap<String, Vec<String>>>,

	/// Status code
	pub status: u16,

	/// Message (for informational responses)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub message: Option<String>,
}

impl<T> ApiResponse<T> {
	/// Create a successful response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Serialize, Deserialize)]
	/// struct User {
	///     id: i64,
	///     name: String,
	/// }
	///
	/// let user = User { id: 1, name: "Alice".to_string() };
	/// let response = ApiResponse::success(user);
	///
	/// assert_eq!(response.status, 200);
	/// assert!(response.data.is_some());
	/// assert!(response.error.is_none());
	/// ```
	pub fn success(data: T) -> Self {
		Self {
			data: Some(data),
			error: None,
			errors: None,
			status: 200,
			message: None,
		}
	}
	/// Create a success response with custom status
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Serialize, Deserialize)]
	/// struct NewUser {
	///     id: i64,
	///     name: String,
	/// }
	///
	/// let user = NewUser { id: 1, name: "Bob".to_string() };
	/// let response = ApiResponse::success_with_status(user, 201);
	///
	/// assert_eq!(response.status, 201);
	/// assert!(response.data.is_some());
	/// ```
	pub fn success_with_status(data: T, status: u16) -> Self {
		Self {
			data: Some(data),
			error: None,
			errors: None,
			status,
			message: None,
		}
	}
	/// Create an error response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	///
	/// let response: ApiResponse<String> = ApiResponse::error("Database connection failed", 500);
	///
	/// assert_eq!(response.status, 500);
	/// assert!(response.data.is_none());
	/// assert_eq!(response.error.as_ref().unwrap(), "Database connection failed");
	/// ```
	pub fn error(message: impl Into<String>, status: u16) -> Self {
		Self {
			data: None,
			error: Some(message.into()),
			errors: None,
			status,
			message: None,
		}
	}
	/// Create a validation error response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	/// use std::collections::HashMap;
	///
	/// let mut errors = HashMap::new();
	/// errors.insert("email".to_string(), vec!["Invalid email format".to_string()]);
	/// errors.insert("age".to_string(), vec!["Must be 18 or older".to_string()]);
	///
	/// let response: ApiResponse<String> = ApiResponse::validation_error(errors);
	///
	/// assert_eq!(response.status, 400);
	/// assert!(response.errors.is_some());
	/// assert_eq!(response.error.as_ref().unwrap(), "Validation failed");
	/// ```
	pub fn validation_error(errors: HashMap<String, Vec<String>>) -> Self {
		Self {
			data: None,
			error: Some("Validation failed".to_string()),
			errors: Some(errors),
			status: 400,
			message: None,
		}
	}
	/// Create a not found response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	///
	/// let response: ApiResponse<String> = ApiResponse::not_found();
	///
	/// assert_eq!(response.status, 404);
	/// assert!(response.data.is_none());
	/// assert_eq!(response.error.as_ref().unwrap(), "Not found");
	/// ```
	pub fn not_found() -> Self {
		Self {
			data: None,
			error: Some("Not found".to_string()),
			errors: None,
			status: 404,
			message: None,
		}
	}
	/// Create an unauthorized response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	///
	/// let response: ApiResponse<String> = ApiResponse::unauthorized();
	///
	/// assert_eq!(response.status, 401);
	/// assert!(response.data.is_none());
	/// assert_eq!(response.error.as_ref().unwrap(), "Unauthorized");
	/// ```
	pub fn unauthorized() -> Self {
		Self {
			data: None,
			error: Some("Unauthorized".to_string()),
			errors: None,
			status: 401,
			message: None,
		}
	}
	/// Create a forbidden response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	///
	/// let response: ApiResponse<String> = ApiResponse::forbidden();
	///
	/// assert_eq!(response.status, 403);
	/// assert!(response.data.is_none());
	/// assert_eq!(response.error.as_ref().unwrap(), "Forbidden");
	/// ```
	pub fn forbidden() -> Self {
		Self {
			data: None,
			error: Some("Forbidden".to_string()),
			errors: None,
			status: 403,
			message: None,
		}
	}
	/// Add a message to the response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Serialize, Deserialize)]
	/// struct Item {
	///     id: i64,
	/// }
	///
	/// let item = Item { id: 1 };
	/// let response = ApiResponse::success(item).with_message("Item created successfully");
	///
	/// assert_eq!(response.message.as_ref().unwrap(), "Item created successfully");
	/// ```
	pub fn with_message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
	/// Convert to JSON
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Serialize, Deserialize)]
	/// struct Data {
	///     value: i32,
	/// }
	///
	/// let response = ApiResponse::success(Data { value: 42 });
	/// let json = response.to_json().unwrap();
	///
	/// assert!(json.contains("\"value\":42"));
	/// assert!(json.contains("\"status\":200"));
	/// ```
	pub fn to_json(&self) -> Result<String, serde_json::Error>
	where
		T: Serialize,
	{
		serde_json::to_string(self)
	}
	/// Convert to pretty JSON
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ApiResponse;
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Serialize, Deserialize)]
	/// struct Data {
	///     value: i32,
	/// }
	///
	/// let response = ApiResponse::success(Data { value: 42 });
	/// let json = response.to_json_pretty().unwrap();
	///
	/// assert!(json.contains("\"value\": 42"));
	/// assert!(json.contains('\n')); // Checks for pretty formatting
	/// ```
	pub fn to_json_pretty(&self) -> Result<String, serde_json::Error>
	where
		T: Serialize,
	{
		serde_json::to_string_pretty(self)
	}
}

/// Response builder for REST API responses
pub struct ResponseBuilder<T> {
	data: Option<T>,
	error: Option<String>,
	errors: Option<HashMap<String, Vec<String>>>,
	status: u16,
	message: Option<String>,
}

impl<T> ResponseBuilder<T> {
	/// Create a new response builder
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ResponseBuilder;
	///
	/// let builder = ResponseBuilder::<String>::new();
	/// let response = builder.build();
	///
	/// assert_eq!(response.status, 200);
	/// assert!(response.data.is_none());
	/// ```
	pub fn new() -> Self {
		Self {
			data: None,
			error: None,
			errors: None,
			status: 200,
			message: None,
		}
	}
	/// Set response data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ResponseBuilder;
	///
	/// let response = ResponseBuilder::new()
	///     .data("Success data")
	///     .build();
	///
	/// assert!(response.data.is_some());
	/// assert_eq!(response.data.unwrap(), "Success data");
	/// ```
	pub fn data(mut self, data: T) -> Self {
		self.data = Some(data);
		self
	}
	/// Set error message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ResponseBuilder;
	///
	/// let response = ResponseBuilder::<String>::new()
	///     .error("Something went wrong")
	///     .status(500)
	///     .build();
	///
	/// assert_eq!(response.error.as_ref().unwrap(), "Something went wrong");
	/// assert_eq!(response.status, 500);
	/// ```
	pub fn error(mut self, error: impl Into<String>) -> Self {
		self.error = Some(error.into());
		self
	}
	/// Set validation errors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ResponseBuilder;
	/// use std::collections::HashMap;
	///
	/// let mut errors = HashMap::new();
	/// errors.insert("email".to_string(), vec!["Invalid format".to_string()]);
	///
	/// let response = ResponseBuilder::<String>::new()
	///     .errors(errors)
	///     .status(400)
	///     .build();
	///
	/// assert_eq!(response.status, 400);
	/// assert!(response.errors.is_some());
	/// ```
	pub fn errors(mut self, errors: HashMap<String, Vec<String>>) -> Self {
		self.errors = Some(errors);
		self
	}
	/// Set status code
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ResponseBuilder;
	///
	/// let response = ResponseBuilder::<String>::new()
	///     .status(201)
	///     .build();
	///
	/// assert_eq!(response.status, 201);
	/// ```
	pub fn status(mut self, status: u16) -> Self {
		self.status = status;
		self
	}
	/// Set message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ResponseBuilder;
	///
	/// let response = ResponseBuilder::<String>::new()
	///     .message("Operation completed successfully")
	///     .build();
	///
	/// assert_eq!(response.message.unwrap(), "Operation completed successfully");
	/// ```
	pub fn message(mut self, message: impl Into<String>) -> Self {
		self.message = Some(message.into());
		self
	}
	/// Build the response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::ResponseBuilder;
	///
	/// let response = ResponseBuilder::<String>::new()
	///     .data("Success".to_string())
	///     .status(200)
	///     .build();
	///
	/// assert_eq!(response.status, 200);
	/// assert!(response.data.is_some());
	/// ```
	pub fn build(self) -> ApiResponse<T> {
		ApiResponse {
			data: self.data,
			error: self.error,
			errors: self.errors,
			status: self.status,
			message: self.message,
		}
	}
}

impl<T> Default for ResponseBuilder<T> {
	fn default() -> Self {
		Self::new()
	}
}

/// Trait for converting to REST API responses
pub trait IntoApiResponse<T> {
	/// Converts this value into an `ApiResponse<T>`.
	fn into_api_response(self) -> ApiResponse<T>;
}

impl<T> IntoApiResponse<T> for Result<T, String> {
	fn into_api_response(self) -> ApiResponse<T> {
		match self {
			Ok(data) => ApiResponse::success(data),
			Err(err) => ApiResponse::error(err, 500),
		}
	}
}

impl<T> IntoApiResponse<T> for Option<T> {
	fn into_api_response(self) -> ApiResponse<T> {
		match self {
			Some(data) => ApiResponse::success(data),
			None => ApiResponse::not_found(),
		}
	}
}

// Tests are located in the underlying specialized crates:
// - Response functionality tests: `reinhardt-http/tests/`
// - Integration tests: `tests/integration/`

// All tests have been moved to specialized crates
// This module only contains re-export functionality
