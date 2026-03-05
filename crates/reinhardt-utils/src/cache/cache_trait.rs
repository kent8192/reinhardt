//! Base cache trait definition

use async_trait::async_trait;
use reinhardt_core::exception::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Base cache trait
#[async_trait]
pub trait Cache: Send + Sync {
	/// Get a value from the cache
	async fn get<T>(&self, key: &str) -> Result<Option<T>>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync;

	/// Set a value in the cache with optional TTL
	async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync;

	/// Delete a value from the cache
	async fn delete(&self, key: &str) -> Result<()>;

	/// Check if a key exists in the cache
	async fn has_key(&self, key: &str) -> Result<bool>;

	/// Clear all values from the cache
	async fn clear(&self) -> Result<()>;

	/// Get multiple values at once
	async fn get_many<T>(&self, keys: &[&str]) -> Result<HashMap<String, T>>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		let mut results = HashMap::new();
		for key in keys {
			if let Some(value) = self.get::<T>(key).await? {
				results.insert(key.to_string(), value);
			}
		}
		Ok(results)
	}

	/// Set multiple values at once
	async fn set_many<T>(&self, values: HashMap<String, T>, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		for (key, value) in values.iter() {
			self.set(key, value, ttl).await?;
		}
		Ok(())
	}

	/// Delete multiple keys at once
	async fn delete_many(&self, keys: &[&str]) -> Result<()> {
		for key in keys {
			self.delete(key).await?;
		}
		Ok(())
	}

	/// Increment a numeric value
	///
	/// # Warning
	///
	/// Default implementation is not atomic. The get-modify-set sequence is
	/// subject to race conditions under concurrent access. Override in backends
	/// that support atomic operations (e.g., Redis INCRBY).
	async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
		// WARNING: Default implementation is not atomic.
		// Override in backends that support atomic operations.
		let current: i64 = self.get(key).await?.unwrap_or(0);
		let new_value = current + delta;
		self.set(key, &new_value, None).await?;
		Ok(new_value)
	}

	/// Decrement a numeric value
	///
	/// # Warning
	///
	/// Default implementation is not atomic. See [`Cache::incr`] for details.
	async fn decr(&self, key: &str, delta: i64) -> Result<i64> {
		self.incr(key, -delta).await
	}
}
