#![cfg(not(target_arch = "wasm32"))]

//! Signal replay system for replaying past signals for debugging and testing
//!
//! This module provides functionality to replay previously stored signals,
//! useful for debugging, testing, and event sourcing scenarios.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::replay::{SignalReplayer, ReplayConfig, ReplaySpeed};
//! use reinhardt_core::signals::persistence::{MemoryStore, PersistentSignal};
//! use reinhardt_core::signals::{Signal, SignalName};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Serialize, Deserialize)]
//! struct Event {
//!     id: i32,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let signal = Signal::<Event>::new(SignalName::custom("events"));
//! let store = MemoryStore::new();
//! let persistent = PersistentSignal::new(signal.clone(), store.clone());
//!
//! // Record some events
//! for i in 0..10 {
//!     persistent.send(Event { id: i }).await?;
//! }
//!
//! // Replay them
//! let config = ReplayConfig::new().with_speed(ReplaySpeed::Fast);
//! let replayer = SignalReplayer::new(store, signal);
//!
//! replayer.replay_all(config).await?;
//! # Ok(())
//! # }
//! ```

use super::error::SignalError;
use super::persistence::{SignalStore, StoredSignal};
use super::signal::Signal;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

/// Replay speed options
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::replay::ReplaySpeed;
///
/// let instant = ReplaySpeed::Instant;
/// let realtime = ReplaySpeed::Realtime;
/// let fast = ReplaySpeed::Fast;
/// let custom = ReplaySpeed::Custom { multiplier: 2.0 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReplaySpeed {
	/// Replay signals as fast as possible (no delays)
	Instant,

	/// Replay signals at their original timing
	Realtime,

	/// Replay at 10x speed
	Fast,

	/// Custom speed multiplier (e.g., 2.0 = 2x speed, 0.5 = half speed)
	Custom { multiplier: f64 },
}

impl ReplaySpeed {
	/// Calculate delay based on original time difference
	fn calculate_delay(&self, original_delay: Duration) -> Duration {
		match self {
			ReplaySpeed::Instant => Duration::from_millis(0),
			ReplaySpeed::Realtime => original_delay,
			ReplaySpeed::Fast => original_delay / 10,
			ReplaySpeed::Custom { multiplier } => {
				if *multiplier <= 0.0 {
					Duration::from_millis(0)
				} else {
					Duration::from_secs_f64(original_delay.as_secs_f64() / multiplier)
				}
			}
		}
	}
}

/// Configuration for signal replay
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::replay::{ReplayConfig, ReplaySpeed};
///
/// let config = ReplayConfig::new()
///     .with_speed(ReplaySpeed::Fast)
///     .with_limit(100)
///     .with_offset(10);
///
/// assert_eq!(config.limit(), Some(100));
/// assert_eq!(config.offset(), 10);
/// ```
#[derive(Debug, Clone)]
pub struct ReplayConfig {
	/// Replay speed
	speed: ReplaySpeed,
	/// Maximum number of signals to replay (None = all)
	limit: Option<usize>,
	/// Number of signals to skip before starting replay
	offset: usize,
	/// Whether to stop on error or continue
	stop_on_error: bool,
}

impl ReplayConfig {
	/// Create a new replay configuration with default values
	///
	/// Defaults:
	/// - `speed`: Instant
	/// - `limit`: None (all signals)
	/// - `offset`: 0
	/// - `stop_on_error`: false
	pub fn new() -> Self {
		Self {
			speed: ReplaySpeed::Instant,
			limit: None,
			offset: 0,
			stop_on_error: false,
		}
	}

	/// Set the replay speed
	pub fn with_speed(mut self, speed: ReplaySpeed) -> Self {
		self.speed = speed;
		self
	}

	/// Set the maximum number of signals to replay
	pub fn with_limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}

	/// Set the number of signals to skip before replaying
	pub fn with_offset(mut self, offset: usize) -> Self {
		self.offset = offset;
		self
	}

	/// Set whether to stop on error
	pub fn with_stop_on_error(mut self, stop: bool) -> Self {
		self.stop_on_error = stop;
		self
	}

	/// Get the replay speed
	pub fn speed(&self) -> ReplaySpeed {
		self.speed
	}

	/// Get the limit
	pub fn limit(&self) -> Option<usize> {
		self.limit
	}

	/// Get the offset
	pub fn offset(&self) -> usize {
		self.offset
	}

	/// Check if replay should stop on error
	pub fn stop_on_error(&self) -> bool {
		self.stop_on_error
	}
}

impl Default for ReplayConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Statistics about a replay operation
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::replay::ReplayStats;
///
/// let stats = ReplayStats::new();
/// assert_eq!(stats.total_replayed(), 0);
/// assert_eq!(stats.errors(), 0);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ReplayStats {
	/// Total number of signals replayed
	total_replayed: usize,
	/// Number of signals that failed to replay
	errors: usize,
	/// Number of signals skipped
	skipped: usize,
}

impl ReplayStats {
	/// Create new replay statistics
	pub fn new() -> Self {
		Self::default()
	}

	/// Get the total number of signals replayed
	pub fn total_replayed(&self) -> usize {
		self.total_replayed
	}

