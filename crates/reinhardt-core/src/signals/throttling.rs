#![cfg(not(target_arch = "wasm32"))]

//! Signal throttling system for rate-limiting signal emissions
//!
//! This module provides functionality to throttle signal emissions based on various strategies,
//! preventing excessive signal dispatches and protecting downstream systems from overload.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::throttling::{ThrottleConfig, ThrottleStrategy, SignalThrottle};
//! use reinhardt_core::signals::{Signal, SignalName};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let signal = Signal::<String>::new(SignalName::custom("user_events"));
//!
//! // Fixed window throttling: max 100 signals per second
//! let config = ThrottleConfig::new()
//!     .with_strategy(ThrottleStrategy::FixedWindow)
//!     .with_max_emissions(100)
//!     .with_window_size(Duration::from_secs(1));
//!
//! let throttle = SignalThrottle::new(signal, config);
//!
//! // Attempt to send signal (may be throttled)
//! throttle.send("user_action".to_string()).await?;
//! # Ok(())
//! # }
//! ```

use super::error::SignalError;
use super::signal::Signal;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Throttling strategies for rate limiting
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::throttling::ThrottleStrategy;
///
/// let fixed = ThrottleStrategy::FixedWindow;
/// let sliding = ThrottleStrategy::SlidingWindow;
/// let token = ThrottleStrategy::TokenBucket { refill_rate: 10 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThrottleStrategy {
	/// Fixed time window strategy
	///
	/// Allows a fixed number of emissions per time window. Counter resets at window boundaries.
	FixedWindow,

	/// Sliding time window strategy
	///
	/// Allows a fixed number of emissions in any sliding time window. More accurate than fixed window.
	SlidingWindow,

	/// Token bucket strategy
	///
	/// Maintains a bucket of tokens that refill at a constant rate. Each emission consumes a token.
	TokenBucket {
		/// Number of tokens to refill per second
		refill_rate: u32,
	},

	/// Leaky bucket strategy
	///
	/// Processes signals at a constant rate, smoothing out bursts.
	LeakyBucket {
		/// Processing rate (signals per second)
		leak_rate: u32,
	},
}

/// Configuration for signal throttling
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::throttling::{ThrottleConfig, ThrottleStrategy};
/// use std::time::Duration;
///
/// let config = ThrottleConfig::new()
///     .with_strategy(ThrottleStrategy::SlidingWindow)
///     .with_max_emissions(50)
///     .with_window_size(Duration::from_secs(1));
///
/// assert_eq!(config.max_emissions(), 50);
/// ```
#[derive(Debug, Clone)]
pub struct ThrottleConfig {
	/// Throttling strategy to use
	strategy: ThrottleStrategy,
	/// Maximum emissions allowed in the time window
	max_emissions: u32,
	/// Size of the time window
	window_size: Duration,
	/// Whether to drop excess signals or queue them
	drop_on_limit: bool,
}

impl ThrottleConfig {
	/// Create a new throttle configuration with default values
	///
	/// Defaults:
	/// - `strategy`: FixedWindow
	/// - `max_emissions`: 100
	/// - `window_size`: 1 second
	/// - `drop_on_limit`: true
	pub fn new() -> Self {
		Self {
			strategy: ThrottleStrategy::FixedWindow,
			max_emissions: 100,
			window_size: Duration::from_secs(1),
			drop_on_limit: true,
		}
	}

	/// Set the throttling strategy
	pub fn with_strategy(mut self, strategy: ThrottleStrategy) -> Self {
		self.strategy = strategy;
		self
	}

	/// Set the maximum number of emissions allowed
	pub fn with_max_emissions(mut self, max: u32) -> Self {
		self.max_emissions = max;
		self
	}

	/// Set the time window size
	pub fn with_window_size(mut self, window: Duration) -> Self {
		self.window_size = window;
		self
	}

	/// Set whether to drop signals when limit is reached (true) or queue them (false)
	pub fn with_drop_on_limit(mut self, drop: bool) -> Self {
		self.drop_on_limit = drop;
		self
	}

	/// Get the throttling strategy
	pub fn strategy(&self) -> ThrottleStrategy {
		self.strategy
	}

	/// Get the maximum emissions allowed
	pub fn max_emissions(&self) -> u32 {
		self.max_emissions
	}

	/// Get the window size
	pub fn window_size(&self) -> Duration {
		self.window_size
	}

	/// Check if signals should be dropped on limit
	pub fn drop_on_limit(&self) -> bool {
		self.drop_on_limit
	}
}

