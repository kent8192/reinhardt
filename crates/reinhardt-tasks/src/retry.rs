//! Task retry logic with exponential backoff

use std::time::Duration;

/// Retry strategy configuration
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::RetryStrategy;
/// use std::time::Duration;
///
/// let strategy = RetryStrategy::exponential_backoff()
///     .with_max_retries(5)
///     .with_max_delay(Duration::from_secs(300));
///
/// assert_eq!(strategy.max_retries(), 5);
/// ```
#[derive(Debug, Clone)]
pub struct RetryStrategy {
	/// Maximum number of retry attempts
	max_retries: u32,
	/// Initial delay between retries
	initial_delay: Duration,
	/// Maximum delay between retries
	max_delay: Duration,
	/// Backoff multiplier
	multiplier: f64,
	/// Add jitter to prevent thundering herd
	jitter: bool,
}

impl RetryStrategy {
	/// Create a new exponential backoff retry strategy
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// assert_eq!(strategy.max_retries(), 3);
	/// ```
	pub fn exponential_backoff() -> Self {
		Self {
			max_retries: 3,
			initial_delay: Duration::from_secs(1),
			max_delay: Duration::from_secs(60),
			multiplier: 2.0,
			jitter: true,
		}
	}

	/// Create a fixed delay retry strategy
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	/// use std::time::Duration;
	///
	/// let strategy = RetryStrategy::fixed_delay(Duration::from_secs(5));
	/// assert_eq!(strategy.max_retries(), 3);
	/// ```
	pub fn fixed_delay(delay: Duration) -> Self {
		Self {
			max_retries: 3,
			initial_delay: delay,
			max_delay: delay,
			multiplier: 1.0,
			jitter: false,
		}
	}

	/// Create a strategy with no retries
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::no_retry();
	/// assert_eq!(strategy.max_retries(), 0);
	/// ```
	pub fn no_retry() -> Self {
		Self {
			max_retries: 0,
			initial_delay: Duration::from_secs(0),
			max_delay: Duration::from_secs(0),
			multiplier: 1.0,
			jitter: false,
		}
	}

	/// Set the maximum number of retries
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff()
	///     .with_max_retries(5);
	/// assert_eq!(strategy.max_retries(), 5);
	/// ```
	pub fn with_max_retries(mut self, max_retries: u32) -> Self {
		self.max_retries = max_retries;
		self
	}

	/// Set the initial delay
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	/// use std::time::Duration;
	///
	/// let strategy = RetryStrategy::exponential_backoff()
	///     .with_initial_delay(Duration::from_millis(500));
	/// ```
	pub fn with_initial_delay(mut self, delay: Duration) -> Self {
		self.initial_delay = delay;
		self
	}

	/// Set the maximum delay
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	/// use std::time::Duration;
	///
	/// let strategy = RetryStrategy::exponential_backoff()
	///     .with_max_delay(Duration::from_secs(300));
	/// ```
	pub fn with_max_delay(mut self, delay: Duration) -> Self {
		self.max_delay = delay;
		self
	}

	/// Set the backoff multiplier
	///
	/// The multiplier must be greater than 0.0.
	/// Values <= 0.0 or NaN/Infinity are rejected with a panic to prevent
	/// infinite loops or nonsensical delay calculations.
	///
	/// # Panics
	///
	/// Panics if `multiplier` is <= 0.0, NaN, or Infinity.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff()
	///     .with_multiplier(3.0);
	/// ```
	pub fn with_multiplier(mut self, multiplier: f64) -> Self {
		assert!(
			multiplier > 0.0 && multiplier.is_finite(),
			"RetryStrategy multiplier must be a positive finite number, got {}",
			multiplier
		);
		self.multiplier = multiplier;
		self
	}

	/// Enable or disable jitter
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff()
	///     .with_jitter(false);
	/// ```
	pub fn with_jitter(mut self, jitter: bool) -> Self {
		self.jitter = jitter;
		self
	}

	/// Get the maximum number of retries
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// assert_eq!(strategy.max_retries(), 3);
	/// ```
	pub fn max_retries(&self) -> u32 {
		self.max_retries
	}

	/// Calculate the delay for a given retry attempt
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff()
	///     .with_jitter(false); // Disable jitter for predictable results
	///
	/// let delay1 = strategy.calculate_delay(1);
	/// let delay2 = strategy.calculate_delay(2);
	/// let delay3 = strategy.calculate_delay(3);
	///
	/// assert!(delay2 > delay1);
	/// assert!(delay3 > delay2);
	/// ```
	pub fn calculate_delay(&self, attempt: u32) -> Duration {
		if attempt == 0 || self.max_retries == 0 {
			return Duration::from_secs(0);
		}

		// Calculate base delay: initial_delay * multiplier^(attempt - 1)
		let base_delay_secs =
			self.initial_delay.as_secs_f64() * self.multiplier.powi((attempt - 1) as i32);

		// Cap at max delay
		let delay_secs = base_delay_secs.min(self.max_delay.as_secs_f64());

		// Add jitter if enabled (random value between 0% and 100% of delay)

		if self.jitter {
			let jitter_factor = rand::random::<f64>();
			Duration::from_secs_f64(delay_secs * jitter_factor)
		} else {
			Duration::from_secs_f64(delay_secs)
		}
	}

