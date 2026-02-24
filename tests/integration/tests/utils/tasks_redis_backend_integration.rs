//! Integration tests for Redis task backend
//!
//! These tests verify that RedisTaskBackend and RedisTaskResultBackend
//! correctly execute operations against a real Redis container.

#![cfg(feature = "redis-backend")]

use reinhardt_tasks::backend::TaskBackend;
use reinhardt_tasks::backends::redis::{RedisTaskBackend, RedisTaskResultBackend};
use reinhardt_tasks::result::ResultBackend;
use reinhardt_tasks::{Task, TaskExecutionError, TaskId, TaskPriority, TaskStatus};
use serial_test::serial;
use testcontainers::{
	GenericImage,
	core::{ContainerPort, WaitFor},
	runners::AsyncRunner,
};

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

async fn setup_redis() -> testcontainers::ContainerAsync<GenericImage> {
	let redis_image = GenericImage::new("redis", "7-alpine")
		.with_exposed_port(ContainerPort::Tcp(6379))
		.with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"));

	redis_image
		.start()
		.await
		.expect("Failed to start Redis container")
}

#[tokio::test]
#[serial(redis)]
async fn test_redis_backend_enqueue() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let backend = RedisTaskBackend::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");

	let task = Box::new(TestTask {
		id: TaskId::new(),
		name: "test_task".to_string(),
	});

	let task_id = task.id();
	let result = backend.enqueue(task).await;
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), task_id);
}

#[tokio::test]
#[serial(redis)]
async fn test_redis_backend_get_status() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let backend = RedisTaskBackend::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");

	let task = Box::new(TestTask {
		id: TaskId::new(),
		name: "test_task".to_string(),
	});

	let task_id = task.id();
	backend.enqueue(task).await.expect("Failed to enqueue");

	let status = backend
		.get_status(task_id)
		.await
		.expect("Failed to get status");
	assert_eq!(status, TaskStatus::Pending);
}

#[tokio::test]
#[serial(redis)]
async fn test_redis_backend_not_found() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let backend = RedisTaskBackend::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");

	let result = backend.get_status(TaskId::new()).await;
	assert!(result.is_err());
	assert!(matches!(result, Err(TaskExecutionError::NotFound(_))));
}

#[tokio::test]
#[serial(redis)]
async fn test_redis_backend_dequeue() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let backend = RedisTaskBackend::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");

	// Enqueue a task first
	let task = Box::new(TestTask {
		id: TaskId::new(),
		name: "dequeue_test".to_string(),
	});
	let task_id = task.id();
	backend.enqueue(task).await.expect("Failed to enqueue");

	// Dequeue the task
	let dequeued = backend.dequeue().await.expect("Failed to dequeue");
	assert_eq!(dequeued, Some(task_id));

	// Second dequeue should return None
	let empty = backend.dequeue().await.expect("Failed to dequeue");
	assert_eq!(empty, None);
}

#[tokio::test]
#[serial(redis)]
async fn test_redis_backend_update_status() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let backend = RedisTaskBackend::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");

	// Enqueue a task
	let task = Box::new(TestTask {
		id: TaskId::new(),
		name: "status_test".to_string(),
	});
	let task_id = task.id();
	backend.enqueue(task).await.expect("Failed to enqueue");

	// Update status
	backend
		.update_status(task_id, TaskStatus::Success)
		.await
		.expect("Failed to update status");

	// Verify status
	let status = backend
		.get_status(task_id)
		.await
		.expect("Failed to get status");
	assert_eq!(status, TaskStatus::Success);
}

#[tokio::test]
#[serial(redis)]
async fn test_redis_result_backend_store_and_retrieve() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let backend = RedisTaskResultBackend::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");

	let task_id = TaskId::new();
	let metadata = reinhardt_tasks::result::TaskResultMetadata::new(
		task_id,
		TaskStatus::Success,
		Some("Test result".to_string()),
	);

	// Store result
	backend
		.store_result(metadata.clone())
		.await
		.expect("Failed to store result");

	// Retrieve result
	let retrieved = backend
		.get_result(task_id)
		.await
		.expect("Failed to get result");
	assert!(retrieved.is_some());
	assert_eq!(retrieved.unwrap().result(), Some("Test result"));
}

#[tokio::test]
#[serial(redis)]
async fn test_redis_result_backend_delete() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let backend = RedisTaskResultBackend::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");

	let task_id = TaskId::new();
	let metadata =
		reinhardt_tasks::result::TaskResultMetadata::new(task_id, TaskStatus::Success, None);

	// Store and then delete
	backend
		.store_result(metadata)
		.await
		.expect("Failed to store result");
	backend
		.delete_result(task_id)
		.await
		.expect("Failed to delete result");

	// Verify deleted
	let retrieved = backend
		.get_result(task_id)
		.await
		.expect("Failed to get result");
	assert!(retrieved.is_none());
}
