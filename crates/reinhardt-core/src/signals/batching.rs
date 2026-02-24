#![cfg(not(target_arch = "wasm32"))]

//! Signal batching system for aggregating multiple signals into single dispatches
//!
//! This module provides functionality to batch multiple signal emissions into a single
//! dispatch operation, reducing overhead and improving performance for high-frequency signals.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::batching::{BatchConfig, SignalBatcher};
//! use reinhardt_core::signals::Signal;
//! use std::time::Duration;
//!
//! # tokio_test::block_on(async {
//! // Create a signal (SignalBatcher requires Signal<Vec<T>>)
//! let signal = Signal::<Vec<String>>::new(reinhardt_core::signals::SignalName::custom("user_activity"));
//!
//! // Create a batcher with custom configuration
//! let config = BatchConfig::new()
//!     .with_max_batch_size(100)
//!     .with_flush_interval(Duration::from_millis(500));
//!
//! let batcher = SignalBatcher::new(signal.clone(), config);
//!
//! // Queue signals for batching
//! batcher.queue("user_1_action".to_string()).await.unwrap();
//! batcher.queue("user_2_action".to_string()).await.unwrap();
//! batcher.queue("user_3_action".to_string()).await.unwrap();
//!
//! // Batch will be automatically flushed based on config
//! // Or manually flush
//! batcher.flush().await.unwrap();
//! # })
//! ```

use super::error::SignalError;
use super::signal::Signal;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;
use tokio::time::interval;

/// Configuration for signal batching behavior
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::batching::BatchConfig;
/// use std::time::Duration;
///
/// let config = BatchConfig::new()
///     .with_max_batch_size(50)
///     .with_flush_interval(Duration::from_millis(100));
///
/// assert_eq!(config.max_batch_size(), 50);
/// assert_eq!(config.flush_interval(), Duration::from_millis(100));
/// ```
#[derive(Debug, Clone)]
pub struct BatchConfig {
	/// Maximum number of signals to batch before automatic flush
	max_batch_size: usize,
	/// Time interval for automatic batch flushing
	flush_interval: Duration,
}

impl BatchConfig {
	/// Create a new batch configuration with default values
	///
	/// Defaults:
	/// - `max_batch_size`: 50
	/// - `flush_interval`: 1 second
	pub fn new() -> Self {
		Self {
			max_batch_size: 50,
			flush_interval: Duration::from_secs(1),
		}
	}

	/// Set the maximum batch size
	///
	/// When this many signals are queued, the batch will be flushed automatically.
	pub fn with_max_batch_size(mut self, size: usize) -> Self {
		self.max_batch_size = size;
		self
	}

	/// Set the flush interval
	///
	/// Batches will be automatically flushed at this interval, regardless of size.
	pub fn with_flush_interval(mut self, interval: Duration) -> Self {
		self.flush_interval = interval;
		self
	}

	/// Get the maximum batch size
	pub fn max_batch_size(&self) -> usize {
		self.max_batch_size
	}

	/// Get the flush interval
	pub fn flush_interval(&self) -> Duration {
		self.flush_interval
	}
}

impl Default for BatchConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Internal batch state
struct BatchState<T> {
	items: Vec<T>,
	last_flush: Instant,
}

impl<T> BatchState<T> {
	fn new() -> Self {
		Self {
			items: Vec::new(),
			last_flush: Instant::now(),
		}
	}

	fn add(&mut self, item: T) {
		self.items.push(item);
	}

	fn should_flush(&self, config: &BatchConfig) -> bool {
		self.items.len() >= config.max_batch_size
			|| self.last_flush.elapsed() >= config.flush_interval
	}

	fn take(&mut self) -> Vec<T> {
		self.last_flush = Instant::now();
		std::mem::take(&mut self.items)
	}

	fn is_empty(&self) -> bool {
		self.items.is_empty()
	}

	fn len(&self) -> usize {
		self.items.len()
	}
}

