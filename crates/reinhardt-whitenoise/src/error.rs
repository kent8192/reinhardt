//! Error types for WhiteNoise operations

use std::io;
use thiserror::Error;

/// Result type for WhiteNoise operations
pub type Result<T> = std::result::Result<T, WhiteNoiseError>;

/// Error types for WhiteNoise operations
#[derive(Debug, Error)]
pub enum WhiteNoiseError {
	/// I/O error occurred during file operations
	#[error("I/O error: {0}")]
	Io(#[from] io::Error),

	/// Invalid configuration provided
	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),

	/// Error parsing manifest file
	#[error("Manifest parse error: {0}")]
	ManifestError(#[from] serde_json::Error),

	/// Attempted path traversal attack detected
	#[error("Path traversal attempt detected: {0}")]
	PathTraversal(String),

	/// File not found in cache
	#[error("File not found: {0}")]
	FileNotFound(String),

	/// Error during directory walking
	#[error("Directory walking error: {0}")]
	WalkDir(#[from] walkdir::Error),
}
