//! Integration tests for Redis PubSub cache invalidation
//!
//! These tests verify the Redis Pub/Sub implementation for cache invalidation
//! using TestContainers and Redis, testing message publishing, subscription,
//! and multi-subscriber scenarios.

use reinhardt_test::fixtures::testcontainers::redis_container;
use reinhardt_utils::cache::{CacheInvalidationChannel, CacheInvalidationMessage};
use rstest::*;
use testcontainers::ContainerAsync;
use tokio::time::{Duration, sleep};

/// Fixture providing Redis container and connection URL for cache pubsub testing
///
/// Test intent: Provide a Redis instance for testing cache invalidation pub/sub functionality
///
/// This fixture chains from the standard redis_container fixture and provides
/// the container and connection URL in the format expected by CacheInvalidationChannel.
#[fixture]
async fn redis_fixture(
	#[future] redis_container: (ContainerAsync<testcontainers::GenericImage>, u16, String),
) -> (ContainerAsync<testcontainers::GenericImage>, String) {
	let (container, _port, url) = redis_container.await;
	(container, url)
}

#[rstest]
#[tokio::test]
async fn test_redis_pubsub_basic(
	#[future] redis_fixture: (ContainerAsync<testcontainers::GenericImage>, String),
) {
	let (_container, redis_url) = redis_fixture.await;

	// Create channel for publishing and subscribing
	let channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create channel");

	let mut subscriber = channel
		.subscribe()
		.await
		.expect("Failed to create subscriber");

	// Publish message in background task
	let publish_channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create publish channel");

	tokio::spawn(async move {
		sleep(Duration::from_millis(100)).await;
		publish_channel
			.invalidate("test_key")
			.await
			.expect("Failed to publish message");
	});

	// Receive message
	let msg = subscriber
		.next_message()
		.await
		.expect("Failed to receive message");

	match msg {
		Some(CacheInvalidationMessage::InvalidateKey { key }) => {
			assert_eq!(key, "test_key");
		}
		_ => panic!("Expected InvalidateKey message"),
	}
}

#[rstest]
#[tokio::test]
async fn test_redis_pubsub_pattern_invalidation(
	#[future] redis_fixture: (ContainerAsync<testcontainers::GenericImage>, String),
) {
	let (_container, redis_url) = redis_fixture.await;

	// Create channel for publishing and subscribing
	let channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create channel");

	let mut subscriber = channel
		.subscribe()
		.await
		.expect("Failed to create subscriber");

	// Publish pattern invalidation message
	let publish_channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create publish channel");

	tokio::spawn(async move {
		sleep(Duration::from_millis(100)).await;
		publish_channel
			.invalidate_pattern("user:*")
			.await
			.expect("Failed to publish pattern message");
	});

	// Receive message
	let msg = subscriber
		.next_message()
		.await
		.expect("Failed to receive message");

	match msg {
		Some(CacheInvalidationMessage::InvalidatePattern { pattern }) => {
			assert_eq!(pattern, "user:*");
		}
		_ => panic!("Expected InvalidatePattern message"),
	}
}

#[rstest]
#[tokio::test]
async fn test_redis_pubsub_clear_all(
	#[future] redis_fixture: (ContainerAsync<testcontainers::GenericImage>, String),
) {
	let (_container, redis_url) = redis_fixture.await;

	// Create channel for publishing and subscribing
	let channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create channel");

	let mut subscriber = channel
		.subscribe()
		.await
		.expect("Failed to create subscriber");

	// Publish clear all message
	let publish_channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create publish channel");

	tokio::spawn(async move {
		sleep(Duration::from_millis(100)).await;
		publish_channel
			.clear_all()
			.await
			.expect("Failed to publish clear all message");
	});

	// Receive message
	let msg = subscriber
		.next_message()
		.await
		.expect("Failed to receive message");

	match msg {
		Some(CacheInvalidationMessage::ClearAll) => {
			// Success
		}
		_ => panic!("Expected ClearAll message"),
	}
}

