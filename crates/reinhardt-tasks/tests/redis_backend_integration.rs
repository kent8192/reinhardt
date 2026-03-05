//! Redis backend integration tests
//!
//! Tests task enqueue/dequeue, status management, and basic Redis operations.

#![cfg(feature = "redis-backend")]

use reinhardt_tasks::backend::TaskBackend;
use reinhardt_tasks::backends::redis::RedisTaskBackend;
use reinhardt_tasks::{Task, TaskId, TaskPriority, TaskStatus};
use reinhardt_testkit::fixtures::redis_container;
use rstest::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::GenericImage;

/// Simple test task implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestTask {
	id: TaskId,
	name: String,
	priority: TaskPriority,
}

impl TestTask {
	fn new(name: impl Into<String>) -> Self {
		Self {
			id: TaskId::new(),
			name: name.into(),
			priority: TaskPriority::new(5), // Default priority
		}
	}

	fn with_priority(mut self, priority: i32) -> Self {
		self.priority = TaskPriority::new(priority);
		self
	}
}

impl Task for TestTask {
	fn id(&self) -> TaskId {
		self.id
	}

	fn name(&self) -> &str {
		&self.name
	}

	fn priority(&self) -> TaskPriority {
		self.priority
	}
}

/// Test: Redis backend initialization
#[rstest]
#[tokio::test]
async fn test_redis_backend_initialization(
	#[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = redis_container.await;

	let backend = RedisTaskBackend::new(&url).await;
	assert!(backend.is_ok(), "Should connect to Redis successfully");
	assert_eq!(backend.unwrap().backend_name(), "redis");
}

/// Test: Task enqueue and dequeue
#[rstest]
#[tokio::test]
async fn test_task_enqueue_dequeue(
	#[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = redis_container.await;
	let backend = RedisTaskBackend::new(&url).await.unwrap();

	// Enqueue task
	let task = TestTask::new("test_task_1");
	let task_id = task.id();
	backend.enqueue(Box::new(task)).await.unwrap();

	// Dequeue task
	let dequeued_id = backend.dequeue().await.unwrap();
	assert_eq!(dequeued_id, Some(task_id));
}

/// Test: Task status management
#[rstest]
#[tokio::test]
async fn test_task_status_management(
	#[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = redis_container.await;
	let backend = RedisTaskBackend::new(&url).await.unwrap();

	let task = TestTask::new("status_test");
	let task_id = task.id();
	backend.enqueue(Box::new(task)).await.unwrap();

	// Check initial status
	let status = backend.get_status(task_id).await.unwrap();
	assert_eq!(status, TaskStatus::Pending);

	// Update status to Running
	backend
		.update_status(task_id, TaskStatus::Running)
		.await
		.unwrap();
	let status = backend.get_status(task_id).await.unwrap();
	assert_eq!(status, TaskStatus::Running);

	// Update status to Success
	backend
		.update_status(task_id, TaskStatus::Success)
		.await
		.unwrap();
	let status = backend.get_status(task_id).await.unwrap();
	assert_eq!(status, TaskStatus::Success);
}

/// Test: FIFO queue behavior
#[rstest]
#[tokio::test]
async fn test_fifo_queue(#[future] redis_container: (ContainerAsync<GenericImage>, u16, String)) {
	let (_container, _port, url) = redis_container.await;
	let backend = Arc::new(RedisTaskBackend::new(&url).await.unwrap());

	// Enqueue tasks with different priorities (Redis backend currently ignores priority)
	let task1 = TestTask::new("task_1").with_priority(0);
	let task2 = TestTask::new("task_2").with_priority(5);
	let task3 = TestTask::new("task_3").with_priority(9);

	let id1 = task1.id();
	let id2 = task2.id();
	let id3 = task3.id();

	// Enqueue in order
	backend.enqueue(Box::new(task1)).await.unwrap();
	backend.enqueue(Box::new(task2)).await.unwrap();
	backend.enqueue(Box::new(task3)).await.unwrap();

	// Should dequeue in FIFO order (task1 -> task2 -> task3)
	// Note: Current Redis backend doesn't implement priority queue
	assert_eq!(backend.dequeue().await.unwrap(), Some(id1));
	assert_eq!(backend.dequeue().await.unwrap(), Some(id2));
	assert_eq!(backend.dequeue().await.unwrap(), Some(id3));
}

/// Test: Multiple tasks enqueue/dequeue
#[rstest]
#[tokio::test]
async fn test_multiple_tasks(
	#[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = redis_container.await;
	let backend = RedisTaskBackend::new(&url).await.unwrap();

	// Enqueue 10 tasks
	let mut task_ids = Vec::new();
	for i in 0..10 {
		let task = TestTask::new(format!("task_{}", i));
		task_ids.push(task.id());
		backend.enqueue(Box::new(task)).await.unwrap();
	}

	// Dequeue all tasks
	for expected_id in task_ids {
		let dequeued = backend.dequeue().await.unwrap();
		assert_eq!(dequeued, Some(expected_id));
	}

	// Queue should be empty
	let empty = backend.dequeue().await.unwrap();
	assert_eq!(empty, None);
}

/// Test: Concurrent task operations
#[rstest]
#[tokio::test]
async fn test_concurrent_operations(
	#[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = redis_container.await;
	let backend = Arc::new(RedisTaskBackend::new(&url).await.unwrap());

	// Spawn 5 concurrent enqueue operations
	let mut handles = Vec::new();
	for i in 0..5 {
		let backend = Arc::clone(&backend);
		let handle = tokio::spawn(async move {
			let task = TestTask::new(format!("concurrent_task_{}", i));
			backend.enqueue(Box::new(task)).await.unwrap();
		});
		handles.push(handle);
	}

	// Wait for all enqueues to complete
	for handle in handles {
		handle.await.unwrap();
	}

	// Should have 5 tasks in queue
	for _ in 0..5 {
		let dequeued = backend.dequeue().await.unwrap();
		assert!(dequeued.is_some());
	}

	// Queue should be empty
	let empty = backend.dequeue().await.unwrap();
	assert_eq!(empty, None);
}

/// Test: Task data retrieval
#[rstest]
#[tokio::test]
async fn test_task_data_retrieval(
	#[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = redis_container.await;
	let backend = RedisTaskBackend::new(&url).await.unwrap();

	let task = TestTask::new("data_test");
	let task_id = task.id();
	backend.enqueue(Box::new(task)).await.unwrap();

	// Retrieve task data
	let task_data = backend.get_task_data(task_id).await.unwrap();
	assert!(task_data.is_some(), "Should retrieve task data");

	// Non-existent task
	let non_existent = backend.get_task_data(TaskId::new()).await.unwrap();
	assert!(non_existent.is_none());
}

/// Test: Empty queue dequeue
#[rstest]
#[tokio::test]
async fn test_empty_queue_dequeue(
	#[future] redis_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = redis_container.await;
	let backend = RedisTaskBackend::new(&url).await.unwrap();

	// Dequeue from empty queue
	let result = backend.dequeue().await.unwrap();
	assert_eq!(result, None);
}
