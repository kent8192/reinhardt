//! Local filesystem storage backend

use super::backend::Storage;
use super::errors::{StorageError, StorageResult};
use super::file::{FileMetadata, StoredFile};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Local filesystem storage
pub struct LocalStorage {
	base_path: PathBuf,
	base_url: String,
}

impl LocalStorage {
	/// Create a new local filesystem storage backend
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::{LocalStorage, Storage};
	///
	/// let storage = LocalStorage::new("/tmp/storage", "http://localhost/media");
	/// assert_eq!(storage.url("test.txt"), "http://localhost/media/test.txt");
	/// ```
	pub fn new(base_path: impl Into<PathBuf>, base_url: impl Into<String>) -> Self {
		Self {
			base_path: base_path.into(),
			base_url: base_url.into(),
		}
	}
	/// Ensure the base directory exists, creating it if necessary
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::LocalStorage;
	/// use tempfile::TempDir;
	///
	/// # tokio_test::block_on(async {
	/// let temp_dir = TempDir::new().unwrap();
	/// let storage_path = temp_dir.path().join("new_storage");
	/// let storage = LocalStorage::new(&storage_path, "http://localhost/media");
	///
	/// storage.ensure_base_dir().await.unwrap();
	/// assert!(storage_path.exists());
	/// # });
	/// ```
	pub async fn ensure_base_dir(&self) -> StorageResult<()> {
		fs::create_dir_all(&self.base_path).await?;
		Ok(())
	}

	fn full_path(&self, path: &str) -> PathBuf {
		self.base_path.join(path)
	}

	/// Validate path to prevent directory traversal attacks
	fn validate_path(path: &str) -> StorageResult<()> {
		// Check for absolute paths
		if path.starts_with('/') || path.starts_with('\\') {
			return Err(StorageError::InvalidPath(format!(
				"Detected path traversal attempt in '{}'",
				path
			)));
		}

		// Check for parent directory references
		let path_obj = Path::new(path);
		for component in path_obj.components() {
			if component == std::path::Component::ParentDir {
				return Err(StorageError::InvalidPath(format!(
					"Detected path traversal attempt in '{}'",
					path
				)));
			}
		}

		// Check if path is just current dir or empty
		if path == "." || path == ".." || path.is_empty() {
			return Err(StorageError::InvalidPath(format!(
				"Could not derive file name from '{}'",
				path
			)));
		}

		Ok(())
	}

	fn compute_checksum(content: &[u8]) -> String {
		let mut hasher = Sha256::new();
		hasher.update(content);
		hex::encode(hasher.finalize())
	}
}

