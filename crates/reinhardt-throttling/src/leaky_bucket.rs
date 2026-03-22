//! Leaky bucket algorithm implementation for rate limiting
//!
//! The leaky bucket algorithm processes requests at a constant rate,
//! smoothing out bursts. Requests that exceed the bucket's capacity are rejected.

use super::time_provider::{SystemTimeProvider, TimeProvider};
use super::{Throttle, ThrottleError, ThrottleResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;

/// Default maximum number of entries in the per-key state HashMap
const DEFAULT_MAX_ENTRIES: usize = 10_000;

/// Configuration for leaky bucket algorithm
#[derive(Debug, Clone)]
pub struct LeakyBucketConfig {
	/// Maximum number of requests in the bucket (queue capacity)
	pub capacity: usize,
	/// Rate at which requests leak from the bucket (requests per second)
	pub leak_rate: f64,
}

impl LeakyBucketConfig {
	/// Creates a new leaky bucket configuration
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidConfig`] if `capacity` is zero or
	/// `leak_rate` is not a positive finite number.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::leaky_bucket::LeakyBucketConfig;
	///
	/// // 10 requests per second with queue capacity of 20
	/// let config = LeakyBucketConfig::new(20, 10.0).unwrap();
	/// assert_eq!(config.capacity, 20);
	/// assert_eq!(config.leak_rate, 10.0);
	/// ```
	pub fn new(capacity: usize, leak_rate: f64) -> ThrottleResult<Self> {
		if capacity == 0 {
			return Err(ThrottleError::InvalidConfig(
				"capacity must be non-zero".to_string(),
			));
		}
		if !leak_rate.is_finite() || leak_rate <= 0.0 {
			return Err(ThrottleError::InvalidConfig(
				"leak_rate must be a positive finite number".to_string(),
			));
		}
		Ok(Self {
			capacity,
			leak_rate,
		})
	}

	/// Create configuration for requests per second
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidConfig`] if `capacity` is zero or
	/// `rate` is not a positive finite number.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::leaky_bucket::LeakyBucketConfig;
	///
	/// // 5 requests per second with queue of 10
	/// let config = LeakyBucketConfig::per_second(5.0, 10).unwrap();
	/// assert_eq!(config.leak_rate, 5.0);
	/// assert_eq!(config.capacity, 10);
	/// ```
	pub fn per_second(rate: f64, capacity: usize) -> ThrottleResult<Self> {
		Self::new(capacity, rate)
	}

	/// Create configuration for requests per minute
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidConfig`] if `capacity` is zero or
	/// `rate` is not a positive finite number.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::leaky_bucket::LeakyBucketConfig;
	///
	/// // 60 requests per minute with queue of 100
	/// let config = LeakyBucketConfig::per_minute(60.0, 100).unwrap();
	/// assert_eq!(config.leak_rate, 1.0);
	/// assert_eq!(config.capacity, 100);
	/// ```
	pub fn per_minute(rate: f64, capacity: usize) -> ThrottleResult<Self> {
		if !rate.is_finite() || rate <= 0.0 {
			return Err(ThrottleError::InvalidConfig(
				"rate must be a positive finite number".to_string(),
			));
		}
		Self::new(capacity, rate / 60.0)
	}
}

/// Bucket state for tracking requests
#[derive(Debug, Clone)]
struct BucketState {
	/// Current number of requests in the bucket
	level: f64,
	/// Last time the bucket was leaked
	last_leak: Instant,
	/// Last time this entry was accessed (for eviction ordering)
	last_accessed: Instant,
}

/// Leaky bucket throttle implementation
///
/// # Examples
///
/// ```
/// use reinhardt_throttling::leaky_bucket::{LeakyBucketThrottle, LeakyBucketConfig};
/// use reinhardt_throttling::Throttle;
///
/// # tokio_test::block_on(async {
/// let config = LeakyBucketConfig::per_second(10.0, 20).unwrap();
/// let throttle = LeakyBucketThrottle::new(config);
///
/// // Requests are processed at constant rate
/// assert!(throttle.allow_request("user_123").await.unwrap());
/// # });
/// ```
pub struct LeakyBucketThrottle<T: TimeProvider = SystemTimeProvider> {
	config: LeakyBucketConfig,
	time_provider: Arc<T>,
	states: Arc<RwLock<HashMap<String, BucketState>>>,
	max_entries: usize,
}

impl LeakyBucketThrottle<SystemTimeProvider> {
	/// Creates a new leaky bucket with default time provider
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::leaky_bucket::{LeakyBucketThrottle, LeakyBucketConfig};
	///
	/// let config = LeakyBucketConfig::per_second(5.0, 10).unwrap();
	/// let throttle = LeakyBucketThrottle::new(config);
	/// ```
	pub fn new(config: LeakyBucketConfig) -> Self {
		Self {
			config,
			time_provider: Arc::new(SystemTimeProvider::new()),
			states: Arc::new(RwLock::new(HashMap::new())),
			max_entries: DEFAULT_MAX_ENTRIES,
		}
	}
}

impl<T: TimeProvider> LeakyBucketThrottle<T> {
	/// Creates a new leaky bucket with custom time provider
	pub fn with_time_provider(config: LeakyBucketConfig, time_provider: Arc<T>) -> Self {
		Self {
			config,
			time_provider,
			states: Arc::new(RwLock::new(HashMap::new())),
			max_entries: DEFAULT_MAX_ENTRIES,
		}
	}

	/// Sets the maximum number of per-key entries before eviction occurs.
	///
	/// When the number of tracked keys exceeds this limit, the least recently
	/// accessed entries are evicted to make room. Defaults to 10,000.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::leaky_bucket::{LeakyBucketThrottle, LeakyBucketConfig};
	///
	/// let config = LeakyBucketConfig::per_second(5.0, 10).unwrap();
	/// let throttle = LeakyBucketThrottle::new(config).with_max_entries(500);
	/// ```
	pub fn with_max_entries(mut self, max_entries: usize) -> Self {
		self.max_entries = max_entries;
		self
	}

	/// Returns the maximum number of per-key entries before eviction occurs
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::leaky_bucket::{LeakyBucketThrottle, LeakyBucketConfig};
	///
	/// let config = LeakyBucketConfig::per_second(5.0, 10).unwrap();
	/// let throttle = LeakyBucketThrottle::new(config);
	/// assert_eq!(throttle.max_entries(), 10_000);
	/// ```
	pub fn max_entries(&self) -> usize {
		self.max_entries
	}

	/// Create a new bucket state initialized as empty
	fn new_bucket_state(&self) -> BucketState {
		let now = self.time_provider.now();
		BucketState {
			level: 0.0,
			last_leak: now,
			last_accessed: now,
		}
	}

	/// Leak requests from the bucket based on elapsed time
	fn leak_bucket(&self, state: &mut BucketState) {
		let now = self.time_provider.now();
		let elapsed = now.duration_since(state.last_leak);
		let elapsed_secs = elapsed.as_secs_f64();

		// Calculate how many requests have leaked
		let leaked = elapsed_secs * self.config.leak_rate;

		// Update bucket level (cannot go below 0)
		state.level = (state.level - leaked).max(0.0);
		state.last_leak = now;
		state.last_accessed = now;
	}

	/// Evict stale entries when the map is at or exceeds its maximum size.
	///
	/// Called before inserting a new key to ensure there is room. First removes
	/// entries whose bucket has effectively drained (accounting for elapsed time
	/// since last leak). If still at capacity, removes the least recently
	/// accessed entries.
	fn evict_if_needed(&self, states: &mut HashMap<String, BucketState>) {
		if states.len() < self.max_entries {
			return;
		}

		let now = self.time_provider.now();

		// Phase 1: remove entries whose effective level is zero
		states.retain(|_, state| {
			let elapsed_secs = now.duration_since(state.last_leak).as_secs_f64();
			let leaked = elapsed_secs * self.config.leak_rate;
			let effective_level = (state.level - leaked).max(0.0);
			effective_level > f64::EPSILON
		});

		if states.len() < self.max_entries {
			return;
		}

		// Phase 2: evict least recently accessed entries to make room
		let mut entries: Vec<(String, Instant)> = states
			.iter()
			.map(|(k, v)| (k.clone(), v.last_accessed))
			.collect();
		entries.sort_by_key(|(_, accessed)| *accessed);

		let to_remove = states.len() - self.max_entries + 1;
		for (key, _) in entries.into_iter().take(to_remove) {
			states.remove(&key);
		}
	}

	/// Get current bucket level for a given key
	pub async fn level_for_key(&self, key: &str) -> f64 {
		let mut states = self.states.write().await;
		if !states.contains_key(key) {
			self.evict_if_needed(&mut states);
		}
		let state = states
			.entry(key.to_string())
			.or_insert_with(|| self.new_bucket_state());
		self.leak_bucket(state);
		state.level
	}

	/// Get current bucket level (uses default empty key for backward compatibility)
	pub async fn level(&self) -> f64 {
		self.level_for_key("").await
	}

	/// Reset the bucket for a specific key
	pub async fn reset_key(&self, key: &str) {
		let mut states = self.states.write().await;
		states.remove(key);
	}

	/// Reset all buckets
	pub async fn reset(&self) {
		let mut states = self.states.write().await;
		states.clear();
	}

	/// Returns the number of tracked keys in the state map
	pub async fn entry_count(&self) -> usize {
		self.states.read().await.len()
	}

	/// Returns whether a specific key exists in the state map
	pub async fn contains_key(&self, key: &str) -> bool {
		self.states.read().await.contains_key(key)
	}
}

#[async_trait]
impl<T: TimeProvider> Throttle for LeakyBucketThrottle<T> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		let mut states = self.states.write().await;
		if !states.contains_key(key) {
			self.evict_if_needed(&mut states);
		}
		let state = states
			.entry(key.to_string())
			.or_insert_with(|| self.new_bucket_state());

		// Leak requests first
		self.leak_bucket(state);

		// Check if there's room in the bucket after adding one request
		if state.level + 1.0 <= self.config.capacity as f64 {
			state.level += 1.0;
			Ok(true)
		} else {
			Ok(false)
		}
	}

	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		let mut states = self.states.write().await;
		if !states.contains_key(key) {
			self.evict_if_needed(&mut states);
		}
		let state = states
			.entry(key.to_string())
			.or_insert_with(|| self.new_bucket_state());

		self.leak_bucket(state);

		if state.level + 1.0 <= self.config.capacity as f64 {
			return Ok(None);
		}

		// Calculate time until space is available
		// Need to wait until level drops below capacity
		let excess = state.level - (self.config.capacity as f64 - 1.0);
		let wait_secs = (excess / self.config.leak_rate).ceil();

		Ok(Some(wait_secs as u64))
	}

	fn get_rate(&self) -> (usize, u64) {
		let rate = self.config.leak_rate;
		if rate >= 1.0 {
			(rate as usize, 1)
		} else {
			// Scale sub-1.0 rates to a whole number representation.
			// e.g., 0.5 req/sec becomes (1, 2) meaning 1 request per 2 seconds.
			let period = (1.0 / rate).ceil() as u64;
			let requests = (rate * period as f64).round() as usize;
			// Ensure at least 1 request in the computed period
			(requests.max(1), period)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::time_provider::MockTimeProvider;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_basic() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(5, 1.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider);

		// Act & Assert - should allow up to capacity
		for _ in 0..5 {
			assert!(throttle.allow_request("user").await.unwrap());
		}

		// Assert - 6th request should fail
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_leak() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(10, 2.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider.clone());

		// Act - fill the bucket
		for _ in 0..10 {
			assert!(throttle.allow_request("user").await.unwrap());
		}
		assert!(!throttle.allow_request("user").await.unwrap());

		// Act - advance time by 1 second (2 requests should leak)
		time_provider.advance(std::time::Duration::from_secs(1));

		// Assert - should allow 2 more requests
		assert!(throttle.allow_request("user").await.unwrap());
		assert!(throttle.allow_request("user").await.unwrap());
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_smoothing() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::per_second(5.0, 10).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider.clone());

		// Act - burst of requests
		for _ in 0..10 {
			assert!(throttle.allow_request("user").await.unwrap());
		}

		// Assert - bucket full
		assert!(!throttle.allow_request("user").await.unwrap());

		// Act - advance time by 2 seconds (10 requests leak)
		time_provider.advance(std::time::Duration::from_secs(2));

		// Assert - bucket should be empty, allow full capacity again
		for _ in 0..10 {
			assert!(throttle.allow_request("user").await.unwrap());
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_level() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(10, 2.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider.clone());

		// Assert - initial level should be 0
		assert_eq!(throttle.level_for_key("user").await, 0.0);

		// Act - add 5 requests
		for _ in 0..5 {
			throttle.allow_request("user").await.unwrap();
		}

		// Assert
		assert_eq!(throttle.level_for_key("user").await, 5.0);

		// Act - advance time by 1 second (2 requests leak)
		time_provider.advance(std::time::Duration::from_secs(1));

		// Assert
		assert_eq!(throttle.level_for_key("user").await, 3.0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_reset() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(10, 1.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider);

		// Act - fill bucket
		for _ in 0..10 {
			throttle.allow_request("user").await.unwrap();
		}
		assert!(throttle.level_for_key("user").await > 0.0);

		// Act - reset all buckets
		throttle.reset().await;

		// Assert - after reset, a new bucket is created with level 0
		assert_eq!(throttle.level_for_key("user").await, 0.0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_wait_time() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(5, 1.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider);

		// Act - fill bucket
		for _ in 0..5 {
			throttle.allow_request("user").await.unwrap();
		}

		// Assert - should have wait time
		let wait = throttle.wait_time("user").await.unwrap();
		assert!(wait.is_some());
		assert!(wait.unwrap() > 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_per_key_isolation() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(5, 1.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider);

		// Act - fill the bucket for "alice"
		for _ in 0..5 {
			assert!(throttle.allow_request("alice").await.unwrap());
		}

		// Assert - "alice" is full, "bob" is independent and still allowed
		assert!(!throttle.allow_request("alice").await.unwrap());
		assert!(throttle.allow_request("bob").await.unwrap());

		// Assert - levels are independent
		assert_eq!(throttle.level_for_key("alice").await, 5.0);
		assert_eq!(throttle.level_for_key("bob").await, 1.0);

		// Assert - wait_time is independent
		let alice_wait = throttle.wait_time("alice").await.unwrap();
		let bob_wait = throttle.wait_time("bob").await.unwrap();
		assert!(alice_wait.is_some());
		assert!(bob_wait.is_none());
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_reset_key() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(5, 1.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider);

		// Act - fill both keys
		for _ in 0..5 {
			throttle.allow_request("alice").await.unwrap();
			throttle.allow_request("bob").await.unwrap();
		}

		// Assert - both full
		assert_eq!(throttle.level_for_key("alice").await, 5.0);
		assert_eq!(throttle.level_for_key("bob").await, 5.0);

		// Act - reset only "alice"
		throttle.reset_key("alice").await;

		// Assert - "alice" is reset, "bob" is unchanged
		assert_eq!(throttle.level_for_key("alice").await, 0.0);
		assert_eq!(throttle.level_for_key("bob").await, 5.0);

		// Assert - "alice" can accept requests again, "bob" cannot
		assert!(throttle.allow_request("alice").await.unwrap());
		assert!(!throttle.allow_request("bob").await.unwrap());
	}

	#[rstest]
	fn test_leaky_bucket_config_per_second() {
		// Arrange & Act
		let config = LeakyBucketConfig::per_second(10.0, 20).unwrap();

		// Assert
		assert_eq!(config.leak_rate, 10.0);
		assert_eq!(config.capacity, 20);
	}

	#[rstest]
	fn test_leaky_bucket_config_per_minute() {
		// Arrange & Act
		let config = LeakyBucketConfig::per_minute(60.0, 100).unwrap();

		// Assert
		assert_eq!(config.leak_rate, 1.0);
		assert_eq!(config.capacity, 100);
	}

	#[rstest]
	fn test_new_rejects_zero_leak_rate() {
		// Arrange & Act
		let result = LeakyBucketConfig::new(10, 0.0);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_new_rejects_negative_leak_rate() {
		// Arrange & Act
		let result = LeakyBucketConfig::new(10, -1.0);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_new_rejects_infinite_leak_rate() {
		// Arrange & Act
		let result = LeakyBucketConfig::new(10, f64::INFINITY);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_new_rejects_nan_leak_rate() {
		// Arrange & Act
		let result = LeakyBucketConfig::new(10, f64::NAN);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_new_rejects_zero_capacity() {
		// Arrange & Act
		let result = LeakyBucketConfig::new(0, 1.0);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_per_second_rejects_zero_rate() {
		// Arrange & Act
		let result = LeakyBucketConfig::per_second(0.0, 10);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_per_minute_rejects_zero_rate() {
		// Arrange & Act
		let result = LeakyBucketConfig::per_minute(0.0, 10);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_eviction_at_capacity() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(5, 1.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider.clone())
			.with_max_entries(3);

		// Act - fill 3 keys to capacity
		for i in 0..3 {
			let key = format!("user_{i}");
			throttle.allow_request(&key).await.unwrap();
		}

		// Arrange - advance time so all buckets drain completely
		time_provider.advance(std::time::Duration::from_secs(10));

		// Act - add a 4th key, which should trigger eviction of drained entries
		assert!(throttle.allow_request("user_new").await.unwrap());

		// Assert - map should have at most max_entries keys
		assert!(throttle.entry_count().await <= 3);
	}

	#[rstest]
	#[tokio::test]
	async fn test_leaky_bucket_eviction_lru_when_active() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = LeakyBucketConfig::new(5, 1.0).unwrap();
		let throttle = LeakyBucketThrottle::with_time_provider(config, time_provider.clone())
			.with_max_entries(3);

		// Act - fill 3 keys with active (non-drained) buckets
		for i in 0..3 {
			let key = format!("user_{i}");
			for _ in 0..5 {
				throttle.allow_request(&key).await.unwrap();
			}
			// Stagger access times so LRU ordering is deterministic
			time_provider.advance(std::time::Duration::from_millis(100));
		}

		// Act - add a 4th key, triggering LRU eviction
		assert!(throttle.allow_request("user_new").await.unwrap());

		// Assert - oldest key (user_0) should have been evicted
		assert!(throttle.entry_count().await <= 3);
		assert!(!throttle.contains_key("user_0").await);
		assert!(throttle.contains_key("user_new").await);
	}

	#[rstest]
	fn test_leaky_bucket_throttle_with_max_entries() {
		// Arrange & Act
		let config = LeakyBucketConfig::new(10, 2.0).unwrap();
		let throttle = LeakyBucketThrottle::new(config).with_max_entries(5000);

		// Assert
		assert_eq!(throttle.max_entries(), 5000);
	}

	#[rstest]
	fn test_leaky_bucket_throttle_default_max_entries() {
		// Arrange & Act
		let config = LeakyBucketConfig::new(10, 2.0).unwrap();
		let throttle = LeakyBucketThrottle::new(config);

		// Assert
		assert_eq!(throttle.max_entries(), DEFAULT_MAX_ENTRIES);
	}

	#[rstest]
	#[case(0.5, 1, 2)]
	#[case(0.1, 1, 10)]
	#[case(0.25, 1, 4)]
	#[case(2.0, 2, 1)]
	#[case(10.0, 10, 1)]
	fn test_get_rate_handles_sub_one_leak_rate(
		#[case] leak_rate: f64,
		#[case] expected_requests: usize,
		#[case] expected_period: u64,
	) {
		// Arrange
		let config = LeakyBucketConfig::new(10, leak_rate).unwrap();
		let throttle = LeakyBucketThrottle::new(config);

		// Act
		let (requests, period) = throttle.get_rate();

		// Assert
		assert_eq!(requests, expected_requests);
		assert_eq!(period, expected_period);
	}

	#[rstest]
	fn test_get_rate_sub_one_never_returns_zero_requests() {
		// Arrange
		let config = LeakyBucketConfig::new(10, 0.01).unwrap();
		let throttle = LeakyBucketThrottle::new(config);

		// Act
		let (requests, period) = throttle.get_rate();

		// Assert - requests must never be zero
		assert!(requests >= 1);
		assert!(period >= 1);
	}
}
