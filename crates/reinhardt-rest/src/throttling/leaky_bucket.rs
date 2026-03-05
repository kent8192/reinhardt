//! Leaky bucket algorithm implementation for rate limiting
//!
//! The leaky bucket algorithm processes requests at a constant rate,
//! smoothing out bursts. Requests that exceed the bucket's capacity are rejected.

use super::backend::ThrottleBackend;
use super::{Throttle, ThrottleResult};
use super::time_provider::{SystemTimeProvider, TimeProvider};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;

/// Configuration for leaky bucket algorithm
#[non_exhaustive]
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
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::leaky_bucket::LeakyBucketConfig;
	///
	/// // 10 requests per second with queue capacity of 20
	/// let config = LeakyBucketConfig::new(20, 10.0);
	/// assert_eq!(config.capacity, 20);
	/// assert_eq!(config.leak_rate, 10.0);
	/// ```
	pub fn new(capacity: usize, leak_rate: f64) -> Self {
		Self {
			capacity,
			leak_rate,
		}
	}

	/// Create configuration for requests per second
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::leaky_bucket::LeakyBucketConfig;
	///
	/// // 5 requests per second with queue of 10
	/// let config = LeakyBucketConfig::per_second(5.0, 10);
	/// assert_eq!(config.leak_rate, 5.0);
	/// assert_eq!(config.capacity, 10);
	/// ```
	pub fn per_second(rate: f64, capacity: usize) -> Self {
		Self {
			capacity,
			leak_rate: rate,
		}
	}

	/// Create configuration for requests per minute
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::leaky_bucket::LeakyBucketConfig;
	///
	/// // 60 requests per minute with queue of 100
	/// let config = LeakyBucketConfig::per_minute(60.0, 100);
	/// assert_eq!(config.leak_rate, 1.0);
	/// assert_eq!(config.capacity, 100);
	/// ```
	pub fn per_minute(rate: f64, capacity: usize) -> Self {
		Self {
			capacity,
			leak_rate: rate / 60.0,
		}
	}
}

/// Bucket state for tracking requests
#[derive(Debug, Clone)]
struct BucketState {
	/// Current number of requests in the bucket
	level: f64,
	/// Last time the bucket was leaked
	last_leak: Instant,
}

/// Leaky bucket throttle implementation
///
/// # Examples
///
/// ```
/// use reinhardt_rest::throttling::leaky_bucket::{LeakyBucketThrottle, LeakyBucketConfig};
/// use reinhardt_rest::throttling::{MemoryBackend, Throttle};
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let backend = Arc::new(MemoryBackend::new());
/// let config = LeakyBucketConfig::per_second(10.0, 20);
/// let throttle = LeakyBucketThrottle::new("api_key".to_string(), backend, config);
///
/// // Requests are processed at constant rate
/// assert!(throttle.allow_request("user_123").await.unwrap());
/// # });
/// ```
pub struct LeakyBucketThrottle<B: ThrottleBackend, T: TimeProvider = SystemTimeProvider> {
	#[allow(dead_code)]
	key: String,
	#[allow(dead_code)]
	backend: Arc<B>,
	config: LeakyBucketConfig,
	time_provider: Arc<T>,
	state: Arc<RwLock<BucketState>>,
}

impl<B: ThrottleBackend> LeakyBucketThrottle<B, SystemTimeProvider> {
	/// Creates a new leaky bucket with default time provider
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::leaky_bucket::{LeakyBucketThrottle, LeakyBucketConfig};
	/// use reinhardt_rest::throttling::MemoryBackend;
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let config = LeakyBucketConfig::per_second(5.0, 10);
	/// let throttle = LeakyBucketThrottle::new("api_key".to_string(), backend, config);
	/// ```
	pub fn new(key: String, backend: Arc<B>, config: LeakyBucketConfig) -> Self {
		let initial_state = BucketState {
			level: 0.0,
			last_leak: SystemTimeProvider::new().now(),
		};

		Self {
			key,
			backend,
			config,
			time_provider: Arc::new(SystemTimeProvider::new()),
			state: Arc::new(RwLock::new(initial_state)),
		}
	}
}

impl<B: ThrottleBackend, T: TimeProvider> LeakyBucketThrottle<B, T> {
	/// Creates a new leaky bucket with custom time provider
	pub fn with_time_provider(
		key: String,
		backend: Arc<B>,
		config: LeakyBucketConfig,
		time_provider: Arc<T>,
	) -> Self {
		let initial_state = BucketState {
			level: 0.0,
			last_leak: time_provider.now(),
		};

		Self {
			key,
			backend,
			config,
			time_provider,
			state: Arc::new(RwLock::new(initial_state)),
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
	}

	/// Get current bucket level
	pub async fn level(&self) -> f64 {
		let mut state = self.state.write().await;
		self.leak_bucket(&mut state);
		state.level
	}

	/// Reset the bucket to empty
	pub async fn reset(&self) {
		let mut state = self.state.write().await;
		state.level = 0.0;
		state.last_leak = self.time_provider.now();
	}
}

#[async_trait]
impl<B: ThrottleBackend, T: TimeProvider> Throttle for LeakyBucketThrottle<B, T> {
	async fn allow_request(&self, _key: &str) -> ThrottleResult<bool> {
		let mut state = self.state.write().await;

		// Leak requests first
		self.leak_bucket(&mut state);

		// Check if there's room in the bucket
		if state.level < self.config.capacity as f64 {
			state.level += 1.0;
			Ok(true)
		} else {
			Ok(false)
		}
	}

