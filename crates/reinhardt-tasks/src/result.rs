//! Task result types and persistence

use crate::{TaskExecutionError, TaskId, TaskStatus};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Task execution output
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{TaskOutput, TaskId};
///
/// let output = TaskOutput::new(TaskId::new(), "Processing completed".to_string());
/// assert_eq!(output.result(), "Processing completed");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
	task_id: TaskId,
	result: String,
}

impl TaskOutput {
	/// Create a new task output
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskOutput, TaskId};
	///
	/// let task_id = TaskId::new();
	/// let output = TaskOutput::new(task_id, "Success".to_string());
	/// ```
	pub fn new(task_id: TaskId, result: String) -> Self {
		Self { task_id, result }
	}

	/// Get the task ID
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskOutput, TaskId};
	///
	/// let task_id = TaskId::new();
	/// let output = TaskOutput::new(task_id, "Done".to_string());
	/// assert_eq!(output.task_id(), task_id);
	/// ```
	pub fn task_id(&self) -> TaskId {
		self.task_id
	}

	/// Get the result string
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskOutput, TaskId};
	///
	/// let output = TaskOutput::new(TaskId::new(), "Result data".to_string());
	/// assert_eq!(output.result(), "Result data");
	/// ```
	pub fn result(&self) -> &str {
		&self.result
	}
}

/// Task result type
pub type TaskResult = Result<TaskOutput, String>;

/// Task result metadata with status information
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{TaskResultMetadata, TaskId, TaskStatus};
///
/// let metadata = TaskResultMetadata::new(
///     TaskId::new(),
///     TaskStatus::Success,
///     Some("Task completed successfully".to_string()),
/// );
///
/// assert_eq!(metadata.status(), TaskStatus::Success);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultMetadata {
	task_id: TaskId,
	status: TaskStatus,
	result: Option<String>,
	error: Option<String>,
	created_at: i64,
}

impl TaskResultMetadata {
	/// Create a new task result metadata
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskResultMetadata, TaskId, TaskStatus};
	///
	/// let metadata = TaskResultMetadata::new(
	///     TaskId::new(),
	///     TaskStatus::Success,
	///     Some("Done".to_string()),
	/// );
	/// ```
	pub fn new(task_id: TaskId, status: TaskStatus, result: Option<String>) -> Self {
		Self {
			task_id,
			status,
			result,
			error: None,
			created_at: chrono::Utc::now().timestamp(),
		}
	}

	/// Create a failed task result
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskResultMetadata, TaskId, TaskStatus};
	///
	/// let metadata = TaskResultMetadata::with_error(
	///     TaskId::new(),
	///     "Database connection failed".to_string(),
	/// );
	///
	/// assert_eq!(metadata.status(), TaskStatus::Failure);
	/// ```
	pub fn with_error(task_id: TaskId, error: String) -> Self {
		Self {
			task_id,
			status: TaskStatus::Failure,
			result: None,
			error: Some(error),
			created_at: chrono::Utc::now().timestamp(),
		}
	}

	/// Set the error message while preserving result and status fields.
	pub fn set_error(&mut self, error: String) {
		self.error = Some(error);
	}

	/// Get the task ID
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskResultMetadata, TaskId, TaskStatus};
	///
	/// let task_id = TaskId::new();
	/// let metadata = TaskResultMetadata::new(task_id, TaskStatus::Success, None);
	/// assert_eq!(metadata.task_id(), task_id);
	/// ```
	pub fn task_id(&self) -> TaskId {
		self.task_id
	}

	/// Get the task status
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskResultMetadata, TaskId, TaskStatus};
	///
	/// let metadata = TaskResultMetadata::new(
	///     TaskId::new(),
	///     TaskStatus::Success,
	///     None,
	/// );
	/// assert_eq!(metadata.status(), TaskStatus::Success);
	/// ```
	pub fn status(&self) -> TaskStatus {
		self.status
	}

	/// Get the result if available
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskResultMetadata, TaskId, TaskStatus};
	///
	/// let metadata = TaskResultMetadata::new(
	///     TaskId::new(),
	///     TaskStatus::Success,
	///     Some("Result".to_string()),
	/// );
	/// assert_eq!(metadata.result(), Some("Result"));
	/// ```
	pub fn result(&self) -> Option<&str> {
		self.result.as_deref()
	}

	/// Get the error if available
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskResultMetadata, TaskId};
	///
	/// let metadata = TaskResultMetadata::with_error(
	///     TaskId::new(),
	///     "Error occurred".to_string(),
	/// );
	/// assert_eq!(metadata.error(), Some("Error occurred"));
	/// ```
	pub fn error(&self) -> Option<&str> {
		self.error.as_deref()
	}

	/// Get the creation timestamp
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{TaskResultMetadata, TaskId, TaskStatus};
	///
	/// let metadata = TaskResultMetadata::new(TaskId::new(), TaskStatus::Success, None);
	/// let timestamp = metadata.created_at();
	/// assert!(timestamp > 0);
	/// ```
	pub fn created_at(&self) -> i64 {
		self.created_at
	}
}

