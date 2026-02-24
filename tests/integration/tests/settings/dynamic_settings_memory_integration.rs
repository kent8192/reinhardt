//! Integration tests for Dynamic Settings with MemoryBackend
//!
//! These tests verify the complete functionality of dynamic settings using
//! the in-memory backend, including CRUD operations, caching, observer pattern,
//! and TTL support.

use reinhardt_conf::settings::backends::MemoryBackend;
use reinhardt_conf::settings::dynamic::{DynamicBackend, DynamicSettings};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[tokio::test]
async fn test_memory_backend_basic_crud() {
	let backend = Arc::new(MemoryBackend::new());
	let settings = DynamicSettings::new(backend);

	// Set a string value
	settings
		.set("name", &"John".to_string(), None)
		.await
		.unwrap();

	// Get the value
	let name: String = settings.get("name").await.unwrap().unwrap();
	assert_eq!(name, "John");

	// Update the value
	settings
		.set("name", &"Jane".to_string(), None)
		.await
		.unwrap();
	let updated_name: String = settings.get("name").await.unwrap().unwrap();
	assert_eq!(updated_name, "Jane");

	// Delete the value
	settings.delete("name").await.unwrap();
	let deleted: Option<String> = settings.get("name").await.unwrap();
	assert!(deleted.is_none());
}

#[tokio::test]
async fn test_memory_backend_multiple_types() {
	let backend = Arc::new(MemoryBackend::new());
	let settings = DynamicSettings::new(backend);

	// Set different types
	settings.set("count", &42i32, None).await.unwrap();
	settings.set("enabled", &true, None).await.unwrap();
	settings.set("ratio", &3.15f64, None).await.unwrap();
	settings
		.set("tags", &vec!["rust", "framework"], None)
		.await
		.unwrap();

	// Retrieve and verify types
	let count: i32 = settings.get("count").await.unwrap().unwrap();
	assert_eq!(count, 42);

	let enabled: bool = settings.get("enabled").await.unwrap().unwrap();
	assert!(enabled);

	let ratio: f64 = settings.get("ratio").await.unwrap().unwrap();
	assert!((ratio - 3.15).abs() < 0.01);

	let tags: Vec<String> = settings.get("tags").await.unwrap().unwrap();
	assert_eq!(tags, vec!["rust".to_string(), "framework".to_string()]);
}

#[tokio::test]
async fn test_memory_backend_ttl() {
	let backend = Arc::new(MemoryBackend::new());
	let settings = DynamicSettings::new(backend);

	// Set with 1 second TTL
	settings
		.set("temp_key", &"temp_value", Some(1))
		.await
		.unwrap();

	// Value should exist immediately
	let value: String = settings.get("temp_key").await.unwrap().unwrap();
	assert_eq!(value, "temp_value");

	// Wait for expiration
	tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

	// Value should be expired
	let expired: Option<String> = settings.get("temp_key").await.unwrap();
	assert!(expired.is_none());
}

