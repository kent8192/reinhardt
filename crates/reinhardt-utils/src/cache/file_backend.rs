//! File-based cache implementation

use super::cache_trait::Cache;
use super::entry::CacheEntry;
use async_trait::async_trait;
use reinhardt_core::exception::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::sync::RwLock;

/// File-based cache backend
///
/// Persists cache entries to the filesystem with TTL support.
/// Cache files are stored in a directory with hashed filenames for safety.
///
/// # Examples
///
/// ```
/// use reinhardt_utils::cache::{Cache, FileCache};
/// use std::path::PathBuf;
///
/// # async fn example() -> reinhardt_core::exception::Result<()> {
/// let cache = FileCache::new(PathBuf::from("/tmp/cache")).await?;
///
/// // Set a value
/// cache.set("key", &"value", None).await?;
///
/// // Get a value
/// let value: Option<String> = cache.get("key").await?;
/// assert_eq!(value, Some("value".to_string()));
///
/// // Delete a value
/// cache.delete("key").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct FileCache {
	cache_dir: PathBuf,
	default_ttl: Option<Duration>,
	index: std::sync::Arc<RwLock<HashMap<String, PathBuf>>>,
}

impl FileCache {
	/// Create a new file-based cache
	///
	/// Creates the cache directory if it doesn't exist.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::FileCache;
	/// use std::path::PathBuf;
	///
	/// # async fn example() -> reinhardt_core::exception::Result<()> {
	/// let cache = FileCache::new(PathBuf::from("/tmp/my_cache")).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(cache_dir: PathBuf) -> Result<Self> {
		fs::create_dir_all(&cache_dir)
			.await
			.map_err(|e| Error::Internal(format!("Failed to create cache directory: {}", e)))?;

		Ok(Self {
			cache_dir,
			default_ttl: None,
			index: std::sync::Arc::new(RwLock::new(HashMap::new())),
		})
	}

	/// Set a default TTL for all cache entries
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{Cache, FileCache};
	/// use std::path::PathBuf;
	/// use std::time::Duration;
	///
	/// # async fn example() -> reinhardt_core::exception::Result<()> {
	/// let cache = FileCache::new(PathBuf::from("/tmp/cache"))
	///     .await?
	///     .with_default_ttl(Duration::from_secs(300));
	///
	/// // Values will expire after 300 seconds by default
	/// cache.set("key", &"value", None).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
		self.default_ttl = Some(ttl);
		self
	}

	/// Clean up expired entries from the filesystem
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::cache::{Cache, FileCache};
	/// use std::path::PathBuf;
	/// use std::time::Duration;
	///
	/// # async fn example() -> reinhardt_core::exception::Result<()> {
	/// let cache = FileCache::new(PathBuf::from("/tmp/cache")).await?;
	///
	/// // Set a value with short TTL
	/// cache.set("key", &"value", Some(Duration::from_millis(10))).await?;
	///
	/// // Wait for expiration
	///
	/// // Clean up expired entries
	/// cache.cleanup_expired().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn cleanup_expired(&self) -> Result<()> {
		let mut index = self.index.write().await;
		let mut to_remove = Vec::new();

		for (key, path) in index.iter() {
			if let Ok(data) = fs::read(path).await
				&& let Ok(entry) = serde_json::from_slice::<CacheEntry>(&data)
				&& entry.is_expired()
			{
				to_remove.push((key.clone(), path.clone()));
			}
		}

		for (key, path) in to_remove {
			let _ = fs::remove_file(&path).await;
			index.remove(&key);
		}

		Ok(())
	}

	/// Get the file path for a cache key
	fn get_file_path(&self, key: &str) -> PathBuf {
		// Hash the key to create a safe filename using SHA-256
		use sha2::{Digest, Sha256};
		let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
		self.cache_dir.join(hash)
	}

	/// Load the cache index from filesystem
	// Reserved for future cache persistence recovery
	#[allow(dead_code)]
	async fn load_index(&self) -> Result<()> {
		let mut index = self.index.write().await;
		index.clear();

		let mut entries = fs::read_dir(&self.cache_dir)
			.await
			.map_err(|e| Error::Internal(format!("Failed to read cache directory: {}", e)))?;

		while let Some(entry) = entries
			.next_entry()
			.await
			.map_err(|e| Error::Internal(format!("Failed to read directory entry: {}", e)))?
		{
			let path = entry.path();
			if path.is_file()
				&& let Ok(data) = fs::read(&path).await
				&& let Ok(cache_entry) = serde_json::from_slice::<StoredEntry>(&data)
				&& !cache_entry.entry.is_expired()
			{
				index.insert(cache_entry.key.clone(), path);
			}
		}

		Ok(())
	}
}

/// Entry stored in file system with key information
#[derive(Debug, Serialize, Deserialize)]
struct StoredEntry {
	key: String,
	entry: CacheEntry,
}

#[async_trait]
impl Cache for FileCache {
	async fn get<T>(&self, key: &str) -> Result<Option<T>>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		let path = self.get_file_path(key);

		if !path.exists() {
			return Ok(None);
		}

		let data = fs::read(&path)
			.await
			.map_err(|e| Error::Internal(format!("Failed to read cache file: {}", e)))?;

		let stored: StoredEntry =
			serde_json::from_slice(&data).map_err(|e| Error::Serialization(e.to_string()))?;

		if stored.entry.is_expired() {
			// Clean up expired file
			let _ = fs::remove_file(&path).await;
			let mut index = self.index.write().await;
			index.remove(key);
			return Ok(None);
		}

		let value = serde_json::from_slice(&stored.entry.value)
			.map_err(|e| Error::Serialization(e.to_string()))?;

