//! Memcached integration tests
//!
//! Tests the MemcachedCache backend with a real Memcached container using TestContainers.

use reinhardt_utils::cache::{Cache, MemcachedCache, MemcachedConfig};
use rstest::*;
use std::time::Duration;
use testcontainers::core::ContainerPort;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

/// Default Memcached port
const MEMCACHED_PORT: u16 = 11211;

/// rstest fixture providing a Memcached container and connection string
///
/// The container is automatically cleaned up when the test ends.
#[fixture]
async fn memcached_fixture() -> (testcontainers::ContainerAsync<GenericImage>, String) {
	// Create a generic Memcached container with verbose mode enabled
	let memcached_image = GenericImage::new("memcached", "1.6-alpine")
		.with_exposed_port(ContainerPort::Tcp(MEMCACHED_PORT))
		.with_wait_for(testcontainers::core::WaitFor::message_on_stderr(
			"listening",
		))
		.with_cmd(vec!["-vv"]); // Enable verbose mode for startup logs

	// Start the container
	let container = memcached_image
		.start()
		.await
		.expect("Failed to start Memcached container");

	// Get the host port
	let host_port = container
		.get_host_port_ipv4(MEMCACHED_PORT)
		.await
		.expect("Failed to get Memcached host port");

	// Build connection string
	let connection_string = format!("127.0.0.1:{}", host_port);

	(container, connection_string)
}

