//! In-memory storage backend for testing and development

use super::backend::Storage;
use super::errors::{StorageError, StorageResult};
use super::file::{FileMetadata, StoredFile};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// File entry in memory storage
#[derive(Clone, Debug)]
struct MemoryFile {
	content: Vec<u8>,
	created_at: DateTime<Utc>,
	modified_at: DateTime<Utc>,
	accessed_at: DateTime<Utc>,
}

impl MemoryFile {
	fn new(content: Vec<u8>) -> Self {
		let now = Utc::now();
		Self {
			content,
			created_at: now,
			modified_at: now,
			accessed_at: now,
		}
	}

	fn update(&mut self, content: Vec<u8>) {
		self.content = content;
		self.modified_at = Utc::now();
	}

	fn access(&mut self) {
		self.accessed_at = Utc::now();
	}
}

/// In-memory storage backend
#[derive(Clone)]
pub struct InMemoryStorage {
	files: Arc<RwLock<HashMap<String, MemoryFile>>>,
	base_url: String,
	location: String,
	file_permissions_mode: Option<u32>,
	directory_permissions_mode: Option<u32>,
}

impl InMemoryStorage {
	/// Create a new in-memory storage backend
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::InMemoryStorage;
	///
	/// let storage = InMemoryStorage::new("memory_root", "http://localhost/media");
	/// assert_eq!(storage.base_location(), "memory_root");
	/// assert_eq!(storage.base_url(), "http://localhost/media");
	/// ```
	pub fn new(location: impl Into<String>, base_url: impl Into<String>) -> Self {
		Self {
			files: Arc::new(RwLock::new(HashMap::new())),
			base_url: base_url.into(),
			location: location.into(),
			file_permissions_mode: None,
			directory_permissions_mode: None,
		}
	}
	/// Set file and directory permission modes
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::InMemoryStorage;
	///
	/// let storage = InMemoryStorage::new("memory_root", "http://localhost/media")
	///     .with_permissions(Some(0o644), Some(0o755));
	/// assert_eq!(storage.file_permissions_mode(), Some(0o644));
	/// assert_eq!(storage.directory_permissions_mode(), Some(0o755));
	/// ```
	pub fn with_permissions(mut self, file_mode: Option<u32>, dir_mode: Option<u32>) -> Self {
		self.file_permissions_mode = file_mode;
		self.directory_permissions_mode = dir_mode;
		self
	}
	/// Get the base location path
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::InMemoryStorage;
	///
	/// let storage = InMemoryStorage::new("my_location", "http://localhost/media");
	/// assert_eq!(storage.base_location(), "my_location");
	/// ```
	pub fn base_location(&self) -> &str {
		&self.location
	}
	/// Get the base URL for file access
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::InMemoryStorage;
	///
	/// let storage = InMemoryStorage::new("memory_root", "http://example.com/files");
	/// assert_eq!(storage.base_url(), "http://example.com/files");
	/// ```
	pub fn base_url(&self) -> &str {
		&self.base_url
	}
	/// Get the file permissions mode if set
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::InMemoryStorage;
	///
	/// let storage = InMemoryStorage::new("memory_root", "http://localhost/media")
	///     .with_permissions(Some(0o644), None);
	/// assert_eq!(storage.file_permissions_mode(), Some(0o644));
	///
	/// let storage_no_perms = InMemoryStorage::new("memory_root", "http://localhost/media");
	/// assert_eq!(storage_no_perms.file_permissions_mode(), None);
	/// ```
	pub fn file_permissions_mode(&self) -> Option<u32> {
		self.file_permissions_mode
	}
	/// Get the directory permissions mode if set
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::InMemoryStorage;
	///
	/// let storage = InMemoryStorage::new("memory_root", "http://localhost/media")
	///     .with_permissions(None, Some(0o755));
	/// assert_eq!(storage.directory_permissions_mode(), Some(0o755));
	///
	/// let storage_no_perms = InMemoryStorage::new("memory_root", "http://localhost/media");
	/// assert_eq!(storage_no_perms.directory_permissions_mode(), None);
	/// ```
	pub fn directory_permissions_mode(&self) -> Option<u32> {
		self.directory_permissions_mode
	}
	/// Deconstruct storage into components for serialization
	///
	/// Returns a tuple of (path, args, kwargs) compatible with Django storage conventions
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::storage::InMemoryStorage;
	///
	/// let storage = InMemoryStorage::new("memory_root", "http://localhost/media");
	/// let (path, args, kwargs) = storage.deconstruct();
	/// assert_eq!(path, "reinhardt_storage.InMemoryStorage");
	/// assert_eq!(args, ());
	/// assert_eq!(kwargs.get("location").unwrap(), "memory_root");
	/// assert_eq!(kwargs.get("base_url").unwrap(), "http://localhost/media");
	/// ```
	pub fn deconstruct(&self) -> (&str, (), HashMap<String, String>) {
		let mut kwargs = HashMap::new();
		kwargs.insert("location".to_string(), self.location.clone());
		kwargs.insert("base_url".to_string(), self.base_url.clone());
		if let Some(mode) = self.file_permissions_mode {
			kwargs.insert("file_permissions_mode".to_string(), format!("0o{:o}", mode));
		}
		if let Some(mode) = self.directory_permissions_mode {
			kwargs.insert(
				"directory_permissions_mode".to_string(),
				format!("0o{:o}", mode),
			);
		}
		("reinhardt_storage.InMemoryStorage", (), kwargs)
	}
}

