//! Memory backend for dynamic settings
//!
//! This backend provides in-memory runtime configuration storage with TTL support.
//!
//! ## Features
//!
//! - **HashMap-based storage**: Fast in-memory key-value store
//! - **TTL support**: Automatic expiration of settings
//! - **Thread-safe**: Uses `Arc<RwLock<_>>` for concurrent access
//! - **Lazy cleanup**: Expired entries are removed on access
//!
//! ## Example
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_conf::settings::backends::MemoryBackend;
//! use reinhardt_conf::settings::dynamic::{DynamicBackend, DynamicSettings};
//! use std::sync::Arc;
//!
//! # futures::executor::block_on(async {
//! // Create backend
//! let backend = Arc::new(MemoryBackend::new());
//!
//! // Create dynamic settings
//! let settings = DynamicSettings::new(backend.clone());
//!
//! // Set a value with 60 second TTL
//! settings.set("session_token", &"abc123", Some(60)).await?;
//!
//! // Get the value
//! let token: String = settings.get("session_token").await?.unwrap();
//! assert_eq!(token, "abc123");
//!
//! // After expiration, value is automatically removed
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }).unwrap();
//! # Ok(())
//! # }
//! ```

use crate::settings::dynamic::{DynamicBackend, DynamicResult};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Value with optional expiration time
#[derive(Clone)]
struct ValueEntry {
	value: serde_json::Value,
	expires_at: Option<Instant>,
}

impl ValueEntry {
	/// Create a new entry with optional TTL
	fn new(value: serde_json::Value, ttl: Option<u64>) -> Self {
		Self {
			value,
			expires_at: ttl.map(|secs| Instant::now() + Duration::from_secs(secs)),
		}
	}

	/// Check if this entry has expired
	fn is_expired(&self) -> bool {
		self.expires_at
			.map(|expires| Instant::now() >= expires)
			.unwrap_or(false)
	}
}

/// In-memory backend for dynamic settings
///
/// This backend stores configuration in memory using a HashMap with automatic
/// TTL-based expiration. It's useful for development, testing, or when you need
/// fast ephemeral configuration storage.
///
/// The backend is thread-safe and can be shared across multiple threads using `Arc`.
///
/// ## Example
///
/// ```
/// # #[cfg(feature = "async")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_conf::settings::backends::MemoryBackend;
/// use reinhardt_conf::settings::dynamic::DynamicBackend;
/// use std::sync::Arc;
///
/// let backend = Arc::new(MemoryBackend::new());
///
/// // Set values
/// backend.set("key1", &serde_json::json!("value1"), None).await?;
/// backend.set("key2", &serde_json::json!(42), Some(60)).await?;
///
/// // Get values
/// let value1 = backend.get("key1").await?;
/// assert!(value1.is_some());
/// assert_eq!(value1.unwrap(), serde_json::json!("value1"));
///
/// // Check existence
/// assert!(backend.exists("key1").await?);
/// assert!(backend.exists("key2").await?);
///
/// // List all keys
/// let keys = backend.keys().await?;
/// assert_eq!(keys.len(), 2);
/// assert!(keys.contains(&"key1".to_string()));
/// assert!(keys.contains(&"key2".to_string()));
/// # Ok(())
/// # }
/// # #[cfg(feature = "async")]
/// # tokio::runtime::Runtime::new().unwrap().block_on(example()).unwrap();
/// ```
pub struct MemoryBackend {
	data: Arc<RwLock<HashMap<String, ValueEntry>>>,
}

