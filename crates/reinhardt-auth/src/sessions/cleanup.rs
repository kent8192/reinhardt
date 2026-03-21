//! Automatic session cleanup and expiration
//!
//! This module provides functionality to automatically clean up expired sessions
//! from different backends. It can be run as a background task or scheduled job.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::cleanup::SessionCleanupTask;
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = InMemorySessionBackend::new();
//!
//! // Create a cleanup task
//! let cleanup = SessionCleanupTask::new(backend, Duration::from_secs(3600));
//!
//! // Run cleanup manually
//! let removed_count = cleanup.run_cleanup().await?;
//! println!("Removed {} expired sessions", removed_count);
//! # Ok(())
//! # }
//! ```

use super::backends::{SessionBackend, SessionError};
use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::marker::PhantomData;
use std::time::Duration;

/// Session cleanup configuration
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::cleanup::CleanupConfig;
/// use std::time::Duration;
///
/// let config = CleanupConfig {
///     max_age: Duration::from_secs(7200),
///     batch_size: 100,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct CleanupConfig {
	/// Maximum session age before cleanup
	pub max_age: Duration,
	/// Number of sessions to clean up in one batch
	pub batch_size: usize,
}

impl Default for CleanupConfig {
	/// Create default cleanup configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::cleanup::CleanupConfig;
	///
	/// let config = CleanupConfig::default();
	/// assert_eq!(config.max_age.as_secs(), 1209600); // 2 weeks
	/// assert_eq!(config.batch_size, 1000);
	/// ```
	fn default() -> Self {
		Self {
			max_age: Duration::from_secs(1209600), // 2 weeks
			batch_size: 1000,
		}
	}
}

/// Trait for session backends that support cleanup
#[async_trait]
pub trait CleanupableBackend: SessionBackend {
	/// Get all session keys
	async fn get_all_keys(&self) -> Result<Vec<String>, SessionError>;

	/// Get session metadata (creation time, last access time)
	async fn get_metadata(
		&self,
		session_key: &str,
	) -> Result<Option<SessionMetadata>, SessionError>;

	/// Get list of keys filtered by prefix
	///
	/// Default implementation uses get_all_keys() for filtering.
	/// Backends may provide more efficient implementations (e.g., database LIKE queries).
	async fn list_keys_with_prefix(&self, prefix: &str) -> Result<Vec<String>, SessionError> {
		// Default implementation: filter using get_all_keys()
		let all_keys = self.get_all_keys().await?;
		Ok(all_keys
			.into_iter()
			.filter(|key| key.starts_with(prefix))
			.collect())
	}

	/// Count keys filtered by prefix
	///
	/// Default implementation uses list_keys_with_prefix().
	/// Backends may provide more efficient implementations (e.g., COUNT queries).
	async fn count_keys_with_prefix(&self, prefix: &str) -> Result<usize, SessionError> {
		let keys = self.list_keys_with_prefix(prefix).await?;
		Ok(keys.len())
	}

	/// Delete all keys matching prefix
	///
	/// Default implementation uses list_keys_with_prefix() and delete().
	/// Backends may provide more efficient implementations (e.g., bulk DELETE).
	///
	/// # Returns
	///
	/// Returns the number of deleted sessions.
	async fn delete_keys_with_prefix(&self, prefix: &str) -> Result<usize, SessionError> {
		let keys = self.list_keys_with_prefix(prefix).await?;
		let mut deleted = 0;
		for key in keys {
			if self.delete(&key).await.is_ok() {
				deleted += 1;
			}
		}
		Ok(deleted)
	}
}

/// Session metadata for cleanup
#[derive(Debug, Clone)]
pub struct SessionMetadata {
	/// When the session was created
	pub created_at: DateTime<Utc>,
	/// When the session was last accessed
	pub last_accessed: Option<DateTime<Utc>>,
}

/// Session cleanup task
///
/// Automatically removes expired sessions based on configuration.
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::cleanup::SessionCleanupTask;
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
/// let cleanup = SessionCleanupTask::new(backend, Duration::from_secs(3600));
///
/// // Run cleanup
/// let count = cleanup.run_cleanup().await?;
/// println!("Cleaned up {} sessions", count);
/// # Ok(())
/// # }
/// ```
pub struct SessionCleanupTask<B: SessionBackend> {
	backend: B,
	config: CleanupConfig,
	_phantom: PhantomData<B>,
}

impl<B: SessionBackend> SessionCleanupTask<B> {
	/// Create a new cleanup task with default configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::cleanup::SessionCleanupTask;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	/// use std::time::Duration;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let cleanup = SessionCleanupTask::new(backend, Duration::from_secs(3600));
	/// ```
	pub fn new(backend: B, max_age: Duration) -> Self {
		Self {
			backend,
			config: CleanupConfig {
				max_age,
				..Default::default()
			},
			_phantom: PhantomData,
		}
	}

	/// Create a new cleanup task with custom configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::cleanup::{SessionCleanupTask, CleanupConfig};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	/// use std::time::Duration;
	///
	/// let backend = InMemorySessionBackend::new();
	/// let config = CleanupConfig {
	///     max_age: Duration::from_secs(7200),
	///     batch_size: 500,
	/// };
	/// let cleanup = SessionCleanupTask::with_config(backend, config);
	/// ```
	pub fn with_config(backend: B, config: CleanupConfig) -> Self {
		Self {
			backend,
			config,
			_phantom: PhantomData,
		}
	}

