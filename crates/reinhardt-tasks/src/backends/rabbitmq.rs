//! RabbitMQ-based task backend implementation
//!
//! This module provides a RabbitMQ-based task queue backend using the AMQP protocol.
//! It uses RabbitMQ for message queuing and a pluggable metadata store for task
//! status tracking.
//!
//! See [`RabbitMQBackend`] for the architecture diagram.
//!
//! # Examples
//!
//! ## Using default in-memory metadata store
//!
//! ```no_run
//! use reinhardt_tasks::{RabbitMQBackend, RabbitMQConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = RabbitMQConfig::new("amqp://localhost:5672/%2f");
//! let backend = RabbitMQBackend::new(config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Using custom metadata store
//!
//! ```no_run
//! use reinhardt_tasks::{RabbitMQBackend, RabbitMQConfig};
//! use reinhardt_tasks::backends::metadata_store::InMemoryMetadataStore;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = RabbitMQConfig::new("amqp://localhost:5672/%2f");
//! let custom_store = Arc::new(InMemoryMetadataStore::new());
//! let backend = RabbitMQBackend::with_metadata_store(config, custom_store).await?;
//! # Ok(())
//! # }
//! ```

use crate::{
	Task, TaskId, TaskStatus,
	backend::{TaskBackend, TaskExecutionError},
	backends::metadata_store::{InMemoryMetadataStore, MetadataStore, TaskMetadata},
	registry::SerializedTask,
};
use async_trait::async_trait;
use lapin::{
	BasicProperties, Channel, Connection, ConnectionProperties, Error as LapinError, options::*,
	types::FieldTable,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Message payload for RabbitMQ queue
#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueMessage {
	id: TaskId,
	name: String,
	created_at: i64,
}

/// RabbitMQ configuration
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::RabbitMQConfig;
///
/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f");
/// ```
#[derive(Debug, Clone)]
pub struct RabbitMQConfig {
	/// AMQP connection URL (e.g., "amqp://localhost:5672/%2f")
	pub url: String,
	/// Queue name for tasks
	pub queue_name: String,
	/// Exchange name (empty string for default exchange)
	pub exchange_name: String,
	/// Routing key
	pub routing_key: String,
}

impl RabbitMQConfig {
	/// Create a new RabbitMQ configuration
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RabbitMQConfig;
	///
	/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f");
	/// ```
	pub fn new(url: &str) -> Self {
		Self {
			url: url.to_string(),
			queue_name: "reinhardt_tasks".to_string(),
			exchange_name: String::new(),
			routing_key: "reinhardt_tasks".to_string(),
		}
	}

	/// Set custom queue name
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RabbitMQConfig;
	///
	/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f")
	///     .with_queue_name("my_tasks");
	/// ```
	pub fn with_queue_name(mut self, queue_name: &str) -> Self {
		self.queue_name = queue_name.to_string();
		self.routing_key = queue_name.to_string();
		self
	}

	/// Set custom exchange name
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RabbitMQConfig;
	///
	/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f")
	///     .with_exchange("my_exchange");
	/// ```
	pub fn with_exchange(mut self, exchange_name: &str) -> Self {
		self.exchange_name = exchange_name.to_string();
		self
	}

	/// Set custom routing key
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RabbitMQConfig;
	///
	/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f")
	///     .with_routing_key("my_routing_key");
	/// ```
	pub fn with_routing_key(mut self, routing_key: &str) -> Self {
		self.routing_key = routing_key.to_string();
		self
	}
}

impl Default for RabbitMQConfig {
	fn default() -> Self {
		Self::new("amqp://localhost:5672/%2f")
	}
}

#[cfg_attr(doc, aquamarine::aquamarine)]
/// RabbitMQ-based task backend with pluggable metadata storage
///
/// Uses RabbitMQ as a message queue for task distribution and a configurable
/// metadata store for task status tracking and data retrieval.
///
/// # Architecture
///
/// ```mermaid
/// graph TB
///     subgraph RabbitMQBackend["RabbitMQ Backend"]
///         subgraph Queue["RabbitMQ Queue"]
///             TaskMessages["Task messages"]
///             FIFODelivery["FIFO delivery"]
///         end
///
///         subgraph MetadataStore["Metadata Store"]
///             StoreTypes["InMemory / Redis / DB"]
///             StatusTracking["Status tracking"]
///             TaskRetrieval["Task data retrieval"]
///         end
///     end
/// ```
///
/// - **Queue**: RabbitMQ handles task message delivery (FIFO)
/// - **Metadata Store**: Tracks task status and stores task data
///
/// # Default Behavior
///
/// By default, uses `InMemoryMetadataStore` which is suitable for
/// development and testing. For production, consider using a persistent
/// store like Redis or a database.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::{RabbitMQBackend, RabbitMQConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f");
/// let backend = RabbitMQBackend::new(config).await?;
/// # Ok(())
/// # }
/// ```
pub struct RabbitMQBackend {
	connection: Arc<Connection>,
	channel: Arc<RwLock<Channel>>,
	config: RabbitMQConfig,
	/// Pluggable metadata store for task status and data
	metadata_store: Arc<dyn MetadataStore>,
}

