//! Task queue management

use crate::backend::TaskExecutionError;
use crate::{Task, TaskBackend, TaskId};

/// Configuration for a task queue.
#[derive(Debug, Clone)]
pub struct QueueConfig {
	/// Name of the queue.
	pub name: String,
	/// Maximum number of retry attempts for failed tasks.
	pub max_retries: u32,
}

impl QueueConfig {
	/// Creates a new queue configuration with the given name and default retry count.
	pub fn new(name: String) -> Self {
		Self {
			name,
			max_retries: 3,
		}
	}
}

impl Default for QueueConfig {
	fn default() -> Self {
		Self::new("default".to_string())
	}
}

/// A task queue that delegates to a backend for task storage and retrieval.
pub struct TaskQueue;

impl TaskQueue {
	/// Creates a new task queue with default configuration.
	pub fn new() -> Self {
		Self
	}

	/// Creates a new task queue with the given configuration.
	pub fn with_config(_config: QueueConfig) -> Self {
		Self
	}

	/// Enqueues a task for execution through the specified backend.
	pub async fn enqueue(
		&self,
		task: Box<dyn Task>,
		backend: &dyn TaskBackend,
	) -> Result<TaskId, TaskExecutionError> {
		backend.enqueue(task).await
	}
}

impl Default for TaskQueue {
	fn default() -> Self {
		Self::new()
	}
}
