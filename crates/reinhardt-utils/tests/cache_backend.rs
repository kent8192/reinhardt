//! Integration tests for the in-memory cache backend.
//!
//! These tests cover the public API of `InMemoryCache` including:
//! - Basic get/set/delete operations
//! - TTL/expiry behavior
//! - Cache clear
//! - Key existence checks
//! - Batch operations (get_many, set_many, delete_many)
//! - Increment/decrement
//! - Statistics tracking
//! - Layered cleanup strategy
//! - Cache warming with the `with_default_ttl` builder

use reinhardt_utils::cache::{Cache, CacheStatistics, InMemoryCache};
use rstest::rstest;
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Polls `condition` every `interval` until it returns `true` or `timeout`
/// elapses.
async fn poll_until<F, Fut>(
	timeout: Duration,
	interval: Duration,
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

// ---------------------------------------------------------------------------
// Basic set/get
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_set_and_get_string_value() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	cache.set("greeting", &"hello", None).await.unwrap();
	let result: Option<String> = cache.get("greeting").await.unwrap();

	// Assert
	assert_eq!(result, Some("hello".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_get_returns_none_for_missing_key() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let result: Option<String> = cache.get("nonexistent").await.unwrap();

	// Assert
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn test_set_overwrites_existing_value() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("key", &"original", None).await.unwrap();

	// Act
	cache.set("key", &"updated", None).await.unwrap();
	let result: Option<String> = cache.get("key").await.unwrap();

	// Assert
	assert_eq!(result, Some("updated".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_set_and_get_integer_value() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	cache.set("count", &42_i64, None).await.unwrap();
	let result: Option<i64> = cache.get("count").await.unwrap();

	// Assert
	assert_eq!(result, Some(42));
}

#[rstest]
#[tokio::test]
async fn test_set_and_get_struct_value() {
	// Arrange
	#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
	struct User {
		id: u32,
		name: String,
	}

	let cache = InMemoryCache::new();
	let user = User {
		id: 1,
		name: "Alice".to_string(),
	};

	// Act
	cache.set("user:1", &user, None).await.unwrap();
	let result: Option<User> = cache.get("user:1").await.unwrap();

	// Assert
	assert_eq!(
		result,
		Some(User {
			id: 1,
			name: "Alice".to_string()
		})
	);
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_delete_removes_existing_key() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("key", &"value", None).await.unwrap();

	// Act
	cache.delete("key").await.unwrap();
	let result: Option<String> = cache.get("key").await.unwrap();

	// Assert
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn test_delete_nonexistent_key_is_ok() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let result = cache.delete("ghost").await;

	// Assert
	assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// has_key
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_has_key_returns_true_for_existing_key() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("present", &"yes", None).await.unwrap();

	// Act
	let exists = cache.has_key("present").await.unwrap();

	// Assert
	assert!(exists);
}

#[rstest]
#[tokio::test]
async fn test_has_key_returns_false_for_missing_key() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let exists = cache.has_key("absent").await.unwrap();

	// Assert
	assert!(!exists);
}

#[rstest]
#[tokio::test]
async fn test_has_key_returns_false_after_delete() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("key", &"value", None).await.unwrap();
	cache.delete("key").await.unwrap();

	// Act
	let exists = cache.has_key("key").await.unwrap();

	// Assert
	assert!(!exists);
}

// ---------------------------------------------------------------------------
// TTL / expiry
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_entry_exists_before_ttl_expires() {
	// Arrange
	let cache = InMemoryCache::new();
	cache
		.set("ttl_key", &"alive", Some(Duration::from_millis(300)))
		.await
		.unwrap();

	// Act
	let result: Option<String> = cache.get("ttl_key").await.unwrap();

	// Assert
	assert_eq!(result, Some("alive".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_entry_expires_after_ttl() {
	// Arrange
	let cache = InMemoryCache::new();
	cache
		.set("expiring", &"value", Some(Duration::from_millis(50)))
		.await
		.unwrap();

	// Act – wait for expiry
	poll_until(
		Duration::from_millis(200),
		Duration::from_millis(10),
		|| async {
			let v: Option<String> = cache.get("expiring").await.unwrap();
			v.is_none()
		},
	)
	.await
	.expect("entry should expire within 200 ms");

	let result: Option<String> = cache.get("expiring").await.unwrap();

	// Assert
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn test_has_key_returns_false_for_expired_entry() {
	// Arrange
	let cache = InMemoryCache::new();
	cache
		.set("expiring", &"value", Some(Duration::from_millis(50)))
		.await
		.unwrap();

	// Act
	poll_until(
		Duration::from_millis(200),
		Duration::from_millis(10),
		|| async { !cache.has_key("expiring").await.unwrap() },
	)
	.await
	.expect("has_key should return false after expiry");

	let exists = cache.has_key("expiring").await.unwrap();

	// Assert
	assert!(!exists);
}

#[rstest]
#[tokio::test]
async fn test_default_ttl_applied_when_no_explicit_ttl() {
	// Arrange
	let cache = InMemoryCache::new().with_default_ttl(Duration::from_millis(50));
	cache.set("key", &"value", None).await.unwrap();

	// Act
	poll_until(
		Duration::from_millis(200),
		Duration::from_millis(10),
		|| async {
			let v: Option<String> = cache.get("key").await.unwrap();
			v.is_none()
		},
	)
	.await
	.expect("entry with default TTL should expire");

	let result: Option<String> = cache.get("key").await.unwrap();

	// Assert
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn test_explicit_ttl_overrides_default_ttl() {
	// Arrange – short default TTL but long explicit TTL
	let cache = InMemoryCache::new().with_default_ttl(Duration::from_millis(20));
	cache
		.set("key", &"value", Some(Duration::from_secs(60)))
		.await
		.unwrap();

	// Act – wait long enough for default TTL to have expired
	tokio::time::sleep(Duration::from_millis(40)).await;
	let result: Option<String> = cache.get("key").await.unwrap();

	// Assert
	assert_eq!(result, Some("value".to_string()));
}

// ---------------------------------------------------------------------------
// clear
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_clear_removes_all_entries() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("a", &1_i32, None).await.unwrap();
	cache.set("b", &2_i32, None).await.unwrap();
	cache.set("c", &3_i32, None).await.unwrap();

	// Act
	cache.clear().await.unwrap();

	// Assert
	assert!(!cache.has_key("a").await.unwrap());
	assert!(!cache.has_key("b").await.unwrap());
	assert!(!cache.has_key("c").await.unwrap());
}

#[rstest]
#[tokio::test]
async fn test_clear_on_empty_cache_is_ok() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let result = cache.clear().await;

	// Assert
	assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Batch operations
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_set_many_and_get_many() {
	// Arrange
	let cache = InMemoryCache::new();
	let mut values: HashMap<String, String> = HashMap::new();
	values.insert("k1".to_string(), "v1".to_string());
	values.insert("k2".to_string(), "v2".to_string());
	values.insert("k3".to_string(), "v3".to_string());

	// Act
	cache.set_many(values, None).await.unwrap();
	let results: HashMap<String, String> = cache.get_many(&["k1", "k2", "k3", "k4"]).await.unwrap();

	// Assert
	assert_eq!(results.len(), 3);
	assert_eq!(results.get("k1").map(String::as_str), Some("v1"));
	assert_eq!(results.get("k2").map(String::as_str), Some("v2"));
	assert_eq!(results.get("k3").map(String::as_str), Some("v3"));
	assert!(!results.contains_key("k4"));
}

#[rstest]
#[tokio::test]
async fn test_delete_many_removes_specified_keys() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("x", &"1", None).await.unwrap();
	cache.set("y", &"2", None).await.unwrap();
	cache.set("z", &"3", None).await.unwrap();

	// Act
	cache.delete_many(&["x", "y"]).await.unwrap();

	// Assert
	assert!(!cache.has_key("x").await.unwrap());
	assert!(!cache.has_key("y").await.unwrap());
	assert!(cache.has_key("z").await.unwrap());
}

// ---------------------------------------------------------------------------
// Increment / decrement
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_incr_starts_from_zero_when_key_missing() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let result = cache.incr("counter", 5).await.unwrap();

	// Assert
	assert_eq!(result, 5);
}

#[rstest]
#[tokio::test]
async fn test_incr_accumulates_value() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.incr("counter", 10).await.unwrap();

	// Act
	let result = cache.incr("counter", 3).await.unwrap();

	// Assert
	assert_eq!(result, 13);
}

#[rstest]
#[tokio::test]
async fn test_decr_subtracts_from_existing_value() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("counter", &100_i64, None).await.unwrap();

	// Act
	let result = cache.decr("counter", 25).await.unwrap();

	// Assert
	assert_eq!(result, 75);
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_statistics_initial_state_is_zero() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let stats: CacheStatistics = cache.get_statistics().await;

	// Assert
	assert_eq!(stats.hits, 0);
	assert_eq!(stats.misses, 0);
	assert_eq!(stats.total_requests, 0);
	assert_eq!(stats.entry_count, 0);
	assert_eq!(stats.memory_usage, 0);
}

#[rstest]
#[tokio::test]
async fn test_statistics_hit_is_recorded() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("hit_key", &"value", None).await.unwrap();

	// Act
	let _: Option<String> = cache.get("hit_key").await.unwrap();
	let stats = cache.get_statistics().await;

	// Assert
	assert_eq!(stats.hits, 1);
	assert_eq!(stats.misses, 0);
}

#[rstest]
#[tokio::test]
async fn test_statistics_miss_is_recorded() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let _: Option<String> = cache.get("no_such_key").await.unwrap();
	let stats = cache.get_statistics().await;

	// Assert
	assert_eq!(stats.hits, 0);
	assert_eq!(stats.misses, 1);
}

