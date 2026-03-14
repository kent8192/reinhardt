//! Signal debugger for visual debugging of signal flow
//!
//! Provides tools for debugging signal connections, tracking signal flow,
//! and identifying issues in signal systems.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::{Signal, SignalName};
//! use reinhardt_core::signals::debugger::SignalDebugger;
//!
//! # tokio_test::block_on(async {
//! let signal = Signal::<String>::new(SignalName::custom("test_signal"));
//! let debugger = SignalDebugger::new();
//!
//! // Attach debugger to signal
//! signal.add_middleware(debugger.clone());
//!
//! // Connect a receiver
//! signal.connect(|msg| async move {
//!     println!("Received: {}", msg);
//!     Ok(())
//! });
//!
//! // Send signal
//! signal.send("Hello".to_string()).await.unwrap();
//!
//! // Get debug report
//! let report = debugger.generate_report();
//! assert!(report.contains("total_sends: 1"));
//! # });
//! ```

use super::error::SignalError;
use super::middleware::SignalMiddleware;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

/// A single debug event in the signal flow
#[derive(Debug, Clone)]
pub struct DebugEvent {
	/// Event type (before_send, after_send, before_receiver, after_receiver)
	pub event_type: String,
	/// Timestamp when the event occurred
	pub timestamp: SystemTime,
	/// Optional dispatch UID of the receiver
	pub dispatch_uid: Option<String>,
	/// Whether the operation succeeded
	pub success: bool,
	/// Optional error message
	pub error_message: Option<String>,
}

/// Statistics about signal executions
#[derive(Debug, Clone, Default)]
pub struct SignalStats {
	/// Total number of signal sends
	pub total_sends: usize,
	/// Total number of receiver executions
	pub total_receiver_calls: usize,
	/// Total number of successful executions
	pub successful_executions: usize,
	/// Total number of failed executions
	pub failed_executions: usize,
	/// Map of receiver dispatch UIDs to their call counts
	pub receiver_call_counts: HashMap<String, usize>,
}

