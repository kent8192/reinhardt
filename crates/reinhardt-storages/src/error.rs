//! Error types for storage operations.

use thiserror::Error;

/// Result type alias for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Error types that can occur during storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
	/// The requested file or resource was not found.
	#[error("File not found: {0}")]
	NotFound(String),

	/// Permission was denied for the operation.
	#[error("Permission denied: {0}")]
	PermissionDenied(String),

	/// A network error occurred during communication with the storage backend.
	#[error("Network error: {0}")]
	NetworkError(String),

	/// Configuration error (invalid or missing configuration).
	#[error("Configuration error: {0}")]
	ConfigError(String),

	/// I/O error occurred during file operations.
	#[error("I/O error: {0}")]
	IoError(#[from] std::io::Error),

	/// Other errors not covered by specific variants.
	#[error("Storage error: {0}")]
	Other(String),
}

#[cfg(feature = "s3")]
impl From<aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::put_object::PutObjectError>>
	for StorageError
{
	fn from(
		err: aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::put_object::PutObjectError>,
	) -> Self {
		StorageError::NetworkError(err.to_string())
	}
}

#[cfg(feature = "s3")]
impl From<aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::get_object::GetObjectError>>
	for StorageError
{
	fn from(
		err: aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::get_object::GetObjectError>,
	) -> Self {
		match err {
			aws_sdk_s3::error::SdkError::ServiceError(ref service_err) => {
				if service_err.err().is_no_such_key() {
					StorageError::NotFound("S3 object not found".to_string())
				} else {
					StorageError::NetworkError(err.to_string())
				}
			}
			_ => StorageError::NetworkError(err.to_string()),
		}
	}
}

#[cfg(feature = "s3")]
impl From<aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::delete_object::DeleteObjectError>>
	for StorageError
{
	fn from(
		err: aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::delete_object::DeleteObjectError>,
	) -> Self {
		StorageError::NetworkError(err.to_string())
	}
}

#[cfg(feature = "s3")]
impl From<aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::head_object::HeadObjectError>>
	for StorageError
{
	fn from(
		err: aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::head_object::HeadObjectError>,
	) -> Self {
		match err {
			aws_sdk_s3::error::SdkError::ServiceError(ref service_err) => {
				if service_err.err().is_not_found() {
					StorageError::NotFound("S3 object not found".to_string())
				} else {
					StorageError::NetworkError(err.to_string())
				}
			}
			_ => StorageError::NetworkError(err.to_string()),
		}
	}
}