#[rstest]
#[tokio::test]
async fn test_memcached_basic_operations(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let config = MemcachedConfig {
		servers: vec![connection_string],
		pool_size: 10,
		timeout_ms: 1000,
	};

	let cache = MemcachedCache::new(config)
		.await
		.expect("Failed to connect to Memcached");

	// Test set and get
	cache
		.set("test_key", &"test_value", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set value");

	let value: Option<String> = cache.get("test_key").await.expect("Failed to get value");
	assert_eq!(value, Some("test_value".to_string()));

	// Test has_key
	let exists = cache
		.has_key("test_key")
		.await
		.expect("Failed to check key");
	assert!(exists);

	// Test non-existent key
	let non_existent: Option<String> = cache
		.get("non_existent_key")
		.await
		.expect("Failed to get non-existent key");
	assert_eq!(non_existent, None);

	let exists = cache
		.has_key("non_existent_key")
		.await
		.expect("Failed to check non-existent key");
	assert!(!exists);
}

#[rstest]
#[tokio::test]
async fn test_memcached_delete_operation(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let cache = MemcachedCache::from_url(&connection_string)
		.await
		.expect("Failed to connect to Memcached");

	// Set a value
	cache
		.set("delete_key", &"delete_value", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set value");

	// Verify it exists
	let exists = cache
		.has_key("delete_key")
		.await
		.expect("Failed to check key");
	assert!(exists);

	// Delete the key
	cache
		.delete("delete_key")
		.await
		.expect("Failed to delete key");

	// Wait a moment for deletion to take effect
	tokio::time::sleep(Duration::from_secs(2)).await;

	// Verify it no longer exists
	let value: Option<String> = cache
		.get("delete_key")
		.await
		.expect("Failed to get after delete");
	assert_eq!(value, None);

	let exists = cache
		.has_key("delete_key")
		.await
		.expect("Failed to check key after delete");
	assert!(!exists);
}

#[rstest]
#[tokio::test]
async fn test_memcached_ttl_expiration(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let cache = MemcachedCache::from_url(&connection_string)
		.await
		.expect("Failed to connect to Memcached");

	// Set a value with short TTL (2 seconds)
	cache
		.set("ttl_key", &"ttl_value", Some(Duration::from_secs(2)))
		.await
		.expect("Failed to set value with TTL");

	// Verify it exists immediately
	let value: Option<String> = cache.get("ttl_key").await.expect("Failed to get value");
	assert_eq!(value, Some("ttl_value".to_string()));

	// Wait for expiration (3 seconds to be safe)
	tokio::time::sleep(Duration::from_secs(3)).await;

	// Verify it has expired
	let value: Option<String> = cache
		.get("ttl_key")
		.await
		.expect("Failed to get after expiration");
	assert_eq!(value, None);
}

#[rstest]
#[tokio::test]
async fn test_memcached_clear(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let cache = MemcachedCache::from_url(&connection_string)
		.await
		.expect("Failed to connect to Memcached");

	// Set multiple values
	cache
		.set("key1", &"value1", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set key1");
	cache
		.set("key2", &"value2", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set key2");
	cache
		.set("key3", &"value3", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set key3");

	// Verify they exist
	assert!(cache.has_key("key1").await.unwrap());
	assert!(cache.has_key("key2").await.unwrap());
	assert!(cache.has_key("key3").await.unwrap());

	// Clear all keys
	cache.clear().await.expect("Failed to clear cache");

	// Verify all keys are gone
	assert!(!cache.has_key("key1").await.unwrap());
	assert!(!cache.has_key("key2").await.unwrap());
	assert!(!cache.has_key("key3").await.unwrap());
}

#[rstest]
#[tokio::test]
async fn test_memcached_complex_types(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let cache = MemcachedCache::from_url(&connection_string)
		.await
		.expect("Failed to connect to Memcached");

	// Test with complex type
	#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
	struct User {
		id: u64,
		name: String,
		email: String,
	}

	let user = User {
		id: 1,
		name: "Alice".to_string(),
		email: "alice@example.com".to_string(),
	};

	// Set complex value
	cache
		.set("user:1", &user, Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set complex value");

	// Get complex value
	let retrieved_user: Option<User> = cache.get("user:1").await.expect("Failed to get value");
	assert_eq!(retrieved_user, Some(user));
}

#[rstest]
#[tokio::test]
async fn test_memcached_error_handling(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let cache = MemcachedCache::from_url(&connection_string)
		.await
		.expect("Failed to connect to Memcached");

	// Test getting a non-existent key (should return None, not error)
	let result: Option<String> = cache
		.get("non_existent")
		.await
		.expect("Failed to get non-existent key");
	assert_eq!(result, None);

	// Test has_key for non-existent key
	let exists = cache
		.has_key("non_existent")
		.await
		.expect("Failed to check non-existent key");
	assert!(!exists);
}

#[rstest]
#[tokio::test]
async fn test_memcached_overwrite(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let cache = MemcachedCache::from_url(&connection_string)
		.await
		.expect("Failed to connect to Memcached");

	// Set initial value
	cache
		.set(
			"overwrite_key",
			&"initial_value",
			Some(Duration::from_secs(60)),
		)
		.await
		.expect("Failed to set initial value");

	let value: Option<String> = cache
		.get("overwrite_key")
		.await
		.expect("Failed to get initial value");
	assert_eq!(value, Some("initial_value".to_string()));

	// Overwrite with new value
	cache
		.set("overwrite_key", &"new_value", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to overwrite value");

	let value: Option<String> = cache
		.get("overwrite_key")
		.await
		.expect("Failed to get overwritten value");
	assert_eq!(value, Some("new_value".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_memcached_no_ttl(
	#[future] memcached_fixture: (testcontainers::ContainerAsync<GenericImage>, String),
) {
	let (_container, connection_string) = memcached_fixture.await;

	let cache = MemcachedCache::from_url(&connection_string)
		.await
		.expect("Failed to connect to Memcached");

	// Set a value without TTL (None means no expiration)
	cache
		.set("no_ttl_key", &"no_ttl_value", None)
		.await
		.expect("Failed to set value without TTL");

	// Verify it exists
	let value: Option<String> = cache.get("no_ttl_key").await.expect("Failed to get value");
	assert_eq!(value, Some("no_ttl_value".to_string()));

	// Wait a bit and verify it still exists
	tokio::time::sleep(Duration::from_secs(2)).await;

	let value: Option<String> = cache
		.get("no_ttl_key")
		.await
		.expect("Failed to get value after wait");
	assert_eq!(value, Some("no_ttl_value".to_string()));
}
