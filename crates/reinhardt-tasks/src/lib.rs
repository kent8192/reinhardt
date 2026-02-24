//! # Reinhardt Background Tasks
//!
//! Celery-inspired background task queue for Reinhardt framework.
//!
//! ## Features
//!
//! - Async task execution
//! - Task scheduling (cron-like)
//! - Task retries with exponential backoff
//! - Task priority
//! - Task chaining
//! - Task dependencies and DAG execution
//! - Result backend
//! - Task execution metrics and monitoring
//! - Worker load balancing (Round-robin, Least-connections, Weighted, Random)
//! - Webhook notifications for task completion
//!
//!
//! ## Example
//!
//! ```rust,no_run
//! # use reinhardt_tasks::TaskResult;
//! # trait Task {}
//! # #[derive(Clone)]
//! # struct SendEmailTask { to: String, subject: String, body: String }
//! # trait TaskQueue {
//! #     fn new() -> Self;
//! #     async fn enqueue(&self, task: SendEmailTask) -> TaskResult<()>;
//! # }
//! # struct QueueImpl;
//! # impl TaskQueue for QueueImpl {
//! #     fn new() -> Self { QueueImpl }
//! #     async fn enqueue(&self, task: SendEmailTask) -> TaskResult<()> { Ok(()) }
//! # }
//! #
//! # #[tokio::main]
//! # async fn main() -> TaskResult<()> {
//! // Example: Define a task
//! // struct SendEmailTask {
//! //     to: String,
//! //     subject: String,
//! //     body: String,
//! // }
//!
//! // #[async_trait]
//! // impl TaskExecutor for SendEmailTask {
//! //     async fn execute(&self) -> TaskResult<()> {
//!         // Send email
//! //         Ok(())
//! //     }
//! // }
//!
//! // Queue the task
//! let queue = QueueImpl::new();
//! queue.enqueue(SendEmailTask {
//!     to: "user@example.com".to_string(),
//!     subject: "Hello".to_string(),
//!     body: "Test email".to_string(),
//! }).await?;
//! # Ok(())
//! # }
//! ```

pub mod backend;
pub mod backends;
pub mod chain;
pub mod dag;
pub mod load_balancer;
pub mod locking;
pub mod metrics;
pub mod priority_queue;
pub mod queue;
pub mod registry;
pub mod result;
pub mod retry;
pub mod scheduler;
pub mod task;
pub mod webhook;
pub mod worker;

pub use backend::{
	DummyBackend, ImmediateBackend, ResultStatus, TaskBackend, TaskBackends, TaskExecutionError,
	TaskResultStatus,
};

#[cfg(feature = "redis-backend")]
pub use backends::RedisTaskBackend;

#[cfg(feature = "database-backend")]
pub use backends::SqliteBackend;

#[cfg(feature = "sqs-backend")]
pub use backends::{SqsBackend, SqsConfig};

#[cfg(feature = "rabbitmq-backend")]
pub use backends::{RabbitMQBackend, RabbitMQConfig};
pub use chain::{ChainStatus, TaskChain, TaskChainBuilder};
pub use dag::{TaskDAG, TaskNode, TaskNodeStatus};
pub use load_balancer::{LoadBalancer, LoadBalancingStrategy, WorkerId, WorkerInfo, WorkerMetrics};
pub use locking::{MemoryTaskLock, TaskLock};

#[cfg(feature = "redis-backend")]
pub use locking::RedisTaskLock;
pub use metrics::{MetricsSnapshot, TaskCounts, TaskMetrics, WorkerStats};
pub use priority_queue::{Priority, PriorityTaskQueue};
pub use queue::{QueueConfig, TaskQueue};
pub use registry::{SerializedTask, TaskFactory, TaskRegistry};
pub use result::{
	MemoryResultBackend, ResultBackend, TaskOutput, TaskResult as TaskResultBackend,
	TaskResultMetadata,
};

#[cfg(feature = "redis-backend")]
pub use backends::redis::RedisTaskResultBackend;

#[cfg(feature = "database-backend")]
pub use backends::sqlite::SqliteResultBackend;

#[cfg(feature = "sqs-backend")]
pub use backends::sqs::SqsResultBackend;
pub use retry::{RetryState, RetryStrategy};
pub use scheduler::{CronSchedule, Schedule, Scheduler};
pub use task::{
	DEFAULT_TASK_QUEUE_NAME, TASK_MAX_PRIORITY, TASK_MIN_PRIORITY, Task, TaskExecutor, TaskId,
	TaskPriority, TaskStatus,
};
pub use webhook::{
	HttpWebhookSender, RetryConfig, TaskStatus as WebhookTaskStatus, WebhookConfig, WebhookError,
	WebhookEvent, WebhookSender, is_blocked_ip, validate_resolved_ips, validate_webhook_url,
};
pub use worker::{Worker, WorkerConfig};

use thiserror::Error;

/// Result type for task operations
pub type TaskResult<T> = Result<T, TaskError>;

/// Task-related errors
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::TaskError;
///
/// let error = TaskError::ExecutionFailed("Database connection failed".to_string());
/// assert_eq!(error.to_string(), "Task execution failed: Database connection failed");
/// ```
#[derive(Debug, Error)]
pub enum TaskError {
	/// Task execution failed
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskError;
	///
	/// let error = TaskError::ExecutionFailed("Network error".to_string());
	/// ```
	#[error("Task execution failed: {0}")]
	ExecutionFailed(String),

	/// Task not found
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskError;
	///
	/// let error = TaskError::TaskNotFound("task-123".to_string());
	/// assert_eq!(error.to_string(), "Task not found: task-123");
	/// ```
	#[error("Task not found: {0}")]
	TaskNotFound(String),

	/// Queue error
	#[error("Queue error: {0}")]
	QueueError(String),

	/// Serialization error
	#[error("Serialization error: {0}")]
	SerializationError(String),

	/// Task timeout
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskError;
	///
	/// let error = TaskError::Timeout;
	/// assert_eq!(error.to_string(), "Task timeout");
	/// ```
	#[error("Task timeout")]
	Timeout,

	/// Max retries exceeded
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskError;
	///
	/// let error = TaskError::MaxRetriesExceeded;
	/// assert_eq!(error.to_string(), "Max retries exceeded");
	/// ```
	#[error("Max retries exceeded")]
	MaxRetriesExceeded,
}
