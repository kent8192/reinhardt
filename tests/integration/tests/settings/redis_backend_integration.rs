//! Redis backend integration tests
//!
//! Tests for the `RedisSettingsBackend`, using TestContainers for Redis.

#![cfg(feature = "dynamic-redis")]

use reinhardt_conf::settings::backends::RedisSettingsBackend;
use reinhardt_test::containers::RedisContainer;

async fn create_test_backend() -> (RedisContainer, RedisSettingsBackend) {
	let redis = RedisContainer::new().await;
	let backend = RedisSettingsBackend::new(&redis.connection_url())
		.await
		.expect("Failed to create test backend");
	(redis, backend)
}

#[tokio::test]
async fn test_set_and_get_setting() {
	let (_container, backend) = create_test_backend().await;
	let key = "test_setting_1";
	let value = "test_value";

	// Set setting
	backend
		.set(key, value, Some(3600))
		.await
		.expect("Failed to set setting");

	// Get setting
	let retrieved = backend.get(key).await.expect("Failed to get setting");

	assert_eq!(retrieved, Some(value.to_string()));

	// Cleanup
	backend.delete(key).await.expect("Failed to delete setting");
}

#[tokio::test]
async fn test_setting_exists() {
	let (_container, backend) = create_test_backend().await;
	let key = "test_setting_2";
	let value = "test_value";

	// Setting should not exist initially
	let exists = backend
		.exists(key)
		.await
		.expect("Failed to check existence");
	assert!(!exists);

	// Set setting
	backend
		.set(key, value, Some(3600))
		.await
		.expect("Failed to set setting");

	// Setting should now exist
	let exists = backend
		.exists(key)
		.await
		.expect("Failed to check existence");
	assert!(exists);

	// Cleanup
	backend.delete(key).await.expect("Failed to delete setting");
}

#[tokio::test]
async fn test_delete_setting() {
	let (_container, backend) = create_test_backend().await;
	let key = "test_setting_3";
	let value = "test_value";

	// Set setting
	backend
		.set(key, value, Some(3600))
		.await
		.expect("Failed to set setting");

	// Verify setting exists
	assert!(
		backend
			.exists(key)
			.await
			.expect("Failed to check existence")
	);

	// Delete setting
	backend.delete(key).await.expect("Failed to delete setting");

	// Verify setting no longer exists
	assert!(
		!backend
			.exists(key)
			.await
			.expect("Failed to check existence")
	);
}

#[tokio::test]
async fn test_setting_without_ttl() {
	let (_container, backend) = create_test_backend().await;
	let key = "permanent_setting";
	let value = "permanent_value";

	// Set setting without TTL
	backend
		.set(key, value, None)
		.await
		.expect("Failed to set setting");

	// Get setting
	let retrieved = backend.get(key).await.expect("Failed to get setting");
	assert_eq!(retrieved, Some(value.to_string()));

	// Verify it exists
	assert!(
		backend
			.exists(key)
			.await
			.expect("Failed to check existence")
	);

	// Cleanup
	backend.delete(key).await.expect("Failed to delete setting");
}

#[tokio::test]
async fn test_keys() {
	let (_container, backend) = create_test_backend().await;

	// Set some settings
	backend
		.set("test_key1", "value1", None)
		.await
		.expect("Failed to set setting");
	backend
		.set("test_key2", "value2", None)
		.await
		.expect("Failed to set setting");

	// Get all keys
	let keys = backend.keys().await.expect("Failed to get keys");

	assert!(keys.contains(&"test_key1".to_string()));
	assert!(keys.contains(&"test_key2".to_string()));

	// Cleanup
	backend
		.delete("test_key1")
		.await
		.expect("Failed to delete setting");
	backend
		.delete("test_key2")
		.await
		.expect("Failed to delete setting");
}

#[tokio::test]
async fn test_overwrite_existing_setting() {
	let (_container, backend) = create_test_backend().await;
	let key = "overwrite_test";

	// Set initial value
	backend
		.set(key, "value1", None)
		.await
		.expect("Failed to set setting");

	// Overwrite with new value
	backend
		.set(key, "value2", None)
		.await
		.expect("Failed to set setting");

	// Get updated value
	let retrieved = backend.get(key).await.expect("Failed to get setting");
	assert_eq!(retrieved, Some("value2".to_string()));

	// Cleanup
	backend.delete(key).await.expect("Failed to delete setting");
}
