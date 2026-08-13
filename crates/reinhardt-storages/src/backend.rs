//! Storage backend trait definition.

use crate::{Result, StorageError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Receives ownership when an exclusive save creates its object.
///
/// Backends that can create an object before their save future returns must
/// call [`StoredObjectAdoption::adopt`] immediately after creation and before
/// any later suspension point.
#[doc(hidden)]
pub trait StoredObjectAdoption: Send + Sync {
	/// Adopt the newly created logical path.
	fn adopt(&self, path: &str);
}

/// Optional operations supported by a storage backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageCapabilities {
	/// Whether the backend can atomically create a file only when it is absent.
	pub exclusive_create: bool,
}

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

	/// Atomically save a file only if no file exists at the logical name.
	///
	/// # Errors
	///
	/// Returns `` `StorageError::AlreadyExists` `` when a file already exists.
	/// Returns `` `StorageError::UnsupportedOperation` `` when the backend does
	/// not provide atomic exclusive creation.
	async fn save_if_absent(&self, name: &str, content: &[u8]) -> Result<String> {
		let _ = (name, content);
		Err(StorageError::UnsupportedOperation(
			"atomic exclusive create is not supported".to_string(),
		))
	}

	/// Atomically save and transfer ownership as soon as creation succeeds.
	///
	/// Implementations that can suspend after creating the object must override
	/// this method and adopt the returned logical path before that suspension.
	#[doc(hidden)]
	async fn save_if_absent_with_adoption(
		&self,
		name: &str,
		content: &[u8],
		adoption: Arc<dyn StoredObjectAdoption>,
	) -> Result<String> {
		let stored = self.save_if_absent(name, content).await?;
		adoption.adopt(&stored);
		Ok(stored)
	}

	/// Return the optional operations supported by this backend.
	fn capabilities(&self) -> StorageCapabilities {
		StorageCapabilities::default()
	}

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