	/// Get the number of errors
	pub fn errors(&self) -> usize {
		self.errors
	}

	/// Get the number of skipped signals
	pub fn skipped(&self) -> usize {
		self.skipped
	}

	/// Get the success rate as a percentage
	pub fn success_rate(&self) -> f64 {
		if self.total_replayed == 0 {
			return 100.0;
		}
		let successful = self.total_replayed - self.errors;
		(successful as f64 / self.total_replayed as f64) * 100.0
	}
}

/// Signal replayer for replaying stored signals
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::replay::{SignalReplayer, ReplayConfig};
/// use reinhardt_core::signals::persistence::MemoryStore;
/// use reinhardt_core::signals::{Signal, SignalName};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct Event {
///     id: i32,
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let store = MemoryStore::<Event>::new();
/// let signal = Signal::<Event>::new(SignalName::custom("events"));
/// let replayer = SignalReplayer::new(store, signal);
///
/// let stats = replayer.replay_all(ReplayConfig::new()).await?;
/// println!("Replayed {} signals", stats.total_replayed());
/// # Ok(())
/// # }
/// ```
pub struct SignalReplayer<T: Send + Sync + 'static> {
	store: Arc<dyn SignalStore<T>>,
	signal: Signal<T>,
}

impl<T: Send + Sync + Clone + 'static> SignalReplayer<T> {
	/// Create a new signal replayer
	///
	/// # Arguments
	///
	/// * `store` - The storage backend containing stored signals
	/// * `signal` - The signal to replay signals to
	pub fn new<S>(store: S, signal: Signal<T>) -> Self
	where
		S: SignalStore<T> + 'static,
	{
		Self {
			store: Arc::new(store),
			signal,
		}
	}

	/// Replay all stored signals
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::replay::{SignalReplayer, ReplayConfig};
	/// # use reinhardt_core::signals::persistence::MemoryStore;
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let store = MemoryStore::<Event>::new();
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let replayer = SignalReplayer::new(store, signal);
	/// let stats = replayer.replay_all(ReplayConfig::new()).await?;
	/// println!("Success rate: {:.2}%", stats.success_rate());
	/// # Ok(())
	/// # }
	/// ```
	pub async fn replay_all(&self, config: ReplayConfig) -> Result<ReplayStats, SignalError> {
		let limit = config.limit.unwrap_or(usize::MAX);
		let signals = self.store.list(limit, config.offset).await?;

		self.replay_signals(signals, config).await
	}

	/// Replay signals within a time range
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::replay::{SignalReplayer, ReplayConfig};
	/// # use reinhardt_core::signals::persistence::MemoryStore;
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # use std::time::{SystemTime, Duration};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let store = MemoryStore::<Event>::new();
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let replayer = SignalReplayer::new(store, signal);
	/// let start = SystemTime::now() - Duration::from_secs(3600);
	/// let end = SystemTime::now();
	///
	/// let stats = replayer.replay_range(start, end, ReplayConfig::new()).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn replay_range(
		&self,
		start: SystemTime,
		end: SystemTime,
		config: ReplayConfig,
	) -> Result<ReplayStats, SignalError> {
		// Get all signals and filter by time range
		let limit = config.limit.unwrap_or(usize::MAX);
		let all_signals = self.store.list(limit, config.offset).await?;

		let filtered_signals: Vec<_> = all_signals
			.into_iter()
			.filter(|s| s.timestamp >= start && s.timestamp <= end)
			.collect();

		self.replay_signals(filtered_signals, config).await
	}

	/// Replay a specific signal by ID
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::replay::{SignalReplayer, ReplayConfig};
	/// # use reinhardt_core::signals::persistence::MemoryStore;
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let store = MemoryStore::<Event>::new();
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let replayer = SignalReplayer::new(store, signal);
	/// replayer.replay_one(42).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn replay_one(&self, id: u64) -> Result<(), SignalError> {
		let signal = self.store.retrieve(id).await?;

		if let Some(signal) = signal {
			self.signal.send(signal.payload).await?;
		}

		Ok(())
	}

	/// Internal method to replay a list of signals
	async fn replay_signals(
		&self,
		signals: Vec<StoredSignal<T>>,
		config: ReplayConfig,
	) -> Result<ReplayStats, SignalError> {
		let mut stats = ReplayStats::new();
		let mut prev_timestamp: Option<SystemTime> = None;

		for stored_signal in signals {
			// Calculate delay based on timestamp difference
			if let Some(prev) = prev_timestamp
				&& let Ok(diff) = stored_signal.timestamp.duration_since(prev)
			{
				let delay = config.speed.calculate_delay(diff);
				if delay > Duration::from_millis(0) {
					sleep(delay).await;
				}
			}

			prev_timestamp = Some(stored_signal.timestamp);

			// Replay the signal
			let result = self.signal.send(stored_signal.payload).await;

			match result {
				Ok(_) => {
					stats.total_replayed += 1;
				}
				Err(e) => {
					stats.errors += 1;
					if config.stop_on_error {
						return Err(e);
					}
				}
			}
		}

		Ok(stats)
	}
}

