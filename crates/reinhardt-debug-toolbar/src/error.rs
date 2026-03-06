//! Error types for debug toolbar

use thiserror::Error;

/// Result type for toolbar operations
pub type ToolbarResult<T> = Result<T, ToolbarError>;

/// Error types for debug toolbar operations
#[derive(Debug, Error)]
pub enum ToolbarError {
	/// Serialization error
	#[error("Serialization error: {0}")]
	SerializationError(#[from] serde_json::Error),

	/// HTML injection error
	#[error("HTML injection error: {0}")]
	InjectionError(String),

	/// Panel not found
	#[error("Panel not found: {0}")]
	PanelNotFound(String),

	/// Panel rendering error
	#[error("Panel rendering error: {0}")]
	RenderError(String),

	/// Toolbar context not available
	#[error("Toolbar context not available")]
	ContextNotAvailable,

	/// HTTP error
	#[error("HTTP error: {0}")]
	HttpError(String),

	/// IO error
	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),
}
