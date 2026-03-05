//! Signal history tracking system for monitoring signal emission patterns
//!
//! This module provides functionality to track signal emission history with timestamps,
//! enabling analysis, debugging, and monitoring of signal patterns.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
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
//!
//! let config = HistoryConfig::new().with_max_entries(1000);
//! let history = SignalHistory::new(signal.clone(), config);
//!
//! history.send(Event { id: 42 }).await?;
//!
//! // Query history
//! let entries = history.get_all();
//! println!("Recorded {} events", entries.len());
//! # Ok(())
//! # }
//! ```

use super::error::SignalError;
use super::signal::Signal;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// A single history entry recording a signal emission
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::history::HistoryEntry;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Clone, Serialize, Deserialize)]
/// struct Event {
///     id: i32,
/// }
///
/// let entry = HistoryEntry::new(Event { id: 42 }, true);
/// assert!(entry.success());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry<T> {
	/// Timestamp when the signal was emitted
	pub timestamp: SystemTime,
	/// The signal payload
	pub payload: T,
	/// Whether the signal was successfully processed
	pub success: bool,
	/// Error message if the signal failed
	pub error_message: Option<String>,
	/// Number of receivers that processed this signal
	pub receiver_count: usize,
}

impl<T> HistoryEntry<T> {
	/// Create a new history entry
	pub fn new(payload: T, success: bool) -> Self {
		Self {
			timestamp: SystemTime::now(),
			payload,
			success,
			error_message: None,
			receiver_count: 0,
		}
	}

	/// Create a new history entry with error
	pub fn with_error(payload: T, error: String) -> Self {
		Self {
			timestamp: SystemTime::now(),
			payload,
			success: false,
			error_message: Some(error),
			receiver_count: 0,
		}
	}

	/// Create a new history entry with full details
	pub fn with_details(
		payload: T,
		success: bool,
		error_message: Option<String>,
		receiver_count: usize,
	) -> Self {
		Self {
			timestamp: SystemTime::now(),
			payload,
			success,
			error_message,
			receiver_count,
		}
	}

	/// Check if the signal was successful
	pub fn success(&self) -> bool {
		self.success
	}

	/// Get the error message if any
	pub fn error(&self) -> Option<&str> {
		self.error_message.as_deref()
	}

	/// Get the age of this entry
	pub fn age(&self) -> Duration {
		SystemTime::now()
			.duration_since(self.timestamp)
			.unwrap_or(Duration::from_secs(0))
	}
}

/// Configuration for signal history tracking
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::history::HistoryConfig;
/// use std::time::Duration;
///
/// let config = HistoryConfig::new()
///     .with_max_entries(500)
///     .with_ttl(Duration::from_secs(3600));
///
/// assert_eq!(config.max_entries(), 500);
/// assert_eq!(config.ttl(), Some(Duration::from_secs(3600)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryConfig {
	/// Maximum number of history entries to keep
	max_entries: usize,
	/// Time-to-live for history entries (None = unlimited)
	ttl: Option<Duration>,
	/// Whether to track errors only
	errors_only: bool,
}

impl HistoryConfig {
	/// Create a new history configuration with default values
	///
	/// Defaults:
	/// - `max_entries`: 1000
	/// - `ttl`: None (unlimited)
	/// - `errors_only`: false
	pub fn new() -> Self {
		Self {
			max_entries: 1000,
			ttl: None,
			errors_only: false,
		}
	}

	/// Set the maximum number of entries to keep
	pub fn with_max_entries(mut self, max: usize) -> Self {
		self.max_entries = max;
		self
	}

	/// Set the time-to-live for entries
	pub fn with_ttl(mut self, ttl: Duration) -> Self {
		self.ttl = Some(ttl);
		self
	}

	/// Track only errors
	pub fn with_errors_only(mut self, errors_only: bool) -> Self {
		self.errors_only = errors_only;
		self
	}

	/// Get the maximum entries
	pub fn max_entries(&self) -> usize {
		self.max_entries
	}

	/// Get the TTL
	pub fn ttl(&self) -> Option<Duration> {
		self.ttl
	}

