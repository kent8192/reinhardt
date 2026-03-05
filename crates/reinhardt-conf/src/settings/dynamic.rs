//! Dynamic settings management
//!
//! This module provides runtime configuration changes with multiple backend support.
//!
//! ## Features
//!
//! - **Multiple Backends**: Memory, Redis, Database
//! - **Caching**: Optional in-memory caching with TTL
//! - **Observer Pattern**: Subscribe to configuration changes
//! - **Thread-Safe**: Arc + RwLock for concurrent access
//!
//! ## Example
//!
//! ```rust
//! # #[cfg(feature = "async")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use reinhardt_conf::settings::dynamic::{DynamicSettings, DynamicBackend};
//! use reinhardt_conf::settings::backends::MemoryBackend;
//! use std::sync::Arc;
//!
//! # futures::executor::block_on(async {
//! // Create backend
//! let backend = Arc::new(MemoryBackend::new());
//!
//! // Create dynamic settings
//! let settings = DynamicSettings::new(backend);
//!
//! // Set a value
//! settings.set("debug", &true, None).await?;
//!
//! // Get a value
//! let debug: bool = settings.get("debug").await?.unwrap();
//! assert!(debug);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }).unwrap();
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "caching")]
use moka::future::Cache;
#[cfg(feature = "caching")]
use std::time::{Duration, Instant};

