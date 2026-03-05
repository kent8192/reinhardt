//! Task metrics and monitoring
//!
//! This module provides metrics collection and monitoring capabilities for background tasks.
//!
//! ## Features
//!
//! - Task execution time tracking with percentile calculation (P50, P95, P99)
//! - Success/failure rate metrics
//! - Queue depth monitoring
//! - Worker utilization metrics
//! - Snapshot capabilities for metrics reporting
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_tasks::{TaskMetrics, TaskId};
//! use std::time::Duration;
//!
//! # async fn example() {
//! let metrics = TaskMetrics::new();
//!
//! // Record task execution
//! let task_id = TaskId::new();
//! metrics.record_task_start(&task_id).await.unwrap();
//! metrics.record_task_success(&task_id, Duration::from_millis(100)).await.unwrap();
//!
//! // Get snapshot
//! let snapshot = metrics.snapshot().await;
//! assert_eq!(snapshot.task_counts.total, 1);
//! assert_eq!(snapshot.task_counts.successful, 1);
//! # }
//! ```

use crate::{TaskId, TaskResult};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Maximum number of execution times to retain in the ring buffer.
/// Prevents unbounded memory growth in long-running services.
const MAX_EXECUTION_TIMES: usize = 10_000;

/// Task count metrics
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::TaskCounts;
///
/// let counts = TaskCounts {
///     total: 100,
///     successful: 95,
///     failed: 5,
///     pending: 10,
///     running: 3,
/// };
/// assert_eq!(counts.total, 100);
/// ```
#[derive(Debug, Clone, Default)]
pub struct TaskCounts {
	/// Total number of tasks processed
	pub total: u64,
	/// Number of successful tasks
	pub successful: u64,
	/// Number of failed tasks
	pub failed: u64,
	/// Number of pending tasks
	pub pending: u64,
	/// Number of running tasks
	pub running: u64,
}

/// Worker statistics
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::WorkerStats;
/// use std::time::Duration;
///
/// let stats = WorkerStats {
///     tasks_processed: 50,
///     average_execution_time: Duration::from_millis(100),
///     idle_time: Duration::from_secs(10),
/// };
/// assert_eq!(stats.tasks_processed, 50);
/// ```
#[derive(Debug, Clone)]
pub struct WorkerStats {
	/// Number of tasks processed by this worker
	pub tasks_processed: u64,
	/// Average execution time for tasks
	pub average_execution_time: Duration,
	/// Total idle time
	pub idle_time: Duration,
}

impl Default for WorkerStats {
	fn default() -> Self {
		Self {
			tasks_processed: 0,
			average_execution_time: Duration::ZERO,
			idle_time: Duration::ZERO,
		}
	}
}

/// Snapshot of current metrics
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_tasks::{TaskMetrics, MetricsSnapshot};
///
/// # async fn example() {
/// let metrics = TaskMetrics::new();
/// let snapshot = metrics.snapshot().await;
/// assert_eq!(snapshot.task_counts.total, 0);
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
	/// Task count metrics
	pub task_counts: TaskCounts,
	/// Average execution time
	pub average_execution_time: Duration,
	/// 50th percentile execution time
	pub p50_execution_time: Duration,
	/// 95th percentile execution time
	pub p95_execution_time: Duration,
	/// 99th percentile execution time
	pub p99_execution_time: Duration,
	/// Queue depths by queue name
	pub queue_depths: HashMap<String, usize>,
	/// Worker statistics by worker ID
	pub worker_stats: HashMap<String, WorkerStats>,
}

/// Task metrics collector
///
/// Collects and aggregates metrics for task execution, queue depths, and worker performance.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_tasks::{TaskMetrics, TaskId};
/// use std::time::Duration;
///
/// # async fn example() {
/// let metrics = TaskMetrics::new();
///
/// // Track a task
/// let task_id = TaskId::new();
/// metrics.record_task_start(&task_id).await.unwrap();
/// metrics.record_task_success(&task_id, Duration::from_millis(150)).await.unwrap();
///
/// // Get metrics
/// let snapshot = metrics.snapshot().await;
/// assert_eq!(snapshot.task_counts.total, 1);
/// # }
/// ```
#[derive(Clone)]
pub struct TaskMetrics {
	task_counts: Arc<RwLock<TaskCounts>>,
	execution_times: Arc<RwLock<VecDeque<Duration>>>,
	queue_depths: Arc<RwLock<HashMap<String, usize>>>,
	worker_stats: Arc<RwLock<HashMap<String, WorkerStats>>>,
}