#[rstest]
#[tokio::test]
async fn test_redis_pubsub_multiple_subscribers(
	#[future] redis_fixture: (ContainerAsync<testcontainers::GenericImage>, String),
) {
	let (_container, redis_url) = redis_fixture.await;

	// Create channel
	let channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create channel");

	// Create two subscribers
	let mut subscriber1 = channel
		.subscribe()
		.await
		.expect("Failed to create subscriber1");

	let channel2 = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create channel2");

	let mut subscriber2 = channel2
		.subscribe()
		.await
		.expect("Failed to create subscriber2");

	// Publish message
	let publish_channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create publish channel");

	tokio::spawn(async move {
		sleep(Duration::from_millis(100)).await;
		publish_channel
			.invalidate("multi_key")
			.await
			.expect("Failed to publish message");
	});

	// Both subscribers should receive the message
	let msg1 = subscriber1
		.next_message()
		.await
		.expect("Failed to receive message from subscriber1");

	let msg2 = subscriber2
		.next_message()
		.await
		.expect("Failed to receive message from subscriber2");

	// Verify both received the same message
	match (msg1, msg2) {
		(
			Some(CacheInvalidationMessage::InvalidateKey { key: key1 }),
			Some(CacheInvalidationMessage::InvalidateKey { key: key2 }),
		) => {
			assert_eq!(key1, "multi_key");
			assert_eq!(key2, "multi_key");
		}
		_ => panic!("Expected InvalidateKey messages from both subscribers"),
	}
}

#[rstest]
#[tokio::test]
async fn test_redis_pubsub_multiple_messages(
	#[future] redis_fixture: (ContainerAsync<testcontainers::GenericImage>, String),
) {
	let (_container, redis_url) = redis_fixture.await;

	// Create channel
	let channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create channel");

	let mut subscriber = channel
		.subscribe()
		.await
		.expect("Failed to create subscriber");

	// Publish multiple messages
	let publish_channel = CacheInvalidationChannel::new(&redis_url)
		.await
		.expect("Failed to create publish channel");

	tokio::spawn(async move {
		sleep(Duration::from_millis(100)).await;
		publish_channel
			.invalidate("key1")
			.await
			.expect("Failed to publish message 1");
		publish_channel
			.invalidate("key2")
			.await
			.expect("Failed to publish message 2");
		publish_channel
			.invalidate_pattern("user:*")
			.await
			.expect("Failed to publish message 3");
	});

	// Receive all three messages
	let msg1 = subscriber
		.next_message()
		.await
		.expect("Failed to receive message 1");
	let msg2 = subscriber
		.next_message()
		.await
		.expect("Failed to receive message 2");
	let msg3 = subscriber
		.next_message()
		.await
		.expect("Failed to receive message 3");

	// Verify messages
	match msg1 {
		Some(CacheInvalidationMessage::InvalidateKey { key }) => {
			assert_eq!(key, "key1");
		}
		_ => panic!("Expected InvalidateKey message for key1"),
	}

	match msg2 {
		Some(CacheInvalidationMessage::InvalidateKey { key }) => {
			assert_eq!(key, "key2");
		}
		_ => panic!("Expected InvalidateKey message for key2"),
	}

	match msg3 {
		Some(CacheInvalidationMessage::InvalidatePattern { pattern }) => {
			assert_eq!(pattern, "user:*");
		}
		_ => panic!("Expected InvalidatePattern message"),
	}
}

#[rstest]
#[tokio::test]
async fn test_redis_pubsub_custom_channel_name(
	#[future] redis_fixture: (ContainerAsync<testcontainers::GenericImage>, String),
) {
	let (_container, redis_url) = redis_fixture.await;

	// Create channel with custom name
	let custom_channel_name = "custom:cache:invalidation".to_string();
	let channel =
		CacheInvalidationChannel::with_channel_name(&redis_url, custom_channel_name.clone())
			.await
			.expect("Failed to create channel");

	let mut subscriber = channel
		.subscribe()
		.await
		.expect("Failed to create subscriber");

	// Publish message
	let publish_channel =
		CacheInvalidationChannel::with_channel_name(&redis_url, custom_channel_name)
			.await
			.expect("Failed to create publish channel");

	tokio::spawn(async move {
		sleep(Duration::from_millis(100)).await;
		publish_channel
			.invalidate("custom_key")
			.await
			.expect("Failed to publish message");
	});

	// Receive message
	let msg = subscriber
		.next_message()
		.await
		.expect("Failed to receive message");

	match msg {
		Some(CacheInvalidationMessage::InvalidateKey { key }) => {
			assert_eq!(key, "custom_key");
		}
		_ => panic!("Expected InvalidateKey message"),
	}
}
