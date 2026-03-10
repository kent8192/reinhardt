//! Cache-based session backend
//!
//! This module provides session storage using cache backends like Redis or in-memory cache.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::backends::{InMemorySessionBackend, SessionBackend};
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an in-memory session backend
//! let backend = InMemorySessionBackend::new();
//!
//! // Store user session with login data
//! let session_data = json!({
//!     "user_id": 123,
//!     "username": "bob",
//!     "last_login": "2024-01-15T10:30:00Z",
//! });
//!
//! backend.save("session_xyz", &session_data, Some(3600)).await?;
//!
//! // Load session data
//! let loaded: Option<serde_json::Value> = backend.load("session_xyz").await?;
//! assert!(loaded.is_some());
//! assert_eq!(loaded.unwrap()["user_id"], 123);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reinhardt_utils::cache::{Cache, InMemoryCache};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::sessions::cleanup::{CleanupableBackend, SessionMetadata};

/// Session backend errors
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SessionError {
	/// An error occurred in the cache backend.
	#[error("Cache error: {0}")]
	CacheError(String),
	/// Session data could not be serialized or deserialized.
	#[error("Serialization error: {0}")]
	SerializationError(String),
	/// The session has expired due to inactivity.
	#[error("Session has expired due to inactivity")]
	SessionExpired,
}

/// Session backend trait
#[async_trait]
pub trait SessionBackend: Send + Sync + Clone {
	/// Load session data by key
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync;

	/// Save session data with optional TTL (in seconds)
	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync;

	/// Delete session by key
	async fn delete(&self, session_key: &str) -> Result<(), SessionError>;

	/// Check if session exists
	async fn exists(&self, session_key: &str) -> Result<bool, SessionError>;
}

/// In-memory session backend
///
/// Stores sessions in memory using the InMemoryCache backend.
/// Sessions are lost when the application restarts.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::backends::{InMemorySessionBackend, SessionBackend};
/// use serde_json::json;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
///
/// // Store shopping cart in session
/// let cart_data = json!({
///     "items": ["item1", "item2"],
///     "total": 59.99,
/// });
///
/// backend.save("cart_session_456", &cart_data, Some(1800)).await?;
///
/// // Check if session exists
/// assert!(backend.exists("cart_session_456").await?);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct InMemorySessionBackend {
	cache: Arc<InMemoryCache>,
}

impl InMemorySessionBackend {
	/// Create a new in-memory session backend
	pub fn new() -> Self {
		Self {
			cache: Arc::new(InMemoryCache::new()),
		}
	}
}

impl Default for InMemorySessionBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl SessionBackend for InMemorySessionBackend {
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		self.cache
			.get(session_key)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}

	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		let duration = ttl.map(std::time::Duration::from_secs);
		self.cache
			.set(session_key, data, duration)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		self.cache
			.delete(session_key)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		self.cache
			.has_key(session_key)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}
}

#[async_trait]
impl CleanupableBackend for InMemorySessionBackend {
	/// Get all session keys
	///
	/// Returns all session keys stored in the backend,
	/// including expired sessions that haven't been cleaned up.
	async fn get_all_keys(&self) -> Result<Vec<String>, SessionError> {
		Ok(self.cache.list_keys().await)
	}

	/// Get session metadata
	///
	/// Returns metadata for the specified session.
	/// Returns `None` if the session does not exist.
	async fn get_metadata(
		&self,
		session_key: &str,
	) -> Result<Option<SessionMetadata>, SessionError> {
		match self.cache.inspect_entry_with_timestamps(session_key).await {
			Ok(Some((created, accessed))) => Ok(Some(SessionMetadata {
				created_at: DateTime::<Utc>::from(created),
				last_accessed: accessed.map(DateTime::<Utc>::from),
			})),
			Ok(None) => Ok(None),
			Err(e) => Err(SessionError::CacheError(e.to_string())),
		}
	}
}

/// Cache-based session backend
///
/// Generic session backend that works with any cache implementation.
///
/// ## Example
///
/// ```rust
/// use reinhardt_auth::sessions::backends::{CacheSessionBackend, SessionBackend};
/// use reinhardt_utils::cache::InMemoryCache;
/// use serde_json::json;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = Arc::new(InMemoryCache::new());
/// let backend = CacheSessionBackend::new(cache);
///
/// // Store user preferences in session
/// let preferences = json!({
///     "theme": "dark",
///     "language": "en",
///     "notifications": true,
/// });
///
/// backend.save("pref_session_789", &preferences, Some(7200)).await?;
///
/// // Load preferences
/// let loaded: Option<serde_json::Value> = backend.load("pref_session_789").await?;
/// assert_eq!(loaded.unwrap()["theme"], "dark");
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CacheSessionBackend<C: Cache + Clone> {
	cache: Arc<C>,
}