	/// Check if more retries are allowed
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff()
	///     .with_max_retries(3);
	///
	/// assert!(strategy.should_retry(0));
	/// assert!(strategy.should_retry(1));
	/// assert!(strategy.should_retry(2));
	/// assert!(!strategy.should_retry(3));
	/// assert!(!strategy.should_retry(4));
	/// ```
	pub fn should_retry(&self, attempt: u32) -> bool {
		attempt < self.max_retries
	}

	/// Get the initial delay
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	/// use std::time::Duration;
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// assert_eq!(strategy.initial_delay(), Duration::from_secs(1));
	/// ```
	pub fn initial_delay(&self) -> Duration {
		self.initial_delay
	}

	/// Get the maximum delay
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	/// use std::time::Duration;
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// assert_eq!(strategy.max_delay(), Duration::from_secs(60));
	/// ```
	pub fn max_delay(&self) -> Duration {
		self.max_delay
	}

	/// Get the multiplier
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// assert_eq!(strategy.multiplier(), 2.0);
	/// ```
	pub fn multiplier(&self) -> f64 {
		self.multiplier
	}

	/// Check if jitter is enabled
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::RetryStrategy;
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// assert!(strategy.has_jitter());
	/// ```
	pub fn has_jitter(&self) -> bool {
		self.jitter
	}
}

impl Default for RetryStrategy {
	fn default() -> Self {
		Self::exponential_backoff()
	}
}

/// Retry state tracker
///
/// # Examples
///
/// ```rust
/// use reinhardt_tasks::{RetryState, RetryStrategy};
///
/// let strategy = RetryStrategy::exponential_backoff();
/// let mut state = RetryState::new(strategy);
///
/// assert_eq!(state.attempts(), 0);
/// assert!(state.can_retry());
/// ```
#[derive(Debug, Clone)]
pub struct RetryState {
	strategy: RetryStrategy,
	attempts: u32,
}

impl RetryState {
	/// Create a new retry state
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{RetryState, RetryStrategy};
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// let state = RetryState::new(strategy);
	/// assert_eq!(state.attempts(), 0);
	/// ```
	pub fn new(strategy: RetryStrategy) -> Self {
		Self {
			strategy,
			attempts: 0,
		}
	}

	/// Get the number of retry attempts made
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{RetryState, RetryStrategy};
	///
	/// let mut state = RetryState::new(RetryStrategy::exponential_backoff());
	/// assert_eq!(state.attempts(), 0);
	///
	/// state.record_attempt();
	/// assert_eq!(state.attempts(), 1);
	/// ```
	pub fn attempts(&self) -> u32 {
		self.attempts
	}

	/// Record a retry attempt
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{RetryState, RetryStrategy};
	///
	/// let mut state = RetryState::new(RetryStrategy::exponential_backoff());
	/// state.record_attempt();
	/// assert_eq!(state.attempts(), 1);
	/// ```
	pub fn record_attempt(&mut self) {
		self.attempts += 1;
	}

	/// Check if more retries are allowed
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{RetryState, RetryStrategy};
	///
	/// let mut state = RetryState::new(
	///     RetryStrategy::exponential_backoff().with_max_retries(2)
	/// );
	///
	/// assert!(state.can_retry());
	/// state.record_attempt();
	/// assert!(state.can_retry());
	/// state.record_attempt();
	/// assert!(!state.can_retry());
	/// ```
	pub fn can_retry(&self) -> bool {
		self.strategy.should_retry(self.attempts)
	}

	/// Get the delay for the next retry
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{RetryState, RetryStrategy};
	/// use std::time::Duration;
	///
	/// let mut state = RetryState::new(
	///     RetryStrategy::exponential_backoff().with_jitter(false)
	/// );
	///
	/// state.record_attempt();
	/// let delay = state.next_delay();
	/// assert!(delay > Duration::from_secs(0));
	/// ```
	pub fn next_delay(&self) -> Duration {
		self.strategy.calculate_delay(self.attempts + 1)
	}

	/// Get the retry strategy
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{RetryState, RetryStrategy};
	///
	/// let strategy = RetryStrategy::exponential_backoff();
	/// let state = RetryState::new(strategy.clone());
	/// assert_eq!(state.strategy().max_retries(), strategy.max_retries());
	/// ```
	pub fn strategy(&self) -> &RetryStrategy {
		&self.strategy
	}

