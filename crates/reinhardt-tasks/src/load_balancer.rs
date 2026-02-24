//! Worker load balancing
//!
//! Provides load balancing strategies for distributing tasks across multiple workers.

use crate::{TaskError, TaskResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;

/// Worker identifier
pub type WorkerId = String;

/// Load balancing strategy
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{LoadBalancingStrategy};
/// use std::collections::HashMap;
///
/// // Round-robin strategy
/// let strategy = LoadBalancingStrategy::RoundRobin;
///
/// // Weighted strategy
/// let mut weights = HashMap::new();
/// weights.insert("worker-1".to_string(), 2);
/// weights.insert("worker-2".to_string(), 1);
/// let strategy = LoadBalancingStrategy::Weighted(weights);
/// ```
#[derive(Debug, Clone)]
pub enum LoadBalancingStrategy {
	/// Round-robin distribution
	RoundRobin,
	/// Least connections - select worker with fewest active tasks
	LeastConnections,
	/// Weighted distribution - workers with higher weights receive more tasks
	Weighted(HashMap<WorkerId, u32>),
	/// Random distribution
	Random,
}

/// Worker information
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::WorkerInfo;
///
/// let worker = WorkerInfo::new("worker-1".to_string(), 1);
/// assert_eq!(worker.id, "worker-1");
/// assert_eq!(worker.weight, 1);
/// assert_eq!(worker.active_tasks.load(std::sync::atomic::Ordering::SeqCst), 0);
/// ```
#[derive(Debug)]
pub struct WorkerInfo {
	pub id: WorkerId,
	pub weight: u32,
	pub active_tasks: AtomicUsize,
}

impl WorkerInfo {
	/// Create a new worker info
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::WorkerInfo;
	///
	/// let worker = WorkerInfo::new("worker-1".to_string(), 2);
	/// assert_eq!(worker.id, "worker-1");
	/// assert_eq!(worker.weight, 2);
	/// ```
	pub fn new(id: WorkerId, weight: u32) -> Self {
		Self {
			id,
			weight,
			active_tasks: AtomicUsize::new(0),
		}
	}

	/// Increment active task count
	pub fn increment_tasks(&self) {
		self.active_tasks.fetch_add(1, Ordering::SeqCst);
	}

	/// Decrement active task count (saturates at 0 to prevent underflow wrap)
	pub fn decrement_tasks(&self) {
		// Use fetch_update with saturating_sub to prevent wrapping to usize::MAX
		let _ = self
			.active_tasks
			.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
				Some(current.saturating_sub(1))
			});
	}

	/// Get current active task count
	pub fn active_task_count(&self) -> usize {
		self.active_tasks.load(Ordering::SeqCst)
	}
}

/// Worker metrics
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::WorkerMetrics;
/// use std::time::Duration;
///
/// let metrics = WorkerMetrics::new();
/// assert_eq!(metrics.tasks_completed, 0);
/// assert_eq!(metrics.tasks_failed, 0);
/// assert_eq!(metrics.average_execution_time, Duration::from_secs(0));
/// ```
#[derive(Debug, Clone)]
pub struct WorkerMetrics {
	pub tasks_completed: u64,
	pub tasks_failed: u64,
	pub average_execution_time: Duration,
}

impl WorkerMetrics {
	/// Create new metrics with default values
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::WorkerMetrics;
	///
	/// let metrics = WorkerMetrics::new();
	/// assert_eq!(metrics.tasks_completed, 0);
	/// ```
	pub fn new() -> Self {
		Self {
			tasks_completed: 0,
			tasks_failed: 0,
			average_execution_time: Duration::from_secs(0),
		}
	}

	/// Create metrics with specific values
	pub fn with_values(
		tasks_completed: u64,
		tasks_failed: u64,
		average_execution_time: Duration,
	) -> Self {
		Self {
			tasks_completed,
			tasks_failed,
			average_execution_time,
		}
	}