#[async_trait]
impl Storage for InMemoryStorage {
	async fn save(&self, path: &str, content: &[u8]) -> StorageResult<FileMetadata> {
		let mut files = self.files.write().unwrap_or_else(|e| e.into_inner());

		if let Some(existing) = files.get_mut(path) {
			existing.update(content.to_vec());
		} else {
			let file = MemoryFile::new(content.to_vec());
			files.insert(path.to_string(), file);
		}

		Ok(FileMetadata::new(path.to_string(), content.len() as u64))
	}

	async fn read(&self, path: &str) -> StorageResult<StoredFile> {
		let mut files = self.files.write().unwrap_or_else(|e| e.into_inner());

		let file = files
			.get_mut(path)
			.ok_or_else(|| StorageError::NotFound(path.to_string()))?;

		file.access();

		let metadata = FileMetadata::new(path.to_string(), file.content.len() as u64);
		Ok(StoredFile::new(metadata, file.content.clone()))
	}

	async fn delete(&self, path: &str) -> StorageResult<()> {
		let mut files = self.files.write().unwrap_or_else(|e| e.into_inner());

		// Support directory deletion - remove all files with this prefix
		if path.ends_with('/') || !files.contains_key(path) {
			let prefix = path.trim_end_matches('/');
			let to_remove: Vec<String> = files
				.keys()
				.filter(|k| k.starts_with(&format!("{}/", prefix)))
				.cloned()
				.collect();

			for key in to_remove {
				files.remove(&key);
			}
		} else {
			files
				.remove(path)
				.ok_or_else(|| StorageError::NotFound(path.to_string()))?;
		}

		Ok(())
	}

	async fn exists(&self, path: &str) -> StorageResult<bool> {
		let files = self.files.read().unwrap_or_else(|e| e.into_inner());

		// Check exact match
		if files.contains_key(path) {
			return Ok(true);
		}

		// Check if it's a directory (has files with this prefix)
		let prefix = format!("{}/", path.trim_end_matches('/'));
		Ok(files.keys().any(|k| k.starts_with(&prefix)))
	}

	async fn metadata(&self, path: &str) -> StorageResult<FileMetadata> {
		let files = self.files.read().unwrap_or_else(|e| e.into_inner());

		let file = files
			.get(path)
			.ok_or_else(|| StorageError::NotFound(path.to_string()))?;

		Ok(FileMetadata::new(
			path.to_string(),
			file.content.len() as u64,
		))
	}

	async fn list(&self, path: &str) -> StorageResult<Vec<FileMetadata>> {
		let files = self.files.read().unwrap_or_else(|e| e.into_inner());

		let prefix = if path.is_empty() {
			String::new()
		} else {
			format!("{}/", path.trim_end_matches('/'))
		};

		let mut results = Vec::new();
		for (key, file) in files.iter() {
			if path.is_empty() {
				// Root level - only files without /
				if !key.contains('/') {
					results.push(FileMetadata::new(key.clone(), file.content.len() as u64));
				}
			} else if key.starts_with(&prefix) {
				let relative = &key[prefix.len()..];
				// Only direct children (no further /)
				if !relative.contains('/') {
					results.push(FileMetadata::new(key.clone(), file.content.len() as u64));
				}
			}
		}

		Ok(results)
	}

	fn url(&self, path: &str) -> String {
		if path.is_empty() || path == "." {
			return format!("{}/", self.base_url.trim_end_matches('/'));
		}
		format!(
			"{}/{}",
			self.base_url.trim_end_matches('/'),
			path.trim_start_matches('/')
		)
	}

	fn path(&self, name: &str) -> String {
		name.to_string()
	}