impl MemoryBackend {
	/// Create a new memory backend
	///
	/// Returns an empty backend ready to store settings.
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	///
	/// let backend = MemoryBackend::new();
	/// ```
	pub fn new() -> Self {
		Self {
			data: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Remove all expired entries from storage
	///
	/// This method scans all entries and removes those that have expired.
	/// It's called automatically during operations like `get` and `exists`,
	/// but can also be called manually for cleanup.
	///
	/// ## Example
	///
	/// ```rust
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	/// use reinhardt_conf::settings::dynamic::DynamicBackend;
	///
	/// let backend = MemoryBackend::new();
	///
	/// // Set value with 1 second TTL
	/// backend.set("temp", &serde_json::json!("value"), Some(1)).await.unwrap();
	///
	/// // Wait for expiration
	/// tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
	///
	/// // Clean up expired entries
	/// backend.cleanup_expired();
	///
	/// assert!(!backend.exists("temp").await.unwrap());
	/// # Ok::<(), Box<dyn std::error::Error>>(())
	/// # }).unwrap();
	/// ```
	pub fn cleanup_expired(&self) {
		let mut data = self.data.write();
		data.retain(|_, entry| !entry.is_expired());
	}

	/// Get the number of entries in storage (including expired)
	///
	/// ## Example
	///
	/// ```rust
	/// # #[cfg(feature = "async")]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	/// use reinhardt_conf::settings::dynamic::DynamicBackend;
	///
	/// # futures::executor::block_on(async {
	/// let backend = MemoryBackend::new();
	/// assert_eq!(backend.len(), 0);
	///
	/// backend.set("key", &serde_json::json!("value"), None).await?;
	/// assert_eq!(backend.len(), 1);
	/// # Ok::<(), Box<dyn std::error::Error>>(())
	/// # }).unwrap();
	/// # Ok(())
	/// # }
	/// ```
	pub fn len(&self) -> usize {
		self.data.read().len()
	}

	/// Check if the backend is empty
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	///
	/// let backend = MemoryBackend::new();
	/// assert!(backend.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.data.read().is_empty()
	}

	/// Clear all entries from storage
	///
	/// ## Example
	///
	/// ```rust
	/// # #[cfg(feature = "async")]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	/// use reinhardt_conf::settings::dynamic::DynamicBackend;
	///
	/// # futures::executor::block_on(async {
	/// let backend = MemoryBackend::new();
	/// backend.set("key", &serde_json::json!("value"), None).await?;
	///
	/// backend.clear();
	/// assert!(backend.is_empty());
	/// # Ok::<(), Box<dyn std::error::Error>>(())
	/// # }).unwrap();
	/// # Ok(())
	/// # }
	/// ```
	pub fn clear(&self) {
		self.data.write().clear();
	}
}

impl Default for MemoryBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl DynamicBackend for MemoryBackend {
	async fn get(&self, key: &str) -> DynamicResult<Option<serde_json::Value>> {
		let data = self.data.read();

		if let Some(entry) = data.get(key) {
			if entry.is_expired() {
				// Drop read lock before acquiring write lock
				drop(data);
				// Remove expired entry
				self.data.write().remove(key);
				Ok(None)
			} else {
				Ok(Some(entry.value.clone()))
			}
		} else {
			Ok(None)
		}
	}

	async fn set(
		&self,
		key: &str,
		value: &serde_json::Value,
		ttl: Option<u64>,
	) -> DynamicResult<()> {
		let entry = ValueEntry::new(value.clone(), ttl);
		self.data.write().insert(key.to_string(), entry);
		Ok(())
	}

	async fn delete(&self, key: &str) -> DynamicResult<()> {
		self.data.write().remove(key);
		Ok(())
	}

	async fn exists(&self, key: &str) -> DynamicResult<bool> {
		let data = self.data.read();

		if let Some(entry) = data.get(key) {
			if entry.is_expired() {
				// Drop read lock before acquiring write lock
				drop(data);
				// Remove expired entry
				self.data.write().remove(key);
				Ok(false)
			} else {
				Ok(true)
			}
		} else {
			Ok(false)
		}
	}

