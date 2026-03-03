//! Session replication support
//!
//! This module provides session replication across multiple backends for high availability.
//! It supports multiple replication strategies with different consistency guarantees.
//!
//! ## Replication Strategies
//!
//! - **AsyncReplication**: Write to primary, then replicate to secondary asynchronously
//!   - Best for: High throughput, eventual consistency acceptable
//!   - Consistency: Eventual
//!   - Performance: Fastest
//!
//! - **SyncReplication**: Write to primary and secondary in parallel
//!   - Best for: Strong consistency requirements
//!   - Consistency: Strong
//!   - Performance: Slower (waits for both)
//!
//! - **AcknowledgedReplication**: Write to primary, wait for secondary acknowledgment
//!   - Best for: Balance between consistency and performance
//!   - Consistency: Strong (with acknowledgment)
//!   - Performance: Moderate
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_auth::sessions::replication::{ReplicatedSessionBackend, ReplicationStrategy};
//! use reinhardt_auth::sessions::backends::{InMemorySessionBackend, CacheSessionBackend};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create primary and secondary backends
//! let primary = InMemorySessionBackend::new();
//! let secondary = InMemorySessionBackend::new();
//!
//! // Create replicated backend with async replication
//! let replicated = ReplicatedSessionBackend::new(
//!     primary,
//!     secondary,
//!     ReplicationStrategy::AsyncReplication,
//! );
//!
//! // All writes go to primary, then replicate asynchronously
//! # Ok(())
//! # }
//! ```

use super::backends::{SessionBackend, SessionError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Replication strategy
///
/// Determines how session data is replicated between primary and secondary backends.
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::replication::ReplicationStrategy;
///
/// // Recommended for most use cases
/// let strategy = ReplicationStrategy::AsyncReplication;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationStrategy {
	/// Replicate asynchronously (eventual consistency)
	///
	/// - Primary write completes immediately
	/// - Secondary replication happens in background
	/// - Best for high throughput
	AsyncReplication,

	/// Replicate synchronously (strong consistency)
	///
	/// - Write to both primary and secondary in parallel
	/// - Operation completes when both succeed
	/// - Best for strong consistency requirements
	SyncReplication,

	/// Replicate with acknowledgment (balanced)
	///
	/// - Write to primary first
	/// - Wait for secondary acknowledgment
	/// - Best balance between consistency and performance
	AcknowledgedReplication,
}

/// Replication event for background processing
#[derive(Debug, Clone)]
enum ReplicationEvent {
	/// Save data to secondary
	Save {
		session_key: String,
		data: Vec<u8>,
		ttl: Option<u64>,
	},
	/// Delete from secondary
	Delete { session_key: String },
}

/// Replication configuration
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::replication::ReplicationConfig;
///
/// let config = ReplicationConfig {
///     channel_buffer_size: 1000,
///     retry_attempts: 3,
///     retry_delay_ms: 100,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
	/// Size of the replication event channel buffer
	pub channel_buffer_size: usize,
	/// Number of retry attempts for failed replications
	pub retry_attempts: u32,
	/// Delay between retry attempts in milliseconds
	pub retry_delay_ms: u64,
}

impl Default for ReplicationConfig {
	fn default() -> Self {
		Self {
			channel_buffer_size: 1000,
			retry_attempts: 3,
			retry_delay_ms: 100,
		}
	}
}

/// Replicated session backend
///
/// Manages session replication between a primary and secondary backend
/// using the configured replication strategy.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::replication::{ReplicatedSessionBackend, ReplicationStrategy};
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let primary = InMemorySessionBackend::new();
/// let secondary = InMemorySessionBackend::new();
///
/// let replicated = ReplicatedSessionBackend::new(
///     primary,
///     secondary,
///     ReplicationStrategy::AsyncReplication,
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ReplicatedSessionBackend<P, S> {
	primary: Arc<P>,
	secondary: Arc<S>,
	strategy: ReplicationStrategy,
	// Allow dead_code: config stored for future replication strategy customization
	#[allow(dead_code)]
	config: ReplicationConfig,
	replication_tx: Option<mpsc::UnboundedSender<ReplicationEvent>>,
}

