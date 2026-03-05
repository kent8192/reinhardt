//! Leaky bucket algorithm implementation for rate limiting
//!
//! The leaky bucket algorithm processes requests at a constant rate,
//! smoothing out bursts. Requests that exceed the bucket's capacity are rejected.

use super::time_provider::{SystemTimeProvider, TimeProvider};
use super::{Throttle, ThrottleError, ThrottleResult};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;

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
	state: Arc<RwLock<BucketState>>,
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
		let initial_state = BucketState {
			level: 0.0,
			last_leak: SystemTimeProvider::new().now(),
		};

		Self {
			config,
			time_provider: Arc::new(SystemTimeProvider::new()),
			state: Arc::new(RwLock::new(initial_state)),
		}
	}
}

impl<T: TimeProvider> LeakyBucketThrottle<T> {
	/// Creates a new leaky bucket with custom time provider
	pub fn with_time_provider(config: LeakyBucketConfig, time_provider: Arc<T>) -> Self {
		let initial_state = BucketState {
			level: 0.0,
			last_leak: time_provider.now(),
		};

		Self {
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
impl<T: TimeProvider> Throttle for LeakyBucketThrottle<T> {
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
		assert_eq!(throttle.level().await, 0.0);

		// Act - add 5 requests
		for _ in 0..5 {
			throttle.allow_request("user").await.unwrap();
		}

		// Assert
		assert_eq!(throttle.level().await, 5.0);

		// Act - advance time by 1 second (2 requests leak)
		time_provider.advance(std::time::Duration::from_secs(1));

		// Assert
		assert_eq!(throttle.level().await, 3.0);
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
		assert!(throttle.level().await > 0.0);

		// Act - reset
		throttle.reset().await;

		// Assert
		assert_eq!(throttle.level().await, 0.0);
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
}
