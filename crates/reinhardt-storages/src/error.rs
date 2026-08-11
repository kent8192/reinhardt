//! Error types for storage operations.

use thiserror::Error;

/// Result type alias for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Error types that can occur during storage operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
	/// The requested file or resource was not found.
	#[error("File not found: {0}")]
	NotFound(String),

	/// A file with the given name already exists.
	#[error("File already exists: {0}")]
	AlreadyExists(String),

	/// The backend does not support the requested operation.
	#[error("Unsupported operation: {0}")]
	UnsupportedOperation(String),

	/// Permission was denied for the operation.
	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	/// A network error occurred during communication with the storage backend.
	#[error("Network error: {0}")]
	NetworkError(String),

	/// Configuration error (invalid or missing configuration).
	#[error("Configuration error: {0}")]
	ConfigError(String),

	/// The provided path is invalid or attempts to escape the storage root.
	#[error("Invalid path: {0}")]
	InvalidPath(String),

	/// I/O error occurred during file operations.
	#[error("I/O error: {0}")]
	IoError(#[from] std::io::Error),

	/// Other errors not covered by specific variants.
	#[error("Storage error: {0}")]
	Other(String),
}

/// Errors raised while resolving file-field storage backends.
#[derive(Debug, Error)]
pub enum FileStorageError {
	/// No storage registry is currently active in this process.
	#[error("the active storage registry is unavailable")]
	RegistryUnavailable,
	/// A requested or configured storage alias does not exist or is invalid.
	#[error("unknown storage alias `{0}`")]
	UnknownStorageAlias(String),
	/// The selected backend cannot atomically create new files.
	#[error("storage alias `{0}` does not support atomic exclusive creation")]
	UnsupportedExclusiveSave(String),
	/// The upload payload omitted its client filename.
	#[error("the upload does not include a filename")]
	MissingFilename,
	/// The client filename has an unsafe structural form.
	#[error("unsafe upload filename: {0}")]
	UnsafeFilename(String),
	/// The configured upload directory template is invalid.
	#[error("invalid upload template: {0}")]
	InvalidUploadTemplate(String),
	/// The generated key cannot fit within the field's character limit.
	#[error("the generated logical path exceeds max_length {max_length}")]
	PathTooLong {
		/// Maximum number of Unicode scalar values allowed by the field.
		max_length: usize,
	},
	/// Every bounded collision candidate already existed.
	#[error("all collision-safe upload names were exhausted")]
	CollisionExhausted,
	/// An underlying storage operation failed.
	#[error(transparent)]
	Storage(#[from] StorageError),
}

#[cfg(feature = "s3")]
impl From<reinhardt_providers::ProviderError> for StorageError {
	fn from(err: reinhardt_providers::ProviderError) -> Self {
		match err {
			reinhardt_providers::ProviderError::Config(message) => {
				StorageError::ConfigError(message)
			}
			reinhardt_providers::ProviderError::NotFound(_) => {
				StorageError::Other("provider resource not found".to_string())
			}
			reinhardt_providers::ProviderError::PermissionDenied(message) => {
				StorageError::PermissionDenied(message)
			}
			reinhardt_providers::ProviderError::Service {
				status: 404,
				message,
			} => StorageError::NotFound(message),
			reinhardt_providers::ProviderError::Service { message, .. }
			| reinhardt_providers::ProviderError::Header(message) => StorageError::NetworkError(message),
			reinhardt_providers::ProviderError::Http(err) => {
				StorageError::NetworkError(err.to_string())
			}
			reinhardt_providers::ProviderError::Url(err) => {
				StorageError::ConfigError(err.to_string())
			}
			_ => StorageError::Other(err.to_string()),
		}
	}
}
