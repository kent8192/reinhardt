//! RabbitMQ backend integration tests
//!
//! Tests task enqueue/dequeue, durability, message persistence, and RabbitMQ-specific features.

#![cfg(feature = "rabbitmq-backend")]

use reinhardt_tasks::backend::TaskBackend;
use reinhardt_tasks::backends::rabbitmq::RabbitMQBackend;
use reinhardt_tasks::{RabbitMQConfig, Task, TaskId, TaskPriority, TaskStatus};
use reinhardt_testkit::fixtures::rabbitmq_container;
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

/// Test: RabbitMQ backend initialization
#[rstest]
#[tokio::test]
async fn test_rabbitmq_backend_initialization(
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;

	let config = RabbitMQConfig::new(&url);
	let backend = RabbitMQBackend::new(config).await;
	assert!(backend.is_ok(), "Should connect to RabbitMQ successfully");
	assert_eq!(backend.unwrap().backend_name(), "rabbitmq");
}

/// Test: Task enqueue and dequeue
#[rstest]
#[tokio::test]
async fn test_task_enqueue_dequeue(
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;
	let config = RabbitMQConfig::new(&url);
	let backend = RabbitMQBackend::new(config).await.unwrap();

	// Enqueue task
	let task = TestTask::new("test_task_1");
	let task_id = task.id();
	backend.enqueue(Box::new(task)).await.unwrap();

	// Dequeue task
	let dequeued_id = backend.dequeue().await.unwrap();
	assert_eq!(dequeued_id, Some(task_id));
}

/// Test: Task status operations (RabbitMQ specific behavior)
///
/// Note: RabbitMQ backend uses a pluggable metadata store for status tracking.
/// Status updates are persisted via the metadata store (default: InMemory).
/// This allows RabbitMQ to provide the same functionality as Redis backend.
#[rstest]
#[tokio::test]
async fn test_task_status_operations(
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;
	let config = RabbitMQConfig::new(&url);
	let backend = RabbitMQBackend::new(config).await.unwrap();

	let task = TestTask::new("status_test");
	let task_id = task.id();
	backend.enqueue(Box::new(task)).await.unwrap();

	// Check initial status (should be Pending)
	let status = backend.get_status(task_id).await.unwrap();
	assert_eq!(status, TaskStatus::Pending);

	// Update status operations succeed but are no-ops in RabbitMQ
	backend
		.update_status(task_id, TaskStatus::Running)
		.await
		.unwrap();

	// Status is now Running (RabbitMQ persists status updates via metadata store)
	let status = backend.get_status(task_id).await.unwrap();
	assert_eq!(
		status,
		TaskStatus::Running,
		"RabbitMQ backend persists status updates via metadata store"
	);
}

/// Test: Multiple tasks enqueue/dequeue (FIFO order)
#[rstest]
#[tokio::test]
async fn test_multiple_tasks(
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;
	let config = RabbitMQConfig::new(&url);
	let backend = RabbitMQBackend::new(config).await.unwrap();

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
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;
	let config = RabbitMQConfig::new(&url);
	let backend = Arc::new(RabbitMQBackend::new(config).await.unwrap());

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

/// Test: Task data retrieval (RabbitMQ specific behavior)
///
/// Note: RabbitMQ backend uses a pluggable metadata store to persist task data.
/// Task data can be retrieved by ID even after dequeue from the message queue.
/// This allows RabbitMQ to provide the same functionality as Redis backend.
#[rstest]
#[tokio::test]
async fn test_task_data_retrieval(
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;
	let config = RabbitMQConfig::new(&url);
	let backend = RabbitMQBackend::new(config).await.unwrap();

	let task = TestTask::new("data_test");
	let task_id = task.id();
	backend.enqueue(Box::new(task)).await.unwrap();

	// Task data is retrievable by ID via metadata store
	let task_data = backend.get_task_data(task_id).await.unwrap();
	assert!(
		task_data.is_some(),
		"RabbitMQ backend supports task data retrieval via metadata store"
	);

	// Non-existent task also returns None
	let non_existent = backend.get_task_data(TaskId::new()).await.unwrap();
	assert!(non_existent.is_none());
}

/// Test: Empty queue dequeue
#[rstest]
#[tokio::test]
async fn test_empty_queue_dequeue(
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;
	let config = RabbitMQConfig::new(&url);
	let backend = RabbitMQBackend::new(config).await.unwrap();

	// Dequeue from empty queue
	let result = backend.dequeue().await.unwrap();
	assert_eq!(result, None);
}

/// Test: Custom queue name configuration
#[rstest]
#[tokio::test]
async fn test_custom_queue_name(
	#[future] rabbitmq_container: (ContainerAsync<GenericImage>, u16, String),
) {
	let (_container, _port, url) = rabbitmq_container.await;
	let config = RabbitMQConfig::new(&url).with_queue_name("custom_tasks");
	let backend = RabbitMQBackend::new(config).await.unwrap();

	let task = TestTask::new("custom_queue_test");
	let task_id = task.id();
	backend.enqueue(Box::new(task)).await.unwrap();

	let dequeued = backend.dequeue().await.unwrap();
	assert_eq!(dequeued, Some(task_id));
}
