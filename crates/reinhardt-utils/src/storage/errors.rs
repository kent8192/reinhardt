//! Storage error types

use thiserror::Error;

/// Errors that can occur during storage operations.
#[derive(Error, Debug)]
pub enum StorageError {
	/// The requested file was not found in storage.
	#[error("File not found: {0}")]
	NotFound(String),

	/// An underlying I/O error occurred.
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	/// The provided path is invalid or unsafe.
	#[error("Invalid path: {0}")]
	InvalidPath(String),

	/// The storage backend has no remaining capacity.
	#[error("Storage full")]
	StorageFull,

	/// The operation was denied due to insufficient permissions.
	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	/// A file with the given name already exists.
	#[error("File already exists: {0}")]
	AlreadyExists(String),
}

/// A convenience type alias for `Result<T, StorageError>`.
pub type StorageResult<T> = Result<T, StorageError>;