/// Signal batcher for aggregating multiple signals
///
/// Collects signals and dispatches them in batches based on configured criteria.
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::batching::{BatchConfig, SignalBatcher};
/// use reinhardt_core::signals::{Signal, SignalName};
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let signal = Signal::<Vec<i32>>::new(SignalName::custom("numbers"));
/// let config = BatchConfig::new().with_max_batch_size(10);
/// let batcher = SignalBatcher::new(signal, config);
///
/// // Queue items
/// for i in 0..5 {
///     batcher.queue(i).await.unwrap();
/// }
///
/// // Manual flush
/// batcher.flush().await.unwrap();
/// # })
/// ```
pub struct SignalBatcher<T: Send + Sync + 'static> {
	signal: Signal<Vec<T>>,
	config: BatchConfig,
	state: Arc<Mutex<BatchState<T>>>,
	flush_notify: Arc<Notify>,
}

impl<T: Send + Sync + 'static> SignalBatcher<T> {
	/// Create a new signal batcher
	///
	/// # Arguments
	///
	/// * `signal` - The signal to batch emissions for (expects `Vec<T>`)
	/// * `config` - Batch configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::batching::{BatchConfig, SignalBatcher};
	/// use reinhardt_core::signals::{Signal, SignalName};
	///
	/// # tokio_test::block_on(async {
	/// let signal = Signal::<Vec<String>>::new(SignalName::custom("batch_signal"));
	/// let config = BatchConfig::new();
	/// let batcher = SignalBatcher::new(signal, config);
	/// # })
	/// ```
	pub fn new(signal: Signal<Vec<T>>, config: BatchConfig) -> Self {
		let batcher = Self {
			signal,
			config,
			state: Arc::new(Mutex::new(BatchState::new())),
			flush_notify: Arc::new(Notify::new()),
		};

		// Start background flush task
		batcher.start_auto_flush();

		batcher
	}

	/// Queue a signal for batching
	///
	/// The signal will be added to the current batch and dispatched when:
	/// - The batch size reaches `max_batch_size`
	/// - The `flush_interval` elapses
	/// - `flush()` is called manually
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::batching::{BatchConfig, SignalBatcher};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let signal = Signal::<Vec<String>>::new(SignalName::custom("test"));
	/// # let batcher = SignalBatcher::new(signal, BatchConfig::new());
	/// batcher.queue("event_data".to_string()).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn queue(&self, item: T) -> Result<(), SignalError> {
		let should_flush = {
			let mut state = self.state.lock();
			state.add(item);
			state.should_flush(&self.config)
		};

		if should_flush {
			self.flush().await?;
		}

		Ok(())
	}

	/// Manually flush the current batch
	///
	/// Dispatches all queued signals immediately, regardless of batch size or time interval.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::batching::{BatchConfig, SignalBatcher};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let signal = Signal::<Vec<String>>::new(SignalName::custom("test"));
	/// # let batcher = SignalBatcher::new(signal, BatchConfig::new());
	/// batcher.queue("item1".to_string()).await?;
	/// batcher.queue("item2".to_string()).await?;
	/// batcher.flush().await?; // Force immediate dispatch
	/// # Ok(())
	/// # }
	/// ```
	pub async fn flush(&self) -> Result<(), SignalError> {
		let items = {
			let mut state = self.state.lock();
			if state.is_empty() {
				return Ok(());
			}
			state.take()
		};

		self.signal.send(items).await?;
		self.flush_notify.notify_one();
		Ok(())
	}

	/// Get the current batch size
	///
	/// Returns the number of signals currently queued but not yet dispatched.
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::batching::{BatchConfig, SignalBatcher};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let signal = Signal::<Vec<String>>::new(SignalName::custom("test"));
	/// # let batcher = SignalBatcher::new(signal, BatchConfig::new());
	/// batcher.queue("item".to_string()).await?;
	/// assert_eq!(batcher.current_batch_size(), 1);
	/// # Ok(())
	/// # }
	/// ```
	pub fn current_batch_size(&self) -> usize {
		self.state.lock().len()
	}

	/// Start automatic flush background task
	fn start_auto_flush(&self) {
		let state = Arc::clone(&self.state);
		let signal = self.signal.clone();
		let config = self.config.clone();
		let flush_notify = Arc::clone(&self.flush_notify);

		tokio::spawn(async move {
			let mut ticker = interval(config.flush_interval);
			ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

			loop {
				ticker.tick().await;

				let items = {
					let mut state = state.lock();
					if state.is_empty() {
						continue;
					}
					state.take()
				};

				if signal.send(items).await.is_ok() {
					flush_notify.notify_one();
				}
			}
		});
	}
}