	/// Check if tracking errors only
	pub fn errors_only(&self) -> bool {
		self.errors_only
	}
}

impl Default for HistoryConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Statistics about signal history
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::history::HistoryStats;
///
/// let stats = HistoryStats::new();
/// assert_eq!(stats.total_count(), 0);
/// assert_eq!(stats.success_rate(), 100.0);
/// ```
#[derive(Debug, Clone, Default)]
pub struct HistoryStats {
	/// Total number of signals in history
	total_count: usize,
	/// Number of successful signals
	success_count: usize,
	/// Number of failed signals
	error_count: usize,
	/// Oldest entry timestamp
	oldest_timestamp: Option<SystemTime>,
	/// Newest entry timestamp
	newest_timestamp: Option<SystemTime>,
}

impl HistoryStats {
	/// Create new history statistics
	pub fn new() -> Self {
		Self::default()
	}

	/// Get the total count
	pub fn total_count(&self) -> usize {
		self.total_count
	}

	/// Get the success count
	pub fn success_count(&self) -> usize {
		self.success_count
	}

	/// Get the error count
	pub fn error_count(&self) -> usize {
		self.error_count
	}

	/// Get the success rate as a percentage
	pub fn success_rate(&self) -> f64 {
		if self.total_count == 0 {
			return 100.0;
		}
		(self.success_count as f64 / self.total_count as f64) * 100.0
	}

	/// Get the timespan of the history
	pub fn timespan(&self) -> Option<Duration> {
		match (self.oldest_timestamp, self.newest_timestamp) {
			(Some(oldest), Some(newest)) => newest.duration_since(oldest).ok(),
			_ => None,
		}
	}
}

/// Signal history tracker
///
/// Wraps a signal and tracks all emissions with timestamps and results.
///
/// # Examples
///
/// ```
/// use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
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
/// let config = HistoryConfig::new();
/// let history = SignalHistory::new(signal, config);
///
/// history.send(Event { id: 1 }).await?;
/// history.send(Event { id: 2 }).await?;
///
/// let stats = history.stats();
/// assert_eq!(stats.total_count(), 2);
/// # Ok(())
/// # }
/// ```
pub struct SignalHistory<T: Send + Sync + 'static> {
	signal: Signal<T>,
	config: HistoryConfig,
	entries: Arc<RwLock<VecDeque<HistoryEntry<T>>>>,
}