/// Signal debugger for tracking and visualizing signal flow
///
/// This middleware records all signal events and provides detailed debugging
/// information about signal execution.
pub struct SignalDebugger<T: Send + Sync + 'static> {
	events: Arc<RwLock<Vec<DebugEvent>>>,
	stats: Arc<RwLock<SignalStats>>,
	_phantom: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> SignalDebugger<T> {
	/// Create a new signal debugger
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	///
	/// let debugger = SignalDebugger::<String>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			events: Arc::new(RwLock::new(Vec::new())),
			stats: Arc::new(RwLock::new(SignalStats::default())),
			_phantom: std::marker::PhantomData,
		}
	}

	/// Get all recorded debug events
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	///
	/// # tokio_test::block_on(async {
	/// let debugger = SignalDebugger::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(debugger.clone());
	///
	/// signal.connect(|_| async { Ok(()) });
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let events = debugger.events();
	/// assert!(!events.is_empty());
	/// # });
	/// ```
	pub fn events(&self) -> Vec<DebugEvent> {
		self.events.read().clone()
	}

	/// Get signal execution statistics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	///
	/// # tokio_test::block_on(async {
	/// let debugger = SignalDebugger::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(debugger.clone());
	///
	/// signal.connect(|_| async { Ok(()) });
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let stats = debugger.stats();
	/// assert_eq!(stats.total_sends, 1);
	/// # });
	/// ```
	pub fn stats(&self) -> SignalStats {
		self.stats.read().clone()
	}

	/// Clear all recorded events and statistics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	///
	/// let debugger = SignalDebugger::<String>::new();
	/// debugger.clear();
	/// assert_eq!(debugger.events().len(), 0);
	/// ```
	pub fn clear(&self) {
		self.events.write().clear();
		*self.stats.write() = SignalStats::default();
	}

	/// Generate a human-readable debug report
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	///
	/// # tokio_test::block_on(async {
	/// let debugger = SignalDebugger::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(debugger.clone());
	///
	/// signal.connect(|_| async { Ok(()) });
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let report = debugger.generate_report();
	/// assert!(report.contains("Signal Debug Report"));
	/// # });
	/// ```
	pub fn generate_report(&self) -> String {
		let stats = self.stats.read();
		let events = self.events.read();

		let mut report = String::from("=== Signal Debug Report ===\n\n");

		report.push_str("Statistics:\n");
		report.push_str(&format!("  total_sends: {}\n", stats.total_sends));
		report.push_str(&format!(
			"  total_receiver_calls: {}\n",
			stats.total_receiver_calls
		));
		report.push_str(&format!(
			"  successful_executions: {}\n",
			stats.successful_executions
		));
		report.push_str(&format!(
			"  failed_executions: {}\n",
			stats.failed_executions
		));

		if !stats.receiver_call_counts.is_empty() {
			report.push_str("\nReceiver Call Counts:\n");
			for (uid, count) in &stats.receiver_call_counts {
				report.push_str(&format!("  {}: {} calls\n", uid, count));
			}
		}

		if !events.is_empty() {
			report.push_str(&format!("\nRecent Events ({} total):\n", events.len()));
			// Show last 10 events
			for event in events.iter().rev().take(10) {
				report.push_str(&format!(
					"  [{:?}] {} - success: {}\n",
					event.timestamp, event.event_type, event.success
				));
				if let Some(uid) = &event.dispatch_uid {
					report.push_str(&format!("    receiver: {}\n", uid));
				}
				if let Some(error) = &event.error_message {
					report.push_str(&format!("    error: {}\n", error));
				}
			}
		}

		report
	}

	/// Get events within a time range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	/// use std::time::{SystemTime, Duration};
	///
	/// let debugger = SignalDebugger::<String>::new();
	/// let now = SystemTime::now();
	/// let one_hour_ago = now - Duration::from_secs(3600);
	///
	/// let recent_events = debugger.events_in_range(one_hour_ago, now);
	/// // Returns events from the last hour
	/// ```
	pub fn events_in_range(&self, start: SystemTime, end: SystemTime) -> Vec<DebugEvent> {
		self.events
			.read()
			.iter()
			.filter(|e| e.timestamp >= start && e.timestamp <= end)
			.cloned()
			.collect()
	}

	/// Get events of a specific type
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	///
	/// # tokio_test::block_on(async {
	/// let debugger = SignalDebugger::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(debugger.clone());
	///
	/// signal.connect(|_| async { Ok(()) });
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let send_events = debugger.events_by_type("before_send");
	/// assert_eq!(send_events.len(), 1);
	/// # });
	/// ```
	pub fn events_by_type(&self, event_type: &str) -> Vec<DebugEvent> {
		self.events
			.read()
			.iter()
			.filter(|e| e.event_type == event_type)
			.cloned()
			.collect()
	}

	/// Get failed events only
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::debugger::SignalDebugger;
	///
	/// let debugger = SignalDebugger::<String>::new();
	/// let failed = debugger.failed_events();
	/// // Returns only events where success = false
	/// ```
	pub fn failed_events(&self) -> Vec<DebugEvent> {
		self.events
			.read()
			.iter()
			.filter(|e| !e.success)
			.cloned()
			.collect()
	}

	fn record_event(&self, event: DebugEvent) {
		self.events.write().push(event);
	}
}

impl<T: Send + Sync + 'static> Clone for SignalDebugger<T> {
	fn clone(&self) -> Self {
		Self {
			events: Arc::clone(&self.events),
			stats: Arc::clone(&self.stats),
			_phantom: std::marker::PhantomData,
		}
	}
}

impl<T: Send + Sync + 'static> Default for SignalDebugger<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl<T: Send + Sync + 'static> SignalMiddleware<T> for SignalDebugger<T> {
	async fn before_send(&self, _instance: &T) -> Result<bool, SignalError> {
		self.stats.write().total_sends += 1;

		self.record_event(DebugEvent {
			event_type: "before_send".to_string(),
			timestamp: SystemTime::now(),
			dispatch_uid: None,
			success: true,
			error_message: None,
		});

		Ok(true)
	}

