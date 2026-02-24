//! Task queue management

use crate::backend::TaskExecutionError;
use crate::{Task, TaskBackend, TaskId};

#[derive(Debug, Clone)]
pub struct QueueConfig {
	pub name: String,
	pub max_retries: u32,
}

impl QueueConfig {
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

pub struct TaskQueue;

impl TaskQueue {
	pub fn new() -> Self {
		Self
	}

	pub fn with_config(_config: QueueConfig) -> Self {
		Self
	}

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
