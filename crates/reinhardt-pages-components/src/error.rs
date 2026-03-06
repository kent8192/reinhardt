//! Error types for reinhardt-pages-components

use thiserror::Error;

/// Error type for component operations
#[derive(Debug, Error)]
pub enum ComponentError {
	/// Invalid component property
	#[error("Invalid component property: {0}")]
	InvalidProperty(String),

	/// Missing required property
	#[error("Missing required property: {0}")]
	MissingProperty(String),

	/// Invalid HTML attribute
	#[error("Invalid HTML attribute: {0}")]
	InvalidAttribute(String),

	/// Rendering error
	#[error("Rendering error: {0}")]
	RenderingError(String),
}

/// Result type for component operations
pub type Result<T> = std::result::Result<T, ComponentError>;
