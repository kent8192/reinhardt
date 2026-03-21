//! Task backend implementations

use crate::{Task, TaskId, TaskStatus, registry::SerializedTask};
use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur during task execution in a backend.
#[derive(Debug, Error)]
pub enum TaskExecutionError {
	/// The task failed during execution.
	#[error("Task execution failed: {0}")]
	ExecutionFailed(String),

	/// The requested task was not found in the backend.
	#[error("Task not found: {0}")]
	NotFound(TaskId),

	/// A backend-specific error occurred.
	#[error("Backend error: {0}")]
	BackendError(String),
}

/// Alias for `TaskStatus` used in result contexts.
pub type ResultStatus = TaskStatus;
/// Alias for `TaskStatus` used in task result contexts.
pub type TaskResultStatus = TaskStatus;

/// Trait for task queue backends that handle enqueueing, dequeueing, and status tracking.
#[async_trait]
pub trait TaskBackend: Send + Sync {
	/// Enqueues a task and returns its assigned ID.
	async fn enqueue(&self, task: Box<dyn Task>) -> Result<TaskId, TaskExecutionError>;
	/// Dequeues the next available task ID, if any.
	async fn dequeue(&self) -> Result<Option<TaskId>, TaskExecutionError>;
	/// Retrieves the current status of a task by its ID.
	async fn get_status(&self, task_id: TaskId) -> Result<TaskStatus, TaskExecutionError>;
	/// Updates the status of a task.
	async fn update_status(
		&self,
		task_id: TaskId,
		status: TaskStatus,
	) -> Result<(), TaskExecutionError>;

	/// Get serialized task data by task ID
	///
	/// Returns the task data if found, None otherwise.
	async fn get_task_data(
		&self,
		task_id: TaskId,
	) -> Result<Option<SerializedTask>, TaskExecutionError>;

	/// Returns the name of this backend implementation.
	fn backend_name(&self) -> &str;
}

/// Registry of available task backends.
pub struct TaskBackends;

impl TaskBackends {
	/// Creates a new empty backend registry.
	pub fn new() -> Self {
		Self
	}
}

impl Default for TaskBackends {
	fn default() -> Self {
		Self::new()
	}
}

/// A no-op backend that discards all tasks. Useful for testing.
pub struct DummyBackend;

impl DummyBackend {
	/// Creates a new dummy backend.
	pub fn new() -> Self {
		Self
	}
}

impl Default for DummyBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl TaskBackend for DummyBackend {
	async fn enqueue(&self, _task: Box<dyn Task>) -> Result<TaskId, TaskExecutionError> {
		Ok(TaskId::new())
	}

	async fn dequeue(&self) -> Result<Option<TaskId>, TaskExecutionError> {
		Ok(None)
	}

	async fn get_status(&self, _task_id: TaskId) -> Result<TaskStatus, TaskExecutionError> {
		Ok(TaskStatus::Success)
	}

	async fn update_status(
		&self,
		_task_id: TaskId,
		_status: TaskStatus,
	) -> Result<(), TaskExecutionError> {
		Ok(())
	}

	async fn get_task_data(
		&self,
		_task_id: TaskId,
	) -> Result<Option<SerializedTask>, TaskExecutionError> {
		// DummyBackend doesn't store task data
		Ok(None)
	}

	fn backend_name(&self) -> &str {
		"dummy"
	}
}

/// A backend that executes tasks immediately upon enqueueing.
pub struct ImmediateBackend;

impl ImmediateBackend {
	/// Creates a new immediate backend.
	pub fn new() -> Self {
		Self
	}
}

impl Default for ImmediateBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl TaskBackend for ImmediateBackend {
	async fn enqueue(&self, _task: Box<dyn Task>) -> Result<TaskId, TaskExecutionError> {
		Ok(TaskId::new())
	}

	async fn dequeue(&self) -> Result<Option<TaskId>, TaskExecutionError> {
		Ok(None)
	}

	async fn get_status(&self, _task_id: TaskId) -> Result<TaskStatus, TaskExecutionError> {
		Ok(TaskStatus::Success)
	}

	async fn update_status(
		&self,
		_task_id: TaskId,
		_status: TaskStatus,
	) -> Result<(), TaskExecutionError> {
		Ok(())
	}

	async fn get_task_data(
		&self,
		_task_id: TaskId,
	) -> Result<Option<SerializedTask>, TaskExecutionError> {
		// ImmediateBackend doesn't store task data
		Ok(None)
	}

	fn backend_name(&self) -> &str {
		"immediate"
	}
}
