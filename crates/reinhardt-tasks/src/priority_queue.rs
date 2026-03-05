//! Priority task queue with weighted scheduling

use crate::{Task, TaskResult};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Type alias for priority queue map
type PriorityQueueMap = BTreeMap<Priority, VecDeque<Box<dyn Task>>>;

/// Priority level for tasks
///
/// Ordering is based on weight values: `Low` (10) < `Normal` (50) < `High` (100).
/// `Custom(w)` is ordered by its weight value relative to the standard priorities.
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::Priority;
///
/// let high = Priority::High;
/// let normal = Priority::Normal;
/// let low = Priority::Low;
/// assert!(high > normal);
/// assert!(normal > low);
///
/// // Custom priority is ordered by weight value
/// let custom_75 = Priority::Custom(75);
/// assert!(custom_75 > normal);  // 75 > 50
/// assert!(custom_75 < high);    // 75 < 100
///
/// let custom_200 = Priority::Custom(200);
/// assert!(custom_200 > high);   // 200 > 100
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub enum Priority {
	/// Low priority (weight: 10)
	Low,
	/// Normal priority (weight: 50)
	#[default]
	Normal,
	/// High priority (weight: 100)
	High,
	/// Custom priority with specified weight
	Custom(u32),
}

impl PartialEq for Priority {
	fn eq(&self, other: &Self) -> bool {
		self.default_weight() == other.default_weight()
	}
}

impl Eq for Priority {}

impl std::hash::Hash for Priority {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.default_weight().hash(state);
	}
}

impl PartialOrd for Priority {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Priority {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.default_weight().cmp(&other.default_weight())
	}
}

impl Priority {
	/// Get the default weight for this priority
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::Priority;
	///
	/// assert_eq!(Priority::High.default_weight(), 100);
	/// assert_eq!(Priority::Normal.default_weight(), 50);
	/// assert_eq!(Priority::Low.default_weight(), 10);
	/// assert_eq!(Priority::Custom(75).default_weight(), 75);
	/// ```
	pub fn default_weight(&self) -> u32 {
		match self {
			Priority::High => 100,
			Priority::Normal => 50,
			Priority::Low => 10,
			Priority::Custom(weight) => *weight,
		}
	}
}

/// Priority task queue with weighted scheduling
///
/// Tasks are dequeued based on their priority weights. Higher priority tasks
/// have a higher chance of being selected, but lower priority tasks are not
/// starved due to the weighted scheduling algorithm.
///
/// # Example
///
/// ```rust
/// use reinhardt_tasks::{Priority, PriorityTaskQueue};
///
/// # async fn example() -> reinhardt_tasks::TaskResult<()> {
/// let queue = PriorityTaskQueue::new();
///
/// // High priority tasks are more likely to be dequeued first
/// // but low priority tasks will also be processed
/// # Ok(())
/// # }
/// ```
// Fixes #785: counter is per-instance instead of global static
pub struct PriorityTaskQueue {
	queues: Arc<RwLock<PriorityQueueMap>>,
	weights: HashMap<Priority, u32>,
	counter: AtomicU64,
}

impl PriorityTaskQueue {
	/// Create a new priority task queue with default weights
	///
	/// Default weights:
	/// - High: 100
	/// - Normal: 50
	/// - Low: 10
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::PriorityTaskQueue;
	///
	/// let queue = PriorityTaskQueue::new();
	/// ```
	pub fn new() -> Self {
		let mut weights = HashMap::new();
		weights.insert(Priority::High, 100);
		weights.insert(Priority::Normal, 50);
		weights.insert(Priority::Low, 10);

		Self {
			queues: Arc::new(RwLock::new(BTreeMap::new())),
			weights,
			counter: AtomicU64::new(0),
		}
	}

	/// Create a new priority task queue with custom weights
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::{Priority, PriorityTaskQueue};
	/// use std::collections::HashMap;
	///
	/// let mut weights = HashMap::new();
	/// weights.insert(Priority::High, 200);
	/// weights.insert(Priority::Normal, 100);
	/// weights.insert(Priority::Low, 20);
	///
	/// let queue = PriorityTaskQueue::with_weights(weights);
	/// ```
	pub fn with_weights(weights: HashMap<Priority, u32>) -> Self {
		Self {
			queues: Arc::new(RwLock::new(BTreeMap::new())),
			weights,
			counter: AtomicU64::new(0),
		}
	}

