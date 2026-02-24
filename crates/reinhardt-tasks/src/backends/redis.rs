//! Redis-based task backend implementation

use crate::{
	Task, TaskExecutionError, TaskId, TaskStatus,
	result::{ResultBackend, TaskResultMetadata},
};
use async_trait::async_trait;
use redis::{AsyncCommands, RedisError, aio::ConnectionManager};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Task metadata for Redis storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskMetadata {
	id: TaskId,
	name: String,
	status: TaskStatus,
	created_at: i64,
	updated_at: i64,
}

/// Redis-based task backend
///
/// Stores tasks in Redis with status tracking.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::RedisTaskBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = RedisTaskBackend::new("redis://127.0.0.1/").await?;
/// # Ok(())
/// # }
/// ```
pub struct RedisTaskBackend {
	connection: Arc<ConnectionManager>,
	key_prefix: String,
}

impl RedisTaskBackend {
	/// Create a new Redis backend
	///
	/// # Arguments
	///
	/// * `redis_url` - Redis connection URL (e.g., "redis://127.0.0.1/")
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RedisTaskBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = RedisTaskBackend::new("redis://localhost/").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(redis_url: &str) -> Result<Self, RedisError> {
		let client = redis::Client::open(redis_url)?;
		let connection = ConnectionManager::new(client).await?;

		Ok(Self {
			connection: Arc::new(connection),
			key_prefix: "reinhardt:tasks:".to_string(),
		})
	}

	/// Create a new Redis backend with custom key prefix
	///
	/// # Arguments
	///
	/// * `redis_url` - Redis connection URL
	/// * `key_prefix` - Custom key prefix for Redis keys
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RedisTaskBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = RedisTaskBackend::with_prefix(
	///     "redis://localhost/",
	///     "myapp:tasks:".to_string()
	/// ).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn with_prefix(redis_url: &str, key_prefix: String) -> Result<Self, RedisError> {
		let client = redis::Client::open(redis_url)?;
		let connection = ConnectionManager::new(client).await?;

		Ok(Self {
			connection: Arc::new(connection),
			key_prefix,
		})
	}

	/// Get Redis key for a task
	fn task_key(&self, task_id: TaskId) -> String {
		format!("{}task:{}", self.key_prefix, task_id)
	}

	/// Get Redis key for task queue
	fn queue_key(&self) -> String {
		format!("{}queue", self.key_prefix)
	}
}

#[async_trait]
impl crate::backend::TaskBackend for RedisTaskBackend {
	// Fixes #790: use MULTI/EXEC transaction to atomically execute SET + RPUSH
	async fn enqueue(&self, task: Box<dyn Task>) -> Result<TaskId, TaskExecutionError> {
		let task_id = task.id();
		let task_name = task.name().to_string();

		let metadata = TaskMetadata {
			id: task_id,
			name: task_name,
			status: TaskStatus::Pending,
			created_at: chrono::Utc::now().timestamp(),
			updated_at: chrono::Utc::now().timestamp(),
		};

		let metadata_json = serde_json::to_string(&metadata)
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		let mut conn = (*self.connection).clone();

		// Atomically store metadata and enqueue task ID using MULTI/EXEC
		redis::pipe()
			.atomic()
			.set(self.task_key(task_id), metadata_json)
			.rpush(self.queue_key(), task_id.to_string())
			.query_async::<()>(&mut conn)
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		Ok(task_id)
	}

	async fn dequeue(&self) -> Result<Option<TaskId>, TaskExecutionError> {
		let mut conn = (*self.connection).clone();

		let task_id_str: Option<String> = conn
			.lpop(self.queue_key(), None)
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		match task_id_str {
			Some(id_str) => {
				let task_id = id_str
					.parse()
					.map_err(|e: uuid::Error| TaskExecutionError::BackendError(e.to_string()))?;
				Ok(Some(task_id))
			}
			None => Ok(None),
		}
	}

	async fn get_status(&self, task_id: TaskId) -> Result<TaskStatus, TaskExecutionError> {
		let mut conn = (*self.connection).clone();

		let metadata_json: Option<String> = conn
			.get(self.task_key(task_id))
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		match metadata_json {
			Some(json) => {
				let metadata: TaskMetadata = serde_json::from_str(&json)
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;
				Ok(metadata.status)
			}
			None => Err(TaskExecutionError::NotFound(task_id)),
		}
	}