#[tokio::test]
async fn test_memory_backend_observer_pattern() {
	let backend = Arc::new(MemoryBackend::new());
	let settings = DynamicSettings::new(backend);

	let change_count = Arc::new(AtomicU32::new(0));
	let change_count_clone = change_count.clone();

	// Subscribe to changes
	let sub_id = settings.subscribe(move |key, _value| {
		if key == "watched_key" {
			change_count_clone.fetch_add(1, Ordering::SeqCst);
		}
	});

	// Make changes - should trigger observer
	settings.set("watched_key", &"value1", None).await.unwrap();
	settings.set("watched_key", &"value2", None).await.unwrap();
	settings.delete("watched_key").await.unwrap();

	// Observer should have been notified 3 times
	assert_eq!(change_count.load(Ordering::SeqCst), 3);

	// Unsubscribe
	settings.unsubscribe(sub_id);

	// Further changes should not trigger observer
	settings.set("watched_key", &"value3", None).await.unwrap();
	assert_eq!(change_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
#[cfg(feature = "caching")]
async fn test_memory_backend_with_cache() {
	let backend = Arc::new(MemoryBackend::new());
	let mut settings = DynamicSettings::new(backend.clone());
	settings.enable_cache(100, None);

	// Set a value
	settings
		.set("cached_key", &"cached_value", None)
		.await
		.unwrap();

	// Get the value (should be cached)
	let value1: String = settings.get("cached_key").await.unwrap().unwrap();
	assert_eq!(value1, "cached_value");

	// Modify directly in backend (bypassing settings)
	backend
		.set("cached_key", &serde_json::json!("modified_value"), None)
		.await
		.unwrap();

	// Should still get cached value
	let value2: String = settings.get("cached_key").await.unwrap().unwrap();
	assert_eq!(value2, "cached_value");

	// Invalidate cache
	settings.invalidate_cache("cached_key").await;

	// Should now get modified value
	let value3: String = settings.get("cached_key").await.unwrap().unwrap();
	assert_eq!(value3, "modified_value");
}

#[tokio::test]
async fn test_memory_backend_keys() {
	let backend = Arc::new(MemoryBackend::new());
	let settings = DynamicSettings::new(backend);

	// Set multiple keys
	settings.set("key1", &"value1", None).await.unwrap();
	settings.set("key2", &"value2", None).await.unwrap();
	settings.set("key3", &"value3", None).await.unwrap();

	// Get all keys
	let keys = settings.keys().await.unwrap();
	assert_eq!(keys.len(), 3);
	assert!(keys.contains(&"key1".to_string()));
	assert!(keys.contains(&"key2".to_string()));
	assert!(keys.contains(&"key3".to_string()));

	// Delete one key
	settings.delete("key2").await.unwrap();

	// Keys should now be 2
	let keys = settings.keys().await.unwrap();
	assert_eq!(keys.len(), 2);
	assert!(!keys.contains(&"key2".to_string()));
}

#[tokio::test]
async fn test_memory_backend_exists() {
	let backend = Arc::new(MemoryBackend::new());
	let settings = DynamicSettings::new(backend);

	// Key should not exist initially
	assert!(!settings.exists("test_key").await.unwrap());

	// Set a value
	settings.set("test_key", &"test_value", None).await.unwrap();

	// Key should now exist
	assert!(settings.exists("test_key").await.unwrap());

	// Delete the key
	settings.delete("test_key").await.unwrap();

	// Key should not exist anymore
	assert!(!settings.exists("test_key").await.unwrap());
}

#[tokio::test]
async fn test_memory_backend_concurrent_access() {
	let backend = Arc::new(MemoryBackend::new());
	let settings = Arc::new(DynamicSettings::new(backend));

	// Spawn multiple tasks writing concurrently
	let mut handles = vec![];
	for i in 0..10 {
		let settings_clone = settings.clone();
		let handle = tokio::spawn(async move {
			let key = format!("key{}", i);
			let value = i;
			settings_clone.set(&key, &value, None).await.unwrap();

			// Read back
			let retrieved: i32 = settings_clone.get(&key).await.unwrap().unwrap();
			assert_eq!(retrieved, value);
		});
		handles.push(handle);
	}

	// Wait for all tasks
	for handle in handles {
		handle.await.unwrap();
	}

	// All keys should exist
	let keys = settings.keys().await.unwrap();
	assert_eq!(keys.len(), 10);
}

#[tokio::test]
#[cfg(feature = "caching")]
async fn test_memory_backend_clear_cache() {
	let backend = Arc::new(MemoryBackend::new());
	let mut settings = DynamicSettings::new(backend.clone());
	settings.enable_cache(100, None);

	// Set multiple values
	settings.set("key1", &"value1", None).await.unwrap();
	settings.set("key2", &"value2", None).await.unwrap();

	// Access to cache them
	let _: String = settings.get("key1").await.unwrap().unwrap();
	let _: String = settings.get("key2").await.unwrap().unwrap();

	// Modify backend directly
	backend
		.set("key1", &serde_json::json!("modified1"), None)
		.await
		.unwrap();
	backend
		.set("key2", &serde_json::json!("modified2"), None)
		.await
		.unwrap();

	// Should still get cached values
	let v1: String = settings.get("key1").await.unwrap().unwrap();
	let v2: String = settings.get("key2").await.unwrap().unwrap();
	assert_eq!(v1, "value1");
	assert_eq!(v2, "value2");

	// Clear all cache
	settings.clear_cache().await;

	// Should now get modified values
	let v1: String = settings.get("key1").await.unwrap().unwrap();
	let v2: String = settings.get("key2").await.unwrap().unwrap();
	assert_eq!(v1, "modified1");
	assert_eq!(v2, "modified2");
}