		Ok(Some(value))
	}

	async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		let serialized =
			serde_json::to_vec(value).map_err(|e| Error::Serialization(e.to_string()))?;

		let ttl = ttl.or(self.default_ttl);
		let entry = CacheEntry::new(serialized, ttl);

		let stored = StoredEntry {
			key: key.to_string(),
			entry,
		};

		let path = self.get_file_path(key);
		let data = serde_json::to_vec(&stored).map_err(|e| Error::Serialization(e.to_string()))?;

		fs::write(&path, data)
			.await
			.map_err(|e| Error::Internal(format!("Failed to write cache file: {}", e)))?;

		let mut index = self.index.write().await;
		index.insert(key.to_string(), path);

		Ok(())
	}

	async fn delete(&self, key: &str) -> Result<()> {
		let path = self.get_file_path(key);

		if path.exists() {
			fs::remove_file(&path)
				.await
				.map_err(|e| Error::Internal(format!("Failed to delete cache file: {}", e)))?;
		}

		let mut index = self.index.write().await;
		index.remove(key);

		Ok(())
	}

	async fn has_key(&self, key: &str) -> Result<bool> {
		let path = self.get_file_path(key);

		if !path.exists() {
			return Ok(false);
		}

		let data = fs::read(&path)
			.await
			.map_err(|e| Error::Internal(format!("Failed to read cache file: {}", e)))?;

		let stored: StoredEntry =
			serde_json::from_slice(&data).map_err(|e| Error::Serialization(e.to_string()))?;

		Ok(!stored.entry.is_expired())
	}

	async fn clear(&self) -> Result<()> {
		let mut index = self.index.write().await;

		for path in index.values() {
			let _ = fs::remove_file(path).await;
		}

		index.clear();

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::time::Duration;

	/// Polls a condition until it returns true or timeout is reached.
	async fn poll_until<F, Fut>(
		timeout: std::time::Duration,
		interval: std::time::Duration,
		mut condition: F,
	) -> std::result::Result<(), String>
	where
		F: FnMut() -> Fut,
		Fut: std::future::Future<Output = bool>,
	{
		let start = std::time::Instant::now();
		while start.elapsed() < timeout {
			if condition().await {
				return Ok(());
			}
			tokio::time::sleep(interval).await;
		}
		Err(format!("Timeout after {:?} waiting for condition", timeout))
	}

	fn get_test_dir(name: &str) -> PathBuf {
		PathBuf::from(format!("/tmp/reinhardt_file_cache_test_{}", name))
	}

	async fn create_test_cache(name: &str) -> FileCache {
		let temp_dir = get_test_dir(name);
		// Clean up before test
		let _ = tokio::fs::remove_dir_all(&temp_dir).await;
		FileCache::new(temp_dir).await.unwrap()
	}

	#[tokio::test]
	async fn test_file_cache_basic() {
		let cache = create_test_cache("basic").await;

		// Set and get
		cache.set("key1", &"value1", None).await.unwrap();
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Has key
		assert!(cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("key2").await.unwrap());

		// Delete
		cache.delete("key1").await.unwrap();
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, None);
	}

	#[tokio::test]
	async fn test_file_cache_ttl() {
		let cache = create_test_cache("ttl").await;

		// Set with short TTL
		cache
			.set("key1", &"value1", Some(Duration::from_millis(100)))
			.await
			.unwrap();

		// Should exist immediately
		let value: Option<String> = cache.get("key1").await.unwrap();
		assert_eq!(value, Some("value1".to_string()));

		// Poll until key expires
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				let value: Option<String> = cache.get("key1").await.unwrap();
				value.is_none()
			},
		)
		.await
		.expect("Key should expire within 200ms");
	}

	#[tokio::test]
	async fn test_file_cache_cleanup_expired() {
		let cache = create_test_cache("cleanup").await;

		// Set some values with different TTLs
		cache
			.set("key1", &"value1", Some(Duration::from_millis(100)))
			.await
			.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		// Poll until first key expires
		poll_until(
			Duration::from_millis(200),
			Duration::from_millis(10),
			|| async {
				let value: Option<String> = cache.get("key1").await.unwrap();
				value.is_none()
			},
		)
		.await
		.expect("Key1 should expire within 200ms");

		// Cleanup expired entries
		cache.cleanup_expired().await.unwrap();

		// key1 should be gone, key2 should remain
		assert!(!cache.has_key("key1").await.unwrap());
		assert!(cache.has_key("key2").await.unwrap());
	}

	#[tokio::test]
	async fn test_file_cache_persistence() {
		let temp_dir = get_test_dir("persistence");
		let _ = tokio::fs::remove_dir_all(&temp_dir).await;

		{
			let cache = FileCache::new(temp_dir.clone()).await.unwrap();
			cache.set("key1", &"value1", None).await.unwrap();
			cache.set("key2", &"value2", None).await.unwrap();
		}

		// Create new cache instance with same directory
		{
			let cache = FileCache::new(temp_dir.clone()).await.unwrap();
			cache.load_index().await.unwrap();

			// Values should still exist
			let value: Option<String> = cache.get("key1").await.unwrap();
			assert_eq!(value, Some("value1".to_string()));

			let value: Option<String> = cache.get("key2").await.unwrap();
			assert_eq!(value, Some("value2".to_string()));
		}
	}

	#[tokio::test]
	async fn test_file_cache_clear() {
		let cache = create_test_cache("clear").await;

		cache.set("key1", &"value1", None).await.unwrap();
		cache.set("key2", &"value2", None).await.unwrap();

		cache.clear().await.unwrap();

		assert!(!cache.has_key("key1").await.unwrap());
		assert!(!cache.has_key("key2").await.unwrap());
	}
}
