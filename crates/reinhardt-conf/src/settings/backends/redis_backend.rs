//! Redis backend for dynamic settings
//!
//! This backend provides runtime configuration storage using Redis.
//!
//! ## Features
//!
//! This module is only available when the `dynamic-redis` feature is enabled.
//!
//! ## Example
//!
//! ```rust,no_run
//! # #[cfg(feature = "dynamic-redis")]
//! # async fn example() -> Result<(), String> {
//! use reinhardt_conf::settings::backends::RedisSettingsBackend;
//! use serde_json::json;
//!
//! // Create backend
//! let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
//!
//! // Set a value with TTL
//! let value = json!({"debug": true, "port": 8080});
//! backend.set("app_config", &value.to_string(), Some(3600)).await?;
//!
//! // Get the value
//! let retrieved = backend.get("app_config").await?;
//! assert!(retrieved.is_some());
//!
//! // Delete the value
//! backend.delete("app_config").await?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "dynamic-redis")]
use redis::aio::ConnectionManager;
#[cfg(feature = "dynamic-redis")]
use redis::{AsyncCommands, Client};

#[cfg(feature = "dynamic-redis")]
use crate::settings::dynamic::{DynamicBackend, DynamicError, DynamicResult};
#[cfg(feature = "dynamic-redis")]
use async_trait::async_trait;

/// Redis backend for runtime configuration changes
///
/// This backend allows dynamic settings to be stored in and retrieved from Redis,
/// enabling runtime configuration changes without application restarts.
///
/// Settings are stored with a key prefix `settings:` to avoid conflicts with other Redis data.
///
/// ## Example
///
/// ```rust,no_run
/// # #[cfg(feature = "dynamic-redis")]
/// # async fn example() -> Result<(), String> {
/// use reinhardt_conf::settings::backends::RedisSettingsBackend;
///
/// let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
///
/// // Store configuration
/// backend.set("feature_flags", r#"{"new_ui": true}"#, None).await?;
///
/// // Retrieve configuration
/// let config = backend.get("feature_flags").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct RedisSettingsBackend {
	#[cfg(feature = "dynamic-redis")]
	conn: ConnectionManager,
	#[cfg(not(feature = "dynamic-redis"))]
	_phantom: std::marker::PhantomData<()>,
}

impl RedisSettingsBackend {
	/// Create a new Redis backend
	///
	/// Initializes a connection manager to the specified Redis URL.
	///
	/// ## Arguments
	///
	/// * `url` - Redis connection URL (e.g., "redis://localhost:6379")
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-redis")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::RedisSettingsBackend;
	///
	/// let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-redis")]
	pub async fn new(url: &str) -> Result<Self, String> {
		let client =
			Client::open(url).map_err(|e| format!("Failed to create Redis client: {}", e))?;

		let conn = ConnectionManager::new(client)
			.await
			.map_err(|e| format!("Failed to connect to Redis: {}", e))?;

		Ok(Self { conn })
	}

	#[cfg(not(feature = "dynamic-redis"))]
	pub async fn new(_url: &str) -> Result<Self, String> {
		Err("Redis backend not enabled. Enable the 'dynamic-redis' feature.".to_string())
	}

	/// Get the prefixed key for Redis storage
	///
	/// All settings are stored with the prefix "settings:" to avoid conflicts.
	#[cfg(feature = "dynamic-redis")]
	fn prefixed_key(&self, key: &str) -> String {
		format!("settings:{}", key)
	}

	/// Get a setting value by key
	///
	/// Returns `None` if the key doesn't exist or has expired.
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-redis")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::RedisSettingsBackend;
	///
	/// let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
	///
	/// backend.set("key", "value", None).await?;
	/// let value = backend.get("key").await?;
	/// assert_eq!(value, Some("value".to_string()));
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-redis")]
	pub async fn get(&self, key: &str) -> Result<Option<String>, String> {
		let mut conn = self.conn.clone();
		let prefixed_key = self.prefixed_key(key);

		conn.get(&prefixed_key)
			.await
			.map_err(|e| format!("Failed to get setting: {}", e))
	}

	/// Set a setting value with optional TTL
	///
	/// ## Arguments
	///
	/// * `key` - Setting key
	/// * `value` - Setting value as a string
	/// * `ttl` - Optional time-to-live in seconds
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-redis")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::RedisSettingsBackend;
	///
	/// let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
	///
	/// // Set with 1 hour TTL
	/// backend.set("temp_config", r#"{"enabled": true}"#, Some(3600)).await?;
	///
	/// // Set without TTL (permanent)
	/// backend.set("permanent_config", r#"{"version": "1.0"}"#, None).await?;
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-redis")]
	pub async fn set(&self, key: &str, value: &str, ttl: Option<u64>) -> Result<(), String> {
		let mut conn = self.conn.clone();
		let prefixed_key = self.prefixed_key(key);

		if let Some(seconds) = ttl {
			let _: () = conn
				.set_ex(&prefixed_key, value, seconds)
				.await
				.map_err(|e| format!("Failed to set setting with TTL: {}", e))?;
		} else {
			let _: () = conn
				.set(&prefixed_key, value)
				.await
				.map_err(|e| format!("Failed to set setting: {}", e))?;
		}

		Ok(())
	}