impl TaskMetrics {
	/// Create a new TaskMetrics instance
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::TaskMetrics;
	///
	/// let metrics = TaskMetrics::new();
	/// ```
	pub fn new() -> Self {
		Self {
			task_counts: Arc::new(RwLock::new(TaskCounts::default())),
			execution_times: Arc::new(RwLock::new(VecDeque::new())),
			queue_depths: Arc::new(RwLock::new(HashMap::new())),
			worker_stats: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Record a task start
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{TaskMetrics, TaskId};
	///
	/// # async fn example() {
	/// let metrics = TaskMetrics::new();
	/// let task_id = TaskId::new();
	/// metrics.record_task_start(&task_id).await.unwrap();
	///
	/// let snapshot = metrics.snapshot().await;
	/// assert_eq!(snapshot.task_counts.running, 1);
	/// # }
	/// ```
	pub async fn record_task_start(&self, _task_id: &TaskId) -> TaskResult<()> {
		let mut counts = self.task_counts.write().await;
		counts.running += 1;
		counts.total += 1;
		Ok(())
	}

	/// Record a successful task completion
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{TaskMetrics, TaskId};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let metrics = TaskMetrics::new();
	/// let task_id = TaskId::new();
	/// metrics.record_task_start(&task_id).await.unwrap();
	/// metrics.record_task_success(&task_id, Duration::from_millis(100)).await.unwrap();
	///
	/// let snapshot = metrics.snapshot().await;
	/// assert_eq!(snapshot.task_counts.successful, 1);
	/// assert_eq!(snapshot.task_counts.running, 0);
	/// # }
	/// ```
	pub async fn record_task_success(
		&self,
		_task_id: &TaskId,
		duration: Duration,
	) -> TaskResult<()> {
		let mut counts = self.task_counts.write().await;
		counts.successful += 1;
		counts.running = counts.running.saturating_sub(1);

		let mut times = self.execution_times.write().await;
		if times.len() >= MAX_EXECUTION_TIMES {
			times.pop_front();
		}
		times.push_back(duration);

		Ok(())
	}

	/// Record a failed task
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{TaskMetrics, TaskId};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let metrics = TaskMetrics::new();
	/// let task_id = TaskId::new();
	/// metrics.record_task_start(&task_id).await.unwrap();
	/// metrics.record_task_failure(&task_id, Duration::from_millis(50)).await.unwrap();
	///
	/// let snapshot = metrics.snapshot().await;
	/// assert_eq!(snapshot.task_counts.failed, 1);
	/// assert_eq!(snapshot.task_counts.running, 0);
	/// # }
	/// ```
	pub async fn record_task_failure(
		&self,
		_task_id: &TaskId,
		duration: Duration,
	) -> TaskResult<()> {
		let mut counts = self.task_counts.write().await;
		counts.failed += 1;
		counts.running = counts.running.saturating_sub(1);

		let mut times = self.execution_times.write().await;
		if times.len() >= MAX_EXECUTION_TIMES {
			times.pop_front();
		}
		times.push_back(duration);

		Ok(())
	}

	/// Record queue depth for a specific queue
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::TaskMetrics;
	///
	/// # async fn example() {
	/// let metrics = TaskMetrics::new();
	/// metrics.record_queue_depth("default".to_string(), 42).await.unwrap();
	///
	/// let snapshot = metrics.snapshot().await;
	/// assert_eq!(snapshot.queue_depths.get("default"), Some(&42));
	/// # }
	/// ```
	pub async fn record_queue_depth(&self, queue_name: String, depth: usize) -> TaskResult<()> {
		let mut depths = self.queue_depths.write().await;
		depths.insert(queue_name, depth);
		Ok(())
	}

	/// Record worker statistics
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{TaskMetrics, WorkerStats};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let metrics = TaskMetrics::new();
	/// let stats = WorkerStats {
	///     tasks_processed: 10,
	///     average_execution_time: Duration::from_millis(100),
	///     idle_time: Duration::from_secs(5),
	/// };
	/// metrics.record_worker_stats("worker-1".to_string(), stats).await.unwrap();
	///
	/// let snapshot = metrics.snapshot().await;
	/// assert_eq!(snapshot.worker_stats.get("worker-1").unwrap().tasks_processed, 10);
	/// # }
	/// ```
	pub async fn record_worker_stats(
		&self,
		worker_id: String,
		stats: WorkerStats,
	) -> TaskResult<()> {
		let mut worker_stats = self.worker_stats.write().await;
		worker_stats.insert(worker_id, stats);
		Ok(())
	}

	/// Get a snapshot of current metrics
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{TaskMetrics, TaskId};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let metrics = TaskMetrics::new();
	///
	/// let task_id = TaskId::new();
	/// metrics.record_task_start(&task_id).await.unwrap();
	/// metrics.record_task_success(&task_id, Duration::from_millis(100)).await.unwrap();
	///
	/// let snapshot = metrics.snapshot().await;
	/// assert_eq!(snapshot.task_counts.total, 1);
	/// assert_eq!(snapshot.task_counts.successful, 1);
	/// assert!(snapshot.average_execution_time >= Duration::from_millis(100));
	/// # }
	/// ```
	pub async fn snapshot(&self) -> MetricsSnapshot {
		let counts = self.task_counts.read().await.clone();
		let times_deque = self.execution_times.read().await.clone();
		let depths = self.queue_depths.read().await.clone();
		let workers = self.worker_stats.read().await.clone();

		let times: Vec<Duration> = times_deque.into_iter().collect();
		let (average, p50, p95, p99) = Self::calculate_percentiles(&times);

		MetricsSnapshot {
			task_counts: counts,
			average_execution_time: average,
			p50_execution_time: p50,
			p95_execution_time: p95,
			p99_execution_time: p99,
			queue_depths: depths,
			worker_stats: workers,
		}
	}

	/// Reset all metrics
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{TaskMetrics, TaskId};
	/// use std::time::Duration;
	///
	/// # async fn example() {
	/// let metrics = TaskMetrics::new();
	///
	/// let task_id = TaskId::new();
	/// metrics.record_task_start(&task_id).await.unwrap();
	/// metrics.record_task_success(&task_id, Duration::from_millis(100)).await.unwrap();
	///
	/// metrics.reset().await.unwrap();
	///
	/// let snapshot = metrics.snapshot().await;
	/// assert_eq!(snapshot.task_counts.total, 0);
	/// # }
	/// ```
	pub async fn reset(&self) -> TaskResult<()> {
		let mut counts = self.task_counts.write().await;
		*counts = TaskCounts::default();

		let mut times = self.execution_times.write().await;
		times.clear();

		let mut depths = self.queue_depths.write().await;
		depths.clear();

		let mut workers = self.worker_stats.write().await;
		workers.clear();

		Ok(())
	}

	/// Calculate percentiles from execution times
	///
	/// Returns (average, p50, p95, p99)
	fn calculate_percentiles(times: &[Duration]) -> (Duration, Duration, Duration, Duration) {
		if times.is_empty() {
			return (
				Duration::ZERO,
				Duration::ZERO,
				Duration::ZERO,
				Duration::ZERO,
			);
		}

		let mut sorted = times.to_vec();
		sorted.sort();

		let total: Duration = sorted.iter().sum();
		let average = total / times.len() as u32;

		let p50 = Self::percentile(&sorted, 0.50);
		let p95 = Self::percentile(&sorted, 0.95);
		let p99 = Self::percentile(&sorted, 0.99);

		(average, p50, p95, p99)
	}

	/// Calculate a specific percentile
	fn percentile(sorted: &[Duration], percentile: f64) -> Duration {
		if sorted.is_empty() {
			return Duration::ZERO;
		}

		let index = ((sorted.len() as f64 - 1.0) * percentile) as usize;
		sorted[index.min(sorted.len() - 1)]
	}
}

impl Default for TaskMetrics {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_record_task_start() {
		let metrics = TaskMetrics::new();
		let task_id = TaskId::new();

		metrics.record_task_start(&task_id).await.unwrap();

		let snapshot = metrics.snapshot().await;
		assert_eq!(snapshot.task_counts.total, 1);
		assert_eq!(snapshot.task_counts.running, 1);
	}

	#[tokio::test]
	async fn test_record_task_success() {
		let metrics = TaskMetrics::new();
		let task_id = TaskId::new();

		metrics.record_task_start(&task_id).await.unwrap();
		metrics
			.record_task_success(&task_id, Duration::from_millis(100))
			.await
			.unwrap();

		let snapshot = metrics.snapshot().await;
		assert_eq!(snapshot.task_counts.total, 1);
		assert_eq!(snapshot.task_counts.successful, 1);
		assert_eq!(snapshot.task_counts.running, 0);
		assert_eq!(snapshot.average_execution_time, Duration::from_millis(100));
	}

	#[tokio::test]
	async fn test_record_task_failure() {
		let metrics = TaskMetrics::new();
		let task_id = TaskId::new();

		metrics.record_task_start(&task_id).await.unwrap();
		metrics
			.record_task_failure(&task_id, Duration::from_millis(50))
			.await
			.unwrap();

		let snapshot = metrics.snapshot().await;
		assert_eq!(snapshot.task_counts.total, 1);
		assert_eq!(snapshot.task_counts.failed, 1);
		assert_eq!(snapshot.task_counts.running, 0);
	}

	#[tokio::test]
	async fn test_record_queue_depth() {
		let metrics = TaskMetrics::new();

		metrics
			.record_queue_depth("default".to_string(), 42)
			.await
			.unwrap();
		metrics
			.record_queue_depth("priority".to_string(), 10)
			.await
			.unwrap();

		let snapshot = metrics.snapshot().await;
		assert_eq!(snapshot.queue_depths.get("default"), Some(&42));
		assert_eq!(snapshot.queue_depths.get("priority"), Some(&10));
	}

	#[tokio::test]
	async fn test_record_worker_stats() {
		let metrics = TaskMetrics::new();

		let stats = WorkerStats {
			tasks_processed: 10,
			average_execution_time: Duration::from_millis(100),
			idle_time: Duration::from_secs(5),
		};

		metrics
			.record_worker_stats("worker-1".to_string(), stats.clone())
			.await
			.unwrap();

		let snapshot = metrics.snapshot().await;
		let worker = snapshot.worker_stats.get("worker-1").unwrap();
		assert_eq!(worker.tasks_processed, 10);
		assert_eq!(worker.average_execution_time, Duration::from_millis(100));
		assert_eq!(worker.idle_time, Duration::from_secs(5));
	}

	#[tokio::test]
	async fn test_percentile_calculation() {
		let metrics = TaskMetrics::new();
		let task_id = TaskId::new();

		// Use 100 data points for accurate percentile calculation
		for i in 1..=100 {
			metrics.record_task_start(&task_id).await.unwrap();
			metrics
				.record_task_success(&task_id, Duration::from_millis(i))
				.await
				.unwrap();
		}

		let snapshot = metrics.snapshot().await;

		// Average: (1+2+...+100)/100 = 5050/100 = 50.5 (rounds down to 50 in Duration)
		// Note: Duration division truncates, not rounds
		let avg = snapshot.average_execution_time;
		assert!(
			avg >= Duration::from_millis(50) && avg <= Duration::from_millis(51),
			"Expected average around 50-51ms, got {:?}",
			avg
		);
		// P50: (100-1) * 0.50 = 49.5, index 49 (50th element in 0-indexed)
		assert_eq!(snapshot.p50_execution_time, Duration::from_millis(50));
		// P95: (100-1) * 0.95 = 94.05, index 94 (95th element in 0-indexed)
		assert_eq!(snapshot.p95_execution_time, Duration::from_millis(95));
		// P99: (100-1) * 0.99 = 98.01, index 98 (99th element in 0-indexed)
		assert_eq!(snapshot.p99_execution_time, Duration::from_millis(99));
	}

	#[tokio::test]
	async fn test_reset() {
		let metrics = TaskMetrics::new();
		let task_id = TaskId::new();

		metrics.record_task_start(&task_id).await.unwrap();
		metrics
			.record_task_success(&task_id, Duration::from_millis(100))
			.await
			.unwrap();
		metrics
			.record_queue_depth("default".to_string(), 42)
			.await
			.unwrap();

		metrics.reset().await.unwrap();

		let snapshot = metrics.snapshot().await;
		assert_eq!(snapshot.task_counts.total, 0);
		assert_eq!(snapshot.task_counts.successful, 0);
		assert_eq!(snapshot.queue_depths.len(), 0);
		assert_eq!(snapshot.average_execution_time, Duration::ZERO);
	}

	#[tokio::test]
	async fn test_concurrent_access() {
		let metrics = Arc::new(TaskMetrics::new());
		let mut handles = vec![];

		for i in 0..10 {
			let metrics = Arc::clone(&metrics);
			let handle = tokio::spawn(async move {
				let task_id = TaskId::new();
				metrics.record_task_start(&task_id).await.unwrap();
				metrics
					.record_task_success(&task_id, Duration::from_millis(i * 10))
					.await
					.unwrap();
			});
			handles.push(handle);
		}

		for handle in handles {
			handle.await.unwrap();
		}

		let snapshot = metrics.snapshot().await;
		assert_eq!(snapshot.task_counts.total, 10);
		assert_eq!(snapshot.task_counts.successful, 10);
	}

	#[tokio::test]
	async fn test_empty_percentiles() {
		let metrics = TaskMetrics::new();
		let snapshot = metrics.snapshot().await;

		assert_eq!(snapshot.average_execution_time, Duration::ZERO);
		assert_eq!(snapshot.p50_execution_time, Duration::ZERO);
		assert_eq!(snapshot.p95_execution_time, Duration::ZERO);
		assert_eq!(snapshot.p99_execution_time, Duration::ZERO);
	}

	#[tokio::test]
	async fn test_single_value_percentiles() {
		let metrics = TaskMetrics::new();
		let task_id = TaskId::new();

		metrics.record_task_start(&task_id).await.unwrap();
		metrics
			.record_task_success(&task_id, Duration::from_millis(100))
			.await
			.unwrap();

		let snapshot = metrics.snapshot().await;
		assert_eq!(snapshot.average_execution_time, Duration::from_millis(100));
		assert_eq!(snapshot.p50_execution_time, Duration::from_millis(100));
		assert_eq!(snapshot.p95_execution_time, Duration::from_millis(100));
		assert_eq!(snapshot.p99_execution_time, Duration::from_millis(100));
	}
}
