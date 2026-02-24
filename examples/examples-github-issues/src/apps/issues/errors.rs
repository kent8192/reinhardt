//! API error types for GraphQL resolvers
//!
//! This module provides structured error types with error codes for GraphQL responses.

use async_graphql::{ErrorExtensions, FieldError};

/// API error types with error codes for GraphQL responses
#[derive(Debug, Clone)]
pub enum ApiError {
	/// Resource not found
	NotFound(String),
	/// Invalid input provided
	InvalidInput(String),
	/// Authentication or authorization failure
	Unauthorized(String),
	/// Internal server error
	InternalError(String),
}

impl ErrorExtensions for ApiError {
	fn extend(&self) -> FieldError {
		let (message, code) = match self {
			ApiError::NotFound(msg) => (msg.clone(), "NOT_FOUND"),
			ApiError::InvalidInput(msg) => (msg.clone(), "INVALID_INPUT"),
			ApiError::Unauthorized(msg) => (msg.clone(), "UNAUTHORIZED"),
			ApiError::InternalError(msg) => (msg.clone(), "INTERNAL_ERROR"),
		};
		FieldError::new(message).extend_with(|_, e| e.set("code", code))
	}
}

impl std::fmt::Display for ApiError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ApiError::NotFound(msg) => write!(f, "Not Found: {}", msg),
			ApiError::InvalidInput(msg) => write!(f, "Invalid Input: {}", msg),
			ApiError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
			ApiError::InternalError(msg) => write!(f, "Internal Error: {}", msg),
		}
	}
}

impl std::error::Error for ApiError {}
