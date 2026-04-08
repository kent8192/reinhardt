//! Redis-backed session storage backend.
//!
//! Provides [`RedisSessionBackend`], an implementation of `AsyncSessionBackend`
//! that stores sessions in Redis using JSON serialization and native Redis TTL.

#![cfg(feature = "session-redis")]

use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use redis::AsyncCommands;
use reinhardt_http::Result;

use crate::session::{AsyncSessionBackend, SessionData};

/// Session backend backed by Redis.
///
/// Sessions are stored as JSON strings under the key `<prefix><session_id>`.
/// Redis native TTL (`SET ... EX`) is used for expiry so that Redis handles
/// garbage collection automatically.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_middleware::RedisSessionBackend;
///
/// let backend = RedisSessionBackend::new_from_url("redis://127.0.0.1/")
///     .expect("failed to connect to Redis")
///     .with_key_prefix("myapp:session:".to_string());
/// ```
pub struct RedisSessionBackend {
	client: redis::Client,
	key_prefix: String,
}

impl RedisSessionBackend {
	/// Create a new backend from a Redis connection URL.
	///
	/// The connection is not established until the first operation is performed;
	/// this constructor only validates the URL format.
	pub fn new_from_url(url: &str) -> std::result::Result<Self, redis::RedisError> {
		let client = redis::Client::open(url)?;
		Ok(Self {
			client,
			key_prefix: "session:".to_string(),
		})
	}

	/// Override the key prefix used when storing sessions in Redis.
	///
	/// Defaults to `"session:"`.
	pub fn with_key_prefix(mut self, prefix: String) -> Self {
		self.key_prefix = prefix;
		self
	}

	/// Build the full Redis key for a session ID.
	fn redis_key(&self, id: &str) -> String {
		format!("{}{}", self.key_prefix, id)
	}

	/// Obtain a multiplexed async connection from the client pool.
	async fn connection(&self) -> Result<redis::aio::MultiplexedConnection> {
		self.client
			.get_multiplexed_async_connection()
			.await
			.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))
	}
}

#[async_trait]
impl AsyncSessionBackend for RedisSessionBackend {
	/// Load a session by ID.
	///
	/// Returns `None` if the key does not exist in Redis or if the stored
	/// session has already expired according to its own `expires_at` field
	/// (the Redis TTL is the authoritative expiry, but we double-check).
	async fn load(&self, id: &str) -> Result<Option<SessionData>> {
		let mut conn = self.connection().await?;
		let key = self.redis_key(id);

		let raw: Option<String> = conn
			.get(&key)
			.await
			.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))?;

		match raw {
			None => Ok(None),
			Some(json) => {
				let session: SessionData = serde_json::from_str(&json)
					.map_err(|e| reinhardt_core::exception::Error::Serialization(e.to_string()))?;

				if session.expires_at <= SystemTime::now() {
					// Session expired in-process; eagerly remove from Redis.
					let _: () = conn
						.del(&key)
						.await
						.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))?;
					return Ok(None);
				}

				Ok(Some(session))
			}
		}
	}

	/// Persist a session to Redis with a TTL derived from `expires_at`.
	///
	/// If `expires_at` is already in the past the session is not stored.
	async fn save(&self, session: &SessionData) -> Result<()> {
		let ttl_secs = session
			.expires_at
			.duration_since(SystemTime::now())
			.map(|d| d.as_secs())
			.unwrap_or(0);

		if ttl_secs == 0 {
			// Nothing useful to store; session is already expired.
			return Ok(());
		}

		let json = serde_json::to_string(session)
			.map_err(|e| reinhardt_core::exception::Error::Serialization(e.to_string()))?;

		let mut conn = self.connection().await?;
		let key = self.redis_key(&session.id);

		redis::cmd("SET")
			.arg(&key)
			.arg(&json)
			.arg("EX")
			.arg(ttl_secs)
			.exec_async(&mut conn)
			.await
			.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))?;

		Ok(())
	}

	/// Remove a session from Redis.
	async fn destroy(&self, id: &str) -> Result<()> {
		let mut conn = self.connection().await?;
		let key = self.redis_key(id);

		let _: () = conn
			.del(&key)
			.await
			.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))?;

		Ok(())
	}

	/// Refresh the Redis TTL for an existing session without rewriting the payload.
	async fn touch(&self, id: &str, ttl: Duration) -> Result<()> {
		let mut conn = self.connection().await?;
		let key = self.redis_key(id);

		let _: () = conn
			.expire(&key, ttl.as_secs() as i64)
			.await
			.map_err(|e| reinhardt_core::exception::Error::Internal(e.to_string()))?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Verify that the struct can be constructed from a valid Redis URL without
	/// requiring a live Redis instance.  The `Client::open` call only parses
	/// the URL; it does not open a TCP connection.
	#[test]
	fn test_redis_session_backend_construction() {
		let backend = RedisSessionBackend::new_from_url("redis://127.0.0.1/")
			.expect("should construct from valid URL");
		assert_eq!(backend.key_prefix, "session:");
		assert_eq!(backend.redis_key("abc123"), "session:abc123");
	}

	#[test]
	fn test_redis_session_backend_custom_prefix() {
		let backend = RedisSessionBackend::new_from_url("redis://127.0.0.1/")
			.expect("should construct")
			.with_key_prefix("myapp:sess:".to_string());
		assert_eq!(backend.redis_key("xyz"), "myapp:sess:xyz");
	}

	#[test]
	fn test_redis_session_backend_invalid_url() {
		let result = RedisSessionBackend::new_from_url("not-a-valid-url");
		assert!(result.is_err(), "invalid URL should return an error");
	}
}
