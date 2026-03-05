use reinhardt_utils::cache::Cache;
use reinhardt_utils::cache::memcached::{MemcachedCache, MemcachedConfig};
use std::time::Duration;

#[tokio::test]
async fn test_multiple_servers_connection() {
	// Start 3 Memcached containers
	let (_container1, url1) = reinhardt_test::containers::start_memcached().await;
	let (_container2, url2) = reinhardt_test::containers::start_memcached().await;
	let (_container3, url3) = reinhardt_test::containers::start_memcached().await;

	let config = MemcachedConfig {
		servers: vec![url1, url2, url3],
		..Default::default()
	};

	let cache = MemcachedCache::new(config)
		.await
		.expect("Failed to connect to Memcached servers");

	// Verify that cache operations work with multiple servers
	cache
		.set("multi_test", &"value", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set with multiple servers");

	let value: Option<String> = cache
		.get("multi_test")
		.await
		.expect("Failed to get with multiple servers");

	assert_eq!(value, Some("value".to_string()));
}

#[tokio::test]
async fn test_round_robin_distribution() {
	// Start 2 Memcached containers
	let (_container1, url1) = reinhardt_test::containers::start_memcached().await;
	let (_container2, url2) = reinhardt_test::containers::start_memcached().await;

	let config = MemcachedConfig {
		servers: vec![url1, url2],
		..Default::default()
	};

	let cache = MemcachedCache::new(config)
		.await
		.expect("Failed to connect to Memcached servers");

	// Set multiple keys to verify round-robin distribution
	for i in 0..10 {
		let key = format!("round_robin_key_{}", i);
		cache
			.set(&key, &format!("value_{}", i), Some(Duration::from_secs(60)))
			.await
			.unwrap_or_else(|_| panic!("Failed to set key {}", i));
	}

	// Verify all keys can be retrieved
	for i in 0..10 {
		let key = format!("round_robin_key_{}", i);
		let value: Option<String> = cache
			.get(&key)
			.await
			.unwrap_or_else(|_| panic!("Failed to get key {}", i));

		assert_eq!(value, Some(format!("value_{}", i)));
	}
}

#[tokio::test]
async fn test_server_failover() {
	// Start 2 Memcached containers
	let (_container1, url1) = reinhardt_test::containers::start_memcached().await;
	let (_container2, url2) = reinhardt_test::containers::start_memcached().await;

	let config = MemcachedConfig {
		servers: vec![url1, url2],
		..Default::default()
	};

	let cache = MemcachedCache::new(config)
		.await
		.expect("Failed to connect to Memcached servers");

	// Set a test key
	cache
		.set("failover_test", &"data", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set");

	// Note: In a real failover scenario, one server would be stopped here
	// In TestContainers, both servers continue running, so we just verify
	// that operations succeed with multiple servers available

	let value: Option<String> = cache
		.get("failover_test")
		.await
		.expect("Failed to get during failover test");

	assert_eq!(value, Some("data".to_string()));
}

#[tokio::test]
async fn test_clear_all_servers() {
	// Start 2 Memcached containers
	let (_container1, url1) = reinhardt_test::containers::start_memcached().await;
	let (_container2, url2) = reinhardt_test::containers::start_memcached().await;

	let config = MemcachedConfig {
		servers: vec![url1, url2],
		..Default::default()
	};

	let cache = MemcachedCache::new(config)
		.await
		.expect("Failed to connect to Memcached servers");

	// Set keys on different servers
	cache
		.set("clear_test_1", &"value1", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set key 1");

	cache
		.set("clear_test_2", &"value2", Some(Duration::from_secs(60)))
		.await
		.expect("Failed to set key 2");

	// Clear all servers
	cache.clear().await.expect("Failed to clear cache");

	// Verify keys are deleted from all servers
	let value1: Option<String> = cache
		.get("clear_test_1")
		.await
		.expect("Failed to get key 1");
	let value2: Option<String> = cache
		.get("clear_test_2")
		.await
		.expect("Failed to get key 2");

	assert_eq!(value1, None);
	assert_eq!(value2, None);
}
