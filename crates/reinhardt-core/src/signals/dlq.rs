#![cfg(not(target_arch = "wasm32"))]

//! Dead Letter Queue (DLQ) system for handling failed signals with retry logic
//!
//! This module provides functionality to handle failed signal emissions by queuing them
//! for retry with configurable retry strategies and backoff mechanisms.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::dlq::{DeadLetterQueue, DlqConfig, RetryStrategy};
//! use reinhardt_core::signals::{Signal, SignalName};
//! use std::time::Duration;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Serialize, Deserialize)]
//! struct Event {
//!     id: i32,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let signal = Signal::<Event>::new(SignalName::custom("events"));
//!
//! let config = DlqConfig::new()
//!     .with_max_retries(3)
//!     .with_retry_strategy(RetryStrategy::ExponentialBackoff {
//!         initial_delay: Duration::from_millis(100),
//!         max_delay: Duration::from_secs(60),
//!     });
//!
//! let dlq = DeadLetterQueue::new(signal, config);
//!
//! // Failed signals will be automatically retried
//! dlq.send(Event { id: 42 }).await?;
//! # Ok(())
//! # }
//! ```

use super::error::SignalError;
use super::signal::Signal;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Retry strategies for failed signals
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::dlq::RetryStrategy;
/// use std::time::Duration;
///
/// let immediate = RetryStrategy::Immediate;
/// let fixed = RetryStrategy::FixedDelay { delay: Duration::from_secs(1) };
/// let exponential = RetryStrategy::ExponentialBackoff {
///     initial_delay: Duration::from_millis(100),
///     max_delay: Duration::from_secs(60),
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RetryStrategy {
	/// Retry immediately without delay
	Immediate,

	/// Retry with a fixed delay between attempts
	FixedDelay {
		/// Delay between retry attempts
		delay: Duration,
	},

	/// Retry with exponential backoff
	ExponentialBackoff {
		/// Initial delay for first retry
		initial_delay: Duration,
		/// Maximum delay between retries
		max_delay: Duration,
	},

	/// Retry with linear backoff
	LinearBackoff {
		/// Base delay that increases linearly
		base_delay: Duration,
	},
}

impl RetryStrategy {
	/// Calculate the delay for a given retry attempt
	fn calculate_delay(&self, attempt: u32) -> Duration {
		match self {
			RetryStrategy::Immediate => Duration::from_millis(0),
			RetryStrategy::FixedDelay { delay } => *delay,
			RetryStrategy::ExponentialBackoff {
				initial_delay,
				max_delay,
			} => {
				let exp_delay = initial_delay.mul_f64(2_f64.powi(attempt as i32));
				exp_delay.min(*max_delay)
			}
			RetryStrategy::LinearBackoff { base_delay } => base_delay.mul_f32(attempt as f32 + 1.0),
		}
	}
}

/// Configuration for Dead Letter Queue
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::dlq::{DlqConfig, RetryStrategy};
/// use std::time::Duration;
///
/// let config = DlqConfig::new()
///     .with_max_retries(5)
///     .with_retry_strategy(RetryStrategy::ExponentialBackoff {
///         initial_delay: Duration::from_millis(100),
///         max_delay: Duration::from_secs(60),
///     })
///     .with_max_queue_size(1000);
///
/// assert_eq!(config.max_retries(), 5);
/// assert_eq!(config.max_queue_size(), 1000);
/// ```
#[derive(Debug, Clone)]
pub struct DlqConfig {
	/// Maximum number of retry attempts
	max_retries: u32,
	/// Retry strategy to use
	retry_strategy: RetryStrategy,
	/// Maximum size of the DLQ
	max_queue_size: usize,
	/// Whether to persist failed messages
	persist_failed: bool,
}