	async fn keys(&self) -> DynamicResult<Vec<String>> {
		let data = self.data.read();

		// Collect non-expired keys
		let valid_keys: Vec<String> = data
			.iter()
			.filter(|(_, entry)| !entry.is_expired())
			.map(|(key, _)| key.clone())
			.collect();

		// If we found expired keys, clean them up
		if valid_keys.len() < data.len() {
			drop(data);
			self.cleanup_expired();
		}

		Ok(valid_keys)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Poll a condition until it returns true or timeout expires
	async fn poll_until<F, Fut>(
		timeout: std::time::Duration,
		interval: std::time::Duration,
		mut condition: F,
	) -> Result<(), String>
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
		Err(format!("Condition not met within {:?}", timeout))
	}

	#[tokio::test]
	async fn test_basic_operations() {
		let backend = MemoryBackend::new();

		// Set and get
		backend
			.set("key1", &serde_json::json!("value1"), None)
			.await
			.unwrap();
		let value = backend.get("key1").await.unwrap();
		assert_eq!(value, Some(serde_json::json!("value1")));

		// Exists
		assert!(backend.exists("key1").await.unwrap());
		assert!(!backend.exists("nonexistent").await.unwrap());

		// Delete
		backend.delete("key1").await.unwrap();
		assert!(!backend.exists("key1").await.unwrap());
	}

	#[tokio::test]
	async fn test_ttl_expiration() {
		let backend = MemoryBackend::new();

		// Set with 1 second TTL
		backend
			.set("temp_key", &serde_json::json!("temp_value"), Some(1))
			.await
			.unwrap();

		// Should exist immediately
		assert!(backend.exists("temp_key").await.unwrap());

		// Poll until key expires (1 second TTL)
		poll_until(
			Duration::from_millis(1200),
			Duration::from_millis(50),
			|| async { !backend.exists("temp_key").await.unwrap() },
		)
		.await
		.expect("Key should expire within 1200ms");

		// Should be expired and auto-removed on access
		assert!(!backend.exists("temp_key").await.unwrap());
		assert_eq!(backend.get("temp_key").await.unwrap(), None);
	}

	#[tokio::test]
	async fn test_multiple_values() {
		let backend = MemoryBackend::new();

		// Set multiple values
		backend
			.set("string", &serde_json::json!("text"), None)
			.await
			.unwrap();
		backend
			.set("number", &serde_json::json!(42), None)
			.await
			.unwrap();
		backend
			.set("boolean", &serde_json::json!(true), None)
			.await
			.unwrap();
		backend
			.set("object", &serde_json::json!({"key": "value"}), None)
			.await
			.unwrap();

		// Get all keys
		let keys = backend.keys().await.unwrap();
		assert_eq!(keys.len(), 4);
		assert!(keys.contains(&"string".to_string()));
		assert!(keys.contains(&"number".to_string()));
		assert!(keys.contains(&"boolean".to_string()));
		assert!(keys.contains(&"object".to_string()));

		// Verify values
		assert_eq!(
			backend.get("string").await.unwrap(),
			Some(serde_json::json!("text"))
		);
		assert_eq!(
			backend.get("number").await.unwrap(),
			Some(serde_json::json!(42))
		);
		assert_eq!(
			backend.get("boolean").await.unwrap(),
			Some(serde_json::json!(true))
		);
		assert_eq!(
			backend.get("object").await.unwrap(),
			Some(serde_json::json!({"key": "value"}))
		);
	}

	#[tokio::test]
	async fn test_overwrite_value() {
		let backend = MemoryBackend::new();

		// Set initial value
		backend
			.set("key", &serde_json::json!("value1"), None)
			.await
			.unwrap();
		assert_eq!(
			backend.get("key").await.unwrap(),
			Some(serde_json::json!("value1"))
		);

		// Overwrite with new value
		backend
			.set("key", &serde_json::json!("value2"), None)
			.await
			.unwrap();
		assert_eq!(
			backend.get("key").await.unwrap(),
			Some(serde_json::json!("value2"))
		);

		// Overwrite with TTL
		backend
			.set("key", &serde_json::json!("value3"), Some(60))
			.await
			.unwrap();
		assert_eq!(
			backend.get("key").await.unwrap(),
			Some(serde_json::json!("value3"))
		);
	}