#[rstest]
#[tokio::test]
async fn test_statistics_expired_entry_counts_as_miss() {
	// Arrange
	let cache = InMemoryCache::new();
	cache
		.set("exp", &"v", Some(Duration::from_millis(20)))
		.await
		.unwrap();
	tokio::time::sleep(Duration::from_millis(30)).await;

	// Act
	let _: Option<String> = cache.get("exp").await.unwrap();
	let stats = cache.get_statistics().await;

	// Assert
	assert_eq!(stats.hits, 0);
	assert_eq!(stats.misses, 1);
}

#[rstest]
#[tokio::test]
async fn test_statistics_hit_rate_calculation() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("k", &"v", None).await.unwrap();
	let _: Option<String> = cache.get("k").await.unwrap(); // hit
	let _: Option<String> = cache.get("k").await.unwrap(); // hit
	let _: Option<String> = cache.get("missing").await.unwrap(); // miss

	// Act
	let stats = cache.get_statistics().await;

	// Assert – 2 hits / 3 total = 0.666…
	assert!((stats.hit_rate() - 2.0 / 3.0).abs() < 1e-9);
	assert!((stats.miss_rate() - 1.0 / 3.0).abs() < 1e-9);
}

#[rstest]
#[tokio::test]
async fn test_statistics_entry_count_reflects_stored_entries() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("a", &1_i32, None).await.unwrap();
	cache.set("b", &2_i32, None).await.unwrap();

	// Act
	let stats = cache.get_statistics().await;

	// Assert
	assert_eq!(stats.entry_count, 2);
	assert!(stats.memory_usage > 0);
}