impl<T: Send + Sync + Clone + 'static> SignalHistory<T> {
	/// Create a new signal history tracker
	///
	/// # Arguments
	///
	/// * `signal` - The signal to track
	/// * `config` - History configuration
	pub fn new(signal: Signal<T>, config: HistoryConfig) -> Self {
		Self {
			signal,
			config,
			entries: Arc::new(RwLock::new(VecDeque::new())),
		}
	}

	/// Send a signal and record it in history
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let history = SignalHistory::new(signal, HistoryConfig::new());
	/// history.send(Event { id: 42 }).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn send(&self, instance: T) -> Result<(), SignalError> {
		let payload = instance.clone();
		let receiver_count = self.signal.receiver_count();

		// Send the signal
		let result = self.signal.send(instance).await;

		// Record in history
		let success = result.is_ok();
		let error_message = result.as_ref().err().map(|e| e.message.clone());

		// Only record if not errors_only or if it's an error
		if !self.config.errors_only || !success {
			let entry = HistoryEntry::with_details(payload, success, error_message, receiver_count);
			self.add_entry(entry);
		}

		result
	}

	/// Add an entry to the history
	fn add_entry(&self, entry: HistoryEntry<T>) {
		let mut entries = self.entries.write();

		// Remove expired entries
		if let Some(ttl) = self.config.ttl {
			let cutoff = SystemTime::now() - ttl;
			entries.retain(|e| e.timestamp >= cutoff);
		}

		// Evict oldest if at capacity
		if entries.len() >= self.config.max_entries {
			entries.pop_front();
		}

		entries.push_back(entry);
	}

	/// Get all history entries
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let history = SignalHistory::new(signal, HistoryConfig::new());
	/// let entries = history.get_all();
	/// println!("History contains {} entries", entries.len());
	/// ```
	pub fn get_all(&self) -> Vec<HistoryEntry<T>> {
		self.entries.read().iter().cloned().collect()
	}

	/// Get recent history entries
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let history = SignalHistory::new(signal, HistoryConfig::new());
	/// let recent = history.get_recent(10);
	/// println!("Last 10 entries: {:?}", recent.len());
	/// ```
	pub fn get_recent(&self, count: usize) -> Vec<HistoryEntry<T>> {
		let entries = self.entries.read();
		entries.iter().rev().take(count).cloned().collect()
	}

	/// Get error entries only
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let history = SignalHistory::new(signal, HistoryConfig::new());
	/// let errors = history.get_errors();
	/// println!("Found {} errors", errors.len());
	/// ```
	pub fn get_errors(&self) -> Vec<HistoryEntry<T>> {
		self.entries
			.read()
			.iter()
			.filter(|e| !e.success)
			.cloned()
			.collect()
	}

	/// Get entries within a time range
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # use std::time::{SystemTime, Duration};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let history = SignalHistory::new(signal, HistoryConfig::new());
	/// let start = SystemTime::now() - Duration::from_secs(3600);
	/// let end = SystemTime::now();
	/// let entries = history.get_range(start, end);
	/// ```
	pub fn get_range(&self, start: SystemTime, end: SystemTime) -> Vec<HistoryEntry<T>> {
		self.entries
			.read()
			.iter()
			.filter(|e| e.timestamp >= start && e.timestamp <= end)
			.cloned()
			.collect()
	}

	/// Get history statistics
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let history = SignalHistory::new(signal, HistoryConfig::new());
	/// let stats = history.stats();
	/// println!("Success rate: {:.2}%", stats.success_rate());
	/// ```
	pub fn stats(&self) -> HistoryStats {
		let entries = self.entries.read();

		let total_count = entries.len();
		let success_count = entries.iter().filter(|e| e.success).count();
		let error_count = total_count - success_count;

		let oldest_timestamp = entries.front().map(|e| e.timestamp);
		let newest_timestamp = entries.back().map(|e| e.timestamp);

		HistoryStats {
			total_count,
			success_count,
			error_count,
			oldest_timestamp,
			newest_timestamp,
		}
	}

	/// Clear all history
	///
	/// # Examples
	///
	/// ```
	/// # use reinhardt_core::signals::history::{SignalHistory, HistoryConfig};
	/// # use reinhardt_core::signals::{Signal, SignalName};
	/// # use serde::{Deserialize, Serialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct Event { id: i32 }
	/// # let signal = Signal::<Event>::new(SignalName::custom("events"));
	/// # let history = SignalHistory::new(signal, HistoryConfig::new());
	/// history.clear();
	/// assert_eq!(history.stats().total_count(), 0);
	/// ```
	pub fn clear(&self) {
		self.entries.write().clear();
	}

	/// Get access to the underlying signal
	pub fn signal(&self) -> &Signal<T> {
		&self.signal
	}
}

