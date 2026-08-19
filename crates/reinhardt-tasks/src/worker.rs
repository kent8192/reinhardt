//! Task worker

#![allow(deprecated)] // WorkerConfig/WebhookConfig are deprecated; still constructed internally.

use crate::{
	TaskBackend, TaskStatus,
	locking::TaskLock,
	registry::TaskRegistry,
	result::{ResultBackend, TaskResultMetadata},
	webhook::{HttpWebhookSender, WebhookConfig, WebhookEvent, WebhookSender},
};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Worker configuration
///
/// Controls worker behavior including name, concurrency, polling interval, and webhook notifications.
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::WorkerConfig;
/// use std::time::Duration;
///
/// let config = WorkerConfig::new("my-worker".to_string())
///     .with_concurrency(8)
///     .with_poll_interval(Duration::from_millis(100));
///
/// assert_eq!(config.name, "my-worker");
/// assert_eq!(config.concurrency, 8);
/// ```
#[deprecated(
	since = "0.2.0",
	note = "Use `WorkerSettings` with the `#[settings]` macro instead."
)]
#[derive(Debug, Clone)]
pub struct WorkerConfig {
	/// Name of this worker instance.
	pub name: String,
	/// Maximum number of tasks to process concurrently.
	pub concurrency: usize,
	/// Interval between polling the backend for new tasks.
	pub poll_interval: Duration,
	/// Webhook configurations for task completion notifications.
	pub webhook_configs: Vec<WebhookConfig>,
}

impl WorkerConfig {
	/// Create a new worker configuration with default values
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::WorkerConfig;
	///
	/// let config = WorkerConfig::new("worker-1".to_string());
	/// assert_eq!(config.name, "worker-1");
	/// assert_eq!(config.concurrency, 4);
	/// ```
	pub fn new(name: String) -> Self {
		Self {
			name,
			concurrency: 4,
			poll_interval: Duration::from_secs(1),
			webhook_configs: Vec::new(),
		}
	}

	/// Set the concurrency level
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::WorkerConfig;
	///
	/// let config = WorkerConfig::new("worker".to_string()).with_concurrency(8);
	/// assert_eq!(config.concurrency, 8);
	/// ```
	pub fn with_concurrency(mut self, concurrency: usize) -> Self {
		self.concurrency = concurrency;
		self
	}

	/// Set the poll interval
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::WorkerConfig;
	/// use std::time::Duration;
	///
	/// let config = WorkerConfig::new("worker".to_string())
	///     .with_poll_interval(Duration::from_millis(500));
	/// assert_eq!(config.poll_interval, Duration::from_millis(500));
	/// ```
	pub fn with_poll_interval(mut self, interval: Duration) -> Self {
		self.poll_interval = interval;
		self
	}

	/// Add a webhook configuration
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{WorkerConfig, webhook::WebhookConfig};
	/// use std::time::Duration;
	///
	/// let webhook_config = WebhookConfig {
	///     url: "https://example.com/webhook".to_string(),
	///     method: "POST".to_string(),
	///     headers: Default::default(),
	///     timeout: Duration::from_secs(5),
	///     retry_config: Default::default(),
	/// };
	///
	/// let config = WorkerConfig::new("worker".to_string())
	///     .with_webhook(webhook_config);
	/// assert_eq!(config.webhook_configs.len(), 1);
	/// ```
	pub fn with_webhook(mut self, webhook_config: WebhookConfig) -> Self {
		self.webhook_configs.push(webhook_config);
		self
	}

	/// Set multiple webhook configurations
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{WorkerConfig, webhook::WebhookConfig};
	///
	/// let webhooks = vec![
	///     WebhookConfig::default(),
	///     WebhookConfig::default(),
	/// ];
	///
	/// let config = WorkerConfig::new("worker".to_string())
	///     .with_webhooks(webhooks);
	/// assert_eq!(config.webhook_configs.len(), 2);
	/// ```
	pub fn with_webhooks(mut self, webhook_configs: Vec<WebhookConfig>) -> Self {
		self.webhook_configs = webhook_configs;
		self
	}
}

impl Default for WorkerConfig {
	fn default() -> Self {
		Self::new("worker".to_string())
	}
}