impl<P, S> ReplicatedSessionBackend<P, S>
where
	P: SessionBackend + Clone + 'static,
	S: SessionBackend + Clone + 'static,
{
	/// Create a new replicated session backend with default config
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::replication::{ReplicatedSessionBackend, ReplicationStrategy};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let primary = InMemorySessionBackend::new();
	/// let secondary = InMemorySessionBackend::new();
	///
	/// let replicated = ReplicatedSessionBackend::new(
	///     primary,
	///     secondary,
	///     ReplicationStrategy::AsyncReplication,
	/// );
	/// ```
	pub fn new(primary: P, secondary: S, strategy: ReplicationStrategy) -> Self {
		Self::with_config(primary, secondary, strategy, ReplicationConfig::default())
	}

	/// Create a new replicated session backend with custom config
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_auth::sessions::replication::{
	///     ReplicatedSessionBackend, ReplicationStrategy, ReplicationConfig,
	/// };
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let config = ReplicationConfig {
	///     channel_buffer_size: 2000,
	///     retry_attempts: 5,
	///     retry_delay_ms: 200,
	/// };
	///
	/// let primary = InMemorySessionBackend::new();
	/// let secondary = InMemorySessionBackend::new();
	///
	/// let replicated = ReplicatedSessionBackend::with_config(
	///     primary,
	///     secondary,
	///     ReplicationStrategy::AsyncReplication,
	///     config,
	/// );
	/// ```
	pub fn with_config(
		primary: P,
		secondary: S,
		strategy: ReplicationStrategy,
		config: ReplicationConfig,
	) -> Self {
		let primary = Arc::new(primary);
		let secondary = Arc::new(secondary);

		// Create replication channel for async strategy
		let replication_tx = if matches!(strategy, ReplicationStrategy::AsyncReplication) {
			let (tx, rx) = mpsc::unbounded_channel();

			// Spawn background replication worker
			let secondary_clone = Arc::clone(&secondary);
			let config_clone = config.clone();
			tokio::spawn(async move {
				Self::replication_worker(rx, secondary_clone, config_clone).await;
			});

			Some(tx)
		} else {
			None
		};

		Self {
			primary,
			secondary,
			strategy,
			config,
			replication_tx,
		}
	}

	/// Background replication worker for async strategy
	async fn replication_worker(
		mut rx: mpsc::UnboundedReceiver<ReplicationEvent>,
		secondary: Arc<S>,
		config: ReplicationConfig,
	) {
		while let Some(event) = rx.recv().await {
			// Retry logic
			let mut attempts = 0;
			loop {
				let result = match &event {
					ReplicationEvent::Save {
						session_key,
						data,
						ttl,
					} => {
						// Deserialize and save
						match serde_json::from_slice::<serde_json::Value>(data) {
							Ok(value) => secondary.save(session_key, &value, *ttl).await,
							Err(e) => Err(SessionError::SerializationError(e.to_string())),
						}
					}
					ReplicationEvent::Delete { session_key } => secondary.delete(session_key).await,
				};

				match result {
					Ok(_) => break, // Success
					Err(e) => {
						attempts += 1;
						if attempts >= config.retry_attempts {
							tracing::error!(
								event = ?event,
								attempts = attempts,
								error = %e,
								"Replication failed after retries"
							);
							break;
						}

						tracing::warn!(
							event = ?event,
							attempt = attempts,
							error = %e,
							"Replication failed, retrying"
						);

						tokio::time::sleep(tokio::time::Duration::from_millis(
							config.retry_delay_ms,
						))
						.await;
					}
				}
			}
		}
	}

	/// Get a reference to the primary backend
	pub fn primary(&self) -> &P {
		&self.primary
	}

	/// Get a reference to the secondary backend
	pub fn secondary(&self) -> &S {
		&self.secondary
	}

	/// Get the replication strategy
	pub fn strategy(&self) -> ReplicationStrategy {
		self.strategy
	}
}

#[async_trait]
impl<P, S> SessionBackend for ReplicatedSessionBackend<P, S>
where
	P: SessionBackend + Clone + 'static,
	S: SessionBackend + Clone + 'static,
{
	async fn load<T>(&self, session_key: &str) -> Result<Option<T>, SessionError>
	where
		T: for<'de> Deserialize<'de> + Serialize + Send + Sync,
	{
		// Always read from primary
		let result = self.primary.load(session_key).await?;

		// If not found in primary, try secondary as fallback
		if result.is_none() {
			return self.secondary.load(session_key).await;
		}

		Ok(result)
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
		match self.strategy {
			ReplicationStrategy::AsyncReplication => {
				// Write to primary first
				self.primary.save(session_key, data, ttl).await?;

				// Queue replication to secondary
				if let Some(ref tx) = self.replication_tx {
					let serialized = serde_json::to_vec(data)
						.map_err(|e| SessionError::SerializationError(e.to_string()))?;

					let _ = tx.send(ReplicationEvent::Save {
						session_key: session_key.to_string(),
						data: serialized,
						ttl,
					});
				}

				Ok(())
			}

			ReplicationStrategy::SyncReplication => {
				// Write to both in parallel
				let primary_future = self.primary.save(session_key, data, ttl);
				let secondary_future = self.secondary.save(session_key, data, ttl);

				let (primary_result, secondary_result) =
					tokio::join!(primary_future, secondary_future);

				// Both must succeed
				primary_result?;
				secondary_result?;

				Ok(())
			}

			ReplicationStrategy::AcknowledgedReplication => {
				// Write to primary first
				self.primary.save(session_key, data, ttl).await?;

				// Then write to secondary (with acknowledgment)
				self.secondary.save(session_key, data, ttl).await?;

				Ok(())
			}
		}
	}

	async fn delete(&self, session_key: &str) -> Result<(), SessionError> {
		match self.strategy {
			ReplicationStrategy::AsyncReplication => {
				// Delete from primary first
				self.primary.delete(session_key).await?;

				// Queue deletion to secondary
				if let Some(ref tx) = self.replication_tx {
					let _ = tx.send(ReplicationEvent::Delete {
						session_key: session_key.to_string(),
					});
				}

				Ok(())
			}

			ReplicationStrategy::SyncReplication => {
				// Delete from both in parallel
				let primary_future = self.primary.delete(session_key);
				let secondary_future = self.secondary.delete(session_key);

				let (primary_result, secondary_result) =
					tokio::join!(primary_future, secondary_future);

				// Both must succeed
				primary_result?;
				secondary_result?;

				Ok(())
			}

			ReplicationStrategy::AcknowledgedReplication => {
				// Delete from primary first
				self.primary.delete(session_key).await?;

				// Then delete from secondary (with acknowledgment)
				self.secondary.delete(session_key).await?;

				Ok(())
			}
		}
	}

	async fn exists(&self, session_key: &str) -> Result<bool, SessionError> {
		// Check primary first
		let exists = self.primary.exists(session_key).await?;

		if !exists {
			// Fallback to secondary
			return self.secondary.exists(session_key).await;
		}

		Ok(exists)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;

	#[tokio::test]
	async fn test_async_replication_save() {
		let primary = InMemorySessionBackend::new();
		let secondary = InMemorySessionBackend::new();

		let replicated = ReplicatedSessionBackend::new(
			primary.clone(),
			secondary.clone(),
			ReplicationStrategy::AsyncReplication,
		);

		let data = serde_json::json!({"key": "value"});

		replicated.save("test_key", &data, None).await.unwrap();

		// Primary should have data immediately
		let primary_data: Option<serde_json::Value> = primary.load("test_key").await.unwrap();
		assert_eq!(primary_data.unwrap(), data);

		// Wait a bit for async replication
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

		// Secondary should have data after async replication
		let secondary_data: Option<serde_json::Value> = secondary.load("test_key").await.unwrap();
		assert_eq!(secondary_data.unwrap(), data);
	}

	#[tokio::test]
	async fn test_sync_replication_save() {
		let primary = InMemorySessionBackend::new();
		let secondary = InMemorySessionBackend::new();

		let replicated = ReplicatedSessionBackend::new(
			primary.clone(),
			secondary.clone(),
			ReplicationStrategy::SyncReplication,
		);

		let data = serde_json::json!({"key": "value"});

		replicated.save("test_key", &data, None).await.unwrap();

		// Both should have data immediately
		let primary_data: Option<serde_json::Value> = primary.load("test_key").await.unwrap();
		assert_eq!(primary_data.unwrap(), data);

		let secondary_data: Option<serde_json::Value> = secondary.load("test_key").await.unwrap();
		assert_eq!(secondary_data.unwrap(), data);
	}

	#[tokio::test]
	async fn test_acknowledged_replication_save() {
		let primary = InMemorySessionBackend::new();
		let secondary = InMemorySessionBackend::new();

		let replicated = ReplicatedSessionBackend::new(
			primary.clone(),
			secondary.clone(),
			ReplicationStrategy::AcknowledgedReplication,
		);

		let data = serde_json::json!({"key": "value"});

		replicated.save("test_key", &data, None).await.unwrap();

		// Both should have data after acknowledged write
		let primary_data: Option<serde_json::Value> = primary.load("test_key").await.unwrap();
		assert_eq!(primary_data.unwrap(), data);

		let secondary_data: Option<serde_json::Value> = secondary.load("test_key").await.unwrap();
		assert_eq!(secondary_data.unwrap(), data);
	}

	#[tokio::test]
	async fn test_replication_delete() {
		let primary = InMemorySessionBackend::new();
		let secondary = InMemorySessionBackend::new();

		let replicated = ReplicatedSessionBackend::new(
			primary.clone(),
			secondary.clone(),
			ReplicationStrategy::SyncReplication,
		);

		let data = serde_json::json!({"key": "value"});

		// Save first
		replicated.save("test_key", &data, None).await.unwrap();

		// Delete
		replicated.delete("test_key").await.unwrap();

		// Both should not have data
		assert!(!primary.exists("test_key").await.unwrap());
		assert!(!secondary.exists("test_key").await.unwrap());
	}

	#[tokio::test]
	async fn test_replication_load_fallback() {
		let primary = InMemorySessionBackend::new();
		let secondary = InMemorySessionBackend::new();

		let replicated = ReplicatedSessionBackend::new(
			primary.clone(),
			secondary.clone(),
			ReplicationStrategy::AsyncReplication,
		);

		let data = serde_json::json!({"key": "value"});

		// Save only to secondary
		secondary.save("test_key", &data, None).await.unwrap();

		// Load should fall back to secondary
		let loaded: Option<serde_json::Value> = replicated.load("test_key").await.unwrap();
		assert_eq!(loaded.unwrap(), data);
	}

	#[tokio::test]
	async fn test_replication_config() {
		let config = ReplicationConfig {
			channel_buffer_size: 2000,
			retry_attempts: 5,
			retry_delay_ms: 200,
		};

		let primary = InMemorySessionBackend::new();
		let secondary = InMemorySessionBackend::new();

		let replicated = ReplicatedSessionBackend::with_config(
			primary,
			secondary,
			ReplicationStrategy::AsyncReplication,
			config.clone(),
		);

		assert_eq!(replicated.config.channel_buffer_size, 2000);
		assert_eq!(replicated.config.retry_attempts, 5);
		assert_eq!(replicated.config.retry_delay_ms, 200);
	}

	#[tokio::test]
	async fn test_replication_strategy_getter() {
		let primary = InMemorySessionBackend::new();
		let secondary = InMemorySessionBackend::new();

		let replicated =
			ReplicatedSessionBackend::new(primary, secondary, ReplicationStrategy::SyncReplication);

		assert_eq!(replicated.strategy(), ReplicationStrategy::SyncReplication);
	}
}