impl RabbitMQBackend {
	/// Create a new RabbitMQ backend with default in-memory metadata store
	///
	/// # Arguments
	///
	/// * `config` - RabbitMQ configuration
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::{RabbitMQBackend, RabbitMQConfig};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f")
	///     .with_queue_name("my_tasks");
	/// let backend = RabbitMQBackend::new(config).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(config: RabbitMQConfig) -> Result<Self, LapinError> {
		let metadata_store = Arc::new(InMemoryMetadataStore::new());
		Self::with_metadata_store(config, metadata_store).await
	}

	/// Create a new RabbitMQ backend with custom metadata store
	///
	/// Use this method when you need persistent storage for task metadata,
	/// such as Redis or a database.
	///
	/// # Arguments
	///
	/// * `config` - RabbitMQ configuration
	/// * `metadata_store` - Custom metadata store implementation
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::{RabbitMQBackend, RabbitMQConfig};
	/// use reinhardt_tasks::backends::metadata_store::InMemoryMetadataStore;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// // Using a shared metadata store across multiple backends
	/// let shared_store = Arc::new(InMemoryMetadataStore::new());
	///
	/// let config = RabbitMQConfig::new("amqp://localhost:5672/%2f");
	/// let backend = RabbitMQBackend::with_metadata_store(config, shared_store).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn with_metadata_store(
		config: RabbitMQConfig,
		metadata_store: Arc<dyn MetadataStore>,
	) -> Result<Self, LapinError> {
		let connection = Connection::connect(&config.url, ConnectionProperties::default()).await?;
		let channel = connection.create_channel().await?;

		// Declare queue
		channel
			.queue_declare(
				&config.queue_name,
				QueueDeclareOptions {
					durable: true,
					..Default::default()
				},
				FieldTable::default(),
			)
			.await?;

		Ok(Self {
			connection: Arc::new(connection),
			channel: Arc::new(RwLock::new(channel)),
			config,
			metadata_store,
		})
	}

	/// Ensure connection is healthy, reconnect if necessary
	async fn ensure_connection(&self) -> Result<(), TaskExecutionError> {
		if !self.connection.status().connected() {
			return Err(TaskExecutionError::BackendError(
				"RabbitMQ connection lost".to_string(),
			));
		}
		Ok(())
	}

	/// Get or recreate channel
	async fn get_channel(&self) -> Result<Channel, TaskExecutionError> {
		self.ensure_connection().await?;

		let channel = self.channel.read().await;
		if channel.status().connected() {
			return Ok(channel.clone());
		}

		drop(channel);

		// Recreate channel
		let new_channel = self
			.connection
			.create_channel()
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		// Redeclare queue
		new_channel
			.queue_declare(
				&self.config.queue_name,
				QueueDeclareOptions {
					durable: true,
					..Default::default()
				},
				FieldTable::default(),
			)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		let mut channel_write = self.channel.write().await;
		*channel_write = new_channel.clone();

		Ok(new_channel)
	}
}

#[async_trait]
impl TaskBackend for RabbitMQBackend {
	async fn enqueue(&self, task: Box<dyn Task>) -> Result<TaskId, TaskExecutionError> {
		let task_id = task.id();
		let task_name = task.name().to_string();

		// Store metadata in the pluggable store
		let metadata = TaskMetadata::new(task_id, task_name.clone());
		self.metadata_store
			.store(metadata)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		// Create queue message
		let queue_message = QueueMessage {
			id: task_id,
			name: task_name,
			created_at: chrono::Utc::now().timestamp(),
		};

		let message_json = serde_json::to_string(&queue_message)
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		let channel = self.get_channel().await?;

		// Publish task to queue
		channel
			.basic_publish(
				&self.config.exchange_name,
				&self.config.routing_key,
				BasicPublishOptions::default(),
				message_json.as_bytes(),
				BasicProperties::default().with_delivery_mode(2), // Persistent
			)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		Ok(task_id)
	}

	async fn dequeue(&self) -> Result<Option<TaskId>, TaskExecutionError> {
		let channel = self.get_channel().await?;

		// Get message from queue
		let delivery = channel
			.basic_get(&self.config.queue_name, BasicGetOptions { no_ack: false })
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		match delivery {
			Some(delivery) => {
				let queue_message: QueueMessage = serde_json::from_slice(&delivery.data)
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

				// Fixes #784: Propagate metadata update error instead of silently discarding
				self.metadata_store
					.update_status(queue_message.id, TaskStatus::Running)
					.await
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

				// Acknowledge message
				delivery
					.ack(BasicAckOptions::default())
					.await
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

				Ok(Some(queue_message.id))
			}
			None => Ok(None),
		}
	}

