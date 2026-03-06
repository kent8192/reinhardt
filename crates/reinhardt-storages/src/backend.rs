//! Storage backend trait definition.

use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Storage backend trait for unified cloud storage operations.
///
/// This trait defines a common interface for all storage backends
/// (S3, Google Cloud Storage, Azure Blob Storage, Local File System).
///
/// All methods are asynchronous and return `` `Result<T, StorageError>` ``.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_storages::{StorageBackend, Result};
///
/// async fn example(storage: &dyn StorageBackend) -> Result<()> {
///     // Save a file
///     storage.save("example.txt", b"Hello, world!").await?;
///
///     // Check if file exists
///     if storage.exists("example.txt").await? {
///         // Get file size
///         let size = storage.size("example.txt").await?;
///         println!("File size: {} bytes", size);
///     }
///
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait StorageBackend: Send + Sync {
	/// Save a file to the storage backend.
	///
	/// # Arguments
	///
	/// * `name` - The file path/name
	/// * `content` - The file content as bytes
	///
	/// # Returns
	///
	/// The final file path/name after saving.
	///
	/// # Errors
	///
	/// Returns `` `StorageError::PermissionDenied` `` if write access is denied.
	/// Returns `` `StorageError::NetworkError` `` if network communication fails.
	async fn save(&self, name: &str, content: &[u8]) -> Result<String>;

	/// Open (read) a file from the storage backend.
	///
	/// # Arguments
	///
	/// * `name` - The file path/name
	///
	/// # Returns
	///
	/// The file content as bytes.
	///
	/// # Errors
	///
	/// Returns `` `StorageError::NotFound` `` if the file doesn't exist.
	/// Returns `` `StorageError::PermissionDenied` `` if read access is denied.
	async fn open(&self, name: &str) -> Result<Vec<u8>>;

	/// Delete a file from the storage backend.
	///
	/// # Arguments
	///
	/// * `name` - The file path/name
	///
	/// # Errors
	///
	/// Returns `` `StorageError::NotFound` `` if the file doesn't exist.
	/// Returns `` `StorageError::PermissionDenied` `` if delete access is denied.
	async fn delete(&self, name: &str) -> Result<()>;

	/// Check if a file exists in the storage backend.
	///
	/// # Arguments
	///
	/// * `name` - The file path/name
	///
	/// # Returns
	///
	/// `true` if the file exists, `false` otherwise.
	async fn exists(&self, name: &str) -> Result<bool>;

	/// Generate a URL for accessing the file.
	///
	/// For cloud providers (S3, GCS, Azure), this generates a presigned/signed URL
	/// with temporary access. For local storage, this returns a file:// URL.
	///
	/// # Arguments
	///
	/// * `name` - The file path/name
	/// * `expiry_secs` - URL expiration time in seconds
	///
	/// # Returns
	///
	/// A URL string for accessing the file.
	///
	/// # Errors
	///
	/// Returns `` `StorageError::NotFound` `` if the file doesn't exist.
	async fn url(&self, name: &str, expiry_secs: u64) -> Result<String>;

	/// Get the file size in bytes.
	///
	/// # Arguments
	///
	/// * `name` - The file path/name
	///
	/// # Returns
	///
	/// File size in bytes.
	///
	/// # Errors
	///
	/// Returns `` `StorageError::NotFound` `` if the file doesn't exist.
	async fn size(&self, name: &str) -> Result<u64>;

	/// Get the file's last modified timestamp.
	///
	/// # Arguments
	///
	/// * `name` - The file path/name
	///
	/// # Returns
	///
	/// Last modified timestamp as `` `DateTime<Utc>` ``.
	///
	/// # Errors
	///
	/// Returns `` `StorageError::NotFound` `` if the file doesn't exist.
	async fn get_modified_time(&self, name: &str) -> Result<DateTime<Utc>>;
}