#[async_trait]
impl Storage for LocalStorage {
	async fn save(&self, path: &str, content: &[u8]) -> StorageResult<FileMetadata> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);

		// Create parent directories if needed
		if let Some(parent) = full_path.parent() {
			fs::create_dir_all(parent).await?;
		}

		// Write file
		fs::write(&full_path, content).await?;

		// Get file metadata
		let file_meta = fs::metadata(&full_path).await?;
		let size = file_meta.len();
		let checksum = Self::compute_checksum(content);

		Ok(FileMetadata::new(path.to_string(), size).with_checksum(checksum))
	}

	async fn read(&self, path: &str) -> StorageResult<StoredFile> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);

		if !full_path.exists() {
			return Err(StorageError::NotFound(path.to_string()));
		}

		let content = fs::read(&full_path).await?;
		let file_meta = fs::metadata(&full_path).await?;
		let size = file_meta.len();

		let metadata = FileMetadata::new(path.to_string(), size);
		Ok(StoredFile::new(metadata, content))
	}

	async fn delete(&self, path: &str) -> StorageResult<()> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);

		if !full_path.exists() {
			return Err(StorageError::NotFound(path.to_string()));
		}

		fs::remove_file(&full_path).await?;
		Ok(())
	}

	async fn exists(&self, path: &str) -> StorageResult<bool> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);
		Ok(full_path.exists())
	}

	async fn metadata(&self, path: &str) -> StorageResult<FileMetadata> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);

		if !full_path.exists() {
			return Err(StorageError::NotFound(path.to_string()));
		}

		let file_meta = fs::metadata(&full_path).await?;
		let size = file_meta.len();

		Ok(FileMetadata::new(path.to_string(), size))
	}

	async fn list(&self, path: &str) -> StorageResult<Vec<FileMetadata>> {
		// Validate path to prevent directory traversal
		// Empty path is allowed to list the base directory
		if !path.is_empty() {
			Self::validate_path(path)?;
		}

		let full_path = self.full_path(path);
		let mut entries = fs::read_dir(&full_path).await?;
		let mut results = Vec::new();

		while let Some(entry) = entries.next_entry().await? {
			let metadata = entry.metadata().await?;
			if metadata.is_file() {
				let file_name = entry.file_name().to_string_lossy().to_string();
				let relative_path = Path::new(path).join(&file_name);
				results.push(FileMetadata::new(
					relative_path.to_string_lossy().to_string(),
					metadata.len(),
				));
			}
		}

		Ok(results)
	}

	fn url(&self, path: &str) -> String {
		format!(
			"{}/{}",
			self.base_url.trim_end_matches('/'),
			path.trim_start_matches('/')
		)
	}

	fn path(&self, name: &str) -> String {
		name.to_string()
	}

	async fn get_accessed_time(&self, path: &str) -> StorageResult<chrono::DateTime<chrono::Utc>> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);

		if !full_path.exists() {
			return Err(StorageError::NotFound(path.to_string()));
		}

		let file_meta = fs::metadata(&full_path).await?;
		let accessed = file_meta.accessed()?;
		let datetime: chrono::DateTime<chrono::Utc> = accessed.into();
		Ok(datetime)
	}

	async fn get_created_time(&self, path: &str) -> StorageResult<chrono::DateTime<chrono::Utc>> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);

		if !full_path.exists() {
			return Err(StorageError::NotFound(path.to_string()));
		}

		let file_meta = fs::metadata(&full_path).await?;
		let created = file_meta.created()?;
		let datetime: chrono::DateTime<chrono::Utc> = created.into();
		Ok(datetime)
	}

	async fn get_modified_time(&self, path: &str) -> StorageResult<chrono::DateTime<chrono::Utc>> {
		// Validate path to prevent directory traversal
		Self::validate_path(path)?;

		let full_path = self.full_path(path);

		if !full_path.exists() {
			return Err(StorageError::NotFound(path.to_string()));
		}

		let file_meta = fs::metadata(&full_path).await?;
		let modified = file_meta.modified()?;
		let datetime: chrono::DateTime<chrono::Utc> = modified.into();
		Ok(datetime)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	async fn create_test_storage() -> (LocalStorage, TempDir) {
		let temp_dir = TempDir::new().unwrap();
		let storage = LocalStorage::new(temp_dir.path(), "http://localhost/media");
		storage.ensure_base_dir().await.unwrap();
		(storage, temp_dir)
	}

	#[tokio::test]
	async fn test_local_storage_path() {
		let storage = LocalStorage::new("/tmp/storage", "http://localhost/media");
		assert_eq!(storage.url("test.txt"), "http://localhost/media/test.txt");
	}

	#[tokio::test]
	async fn test_file_access_options() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test that file doesn't exist initially
		assert!(!storage.exists("storage_test").await.unwrap());

		// Save a file
		let content = b"storage contents";
		storage.save("storage_test", content).await.unwrap();

		// Check file exists
		assert!(storage.exists("storage_test").await.unwrap());

		// Read file content
		let file = storage.read("storage_test").await.unwrap();
		assert_eq!(file.content, content);

		// Delete file
		storage.delete("storage_test").await.unwrap();
		assert!(!storage.exists("storage_test").await.unwrap());
	}

	#[tokio::test]
	async fn test_file_save_with_path() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Saving a pathname should create intermediate directories
		assert!(!storage.exists("path/to").await.unwrap());

		storage
			.save("path/to/test.file", b"file saved with path")
			.await
			.unwrap();

		assert!(storage.exists("path/to/test.file").await.unwrap());

		let file = storage.read("path/to/test.file").await.unwrap();
		assert_eq!(file.content, b"file saved with path");

		storage.delete("path/to/test.file").await.unwrap();
	}

	#[tokio::test]
	async fn test_file_size() {
		let (storage, _temp_dir) = create_test_storage().await;

		storage.save("file.txt", b"test").await.unwrap();
		let metadata = storage.metadata("file.txt").await.unwrap();
		assert_eq!(metadata.size, 4);

		storage.delete("file.txt").await.unwrap();
	}

	#[tokio::test]
	async fn test_exists() {
		let (storage, _temp_dir) = create_test_storage().await;

		storage.save("dir/subdir/file.txt", b"test").await.unwrap();
		assert!(storage.exists("dir/subdir/file.txt").await.unwrap());

		storage.delete("dir/subdir/file.txt").await.unwrap();
	}

	#[tokio::test]
	async fn test_delete() {
		let (storage, _temp_dir) = create_test_storage().await;

		storage.save("dir/subdir/file.txt", b"test").await.unwrap();
		storage
			.save("dir/subdir/other_file.txt", b"test")
			.await
			.unwrap();

		assert!(storage.exists("dir/subdir/file.txt").await.unwrap());
		assert!(storage.exists("dir/subdir/other_file.txt").await.unwrap());

		storage.delete("dir/subdir/other_file.txt").await.unwrap();
		assert!(!storage.exists("dir/subdir/other_file.txt").await.unwrap());

		storage.delete("dir/subdir/file.txt").await.unwrap();
		assert!(!storage.exists("dir/subdir/file.txt").await.unwrap());
	}

	#[tokio::test]
	async fn test_delete_missing_file() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Deleting a missing file should return an error
		let result = storage.delete("missing_file.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::NotFound(_)));
	}

	#[tokio::test]
	async fn test_file_url() {
		let storage = LocalStorage::new("/tmp/storage", "http://localhost/media");

		assert_eq!(storage.url("test.file"), "http://localhost/media/test.file");

		// Test URL with base_url without trailing slash
		let storage2 = LocalStorage::new("/tmp/storage", "http://localhost/media/");
		assert_eq!(
			storage2.url("test.file"),
			"http://localhost/media/test.file"
		);
	}

	#[tokio::test]
	async fn test_base_url() {
		// Test with no trailing slash in base_url
		let storage = LocalStorage::new("/tmp/storage", "http://localhost/no_ending_slash");
		assert_eq!(
			storage.url("test.file"),
			"http://localhost/no_ending_slash/test.file"
		);
	}

	#[tokio::test]
	async fn test_listdir() {
		let (storage, _temp_dir) = create_test_storage().await;

		storage
			.save("storage_test_1", b"custom content")
			.await
			.unwrap();
		storage
			.save("storage_test_2", b"custom content")
			.await
			.unwrap();
		storage.save("dir/file_c.txt", b"test").await.unwrap();

		let files = storage.list("").await.unwrap();
		let file_names: Vec<String> = files
			.iter()
			.map(|f| {
				std::path::Path::new(&f.path)
					.file_name()
					.unwrap()
					.to_string_lossy()
					.to_string()
			})
			.collect();

		assert!(file_names.contains(&"storage_test_1".to_string()));
		assert!(file_names.contains(&"storage_test_2".to_string()));

		// Cleanup
		storage.delete("storage_test_1").await.unwrap();
		storage.delete("storage_test_2").await.unwrap();
		storage.delete("dir/file_c.txt").await.unwrap();
	}

	#[tokio::test]
	async fn test_open_missing_file() {
		let (storage, _temp_dir) = create_test_storage().await;

		let result = storage.read("missing.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::NotFound(_)));
	}

	#[tokio::test]
	async fn test_large_file_saving() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Create a large file (3 * 64KB)
		let large_content = vec![b'A'; 64 * 1024 * 3];
		storage
			.save("large_file.txt", &large_content)
			.await
			.unwrap();

		let metadata = storage.metadata("large_file.txt").await.unwrap();
		assert_eq!(metadata.size, large_content.len() as u64);

		storage.delete("large_file.txt").await.unwrap();
	}

	#[tokio::test]
	async fn test_file_checksum() {
		let (storage, _temp_dir) = create_test_storage().await;

		let metadata = storage.save("file.txt", b"test").await.unwrap();
		assert!(metadata.checksum.is_some());

		// Same content should produce same checksum
		let metadata2 = storage.save("file2.txt", b"test").await.unwrap();
		assert_eq!(metadata.checksum, metadata2.checksum);

		storage.delete("file.txt").await.unwrap();
		storage.delete("file2.txt").await.unwrap();
	}

	#[tokio::test]
	async fn test_file_get_accessed_time() {
		let (storage, _temp_dir) = create_test_storage().await;

		storage.save("test.file", b"custom contents").await.unwrap();

		let atime = storage.get_accessed_time("test.file").await.unwrap();
		let now = chrono::Utc::now();

		// Access time should be close to current time
		let diff = (now - atime).num_seconds().abs();
		assert!(
			diff < 5,
			"Access time difference too large: {} seconds",
			diff
		);

		storage.delete("test.file").await.unwrap();
	}

	#[tokio::test]
	async fn test_file_get_created_time() {
		let (storage, _temp_dir) = create_test_storage().await;

		storage.save("test.file", b"custom contents").await.unwrap();

		let ctime = storage.get_created_time("test.file").await.unwrap();
		let now = chrono::Utc::now();

		// Creation time should be close to current time
		let diff = (now - ctime).num_seconds().abs();
		assert!(
			diff < 5,
			"Creation time difference too large: {} seconds",
			diff
		);

		storage.delete("test.file").await.unwrap();
	}

	#[tokio::test]
	async fn test_file_get_modified_time() {
		let (storage, _temp_dir) = create_test_storage().await;

		storage.save("test.file", b"custom contents").await.unwrap();

		let mtime = storage.get_modified_time("test.file").await.unwrap();
		let now = chrono::Utc::now();

		// Modified time should be close to current time
		let diff = (now - mtime).num_seconds().abs();
		assert!(
			diff < 5,
			"Modified time difference too large: {} seconds",
			diff
		);

		storage.delete("test.file").await.unwrap();
	}

	#[tokio::test]
	async fn test_file_modified_time_changes() {
		use tokio::time::{Duration, sleep};

		let (storage, _temp_dir) = create_test_storage().await;

		storage.save("file.txt", b"test").await.unwrap();
		let modified_time = storage.get_modified_time("file.txt").await.unwrap();

		// Wait a bit
		sleep(Duration::from_millis(100)).await;

		// Modify the file
		storage.save("file.txt", b"new content").await.unwrap();

		let new_modified_time = storage.get_modified_time("file.txt").await.unwrap();
		assert!(
			new_modified_time > modified_time,
			"Modified time should increase after file change"
		);

		storage.delete("file.txt").await.unwrap();
	}

	#[tokio::test]
	async fn test_file_storage_prevents_directory_traversal() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test parent directory traversal
		let result = storage.save("../test.txt", b"test").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test absolute path
		let result = storage.save("/etc/passwd", b"test").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
	}

	#[tokio::test]
	async fn test_storage_dangerous_paths() {
		let (storage, _temp_dir) = create_test_storage().await;

		let dangerous_paths = vec!["..", ".", "", "../path", "tmp/../path", "/tmp/path"];

		for path in dangerous_paths {
			let result = storage.save(path, b"test").await;
			assert!(
				result.is_err(),
				"Path '{}' should be rejected but was accepted",
				path
			);
			assert!(
				matches!(result.unwrap_err(), StorageError::InvalidPath(_)),
				"Path '{}' should return InvalidPath error",
				path
			);
		}
	}

	#[tokio::test]
	async fn test_path_with_dots_in_filename() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Valid path with dots in directory and filename should work
		storage.save("my.dir/test.file.txt", b"test").await.unwrap();
		assert!(storage.exists("my.dir/test.file.txt").await.unwrap());

		storage.delete("my.dir/test.file.txt").await.unwrap();
	}

	#[tokio::test]
	async fn test_url_encoding() {
		let storage = LocalStorage::new("/tmp/storage", "http://localhost/media");

		// Basic file
		assert_eq!(storage.url("test.file"), "http://localhost/media/test.file");

		// File with special characters (note: basic implementation doesn't encode)
		// This test documents current behavior
		let url = storage.url("test file.txt");
		assert!(url.contains("test file.txt"));
	}

	#[tokio::test]
	async fn test_read_prevents_directory_traversal() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test parent directory traversal
		let result = storage.read("../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test absolute path
		let result = storage.read("/etc/passwd").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test embedded parent directory
		let result = storage.read("tmp/../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
	}

	#[tokio::test]
	async fn test_delete_prevents_directory_traversal() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test parent directory traversal
		let result = storage.delete("../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test absolute path
		let result = storage.delete("/etc/passwd").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
	}

	#[tokio::test]
	async fn test_exists_prevents_directory_traversal() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test parent directory traversal
		let result = storage.exists("../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test absolute path
		let result = storage.exists("/etc/passwd").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
	}

	#[tokio::test]
	async fn test_metadata_prevents_directory_traversal() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test parent directory traversal
		let result = storage.metadata("../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test absolute path
		let result = storage.metadata("/etc/passwd").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
	}

	#[tokio::test]
	async fn test_list_prevents_directory_traversal() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test parent directory traversal
		let result = storage.list("../test").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test absolute path
		let result = storage.list("/etc").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
	}

	#[tokio::test]
	async fn test_get_time_operations_prevent_directory_traversal() {
		let (storage, _temp_dir) = create_test_storage().await;

		// Test get_accessed_time
		let result = storage.get_accessed_time("../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test get_created_time
		let result = storage.get_created_time("../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));

		// Test get_modified_time
		let result = storage.get_modified_time("../test.txt").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
	}
}
