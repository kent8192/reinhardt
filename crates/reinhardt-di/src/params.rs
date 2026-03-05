//! # Reinhardt Parameter Extraction
//!
//! FastAPI-inspired parameter extraction system.
//!
//! ## Features
//!
//! - **Path Parameters**: Extract from URL path
//! - **Query Parameters**: Extract from query string
//! - **Headers**: Extract from request headers
//! - **Cookies**: Extract from cookies
//! - **Body**: Extract from request body
//! - **Type-safe**: Full compile-time type checking
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_di::params::{Path, Query, Json};
//! # use serde::Deserialize;
//! # #[derive(Deserialize)]
//! # struct UserFilter { page: i32 }
//! # #[derive(Deserialize)]
//! # struct UpdateUser { name: String }
//!
//! // Extract path parameter
//! let id = Path(42_i64);
//! let user_id: i64 = id.0; // or *id
//!
//! // Extract query parameters
//! # let filter_data = UserFilter { page: 1 };
//! let filter = Query(filter_data);
//! let page = filter.0.page;
//!
//! // Extract JSON body
//! # let user_data = UpdateUser { name: "Alice".to_string() };
//! let body = Json(user_data);
//! let name = &body.0.name;
//! ```

pub mod body;
pub mod cookie;
pub mod cookie_named;
pub(crate) mod cookie_util;
pub mod extract;
pub mod form;
pub mod header;
pub mod header_named;
pub mod json;
#[cfg(feature = "multipart")]
pub mod multipart;
pub mod path;
pub mod query;
pub mod validation;

use reinhardt_http::Error as CoreError;
use std::any::TypeId;
use std::collections::HashMap;
use thiserror::Error;

// Re-export Request from reinhardt-http and parameter error types from reinhardt-exception
pub use reinhardt_core::exception::{ParamErrorContext, ParamType};
pub use reinhardt_http::Request;

// Import helper functions for parameter error extraction
use reinhardt_core::exception::param_error::{
	extract_field_from_serde_error, extract_field_from_urlencoded_error,
};

pub use body::Body;
pub use cookie::{Cookie, CookieStruct};
pub use cookie_named::{CookieName, CookieNamed, CsrfToken, SessionId};
pub use extract::FromRequest;
pub use form::Form;
pub use header::{Header, HeaderStruct};
pub use header_named::{Authorization, ContentType, HeaderName, HeaderNamed};
pub use json::Json;
#[cfg(feature = "multipart")]
pub use multipart::Multipart;
pub use path::{Path, PathStruct};
pub use query::Query;
#[cfg(feature = "validation")]
pub use validation::Validated;
pub use validation::{
	ValidatedForm, ValidatedPath, ValidatedQuery, ValidationConstraints, WithValidation,
};

// Box wrappers to reduce enum size (clippy::result_large_err mitigation)
// ParamErrorContext contains multiple String fields which make the enum large
#[derive(Debug, Error)]
pub enum ParamError {
	#[error("Missing required parameter: {0}")]
	MissingParameter(String),

	#[error("{}", .0.format_error())]
	InvalidParameter(Box<ParamErrorContext>),

	#[error("{}", .0.format_error())]
	ParseError(Box<ParamErrorContext>),

	#[error("{}", .0.format_error())]
	DeserializationError(Box<ParamErrorContext>),

	#[error("{}", .0.format_error())]
	UrlEncodingError(Box<ParamErrorContext>),

	#[error("Request body error: {0}")]
	BodyError(String),

	#[error("Payload too large: {0}")]
	PayloadTooLarge(String),

	#[cfg(feature = "validation")]
	#[error("{}", .0.format_error())]
	ValidationError(Box<ParamErrorContext>),
}

impl ParamError {
	/// Create a deserialization error from serde_json::Error
	pub fn json_deserialization<T>(err: serde_json::Error, raw_value: Option<String>) -> Self {
		let field_name = extract_field_from_serde_error(&err);
		let mut ctx = ParamErrorContext::new(ParamType::Json, err.to_string())
			.with_expected_type::<T>()
			.with_source(Box::new(err));

		if let Some(field) = field_name {
			ctx = ctx.with_field(field);
		}

		if let Some(raw) = raw_value {
			ctx = ctx.with_raw_value(raw);
		}

		ParamError::DeserializationError(Box::new(ctx))
	}

	/// Create a URL encoding error
	pub fn url_encoding<T>(
		param_type: ParamType,
		err: serde_urlencoded::de::Error,
		raw_value: Option<String>,
	) -> Self {
		let field_name = extract_field_from_urlencoded_error(&err);
		let mut ctx = ParamErrorContext::new(param_type, err.to_string())
			.with_expected_type::<T>()
			.with_source(Box::new(err));

		if let Some(field) = field_name {
			ctx = ctx.with_field(field);
		}

		if let Some(raw) = raw_value {
			ctx = ctx.with_raw_value(raw);
		}

		ParamError::UrlEncodingError(Box::new(ctx))
	}