impl Default for ThrottleConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Internal throttle state
struct ThrottleState<T> {
	/// Emission timestamps for sliding window
	emissions: VecDeque<Instant>,
	/// Current token count for token bucket
	tokens: f64,
	/// Last token refill time
	last_refill: Instant,
	/// Fixed window start time
	window_start: Instant,
	/// Fixed window emission count
	window_count: u32,
	/// Queued items (when drop_on_limit is false)
	queue: VecDeque<T>,
}

impl<T> ThrottleState<T> {
	fn new(max_emissions: u32) -> Self {
		Self {
			emissions: VecDeque::new(),
			tokens: max_emissions as f64,
			last_refill: Instant::now(),
			window_start: Instant::now(),
			window_count: 0,
			queue: VecDeque::new(),
		}
	}

	fn can_emit_fixed_window(&mut self, config: &ThrottleConfig) -> bool {
		let now = Instant::now();

		// Check if window has expired
		if now.duration_since(self.window_start) >= config.window_size {
			self.window_start = now;
			self.window_count = 0;
		}

		if self.window_count < config.max_emissions {
			self.window_count += 1;
			true
		} else {
			false
		}
	}

	fn can_emit_sliding_window(&mut self, config: &ThrottleConfig) -> bool {
		let now = Instant::now();
		let window_start = now - config.window_size;

		// Remove old emissions outside the sliding window
		while let Some(&emission_time) = self.emissions.front() {
			if emission_time < window_start {
				self.emissions.pop_front();
			} else {
				break;
			}
		}

		if self.emissions.len() < config.max_emissions as usize {
			self.emissions.push_back(now);
			true
		} else {
			false
		}
	}

	fn can_emit_token_bucket(&mut self, refill_rate: u32) -> bool {
		let now = Instant::now();
		let elapsed = now.duration_since(self.last_refill).as_secs_f64();

		// Refill tokens based on elapsed time
		let tokens_to_add = elapsed * refill_rate as f64;
		self.tokens = (self.tokens + tokens_to_add).min(refill_rate as f64);
		self.last_refill = now;

		if self.tokens >= 1.0 {
			self.tokens -= 1.0;
			true
		} else {
			false
		}
	}

	fn can_emit(&mut self, config: &ThrottleConfig) -> bool {
		match config.strategy {
			ThrottleStrategy::FixedWindow => self.can_emit_fixed_window(config),
			ThrottleStrategy::SlidingWindow => self.can_emit_sliding_window(config),
			ThrottleStrategy::TokenBucket { refill_rate } => {
				self.can_emit_token_bucket(refill_rate)
			}
			ThrottleStrategy::LeakyBucket { leak_rate } => {
				// Leaky bucket is similar to token bucket but with constant processing
				self.can_emit_token_bucket(leak_rate)
			}
		}
	}

	fn enqueue(&mut self, item: T) {
		self.queue.push_back(item);
	}

	fn dequeue(&mut self) -> Option<T> {
		self.queue.pop_front()
	}

	fn queue_len(&self) -> usize {
		self.queue.len()
	}
}

/// Signal throttle for rate-limiting signal emissions
///
/// Wraps a signal and enforces rate limits based on the configured strategy.
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::throttling::{ThrottleConfig, SignalThrottle};
/// use reinhardt_core::signals::{Signal, SignalName};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let signal = Signal::<i32>::new(SignalName::custom("numbers"));
/// let config = ThrottleConfig::new()
///     .with_max_emissions(10)
///     .with_window_size(Duration::from_secs(1));
///
/// let throttle = SignalThrottle::new(signal, config);
///
/// // Send with throttling
/// throttle.send(42).await?;
/// # Ok(())
/// # }
/// ```
pub struct SignalThrottle<T: Send + Sync + 'static> {
	signal: Signal<T>,
	config: ThrottleConfig,
	state: Arc<Mutex<ThrottleState<T>>>,
	dropped_count: Arc<Mutex<u64>>,
}