/// Task worker
///
/// Polls the backend for tasks and executes them concurrently.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_tasks::{Worker, WorkerConfig, DummyBackend};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = WorkerConfig::new("worker-1".to_string());
/// let worker = Arc::new(Worker::new(config));
/// let backend = Arc::new(DummyBackend::new());
///
/// // Start worker in background
/// let handle = tokio::spawn(async move {
///     worker.run(backend).await
/// });
///
/// // Later: stop the worker
/// handle.abort();
/// # Ok(())
/// # }
/// ```
pub struct Worker {
	config: WorkerConfig,
	// Stateful shutdown signal. A `watch` channel retains the latest value, so a
	// `stop()` that fires before a consumer first polls its receiver is still
	// observed (unlike a `broadcast` channel, where a notification sent while no
	// receiver is registered is lost). `false` means "keep running", `true` means
	// "stop". See issues #2 and #4 in the concurrency review.
	shutdown_tx: watch::Sender<bool>,
	registry: Option<Arc<TaskRegistry>>,
	task_lock: Option<Arc<dyn TaskLock>>,
	result_backend: Option<Arc<dyn ResultBackend>>,
	webhook_senders: Vec<Arc<dyn WebhookSender>>,
}

impl Worker {
	/// Create a new worker
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{Worker, WorkerConfig};
	///
	/// let config = WorkerConfig::new("worker-1".to_string());
	/// let worker = Worker::new(config.clone());
	/// ```
	pub fn new(config: WorkerConfig) -> Self {
		let (shutdown_tx, _) = watch::channel(false);

		// Create webhook senders from configuration
		let webhook_senders: Vec<Arc<dyn WebhookSender>> = config
			.webhook_configs
			.iter()
			.map(|webhook_config| {
				Arc::new(HttpWebhookSender::new(webhook_config.clone())) as Arc<dyn WebhookSender>
			})
			.collect();

		Self {
			config,
			shutdown_tx,
			registry: None,
			task_lock: None,
			result_backend: None,
			webhook_senders,
		}
	}

	/// Set the task registry for dynamic task dispatch
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{Worker, WorkerConfig, TaskRegistry};
	/// use std::sync::Arc;
	///
	/// let worker = Worker::new(WorkerConfig::default())
	///     .with_registry(Arc::new(TaskRegistry::new()));
	/// ```
	pub fn with_registry(mut self, registry: Arc<TaskRegistry>) -> Self {
		self.registry = Some(registry);
		self
	}

	/// Set the task lock for distributed task execution
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{Worker, WorkerConfig, MemoryTaskLock};
	/// use std::sync::Arc;
	///
	/// let worker = Worker::new(WorkerConfig::default())
	///     .with_lock(Arc::new(MemoryTaskLock::new()));
	/// ```
	pub fn with_lock(mut self, task_lock: Arc<dyn TaskLock>) -> Self {
		self.task_lock = Some(task_lock);
		self
	}

	/// Set the result backend for storing task results
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{Worker, WorkerConfig, MemoryResultBackend};
	/// use std::sync::Arc;
	///
	/// let worker = Worker::new(WorkerConfig::default())
	///     .with_result_backend(Arc::new(MemoryResultBackend::new()));
	/// ```
	pub fn with_result_backend(mut self, result_backend: Arc<dyn ResultBackend>) -> Self {
		self.result_backend = Some(result_backend);
		self
	}

