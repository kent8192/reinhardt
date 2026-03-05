//! Task definitions and execution

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const DEFAULT_TASK_QUEUE_NAME: &str = "default";
pub const TASK_MIN_PRIORITY: i32 = 0;
pub const TASK_MAX_PRIORITY: i32 = 9;

/// Unique identifier for a task
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::TaskId;
///
/// let id1 = TaskId::new();
/// let id2 = TaskId::new();
/// assert_ne!(id1, id2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub uuid::Uuid);

impl TaskId {
	/// Create a new unique task ID
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskId;
	///
	/// let id = TaskId::new();
	/// println!("Task ID: {}", id);
	/// ```
	pub fn new() -> Self {
		Self(uuid::Uuid::new_v4())
	}
}

impl Default for TaskId {
	fn default() -> Self {
		Self::new()
	}
}

impl fmt::Display for TaskId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl FromStr for TaskId {
	type Err = uuid::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self(uuid::Uuid::parse_str(s)?))
	}
}

/// Status of a task
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::TaskStatus;
///
/// let status = TaskStatus::Pending;
/// assert_eq!(status, TaskStatus::Pending);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
	Pending,
	Running,
	Success,
	Failure,
	Retry,
}

/// Task priority (0-9, where 9 is highest)
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::TaskPriority;
///
/// let high = TaskPriority::new(9);
/// let low = TaskPriority::new(0);
/// assert!(high > low);
/// assert_eq!(high.value(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TaskPriority(i32);

impl TaskPriority {
	/// Create a new task priority, clamped to valid range (0-9)
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskPriority;
	///
	/// let p1 = TaskPriority::new(5);
	/// assert_eq!(p1.value(), 5);
	///
	/// // Out of range values are clamped
	/// let p2 = TaskPriority::new(100);
	/// assert_eq!(p2.value(), 9);
	///
	/// let p3 = TaskPriority::new(-10);
	/// assert_eq!(p3.value(), 0);
	/// ```
	pub fn new(priority: i32) -> Self {
		Self(priority.clamp(TASK_MIN_PRIORITY, TASK_MAX_PRIORITY))
	}

	/// Get the priority value
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskPriority;
	///
	/// let priority = TaskPriority::new(7);
	/// assert_eq!(priority.value(), 7);
	/// ```
	pub fn value(&self) -> i32 {
		self.0
	}
}

impl Default for TaskPriority {
	fn default() -> Self {
		Self(5)
	}
}

pub trait Task: Send + Sync {
	fn id(&self) -> TaskId;
	fn name(&self) -> &str;
	fn priority(&self) -> TaskPriority {
		TaskPriority::default()
	}
}

#[async_trait]
pub trait TaskExecutor: Task {
	async fn execute(&self) -> crate::TaskResult<()>;
}