	#[tokio::test]
	async fn test_cleanup_expired() {
		let backend = MemoryBackend::new();

		// Set multiple values with different TTLs
		backend
			.set("permanent", &serde_json::json!("forever"), None)
			.await
			.unwrap();
		backend
			.set("temp1", &serde_json::json!("expires"), Some(1))
			.await
			.unwrap();
		backend
			.set("temp2", &serde_json::json!("expires"), Some(1))
			.await
			.unwrap();

		assert_eq!(backend.len(), 3);

		// Wait for expiration (TTL is 1 second, wait 1100ms to ensure expiration)
		tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

		// Manual cleanup
		backend.cleanup_expired();

		// Only permanent key should remain
		assert_eq!(backend.len(), 1);
		assert!(backend.exists("permanent").await.unwrap());
		assert!(!backend.exists("temp1").await.unwrap());
		assert!(!backend.exists("temp2").await.unwrap());
	}

	#[tokio::test]
	async fn test_keys_filters_expired() {
		let backend = MemoryBackend::new();

		// Set some values
		backend
			.set("active1", &serde_json::json!("value1"), None)
			.await
			.unwrap();
		backend
			.set("active2", &serde_json::json!("value2"), None)
			.await
			.unwrap();
		backend
			.set("expired", &serde_json::json!("value"), Some(1))
			.await
			.unwrap();

		// Wait for one to expire (1 second TTL)
		tokio::time::sleep(Duration::from_millis(1100)).await;

		// keys() should only return non-expired keys
		let keys = backend.keys().await.unwrap();
		assert_eq!(keys.len(), 2);
		assert!(keys.contains(&"active1".to_string()));
		assert!(keys.contains(&"active2".to_string()));
		assert!(!keys.contains(&"expired".to_string()));
	}

	#[tokio::test]
	async fn test_clear() {
		let backend = MemoryBackend::new();

		// Add some data
		backend
			.set("key1", &serde_json::json!("value1"), None)
			.await
			.unwrap();
		backend
			.set("key2", &serde_json::json!("value2"), None)
			.await
			.unwrap();
		backend
			.set("key3", &serde_json::json!("value3"), Some(60))
			.await
			.unwrap();

		assert_eq!(backend.len(), 3);
		assert!(!backend.is_empty());

		// Clear all
		backend.clear();

		assert_eq!(backend.len(), 0);
		assert!(backend.is_empty());
		assert_eq!(backend.keys().await.unwrap().len(), 0);
	}

	#[tokio::test]
	async fn test_concurrent_access() {
		use std::sync::Arc;

		let backend = Arc::new(MemoryBackend::new());

		// Spawn multiple tasks writing to the backend
		let mut handles = vec![];
		for i in 0..10 {
			let backend_clone = backend.clone();
			let handle = tokio::spawn(async move {
				let key = format!("key{}", i);
				let value = serde_json::json!(i);
				backend_clone.set(&key, &value, None).await.unwrap();

				// Read back
				let retrieved = backend_clone.get(&key).await.unwrap();
				assert_eq!(retrieved, Some(value));
			});
			handles.push(handle);
		}

		// Wait for all tasks
		for handle in handles {
			handle.await.unwrap();
		}

		// All keys should exist
		let keys = backend.keys().await.unwrap();
		assert_eq!(keys.len(), 10);
	}

	#[tokio::test]
	async fn test_default_implementation() {
		let backend = MemoryBackend::default();
		assert!(backend.is_empty());

		backend
			.set("test", &serde_json::json!("value"), None)
			.await
			.unwrap();
		assert_eq!(backend.len(), 1);
	}
}