	/// Enqueue a task with the specified priority
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::{Priority, PriorityTaskQueue};
	///
	/// # async fn example() -> reinhardt_tasks::TaskResult<()> {
	/// # struct MyTask;
	/// # impl MyTask { fn new() -> Self { MyTask } }
	/// let queue = PriorityTaskQueue::new();
	/// let task = MyTask::new();
	///
	/// // queue.enqueue(Box::new(task), Priority::High).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn enqueue(&self, task: Box<dyn Task>, priority: Priority) -> TaskResult<()> {
		let mut queues = self.queues.write().await;
		queues.entry(priority).or_default().push_back(task);
		Ok(())
	}

	/// Dequeue a task using weighted scheduling
	///
	/// Tasks are selected based on their priority weights. Higher priority
	/// tasks have a higher probability of being selected, but lower priority
	/// tasks are not starved.
	///
	/// Returns `None` if the queue is empty.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_tasks::PriorityTaskQueue;
	///
	/// # async fn example() -> reinhardt_tasks::TaskResult<()> {
	/// let queue = PriorityTaskQueue::new();
	///
	/// if let Some(task) = queue.dequeue().await? {
	///     // Process task
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub async fn dequeue(&self) -> TaskResult<Option<Box<dyn Task>>> {
		let mut queues = self.queues.write().await;

		if queues.is_empty() {
			return Ok(None);
		}

		// Calculate total weight of non-empty queues
		let mut total_weight = 0u32;
		let mut priorities_with_weight = Vec::new();

		for (priority, queue) in queues.iter() {
			if !queue.is_empty() {
				let weight = self.weights.get(priority).copied().unwrap_or_else(|| {
					if let Priority::Custom(w) = priority {
						*w
					} else {
						priority.default_weight()
					}
				});
				total_weight += weight;
				priorities_with_weight.push((*priority, weight));
			}
		}

		if total_weight == 0 {
			return Ok(None);
		}

		// Select a priority based on weights
		// Use a simple counter-based approach for deterministic weighted round-robin
		let selected_priority =
			self.select_priority_weighted(&priorities_with_weight, total_weight);

		// Dequeue from the selected priority
		if let Some(queue) = queues.get_mut(&selected_priority)
			&& let Some(task) = queue.pop_front()
		{
			return Ok(Some(task));
		}

		Ok(None)
	}

	/// Select a priority using weighted round-robin
	fn select_priority_weighted(
		&self,
		priorities: &[(Priority, u32)],
		total_weight: u32,
	) -> Priority {
		// Simple weighted selection: iterate through priorities in order
		// and select based on accumulated weights
		// This ensures FIFO within same priority and fair distribution

		// Fixes #785: use instance counter instead of global static to avoid
		// cross-instance interference between independent queue instances
		let counter = self.counter.fetch_add(1, Ordering::Relaxed);
		let target = (counter % total_weight as u64) as u32;

		let mut accumulated = 0;
		for (priority, weight) in priorities {
			accumulated += weight;
			if target < accumulated {
				return *priority;
			}
		}

		// Fallback to highest priority
		priorities
			.first()
			.map(|(p, _)| *p)
			.unwrap_or(Priority::Normal)
	}

	/// Get the total number of tasks in all queues
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::PriorityTaskQueue;
	///
	/// # async fn example() {
	/// let queue = PriorityTaskQueue::new();
	/// assert_eq!(queue.len().await, 0);
	/// # }
	/// ```
	pub async fn len(&self) -> usize {
		let queues = self.queues.read().await;
		queues.values().map(|q| q.len()).sum()
	}

	/// Check if the queue is empty
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::PriorityTaskQueue;
	///
	/// # async fn example() {
	/// let queue = PriorityTaskQueue::new();
	/// assert!(queue.is_empty().await);
	/// # }
	/// ```
	pub async fn is_empty(&self) -> bool {
		let queues = self.queues.read().await;
		queues.values().all(|q| q.is_empty())
	}

	/// Get the number of tasks for a specific priority
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_tasks::{Priority, PriorityTaskQueue};
	///
	/// # async fn example() {
	/// let queue = PriorityTaskQueue::new();
	/// assert_eq!(queue.len_for_priority(Priority::High).await, 0);
	/// # }
	/// ```
	pub async fn len_for_priority(&self, priority: Priority) -> usize {
		let queues = self.queues.read().await;
		queues.get(&priority).map(|q| q.len()).unwrap_or(0)
	}
}