	async fn update_status(
		&self,
		task_id: TaskId,
		status: TaskStatus,
	) -> Result<(), TaskExecutionError> {
		let mut conn = (*self.connection).clone();

		let metadata_json: Option<String> = conn
			.get(self.task_key(task_id))
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		match metadata_json {
			Some(json) => {
				let mut metadata: TaskMetadata = serde_json::from_str(&json)
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

				metadata.status = status;
				metadata.updated_at = chrono::Utc::now().timestamp();

				let updated_json = serde_json::to_string(&metadata)
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

				let _: () = conn
					.set(self.task_key(task_id), updated_json)
					.await
					.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

				Ok(())
			}
			None => Err(TaskExecutionError::NotFound(task_id)),
		}
	}

	async fn get_task_data(
		&self,
		task_id: TaskId,
	) -> Result<Option<crate::registry::SerializedTask>, TaskExecutionError> {
		let mut conn = (*self.connection).clone();

		let metadata_json: Option<String> = conn
			.get(self.task_key(task_id))
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		match metadata_json {
			Some(json) => {
				let metadata: TaskMetadata = serde_json::from_str(&json)
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

				// Return a placeholder serialized task
				// In production, task data should be stored separately
				Ok(Some(crate::registry::SerializedTask::new(
					metadata.name,
					"{}".to_string(),
				)))
			}
			None => Ok(None),
		}
	}

	fn backend_name(&self) -> &str {
		"redis"
	}
}

/// Redis-based result backend for task result persistence
///
/// # Examples
///
/// ```no_run
/// use reinhardt_tasks::{RedisTaskResultBackend, ResultBackend, TaskResultMetadata, TaskId, TaskStatus};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = RedisTaskResultBackend::new("redis://127.0.0.1/").await?;
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
pub struct RedisTaskResultBackend {
	connection: Arc<ConnectionManager>,
	key_prefix: String,
	default_ttl: i64,
}

impl RedisTaskResultBackend {
	/// Create a new Redis result backend
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RedisTaskResultBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = RedisTaskResultBackend::new("redis://localhost/").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(redis_url: &str) -> Result<Self, RedisError> {
		let client = redis::Client::open(redis_url)?;
		let connection = ConnectionManager::new(client).await?;

		Ok(Self {
			connection: Arc::new(connection),
			key_prefix: "reinhardt:results:".to_string(),
			default_ttl: 86400, // 24 hours
		})
	}

	/// Create a new Redis result backend with custom settings
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_tasks::RedisTaskResultBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = RedisTaskResultBackend::with_config(
	///     "redis://localhost/",
	///     "myapp:results:".to_string(),
	///     3600, // 1 hour TTL
	/// ).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn with_config(
		redis_url: &str,
		key_prefix: String,
		default_ttl: i64,
	) -> Result<Self, RedisError> {
		let client = redis::Client::open(redis_url)?;
		let connection = ConnectionManager::new(client).await?;

		Ok(Self {
			connection: Arc::new(connection),
			key_prefix,
			default_ttl,
		})
	}

	fn result_key(&self, task_id: TaskId) -> String {
		format!("{}task:{}", self.key_prefix, task_id)
	}
}

#[async_trait]
impl ResultBackend for RedisTaskResultBackend {
	async fn store_result(&self, metadata: TaskResultMetadata) -> Result<(), TaskExecutionError> {
		let task_id = metadata.task_id();
		let metadata_json = serde_json::to_string(&metadata)
			.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;

		let mut conn = (*self.connection).clone();
		let key = self.result_key(task_id);

		let _: () = conn
			.set_ex(&key, metadata_json, self.default_ttl as u64)
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		Ok(())
	}

	async fn get_result(
		&self,
		task_id: TaskId,
	) -> Result<Option<TaskResultMetadata>, TaskExecutionError> {
		let mut conn = (*self.connection).clone();
		let key = self.result_key(task_id);

		let metadata_json: Option<String> = conn
			.get(&key)
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		match metadata_json {
			Some(json) => {
				let metadata: TaskResultMetadata = serde_json::from_str(&json)
					.map_err(|e| TaskExecutionError::BackendError(e.to_string()))?;
				Ok(Some(metadata))
			}
			None => Ok(None),
		}
	}

	async fn delete_result(&self, task_id: TaskId) -> Result<(), TaskExecutionError> {
		let mut conn = (*self.connection).clone();
		let key = self.result_key(task_id);

		let _: () = conn
			.del(&key)
			.await
			.map_err(|e: RedisError| TaskExecutionError::BackendError(e.to_string()))?;

		Ok(())
	}
}
