//! Integration tests for RabbitMQ task backend
//!
//! These tests verify that RabbitMQBackend correctly executes operations
//! against a real RabbitMQ container.

#![cfg(feature = "rabbitmq-backend")]

use reinhardt_tasks::backend::TaskBackend;
use reinhardt_tasks::backends::rabbitmq::{RabbitMQBackend, RabbitMQConfig};
use reinhardt_tasks::{Task, TaskExecutionError, TaskId, TaskPriority};
use serial_test::serial;
use testcontainers::{GenericImage, ImageExt, core::WaitFor, runners::AsyncRunner};

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

async fn setup_rabbitmq() -> testcontainers::ContainerAsync<GenericImage> {
	use testcontainers::core::IntoContainerPort;

	let rabbitmq_image = GenericImage::new("rabbitmq", "3-management-alpine")
		.with_exposed_port(5672.tcp())
		.with_exposed_port(15672.tcp()) // Management UI port
		.with_wait_for(WaitFor::message_on_stdout("Server startup complete"))
		.with_startup_timeout(std::time::Duration::from_secs(120));

	rabbitmq_image
		.start()
		.await
		.expect("Failed to start RabbitMQ container")
}

#[tokio::test]
#[serial(rabbitmq)]
async fn test_rabbitmq_backend_enqueue() {
	let container = setup_rabbitmq().await;
	let port = container
		.get_host_port_ipv4(5672)
		.await
		.expect("Failed to get port");
	let amqp_url = format!("amqp://guest:guest@127.0.0.1:{}/%2f", port);

	let config = RabbitMQConfig::new(&amqp_url);
	let backend = RabbitMQBackend::new(config)
		.await
		.expect("Failed to connect to RabbitMQ");

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
#[serial(rabbitmq)]
async fn test_rabbitmq_backend_dequeue() {
	let container = setup_rabbitmq().await;
	let port = container
		.get_host_port_ipv4(5672)
		.await
		.expect("Failed to get port");
	let amqp_url = format!("amqp://guest:guest@127.0.0.1:{}/%2f", port);

	let config = RabbitMQConfig::new(&amqp_url).with_queue_name("test_queue");
	let backend = RabbitMQBackend::new(config)
		.await
		.expect("Failed to connect to RabbitMQ");

	// Enqueue a task first
	let task = Box::new(TestTask {
		id: TaskId::new(),
		name: "dequeue_test".to_string(),
	});
	let task_id = task.id();
	backend.enqueue(task).await.expect("Failed to enqueue");

	// Wait a bit for message to be available
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

	// Dequeue the task
	let dequeued = backend.dequeue().await.expect("Failed to dequeue");
	assert_eq!(dequeued, Some(task_id));

	// Second dequeue should return None
	let empty = backend.dequeue().await.expect("Failed to dequeue");
	assert_eq!(empty, None);
}

#[tokio::test]
#[serial(rabbitmq)]
async fn test_rabbitmq_backend_get_status() {
	let container = setup_rabbitmq().await;
	let port = container
		.get_host_port_ipv4(5672)
		.await
		.expect("Failed to get port");
	let amqp_url = format!("amqp://guest:guest@127.0.0.1:{}/%2f", port);

	let config = RabbitMQConfig::new(&amqp_url);
	let backend = RabbitMQBackend::new(config)
		.await
		.expect("Failed to connect to RabbitMQ");

	// get_status returns NotFound for non-existent task ID
	let status_result = backend.get_status(TaskId::new()).await;
	assert!(
		matches!(status_result, Err(TaskExecutionError::NotFound(_))),
		"Expected NotFound error for non-existent task ID"
	);
}

#[tokio::test]
#[serial(rabbitmq)]
async fn test_rabbitmq_backend_multiple_tasks() {
	let container = setup_rabbitmq().await;
	let port = container
		.get_host_port_ipv4(5672)
		.await
		.expect("Failed to get port");
	let amqp_url = format!("amqp://guest:guest@127.0.0.1:{}/%2f", port);

	let config = RabbitMQConfig::new(&amqp_url).with_queue_name("multi_queue");
	let backend = RabbitMQBackend::new(config)
		.await
		.expect("Failed to connect to RabbitMQ");

	// Enqueue multiple tasks
	let task1_id = TaskId::new();
	let task2_id = TaskId::new();
	let task3_id = TaskId::new();

	backend
		.enqueue(Box::new(TestTask {
			id: task1_id,
			name: "task1".to_string(),
		}))
		.await
		.expect("Failed to enqueue task1");

	backend
		.enqueue(Box::new(TestTask {
			id: task2_id,
			name: "task2".to_string(),
		}))
		.await
		.expect("Failed to enqueue task2");

	backend
		.enqueue(Box::new(TestTask {
			id: task3_id,
			name: "task3".to_string(),
		}))
		.await
		.expect("Failed to enqueue task3");

	// Wait for messages
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

	// Dequeue in FIFO order
	assert_eq!(
		backend.dequeue().await.expect("Failed to dequeue"),
		Some(task1_id)
	);
	assert_eq!(
		backend.dequeue().await.expect("Failed to dequeue"),
		Some(task2_id)
	);
	assert_eq!(
		backend.dequeue().await.expect("Failed to dequeue"),
		Some(task3_id)
	);
	assert_eq!(backend.dequeue().await.expect("Failed to dequeue"), None);
}

#[tokio::test]
#[serial(rabbitmq)]
async fn test_rabbitmq_config() {
	let config = RabbitMQConfig::new("amqp://localhost:5672/%2f")
		.with_queue_name("custom_queue")
		.with_exchange("custom_exchange")
		.with_routing_key("custom_key");

	assert_eq!(config.url, "amqp://localhost:5672/%2f");
	assert_eq!(config.queue_name, "custom_queue");
	assert_eq!(config.exchange_name, "custom_exchange");
	assert_eq!(config.routing_key, "custom_key");
}