	/// Update average execution time with a new task duration.
	/// Uses checked/saturating arithmetic to prevent overflow on duration casting.
	fn update_execution_time(&mut self, duration: Duration) {
		let current_tasks = self.tasks_completed.saturating_add(self.tasks_failed);
		if current_tasks == 0 {
			self.average_execution_time = duration;
		} else {
			let avg_ms = self.average_execution_time.as_millis();
			let dur_ms = duration.as_millis();
			let total_time = avg_ms
				.saturating_mul(current_tasks as u128)
				.saturating_add(dur_ms);
			let new_count = (current_tasks as u128).saturating_add(1);
			let avg = total_time / new_count;
			// Clamp to u64::MAX to prevent truncation panic
			self.average_execution_time = Duration::from_millis(avg.min(u64::MAX as u128) as u64);
		}
	}

	/// Record a successful task completion
	pub fn record_success(&mut self, duration: Duration) {
		self.update_execution_time(duration);
		self.tasks_completed += 1;
	}

	/// Record a failed task
	pub fn record_failure(&mut self, duration: Duration) {
		self.update_execution_time(duration);
		self.tasks_failed += 1;
	}
}

impl Default for WorkerMetrics {
	fn default() -> Self {
		Self::new()
	}
}

/// Load balancer for distributing tasks across workers
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
/// balancer.register_worker(WorkerInfo::new("worker-2".to_string(), 1)).await?;
///
/// // Select worker for task
/// let worker_id = balancer.select_worker().await?;
/// println!("Selected worker: {}", worker_id);
/// # Ok(())
/// # }
/// ```
pub struct LoadBalancer {
	strategy: LoadBalancingStrategy,
	workers: Arc<RwLock<Vec<Arc<WorkerInfo>>>>,
	metrics: Arc<RwLock<HashMap<WorkerId, WorkerMetrics>>>,
	round_robin_index: Arc<AtomicUsize>,
}