	/// Run the worker loop
	///
	/// This method blocks until the worker is stopped via `stop()`.
	///
	/// A worker is single-shot: once `stop()` has been called, the shutdown
	/// state is permanent and any subsequent `run()` returns immediately.
	/// Create a new `Worker` to run again.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{Worker, WorkerConfig, DummyBackend};
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	/// let worker = Worker::new(WorkerConfig::default());
	/// let backend = Arc::new(DummyBackend::new());
	///
	/// Arc::new(worker).run(backend).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn run(
		self: Arc<Self>,
		backend: Arc<dyn TaskBackend>,
	) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
		let concurrency = self.config.concurrency.max(1);
		tracing::info!(
			worker = %self.config.name,
			concurrency,
			"Worker started"
		);

		// Concurrency is N independent consumer loops — one spawned task each.
		// That IS the limit: each loop holds a single message at a time, so at
		// most `concurrency` tasks run at once with nothing reserved ahead. No
		// shared permit, no prefetch. tokio schedules the loops across its threads.
		//
		// Each receiver is subscribed HERE, before the consumer future is spawned,
		// so the receiver exists the instant `stop()` can be called. Combined with
		// the stateful `watch` channel, a `stop()` racing with startup is never
		// lost (issue #2).
		let mut consumers = tokio::task::JoinSet::new();
		for _ in 0..concurrency {
			let worker = Arc::clone(&self);
			let backend = backend.clone();
			let shutdown_rx = self.shutdown_tx.subscribe();
			consumers.spawn(async move { worker.consume(backend, shutdown_rx).await });
		}

		// Monitor consumer completion. A consumer that returns normally has
		// observed shutdown. A consumer that panics surfaces as a `JoinError`;
		// we must not silently drop it (issue #3), otherwise the worker would run
		// with reduced capacity — or, if every consumer panicked, `run` would
		// still return `Ok(())`. On the first panic we stop the remaining
		// consumers and propagate the failure to the caller.
		let mut first_error: Option<Box<dyn std::error::Error + Send + Sync>> = None;
		while let Some(join_result) = consumers.join_next().await {
			match join_result {
				Ok(()) => {}
				Err(join_error) => {
					tracing::error!(
						worker = %self.config.name,
						error = %join_error,
						"Consumer task terminated abnormally; stopping worker"
					);
					// Bring the surviving consumers down so we don't keep running
					// at reduced capacity.
					self.stop().await;
					if first_error.is_none() {
						first_error = Some(Box::new(join_error));
					}
				}
			}
		}

		if let Some(error) = first_error {
			tracing::info!(worker = %self.config.name, "Worker stopped after consumer failure");
			return Err(error);
		}

		tracing::info!(worker = %self.config.name, "Worker stopped");
		Ok(())
	}

	/// One consumer loop: dequeue a task, run it, report status, repeat. `run`
	/// spawns `concurrency` of these. Each holds one message at a time (it runs
	/// the task inline before dequeuing the next), so in-flight work is bounded by
	/// the number of loops. When the queue is empty the loop waits `poll_interval`.
	async fn consume(&self, backend: Arc<dyn TaskBackend>, mut shutdown_rx: watch::Receiver<bool>) {
		use tokio::time::interval;

		let mut poll_interval = interval(self.config.poll_interval);

		loop {
			// Stop promptly on shutdown, even while the queue is busy. `watch` is
			// stateful: a `stop()` that fired before this receiver was first polled
			// is still observed here through the current value, so the signal can
			// never be missed (issues #2 and #4). `borrow()` inspects without
			// marking the value seen, keeping the `changed()` wakeup below armed.
			if *shutdown_rx.borrow() {
				break;
			}

			match backend.dequeue().await {
				Ok(Some(task_id)) => {
					tracing::info!(worker = %self.config.name, task_id = %task_id, "Processing task");
					let status = match self.execute_task(task_id, backend.clone()).await {
						Ok(_) => {
							tracing::info!(
								worker = %self.config.name,
								task_id = %task_id,
								"Task completed successfully"
							);
							TaskStatus::Success
						}
						Err(e) => {
							tracing::error!(
								worker = %self.config.name,
								task_id = %task_id,
								error = %e,
								"Task failed"
							);
							TaskStatus::Failure
						}
					};
					if let Err(e) = backend.update_status(task_id, status).await {
						tracing::error!(
							worker = %self.config.name,
							task_id = %task_id,
							error = %e,
							"Failed to update task status"
						);
					}
				}
				// Empty queue or a transient dequeue error: wait for the next
				// tick or shutdown, then poll again.
				Ok(None) => {
					tokio::select! {
						// `changed()` resolves when `stop()` flips the value, or
						// (as `Err`) when the sender is dropped during teardown;
						// either way it means "stop".
						_ = shutdown_rx.changed() => break,
						_ = poll_interval.tick() => {}
					}
				}
				Err(e) => {
					tracing::error!(worker = %self.config.name, error = %e, "Failed to dequeue task");
					tokio::select! {
						_ = shutdown_rx.changed() => break,
						_ = poll_interval.tick() => {}
					}
				}
			}
		}
	}

	/// Execute a task
	async fn execute_task(
		&self,
		task_id: crate::TaskId,
		backend: Arc<dyn TaskBackend>,
	) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
		tracing::debug!(worker = %self.config.name, task_id = %task_id, "Executing task");

		let started_at = Utc::now();

		// Try to acquire lock if available
		let mut lock_token = None;
		if let Some(ref lock) = self.task_lock {
			match lock.acquire(task_id, Duration::from_secs(300)).await? {
				Some(token) => lock_token = Some(token),
				None => {
					tracing::info!(
						worker = %self.config.name,
						task_id = %task_id,
						"Task already locked by another worker"
					);
					return Ok(());
				}
			}
		}

		// Fetch task data once and reuse for both name extraction and execution
		let serialized_task = backend.get_task_data(task_id).await?;
		let task_name = serialized_task
			.as_ref()
			.map(|t| t.name().to_string())
			.unwrap_or_else(|| "unknown_task".to_string());

		// Execute task with registry if available
		let result: Result<(), Box<dyn std::error::Error + Send + Sync>> =
			if let Some(ref registry) = self.registry {
				match serialized_task {
					Some(serialized_task) => {
						tracing::debug!(
							worker = %self.config.name,
							task_name = %task_name,
							"Executing task with registry"
						);

						// Deserialize task using registry to get concrete task instance
						match registry
							.create(serialized_task.name(), serialized_task.data())
							.await
						{
							Ok(task_executor) => {
								// Execute the deserialized task with its arguments
								match task_executor.execute().await {
									Ok(_) => {
										tracing::info!(
											worker = %self.config.name,
											task_name = %task_name,
											"Task completed successfully"
										);
										Ok(())
									}
									Err(e) => {
										tracing::error!(
											worker = %self.config.name,
											task_name = %task_name,
											error = %e,
											"Task failed"
										);
										Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
									}
								}
							}
							Err(e) => {
								tracing::error!(
									worker = %self.config.name,
									task_name = %task_name,
									error = %e,
									"Failed to deserialize task"
								);
								Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
							}
						}
					}
					None => {
						tracing::warn!(
							worker = %self.config.name,
							task_id = %task_id,
							"Task not found in backend"
						);
						Err(format!("Task {} not found", task_id).into())
					}
				}
			} else {
				tracing::debug!(
					worker = %self.config.name,
					"Task execution without registry (basic mode)"
				);
				Ok(())
			};

		let completed_at = Utc::now();
		// Use saturating conversion to prevent overflow on negative or very large durations
		let duration_ms = (completed_at - started_at).num_milliseconds().max(0) as u64;

		// Determine final task status
		let (task_status, webhook_status) = match &result {
			Ok(_) => (TaskStatus::Success, crate::webhook::TaskStatus::Success),
			Err(_) => (TaskStatus::Failure, crate::webhook::TaskStatus::Failed),
		};

		// Store result if result backend is available.
		// Capture store_result error separately to ensure lock is always released.
		let store_error = if let Some(ref result_backend) = self.result_backend {
			let metadata = match result {
				Ok(_) => TaskResultMetadata::new(
					task_id,
					task_status,
					Some("Task completed successfully".to_string()),
				),
				Err(ref e) => {
					TaskResultMetadata::with_error(task_id, format!("Task failed: {}", e))
				}
			};

			result_backend.store_result(metadata).await.err()
		} else {
			None
		};

		// Send webhook notifications
		if !self.webhook_senders.is_empty() {
			let webhook_event = WebhookEvent {
				task_id,
				task_name,
				status: webhook_status,
				result: match webhook_status {
					crate::webhook::TaskStatus::Success => {
						Some("Task completed successfully".to_string())
					}
					crate::webhook::TaskStatus::Failed => None,
					crate::webhook::TaskStatus::Cancelled => None,
				},
				error: match webhook_status {
					crate::webhook::TaskStatus::Failed => match &result {
						Err(e) => Some(e.to_string()),
						_ => Some("Unknown error".to_string()),
					},
					_ => None,
				},
				started_at,
				completed_at,
				duration_ms,
			};

			// Send to all configured webhooks (fire and forget)
			for sender in &self.webhook_senders {
				let sender_clone = Arc::clone(sender);
				let event_clone = webhook_event.clone();
				tokio::spawn(async move {
					if let Err(e) = sender_clone.send(&event_clone).await {
						tracing::error!(error = %e, "Failed to send webhook notification");
					}
				});
			}
		}

		// Always release lock if acquired, regardless of store_result outcome
		if let Some(ref lock) = self.task_lock
			&& let Some(ref token) = lock_token
		{
			match lock.release(task_id, token).await {
				Ok(false) => {
					tracing::warn!(
						worker = %self.config.name,
						task_id = %task_id,
						"Lock release returned false: token mismatch or lock already expired"
					);
				}
				Err(e) => {
					tracing::error!(
						worker = %self.config.name,
						task_id = %task_id,
						error = %e,
						"Failed to release task lock"
					);
				}
				Ok(true) => {}
			}
		}

		// Propagate store_result error after lock is released
		if let Some(e) = store_error {
			return Err(Box::new(e));
		}

		result
	}

	/// Stop the worker
	///
	/// Sends a shutdown signal to all worker loops. The signal is retained, so
	/// a `stop()` issued before `run()` is still observed. Stopping is
	/// permanent: the worker cannot be restarted afterwards — `run()` will
	/// return immediately. Create a new `Worker` to run again.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{Worker, WorkerConfig};
	///
	/// # async fn example() {
	/// let worker = Worker::new(WorkerConfig::default());
	/// worker.stop().await;
	/// # }
	/// ```
	pub async fn stop(&self) {
		// `send_replace` (rather than `send`) updates the retained value even when
		// no receivers are currently subscribed. `send` would fail — and leave the
		// value untouched — if called before `run()` subscribes its consumers,
		// losing the shutdown signal (issue #2). The retained `true` is then
		// observed by every consumer, including any that has not yet polled its
		// receiver.
		let _ = self.shutdown_tx.send_replace(true);
	}
}

