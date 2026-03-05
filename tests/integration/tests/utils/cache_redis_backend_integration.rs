use reinhardt_test::containers::RedisContainer;
use reinhardt_utils::cache::Cache;
use reinhardt_utils::cache::redis_backend::RedisCache;
use std::time::Duration;

#[tokio::test]
async fn test_redis_cache_basic_operations() {
	let redis = RedisContainer::new().await;
	let cache = RedisCache::new(redis.connection_url())
		.await
		.unwrap()
		.with_key_prefix("test");

	// Test set and get
	cache.set("key1", &"value1", None).await.unwrap();
	let value: Option<String> = cache.get("key1").await.unwrap();
	assert_eq!(value, Some("value1".to_string()));

	// Test delete
	cache.delete("key1").await.unwrap();
	let value: Option<String> = cache.get("key1").await.unwrap();
	assert_eq!(value, None);

	// Test has_key
	cache.set("key2", &"value2", None).await.unwrap();
	assert!(cache.has_key("key2").await.unwrap());
	cache.delete("key2").await.unwrap();
	assert!(!cache.has_key("key2").await.unwrap());
}

/// Polls a condition until it returns true or timeout is reached.
async fn poll_until<F, Fut>(
	timeout: std::time::Duration,
	interval: std::time::Duration,
	mut condition: F,
) -> Result<(), String>
where
	F: FnMut() -> Fut,
	Fut: std::future::Future<Output = bool>,
{
	let start = std::time::Instant::now();
	while start.elapsed() < timeout {
		if condition().await {
			return Ok(());
		}
		tokio::time::sleep(interval).await;
	}
	Err(format!("Timeout after {:?} waiting for condition", timeout))
}

#[tokio::test]
async fn test_redis_cache_ttl() {
	let redis = RedisContainer::new().await;
	let cache = RedisCache::new(redis.connection_url())
		.await
		.unwrap()
		.with_key_prefix("test");

	// Set with TTL
	cache
		.set("ttl_key", &"value", Some(Duration::from_secs(2)))
		.await
		.unwrap();

	// Key should exist immediately
	let value: Option<String> = cache.get("ttl_key").await.unwrap();
	assert_eq!(value, Some("value".to_string()));

	// Poll until key expires (2 second TTL)
	poll_until(
		Duration::from_millis(2500),
		Duration::from_millis(100),
		|| async {
			let value: Option<String> = cache.get("ttl_key").await.unwrap();
			value.is_none()
		},
	)
	.await
	.expect("Key should expire within 2500ms");

	// Key should be expired
	let value: Option<String> = cache.get("ttl_key").await.unwrap();
	assert_eq!(value, None);
}

#[tokio::test]
async fn test_redis_cache_batch_operations() {
	let redis = RedisContainer::new().await;
	let cache = RedisCache::new(redis.connection_url())
		.await
		.unwrap()
		.with_key_prefix("test");

	// Set multiple values
	let mut values = std::collections::HashMap::new();
	values.insert("batch_key1".to_string(), "value1".to_string());
	values.insert("batch_key2".to_string(), "value2".to_string());
	values.insert("batch_key3".to_string(), "value3".to_string());

	cache.set_many(values, None).await.unwrap();

	// Get multiple values
	let keys = vec!["batch_key1", "batch_key2", "batch_key3"];
	let results: std::collections::HashMap<String, String> = cache.get_many(&keys).await.unwrap();

	assert_eq!(results.get("batch_key1"), Some(&"value1".to_string()));
	assert_eq!(results.get("batch_key2"), Some(&"value2".to_string()));
	assert_eq!(results.get("batch_key3"), Some(&"value3".to_string()));

	// Delete multiple values
	cache.delete_many(&keys).await.unwrap();

	let results: std::collections::HashMap<String, String> = cache.get_many(&keys).await.unwrap();
	assert!(results.get("batch_key1").is_none());
	assert!(results.get("batch_key2").is_none());
	assert!(results.get("batch_key3").is_none());
}

#[tokio::test]
async fn test_redis_cache_atomic_operations() {
	let redis = RedisContainer::new().await;
	let cache = RedisCache::new(redis.connection_url())
		.await
		.unwrap()
		.with_key_prefix("test");

	// Test incr
	cache.set("counter", &0i64, None).await.unwrap();
	let result: i64 = cache.incr("counter", 1).await.unwrap();
	assert_eq!(result, 1);

	let result: i64 = cache.incr("counter", 5).await.unwrap();
	assert_eq!(result, 6);

	// Test decr
	let result: i64 = cache.decr("counter", 2).await.unwrap();
	assert_eq!(result, 4);
}

#[tokio::test]
async fn test_redis_cache_prefix() {
	let redis = RedisContainer::new().await;
	let cache = RedisCache::new(redis.connection_url())
		.await
		.unwrap()
		.with_key_prefix("myapp");

	cache.set("user:1", &"Alice", None).await.unwrap();

	// Key should be stored with prefix
	let value: Option<String> = cache.get("user:1").await.unwrap();
	assert_eq!(value, Some("Alice".to_string()));

	// Try to get with full prefixed key should not work
	// (because the cache will add prefix again)
	let value: Option<String> = cache.get("myapp:user:1").await.unwrap();
	assert_eq!(value, None);

	cache.delete("user:1").await.unwrap();
	let value: Option<String> = cache.get("user:1").await.unwrap();
	assert_eq!(value, None);
}
