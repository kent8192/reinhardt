//! Signal context and metrics

use parking_lot::RwLock;
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Context information passed with signals
/// Allows passing additional metadata alongside the signal instance
pub struct SignalContext {
	metadata: Arc<RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>>,
}

impl SignalContext {
	/// Creates a new empty signal context.
	pub fn new() -> Self {
		Self {
			metadata: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Insert a value into the context
	pub fn insert<V: Any + Send + Sync>(&self, key: impl Into<String>, value: V) {
		self.metadata.write().insert(key.into(), Arc::new(value));
	}

	/// Get a value from the context
	pub fn get<V: Any + Send + Sync>(&self, key: &str) -> Option<Arc<V>> {
		let metadata = self.metadata.read();
		let value = metadata.get(key)?;
		value.clone().downcast::<V>().ok()
	}

	/// Check if a key exists in the context
	pub fn contains_key(&self, key: &str) -> bool {
		self.metadata.read().contains_key(key)
	}

	/// Remove a value from the context
	pub fn remove(&self, key: &str) -> bool {
		self.metadata.write().remove(key).is_some()
	}

	/// Clear all context data
	pub fn clear(&self) {
		self.metadata.write().clear();
	}

	/// Get all keys in the context
	pub fn keys(&self) -> Vec<String> {
		self.metadata.read().keys().cloned().collect()
	}
}

impl Clone for SignalContext {
	fn clone(&self) -> Self {
		Self {
			metadata: Arc::clone(&self.metadata),
		}
	}
}

impl Default for SignalContext {
	fn default() -> Self {
		Self::new()
	}
}

impl fmt::Debug for SignalContext {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let keys = self.keys();
		f.debug_struct("SignalContext")
			.field("keys", &keys)
			.finish()
	}
}

/// Performance metrics for a signal
#[derive(Debug, Clone)]
pub struct SignalMetrics {
	/// Total number of times the signal was sent
	pub send_count: u64,
	/// Total number of receiver executions
	pub receiver_executions: u64,
	/// Total number of failed receiver executions
	pub failed_executions: u64,
	/// Total execution time in nanoseconds
	pub total_execution_time_ns: u64,
	/// Minimum execution time in nanoseconds
	pub min_execution_time_ns: u64,
	/// Maximum execution time in nanoseconds
	pub max_execution_time_ns: u64,
	/// Average execution time in nanoseconds
	pub avg_execution_time_ns: u64,
}

impl SignalMetrics {
	/// Creates a new `SignalMetrics` with all counters at zero.
	pub fn new() -> Self {
		Self {
			send_count: 0,
			receiver_executions: 0,
			failed_executions: 0,
			total_execution_time_ns: 0,
			min_execution_time_ns: u64::MAX,
			max_execution_time_ns: 0,
			avg_execution_time_ns: 0,
		}
	}

	/// Get the success rate as a percentage (0.0 to 100.0)
	pub fn success_rate(&self) -> f64 {
		if self.receiver_executions == 0 {
			return 100.0;
		}
		let successful = self.receiver_executions - self.failed_executions;
		(successful as f64 / self.receiver_executions as f64) * 100.0
	}

	/// Get the average execution time as a Duration
	pub fn avg_execution_time(&self) -> Duration {
		Duration::from_nanos(self.avg_execution_time_ns)
	}

	/// Get the minimum execution time as a Duration
	pub fn min_execution_time(&self) -> Duration {
		if self.min_execution_time_ns == u64::MAX {
			Duration::from_nanos(0)
		} else {
			Duration::from_nanos(self.min_execution_time_ns)
		}
	}

	/// Get the maximum execution time as a Duration
	pub fn max_execution_time(&self) -> Duration {
		Duration::from_nanos(self.max_execution_time_ns)
	}
}

impl Default for SignalMetrics {
	fn default() -> Self {
		Self::new()
	}
}

/// Internal metrics collector for a signal
pub(crate) struct MetricsCollector {
	send_count: AtomicU64,
	receiver_executions: AtomicU64,
	failed_executions: AtomicU64,
	total_execution_time_ns: AtomicU64,
	min_execution_time_ns: AtomicU64,
	max_execution_time_ns: AtomicU64,
}

impl MetricsCollector {
	pub(crate) fn new() -> Self {
		Self {
			send_count: AtomicU64::new(0),
			receiver_executions: AtomicU64::new(0),
			failed_executions: AtomicU64::new(0),
			total_execution_time_ns: AtomicU64::new(0),
			min_execution_time_ns: AtomicU64::new(u64::MAX),
			max_execution_time_ns: AtomicU64::new(0),
		}
	}

	pub(crate) fn record_send(&self) {
		self.send_count.fetch_add(1, Ordering::Relaxed);
	}

	pub(crate) fn record_receiver_execution(&self, duration: Duration, success: bool) {
		self.receiver_executions.fetch_add(1, Ordering::Relaxed);

		if !success {
			self.failed_executions.fetch_add(1, Ordering::Relaxed);
		}

		let duration_ns = duration.as_nanos() as u64;
		self.total_execution_time_ns
			.fetch_add(duration_ns, Ordering::Relaxed);

		// Update min
		let mut current_min = self.min_execution_time_ns.load(Ordering::Relaxed);
		while duration_ns < current_min {
			match self.min_execution_time_ns.compare_exchange(
				current_min,
				duration_ns,
				Ordering::Relaxed,
				Ordering::Relaxed,
			) {
				Ok(_) => break,
				Err(new_min) => current_min = new_min,
			}
		}

		// Update max
		let mut current_max = self.max_execution_time_ns.load(Ordering::Relaxed);
		while duration_ns > current_max {
			match self.max_execution_time_ns.compare_exchange(
				current_max,
				duration_ns,
				Ordering::Relaxed,
				Ordering::Relaxed,
			) {
				Ok(_) => break,
				Err(new_max) => current_max = new_max,
			}
		}
	}

	pub(crate) fn snapshot(&self) -> SignalMetrics {
		let send_count = self.send_count.load(Ordering::Relaxed);
		let receiver_executions = self.receiver_executions.load(Ordering::Relaxed);
		let failed_executions = self.failed_executions.load(Ordering::Relaxed);
		let total_execution_time_ns = self.total_execution_time_ns.load(Ordering::Relaxed);
		let min_execution_time_ns = self.min_execution_time_ns.load(Ordering::Relaxed);
		let max_execution_time_ns = self.max_execution_time_ns.load(Ordering::Relaxed);

		let avg_execution_time_ns = if receiver_executions > 0 {
			total_execution_time_ns / receiver_executions
		} else {
			0
		};

		SignalMetrics {
			send_count,
			receiver_executions,
			failed_executions,
			total_execution_time_ns,
			min_execution_time_ns,
			max_execution_time_ns,
			avg_execution_time_ns,
		}
	}

	pub(crate) fn reset(&self) {
		self.send_count.store(0, Ordering::Relaxed);
		self.receiver_executions.store(0, Ordering::Relaxed);
		self.failed_executions.store(0, Ordering::Relaxed);
		self.total_execution_time_ns.store(0, Ordering::Relaxed);
		self.min_execution_time_ns
			.store(u64::MAX, Ordering::Relaxed);
		self.max_execution_time_ns.store(0, Ordering::Relaxed);
	}
}