	/// Delete a setting by key
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-redis")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::RedisSettingsBackend;
	///
	/// let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
	///
	/// backend.set("key", "value", None).await?;
	/// backend.delete("key").await?;
	/// assert!(!backend.exists("key").await?);
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-redis")]
	pub async fn delete(&self, key: &str) -> Result<(), String> {
		let mut conn = self.conn.clone();
		let prefixed_key = self.prefixed_key(key);

		let _: () = conn
			.del(&prefixed_key)
			.await
			.map_err(|e| format!("Failed to delete setting: {}", e))?;

		Ok(())
	}

	/// Check if a setting exists
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-redis")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::RedisSettingsBackend;
	///
	/// let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
	///
	/// assert!(!backend.exists("nonexistent").await?);
	///
	/// backend.set("key", "value", None).await?;
	/// assert!(backend.exists("key").await?);
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-redis")]
	pub async fn exists(&self, key: &str) -> Result<bool, String> {
		let mut conn = self.conn.clone();
		let prefixed_key = self.prefixed_key(key);

		conn.exists(&prefixed_key)
			.await
			.map_err(|e| format!("Failed to check setting existence: {}", e))
	}

	/// Get all setting keys
	///
	/// Returns all keys matching the settings prefix.
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # #[cfg(feature = "dynamic-redis")]
	/// # async fn example() -> Result<(), String> {
	/// use reinhardt_conf::settings::backends::RedisSettingsBackend;
	///
	/// let backend = RedisSettingsBackend::new("redis://localhost:6379").await?;
	///
	/// backend.set("key1", "value1", None).await?;
	/// backend.set("key2", "value2", None).await?;
	///
	/// let keys = backend.keys().await?;
	/// assert!(keys.contains(&"key1".to_string()));
	/// assert!(keys.contains(&"key2".to_string()));
	/// # Ok(())
	/// # }
	/// ```
	#[cfg(feature = "dynamic-redis")]
	pub async fn keys(&self) -> Result<Vec<String>, String> {
		let mut conn = self.conn.clone();
		let pattern = "settings:*";

		let prefixed_keys: Vec<String> = conn
			.keys(pattern)
			.await
			.map_err(|e| format!("Failed to get keys: {}", e))?;

		// Remove the prefix from the keys
		let keys = prefixed_keys
			.into_iter()
			.filter_map(|k| k.strip_prefix("settings:").map(|s| s.to_string()))
			.collect();

		Ok(keys)
	}
}

// DynamicBackend trait implementation
#[cfg(feature = "dynamic-redis")]
#[async_trait]
impl DynamicBackend for RedisSettingsBackend {
	async fn get(&self, key: &str) -> DynamicResult<Option<serde_json::Value>> {
		// Call existing method and convert result
		let result = RedisSettingsBackend::get(self, key)
			.await
			.map_err(DynamicError::Backend)?;

		// Convert String to serde_json::Value if present
		match result {
			Some(s) => {
				let value = serde_json::from_str(&s).map_err(DynamicError::from)?;
				Ok(Some(value))
			}
			None => Ok(None),
		}
	}

	async fn set(
		&self,
		key: &str,
		value: &serde_json::Value,
		ttl: Option<u64>,
	) -> DynamicResult<()> {
		// Convert serde_json::Value to String
		let value_str = serde_json::to_string(value).map_err(DynamicError::from)?;

		// Call existing method
		RedisSettingsBackend::set(self, key, &value_str, ttl)
			.await
			.map_err(DynamicError::Backend)
	}

	async fn delete(&self, key: &str) -> DynamicResult<()> {
		RedisSettingsBackend::delete(self, key)
			.await
			.map_err(DynamicError::Backend)
	}

	async fn exists(&self, key: &str) -> DynamicResult<bool> {
		RedisSettingsBackend::exists(self, key)
			.await
			.map_err(DynamicError::Backend)
	}

	async fn keys(&self) -> DynamicResult<Vec<String>> {
		RedisSettingsBackend::keys(self)
			.await
			.map_err(DynamicError::Backend)
	}
}

#[cfg(all(test, not(feature = "dynamic-redis")))]
mod tests_no_feature {
	use super::*;

	#[tokio::test]
	async fn test_redis_backend_disabled() {
		let result = RedisSettingsBackend::new("redis://localhost:6379").await;
		assert!(result.is_err());
		assert!(result.unwrap_err().contains("Redis backend not enabled"));
	}
}
