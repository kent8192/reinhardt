//! Integration tests for Redis-based task locking
//!
//! These tests verify that RedisTaskLock correctly executes distributed
//! locking operations against a real Redis container.

#![cfg(feature = "redis-backend")]

use reinhardt_tasks::TaskId;
use reinhardt_tasks::locking::{RedisTaskLock, TaskLock};
use serial_test::serial;
use std::time::Duration;
use testcontainers::{
	GenericImage,
	core::{ContainerPort, WaitFor},
	runners::AsyncRunner,
};

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
#[serial(redis_lock)]
async fn test_redis_lock_acquire() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let lock = RedisTaskLock::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");
	let task_id = TaskId::new();

	let acquired = lock
		.acquire(task_id, Duration::from_secs(60))
		.await
		.unwrap();
	assert!(acquired);
}

#[tokio::test]
#[serial(redis_lock)]
async fn test_redis_lock_already_locked() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let lock = RedisTaskLock::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");
	let task_id = TaskId::new();

	lock.acquire(task_id, Duration::from_secs(60))
		.await
		.unwrap();
	let acquired = lock
		.acquire(task_id, Duration::from_secs(60))
		.await
		.unwrap();
	assert!(!acquired);
}

#[tokio::test]
#[serial(redis_lock)]
async fn test_redis_lock_release() {
	let container = setup_redis().await;
	let port = container
		.get_host_port_ipv4(6379)
		.await
		.expect("Failed to get port");
	let redis_url = format!("redis://127.0.0.1:{}/", port);

	let lock = RedisTaskLock::new(&redis_url)
		.await
		.expect("Failed to connect to Redis");
	let task_id = TaskId::new();

	lock.acquire(task_id, Duration::from_secs(60))
		.await
		.unwrap();
	lock.release(task_id).await.unwrap();

	let is_locked = lock.is_locked(task_id).await.unwrap();
	assert!(!is_locked);
}
