//! Token bucket algorithm implementation for rate limiting
//!
//! The token bucket algorithm allows for burst traffic while maintaining
//! an average rate limit. Tokens are added to a bucket at a fixed rate,
//! and each request consumes one or more tokens.

use super::time_provider::{SystemTimeProvider, TimeProvider};
use super::{Throttle, ThrottleError, ThrottleResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

/// Token bucket configuration
#[derive(Debug, Clone)]
pub struct TokenBucketConfig {
	/// Maximum number of tokens in the bucket (burst capacity)
	pub capacity: usize,
	/// Number of tokens added per refill interval
	pub refill_rate: usize,
	/// Refill interval in seconds
	pub refill_interval: u64,
	/// Number of tokens consumed per request
	pub tokens_per_request: usize,
}

impl TokenBucketConfig {
	/// Creates a new token bucket configuration
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidConfig`] if `refill_interval` is zero.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::token_bucket::TokenBucketConfig;
	///
	/// // 100 requests per minute with burst capacity of 100
	/// let config = TokenBucketConfig::new(100, 100, 60, 1).unwrap();
	/// assert_eq!(config.capacity, 100);
	/// assert_eq!(config.refill_rate, 100);
	/// ```
	pub fn new(
		capacity: usize,
		refill_rate: usize,
		refill_interval: u64,
		tokens_per_request: usize,
	) -> ThrottleResult<Self> {
		if refill_interval == 0 {
			return Err(ThrottleError::InvalidConfig(
				"refill_interval must be non-zero".to_string(),
			));
		}
		Ok(Self {
			capacity,
			refill_rate,
			refill_interval,
			tokens_per_request,
		})
	}

	/// Creates a builder for fluent configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::token_bucket::TokenBucketConfig;
	///
	/// let config = TokenBucketConfig::builder()
	///     .capacity(50)
	///     .refill_rate(10)
	///     .refill_interval(1)
	///     .tokens_per_request(1)
	///     .build()
	///     .unwrap();
	///
	/// assert_eq!(config.capacity, 50);
	/// assert_eq!(config.refill_rate, 10);
	/// ```
	pub fn builder() -> TokenBucketConfigBuilder {
		TokenBucketConfigBuilder::default()
	}

	/// Create configuration for requests per second
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::token_bucket::TokenBucketConfig;
	///
	/// // 10 requests per second with burst of 20
	/// let config = TokenBucketConfig::per_second(10, 20);
	/// assert_eq!(config.refill_rate, 10);
	/// assert_eq!(config.capacity, 20);
	/// ```
	pub fn per_second(rate: usize, burst: usize) -> Self {
		Self {
			capacity: burst,
			refill_rate: rate,
			refill_interval: 1,
			tokens_per_request: 1,
		}
	}

	/// Create configuration for requests per minute
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::token_bucket::TokenBucketConfig;
	///
	/// // 100 requests per minute with burst of 150
	/// let config = TokenBucketConfig::per_minute(100, 150);
	/// assert_eq!(config.refill_rate, 100);
	/// assert_eq!(config.refill_interval, 60);
	/// ```
	pub fn per_minute(rate: usize, burst: usize) -> Self {
		Self {
			capacity: burst,
			refill_rate: rate,
			refill_interval: 60,
			tokens_per_request: 1,
		}
	}

	/// Create configuration for requests per hour
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::token_bucket::TokenBucketConfig;
	///
	/// // 1000 requests per hour with burst of 1500
	/// let config = TokenBucketConfig::per_hour(1000, 1500);
	/// assert_eq!(config.refill_rate, 1000);
	/// assert_eq!(config.refill_interval, 3600);
	/// ```
	pub fn per_hour(rate: usize, burst: usize) -> Self {
		Self {
			capacity: burst,
			refill_rate: rate,
			refill_interval: 3600,
			tokens_per_request: 1,
		}
	}
}

/// Builder for TokenBucketConfig
#[derive(Debug, Default)]
pub struct TokenBucketConfigBuilder {
	capacity: Option<usize>,
	refill_rate: Option<usize>,
	refill_interval: Option<u64>,
	tokens_per_request: Option<usize>,
}

impl TokenBucketConfigBuilder {
	/// Set bucket capacity
	pub fn capacity(mut self, capacity: usize) -> Self {
		self.capacity = Some(capacity);
		self
	}

	/// Set refill rate
	pub fn refill_rate(mut self, rate: usize) -> Self {
		self.refill_rate = Some(rate);
		self
	}

	/// Set refill interval in seconds
	pub fn refill_interval(mut self, interval: u64) -> Self {
		self.refill_interval = Some(interval);
		self
	}

	/// Set tokens consumed per request
	pub fn tokens_per_request(mut self, tokens: usize) -> Self {
		self.tokens_per_request = Some(tokens);
		self
	}

	/// Build the configuration
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidConfig`] if any required field is not set
	/// or if `refill_interval` is zero.
	pub fn build(self) -> ThrottleResult<TokenBucketConfig> {
		let refill_interval = self.refill_interval.unwrap_or(0);
		if refill_interval == 0 {
			return Err(ThrottleError::InvalidConfig(
				"refill_interval must be set and non-zero".to_string(),
			));
		}
		Ok(TokenBucketConfig {
			capacity: self
				.capacity
				.ok_or_else(|| ThrottleError::InvalidConfig("capacity must be set".to_string()))?,
			refill_rate: self.refill_rate.ok_or_else(|| {
				ThrottleError::InvalidConfig("refill_rate must be set".to_string())
			})?,
			refill_interval,
			tokens_per_request: self.tokens_per_request.unwrap_or(1),
		})
	}
}

/// Bucket state for tracking tokens
#[derive(Debug, Clone)]
struct BucketState {
	tokens: usize,
	last_refill: Instant,
}

/// Token bucket throttle implementation
///
/// # Examples
///
/// ```
/// use reinhardt_throttling::token_bucket::{TokenBucket, TokenBucketConfig};
/// use reinhardt_throttling::Throttle;
///
/// # tokio_test::block_on(async {
/// let config = TokenBucketConfig::per_second(10, 20);
/// let throttle = TokenBucket::new(config);
///
/// // First request should succeed
/// assert!(throttle.allow_request("user_123").await.unwrap());
/// # });
/// ```
pub struct TokenBucket<T: TimeProvider = SystemTimeProvider> {
	config: TokenBucketConfig,
	time_provider: Arc<T>,
	buckets: Arc<RwLock<HashMap<String, BucketState>>>,
}

impl TokenBucket<SystemTimeProvider> {
	/// Creates a new token bucket with default time provider
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::token_bucket::{TokenBucket, TokenBucketConfig};
	///
	/// let config = TokenBucketConfig::per_second(5, 10);
	/// let throttle = TokenBucket::new(config);
	/// ```
	pub fn new(config: TokenBucketConfig) -> Self {
		Self {
			config,
			time_provider: Arc::new(SystemTimeProvider::new()),
			buckets: Arc::new(RwLock::new(HashMap::new())),
		}
	}
}

impl<T: TimeProvider> TokenBucket<T> {
	/// Creates a new token bucket with custom time provider
	pub fn with_time_provider(config: TokenBucketConfig, time_provider: Arc<T>) -> Self {
		Self {
			config,
			time_provider,
			buckets: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Create a new bucket state initialized with full capacity
	fn new_bucket_state(&self) -> BucketState {
		BucketState {
			tokens: self.config.capacity,
			last_refill: self.time_provider.now(),
		}
	}

	/// Refill tokens based on elapsed time
	fn refill_tokens(&self, state: &mut BucketState) {
		let now = self.time_provider.now();
		let elapsed = now.duration_since(state.last_refill);
		let refill_duration = Duration::from_secs(self.config.refill_interval);

		if elapsed >= refill_duration {
			// Calculate number of refill intervals that have passed
			let intervals = elapsed.as_secs() / self.config.refill_interval;
			let tokens_to_add = (intervals as usize) * self.config.refill_rate;

			// Add tokens but cap at capacity
			state.tokens = (state.tokens + tokens_to_add).min(self.config.capacity);

			// Update last refill time
			state.last_refill = now;
		}
	}

	/// Try to consume tokens from the bucket for a given key
	async fn consume_tokens(&self, key: &str, count: usize) -> ThrottleResult<bool> {
		let mut buckets = self.buckets.write().await;
		let new_state = self.new_bucket_state();
		let state = buckets.entry(key.to_string()).or_insert(new_state);

		// Refill tokens first
		self.refill_tokens(state);

		// Check if we have enough tokens
		if state.tokens >= count {
			state.tokens -= count;
			Ok(true)
		} else {
			Ok(false)
		}
	}

	/// Get current token count for a given key
	pub async fn tokens_for_key(&self, key: &str) -> usize {
		let mut buckets = self.buckets.write().await;
		let new_state = self.new_bucket_state();
		let state = buckets.entry(key.to_string()).or_insert(new_state);
		self.refill_tokens(state);
		state.tokens
	}

	/// Get current token count (uses default empty key for backward compatibility)
	pub async fn tokens(&self) -> usize {
		self.tokens_for_key("").await
	}

	/// Reset the bucket for a specific key
	pub async fn reset_key(&self, key: &str) {
		let mut buckets = self.buckets.write().await;
		if let Some(state) = buckets.get_mut(key) {
			state.tokens = self.config.capacity;
			state.last_refill = self.time_provider.now();
		}
	}

	/// Reset all buckets
	pub async fn reset(&self) {
		let mut buckets = self.buckets.write().await;
		buckets.clear();
	}
}

#[async_trait]
impl<T: TimeProvider> Throttle for TokenBucket<T> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		self.consume_tokens(key, self.config.tokens_per_request)
			.await
	}

	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		let mut buckets = self.buckets.write().await;
		let new_state = self.new_bucket_state();
		let state = buckets.entry(key.to_string()).or_insert(new_state);

		if state.tokens >= self.config.tokens_per_request {
			return Ok(None);
		}

		// Calculate time until next refill
		let now = self.time_provider.now();
		let elapsed = now.duration_since(state.last_refill);
		let refill_duration = Duration::from_secs(self.config.refill_interval);

		if elapsed < refill_duration {
			let wait = refill_duration - elapsed;
			Ok(Some(wait.as_secs()))
		} else {
			Ok(Some(0))
		}
	}

	fn get_rate(&self) -> (usize, u64) {
		(self.config.refill_rate, self.config.refill_interval)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::time_provider::MockTimeProvider;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_basic() {
		// Arrange
		let config = TokenBucketConfig::new(5, 5, 10, 1).unwrap();
		let throttle = TokenBucket::new(config);

		// Act & Assert - should allow 5 requests (capacity)
		for _ in 0..5 {
			assert!(throttle.allow_request("user").await.unwrap());
		}

		// Assert - 6th request should fail
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_refill() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = TokenBucketConfig::new(10, 5, 1, 1).unwrap();
		let throttle = TokenBucket::with_time_provider(config.clone(), time_provider.clone());

		// Act - consume all tokens
		for _ in 0..10 {
			assert!(throttle.allow_request("user").await.unwrap());
		}
		assert!(!throttle.allow_request("user").await.unwrap());

		// Act - advance time by refill interval
		time_provider.advance(std::time::Duration::from_secs(1));

		// Assert - should have 5 new tokens
		for _ in 0..5 {
			assert!(throttle.allow_request("user").await.unwrap());
		}
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_burst() {
		// Arrange
		let config = TokenBucketConfig::per_second(5, 20);
		let throttle = TokenBucket::new(config);

		// Act & Assert - should handle burst of 20
		for _ in 0..20 {
			assert!(throttle.allow_request("user").await.unwrap());
		}

		// Assert - 21st should fail
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_tokens_per_request() {
		// Arrange
		let config = TokenBucketConfig::new(10, 10, 10, 2).unwrap();
		let throttle = TokenBucket::new(config);

		// Act & Assert - should allow 5 requests (10 tokens / 2 per request)
		for _ in 0..5 {
			assert!(throttle.allow_request("user").await.unwrap());
		}

		// Assert - 6th request should fail
		assert!(!throttle.allow_request("user").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_get_tokens() {
		// Arrange
		let config = TokenBucketConfig::new(10, 5, 10, 1).unwrap();
		let throttle = TokenBucket::new(config);

		// Assert - initial tokens should equal capacity
		assert_eq!(throttle.tokens_for_key("user").await, 10);

		// Act - consume 3 tokens
		for _ in 0..3 {
			throttle.allow_request("user").await.unwrap();
		}

		// Assert
		assert_eq!(throttle.tokens_for_key("user").await, 7);
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_per_key_isolation() {
		// Arrange
		let config = TokenBucketConfig::new(3, 3, 60, 1).unwrap();
		let throttle = TokenBucket::new(config);

		// Act - exhaust user_a's tokens
		for _ in 0..3 {
			assert!(throttle.allow_request("user_a").await.unwrap());
		}

		// Assert - user_a is throttled
		assert!(!throttle.allow_request("user_a").await.unwrap());

		// Assert - user_b still has full capacity (independent bucket)
		for _ in 0..3 {
			assert!(throttle.allow_request("user_b").await.unwrap());
		}
		assert!(!throttle.allow_request("user_b").await.unwrap());

		// Assert - token counts are independent
		assert_eq!(throttle.tokens_for_key("user_a").await, 0);
		assert_eq!(throttle.tokens_for_key("user_b").await, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_reset() {
		// Arrange
		let config = TokenBucketConfig::new(10, 5, 10, 1).unwrap();
		let throttle = TokenBucket::new(config);

		// Act - consume all tokens
		for _ in 0..10 {
			throttle.allow_request("user").await.unwrap();
		}
		assert_eq!(throttle.tokens_for_key("user").await, 0);

		// Act - reset all buckets
		throttle.reset().await;

		// Assert - after reset, a new bucket is created with full capacity
		assert_eq!(throttle.tokens_for_key("user").await, 10);
	}

	#[rstest]
	#[tokio::test]
	async fn test_token_bucket_wait_time() {
		// Arrange
		use tokio::time::Instant;
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let config = TokenBucketConfig::new(5, 5, 10, 1).unwrap();
		let throttle = TokenBucket::with_time_provider(config, time_provider.clone());

		// Act - consume all tokens
		for _ in 0..5 {
			throttle.allow_request("user").await.unwrap();
		}

		// Assert - should have wait time
		let wait = throttle.wait_time("user").await.unwrap();
		assert!(wait.is_some());
		assert!(wait.unwrap() > 0);
	}

	#[rstest]
	fn test_token_bucket_config_builder() {
		// Arrange & Act
		let config = TokenBucketConfig::builder()
			.capacity(100)
			.refill_rate(50)
			.refill_interval(60)
			.tokens_per_request(2)
			.build()
			.unwrap();

		// Assert
		assert_eq!(config.capacity, 100);
		assert_eq!(config.refill_rate, 50);
		assert_eq!(config.refill_interval, 60);
		assert_eq!(config.tokens_per_request, 2);
	}

	#[rstest]
	fn test_token_bucket_config_per_second() {
		// Arrange & Act
		let config = TokenBucketConfig::per_second(10, 20);

		// Assert
		assert_eq!(config.refill_rate, 10);
		assert_eq!(config.capacity, 20);
		assert_eq!(config.refill_interval, 1);
	}

	#[rstest]
	fn test_token_bucket_config_per_minute() {
		// Arrange & Act
		let config = TokenBucketConfig::per_minute(100, 150);

		// Assert
		assert_eq!(config.refill_rate, 100);
		assert_eq!(config.capacity, 150);
		assert_eq!(config.refill_interval, 60);
	}

	#[rstest]
	fn test_token_bucket_config_per_hour() {
		// Arrange & Act
		let config = TokenBucketConfig::per_hour(1000, 1500);

		// Assert
		assert_eq!(config.refill_rate, 1000);
		assert_eq!(config.capacity, 1500);
		assert_eq!(config.refill_interval, 3600);
	}

	#[rstest]
	fn test_new_rejects_zero_refill_interval() {
		// Arrange & Act
		let result = TokenBucketConfig::new(10, 5, 0, 1);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_builder_rejects_zero_refill_interval() {
		// Arrange & Act
		let result = TokenBucketConfig::builder()
			.capacity(10)
			.refill_rate(5)
			.refill_interval(0)
			.tokens_per_request(1)
			.build();

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_builder_rejects_missing_refill_interval() {
		// Arrange & Act
		let result = TokenBucketConfig::builder()
			.capacity(10)
			.refill_rate(5)
			.build();

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_builder_rejects_missing_capacity() {
		// Arrange & Act
		let result = TokenBucketConfig::builder()
			.refill_rate(5)
			.refill_interval(10)
			.build();

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_builder_rejects_missing_refill_rate() {
		// Arrange & Act
		let result = TokenBucketConfig::builder()
			.capacity(10)
			.refill_interval(10)
			.build();

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::InvalidConfig(_)
		));
	}
}