impl<T: Send + Sync + Clone + 'static> Clone for SignalReplayer<T> {
	fn clone(&self) -> Self {
		Self {
			store: Arc::clone(&self.store),
			signal: self.signal.clone(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::signals::SignalName;
	use crate::signals::persistence::{MemoryStore, PersistentSignal};
	use serde::{Deserialize, Serialize};
	use std::sync::atomic::{AtomicUsize, Ordering};

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestEvent {
		id: i32,
		message: String,
	}

	#[test]
	fn test_replay_config() {
		let config = ReplayConfig::new()
			.with_speed(ReplaySpeed::Fast)
			.with_limit(50)
			.with_offset(10)
			.with_stop_on_error(true);

		assert_eq!(config.speed(), ReplaySpeed::Fast);
		assert_eq!(config.limit(), Some(50));
		assert_eq!(config.offset(), 10);
		assert!(config.stop_on_error());
	}

	#[test]
	fn test_replay_speed_calculations() {
		let original = Duration::from_secs(10);

		assert_eq!(
			ReplaySpeed::Instant.calculate_delay(original),
			Duration::from_millis(0)
		);
		assert_eq!(ReplaySpeed::Realtime.calculate_delay(original), original);
		assert_eq!(
			ReplaySpeed::Fast.calculate_delay(original),
			Duration::from_secs(1)
		);

		let custom = ReplaySpeed::Custom { multiplier: 2.0 };
		assert_eq!(custom.calculate_delay(original), Duration::from_secs(5));
	}

	#[tokio::test]
	async fn test_replay_stats() {
		let mut stats = ReplayStats::new();
		assert_eq!(stats.total_replayed(), 0);
		assert_eq!(stats.errors(), 0);
		assert_eq!(stats.success_rate(), 100.0);

		stats.total_replayed = 10;
		stats.errors = 2;
		assert_eq!(stats.success_rate(), 80.0);
	}

	#[tokio::test]
	async fn test_signal_replay_basic() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_replay"));
		let store = MemoryStore::new();
		let persistent = PersistentSignal::new(signal.clone(), store.clone());

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		signal.connect(move |_event| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		// Record events
		for i in 0..5 {
			persistent
				.send(TestEvent {
					id: i,
					message: format!("Event {}", i),
				})
				.await
				.unwrap();
		}

		// Wait for processing

		// Reset counter
		counter.store(0, Ordering::SeqCst);

		// Replay
		let replayer = SignalReplayer::new(store, signal);
		let stats = replayer.replay_all(ReplayConfig::new()).await.unwrap();

		// Wait for replay processing

		assert_eq!(stats.total_replayed(), 5);
		assert_eq!(stats.errors(), 0);
		assert_eq!(counter.load(Ordering::SeqCst), 5);
	}

	#[tokio::test]
	async fn test_replay_with_limit() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_limit"));
		let store = MemoryStore::new();
		let persistent = PersistentSignal::new(signal.clone(), store.clone());

		// Record 10 events
		for i in 0..10 {
			persistent
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await
				.unwrap();
		}

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		signal.connect(move |_event| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		// Replay only 5
		let replayer = SignalReplayer::new(store, signal);
		let config = ReplayConfig::new().with_limit(5);
		let stats = replayer.replay_all(config).await.unwrap();

		assert_eq!(stats.total_replayed(), 5);
		assert_eq!(counter.load(Ordering::SeqCst), 5);
	}

	#[tokio::test]
	async fn test_replay_with_offset() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_offset"));
		let store = MemoryStore::new();
		let persistent = PersistentSignal::new(signal.clone(), store.clone());

		// Record 10 events
		for i in 0..10 {
			persistent
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await
				.unwrap();
		}

		let replayed_ids = Arc::new(parking_lot::Mutex::new(Vec::new()));
		let replayed_clone = Arc::clone(&replayed_ids);

		signal.connect(move |event| {
			let replayed = Arc::clone(&replayed_clone);
			async move {
				replayed.lock().push(event.id);
				Ok(())
			}
		});

		// Skip first 5, replay remaining 5
		let replayer = SignalReplayer::new(store, signal);
		let config = ReplayConfig::new().with_offset(5);
		let stats = replayer.replay_all(config).await.unwrap();

		assert_eq!(stats.total_replayed(), 5);

		let ids = replayed_ids.lock();
		assert_eq!(ids.len(), 5);
		// Should have replayed IDs 5-9
		assert!(ids.contains(&5));
		assert!(ids.contains(&9));
		assert!(!ids.contains(&4));
	}

	#[tokio::test]
	async fn test_replay_one() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_one"));
		let store = MemoryStore::new();
		let persistent = PersistentSignal::new(signal.clone(), store.clone());

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		signal.connect(move |_event| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		// Record events
		for i in 0..5 {
			persistent
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await
				.unwrap();
		}

		counter.store(0, Ordering::SeqCst);

		// Replay just one specific signal
		let replayer = SignalReplayer::new(store, signal);
		replayer.replay_one(3).await.unwrap();

		assert_eq!(counter.load(Ordering::SeqCst), 1);
	}
}