	async fn get_accessed_time(&self, path: &str) -> StorageResult<DateTime<Utc>> {
		let files = self.files.read().unwrap_or_else(|e| e.into_inner());

		let file = files
			.get(path)
			.ok_or_else(|| StorageError::NotFound(path.to_string()))?;

		Ok(file.accessed_at)
	}

	async fn get_created_time(&self, path: &str) -> StorageResult<DateTime<Utc>> {
		let files = self.files.read().unwrap_or_else(|e| e.into_inner());

		let file = files
			.get(path)
			.ok_or_else(|| StorageError::NotFound(path.to_string()))?;

		Ok(file.created_at)
	}

	async fn get_modified_time(&self, path: &str) -> StorageResult<DateTime<Utc>> {
		let files = self.files.read().unwrap_or_else(|e| e.into_inner());

		let file = files
			.get(path)
			.ok_or_else(|| StorageError::NotFound(path.to_string()))?;

		Ok(file.modified_at)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_inmemory_write_and_read() {
		let storage = InMemoryStorage::new("memory_root", "http://localhost/media");

		// Write string content
		storage.save("file.txt", b"hello").await.unwrap();
		let file = storage.read("file.txt").await.unwrap();
		assert_eq!(file.content, b"hello");

		// Write binary content
		storage.save("file.dat", b"hello").await.unwrap();
		let file = storage.read("file.dat").await.unwrap();
		assert_eq!(file.content, b"hello");
	}

	#[tokio::test]
	async fn test_inmemory_str_bytes_conversion() {
		let storage = InMemoryStorage::new("memory_root", "http://localhost/media");

		// InMemoryStorage handles conversion from str to bytes and back
		storage.save("file.txt", b"hello").await.unwrap();
		let file = storage.read("file.txt").await.unwrap();
		assert_eq!(file.content, b"hello");

		storage.save("file.dat", b"hello").await.unwrap();
		let file = storage.read("file.dat").await.unwrap();
		assert_eq!(file.content, b"hello");
	}

	#[tokio::test]
	async fn test_inmemory_url_generation() {
		let storage = InMemoryStorage::new("memory_root", "http://localhost/media");
		assert_eq!(storage.url("test.txt"), "http://localhost/media/test.txt");

		// Test with base_url ending with slash
		let storage2 = InMemoryStorage::new("memory_root", "http://localhost/media/");
		assert_eq!(storage2.url("test.txt"), "http://localhost/media/test.txt");
	}

	#[tokio::test]
	async fn test_inmemory_url_with_none_filename() {
		let storage = InMemoryStorage::new("memory_root", "/test_media_url/");
		assert_eq!(storage.url(""), "/test_media_url/");
	}

	#[tokio::test]
	async fn test_inmemory_deconstruction() {
		let storage = InMemoryStorage::new("memory_root", "http://localhost/media");
		let (path, args, kwargs) = storage.deconstruct();

		assert_eq!(path, "reinhardt_storage.InMemoryStorage");
		assert_eq!(args, ());
		assert_eq!(kwargs.get("location").unwrap(), "memory_root");
		assert_eq!(kwargs.get("base_url").unwrap(), "http://localhost/media");

		// Test with permissions
		let storage_with_perms = InMemoryStorage::new("custom_path", "http://example.com/")
			.with_permissions(Some(0o755), Some(0o600));
		let (_, _, kwargs) = storage_with_perms.deconstruct();

		assert_eq!(kwargs.get("location").unwrap(), "custom_path");
		assert_eq!(kwargs.get("base_url").unwrap(), "http://example.com/");
		assert_eq!(kwargs.get("file_permissions_mode").unwrap(), "0o755");
		assert_eq!(kwargs.get("directory_permissions_mode").unwrap(), "0o600");
	}

	#[tokio::test]
	async fn test_inmemory_settings_changed() {
		// Properties using settings values as defaults should be updated
		let storage = InMemoryStorage::new("explicit_location", "explicit_base_url/")
			.with_permissions(Some(0o666), Some(0o666));

		assert_eq!(storage.base_location(), "explicit_location");
		assert_eq!(storage.base_url(), "explicit_base_url/");
		assert_eq!(storage.file_permissions_mode(), Some(0o666));
		assert_eq!(storage.directory_permissions_mode(), Some(0o666));

		// Test defaults
		let defaults_storage = InMemoryStorage::new("media_root", "media_url/");
		assert_eq!(defaults_storage.base_location(), "media_root");
		assert_eq!(defaults_storage.base_url(), "media_url/");
	}
}