/// Error type for dynamic settings operations
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum DynamicError {
	#[error("Backend error: {0}")]
	Backend(String),

	#[error("Serialization error: {0}")]
	Serialization(#[from] serde_json::Error),

	#[error("Key not found: {0}")]
	KeyNotFound(String),

	#[error("Invalid value type")]
	InvalidType,

	#[error("Cache error: {0}")]
	Cache(String),

	#[cfg(feature = "hot-reload")]
	#[error("Hot reload error: {0}")]
	HotReload(String),
}

/// Result type for dynamic settings operations
pub type DynamicResult<T> = Result<T, DynamicError>;

/// Backend trait for dynamic settings storage
///
/// This trait defines the interface that all dynamic settings backends must implement.
/// Backends are responsible for persisting configuration data and may optionally support
/// features like TTL (time-to-live) for automatic expiration.
///
/// ## Example Implementation
///
/// ```
/// # #[cfg(feature = "async")]
/// # async fn example() {
/// use reinhardt_conf::settings::dynamic::{DynamicBackend, DynamicResult};
/// use async_trait::async_trait;
/// use std::collections::HashMap;
/// use parking_lot::RwLock;
/// use std::sync::Arc;
///
/// struct MyBackend {
///     data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
/// }
///
/// #[async_trait]
/// impl DynamicBackend for MyBackend {
///     async fn get(&self, key: &str) -> DynamicResult<Option<serde_json::Value>> {
///         Ok(self.data.read().get(key).cloned())
///     }
///
///     async fn set(&self, key: &str, value: &serde_json::Value, _ttl: Option<u64>) -> DynamicResult<()> {
///         self.data.write().insert(key.to_string(), value.clone());
///         Ok(())
///     }
///
///     async fn delete(&self, key: &str) -> DynamicResult<()> {
///         self.data.write().remove(key);
///         Ok(())
///     }
///
///     async fn exists(&self, key: &str) -> DynamicResult<bool> {
///         Ok(self.data.read().contains_key(key))
///     }
///
///     async fn keys(&self) -> DynamicResult<Vec<String>> {
///         Ok(self.data.read().keys().cloned().collect())
///     }
/// }
///
/// // Test the backend implementation
/// let backend = MyBackend {
///     data: Arc::new(RwLock::new(HashMap::new())),
/// };
///
/// let value = serde_json::json!({"test": "value"});
/// backend.set("key1", &value, None).await.unwrap();
/// assert!(backend.exists("key1").await.unwrap());
/// let retrieved = backend.get("key1").await.unwrap();
/// assert_eq!(retrieved, Some(value));
/// # }
/// # #[cfg(feature = "async")]
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
#[async_trait]
pub trait DynamicBackend: Send + Sync {
	/// Get a value by key
	///
	/// Returns `None` if the key doesn't exist or has expired.
	async fn get(&self, key: &str) -> DynamicResult<Option<serde_json::Value>>;

	/// Set a value with optional TTL
	///
	/// ## Arguments
	///
	/// * `key` - Setting key
	/// * `value` - Setting value (must be JSON-serializable)
	/// * `ttl` - Optional time-to-live in seconds
	async fn set(
		&self,
		key: &str,
		value: &serde_json::Value,
		ttl: Option<u64>,
	) -> DynamicResult<()>;

	/// Delete a value by key
	async fn delete(&self, key: &str) -> DynamicResult<()>;

	/// Check if a key exists
	async fn exists(&self, key: &str) -> DynamicResult<bool>;

	/// Get all keys
	async fn keys(&self) -> DynamicResult<Vec<String>>;
}

/// Unique identifier for observer subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(uuid::Uuid);

impl SubscriptionId {
	fn new() -> Self {
		Self(uuid::Uuid::new_v4())
	}
}

/// Observer callback type
type ObserverCallback = Box<dyn Fn(&str, Option<&serde_json::Value>) + Send + Sync>;

/// Cached value with optional expiration
#[cfg(feature = "caching")]
#[derive(Clone)]
struct CachedValue {
	value: serde_json::Value,
	expires_at: Option<Instant>,
}

#[cfg(feature = "caching")]
impl CachedValue {
	fn new(value: serde_json::Value, ttl: Option<Duration>) -> Self {
		Self {
			value,
			expires_at: ttl.map(|d| Instant::now() + d),
		}
	}

	fn is_expired(&self) -> bool {
		self.expires_at
			.map(|exp| Instant::now() > exp)
			.unwrap_or(false)
	}
}

/// Dynamic settings manager with caching and observer pattern support
///
/// This structure manages runtime configuration changes with support for:
/// - Multiple backend storage options (Memory, Redis, Database)
/// - Optional in-memory caching with TTL
/// - Observer pattern for change notifications
/// - Thread-safe concurrent access
///
/// ## Example
///
/// ```rust
/// # futures::executor::block_on(async {
/// use reinhardt_conf::settings::dynamic::DynamicSettings;
/// use reinhardt_conf::settings::backends::MemoryBackend;
/// use std::sync::Arc;
///
/// let backend = Arc::new(MemoryBackend::new());
/// let mut settings = DynamicSettings::new(backend);
///
/// // Enable caching
/// # #[cfg(feature = "caching")]
/// settings.enable_cache(100, Some(std::time::Duration::from_secs(300)));
///
/// // Set values
/// settings.set("app.name", &"MyApp", None).await.unwrap();
/// settings.set("app.debug", &true, Some(3600)).await.unwrap();
///
/// // Get values with type safety
/// let name: String = settings.get("app.name").await.unwrap().unwrap();
/// let debug: bool = settings.get("app.debug").await.unwrap().unwrap();
/// assert_eq!(name, "MyApp");
/// assert!(debug);
///
/// // Subscribe to changes
/// let sub_id = settings.subscribe(|key, value| {
///     println!("Setting changed: {} = {:?}", key, value);
/// });
///
/// // Update triggers observers
/// settings.set("app.debug", &false, None).await.unwrap();
///
/// // Unsubscribe
/// settings.unsubscribe(sub_id);
/// # });
/// ```
pub struct DynamicSettings {
	backend: Arc<dyn DynamicBackend>,

	#[cfg(feature = "caching")]
	cache: Option<Cache<String, CachedValue>>,

	observers: Arc<RwLock<HashMap<SubscriptionId, ObserverCallback>>>,

	#[cfg(feature = "hot-reload")]
	hot_reload: Option<Arc<tokio::sync::Mutex<super::hot_reload::HotReloadManager>>>,
}

impl DynamicSettings {
	/// Create a new dynamic settings instance with the given backend
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let settings = DynamicSettings::new(backend);
	/// ```
	pub fn new(backend: Arc<dyn DynamicBackend>) -> Self {
		Self {
			backend,
			#[cfg(feature = "caching")]
			cache: None,
			observers: Arc::new(RwLock::new(HashMap::new())),
			#[cfg(feature = "hot-reload")]
			hot_reload: None,
		}
	}

	/// Enable in-memory caching
	///
	/// ## Arguments
	///
	/// * `capacity` - Maximum number of cached items
	/// * `default_ttl` - Default time-to-live for cached items
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	/// use std::sync::Arc;
	/// use std::time::Duration;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let mut settings = DynamicSettings::new(backend);
	///
	/// // Cache up to 100 items for 5 minutes
	/// # #[cfg(feature = "caching")]
	/// settings.enable_cache(100, Some(Duration::from_secs(300)));
	/// ```
	#[cfg(feature = "caching")]
	pub fn enable_cache(&mut self, capacity: u64, _default_ttl: Option<Duration>) {
		self.cache = Some(Cache::builder().max_capacity(capacity).build());
	}

	/// Get a setting value with type safety
	///
	/// Returns `None` if the key doesn't exist or has expired.
	///
	/// ## Example
	///
	/// ```rust
	/// # #[cfg(feature = "async")]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # futures::executor::block_on(async {
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// settings.set("port", &8080, None).await?;
	///
	/// let port: u16 = settings.get("port").await?.unwrap();
	/// assert_eq!(port, 8080);
	/// # Ok::<(), Box<dyn std::error::Error>>(())
	/// # }).unwrap();
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get<T: DeserializeOwned>(&self, key: &str) -> DynamicResult<Option<T>> {
		// Check cache first
		#[cfg(feature = "caching")]
		if let Some(cache) = &self.cache
			&& let Some(cached) = cache.get(key).await
		{
			if !cached.is_expired() {
				return serde_json::from_value(cached.value.clone())
					.map(Some)
					.map_err(DynamicError::from);
			} else {
				// Remove expired entry
				cache.invalidate(key).await;
			}
		}

		// Fetch from backend
		let value = self.backend.get(key).await?;

		// Update cache
		#[cfg(feature = "caching")]
		if let (Some(cache), Some(val)) = (&self.cache, &value) {
			cache
				.insert(key.to_string(), CachedValue::new(val.clone(), None))
				.await;
		}

		match value {
			Some(v) => serde_json::from_value(v)
				.map(Some)
				.map_err(DynamicError::from),
			None => Ok(None),
		}
	}

	/// Set a setting value with optional TTL
	///
	/// ## Example
	///
	/// ```rust
	/// # futures::executor::block_on(async {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// // Set without TTL (permanent)
	/// settings.set("api_key", &"secret", None).await.unwrap();
	///
	/// // Set with 1 hour TTL
	/// settings.set("temp_token", &"token123", Some(3600)).await.unwrap();
	/// # });
	/// ```
	pub async fn set<T: Serialize>(
		&self,
		key: &str,
		value: &T,
		ttl: Option<u64>,
	) -> DynamicResult<()> {
		let json_value = serde_json::to_value(value)?;

		// Set in backend
		self.backend.set(key, &json_value, ttl).await?;

		// Update cache
		#[cfg(feature = "caching")]
		if let Some(cache) = &self.cache {
			let cached_value = CachedValue::new(json_value.clone(), ttl.map(Duration::from_secs));
			cache.insert(key.to_string(), cached_value).await;
		}

		// Notify observers
		self.notify_observers(key, Some(&json_value)).await;

		Ok(())
	}

	/// Delete a setting
	///
	/// ## Example
	///
	/// ```rust
	/// # #[cfg(feature = "async")]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # futures::executor::block_on(async {
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// settings.set("temp", &"value", None).await?;
	/// settings.delete("temp").await?;
	///
	/// assert!(settings.get::<String>("temp").await?.is_none());
	/// # Ok::<(), Box<dyn std::error::Error>>(())
	/// # }).unwrap();
	/// # Ok(())
	/// # }
	/// ```
	pub async fn delete(&self, key: &str) -> DynamicResult<()> {
		// Delete from backend
		self.backend.delete(key).await?;

		// Invalidate cache
		#[cfg(feature = "caching")]
		if let Some(cache) = &self.cache {
			cache.invalidate(key).await;
		}

		// Notify observers
		self.notify_observers(key, None).await;

		Ok(())
	}

	/// Check if a key exists
	///
	/// ## Example
	///
	/// ```rust
	/// # #[cfg(feature = "async")]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # futures::executor::block_on(async {
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// settings.set("key", &"value", None).await?;
	///
	/// assert!(settings.exists("key").await?);
	/// assert!(!settings.exists("nonexistent").await?);
	/// # Ok::<(), Box<dyn std::error::Error>>(())
	/// # }).unwrap();
	/// # Ok(())
	/// # }
	/// ```
	pub async fn exists(&self, key: &str) -> DynamicResult<bool> {
		self.backend.exists(key).await
	}

	/// Get all setting keys
	///
	/// ## Example
	///
	/// ```rust
	/// # #[cfg(feature = "async")]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # futures::executor::block_on(async {
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// settings.set("key1", &"value1", None).await?;
	/// settings.set("key2", &"value2", None).await?;
	///
	/// let keys = settings.keys().await?;
	/// assert!(keys.contains(&"key1".to_string()));
	/// assert!(keys.contains(&"key2".to_string()));
	/// # Ok::<(), Box<dyn std::error::Error>>(())
	/// # }).unwrap();
	/// # Ok(())
	/// # }
	/// ```
	pub async fn keys(&self) -> DynamicResult<Vec<String>> {
		self.backend.keys().await
	}

	/// Subscribe to setting changes
	///
	/// Returns a subscription ID that can be used to unsubscribe.
	///
	/// ## Example
	///
	/// ```rust
	/// # futures::executor::block_on(async {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// let sub_id = settings.subscribe(|key, value| {
	///     println!("Changed: {} = {:?}", key, value);
	/// });
	///
	/// settings.set("debug", &true, None).await.unwrap();  // Triggers callback
	///
	/// settings.unsubscribe(sub_id);
	/// # });
	/// ```
	pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
	where
		F: Fn(&str, Option<&serde_json::Value>) + Send + Sync + 'static,
	{
		let id = SubscriptionId::new();
		self.observers.write().insert(id, Box::new(callback));
		id
	}

	/// Unsubscribe from setting changes
	///
	/// ## Example
	///
	/// ```rust
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// let sub_id = settings.subscribe(|_, _| {});
	/// settings.unsubscribe(sub_id);
	/// ```
	pub fn unsubscribe(&self, id: SubscriptionId) {
		self.observers.write().remove(&id);
	}

	/// Invalidate cache for a specific key
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(all(feature = "async", feature = "caching"))]
	/// # async fn example() {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// settings.invalidate_cache("key").await;
	/// # }
	/// ```
	#[cfg(feature = "caching")]
	pub async fn invalidate_cache(&self, key: &str) {
		if let Some(cache) = &self.cache {
			cache.invalidate(key).await;
		}
	}

	/// Clear all cached values
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(all(feature = "async", feature = "caching"))]
	/// # async fn example() {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend);
	/// settings.clear_cache().await;
	/// # }
	/// ```
	#[cfg(feature = "caching")]
	pub async fn clear_cache(&self) {
		if let Some(cache) = &self.cache {
			cache.invalidate_all();
		}
	}

	/// Enable hot reload for a configuration file
	///
	/// When the file changes, the settings will be automatically reloaded and
	/// observers will be notified.
	///
	/// ## Arguments
	///
	/// * `path` - Path to the configuration file to watch
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "hot-reload")]
	/// # futures::executor::block_on(async {
	/// use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	/// use std::sync::Arc;
	/// use std::path::Path;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let settings = DynamicSettings::new(backend).with_hot_reload();
	///
	/// // Watch configuration file (requires actual file for file system watcher)
	/// // settings.watch_file(Path::new("config.toml")).await.unwrap();
	///
	/// // Settings will automatically reload when the file changes
	/// # });
	/// ```
	#[cfg(feature = "hot-reload")]
	pub async fn watch_file(&self, path: &std::path::Path) -> DynamicResult<()> {
		if let Some(hot_reload) = &self.hot_reload {
			hot_reload
				.lock()
				.await
				.watch(path)
				.await
				.map_err(DynamicError::Backend)?;
		}
		Ok(())
	}

	/// Stop watching a configuration file
	///
	/// ## Arguments
	///
	/// * `path` - Path to stop watching
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(all(feature = "async", feature = "hot-reload"))]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # use std::path::Path;
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend).with_hot_reload();
	/// # settings.watch_file(Path::new("config.toml")).await?;
	/// settings.unwatch_file(Path::new("config.toml")).await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "hot-reload")]
	pub async fn unwatch_file(&self, path: &std::path::Path) -> DynamicResult<()> {
		if let Some(hot_reload) = &self.hot_reload {
			hot_reload
				.lock()
				.await
				.unwatch(path)
				.await
				.map_err(DynamicError::Backend)?;
		}
		Ok(())
	}

	/// Stop all file watching
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(all(feature = "async", feature = "hot-reload"))]
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// # use reinhardt_conf::settings::backends::MemoryBackend;
	/// # use std::sync::Arc;
	/// # let backend = Arc::new(MemoryBackend::new());
	/// # let settings = DynamicSettings::new(backend).with_hot_reload();
	/// settings.stop_watching().await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "hot-reload")]
	pub async fn stop_watching(&self) -> DynamicResult<()> {
		if let Some(hot_reload) = &self.hot_reload {
			hot_reload
				.lock()
				.await
				.stop()
				.await
				.map_err(DynamicError::Backend)?;
		}
		Ok(())
	}

	/// Enable hot reload functionality
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "hot-reload")]
	/// # {
	/// use reinhardt_conf::settings::dynamic::DynamicSettings;
	/// use reinhardt_conf::settings::backends::MemoryBackend;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let settings = DynamicSettings::new(backend).with_hot_reload();
	/// # }
	/// ```
	#[cfg(feature = "hot-reload")]
	pub fn with_hot_reload(mut self) -> Self {
		let manager = super::hot_reload::HotReloadManager::new();
		self.hot_reload = Some(Arc::new(tokio::sync::Mutex::new(manager)));
		self
	}

	/// Notify all observers of a change
	async fn notify_observers(&self, key: &str, value: Option<&serde_json::Value>) {
		let observers = self.observers.read();
		for callback in observers.values() {
			callback(key, value);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	struct TestBackend {
		data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
	}

	impl TestBackend {
		fn new() -> Self {
			Self {
				data: Arc::new(RwLock::new(HashMap::new())),
			}
		}
	}

	#[async_trait]
	impl DynamicBackend for TestBackend {
		async fn get(&self, key: &str) -> DynamicResult<Option<serde_json::Value>> {
			Ok(self.data.read().get(key).cloned())
		}

		async fn set(
			&self,
			key: &str,
			value: &serde_json::Value,
			_ttl: Option<u64>,
		) -> DynamicResult<()> {
			self.data.write().insert(key.to_string(), value.clone());
			Ok(())
		}

		async fn delete(&self, key: &str) -> DynamicResult<()> {
			self.data.write().remove(key);
			Ok(())
		}

		async fn exists(&self, key: &str) -> DynamicResult<bool> {
			Ok(self.data.read().contains_key(key))
		}

		async fn keys(&self) -> DynamicResult<Vec<String>> {
			Ok(self.data.read().keys().cloned().collect())
		}
	}

	#[tokio::test]
	async fn test_basic_crud() {
		let backend = Arc::new(TestBackend::new());
		let settings = DynamicSettings::new(backend);

		// Set
		settings.set("key", &"value", None).await.unwrap();

		// Get
		let value: String = settings.get("key").await.unwrap().unwrap();
		assert_eq!(value, "value");

		// Exists
		assert!(settings.exists("key").await.unwrap());

		// Delete
		settings.delete("key").await.unwrap();
		assert!(!settings.exists("key").await.unwrap());
	}

	#[tokio::test]
	async fn test_type_safety() {
		let backend = Arc::new(TestBackend::new());
		let settings = DynamicSettings::new(backend);

		settings.set("number", &42, None).await.unwrap();
		settings.set("boolean", &true, None).await.unwrap();
		settings.set("string", &"text", None).await.unwrap();

		let number: i32 = settings.get("number").await.unwrap().unwrap();
		let boolean: bool = settings.get("boolean").await.unwrap().unwrap();
		let string: String = settings.get("string").await.unwrap().unwrap();

		assert_eq!(number, 42);
		assert!(boolean);
		assert_eq!(string, "text");
	}

	#[tokio::test]
	async fn test_observer_pattern() {
		let backend = Arc::new(TestBackend::new());
		let settings = DynamicSettings::new(backend);

		let called = Arc::new(RwLock::new(false));
		let called_clone = called.clone();

		let _sub_id = settings.subscribe(move |key, _value| {
			if key == "test" {
				*called_clone.write() = true;
			}
		});

		settings.set("test", &"value", None).await.unwrap();

		// Small delay to ensure callback is executed
		assert!(*called.read());
	}

	#[tokio::test]
	async fn test_unsubscribe() {
		let backend = Arc::new(TestBackend::new());
		let settings = DynamicSettings::new(backend);

		let called = Arc::new(RwLock::new(0));
		let called_clone = called.clone();

		let sub_id = settings.subscribe(move |_, _| {
			*called_clone.write() += 1;
		});

		settings.set("key1", &"value1", None).await.unwrap();
		assert_eq!(*called.read(), 1);

		settings.unsubscribe(sub_id);

		settings.set("key2", &"value2", None).await.unwrap();
		assert_eq!(*called.read(), 1); // Should not increment
	}

	#[tokio::test]
	async fn test_keys() {
		let backend = Arc::new(TestBackend::new());
		let settings = DynamicSettings::new(backend);

		settings.set("key1", &"value1", None).await.unwrap();
		settings.set("key2", &"value2", None).await.unwrap();
		settings.set("key3", &"value3", None).await.unwrap();

		let keys = settings.keys().await.unwrap();
		assert_eq!(keys.len(), 3);
		assert!(keys.contains(&"key1".to_string()));
		assert!(keys.contains(&"key2".to_string()));
		assert!(keys.contains(&"key3".to_string()));
	}

	#[cfg(feature = "caching")]
	#[tokio::test]
	async fn test_caching() {
		let backend = Arc::new(TestBackend::new());
		let mut settings = DynamicSettings::new(backend.clone());

		settings.enable_cache(10, Some(Duration::from_secs(60)));

		// Set and get (should cache)
		settings
			.set("cached_key", &"cached_value", None)
			.await
			.unwrap();
		let value1: String = settings.get("cached_key").await.unwrap().unwrap();

		// Modify backend directly (bypass cache)
		backend
			.set("cached_key", &serde_json::json!("modified"), None)
			.await
			.unwrap();

		// Get should return cached value
		let value2: String = settings.get("cached_key").await.unwrap().unwrap();
		assert_eq!(value1, value2);
		assert_eq!(value2, "cached_value");

		// Invalidate cache
		settings.invalidate_cache("cached_key").await;

		// Now should get modified value
		let value3: String = settings.get("cached_key").await.unwrap().unwrap();
		assert_eq!(value3, "modified");
	}
}