impl Default for PriorityTaskQueue {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::TaskId;

	#[derive(Debug)]
	struct TestTask {
		id: TaskId,
		name: String,
	}

	impl TestTask {
		fn new(name: &str) -> Self {
			Self {
				id: TaskId::new(),
				name: name.to_string(),
			}
		}
	}

	impl Task for TestTask {
		fn id(&self) -> TaskId {
			self.id
		}

		fn name(&self) -> &str {
			&self.name
		}
	}

	#[tokio::test]
	async fn test_priority_ordering() {
		let queue = PriorityTaskQueue::new();

		// Enqueue tasks with different priorities
		queue
			.enqueue(Box::new(TestTask::new("low1")), Priority::Low)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("high1")), Priority::High)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("normal1")), Priority::Normal)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("high2")), Priority::High)
			.await
			.unwrap();

		assert_eq!(queue.len().await, 4);

		// High priority tasks should be more likely to be dequeued first
		let mut high_count = 0;
		let mut dequeued = Vec::new();

		for _ in 0..4 {
			if let Some(task) = queue.dequeue().await.unwrap() {
				dequeued.push(task.name().to_string());
				if task.name().starts_with("high") {
					high_count += 1;
				}
			}
		}

		// Should have dequeued at least one high priority task
		assert!(high_count > 0);
		assert_eq!(queue.len().await, 0);
	}

	#[tokio::test]
	async fn test_weighted_scheduling() {
		let mut weights = HashMap::new();
		weights.insert(Priority::High, 90);
		weights.insert(Priority::Normal, 9);
		weights.insert(Priority::Low, 1);

		let queue = PriorityTaskQueue::with_weights(weights);

		// Enqueue more high priority tasks to match the weight ratio
		// This ensures we can observe the weighted scheduling behavior
		for i in 0..30 {
			queue
				.enqueue(
					Box::new(TestTask::new(&format!("high{}", i))),
					Priority::High,
				)
				.await
				.unwrap();
		}
		for i in 0..10 {
			queue
				.enqueue(
					Box::new(TestTask::new(&format!("normal{}", i))),
					Priority::Normal,
				)
				.await
				.unwrap();
		}
		for i in 0..5 {
			queue
				.enqueue(Box::new(TestTask::new(&format!("low{}", i))), Priority::Low)
				.await
				.unwrap();
		}

		let mut high_count = 0;
		let mut normal_count = 0;
		let mut low_count = 0;

		// Dequeue all tasks and count by priority
		while let Some(task) = queue.dequeue().await.unwrap() {
			if task.name().starts_with("high") {
				high_count += 1;
			} else if task.name().starts_with("normal") {
				normal_count += 1;
			} else if task.name().starts_with("low") {
				low_count += 1;
			}
		}

		// Verify all tasks were dequeued
		assert_eq!(high_count + normal_count + low_count, 45);

		// All priorities should get at least some tasks (no starvation)
		assert!(high_count > 0, "High priority tasks should be dequeued");
		assert!(normal_count > 0, "Normal priority tasks should be dequeued");
		assert!(low_count > 0, "Low priority tasks should be dequeued");

		// High priority should get more tasks than normal
		assert!(
			high_count > normal_count,
			"High count {} should be greater than normal count {}",
			high_count,
			normal_count
		);

		// Normal priority should get more tasks than low
		assert!(
			normal_count > low_count,
			"Normal count {} should be greater than low count {}",
			normal_count,
			low_count
		);
	}

	#[tokio::test]
	async fn test_fifo_within_priority() {
		let queue = PriorityTaskQueue::new();

		// Enqueue multiple tasks with the same priority
		queue
			.enqueue(Box::new(TestTask::new("task1")), Priority::Normal)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("task2")), Priority::Normal)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("task3")), Priority::Normal)
			.await
			.unwrap();

		// Tasks should be dequeued in FIFO order for the same priority
		let task1 = queue.dequeue().await.unwrap().unwrap();
		let task2 = queue.dequeue().await.unwrap().unwrap();
		let task3 = queue.dequeue().await.unwrap().unwrap();

		assert_eq!(task1.name(), "task1");
		assert_eq!(task2.name(), "task2");
		assert_eq!(task3.name(), "task3");
	}

	#[tokio::test]
	async fn test_concurrent_access() {
		let queue = Arc::new(PriorityTaskQueue::new());

		// Spawn multiple tasks that enqueue
		let mut handles = vec![];
		for i in 0..10 {
			let queue_clone = queue.clone();
			handles.push(tokio::spawn(async move {
				queue_clone
					.enqueue(
						Box::new(TestTask::new(&format!("task{}", i))),
						Priority::Normal,
					)
					.await
					.unwrap();
			}));
		}

		// Wait for all enqueues to complete
		for handle in handles {
			handle.await.unwrap();
		}

		assert_eq!(queue.len().await, 10);

		// Spawn multiple tasks that dequeue
		let mut handles = vec![];
		for _ in 0..10 {
			let queue_clone = queue.clone();
			handles.push(tokio::spawn(
				async move { queue_clone.dequeue().await.unwrap() },
			));
		}

		let mut count = 0;
		for handle in handles {
			if handle.await.unwrap().is_some() {
				count += 1;
			}
		}

		assert_eq!(count, 10);
		assert!(queue.is_empty().await);
	}

	#[tokio::test]
	async fn test_custom_priority() {
		let queue = PriorityTaskQueue::new();

		queue
			.enqueue(Box::new(TestTask::new("custom75")), Priority::Custom(75))
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("high")), Priority::High)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("normal")), Priority::Normal)
			.await
			.unwrap();

		assert_eq!(queue.len().await, 3);

		// Custom(75) should be between Normal(50) and High(100)
		// Just verify all tasks are dequeued correctly
		for _ in 0..3 {
			let task = queue.dequeue().await.unwrap();
			assert!(task.is_some());
		}

		// All tasks should have been dequeued
		assert!(queue.is_empty().await);
	}

	#[tokio::test]
	async fn test_empty_queue() {
		let queue = PriorityTaskQueue::new();

		assert!(queue.is_empty().await);
		assert_eq!(queue.len().await, 0);

		let task = queue.dequeue().await.unwrap();
		assert!(task.is_none());
	}

	#[tokio::test]
	async fn test_len_for_priority() {
		let queue = PriorityTaskQueue::new();

		queue
			.enqueue(Box::new(TestTask::new("high1")), Priority::High)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("high2")), Priority::High)
			.await
			.unwrap();
		queue
			.enqueue(Box::new(TestTask::new("normal1")), Priority::Normal)
			.await
			.unwrap();

		assert_eq!(queue.len_for_priority(Priority::High).await, 2);
		assert_eq!(queue.len_for_priority(Priority::Normal).await, 1);
		assert_eq!(queue.len_for_priority(Priority::Low).await, 0);
	}

	#[test]
	fn test_priority_default_weights() {
		assert_eq!(Priority::High.default_weight(), 100);
		assert_eq!(Priority::Normal.default_weight(), 50);
		assert_eq!(Priority::Low.default_weight(), 10);
		assert_eq!(Priority::Custom(75).default_weight(), 75);
	}

	#[test]
	fn test_priority_comparison() {
		use std::cmp::Ordering;

		// Ordering is based on weight values
		assert!(Priority::High > Priority::Normal);
		assert!(Priority::Normal > Priority::Low);

		// Custom priorities are ordered by their weight value
		assert!(Priority::Custom(75) > Priority::Normal); // 75 > 50
		assert!(Priority::Custom(75) < Priority::High); // 75 < 100
		assert!(Priority::Custom(200) > Priority::High); // 200 > 100
		assert!(Priority::Custom(0) < Priority::Low); // 0 < 10

		// Custom with same weight as standard priority is equal
		assert_eq!(Priority::Custom(100), Priority::High);
		assert_eq!(Priority::Custom(50), Priority::Normal);
		assert_eq!(Priority::Custom(10), Priority::Low);
	}

	#[test]
	fn test_priority_default() {
		assert_eq!(Priority::default(), Priority::Normal);
	}
}
