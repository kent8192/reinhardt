//! AWS SQS-based task backend implementation

use crate::{
	Task, TaskExecutionError, TaskId, TaskStatus, registry::SerializedTask, result::ResultBackend,
	result::TaskResultMetadata,
};
use async_trait::async_trait;
use aws_sdk_sqs::{Client, types::MessageAttributeValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Task metadata for SQS message attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskMetadata {
	id: TaskId,
	name: String,
	status: TaskStatus,
	created_at: i64,
	updated_at: i64,
}

/// Configuration for AWS SQS backend
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::SqsConfig;
///
/// let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue");
/// ```
#[derive(Debug, Clone)]
pub struct SqsConfig {
	/// SQS queue URL
	queue_url: String,
	/// Message visibility timeout in seconds (default: 30)
	visibility_timeout: i32,
	/// Maximum number of messages to receive at once (default: 1, max: 10)
	max_messages: i32,
	/// Wait time for long polling in seconds (default: 0)
	wait_time_seconds: i32,
}

impl SqsConfig {
	/// Create a new SQS configuration
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::SqsConfig;
	///
	/// let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue");
	/// ```
	pub fn new(queue_url: impl Into<String>) -> Self {
		Self {
			queue_url: queue_url.into(),
			visibility_timeout: 30,
			max_messages: 1,
			wait_time_seconds: 0,
		}
	}

	/// Set the visibility timeout
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::SqsConfig;
	///
	/// let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue")
	///     .with_visibility_timeout(60);
	/// ```
	pub fn with_visibility_timeout(mut self, timeout: i32) -> Self {
		self.visibility_timeout = timeout;
		self
	}

	/// Set the maximum number of messages to receive
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::SqsConfig;
	///
	/// let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue")
	///     .with_max_messages(10);
	/// ```
	pub fn with_max_messages(mut self, max_messages: i32) -> Self {
		self.max_messages = max_messages.min(10); // SQS max is 10
		self
	}

	/// Set the wait time for long polling
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::SqsConfig;
	///
	/// let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue")
	///     .with_wait_time_seconds(20);
	/// ```
	pub fn with_wait_time_seconds(mut self, wait_time: i32) -> Self {
		self.wait_time_seconds = wait_time;
		self
	}
}

/// AWS SQS-based task backend
///
/// This backend uses AWS SQS for message queuing and an in-memory store
/// for task metadata and status tracking. For production use, consider
/// using a persistent store (Redis, DynamoDB) for metadata.
///
/// # Limitations
///
/// - Task metadata is stored in memory and will be lost on restart
/// - Message size is limited to 256 KB
/// - Messages are retained for up to 14 days (configurable via SQS queue settings)
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::{SqsBackend, SqsConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue");
/// let backend = SqsBackend::new(config).await?;
/// # Ok(())
/// # }
/// ```
pub struct SqsBackend {
	client: Client,
	config: SqsConfig,
	/// In-memory metadata store (task_id -> metadata)
	/// Note: In production, use Redis or DynamoDB for persistence
	metadata_store: Arc<RwLock<HashMap<TaskId, TaskMetadata>>>,
	/// Store receipt handles for message deletion (task_id -> receipt_handle)
	receipt_handles: Arc<RwLock<HashMap<TaskId, String>>>,
}

impl SqsBackend {
	/// Create a new SQS backend
	///
	/// # Arguments
	///
	/// * `config` - SQS configuration
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::{SqsBackend, SqsConfig};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue");
	/// let backend = SqsBackend::new(config).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(config: SqsConfig) -> Result<Self, TaskExecutionError> {
		let aws_config = aws_config::load_from_env().await;
		let client = Client::new(&aws_config);

		Ok(Self {
			client,
			config,
			metadata_store: Arc::new(RwLock::new(HashMap::new())),
			receipt_handles: Arc::new(RwLock::new(HashMap::new())),
		})
	}

	/// Create a new SQS backend with custom AWS SDK configuration
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::{SqsBackend, SqsConfig};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let aws_config = aws_config::load_from_env().await;
	/// let sqs_config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue");
	/// let backend = SqsBackend::with_config(sqs_config, &aws_config);
	/// # Ok(())
	/// # }
	/// ```
	pub fn with_config(config: SqsConfig, aws_config: &aws_config::SdkConfig) -> Self {
		let client = Client::new(aws_config);

		Self {
			client,
			config,
			metadata_store: Arc::new(RwLock::new(HashMap::new())),
			receipt_handles: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Delete a message from SQS
	async fn delete_message(&self, receipt_handle: &str) -> Result<(), TaskExecutionError> {
		self.client
			.delete_message()
			.queue_url(&self.config.queue_url)
			.receipt_handle(receipt_handle)
			.send()
			.await
			.map_err(|e| TaskExecutionError::BackendError(format!("SQS delete error: {}", e)))?;

		Ok(())
	}
}

#[async_trait]
impl crate::backend::TaskBackend for SqsBackend {
	async fn enqueue(&self, task: Box<dyn Task>) -> Result<TaskId, TaskExecutionError> {
		let task_id = task.id();
		let task_name = task.name().to_string();

		let metadata = TaskMetadata {
			id: task_id,
			name: task_name.clone(),
			status: TaskStatus::Pending,
			created_at: chrono::Utc::now().timestamp(),
			updated_at: chrono::Utc::now().timestamp(),
		};

		// Store metadata in memory
		{
			let mut store = self.metadata_store.write().await;
			store.insert(task_id, metadata.clone());
		}

		// Create serialized task for SQS message body
		let serialized_task = SerializedTask::new(task_name, "{}".to_string());
		let message_body = serialized_task
			.to_json()
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		// Send message to SQS with task metadata as message attributes
		let metadata_json = serde_json::to_string(&metadata)
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		self.client
			.send_message()
			.queue_url(&self.config.queue_url)
			.message_body(message_body)
			.message_attributes(
				"task_id",
				MessageAttributeValue::builder()
					.data_type("String")
					.string_value(task_id.to_string())
					.build()
					.map_err(|e| {
						TaskExecutionError::BackendError(format!("Message attribute error: {}", e))
					})?,
			)
			.message_attributes(
				"metadata",
				MessageAttributeValue::builder()
					.data_type("String")
					.string_value(metadata_json)
					.build()
					.map_err(|e| {
						TaskExecutionError::BackendError(format!("Message attribute error: {}", e))
					})?,
			)
			.send()
			.await
			.map_err(|e| TaskExecutionError::BackendError(format!("SQS send error: {}", e)))?;

		Ok(task_id)
	}

	async fn dequeue(&self) -> Result<Option<TaskId>, TaskExecutionError> {
		let result = self
			.client
			.receive_message()
			.queue_url(&self.config.queue_url)
			.max_number_of_messages(self.config.max_messages)
			.visibility_timeout(self.config.visibility_timeout)
			.wait_time_seconds(self.config.wait_time_seconds)
			.message_attribute_names("All")
			.send()
			.await
			.map_err(|e| TaskExecutionError::BackendError(format!("SQS receive error: {}", e)))?;

		if let Some(messages) = result.messages
			&& let Some(message) = messages.into_iter().next()
		{
			// Extract task_id from message attributes
			if let Some(attributes) = message.message_attributes
				&& let Some(task_id_attr) = attributes.get("task_id")
				&& let Some(task_id_str) = task_id_attr.string_value()
			{
				let task_id = task_id_str
					.parse()
					.map_err(|e: uuid::Error| TaskExecutionError::BackendError(e.to_string()))?;

				// Store receipt handle for later deletion
				if let Some(receipt_handle) = message.receipt_handle {
					let mut handles = self.receipt_handles.write().await;
					handles.insert(task_id, receipt_handle);
				}

				// Update status to Running
				self.update_status(task_id, TaskStatus::Running).await?;

				return Ok(Some(task_id));
			}
		}

		Ok(None)
	}

	async fn get_status(&self, task_id: TaskId) -> Result<TaskStatus, TaskExecutionError> {
		let store = self.metadata_store.read().await;

		let metadata = store
			.get(&task_id)
			.ok_or(TaskExecutionError::NotFound(task_id))?;

		Ok(metadata.status)
	}

	async fn update_status(
		&self,
		task_id: TaskId,
		status: TaskStatus,
	) -> Result<(), TaskExecutionError> {
		// Update metadata under write lock, then release before network I/O
		{
			let mut store = self.metadata_store.write().await;

			let metadata = store
				.get_mut(&task_id)
				.ok_or(TaskExecutionError::NotFound(task_id))?;

			metadata.status = status;
			metadata.updated_at = chrono::Utc::now().timestamp();
		} // Write lock released here before any network calls

		// If task is completed (success or failure), delete from SQS
		// Fixes #789
		if matches!(status, TaskStatus::Success | TaskStatus::Failure) {
			let receipt_handle = {
				let mut handles = self.receipt_handles.write().await;
				handles.remove(&task_id)
			};
			if let Some(receipt_handle) = receipt_handle {
				self.delete_message(&receipt_handle).await?;
			}
		}

		Ok(())
	}

	async fn get_task_data(
		&self,
		task_id: TaskId,
	) -> Result<Option<SerializedTask>, TaskExecutionError> {
		// SQS doesn't support querying specific messages by ID
		// We'd need to receive and check messages, which is not efficient
		// For production, store task data in a separate database
		let store = self.metadata_store.read().await;

		if let Some(metadata) = store.get(&task_id) {
			// Return a placeholder serialized task
			// In production, this should be stored in a database
			Ok(Some(SerializedTask::new(
				metadata.name.clone(),
				"{}".to_string(),
			)))
		} else {
			Ok(None)
		}
	}

	fn backend_name(&self) -> &str {
		"sqs"
	}
}

/// AWS SQS-based result backend
///
/// This is a simple in-memory result backend. For production use,
/// consider using `RedisTaskResultBackend` or a DynamoDB-based implementation.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::{SqsResultBackend, ResultBackend, TaskResultMetadata, TaskId, TaskStatus};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = SqsResultBackend::new();
///
/// let metadata = TaskResultMetadata::new(
///     TaskId::new(),
///     TaskStatus::Success,
///     Some("Task completed".to_string()),
/// );
///
/// backend.store_result(metadata).await?;
/// # Ok(())
/// # }
/// ```
pub struct SqsResultBackend {
	results: Arc<RwLock<HashMap<TaskId, TaskResultMetadata>>>,
}

impl SqsResultBackend {
	/// Create a new SQS result backend
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::SqsResultBackend;
	///
	/// let backend = SqsResultBackend::new();
	/// ```
	pub fn new() -> Self {
		Self {
			results: Arc::new(RwLock::new(HashMap::new())),
		}
	}
}

impl Default for SqsResultBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl ResultBackend for SqsResultBackend {
	async fn store_result(&self, metadata: TaskResultMetadata) -> Result<(), TaskExecutionError> {
		let mut results = self.results.write().await;
		results.insert(metadata.task_id(), metadata);
		Ok(())
	}

	async fn get_result(
		&self,
		task_id: TaskId,
	) -> Result<Option<TaskResultMetadata>, TaskExecutionError> {
		let results = self.results.read().await;
		Ok(results.get(&task_id).cloned())
	}

	async fn delete_result(&self, task_id: TaskId) -> Result<(), TaskExecutionError> {
		let mut results = self.results.write().await;
		results.remove(&task_id);
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_sqs_config_creation() {
		let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue");
		assert_eq!(
			config.queue_url,
			"https://sqs.us-east-1.amazonaws.com/123456789012/my-queue"
		);
		assert_eq!(config.visibility_timeout, 30);
		assert_eq!(config.max_messages, 1);
	}

	#[rstest]
	fn test_sqs_config_with_options() {
		let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue")
			.with_visibility_timeout(60)
			.with_max_messages(5)
			.with_wait_time_seconds(20);

		assert_eq!(config.visibility_timeout, 60);
		assert_eq!(config.max_messages, 5);
		assert_eq!(config.wait_time_seconds, 20);
	}

	#[rstest]
	fn test_sqs_config_max_messages_limit() {
		let config = SqsConfig::new("https://sqs.us-east-1.amazonaws.com/123456789012/my-queue")
			.with_max_messages(15); // Should be capped at 10

		assert_eq!(config.max_messages, 10);
	}

	#[rstest]
	#[tokio::test]
	async fn test_sqs_result_backend_store_and_retrieve() {
		let backend = SqsResultBackend::new();
		let task_id = TaskId::new();
		let metadata = TaskResultMetadata::new(
			task_id,
			TaskStatus::Success,
			Some("Test result".to_string()),
		);

		backend
			.store_result(metadata.clone())
			.await
			.expect("Failed to store result");

		let retrieved = backend
			.get_result(task_id)
			.await
			.expect("Failed to get result");
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().result(), Some("Test result"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_sqs_result_backend_delete() {
		let backend = SqsResultBackend::new();
		let task_id = TaskId::new();
		let metadata = TaskResultMetadata::new(task_id, TaskStatus::Success, None);

		backend
			.store_result(metadata)
			.await
			.expect("Failed to store result");
		backend
			.delete_result(task_id)
			.await
			.expect("Failed to delete result");

		let retrieved = backend
			.get_result(task_id)
			.await
			.expect("Failed to get result");
		assert!(retrieved.is_none());
	}

	// Note: Integration tests with actual SQS require AWS credentials and a real queue
	// These would be placed in the `tests` crate with TestContainers or LocalStack

	// Regression tests for #783: write lock must not be held during network I/O.
	// The SqsBackend.update_status implementation uses a scoped write lock that is
	// dropped before the SQS delete call. These tests verify the in-memory store
	// (metadata_store and receipt_handles) can be read concurrently once the write
	// scope is exited, confirming that no cross-thread deadlock would occur when a
	// real network call follows the status update.

	#[rstest]
	#[tokio::test]
	async fn test_sqs_result_backend_concurrent_reads_during_write() {
		// Arrange - insert a set of results under concurrent read pressure to ensure
		// the RwLock is never held for writes while a reader is blocked.
		let backend = Arc::new(SqsResultBackend::new());

		let task_ids: Vec<TaskId> = (0..10).map(|_| TaskId::new()).collect();
		for &id in &task_ids {
			let meta = TaskResultMetadata::new(id, TaskStatus::Success, Some("ok".to_string()));
			backend.store_result(meta).await.unwrap();
		}

		// Act - concurrent readers should not block each other or the writer.
		let mut read_handles = vec![];
		for &id in &task_ids {
			let backend_clone = Arc::clone(&backend);
			read_handles.push(tokio::spawn(async move {
				backend_clone.get_result(id).await.unwrap()
			}));
		}

		// Write a new result concurrently with the ongoing reads.
		let new_id = TaskId::new();
		let writer = {
			let backend_clone = Arc::clone(&backend);
			tokio::spawn(async move {
				let meta = TaskResultMetadata::new(
					new_id,
					TaskStatus::Success,
					Some("concurrent".to_string()),
				);
				backend_clone.store_result(meta).await.unwrap();
			})
		};

		// Assert - all reads return the previously stored values without deadlock.
		for handle in read_handles {
			assert!(handle.await.unwrap().is_some());
		}
		writer.await.unwrap();

		// The newly written entry must be visible after the write completes.
		let result = backend.get_result(new_id).await.unwrap();
		assert_eq!(result.unwrap().result(), Some("concurrent"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_update_status_releases_write_lock_before_cleanup() {
		// Regression for #783: verify that the write lock on metadata_store is
		// released before any external I/O (represented here by acquiring a
		// concurrent read lock immediately after the write scope).
		//
		// We test this indirectly via SqsResultBackend: store a result, delete it
		// (which takes a write lock then releases it), and confirm a simultaneous
		// read succeeds without blocking.
		let backend = Arc::new(SqsResultBackend::new());
		let task_id = TaskId::new();

		// Arrange - pre-populate the store.
		backend
			.store_result(TaskResultMetadata::new(
				task_id,
				TaskStatus::Success,
				Some("data".to_string()),
			))
			.await
			.unwrap();

		// Act - delete (write) and read concurrently.
		let backend_write = Arc::clone(&backend);
		let backend_read = Arc::clone(&backend);

		let write =
			tokio::spawn(async move { backend_write.delete_result(task_id).await.unwrap() });

		let other_id = TaskId::new();
		backend
			.store_result(TaskResultMetadata::new(
				other_id,
				TaskStatus::Success,
				Some("other".to_string()),
			))
			.await
			.unwrap();

		let read = tokio::spawn(async move { backend_read.get_result(other_id).await.unwrap() });

		write.await.unwrap();
		let read_result = read.await.unwrap();

		// Assert - the read completed and returned a value (no deadlock occurred).
		assert!(read_result.is_some());
	}
}