impl DlqConfig {
	/// Create a new DLQ configuration with default values
	///
	/// Defaults:
	/// - `max_retries`: 3
	/// - `retry_strategy`: ExponentialBackoff (100ms initial, 60s max)
	/// - `max_queue_size`: 10000
	/// - `persist_failed`: false
	pub fn new() -> Self {
		Self {
			max_retries: 3,
			retry_strategy: RetryStrategy::ExponentialBackoff {
				initial_delay: Duration::from_millis(100),
				max_delay: Duration::from_secs(60),
			},
			max_queue_size: 10000,
			persist_failed: false,
		}
	}

	/// Set the maximum number of retries
	pub fn with_max_retries(mut self, max: u32) -> Self {
		self.max_retries = max;
		self
	}

	/// Set the retry strategy
	pub fn with_retry_strategy(mut self, strategy: RetryStrategy) -> Self {
		self.retry_strategy = strategy;
		self
	}

	/// Set the maximum queue size
	pub fn with_max_queue_size(mut self, size: usize) -> Self {
		self.max_queue_size = size;
		self
	}

	/// Set whether to persist failed messages
	pub fn with_persist_failed(mut self, persist: bool) -> Self {
		self.persist_failed = persist;
		self
	}

	/// Get the maximum retries
	pub fn max_retries(&self) -> u32 {
		self.max_retries
	}

	/// Get the retry strategy
	pub fn retry_strategy(&self) -> RetryStrategy {
		self.retry_strategy
	}

	/// Get the maximum queue size
	pub fn max_queue_size(&self) -> usize {
		self.max_queue_size
	}

	/// Check if failed messages should be persisted
	pub fn persist_failed(&self) -> bool {
		self.persist_failed
	}
}

impl Default for DlqConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// A message in the Dead Letter Queue
#[derive(Debug, Clone)]
pub struct DlqMessage<T> {
	/// The message payload
	pub payload: T,
	/// Number of retry attempts made
	pub retry_count: u32,
	/// Timestamp when first queued
	pub first_queued_at: SystemTime,
	/// Timestamp when last attempted
	pub last_attempt_at: SystemTime,
	/// Last error message
	pub last_error: String,
	/// Next retry time
	pub next_retry_at: Instant,
}

impl<T> DlqMessage<T> {
	/// Create a new DLQ message
	fn new(payload: T, error: String, next_retry_at: Instant) -> Self {
		let now = SystemTime::now();
		Self {
			payload,
			retry_count: 0,
			first_queued_at: now,
			last_attempt_at: now,
			last_error: error,
			next_retry_at,
		}
	}

	/// Mark a retry attempt
	fn mark_retry(&mut self, error: String, next_retry_at: Instant) {
		self.retry_count += 1;
		self.last_attempt_at = SystemTime::now();
		self.last_error = error;
		self.next_retry_at = next_retry_at;
	}
}

/// Statistics about the Dead Letter Queue
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::dlq::DlqStats;
///
/// let stats = DlqStats::new();
/// assert_eq!(stats.queue_size(), 0);
/// assert_eq!(stats.total_failed(), 0);
/// ```
#[derive(Debug, Clone, Default)]
pub struct DlqStats {
	/// Current queue size
	queue_size: usize,
	/// Total messages that failed permanently
	total_failed: u64,
	/// Total messages retried successfully
	total_recovered: u64,
	/// Total retry attempts made
	total_retries: u64,
}

impl DlqStats {
	/// Create new DLQ statistics
	pub fn new() -> Self {
		Self::default()
	}

	/// Get the current queue size
	pub fn queue_size(&self) -> usize {
		self.queue_size
	}

	/// Get the total permanently failed messages
	pub fn total_failed(&self) -> u64 {
		self.total_failed
	}

	/// Get the total recovered messages
	pub fn total_recovered(&self) -> u64 {
		self.total_recovered
	}

	/// Get the total retry attempts
	pub fn total_retries(&self) -> u64 {
		self.total_retries
	}

	/// Get the recovery rate as a percentage
	pub fn recovery_rate(&self) -> f64 {
		let total = self.total_recovered + self.total_failed;
		if total == 0 {
			return 100.0;
		}
		(self.total_recovered as f64 / total as f64) * 100.0
	}
}