impl<T: Send + Sync + 'static> Clone for SignalBatcher<T> {
	fn clone(&self) -> Self {
		Self {
			signal: self.signal.clone(),
			config: self.config.clone(),
			state: Arc::clone(&self.state),
			flush_notify: Arc::clone(&self.flush_notify),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::signals::SignalName;
	use parking_lot::Mutex as ParkingLotMutex;
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
	fn test_batch_config() {
		let config = BatchConfig::new()
			.with_max_batch_size(100)
			.with_flush_interval(Duration::from_millis(500));

		assert_eq!(config.max_batch_size(), 100);
		assert_eq!(config.flush_interval(), Duration::from_millis(500));
	}

	#[test]
	fn test_batch_config_default() {
		let config = BatchConfig::default();
		assert_eq!(config.max_batch_size(), 50);
		assert_eq!(config.flush_interval(), Duration::from_secs(1));
	}

	#[tokio::test]
	async fn test_signal_batcher_manual_flush() {
		let signal = Signal::<Vec<i32>>::new(SignalName::custom("test_batch"));
		let received = Arc::new(ParkingLotMutex::new(Vec::new()));

		let received_clone = Arc::clone(&received);
		signal.connect(move |batch| {
			let received = Arc::clone(&received_clone);
			async move {
				received.lock().extend(batch.iter().copied());
				Ok(())
			}
		});

		let config = BatchConfig::new().with_max_batch_size(10);
		let batcher = SignalBatcher::new(signal, config);

		// Queue items
		for i in 0..5 {
			batcher.queue(i).await.unwrap();
		}

		assert_eq!(batcher.current_batch_size(), 5);

		// Manual flush
		batcher.flush().await.unwrap();

		// Wait for processing

		let results = received.lock();
		assert_eq!(results.len(), 5);
		assert_eq!(*results, vec![0, 1, 2, 3, 4]);
	}

	#[tokio::test]
	async fn test_signal_batcher_auto_flush_by_size() {
		let signal = Signal::<Vec<i32>>::new(SignalName::custom("test_auto_batch"));
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(move |batch| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(batch.len(), Ordering::SeqCst);
				Ok(())
			}
		});

		let config = BatchConfig::new().with_max_batch_size(5);
		let batcher = SignalBatcher::new(signal, config);

		// Queue exactly max_batch_size items
		for i in 0..5 {
			batcher.queue(i).await.unwrap();
		}

		// Wait for auto flush

		assert_eq!(counter.load(Ordering::SeqCst), 5);
		assert_eq!(batcher.current_batch_size(), 0);
	}

	#[tokio::test]
	async fn test_signal_batcher_auto_flush_by_time() {
		let signal = Signal::<Vec<i32>>::new(SignalName::custom("test_time_batch"));
		let counter = Arc::new(AtomicUsize::new(0));

		let counter_clone = Arc::clone(&counter);
		signal.connect(move |batch| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(batch.len(), Ordering::SeqCst);
				Ok(())
			}
		});

		let config = BatchConfig::new()
			.with_max_batch_size(100)
			.with_flush_interval(Duration::from_millis(200));

		let batcher = SignalBatcher::new(signal, config);

		// Queue just a few items
		for i in 0..3 {
			batcher.queue(i).await.unwrap();
		}

		// Poll until time-based flush completes
		poll_until(
			Duration::from_millis(400),
			Duration::from_millis(20),
			|| async { counter.load(Ordering::SeqCst) == 3 },
		)
		.await
		.expect("Batch should be flushed within 400ms");
	}

	#[tokio::test]
	async fn test_signal_batcher_empty_flush() {
		let signal = Signal::<Vec<i32>>::new(SignalName::custom("test_empty"));
		let config = BatchConfig::new();
		let batcher = SignalBatcher::new(signal, config);

		// Flushing empty batch should not error
		assert!(batcher.flush().await.is_ok());
	}
}
