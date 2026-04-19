//! Redis Sentinel support for high availability.
//!
//! Redis Sentinel provides automatic failover for Redis masters.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt_utils::cache::{Cache, RedisSentinelCache, RedisSentinelConfig};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RedisSentinelConfig {
//!         sentinels: vec![
//!             "redis://127.0.0.1:26379".to_string(),
//!             "redis://127.0.0.1:26380".to_string(),
//!             "redis://127.0.0.1:26381".to_string(),
//!         ],
//!         master_name: "mymaster".to_string(),
//!         password: None,
//!         db: 0,
//!     };
//!
//!     let cache = RedisSentinelCache::new(config).await?;
//!
//!     cache.set("key", &"value", Some(Duration::from_secs(3600))).await?;
//!     Ok(())
//! }
//! ```

use super::cache_trait::Cache;
use async_trait::async_trait;
use redis::{AsyncCommands, sentinel::Sentinel};
use reinhardt_core::exception::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Redis Sentinel configuration.
#[derive(Debug, Clone)]
pub struct RedisSentinelConfig {
	/// Sentinel server URLs
	pub sentinels: Vec<String>,
	/// Master name configured in Sentinel
	pub master_name: String,
	/// Password for Redis (if required)
	pub password: Option<String>,
	/// Database number
	pub db: u8,
}

/// Redis Sentinel cache backend with automatic failover.
#[derive(Clone)]
pub struct RedisSentinelCache {
	sentinel: Arc<RwLock<Sentinel>>,
	master_name: String,
}

impl RedisSentinelCache {
	/// Create a new Redis Sentinel cache instance.
	pub async fn new(config: RedisSentinelConfig) -> Result<Self> {
		let sentinel_urls: Vec<String> = config.sentinels.clone();

		let sentinel = Sentinel::build(sentinel_urls)
			.map_err(|e| Error::Http(format!("Failed to build Sentinel: {}", e)))?;

		Ok(Self {
			sentinel: Arc::new(RwLock::new(sentinel)),
			master_name: config.master_name,
		})
	}

	/// Get a connection to the current master.
	async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection> {
		let mut sentinel = self.sentinel.write().await;

		let client = sentinel
			.master_for(&self.master_name, None)
			.map_err(|e| Error::Http(format!("Failed to get master client: {}", e)))?;

		client
			.get_multiplexed_async_connection()
			.await
			.map_err(|e| Error::Http(format!("Failed to connect to master: {}", e)))
	}
}

#[async_trait]
impl Cache for RedisSentinelCache {
	async fn get<T>(&self, key: &str) -> Result<Option<T>>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		let mut conn = self.get_connection().await?;

		let value: Option<Vec<u8>> = conn
			.get(key)
			.await
			.map_err(|e| Error::Http(format!("Failed to get value from Redis: {}", e)))?;

		match value {
			Some(bytes) => {
				let deserialized = serde_json::from_slice(&bytes)
					.map_err(|e| Error::Serialization(e.to_string()))?;
				Ok(Some(deserialized))
			}
			None => Ok(None),
		}
	}

	async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		let mut conn = self.get_connection().await?;
		let serialized =
			serde_json::to_vec(value).map_err(|e| Error::Serialization(e.to_string()))?;

		if let Some(ttl_duration) = ttl {
			let seconds = ttl_duration.as_secs();
			let _: () = conn
				.set_ex(key, serialized, seconds)
				.await
				.map_err(|e| Error::Http(format!("Failed to set value in Redis: {}", e)))?;
		} else {
			let _: () = conn
				.set(key, serialized)
				.await
				.map_err(|e| Error::Http(format!("Failed to set value in Redis: {}", e)))?;
		}

		Ok(())
	}

	async fn delete(&self, key: &str) -> Result<()> {
		let mut conn = self.get_connection().await?;
		let _: () = conn
			.del(key)
			.await
			.map_err(|e| Error::Http(format!("Failed to delete value from Redis: {}", e)))?;
		Ok(())
	}

	async fn has_key(&self, key: &str) -> Result<bool> {
		let mut conn = self.get_connection().await?;
		let exists: bool = conn
			.exists(key)
			.await
			.map_err(|e| Error::Http(format!("Failed to check key existence in Redis: {}", e)))?;
		Ok(exists)
	}

	async fn clear(&self) -> Result<()> {
		let mut conn = self.get_connection().await?;
		let _: () = redis::cmd("FLUSHDB")
			.query_async(&mut conn)
			.await
			.map_err(|e| Error::Http(format!("Failed to clear Redis cache: {}", e)))?;
		Ok(())
	}

	async fn get_many<T>(&self, keys: &[&str]) -> Result<HashMap<String, T>>
	where
		T: for<'de> Deserialize<'de> + Send,
	{
		let mut conn = self.get_connection().await?;

		let values: Vec<Option<Vec<u8>>> = conn
			.mget(keys)
			.await
			.map_err(|e| Error::Http(format!("Failed to get multiple values from Redis: {}", e)))?;

		let mut results = HashMap::new();
		for (i, value_opt) in values.into_iter().enumerate() {
			if let Some(bytes) = value_opt {
				let deserialized: T = serde_json::from_slice(&bytes)
					.map_err(|e| Error::Serialization(e.to_string()))?;
				results.insert(keys[i].to_string(), deserialized);
			}
		}

		Ok(results)
	}

	async fn set_many<T>(&self, values: HashMap<String, T>, ttl: Option<Duration>) -> Result<()>
	where
		T: Serialize + Send + Sync,
	{
		let mut conn = self.get_connection().await?;

		for (key, value) in values.iter() {
			let serialized =
				serde_json::to_vec(value).map_err(|e| Error::Serialization(e.to_string()))?;

			if let Some(ttl_duration) = ttl {
				let seconds = ttl_duration.as_secs();
				let _: () = conn
					.set_ex(key, serialized, seconds)
					.await
					.map_err(|e| Error::Http(format!("Failed to set value in Redis: {}", e)))?;
			} else {
				let _: () = conn
					.set(key, serialized)
					.await
					.map_err(|e| Error::Http(format!("Failed to set value in Redis: {}", e)))?;
			}
		}

		Ok(())
	}

	async fn delete_many(&self, keys: &[&str]) -> Result<()> {
		let mut conn = self.get_connection().await?;
		let _: () = conn.del(keys).await.map_err(|e| {
			Error::Http(format!(
				"Failed to delete multiple values from Redis: {}",
				e
			))
		})?;
		Ok(())
	}

	async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
		let mut conn = self.get_connection().await?;
		let result: i64 = conn
			.incr(key, delta)
			.await
			.map_err(|e| Error::Http(format!("Failed to increment value in Redis: {}", e)))?;
		Ok(result)
	}

	async fn decr(&self, key: &str, delta: i64) -> Result<i64> {
		let mut conn = self.get_connection().await?;
		let result: i64 = conn
			.decr(key, delta)
			.await
			.map_err(|e| Error::Http(format!("Failed to decrement value in Redis: {}", e)))?;
		Ok(result)
	}
}