// ---------------------------------------------------------------------------
// cleanup_expired / list_keys / inspect_entry
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_cleanup_expired_removes_only_expired_entries() {
	// Arrange
	let cache = InMemoryCache::new();
	cache
		.set("short", &"v", Some(Duration::from_millis(30)))
		.await
		.unwrap();
	cache.set("long", &"v", None).await.unwrap();

	// Wait for short-lived entry to expire
	poll_until(
		Duration::from_millis(200),
		Duration::from_millis(10),
		|| async {
			let v: Option<String> = cache.get("short").await.unwrap();
			v.is_none()
		},
	)
	.await
	.expect("short-lived entry should expire");

	// Act
	cache.cleanup_expired().await;

	// Assert
	assert!(!cache.has_key("short").await.unwrap());
	assert!(cache.has_key("long").await.unwrap());
}

#[rstest]
#[tokio::test]
async fn test_list_keys_returns_all_stored_keys() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("key1", &"v", None).await.unwrap();
	cache.set("key2", &"v", None).await.unwrap();
	cache.set("key3", &"v", None).await.unwrap();

	// Act
	let keys = cache.list_keys().await;

	// Assert
	assert_eq!(keys.len(), 3);
	assert!(keys.contains(&"key1".to_string()));
	assert!(keys.contains(&"key2".to_string()));
	assert!(keys.contains(&"key3".to_string()));
}

#[rstest]
#[tokio::test]
async fn test_inspect_entry_returns_none_for_nonexistent_key() {
	// Arrange
	let cache = InMemoryCache::new();

	// Act
	let info = cache.inspect_entry("ghost").await;

	// Assert
	assert!(info.is_none());
}