impl<T: Send + Sync + 'static> SignalThrottle<T> {
	/// Create a new signal throttle
	///
	/// # Arguments
	///
	/// * `signal` - The signal to throttle
	/// * `config` - Throttle configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::throttling::{ThrottleConfig, SignalThrottle};
	/// use reinhardt_core::signals::{Signal, SignalName};
	///
	/// let signal = Signal::<String>::new(SignalName::custom("events"));
	/// let config = ThrottleConfig::new();
	/// let throttle = SignalThrottle::new(signal, config);
	/// ```
	pub fn new(signal: Signal<T>, config: ThrottleConfig) -> Self {
		let throttle = Self {
			signal,
			config: config.clone(),
			state: Arc::new(Mutex::new(ThrottleState::new(config.max_emissions))),
			dropped_count: Arc::new(Mutex::new(0)),
		};

		// Start queue processing task if queueing is enabled
		if !config.drop_on_limit {
			throttle.start_queue_processor();
		}

		throttle
	}

	/// Send a signal with throttling applied
	///
	/// Depending on the configuration, the signal may be:
	/// - Sent immediately if rate limit allows
	/// - Dropped if rate limit is exceeded and `drop_on_limit` is true
	/// - Queued if rate limit is exceeded and `drop_on_limit` is false
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::throttling::{ThrottleConfig, SignalThrottle};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let signal = Signal::<String>::new(SignalName::custom("test"));
	/// # let throttle = SignalThrottle::new(signal, ThrottleConfig::new());
	/// throttle.send("event_data".to_string()).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn send(&self, item: T) -> Result<(), SignalError> {
		let can_emit = {
			let mut state = self.state.lock();
			state.can_emit(&self.config)
		};

		if can_emit {
			self.signal.send(item).await
		} else if self.config.drop_on_limit {
			// Drop the signal and record it
			*self.dropped_count.lock() += 1;
			Ok(())
		} else {
			// Queue the signal for later processing
			self.state.lock().enqueue(item);
			Ok(())
		}
	}

	/// Get the number of signals dropped due to throttling
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::throttling::{ThrottleConfig, SignalThrottle};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # let signal = Signal::<String>::new(SignalName::custom("test"));
	/// # let throttle = SignalThrottle::new(signal, ThrottleConfig::new());
	/// let dropped = throttle.dropped_count();
	/// assert_eq!(dropped, 0);
	/// ```
	pub fn dropped_count(&self) -> u64 {
		*self.dropped_count.lock()
	}

	/// Get the current queue length (only relevant when `drop_on_limit` is false)
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::throttling::{ThrottleConfig, SignalThrottle};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # tokio_test::block_on(async {
	/// # let signal = Signal::<String>::new(SignalName::custom("test"));
	/// # let config = ThrottleConfig::new().with_drop_on_limit(false);
	/// # let throttle = SignalThrottle::new(signal, config);
	/// let queued = throttle.queue_length();
	/// assert_eq!(queued, 0);
	/// # })
	/// ```
	pub fn queue_length(&self) -> usize {
		self.state.lock().queue_len()
	}

	/// Reset throttle statistics
	pub fn reset(&self) {
		*self.dropped_count.lock() = 0;
		let mut state = self.state.lock();
		*state = ThrottleState::new(self.config.max_emissions);
	}

	/// Start background task to process queued signals
	fn start_queue_processor(&self) {
		let state = Arc::clone(&self.state);
		let signal = self.signal.clone();
		let config = self.config.clone();

		tokio::spawn(async move {
			let mut interval = tokio::time::interval(Duration::from_millis(100));
			interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				interval.tick().await;

				loop {
					let (_can_emit, item) = {
						let mut state = state.lock();
						let can_emit = state.can_emit(&config);
						let item = if can_emit { state.dequeue() } else { None };
						(can_emit, item)
					};

					if let Some(item) = item {
						if let Err(e) = signal.send(item).await {
							eprintln!("Failed to send throttled signal: {}", e);
						}
					} else {
						break;
					}
				}
			}
		});
	}
}