impl Default for Worker {
	fn default() -> Self {
		let config = WorkerConfig::default();
		Self {
			config,
			shutdown_tx: watch::channel(false).0,
			registry: None,
			task_lock: None,
			result_backend: None,
			webhook_senders: Vec::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{DummyBackend, Task, TaskId, TaskPriority};
	use rstest::rstest;
	use std::time::Duration;
	use tokio::time::sleep;

	// Allow dead_code: fields are accessed indirectly through Task trait implementation
	#[allow(dead_code)]
	struct TestTask {
		id: TaskId,
		name: String,
	}

	impl Task for TestTask {
		fn id(&self) -> TaskId {
			self.id
		}

		fn name(&self) -> &str {
			&self.name
		}

		fn priority(&self) -> TaskPriority {
			TaskPriority::new(5)
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_creation() {
		// Arrange
		let config = WorkerConfig::new("test-worker".to_string());

		// Act
		let worker = Worker::new(config);

		// Assert
		assert_eq!(worker.config.name, "test-worker");
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_config_builder() {
		// Arrange & Act
		let config = WorkerConfig::new("test".to_string())
			.with_concurrency(8)
			.with_poll_interval(Duration::from_millis(100));

		// Assert
		assert_eq!(config.concurrency, 8);
		assert_eq!(config.poll_interval, Duration::from_millis(100));
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_start_and_stop() {
		// Arrange
		let worker = Worker::new(WorkerConfig::default());
		let backend = Arc::new(DummyBackend::new());
		let worker_clone = Worker {
			config: worker.config.clone(),
			shutdown_tx: worker.shutdown_tx.clone(),
			registry: None,
			task_lock: None,
			result_backend: None,
			webhook_senders: Vec::new(),
		};

		let handle = tokio::spawn(async move { Arc::new(worker).run(backend).await });

		// Give worker time to start
		sleep(Duration::from_millis(100)).await;

		// Act
		worker_clone.stop().await;

		// Assert - worker should finish within timeout
		let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_with_registry() {
		// Arrange
		use crate::registry::TaskRegistry;
		let registry = Arc::new(TaskRegistry::new());

		// Act
		let worker = Worker::new(WorkerConfig::default()).with_registry(registry);

		// Assert
		assert!(worker.registry.is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_with_lock() {
		// Arrange
		use crate::locking::MemoryTaskLock;
		let lock = Arc::new(MemoryTaskLock::new());

		// Act
		let worker = Worker::new(WorkerConfig::default()).with_lock(lock);

		// Assert
		assert!(worker.task_lock.is_some());
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_with_result_backend() {
		// Arrange
		use crate::result::MemoryResultBackend;
		let backend = Arc::new(MemoryResultBackend::new());

		// Act
		let worker = Worker::new(WorkerConfig::default()).with_result_backend(backend);

		// Assert
		assert!(worker.result_backend.is_some());
	}

	/// Backend whose `dequeue` always panics, used to reproduce a consumer
	/// crashing mid-loop.
	struct PanicOnDequeueBackend;

	#[async_trait::async_trait]
	impl TaskBackend for PanicOnDequeueBackend {
		async fn enqueue(&self, _task: Box<dyn Task>) -> Result<TaskId, crate::TaskExecutionError> {
			Ok(TaskId::new())
		}

		async fn dequeue(&self) -> Result<Option<TaskId>, crate::TaskExecutionError> {
			panic!("simulated consumer panic");
		}

		async fn get_status(
			&self,
			_task_id: TaskId,
		) -> Result<TaskStatus, crate::TaskExecutionError> {
			Ok(TaskStatus::Pending)
		}

		async fn update_status(
			&self,
			_task_id: TaskId,
			_status: TaskStatus,
		) -> Result<(), crate::TaskExecutionError> {
			Ok(())
		}

		async fn get_task_data(
			&self,
			_task_id: TaskId,
		) -> Result<Option<crate::registry::SerializedTask>, crate::TaskExecutionError> {
			Ok(None)
		}

		fn backend_name(&self) -> &str {
			"panic-on-dequeue"
		}
	}

	/// Regression for issue #2 / #4: a `stop()` that fires before `run()` even
	/// subscribes its consumers must still be observed. With the previous
	/// `broadcast` channel the notification was dropped (no receivers were
	/// registered when `send` ran), and `run()` would block forever. The
	/// stateful `watch` channel retains the value, so `run()` returns promptly.
	#[rstest]
	#[tokio::test]
	async fn test_stop_before_run_terminates_immediately() {
		// Arrange
		let worker = Worker::new(WorkerConfig::new("stop-before-run".to_string()));
		let backend: Arc<dyn TaskBackend> = Arc::new(DummyBackend::new());
		// Signal shutdown before the worker loop is ever started.
		worker.stop().await;

		// Act
		let result =
			tokio::time::timeout(Duration::from_secs(2), Arc::new(worker).run(backend)).await;

		// Assert
		assert!(
			result.is_ok(),
			"run() must not hang when stopped before start"
		);
		assert!(result.unwrap().is_ok());
	}

	/// Regression for issue #2: stopping immediately after startup, while
	/// consumers may not have polled their receivers yet, must still terminate
	/// the worker.
	#[rstest]
	#[tokio::test]
	async fn test_immediate_stop_after_startup() {
		// Arrange
		let worker = Arc::new(Worker::new(
			WorkerConfig::new("immediate-stop".to_string()).with_concurrency(4),
		));
		let backend: Arc<dyn TaskBackend> = Arc::new(DummyBackend::new());
		let run_handle = {
			let worker = Arc::clone(&worker);
			tokio::spawn(async move { worker.run(backend).await })
		};

		// Act
		worker.stop().await;

		// Assert
		let result = tokio::time::timeout(Duration::from_secs(2), run_handle).await;
		assert!(result.is_ok(), "worker did not stop promptly after startup");
		assert!(result.unwrap().expect("run task panicked").is_ok());
	}

	/// Regression for issue #3: a panicking consumer must not be silently
	/// discarded. `run()` must surface the failure instead of returning `Ok(())`
	/// (or hanging) with reduced capacity.
	#[rstest]
	#[tokio::test]
	async fn test_consumer_panic_propagates() {
		// Arrange
		let worker = Arc::new(Worker::new(
			WorkerConfig::new("panicking".to_string()).with_concurrency(2),
		));
		let backend: Arc<dyn TaskBackend> = Arc::new(PanicOnDequeueBackend);

		// Act
		let result = tokio::time::timeout(Duration::from_secs(2), worker.run(backend)).await;

		// Assert
		let run_result = result.expect("run() hung after consumer panic");
		assert!(
			run_result.is_err(),
			"a consumer panic must be propagated, not swallowed"
		);
	}
}