impl<T: Send + Sync + Clone + 'static> Clone for SignalHistory<T> {
	fn clone(&self) -> Self {
		Self {
			signal: self.signal.clone(),
			config: self.config.clone(),
			entries: Arc::clone(&self.entries),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::signals::SignalName;
	use std::sync::atomic::{AtomicUsize, Ordering};

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestEvent {
		id: i32,
		message: String,
	}

	#[test]
	fn test_history_config() {
		let config = HistoryConfig::new()
			.with_max_entries(500)
			.with_ttl(Duration::from_secs(3600))
			.with_errors_only(true);

		assert_eq!(config.max_entries(), 500);
		assert_eq!(config.ttl(), Some(Duration::from_secs(3600)));
		assert!(config.errors_only());
	}

	#[test]
	fn test_history_entry() {
		let entry = HistoryEntry::new(
			TestEvent {
				id: 1,
				message: "test".to_string(),
			},
			true,
		);

		assert!(entry.success());
		assert!(entry.error().is_none());
		assert!(entry.age() < Duration::from_secs(1));
	}

	#[tokio::test]
	async fn test_signal_history_basic() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_history"));
		let config = HistoryConfig::new();
		let history = SignalHistory::new(signal.clone(), config);

		let counter = Arc::new(AtomicUsize::new(0));
		let counter_clone = Arc::clone(&counter);

		signal.connect(move |_event| {
			let counter = Arc::clone(&counter_clone);
			async move {
				counter.fetch_add(1, Ordering::SeqCst);
				Ok(())
			}
		});

		// Send some events
		for i in 0..5 {
			history
				.send(TestEvent {
					id: i,
					message: format!("Event {}", i),
				})
				.await
				.unwrap();
		}

		// Check history
		let stats = history.stats();
		assert_eq!(stats.total_count(), 5);
		assert_eq!(stats.success_count(), 5);
		assert_eq!(stats.error_count(), 0);
		assert_eq!(stats.success_rate(), 100.0);

		// Check counter
		assert_eq!(counter.load(Ordering::SeqCst), 5);
	}

	#[tokio::test]
	async fn test_history_max_entries() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_max"));
		let config = HistoryConfig::new().with_max_entries(3);
		let history = SignalHistory::new(signal, config);

		// Send 5 events
		for i in 0..5 {
			history
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await
				.unwrap();
		}

		// Should only keep last 3
		let all = history.get_all();
		assert_eq!(all.len(), 3);
		assert_eq!(all[0].payload.id, 2);
		assert_eq!(all[1].payload.id, 3);
		assert_eq!(all[2].payload.id, 4);
	}

	#[tokio::test]
	async fn test_history_errors_only() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_errors"));
		let config = HistoryConfig::new().with_errors_only(true);
		let history = SignalHistory::new(signal.clone(), config);

		// Connect a receiver that always fails
		signal.connect(|_event| async move { Err(SignalError::new("Test error")) });

		// Send some events (all will fail)
		for i in 0..3 {
			let _ = history
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await;
		}

		// All should be recorded (errors_only mode)
		let stats = history.stats();
		assert_eq!(stats.total_count(), 3);
		assert_eq!(stats.error_count(), 3);
	}

	#[tokio::test]
	async fn test_history_get_recent() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_recent"));
		let history = SignalHistory::new(signal, HistoryConfig::new());

		for i in 0..10 {
			history
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await
				.unwrap();
		}

		let recent = history.get_recent(3);
		assert_eq!(recent.len(), 3);

		// Most recent first
		assert_eq!(recent[0].payload.id, 9);
		assert_eq!(recent[1].payload.id, 8);
		assert_eq!(recent[2].payload.id, 7);
	}

	#[tokio::test]
	async fn test_history_get_errors() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_get_errors"));
		let history = SignalHistory::new(signal.clone(), HistoryConfig::new());

		let fail_on_odd = Arc::new(AtomicUsize::new(0));
		let fail_clone = Arc::clone(&fail_on_odd);

		signal.connect(move |event| {
			let fail = Arc::clone(&fail_clone);
			async move {
				if event.id % 2 == 1 {
					fail.fetch_add(1, Ordering::SeqCst);
					Err(SignalError::new("Odd number"))
				} else {
					Ok(())
				}
			}
		});

		// Send 10 events (odd ones will fail)
		for i in 0..10 {
			let _ = history
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await;
		}

		let errors = history.get_errors();
		assert_eq!(errors.len(), 5); // 1, 3, 5, 7, 9
		assert!(errors.iter().all(|e| !e.success));
	}

	#[tokio::test]
	async fn test_history_clear() {
		let signal = Signal::<TestEvent>::new(SignalName::custom("test_clear"));
		let history = SignalHistory::new(signal, HistoryConfig::new());

		for i in 0..5 {
			history
				.send(TestEvent {
					id: i,
					message: "test".to_string(),
				})
				.await
				.unwrap();
		}

		assert_eq!(history.stats().total_count(), 5);

		history.clear();
		assert_eq!(history.stats().total_count(), 0);
	}
}