	async fn after_send(
		&self,
		_instance: &T,
		results: &[Result<(), SignalError>],
	) -> Result<(), SignalError> {
		let errors: Vec<String> = results
			.iter()
			.filter_map(|r| r.as_ref().err().map(|e| e.message.clone()))
			.collect();

		self.record_event(DebugEvent {
			event_type: "after_send".to_string(),
			timestamp: SystemTime::now(),
			dispatch_uid: None,
			success: errors.is_empty(),
			error_message: if errors.is_empty() {
				None
			} else {
				Some(errors.join(", "))
			},
		});

		Ok(())
	}

	async fn before_receiver(
		&self,
		_instance: &T,
		dispatch_uid: Option<&str>,
	) -> Result<bool, SignalError> {
		self.stats.write().total_receiver_calls += 1;

		if let Some(uid) = dispatch_uid {
			let mut stats = self.stats.write();
			*stats
				.receiver_call_counts
				.entry(uid.to_string())
				.or_insert(0) += 1;
		}

		self.record_event(DebugEvent {
			event_type: "before_receiver".to_string(),
			timestamp: SystemTime::now(),
			dispatch_uid: dispatch_uid.map(String::from),
			success: true,
			error_message: None,
		});

		Ok(true)
	}

	async fn after_receiver(
		&self,
		_instance: &T,
		dispatch_uid: Option<&str>,
		result: &Result<(), SignalError>,
	) -> Result<(), SignalError> {
		let success = result.is_ok();

		if success {
			self.stats.write().successful_executions += 1;
		} else {
			self.stats.write().failed_executions += 1;
		}

		self.record_event(DebugEvent {
			event_type: "after_receiver".to_string(),
			timestamp: SystemTime::now(),
			dispatch_uid: dispatch_uid.map(String::from),
			success,
			error_message: result.as_ref().err().map(|e| e.message.clone()),
		});

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::signals::{SignalName, signal::Signal};

	#[derive(Debug, Clone)]
	// Allow dead_code: test-only data struct; fields accessed indirectly via Clone/Debug derives and signal dispatch
	#[allow(dead_code)]
	struct TestData {
		value: String,
	}

	#[tokio::test]
	async fn test_debugger_tracks_sends() {
		let debugger = SignalDebugger::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(debugger.clone());

		signal.connect(|_| async { Ok(()) });

		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		let stats = debugger.stats();
		assert_eq!(stats.total_sends, 1);
		assert_eq!(stats.total_receiver_calls, 1);
		assert_eq!(stats.successful_executions, 1);
	}

	#[tokio::test]
	async fn test_debugger_tracks_failures() {
		let debugger = SignalDebugger::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(debugger.clone());

		signal.connect(|_| async { Err(SignalError::new("Test error")) });

		let _ = signal
			.send_robust(
				TestData {
					value: "test".to_string(),
				},
				None,
			)
			.await;

		let stats = debugger.stats();
		assert_eq!(stats.failed_executions, 1);

		let failed = debugger.failed_events();
		assert!(!failed.is_empty());
	}

	#[tokio::test]
	async fn test_debugger_report() {
		let debugger = SignalDebugger::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(debugger.clone());

		signal.connect(|_| async { Ok(()) });
		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		let report = debugger.generate_report();
		assert!(report.contains("Signal Debug Report"));
		assert!(report.contains("total_sends: 1"));
	}

	#[tokio::test]
	async fn test_debugger_clear() {
		let debugger = SignalDebugger::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(debugger.clone());

		signal.connect(|_| async { Ok(()) });
		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		assert_eq!(debugger.stats().total_sends, 1);

		debugger.clear();

		assert_eq!(debugger.stats().total_sends, 0);
		assert_eq!(debugger.events().len(), 0);
	}

	#[tokio::test]
	async fn test_events_by_type() {
		let debugger = SignalDebugger::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(debugger.clone());

		signal.connect(|_| async { Ok(()) });
		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		let before_send = debugger.events_by_type("before_send");
		assert_eq!(before_send.len(), 1);

		let after_send = debugger.events_by_type("after_send");
		assert_eq!(after_send.len(), 1);
	}
}