	/// Create an invalid parameter error
	pub fn invalid<T>(param_type: ParamType, message: impl Into<String>) -> Self {
		let ctx = ParamErrorContext::new(param_type, message).with_expected_type::<T>();
		ParamError::InvalidParameter(Box::new(ctx))
	}

	/// Create a parse error
	pub fn parse<T>(
		param_type: ParamType,
		message: impl Into<String>,
		source: Box<dyn std::error::Error + Send + Sync>,
	) -> Self {
		let ctx = ParamErrorContext::new(param_type, message)
			.with_expected_type::<T>()
			.with_source(source);
		ParamError::ParseError(Box::new(ctx))
	}

	/// Get the error context if available
	pub fn context(&self) -> Option<&ParamErrorContext> {
		match self {
			ParamError::InvalidParameter(ctx) => Some(ctx),
			ParamError::ParseError(ctx) => Some(ctx),
			ParamError::DeserializationError(ctx) => Some(ctx),
			ParamError::UrlEncodingError(ctx) => Some(ctx),
			#[cfg(feature = "validation")]
			ParamError::ValidationError(ctx) => Some(ctx),
			_ => None,
		}
	}

	/// Format the error as multiple lines for detailed logging
	pub fn format_multiline(&self, include_raw_value: bool) -> String {
		match self.context() {
			Some(ctx) => ctx.format_multiline(include_raw_value),
			None => format!("  {}", self),
		}
	}
}

impl From<ParamError> for CoreError {
	fn from(err: ParamError) -> Self {
		// Use structured context if available, otherwise fall back to generic validation error
		match err.context() {
			Some(ctx) => CoreError::ParamValidation(Box::new(ctx.clone())),
			None => CoreError::Validation(err.to_string()),
		}
	}
}

pub type ParamResult<T> = std::result::Result<T, ParamError>;

/// Context for parameter extraction
pub struct ParamContext {
	/// Path parameters extracted from the URL
	pub path_params: std::collections::HashMap<String, String>,
	/// Header name registry keyed by value type
	header_names: HashMap<TypeId, &'static str>,
	/// Cookie name registry keyed by value type
	cookie_names: HashMap<TypeId, &'static str>,
}

impl ParamContext {
	/// Create a new empty ParamContext
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::ParamContext;
	///
	/// let ctx = ParamContext::new();
	/// assert_eq!(ctx.path_params.len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			path_params: std::collections::HashMap::new(),
			header_names: HashMap::new(),
			cookie_names: HashMap::new(),
		}
	}
	/// Create a ParamContext with pre-populated path parameters
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::ParamContext;
	/// use std::collections::HashMap;
	///
	/// let mut params = HashMap::new();
	/// params.insert("id".to_string(), "42".to_string());
	/// params.insert("name".to_string(), "test".to_string());
	///
	/// let ctx = ParamContext::with_path_params(params);
	/// assert_eq!(ctx.get_path_param("id"), Some("42"));
	/// assert_eq!(ctx.get_path_param("name"), Some("test"));
	/// ```
	pub fn with_path_params(path_params: std::collections::HashMap<String, String>) -> Self {
		Self {
			path_params,
			header_names: HashMap::new(),
			cookie_names: HashMap::new(),
		}
	}
	/// Get a path parameter by name
	///
	/// Returns `None` if the parameter doesn't exist.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::params::ParamContext;
	/// use std::collections::HashMap;
	///
	/// let mut params = HashMap::new();
	/// params.insert("user_id".to_string(), "123".to_string());
	///
	/// let ctx = ParamContext::with_path_params(params);
	/// assert_eq!(ctx.get_path_param("user_id"), Some("123"));
	/// assert_eq!(ctx.get_path_param("missing"), None);
	/// ```
	pub fn get_path_param(&self, name: &str) -> Option<&str> {
		self.path_params.get(name).map(|s| s.as_str())
	}

	/// Register a header name for the given value type `T`
	pub fn set_header_name<T: 'static>(&mut self, name: &'static str) {
		self.header_names.insert(TypeId::of::<T>(), name);
	}

	/// Get a registered header name for value type `T`
	pub fn get_header_name<T: 'static>(&self) -> Option<&'static str> {
		self.header_names.get(&TypeId::of::<T>()).copied()
	}

	/// Register a cookie name for the given value type `T`
	pub fn set_cookie_name<T: 'static>(&mut self, name: &'static str) {
		self.cookie_names.insert(TypeId::of::<T>(), name);
	}

	/// Get a registered cookie name for value type `T`
	pub fn get_cookie_name<T: 'static>(&self) -> Option<&'static str> {
		self.cookie_names.get(&TypeId::of::<T>()).copied()
	}
}

impl Default for ParamContext {
	fn default() -> Self {
		Self::new()
	}
}