impl<T: Send + Sync + 'static> Clone for SignalThrottle<T> {
	fn clone(&self) -> Self {
		Self {
			signal: self.signal.clone(),
			config: self.config.clone(),
			state: Arc::clone(&self.state),
			dropped_count: Arc::clone(&self.dropped_count),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::signals::SignalName;
	use std::sync::atomic::{AtomicUsize, Ordering};

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

	#[test]
	fn test_throttle_config() {
		let config = ThrottleConfig::new()
			.with_strategy(ThrottleStrategy::SlidingWindow)
			.with_max_emissions(50)
			.with_window_size(Duration::from_millis(500))
			.with_drop_on_limit(false);

		assert_eq!(config.strategy(), ThrottleStrategy::SlidingWindow);
		assert_eq!(config.max_emissions(), 50);
		assert_eq!(config.window_size(), Duration::from_millis(500));
		assert!(!config.drop_on_limit());
	}

	#[tokio::test]
	async fn test_fixed_window_throttle() {
		let signal = Signal::<i32>::new(SignalName::custom("test_fixed"));
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(move |_| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let config = ThrottleConfig::new()
			.with_strategy(ThrottleStrategy::FixedWindow)
			.with_max_emissions(5)
			.with_window_size(Duration::from_millis(500));

		let throttle = SignalThrottle::new(signal, config);

		// Send 10 signals, only 5 should go through
		for i in 0..10 {
			throttle.send(i).await.unwrap();
		}

		// Poll until signals are processed
		poll_until(
			Duration::from_millis(100),
			Duration::from_millis(10),
			|| async { counter.load(Ordering::SeqCst) == 5 },
		)
		.await
		.expect("5 signals should be processed within 100ms");

		assert_eq!(counter.load(Ordering::SeqCst), 5);
		assert_eq!(throttle.dropped_count(), 5);
	}

	#[tokio::test]
	async fn test_sliding_window_throttle() {
		let signal = Signal::<i32>::new(SignalName::custom("test_sliding"));
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(move |_| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let config = ThrottleConfig::new()
			.with_strategy(ThrottleStrategy::SlidingWindow)
			.with_max_emissions(3)
			.with_window_size(Duration::from_millis(200));

		let throttle = SignalThrottle::new(signal, config);

		// Send 5 signals rapidly
		for i in 0..5 {
			throttle.send(i).await.unwrap();
		}

		// Poll until initial 3 signals are processed
		poll_until(
			Duration::from_millis(50),
			Duration::from_millis(10),
			|| async { counter.load(Ordering::SeqCst) == 3 },
		)
		.await
		.expect("3 signals should be processed within 50ms");

		// Wait for window to pass (200ms window duration)
		tokio::time::sleep(Duration::from_millis(200)).await;

		// Should be able to send again
		throttle.send(99).await.unwrap();

		// Poll until the new signal is processed
		poll_until(
			Duration::from_millis(50),
			Duration::from_millis(10),
			|| async { counter.load(Ordering::SeqCst) == 4 },
		)
		.await
		.expect("4th signal should be processed within 50ms");
	}

	#[tokio::test]
	async fn test_token_bucket_throttle() {
		let signal = Signal::<i32>::new(SignalName::custom("test_token"));
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(move |_| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let config = ThrottleConfig::new()
			.with_strategy(ThrottleStrategy::TokenBucket { refill_rate: 10 })
			.with_max_emissions(10);

		let throttle = SignalThrottle::new(signal, config);

		// Burst send 10 signals (should all go through using initial tokens)
		for i in 0..10 {
			throttle.send(i).await.unwrap();
		}

		// Poll until all 10 signals are processed
		poll_until(
			Duration::from_millis(50),
			Duration::from_millis(10),
			|| async { counter.load(Ordering::SeqCst) == 10 },
		)
		.await
		.expect("10 signals should be processed within 50ms");

		// Next signal should be dropped (no tokens)
		throttle.send(100).await.unwrap();

		// Poll until dropped count increments
		poll_until(
			Duration::from_millis(50),
			Duration::from_millis(10),
			|| async { throttle.dropped_count() == 1 },
		)
		.await
		.expect("11th signal should be dropped within 50ms");
	}

	#[tokio::test]
	async fn test_queue_mode() {
		let signal = Signal::<i32>::new(SignalName::custom("test_queue"));
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(move |_| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		let config = ThrottleConfig::new()
			.with_strategy(ThrottleStrategy::FixedWindow)
			.with_max_emissions(5)
			.with_window_size(Duration::from_millis(300))
			.with_drop_on_limit(false); // Enable queueing

		let throttle = SignalThrottle::new(signal, config);

		// Send 10 signals
		for i in 0..10 {
			throttle.send(i).await.unwrap();
		}

		// Poll until initial 5 signals are processed
		poll_until(
			Duration::from_millis(100),
			Duration::from_millis(10),
			|| async { counter.load(Ordering::SeqCst) == 5 },
		)
		.await
		.expect("5 signals should be processed immediately within 100ms");

		// 5 should go through immediately, 5 should be queued
		assert_eq!(counter.load(Ordering::SeqCst), 5);
		assert!(throttle.queue_length() > 0);

		// Poll until queue is processed (window is 300ms)
		poll_until(
			Duration::from_millis(500),
			Duration::from_millis(20),
			|| async { counter.load(Ordering::SeqCst) >= 9 },
		)
		.await
		.expect("Queue should be processed within 500ms");

		// Eventually all should be processed
		assert!(counter.load(Ordering::SeqCst) >= 9);
		assert_eq!(throttle.dropped_count(), 0);
	}
}