impl LoadBalancer {
	/// Create a new load balancer with the specified strategy
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy};
	///
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
	/// ```
	pub fn new(strategy: LoadBalancingStrategy) -> Self {
		Self {
			strategy,
			workers: Arc::new(RwLock::new(Vec::new())),
			metrics: Arc::new(RwLock::new(HashMap::new())),
			round_robin_index: Arc::new(AtomicUsize::new(0)),
		}
	}

	/// Register a new worker
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
	/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn register_worker(&self, worker: WorkerInfo) -> TaskResult<()> {
		let worker_id = worker.id.clone();
		self.workers.write().await.push(Arc::new(worker));
		self.metrics
			.write()
			.await
			.insert(worker_id, WorkerMetrics::new());
		Ok(())
	}

	/// Unregister a worker
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
	/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
	/// balancer.unregister_worker("worker-1").await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn unregister_worker(&self, worker_id: &str) -> TaskResult<()> {
		self.workers.write().await.retain(|w| w.id != worker_id);
		self.metrics.write().await.remove(worker_id);
		Ok(())
	}

	/// Select a worker based on the load balancing strategy
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
	/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
	///
	/// let worker_id = balancer.select_worker().await?;
	/// assert_eq!(worker_id, "worker-1");
	/// # Ok(())
	/// # }
	/// ```
	pub async fn select_worker(&self) -> TaskResult<WorkerId> {
		let workers = self.workers.read().await;
		if workers.is_empty() {
			return Err(TaskError::QueueError("No workers available".to_string()));
		}

		let selected = match &self.strategy {
			LoadBalancingStrategy::RoundRobin => self.select_round_robin(&workers),
			LoadBalancingStrategy::LeastConnections => self.select_least_connections(&workers),
			LoadBalancingStrategy::Weighted(weights) => self.select_weighted(&workers, weights),
			LoadBalancingStrategy::Random => self.select_random(&workers),
		};

		selected.increment_tasks();
		Ok(selected.id.clone())
	}

	/// Round-robin selection
	fn select_round_robin(&self, workers: &[Arc<WorkerInfo>]) -> Arc<WorkerInfo> {
		let index = self.round_robin_index.fetch_add(1, Ordering::SeqCst) % workers.len();
		workers[index].clone()
	}

	/// Least connections selection
	fn select_least_connections(&self, workers: &[Arc<WorkerInfo>]) -> Arc<WorkerInfo> {
		workers
			.iter()
			.min_by_key(|w| w.active_task_count())
			.unwrap()
			.clone()
	}

	/// Weighted selection
	fn select_weighted(
		&self,
		workers: &[Arc<WorkerInfo>],
		weights: &HashMap<WorkerId, u32>,
	) -> Arc<WorkerInfo> {
		use rand::Rng;
		let total_weight: u32 = workers
			.iter()
			.map(|w| weights.get(&w.id).copied().unwrap_or(w.weight))
			.sum();

		// Guard against zero total weight to prevent panic in gen_range(0..0)
		if total_weight == 0 {
			return workers[0].clone();
		}

		let mut rng = rand::thread_rng();
		let mut random = rng.gen_range(0..total_weight);
		for worker in workers {
			let weight = weights.get(&worker.id).copied().unwrap_or(worker.weight);
			if random < weight {
				return worker.clone();
			}
			random -= weight;
		}

		workers[0].clone()
	}

	/// Random selection
	fn select_random(&self, workers: &[Arc<WorkerInfo>]) -> Arc<WorkerInfo> {
		use rand::Rng;
		let mut rng = rand::thread_rng();
		let index = rng.gen_range(0..workers.len());
		workers[index].clone()
	}

	/// Mark a task as completed on a worker
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
	/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
	///
	/// let worker_id = balancer.select_worker().await?;
	/// balancer.task_completed(&worker_id).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn task_completed(&self, worker_id: &str) -> TaskResult<()> {
		let workers = self.workers.read().await;
		if let Some(worker) = workers.iter().find(|w| w.id == worker_id) {
			worker.decrement_tasks();
		}
		Ok(())
	}

	/// Update worker metrics
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo, WorkerMetrics};
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
	/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
	///
	/// let metrics = WorkerMetrics::with_values(10, 1, Duration::from_millis(500));
	/// balancer.update_metrics("worker-1", metrics).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn update_metrics(&self, worker_id: &str, metrics: WorkerMetrics) -> TaskResult<()> {
		self.metrics
			.write()
			.await
			.insert(worker_id.to_string(), metrics);
		Ok(())
	}

	/// Get statistics for all workers
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
	/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
	///
	/// let stats = balancer.get_worker_stats().await;
	/// assert_eq!(stats.len(), 1);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get_worker_stats(&self) -> HashMap<WorkerId, WorkerMetrics> {
		self.metrics.read().await.clone()
	}

	/// Get active worker count
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{LoadBalancer, LoadBalancingStrategy, WorkerInfo};
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
	/// balancer.register_worker(WorkerInfo::new("worker-1".to_string(), 1)).await?;
	/// balancer.register_worker(WorkerInfo::new("worker-2".to_string(), 1)).await?;
	///
	/// assert_eq!(balancer.worker_count().await, 2);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn worker_count(&self) -> usize {
		self.workers.read().await.len()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::time::Duration;

	#[rstest]
	#[tokio::test]
	async fn test_worker_info_creation() {
		// Arrange
		let worker = WorkerInfo::new("worker-1".to_string(), 2);

		// Assert
		assert_eq!(worker.id, "worker-1");
		assert_eq!(worker.weight, 2);
		assert_eq!(worker.active_task_count(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_info_task_count() {
		// Arrange
		let worker = WorkerInfo::new("worker-1".to_string(), 1);

		// Act & Assert
		worker.increment_tasks();
		assert_eq!(worker.active_task_count(), 1);
		worker.increment_tasks();
		assert_eq!(worker.active_task_count(), 2);
		worker.decrement_tasks();
		assert_eq!(worker.active_task_count(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_metrics_creation() {
		// Arrange
		let metrics = WorkerMetrics::new();

		// Assert
		assert_eq!(metrics.tasks_completed, 0);
		assert_eq!(metrics.tasks_failed, 0);
		assert_eq!(metrics.average_execution_time, Duration::from_secs(0));
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_metrics_record_success() {
		// Arrange
		let mut metrics = WorkerMetrics::new();

		// Act
		metrics.record_success(Duration::from_millis(100));

		// Assert
		assert_eq!(metrics.tasks_completed, 1);
		assert_eq!(metrics.average_execution_time, Duration::from_millis(100));

		// Act
		metrics.record_success(Duration::from_millis(200));

		// Assert
		assert_eq!(metrics.tasks_completed, 2);
		assert_eq!(metrics.average_execution_time, Duration::from_millis(150));
	}

	#[rstest]
	#[tokio::test]
	async fn test_worker_metrics_record_failure() {
		// Arrange
		let mut metrics = WorkerMetrics::new();

		// Act
		metrics.record_failure(Duration::from_millis(50));

		// Assert
		assert_eq!(metrics.tasks_failed, 1);
		assert_eq!(metrics.average_execution_time, Duration::from_millis(50));
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_balancer_creation() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);

		// Assert
		assert_eq!(balancer.worker_count().await, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_balancer_register_worker() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);

		// Act
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 1))
			.await
			.unwrap();

		// Assert
		assert_eq!(balancer.worker_count().await, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_load_balancer_unregister_worker() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 1))
			.await
			.unwrap();

		// Act
		balancer.unregister_worker("worker-1").await.unwrap();

		// Assert
		assert_eq!(balancer.worker_count().await, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_round_robin_strategy() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 1))
			.await
			.unwrap();
		balancer
			.register_worker(WorkerInfo::new("worker-2".to_string(), 1))
			.await
			.unwrap();

		// Act
		let worker1 = balancer.select_worker().await.unwrap();
		let worker2 = balancer.select_worker().await.unwrap();
		let worker3 = balancer.select_worker().await.unwrap();

		// Assert
		assert_eq!(worker1, "worker-1");
		assert_eq!(worker2, "worker-2");
		assert_eq!(worker3, "worker-1");
	}

	#[rstest]
	#[tokio::test]
	async fn test_least_connections_strategy() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
		let worker1 = WorkerInfo::new("worker-1".to_string(), 1);
		let worker2 = WorkerInfo::new("worker-2".to_string(), 1);

		// Simulate worker-1 having more tasks
		worker1.increment_tasks();
		worker1.increment_tasks();

		balancer.register_worker(worker1).await.unwrap();
		balancer.register_worker(worker2).await.unwrap();

		// Act - should select worker-2 as it has fewer tasks
		let selected = balancer.select_worker().await.unwrap();

		// Assert
		assert_eq!(selected, "worker-2");
	}

	#[rstest]
	#[tokio::test]
	async fn test_weighted_strategy() {
		// Arrange
		let mut weights = HashMap::new();
		weights.insert("worker-1".to_string(), 3);
		weights.insert("worker-2".to_string(), 1);

		let balancer = LoadBalancer::new(LoadBalancingStrategy::Weighted(weights));
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 3))
			.await
			.unwrap();
		balancer
			.register_worker(WorkerInfo::new("worker-2".to_string(), 1))
			.await
			.unwrap();

		// Act - run multiple selections
		let mut worker1_count = 0;
		let mut worker2_count = 0;

		for _ in 0..100 {
			let selected = balancer.select_worker().await.unwrap();
			balancer.task_completed(&selected).await.unwrap();
			if selected == "worker-1" {
				worker1_count += 1;
			} else {
				worker2_count += 1;
			}
		}

		// Assert - worker-1 should be selected approximately 3x more often
		assert!(worker1_count > worker2_count);
	}

	#[rstest]
	#[tokio::test]
	async fn test_random_strategy() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::Random);
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 1))
			.await
			.unwrap();
		balancer
			.register_worker(WorkerInfo::new("worker-2".to_string(), 1))
			.await
			.unwrap();

		// Act
		let worker = balancer.select_worker().await.unwrap();

		// Assert
		assert!(worker == "worker-1" || worker == "worker-2");
	}

	#[rstest]
	#[tokio::test]
	async fn test_select_worker_no_workers() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);

		// Act
		let result = balancer.select_worker().await;

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	#[tokio::test]
	async fn test_task_completed() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 1))
			.await
			.unwrap();

		let worker_id = balancer.select_worker().await.unwrap();
		let workers = balancer.workers.read().await;
		let worker = workers.iter().find(|w| w.id == worker_id).unwrap();
		assert_eq!(worker.active_task_count(), 1);
		drop(workers);

		// Act
		balancer.task_completed(&worker_id).await.unwrap();

		// Assert
		let workers = balancer.workers.read().await;
		let worker = workers.iter().find(|w| w.id == worker_id).unwrap();
		assert_eq!(worker.active_task_count(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_update_metrics() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 1))
			.await
			.unwrap();

		let metrics = WorkerMetrics::with_values(10, 2, Duration::from_millis(500));

		// Act
		balancer
			.update_metrics("worker-1", metrics.clone())
			.await
			.unwrap();

		// Assert
		let stats = balancer.get_worker_stats().await;
		let worker_metrics = stats.get("worker-1").unwrap();
		assert_eq!(worker_metrics.tasks_completed, 10);
		assert_eq!(worker_metrics.tasks_failed, 2);
		assert_eq!(
			worker_metrics.average_execution_time,
			Duration::from_millis(500)
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_get_worker_stats() {
		// Arrange
		let balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 1))
			.await
			.unwrap();
		balancer
			.register_worker(WorkerInfo::new("worker-2".to_string(), 1))
			.await
			.unwrap();

		// Act
		let stats = balancer.get_worker_stats().await;

		// Assert
		assert_eq!(stats.len(), 2);
		assert!(stats.contains_key("worker-1"));
		assert!(stats.contains_key("worker-2"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_decrement_tasks_at_zero_does_not_underflow() {
		// Arrange
		let worker = WorkerInfo::new("worker-1".to_string(), 1);
		assert_eq!(worker.active_task_count(), 0);

		// Act - decrement at 0 should saturate, not wrap to usize::MAX
		worker.decrement_tasks();

		// Assert
		assert_eq!(worker.active_task_count(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_decrement_tasks_multiple_times_at_zero_stays_at_zero() {
		// Arrange
		let worker = WorkerInfo::new("worker-1".to_string(), 1);
		worker.increment_tasks();
		worker.decrement_tasks();
		assert_eq!(worker.active_task_count(), 0);

		// Act - multiple decrements below zero should all saturate at 0
		worker.decrement_tasks();
		worker.decrement_tasks();
		worker.decrement_tasks();

		// Assert
		assert_eq!(worker.active_task_count(), 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_weighted_strategy_zero_total_weight_does_not_panic() {
		// Arrange
		let mut weights = HashMap::new();
		weights.insert("worker-1".to_string(), 0);
		weights.insert("worker-2".to_string(), 0);

		let balancer = LoadBalancer::new(LoadBalancingStrategy::Weighted(weights));
		balancer
			.register_worker(WorkerInfo::new("worker-1".to_string(), 0))
			.await
			.unwrap();
		balancer
			.register_worker(WorkerInfo::new("worker-2".to_string(), 0))
			.await
			.unwrap();

		// Act - should not panic, returns first worker as fallback
		let selected = balancer.select_worker().await.unwrap();

		// Assert
		assert!(selected == "worker-1" || selected == "worker-2");
	}

	#[rstest]
	#[tokio::test]
	async fn test_update_execution_time_does_not_overflow() {
		// Arrange
		let mut metrics = WorkerMetrics::new();
		metrics.tasks_completed = u64::MAX - 1;
		metrics.average_execution_time = Duration::from_millis(u64::MAX);

		// Act - should not overflow or panic
		metrics.record_success(Duration::from_millis(1000));

		// Assert - tasks_completed wraps via addition (that's expected for the counter)
		// but the average_execution_time calculation should not panic
		assert!(metrics.tasks_completed > 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_update_execution_time_saturates_at_u64_max() {
		// Arrange
		let mut metrics = WorkerMetrics::new();
		metrics.tasks_completed = 1;
		metrics.average_execution_time = Duration::from_millis(u64::MAX);

		// Act - saturating arithmetic should clamp instead of overflowing
		metrics.record_success(Duration::from_millis(u64::MAX));

		// Assert - the result should be clamped to u64::MAX milliseconds
		assert_eq!(
			metrics.average_execution_time,
			Duration::from_millis(u64::MAX)
		);
	}
}