impl<C: Cache + Clone> CacheSessionBackend<C> {
	/// Create a new cache-based session backend
	pub fn new(cache: Arc<C>) -> Self {
		Self { cache }
	}
}

#[async_trait]
impl<C: Cache + Clone + 'static> SessionBackend for CacheSessionBackend<C> {
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		self.cache
			.get(session_key)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}

	async fn save<T>(
		&self,
		session_key: &str,
		data: &T,
		ttl: Option<u64>,
	) -> Result<(), SessionError>
	where
		T: Serialize + Send + Sync,
	{
		let duration = ttl.map(std::time::Duration::from_secs);
		self.cache
			.set(session_key, data, duration)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		self.cache
			.delete(session_key)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		self.cache
			.has_key(session_key)
			.await
			.map_err(|e| SessionError::CacheError(e.to_string()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;
	use std::collections::HashMap;

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_save_and_load_roundtrip() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let mut data = HashMap::new();
		data.insert("user_id".to_string(), json!(42));
		data.insert("username".to_string(), json!("alice"));

		// Act
		backend.save("sess_1", &data, Some(3600)).await.unwrap();
		let loaded: Option<HashMap<String, serde_json::Value>> =
			backend.load("sess_1").await.unwrap();

		// Assert
		let loaded = loaded.unwrap();
		assert_eq!(loaded["user_id"], json!(42));
		assert_eq!(loaded["username"], json!("alice"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_load_nonexistent_key() {
		// Arrange
		let backend = InMemorySessionBackend::new();

		// Act
		let loaded: Option<serde_json::Value> = backend.load("nonexistent").await.unwrap();

		// Assert
		assert!(loaded.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_delete_removes_session() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let data = json!({"key": "value"});
		backend.save("sess_del", &data, Some(3600)).await.unwrap();

		// Act
		backend.delete("sess_del").await.unwrap();
		let loaded: Option<serde_json::Value> = backend.load("sess_del").await.unwrap();

		// Assert
		assert!(loaded.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_exists_reflects_state() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let data = json!({"active": true});

		// Assert - initially does not exist
		assert!(!backend.exists("sess_ex").await.unwrap());

		// Act - save
		backend.save("sess_ex", &data, Some(3600)).await.unwrap();

		// Assert - exists after save
		assert!(backend.exists("sess_ex").await.unwrap());

		// Act - delete
		backend.delete("sess_ex").await.unwrap();

		// Assert - does not exist after delete
		assert!(!backend.exists("sess_ex").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_save_overwrites_existing() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let data_v1 = json!({"version": 1});
		let data_v2 = json!({"version": 2});

		// Act
		backend.save("sess_ow", &data_v1, Some(3600)).await.unwrap();
		backend.save("sess_ow", &data_v2, Some(3600)).await.unwrap();
		let loaded: Option<serde_json::Value> = backend.load("sess_ow").await.unwrap();

		// Assert
		assert_eq!(loaded.unwrap()["version"], 2);
	}

	#[rstest]
	#[tokio::test]
	async fn test_in_memory_save_with_ttl() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let data = json!({"ttl_test": true});

		// Act
		backend.save("sess_ttl", &data, Some(60)).await.unwrap();
		let loaded: Option<serde_json::Value> = backend.load("sess_ttl").await.unwrap();

		// Assert
		assert_eq!(loaded.unwrap()["ttl_test"], true);
	}

	#[rstest]
	#[tokio::test]
	async fn test_cache_backend_wrapper_save_and_load() {
		// Arrange
		let cache = Arc::new(InMemoryCache::new());
		let backend = CacheSessionBackend::new(cache);
		let data = json!({"wrapped": "value", "count": 99});

		// Act
		backend
			.save("wrapped_sess", &data, Some(3600))
			.await
			.unwrap();
		let loaded: Option<serde_json::Value> = backend.load("wrapped_sess").await.unwrap();

		// Assert
		let loaded = loaded.unwrap();
		assert_eq!(loaded["wrapped"], "value");
		assert_eq!(loaded["count"], 99);
	}

	#[rstest]
	#[tokio::test]
	async fn test_cache_backend_wrapper_delete_and_exists() {
		// Arrange
		let cache = Arc::new(InMemoryCache::new());
		let backend = CacheSessionBackend::new(cache);
		let data = json!({"item": "to_delete"});

		// Act - save and verify exists
		backend
			.save("wrap_del", &data, Some(3600))
			.await
			.unwrap();
		assert!(backend.exists("wrap_del").await.unwrap());

		// Act - delete
		backend.delete("wrap_del").await.unwrap();

		// Assert
		assert!(!backend.exists("wrap_del").await.unwrap());
		let loaded: Option<serde_json::Value> = backend.load("wrap_del").await.unwrap();
		assert!(loaded.is_none());
	}
}