	/// Run cleanup operation
	///
	/// Returns the number of sessions that were removed.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::cleanup::SessionCleanupTask;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let cleanup = SessionCleanupTask::new(backend, Duration::from_secs(3600));
	///
	/// let removed = cleanup.run_cleanup().await?;
	/// println!("Removed {} expired sessions", removed);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn run_cleanup(&self) -> Result<usize, SessionError> {
		// For basic backends without metadata support, we can't determine age
		// This is a simplified implementation that always returns 0
		// Specific backends (database, file) should implement CleanupableBackend
		Ok(0)
	}
}

impl<B: SessionBackend + CleanupableBackend> SessionCleanupTask<B> {
	/// Run cleanup operation for backends with metadata support
	///
	/// # Example
	///
	/// ```rust,no_run,ignore
	/// use reinhardt_auth::sessions::cleanup::SessionCleanupTask;
	/// # use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let backend = InMemorySessionBackend::new();
	/// let cleanup = SessionCleanupTask::new(backend, Duration::from_secs(3600));
	///
	/// let removed = cleanup.run_cleanup_with_metadata().await?;
	/// println!("Removed {} expired sessions", removed);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn run_cleanup_with_metadata(&self) -> Result<usize, SessionError> {
		let all_keys = self.backend.get_all_keys().await?;
		let cutoff_time = Utc::now() - ChronoDuration::from_std(self.config.max_age).unwrap();

		let mut removed_count = 0;

		for chunk in all_keys.chunks(self.config.batch_size) {
			for key in chunk {
				if let Some(metadata) = self.backend.get_metadata(key).await? {
					// Check if session is expired based on last_accessed time
					if metadata.last_accessed < Some(cutoff_time)
						&& self.backend.delete(key).await.is_ok()
					{
						removed_count += 1;
					}
				}
			}
		}

		Ok(removed_count)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_cleanup_config_default() {
		let config = CleanupConfig::default();
		assert_eq!(config.max_age.as_secs(), 1209600); // 2 weeks
		assert_eq!(config.batch_size, 1000);
	}

	#[rstest]
	#[tokio::test]
	async fn test_cleanup_task_creation() {
		let backend = InMemorySessionBackend::new();
		let _cleanup = SessionCleanupTask::new(backend, Duration::from_secs(3600));
	}

	#[rstest]
	#[tokio::test]
	async fn test_cleanup_task_with_config() {
		let backend = InMemorySessionBackend::new();
		let config = CleanupConfig {
			max_age: Duration::from_secs(7200),
			batch_size: 500,
		};
		let _cleanup = SessionCleanupTask::with_config(backend, config);
	}

	#[rstest]
	#[tokio::test]
	async fn test_run_cleanup_basic_backend() {
		let backend = InMemorySessionBackend::new();
		let cleanup = SessionCleanupTask::new(backend, Duration::from_secs(3600));

		// Basic backend without metadata support returns 0
		let removed = cleanup.run_cleanup().await.unwrap();
		assert_eq!(removed, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_cleanup_removes_expired_sessions() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let data = serde_json::json!({"user": "expired_user"});
		backend
			.save("sess_expired", &data, Some(3600))
			.await
			.unwrap();

		// Wait briefly so the session ages past the very short max_age
		tokio::time::sleep(Duration::from_millis(50)).await;

		// Create cleanup task with very short max_age (1ms) so session is considered expired
		let cleanup = SessionCleanupTask::new(backend.clone(), Duration::from_millis(1));

		// Act
		let removed = cleanup.run_cleanup_with_metadata().await.unwrap();

		// Assert
		assert_eq!(removed, 1);
		let loaded: Option<serde_json::Value> = backend.load("sess_expired").await.unwrap();
		assert!(loaded.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_cleanup_preserves_recent_sessions() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let data = serde_json::json!({"user": "active_user"});
		backend
			.save("sess_recent", &data, Some(3600))
			.await
			.unwrap();

		// Access the session to populate last_accessed timestamp
		let _: Option<serde_json::Value> = backend.load("sess_recent").await.unwrap();

		// Create cleanup task with long max_age so session is still valid
		let cleanup = SessionCleanupTask::new(backend.clone(), Duration::from_secs(3600));

		// Act
		let removed = cleanup.run_cleanup_with_metadata().await.unwrap();

		// Assert
		assert_eq!(removed, 0);
		let loaded: Option<serde_json::Value> = backend.load("sess_recent").await.unwrap();
		assert!(loaded.is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn test_cleanup_batch_processing() {
		// Arrange
		let backend = InMemorySessionBackend::new();
		let data = serde_json::json!({"batch": true});
		backend
			.save("batch_sess_1", &data, Some(3600))
			.await
			.unwrap();
		backend
			.save("batch_sess_2", &data, Some(3600))
			.await
			.unwrap();
		backend
			.save("batch_sess_3", &data, Some(3600))
			.await
			.unwrap();

		// Wait briefly so sessions age past the very short max_age
		tokio::time::sleep(Duration::from_millis(50)).await;

		// Create cleanup task with batch_size=1 and very short max_age
		let config = CleanupConfig {
			max_age: Duration::from_millis(1),
			batch_size: 1,
		};
		let cleanup = SessionCleanupTask::with_config(backend.clone(), config);

		// Act
		let removed = cleanup.run_cleanup_with_metadata().await.unwrap();

		// Assert
		assert_eq!(removed, 3);
		assert!(!backend.exists("batch_sess_1").await.unwrap());
		assert!(!backend.exists("batch_sess_2").await.unwrap());
		assert!(!backend.exists("batch_sess_3").await.unwrap());
	}
}