#[rstest]
#[tokio::test]
async fn test_inspect_entry_reports_no_expiry_for_persistent_entry() {
	// Arrange
	let cache = InMemoryCache::new();
	cache.set("perm", &"value", None).await.unwrap();

	// Act
	let info = cache.inspect_entry("perm").await.unwrap();

	// Assert
	assert_eq!(info.key, "perm");
	assert!(!info.has_expiry);
	assert!(info.ttl_seconds.is_none());
	assert!(info.size > 0);
}

#[rstest]
#[tokio::test]
async fn test_inspect_entry_reports_correct_ttl() {
	// Arrange
	let cache = InMemoryCache::new();
	cache
		.set("ttl_entry", &"value", Some(Duration::from_secs(120)))
		.await
		.unwrap();

	// Act
	let info = cache.inspect_entry("ttl_entry").await.unwrap();

	// Assert
	assert!(info.has_expiry);
	let ttl = info.ttl_seconds.unwrap();
	assert!(ttl <= 120);
	assert!(ttl > 0);
}

// ---------------------------------------------------------------------------
// Layered cleanup strategy – mirror basic tests
// ---------------------------------------------------------------------------

#[rstest]
#[tokio::test]
async fn test_layered_basic_set_get_delete() {
	// Arrange
	let cache = InMemoryCache::with_layered_cleanup();

	// Act – set
	cache.set("lkey", &"lvalue", None).await.unwrap();
	let result: Option<String> = cache.get("lkey").await.unwrap();

	// Assert set/get
	assert_eq!(result, Some("lvalue".to_string()));

	// Act – delete
	cache.delete("lkey").await.unwrap();
	let after_delete: Option<String> = cache.get("lkey").await.unwrap();

	// Assert delete
	assert!(after_delete.is_none());
}

#[rstest]
#[tokio::test]
async fn test_layered_entry_expires_after_ttl() {
	// Arrange
	let cache = InMemoryCache::with_layered_cleanup();
	cache
		.set("lexpire", &"value", Some(Duration::from_millis(50)))
		.await
		.unwrap();

	// Act
	poll_until(
		Duration::from_millis(200),
		Duration::from_millis(10),
		|| async {
			let v: Option<String> = cache.get("lexpire").await.unwrap();
			v.is_none()
		},
	)
	.await
	.expect("layered entry should expire");

	let result: Option<String> = cache.get("lexpire").await.unwrap();

	// Assert
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn test_layered_clear_removes_all_entries() {
	// Arrange
	let cache = InMemoryCache::with_layered_cleanup();
	cache.set("la", &1_i32, None).await.unwrap();
	cache.set("lb", &2_i32, None).await.unwrap();

	// Act
	cache.clear().await.unwrap();

	// Assert
	assert!(!cache.has_key("la").await.unwrap());
	assert!(!cache.has_key("lb").await.unwrap());
}

#[rstest]
#[tokio::test]
async fn test_layered_statistics_tracks_hits_and_misses() {
	// Arrange
	let cache = InMemoryCache::with_layered_cleanup();
	cache.set("lk", &"lv", None).await.unwrap();

	// Act
	let _: Option<String> = cache.get("lk").await.unwrap(); // hit
	let _: Option<String> = cache.get("lmiss").await.unwrap(); // miss
	let stats = cache.get_statistics().await;

	// Assert
	assert_eq!(stats.hits, 1);
	assert_eq!(stats.misses, 1);
}

#[rstest]
#[tokio::test]
async fn test_layered_cleanup_expired_removes_stale_entries() {
	// Arrange
	let cache = InMemoryCache::with_layered_cleanup();
	cache
		.set("stale", &"v", Some(Duration::from_millis(30)))
		.await
		.unwrap();
	cache.set("fresh", &"v", None).await.unwrap();

	tokio::time::sleep(Duration::from_millis(60)).await;

	// Act
	cache.cleanup_expired().await;

	// Assert
	assert!(!cache.has_key("stale").await.unwrap());
	assert!(cache.has_key("fresh").await.unwrap());
}

#[rstest]
#[tokio::test]
async fn test_layered_list_keys_reflects_stored_entries() {
	// Arrange
	let cache = InMemoryCache::with_layered_cleanup();
	cache.set("p", &"v", None).await.unwrap();
	cache.set("q", &"v", None).await.unwrap();

	// Act
	let keys = cache.list_keys().await;

	// Assert
	assert_eq!(keys.len(), 2);
	assert!(keys.contains(&"p".to_string()));
	assert!(keys.contains(&"q".to_string()));
}