/// Result backend trait for persisting task results
///
/// # Examples
///
/// ```
/// use reinhardt_tasks::{ResultBackend, TaskResultMetadata, TaskId, TaskStatus};
/// use async_trait::async_trait;
///
/// struct MyResultBackend;
///
/// #[async_trait]
/// impl ResultBackend for MyResultBackend {
///     async fn store_result(
///         &self,
///         metadata: TaskResultMetadata,
///     ) -> Result<(), reinhardt_tasks::TaskExecutionError> {
///         // Store the result
///         Ok(())
///     }
///
///     async fn get_result(
///         &self,
///         task_id: TaskId,
///     ) -> Result<Option<TaskResultMetadata>, reinhardt_tasks::TaskExecutionError> {
///         // Retrieve the result
///         Ok(None)
///     }
///
///     async fn delete_result(
///         &self,
///         task_id: TaskId,
///     ) -> Result<(), reinhardt_tasks::TaskExecutionError> {
///         // Delete the result
///         Ok(())
///     }
/// }
///
/// # async fn test_backend() {
/// let backend = MyResultBackend;
/// let task_id = TaskId::new();
/// let metadata = TaskResultMetadata::new(
///     task_id,
///     TaskStatus::Success,
///     Some("Task completed".to_string()),
/// );
///
/// // Test store and retrieve
/// assert!(backend.store_result(metadata.clone()).await.is_ok());
/// let result = backend.get_result(task_id).await.unwrap();
/// assert!(result.is_none()); // Our implementation returns None
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(test_backend());
/// ```
#[async_trait]
pub trait ResultBackend: Send + Sync {
	/// Store a task result
	async fn store_result(&self, metadata: TaskResultMetadata) -> Result<(), TaskExecutionError>;

	/// Get a task result
	async fn get_result(
		&self,
		task_id: TaskId,
	) -> Result<Option<TaskResultMetadata>, TaskExecutionError>;

	/// Delete a task result
	async fn delete_result(&self, task_id: TaskId) -> Result<(), TaskExecutionError>;
}

/// In-memory result backend for testing
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::MemoryResultBackend;
///
/// let backend = MemoryResultBackend::new();
/// ```
pub struct MemoryResultBackend {
	results:
		std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<TaskId, TaskResultMetadata>>>,
}

impl MemoryResultBackend {
	/// Create a new in-memory result backend
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::MemoryResultBackend;
	///
	/// let backend = MemoryResultBackend::new();
	/// ```
	pub fn new() -> Self {
		Self {
			results: std::sync::Arc::new(
				tokio::sync::RwLock::new(std::collections::HashMap::new()),
			),
		}
	}
}

impl Default for MemoryResultBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl ResultBackend for MemoryResultBackend {
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

	#[test]
	fn test_task_output() {
		let task_id = TaskId::new();
		let output = TaskOutput::new(task_id, "test result".to_string());
		assert_eq!(output.task_id(), task_id);
		assert_eq!(output.result(), "test result");
	}

	#[test]
	fn test_task_result_metadata() {
		let task_id = TaskId::new();
		let metadata =
			TaskResultMetadata::new(task_id, TaskStatus::Success, Some("success".to_string()));

		assert_eq!(metadata.task_id(), task_id);
		assert_eq!(metadata.status(), TaskStatus::Success);
		assert_eq!(metadata.result(), Some("success"));
		assert_eq!(metadata.error(), None);
	}

	#[test]
	fn test_task_result_metadata_with_error() {
		let task_id = TaskId::new();
		let metadata = TaskResultMetadata::with_error(task_id, "error occurred".to_string());

		assert_eq!(metadata.status(), TaskStatus::Failure);
		assert_eq!(metadata.error(), Some("error occurred"));
		assert_eq!(metadata.result(), None);
	}

	#[test]
	fn test_set_error_preserves_result_and_status() {
		// Arrange
		let task_id = TaskId::new();
		let mut metadata =
			TaskResultMetadata::new(task_id, TaskStatus::Success, Some("my result".to_string()));

		// Act
		metadata.set_error("something went wrong".to_string());

		// Assert
		assert_eq!(metadata.status(), TaskStatus::Success);
		assert_eq!(metadata.result(), Some("my result"));
		assert_eq!(metadata.error(), Some("something went wrong"));
	}

	#[tokio::test]
	async fn test_memory_result_backend() {
		let backend = MemoryResultBackend::new();
		let task_id = TaskId::new();

		let metadata =
			TaskResultMetadata::new(task_id, TaskStatus::Success, Some("result".to_string()));

		// Store result
		backend.store_result(metadata.clone()).await.unwrap();

		// Get result
		let retrieved = backend.get_result(task_id).await.unwrap();
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().result(), Some("result"));

		// Delete result
		backend.delete_result(task_id).await.unwrap();
		let deleted = backend.get_result(task_id).await.unwrap();
		assert!(deleted.is_none());
	}
}