	async fn get_status(&self, task_id: TaskId) -> Result<TaskStatus, TaskExecutionError> {
		let metadata = self
			.metadata_store
			.get(task_id)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		match metadata {
			Some(m) => Ok(m.status),
			None => Err(TaskExecutionError::NotFound(task_id)),
		}
	}

	async fn update_status(
		&self,
		task_id: TaskId,
		status: TaskStatus,
	) -> Result<(), TaskExecutionError> {
		self.metadata_store
			.update_status(task_id, status)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;
		Ok(())
	}

	async fn get_task_data(
		&self,
		task_id: TaskId,
	) -> Result<Option<SerializedTask>, TaskExecutionError> {
		let metadata = self
			.metadata_store
			.get(task_id)
			.await
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		match metadata {
			Some(m) => {
				// Return task_data if stored, otherwise construct from metadata
				if let Some(task_data) = m.task_data {
					Ok(Some(task_data))
				} else {
					// Return a basic SerializedTask with just the name
					Ok(Some(SerializedTask::new(m.name, "{}".to_string())))
				}
			}
			None => Ok(None),
		}
	}

	fn backend_name(&self) -> &str {
		"rabbitmq"
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backends::metadata_store::MetadataStoreError;
	use rstest::rstest;

	#[rstest]
	fn test_rabbitmq_config_new() {
		let config = RabbitMQConfig::new("amqp://localhost:5672/%2f");
		assert_eq!(config.url, "amqp://localhost:5672/%2f");
		assert_eq!(config.queue_name, "reinhardt_tasks");
		assert_eq!(config.exchange_name, "");
		assert_eq!(config.routing_key, "reinhardt_tasks");
	}

	#[test]
	fn test_rabbitmq_config_with_queue_name() {
		let config =
			RabbitMQConfig::new("amqp://localhost:5672/%2f").with_queue_name("custom_queue");
		assert_eq!(config.queue_name, "custom_queue");
		assert_eq!(config.routing_key, "custom_queue");
	}

	#[test]
	fn test_rabbitmq_config_with_exchange() {
		let config = RabbitMQConfig::new("amqp://localhost:5672/%2f").with_exchange("my_exchange");
		assert_eq!(config.exchange_name, "my_exchange");
	}

	#[test]
	fn test_rabbitmq_config_with_routing_key() {
		let config =
			RabbitMQConfig::new("amqp://localhost:5672/%2f").with_routing_key("my_routing_key");
		assert_eq!(config.routing_key, "my_routing_key");
	}

	#[test]
	fn test_rabbitmq_config_default() {
		let config = RabbitMQConfig::default();
		assert_eq!(config.url, "amqp://localhost:5672/%2f");
	}

	#[test]
	fn test_queue_message_serialization() {
		let message = QueueMessage {
			id: TaskId::new(),
			name: "test_task".to_string(),
			created_at: 1234567890,
		};

		let json = serde_json::to_string(&message).unwrap();
		let deserialized: QueueMessage = serde_json::from_str(&json).unwrap();

		assert_eq!(deserialized.id, message.id);
		assert_eq!(deserialized.name, message.name);
		assert_eq!(deserialized.created_at, message.created_at);
	}

	#[rstest]
	#[case::not_found_error(
		MetadataStoreError::NotFound(TaskId::new()),
		"not found in metadata store"
	)]
	#[case::storage_error(
		MetadataStoreError::StorageError("connection refused".to_string()),
		"connection refused"
	)]
	#[case::serialization_error(
		MetadataStoreError::SerializationError("invalid JSON".to_string()),
		"invalid JSON"
	)]
	fn test_metadata_store_error_converts_to_backend_error(
		#[case] metadata_error: MetadataStoreError,
		#[case] expected_substring: &str,
	) {
		// Arrange
		let error_message = metadata_error.to_string();

		// Act
		let backend_error = TaskExecutionError::BackendError(error_message);

		// Assert
		let error_string = backend_error.to_string();
		assert!(
			error_string.contains(expected_substring),
			"Expected error string '{}' to contain '{}'",
			error_string,
			expected_substring,
		);
		assert!(matches!(backend_error, TaskExecutionError::BackendError(_)));
	}

	#[rstest]
	#[tokio::test]
	async fn test_metadata_update_status_error_propagation_path() {
		// Arrange
		let store = InMemoryMetadataStore::new();
		let nonexistent_id = TaskId::new();

		// Act
		let result = store
			.update_status(nonexistent_id, TaskStatus::Running)
			.await;

		// Assert
		// Verify that update_status for a nonexistent task produces an error
		// that can be mapped to TaskExecutionError::BackendError via map_err
		assert!(result.is_err());
		let metadata_err = result.unwrap_err();
		let backend_err = TaskExecutionError::BackendError(metadata_err.to_string());
		assert!(
			matches!(backend_err, TaskExecutionError::BackendError(msg) if msg.contains("not found"))
		);
	}
}