/// Dead Letter Queue for handling failed signals
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::dlq::{DeadLetterQueue, DlqConfig};
/// use reinhardt_core::signals::{Signal, SignalName};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct Event {
///     id: i32,
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let signal = Signal::<Event>::new(SignalName::custom("events"));
/// let config = DlqConfig::new().with_max_retries(3);
/// let dlq = DeadLetterQueue::new(signal, config);
///
/// // Send will retry on failure
/// dlq.send(Event { id: 42 }).await?;
/// # Ok(())
/// # }
/// ```
pub struct DeadLetterQueue<T: Send + Sync + 'static> {
	signal: Signal<T>,
	config: DlqConfig,
	queue: Arc<Mutex<VecDeque<DlqMessage<T>>>>,
	stats: Arc<Mutex<DlqStats>>,
}

impl<T: Send + Sync + Clone + 'static> DeadLetterQueue<T> {
	/// Create a new Dead Letter Queue
	///
	/// # Arguments
	///
	/// * `signal` - The signal to wrap
	/// * `config` - DLQ configuration
	pub fn new(signal: Signal<T>, config: DlqConfig) -> Self {
		let dlq = Self {
			signal,
			config,
			queue: Arc::new(Mutex::new(VecDeque::new())),
			stats: Arc::new(Mutex::new(DlqStats::new())),
		};

		// Start retry processor
		dlq.start_retry_processor();

		dlq
	}

	/// Send a signal with automatic retry on failure
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::dlq::{DeadLetterQueue, DlqConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let dlq = DeadLetterQueue::new(signal, DlqConfig::new());
	/// dlq.send(Event { id: 1 }).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn send(&self, instance: T) -> Result<(), SignalError> {
		let result = self.signal.send(instance.clone()).await;

		if let Err(e) = result {
			// Queue for retry
			self.enqueue(instance, e.message.clone());
			Ok(()) // Return Ok since it's queued for retry
		} else {
			Ok(())
		}
	}

	/// Enqueue a failed message
	fn enqueue(&self, payload: T, error: String) {
		let mut queue = self.queue.lock();

		// Check queue size limit
		if queue.len() >= self.config.max_queue_size {
			// Drop oldest message
			queue.pop_front();
		}

		let delay = self.config.retry_strategy.calculate_delay(0);
		let next_retry_at = Instant::now() + delay;

		let message = DlqMessage::new(payload, error, next_retry_at);
		queue.push_back(message);

		// Update stats
		self.stats.lock().queue_size = queue.len();
	}

	/// Get current DLQ statistics
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::dlq::{DeadLetterQueue, DlqConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # tokio_test::block_on(async {
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let dlq = DeadLetterQueue::new(signal, DlqConfig::new());
	/// let stats = dlq.stats();
	/// println!("Queue size: {}", stats.queue_size());
	/// println!("Recovery rate: {:.2}%", stats.recovery_rate());
	/// # })
	/// ```
	pub fn stats(&self) -> DlqStats {
		self.stats.lock().clone()
	}

	/// Get all messages in the DLQ
	pub fn get_messages(&self) -> Vec<DlqMessage<T>> {
		self.queue.lock().iter().cloned().collect()
	}

	/// Clear the DLQ
	pub fn clear(&self) {
		self.queue.lock().clear();
		self.stats.lock().queue_size = 0;
	}

	/// Start background task to process retry queue
	fn start_retry_processor(&self) {
		let queue = Arc::clone(&self.queue);
		let signal = self.signal.clone();
		let config = self.config.clone();
		let stats = Arc::clone(&self.stats);

		tokio::spawn(async move {
			let mut interval = tokio::time::interval(Duration::from_millis(100));
			interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				interval.tick().await;

				let now = Instant::now();
				let mut message_to_retry = None;

				// Find a message ready for retry
				{
					let mut queue_guard = queue.lock();
					if let Some(msg) = queue_guard.front()
						&& msg.next_retry_at <= now
					{
						message_to_retry = queue_guard.pop_front();
					}
				}

				if let Some(mut msg) = message_to_retry {
					stats.lock().total_retries += 1;

					let result = signal.send(msg.payload.clone()).await;

					match result {
						Ok(_) => {
							// Success! Message recovered
							let mut stats_guard = stats.lock();
							stats_guard.total_recovered += 1;
							stats_guard.queue_size = queue.lock().len();
						}
						Err(e) => {
							// Failed again
							if msg.retry_count >= config.max_retries {
								// Max retries exceeded, mark as permanently failed
								let mut stats_guard = stats.lock();
								stats_guard.total_failed += 1;
								stats_guard.queue_size = queue.lock().len();
							} else {
								// Re-queue for another retry
								let delay =
									config.retry_strategy.calculate_delay(msg.retry_count + 1);
								let next_retry_at = Instant::now() + delay;

								msg.mark_retry(e.message, next_retry_at);

								queue.lock().push_back(msg);
							}
						}
					}
				}
			}
		});
	}

	/// Get access to the underlying signal
	pub fn signal(&self) -> &Signal<T> {
		&self.signal
	}
}

