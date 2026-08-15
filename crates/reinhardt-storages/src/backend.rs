//! Storage backend trait definition.

use crate::{Result, StorageError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::future::Future;
use std::sync::Arc;

/// Receives ownership when an exclusive save creates its object.
///
/// Backends that can create an object before their save future returns must
/// either adopt at the creation boundary or keep the provider operation alive
/// after caller cancellation and adopt as soon as success becomes observable.
#[doc(hidden)]
pub trait StoredObjectAdoption: Send + Sync {
	/// Adopt the newly created logical path.
	fn adopt(&self, path: &str);
}

/// Run a cloud exclusive save independently of caller cancellation.
pub(crate) async fn save_if_absent_with_adoption_task<F>(
	save: F,
	adoption: Arc<dyn StoredObjectAdoption>,
) -> Result<String>
where
	F: Future<Output = Result<String>> + Send + 'static,
{
	tokio::spawn(async move {
		let stored = save.await?;
		adoption.adopt(&stored);
		Ok(stored)
	})
	.await
	.map_err(|error| StorageError::Other(format!("exclusive save task failed: {error}")))?
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
	/// Implementations whose provider can accept a create before this future
	/// returns must override this method and make adoption cancellation-safe.
	#[doc(hidden)]
	async fn save_if_absent_with_adoption(
		&self,
		name: &str,
		content: &[u8],
		adoption: Arc<dyn StoredObjectAdoption>,
	) -> Result<String> {
		let _ = (name, content, adoption);
		Err(StorageError::UnsupportedOperation(
			"cancellation-safe exclusive create is not supported".to_owned(),
		))
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

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::sync::Mutex;
	use tokio::sync::oneshot;
	use tokio::time::{Duration, timeout};

	struct ChannelAdoption(Mutex<Option<oneshot::Sender<String>>>);

	impl StoredObjectAdoption for ChannelAdoption {
		fn adopt(&self, path: &str) {
			self.0
				.lock()
				.expect("adoption signal lock should not be poisoned")
				.take()
				.expect("object should be adopted once")
				.send(path.to_owned())
				.expect("test should wait for adoption");
		}
	}

	#[rstest]
	#[tokio::test]
	async fn cancellation_after_remote_acceptance_still_adopts() {
		// Arrange
		let (accepted_tx, accepted_rx) = oneshot::channel();
		let (release_tx, release_rx) = oneshot::channel();
		let (adopted_tx, adopted_rx) = oneshot::channel();
		let adoption = Arc::new(ChannelAdoption(Mutex::new(Some(adopted_tx))));

		// Act
		let caller = tokio::spawn(save_if_absent_with_adoption_task(
			async move {
				accepted_tx
					.send(())
					.expect("test should wait for acceptance");
				release_rx.await.expect("test should release the provider");
				Ok("accepted.txt".to_owned())
			},
			adoption,
		));
		timeout(Duration::from_secs(1), accepted_rx)
			.await
			.expect("provider should accept the object")
			.expect("acceptance sender should remain alive");
		caller.abort();
		assert!(
			caller
				.await
				.expect_err("caller should be cancelled")
				.is_cancelled()
		);
		release_tx
			.send(())
			.expect("detached provider task should remain alive");

		// Assert
		assert_eq!(
			timeout(Duration::from_secs(1), adopted_rx)
				.await
				.expect("adoption should complete")
				.expect("adoption sender should remain alive"),
			"accepted.txt"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn provider_errors_are_forwarded_without_adoption() {
		// Arrange
		let errors = [
			StorageError::AlreadyExists("existing.txt".to_owned()),
			StorageError::NetworkError("provider unavailable".to_owned()),
		];

		for error in errors {
			let (adopted_tx, adopted_rx) = oneshot::channel();
			let expected_kind = std::mem::discriminant(&error);
			let expected_message = error.to_string();

			// Act
			let result = save_if_absent_with_adoption_task(
				async move { Err(error) },
				Arc::new(ChannelAdoption(Mutex::new(Some(adopted_tx)))),
			)
			.await
			.expect_err("provider error should be forwarded");

			// Assert
			assert_eq!(std::mem::discriminant(&result), expected_kind);
			assert_eq!(result.to_string(), expected_message);
			assert!(adopted_rx.await.is_err());
		}
	}
}