	async fn wait_time(&self, _key: &str) -> ThrottleResult<Option<u64>> {
		let state = self.state.read().await;

		if state.level < self.config.capacity as f64 {
			return Ok(None);
		}

		// Calculate time until space is available
		// Need to wait until level drops below capacity
		let excess = state.level - (self.config.capacity as f64 - 1.0);
		let wait_secs = (excess / self.config.leak_rate).ceil();

		Ok(Some(wait_secs as u64))
	}

	fn get_rate(&self) -> (usize, u64) {
		(self.config.leak_rate as usize, 1)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::throttling::backend::MemoryBackend;
	use crate::throttling::time_provider::MockTimeProvider;

	#[tokio::test]
	async fn test_leaky_bucket_basic() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = LeakyBucketConfig::new(5, 1.0);
		let throttle = LeakyBucketThrottle::with_time_provider(
			"test".to_string(),
			backend,
			config,
			time_provider,
		);

		// Should allow up to capacity
		for _ in 0..5 {
			assert!(throttle.allow_request("user").await.unwrap());
		}

		// 6th request should fail
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[tokio::test]
	async fn test_leaky_bucket_leak() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = LeakyBucketConfig::new(10, 2.0); // 2 requests per second leak rate
		let throttle = LeakyBucketThrottle::with_time_provider(
			"test".to_string(),
			backend,
			config,
			time_provider.clone(),
		);

		// Fill the bucket
		for _ in 0..10 {
			assert!(throttle.allow_request("user").await.unwrap());
		}
		assert!(!throttle.allow_request("user").await.unwrap());

		// Advance time by 1 second (2 requests should leak)
		time_provider.advance(std::time::Duration::from_secs(1));

		// Should allow 2 more requests
		assert!(throttle.allow_request("user").await.unwrap());
		assert!(throttle.allow_request("user").await.unwrap());
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[tokio::test]
	async fn test_leaky_bucket_smoothing() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = LeakyBucketConfig::per_second(5.0, 10);
		let throttle = LeakyBucketThrottle::with_time_provider(
			"test".to_string(),
			backend,
			config,
			time_provider.clone(),
		);

		// Burst of requests
		for _ in 0..10 {
			assert!(throttle.allow_request("user").await.unwrap());
		}

		// Bucket full
		assert!(!throttle.allow_request("user").await.unwrap());

		// Advance time by 2 seconds (10 requests leak)
		time_provider.advance(std::time::Duration::from_secs(2));

		// Bucket should be empty, allow full capacity again
		for _ in 0..10 {
			assert!(throttle.allow_request("user").await.unwrap());
		}
	}

	#[tokio::test]
	async fn test_leaky_bucket_level() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = LeakyBucketConfig::new(10, 2.0);
		let throttle = LeakyBucketThrottle::with_time_provider(
			"test".to_string(),
			backend,
			config,
			time_provider.clone(),
		);

		// Initial level should be 0
		assert_eq!(throttle.level().await, 0.0);

		// Add 5 requests
		for _ in 0..5 {
			throttle.allow_request("user").await.unwrap();
		}
		assert_eq!(throttle.level().await, 5.0);

		// Advance time by 1 second (2 requests leak)
		time_provider.advance(std::time::Duration::from_secs(1));
		assert_eq!(throttle.level().await, 3.0);
	}

	#[tokio::test]
	async fn test_leaky_bucket_reset() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = LeakyBucketConfig::new(10, 1.0);
		let throttle = LeakyBucketThrottle::with_time_provider(
			"test".to_string(),
			backend,
			config,
			time_provider,
		);

		// Fill bucket
		for _ in 0..10 {
			throttle.allow_request("user").await.unwrap();
		}
		assert!(throttle.level().await > 0.0);

		// Reset
		throttle.reset().await;
		assert_eq!(throttle.level().await, 0.0);
	}

	#[tokio::test]
	async fn test_leaky_bucket_wait_time() {
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = LeakyBucketConfig::new(5, 1.0);
		let throttle = LeakyBucketThrottle::with_time_provider(
			"test".to_string(),
			backend,
			config,
			time_provider,
		);

		// Fill bucket
		for _ in 0..5 {
			throttle.allow_request("user").await.unwrap();
		}

		// Should have wait time
		let wait = throttle.wait_time("user").await.unwrap();
		assert!(wait.is_some());
		assert!(wait.unwrap() > 0);
	}

	#[test]
	fn test_leaky_bucket_config_per_second() {
		let config = LeakyBucketConfig::per_second(10.0, 20);
		assert_eq!(config.leak_rate, 10.0);
		assert_eq!(config.capacity, 20);
	}

	#[test]
	fn test_leaky_bucket_config_per_minute() {
		let config = LeakyBucketConfig::per_minute(60.0, 100);
		assert_eq!(config.leak_rate, 1.0);
		assert_eq!(config.capacity, 100);
	}
}