	/// Reset the retry state
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_tasks::{RetryState, RetryStrategy};
	///
	/// let mut state = RetryState::new(RetryStrategy::exponential_backoff());
	/// state.record_attempt();
	/// state.record_attempt();
	/// assert_eq!(state.attempts(), 2);
	///
	/// state.reset();
	/// assert_eq!(state.attempts(), 0);
	/// ```
	pub fn reset(&mut self) {
		self.attempts = 0;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_exponential_backoff_creation() {
		let strategy = RetryStrategy::exponential_backoff();
		assert_eq!(strategy.max_retries(), 3);
		assert_eq!(strategy.initial_delay(), Duration::from_secs(1));
		assert_eq!(strategy.max_delay(), Duration::from_secs(60));
		assert_eq!(strategy.multiplier(), 2.0);
		assert!(strategy.has_jitter());
	}

	#[test]
	fn test_fixed_delay_creation() {
		let delay = Duration::from_secs(5);
		let strategy = RetryStrategy::fixed_delay(delay);
		assert_eq!(strategy.initial_delay(), delay);
		assert_eq!(strategy.max_delay(), delay);
		assert_eq!(strategy.multiplier(), 1.0);
		assert!(!strategy.has_jitter());
	}

	#[test]
	fn test_no_retry_creation() {
		let strategy = RetryStrategy::no_retry();
		assert_eq!(strategy.max_retries(), 0);
	}

	#[test]
	fn test_strategy_builder() {
		let strategy = RetryStrategy::exponential_backoff()
			.with_max_retries(5)
			.with_initial_delay(Duration::from_millis(500))
			.with_max_delay(Duration::from_secs(120))
			.with_multiplier(3.0)
			.with_jitter(false);

		assert_eq!(strategy.max_retries(), 5);
		assert_eq!(strategy.initial_delay(), Duration::from_millis(500));
		assert_eq!(strategy.max_delay(), Duration::from_secs(120));
		assert_eq!(strategy.multiplier(), 3.0);
		assert!(!strategy.has_jitter());
	}

	#[test]
	fn test_calculate_delay_without_jitter() {
		let strategy = RetryStrategy::exponential_backoff()
			.with_initial_delay(Duration::from_secs(1))
			.with_multiplier(2.0)
			.with_jitter(false);

		let delay1 = strategy.calculate_delay(1);
		let delay2 = strategy.calculate_delay(2);
		let delay3 = strategy.calculate_delay(3);

		assert_eq!(delay1, Duration::from_secs(1)); // 1 * 2^0
		assert_eq!(delay2, Duration::from_secs(2)); // 1 * 2^1
		assert_eq!(delay3, Duration::from_secs(4)); // 1 * 2^2
	}

	#[test]
	fn test_calculate_delay_with_max() {
		let strategy = RetryStrategy::exponential_backoff()
			.with_initial_delay(Duration::from_secs(1))
			.with_max_delay(Duration::from_secs(5))
			.with_multiplier(2.0)
			.with_jitter(false);

		let delay5 = strategy.calculate_delay(5);
		assert_eq!(delay5, Duration::from_secs(5)); // Capped at max
	}

	#[test]
	fn test_should_retry() {
		let strategy = RetryStrategy::exponential_backoff().with_max_retries(3);

		assert!(strategy.should_retry(0));
		assert!(strategy.should_retry(1));
		assert!(strategy.should_retry(2));
		assert!(!strategy.should_retry(3));
		assert!(!strategy.should_retry(4));
	}

	#[test]
	fn test_retry_state() {
		let strategy = RetryStrategy::exponential_backoff().with_max_retries(2);
		let mut state = RetryState::new(strategy);

		assert_eq!(state.attempts(), 0);
		assert!(state.can_retry());

		state.record_attempt();
		assert_eq!(state.attempts(), 1);
		assert!(state.can_retry());

		state.record_attempt();
		assert_eq!(state.attempts(), 2);
		assert!(!state.can_retry());
	}

	#[test]
	fn test_retry_state_reset() {
		let strategy = RetryStrategy::exponential_backoff();
		let mut state = RetryState::new(strategy);

		state.record_attempt();
		state.record_attempt();
		assert_eq!(state.attempts(), 2);

		state.reset();
		assert_eq!(state.attempts(), 0);
	}

	#[test]
	fn test_next_delay() {
		let strategy = RetryStrategy::exponential_backoff()
			.with_initial_delay(Duration::from_secs(1))
			.with_jitter(false);
		let mut state = RetryState::new(strategy);

		state.record_attempt();
		let delay = state.next_delay();
		assert!(delay >= Duration::from_secs(1));
	}
}