impl<T: Send + Sync + Clone + 'static> Clone for DeadLetterQueue<T> {
	fn clone(&self) -> Self {
		Self {
			signal: self.signal.clone(),
			config: self.config.clone(),
			queue: Arc::clone(&self.queue),
			stats: Arc::clone(&self.stats),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::signals::SignalName;
	use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

	#[derive(Debug, Clone, PartialEq)]
	struct TestEvent {
		id: i32,
		message: String,
	}

	#[test]
	fn test_dlq_config() {
		let config = DlqConfig::new()
			.with_max_retries(5)
			.with_retry_strategy(RetryStrategy::FixedDelay {
				delay: Duration::from_secs(1),
			})
			.with_max_queue_size(500)
			.with_persist_failed(true);

		assert_eq!(config.max_retries(), 5);
		assert_eq!(
			config.retry_strategy(),
			RetryStrategy::FixedDelay {
				delay: Duration::from_secs(1)
			}
		);
		assert_eq!(config.max_queue_size(), 500);
		assert!(config.persist_failed());
	}

	#[test]
	fn test_retry_strategy_delays() {
		// Immediate
		let immediate = RetryStrategy::Immediate;
		assert_eq!(immediate.calculate_delay(0), Duration::from_millis(0));
		assert_eq!(immediate.calculate_delay(5), Duration::from_millis(0));

		// Fixed delay
		let fixed = RetryStrategy::FixedDelay {
			delay: Duration::from_secs(1),
		};
		assert_eq!(fixed.calculate_delay(0), Duration::from_secs(1));
		assert_eq!(fixed.calculate_delay(10), Duration::from_secs(1));

		// Exponential backoff
		let exponential = RetryStrategy::ExponentialBackoff {
			initial_delay: Duration::from_millis(100),
			max_delay: Duration::from_secs(10),
		};
		assert_eq!(exponential.calculate_delay(0), Duration::from_millis(100));
		assert_eq!(exponential.calculate_delay(1), Duration::from_millis(200));
		assert_eq!(exponential.calculate_delay(2), Duration::from_millis(400));
		// Should cap at max_delay
		assert!(exponential.calculate_delay(10) <= Duration::from_secs(10));
	}

	#[tokio::test]
	async fn test_dlq_basic() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_dlq"));
		let config =
			DlqConfig::new()
				.with_max_retries(2)
				.with_retry_strategy(RetryStrategy::FixedDelay {
					delay: Duration::from_millis(100),
				});

		let dlq = DeadLetterQueue::new(signal.clone(), config);

		let should_fail = Arc::new(AtomicBool::new(true));
		let attempt_count = Arc::new(AtomicUsize::new(0));

		let fail_clone = Arc::clone(&should_fail);
		let count_clone = Arc::clone(&attempt_count);

		signal.connect(move |_event| {
			let fail = Arc::clone(&fail_clone);
			let count = Arc::clone(&count_clone);
			async move {
				count.fetch_add(1, Ordering::SeqCst);
				if fail.load(Ordering::SeqCst) {
					Err(SignalError::new("Test failure"))
				} else {
					Ok(())
				}
			}
		});

		// Send event (will fail initially)
		dlq.send(TestEvent {
			id: 1,
			message: "test".to_string(),
		})
		.await
		.unwrap();

		// Poll until initial attempt completes
		poll_until(
			Duration::from_millis(100),
			Duration::from_millis(10),
			|| async { attempt_count.load(Ordering::SeqCst) >= 1 },
		)
		.await
		.expect("Initial attempt should complete");

		// Make it succeed on retry
		should_fail.store(false, Ordering::SeqCst);

		// Poll until retry completes
		poll_until(
			Duration::from_millis(300),
			Duration::from_millis(20),
			|| async { attempt_count.load(Ordering::SeqCst) >= 2 },
		)
		.await
		.expect("Retry should complete within 300ms");

		let stats = dlq.stats();
		assert_eq!(stats.total_recovered(), 1);
	}

	#[tokio::test]
	async fn test_dlq_max_retries() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_max_retries"));
		let config = DlqConfig::new()
			.with_max_retries(2)
			.with_retry_strategy(RetryStrategy::Immediate);

		let dlq = DeadLetterQueue::new(signal.clone(), config);

		let attempt_count = Arc::new(AtomicUsize::new(0));
		let count_clone = Arc::clone(&attempt_count);

		// Always fail
		signal.connect(move |_event| {
			let count = Arc::clone(&count_clone);
			async move {
				count.fetch_add(1, Ordering::SeqCst);
				Err(SignalError::new("Always fails"))
			}
		});

		dlq.send(TestEvent {
			id: 1,
			message: "test".to_string(),
		})
		.await
		.unwrap();

		// Poll until all retries complete (1 initial + 2 retries = 3 total)
		poll_until(
			Duration::from_secs(1),
			Duration::from_millis(50),
			|| async { attempt_count.load(Ordering::SeqCst) >= 3 },
		)
		.await
		.expect("All retries should complete within 1000ms");

		// Wait for stats to be updated (background task updates stats after final retry)
		poll_until(
			Duration::from_secs(1),
			Duration::from_millis(50),
			|| async {
				let stats = dlq.stats();
				stats.total_failed() == 1
			},
		)
		.await
		.expect("Stats should be updated within 1000ms");

		let stats = dlq.stats();
		assert_eq!(stats.total_failed(), 1);
		assert_eq!(stats.total_recovered(), 0);
	}

	#[tokio::test]
	async fn test_dlq_queue_size_limit() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_queue_limit"));
		let config = DlqConfig::new()
            .with_max_queue_size(3)
            .with_max_retries(10) // High retry count to keep in queue
            .with_retry_strategy(RetryStrategy::FixedDelay {
                delay: Duration::from_secs(100), // Long delay to keep in queue
            });

		let dlq = DeadLetterQueue::new(signal.clone(), config);

		// Always fail
		signal.connect(|_event| async move { Err(SignalError::new("Always fails")) });

		// Send 5 events (should only keep 3)
		for i in 0..5 {
			dlq.send(TestEvent {
				id: i,
				message: "test".to_string(),
			})
			.await
			.unwrap();
		}

		// Queue should be capped at 3
		let stats = dlq.stats();
		assert!(stats.queue_size() <= 3);
	}

	#[tokio::test]
	async fn test_dlq_clear() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_clear"));
		let config = DlqConfig::new().with_retry_strategy(RetryStrategy::FixedDelay {
			delay: Duration::from_secs(100),
		});

		let dlq = DeadLetterQueue::new(signal.clone(), config);

		signal.connect(|_event| async move { Err(SignalError::new("Fail")) });

		for i in 0..3 {
			dlq.send(TestEvent {
				id: i,
				message: "test".to_string(),
			})
			.await
			.unwrap();
		}

		assert!(dlq.stats().queue_size() > 0);

		dlq.clear();
		assert_eq!(dlq.stats().queue_size(), 0);
	}
}
