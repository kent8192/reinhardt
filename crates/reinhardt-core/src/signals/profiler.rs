//! Signal profiler for performance analysis of signal systems
//!
//! Provides detailed performance metrics, bottleneck detection,
//! and optimization recommendations for signal-based systems.
//!
//! # Examples
//!
//! ```
//! use reinhardt_core::signals::{Signal, SignalName};
//! use reinhardt_core::signals::profiler::SignalProfiler;
//!
//! # tokio_test::block_on(async {
//! let signal = Signal::<String>::new(SignalName::custom("user_created"));
//! let profiler = SignalProfiler::new();
//!
//! // Attach profiler to signal
//! signal.add_middleware(profiler.clone());
//!
//! // Connect receivers
//! signal.connect(|_| async {
//!     Ok(())
//! });
//!
//! // Send signal
//! signal.send("user123".to_string()).await.unwrap();
//!
//! // Get performance report
//! let report = profiler.generate_report();
//! assert!(report.contains("Performance Profile"));
//! # });
//! ```

use super::error::SignalError;
use super::middleware::SignalMiddleware;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Performance profile for a single receiver
#[derive(Debug, Clone)]
pub struct ReceiverProfile {
	/// Receiver dispatch UID
	pub dispatch_uid: String,
	/// Number of times this receiver was called
	pub call_count: usize,
	/// Total execution time
	pub total_duration: Duration,
	/// Minimum execution time
	pub min_duration: Duration,
	/// Maximum execution time
	pub max_duration: Duration,
	/// Average execution time
	pub avg_duration: Duration,
	/// Number of failed executions
	pub failure_count: usize,
	/// Last execution timestamp
	pub last_execution: Option<SystemTime>,
}

impl ReceiverProfile {
	fn new(dispatch_uid: String) -> Self {
		Self {
			dispatch_uid,
			call_count: 0,
			total_duration: Duration::ZERO,
			min_duration: Duration::MAX,
			max_duration: Duration::ZERO,
			avg_duration: Duration::ZERO,
			failure_count: 0,
			last_execution: None,
		}
	}

	fn record_execution(&mut self, duration: Duration, success: bool) {
		self.call_count += 1;
		self.total_duration += duration;
		self.min_duration = self.min_duration.min(duration);
		self.max_duration = self.max_duration.max(duration);
		self.avg_duration = self.total_duration / u32::try_from(self.call_count).unwrap_or(u32::MAX);
		self.last_execution = Some(SystemTime::now());

		if !success {
			self.failure_count += 1;
		}
	}

	/// Get success rate as a percentage (0.0 to 100.0)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// # tokio_test::block_on(async {
	/// let profiler = SignalProfiler::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(profiler.clone());
	///
	/// signal.connect_with_options(
	///     |_| async { Ok(()) },
	///     None,
	///     Some("test_receiver".to_string()),
	///     0
	/// );
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let profile = profiler.get_receiver_profile("test_receiver").unwrap();
	/// assert_eq!(profile.success_rate(), 100.0);
	/// # });
	/// ```
	pub fn success_rate(&self) -> f64 {
		if self.call_count == 0 {
			return 100.0;
		}
		let successful = self.call_count - self.failure_count;
		(successful as f64 / self.call_count as f64) * 100.0
	}
}

/// Performance statistics for signal emissions
#[derive(Debug, Clone)]
pub struct SignalPerformanceStats {
	/// Total number of signal sends
	pub total_sends: usize,
	/// Total execution time for all signals
	pub total_duration: Duration,
	/// Average time per signal send
	pub avg_send_duration: Duration,
	/// Slowest signal send duration
	pub slowest_send: Duration,
	/// Fastest signal send duration
	pub fastest_send: Duration,
}

impl Default for SignalPerformanceStats {
	fn default() -> Self {
		Self {
			total_sends: 0,
			total_duration: Duration::ZERO,
			avg_send_duration: Duration::ZERO,
			slowest_send: Duration::ZERO,
			fastest_send: Duration::MAX,
		}
	}
}

/// Signal profiler for performance analysis
///
/// This middleware tracks execution times, call counts, and failure rates
/// for each receiver to help identify performance bottlenecks.
pub struct SignalProfiler<T: Send + Sync + 'static> {
	receiver_profiles: Arc<RwLock<HashMap<String, ReceiverProfile>>>,
	performance_stats: Arc<RwLock<SignalPerformanceStats>>,
	current_send_start: Arc<RwLock<Option<Instant>>>,
	current_receiver_start: Arc<RwLock<HashMap<String, Instant>>>,
	_phantom: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> SignalProfiler<T> {
	/// Create a new signal profiler
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// let profiler = SignalProfiler::<String>::new();
	/// ```
	pub fn new() -> Self {
		Self {
			receiver_profiles: Arc::new(RwLock::new(HashMap::new())),
			performance_stats: Arc::new(RwLock::new(SignalPerformanceStats::default())),
			current_send_start: Arc::new(RwLock::new(None)),
			current_receiver_start: Arc::new(RwLock::new(HashMap::new())),
			_phantom: std::marker::PhantomData,
		}
	}

	/// Get profile for a specific receiver
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// # tokio_test::block_on(async {
	/// let profiler = SignalProfiler::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(profiler.clone());
	///
	/// signal.connect_with_options(
	///     |_| async { Ok(()) },
	///     None,
	///     Some("my_receiver".to_string()),
	///     0
	/// );
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let profile = profiler.get_receiver_profile("my_receiver");
	/// assert!(profile.is_some());
	/// # });
	/// ```
	pub fn get_receiver_profile(&self, dispatch_uid: &str) -> Option<ReceiverProfile> {
		self.receiver_profiles.read().get(dispatch_uid).cloned()
	}

	/// Get all receiver profiles
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// let profiler = SignalProfiler::<String>::new();
	/// let profiles = profiler.all_receiver_profiles();
	/// // Returns all receiver performance profiles
	/// ```
	pub fn all_receiver_profiles(&self) -> Vec<ReceiverProfile> {
		self.receiver_profiles.read().values().cloned().collect()
	}

	/// Get performance statistics
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// # tokio_test::block_on(async {
	/// let profiler = SignalProfiler::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(profiler.clone());
	///
	/// signal.connect(|_| async { Ok(()) });
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let stats = profiler.performance_stats();
	/// assert_eq!(stats.total_sends, 1);
	/// # });
	/// ```
	pub fn performance_stats(&self) -> SignalPerformanceStats {
		self.performance_stats.read().clone()
	}

	/// Get slowest receivers (top N by average execution time)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// let profiler = SignalProfiler::<String>::new();
	/// let slowest = profiler.slowest_receivers(5);
	/// // Returns up to 5 slowest receivers
	/// ```
	pub fn slowest_receivers(&self, count: usize) -> Vec<ReceiverProfile> {
		let mut profiles = self.all_receiver_profiles();
		profiles.sort_by(|a, b| b.avg_duration.cmp(&a.avg_duration));
		profiles.truncate(count);
		profiles
	}

	/// Get receivers with highest failure rates
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// let profiler = SignalProfiler::<String>::new();
	/// let unreliable = profiler.most_unreliable_receivers(5);
	/// // Returns up to 5 receivers with highest failure rates
	/// ```
	pub fn most_unreliable_receivers(&self, count: usize) -> Vec<ReceiverProfile> {
		let mut profiles = self.all_receiver_profiles();
		profiles.sort_by(|a, b| {
			let a_rate = a.success_rate();
			let b_rate = b.success_rate();
			a_rate.partial_cmp(&b_rate).unwrap()
		});
		profiles.truncate(count);
		profiles
	}

	/// Reset all profiling data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// let profiler = SignalProfiler::<String>::new();
	/// profiler.reset();
	/// assert_eq!(profiler.all_receiver_profiles().len(), 0);
	/// ```
	pub fn reset(&self) {
		self.receiver_profiles.write().clear();
		*self.performance_stats.write() = SignalPerformanceStats::default();
		*self.current_send_start.write() = None;
		self.current_receiver_start.write().clear();
	}

	/// Generate a human-readable performance report
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::signals::{Signal, SignalName};
	/// use reinhardt_core::signals::profiler::SignalProfiler;
	///
	/// # tokio_test::block_on(async {
	/// let profiler = SignalProfiler::<String>::new();
	/// let signal = Signal::<String>::new(SignalName::custom("test"));
	/// signal.add_middleware(profiler.clone());
	///
	/// signal.connect(|_| async { Ok(()) });
	/// signal.send("test".to_string()).await.unwrap();
	///
	/// let report = profiler.generate_report();
	/// assert!(report.contains("Performance Profile"));
	/// # });
	/// ```
	pub fn generate_report(&self) -> String {
		let stats = self.performance_stats.read();
		let profiles = self.receiver_profiles.read();

		let mut report = String::from("=== Signal Performance Profile ===\n\n");

		report.push_str("Overall Statistics:\n");
		report.push_str(&format!("  Total sends: {}\n", stats.total_sends));
		report.push_str(&format!("  Total duration: {:?}\n", stats.total_duration));
		report.push_str(&format!(
			"  Average send duration: {:?}\n",
			stats.avg_send_duration
		));
		report.push_str(&format!("  Slowest send: {:?}\n", stats.slowest_send));
		report.push_str(&format!(
			"  Fastest send: {:?}\n",
			if stats.fastest_send == Duration::MAX {
				Duration::ZERO
			} else {
				stats.fastest_send
			}
		));

		if !profiles.is_empty() {
			report.push_str("\nReceiver Profiles:\n");

			let mut sorted_profiles: Vec<_> = profiles.values().collect();
			sorted_profiles.sort_by(|a, b| b.avg_duration.cmp(&a.avg_duration));

			for profile in sorted_profiles {
				report.push_str(&format!("\n  {}:\n", profile.dispatch_uid));
				report.push_str(&format!("    Calls: {}\n", profile.call_count));
				report.push_str(&format!("    Total time: {:?}\n", profile.total_duration));
				report.push_str(&format!("    Avg time: {:?}\n", profile.avg_duration));
				report.push_str(&format!("    Min time: {:?}\n", profile.min_duration));
				report.push_str(&format!("    Max time: {:?}\n", profile.max_duration));
				report.push_str(&format!(
					"    Success rate: {:.2}%\n",
					profile.success_rate()
				));

				if profile.failure_count > 0 {
					report.push_str(&format!("    Failures: {}\n", profile.failure_count));
				}
			}
		}

		// Recommendations
		report.push_str("\nRecommendations:\n");

		if let Some(slowest) = self.slowest_receivers(1).first()
			&& slowest.avg_duration.as_millis() > 100
		{
			report.push_str(&format!(
				"  ⚠ Receiver '{}' is slow (avg: {:?}). Consider optimization.\n",
				slowest.dispatch_uid, slowest.avg_duration
			));
		}

		let unreliable = self.most_unreliable_receivers(3);
		for profile in unreliable {
			if profile.success_rate() < 95.0 {
				report.push_str(&format!(
					"  ⚠ Receiver '{}' has low success rate ({:.2}%). Check error handling.\n",
					profile.dispatch_uid,
					profile.success_rate()
				));
			}
		}

		if stats.total_sends > 0 && stats.avg_send_duration.as_millis() > 500 {
			report.push_str(&format!(
				"  ⚠ Average send duration is high ({:?}). Consider async execution or batching.\n",
				stats.avg_send_duration
			));
		}

		report
	}
}

impl<T: Send + Sync + 'static> Clone for SignalProfiler<T> {
	fn clone(&self) -> Self {
		Self {
			receiver_profiles: Arc::clone(&self.receiver_profiles),
			performance_stats: Arc::clone(&self.performance_stats),
			current_send_start: Arc::clone(&self.current_send_start),
			current_receiver_start: Arc::clone(&self.current_receiver_start),
			_phantom: std::marker::PhantomData,
		}
	}
}

impl<T: Send + Sync + 'static> Default for SignalProfiler<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl<T: Send + Sync + 'static> SignalMiddleware<T> for SignalProfiler<T> {
	async fn before_send(&self, _instance: &T) -> Result<bool, SignalError> {
		*self.current_send_start.write() = Some(Instant::now());
		Ok(true)
	}

	async fn after_send(
		&self,
		_instance: &T,
		_results: &[Result<(), SignalError>],
	) -> Result<(), SignalError> {
		if let Some(start) = *self.current_send_start.read() {
			let duration = start.elapsed();
			let mut stats = self.performance_stats.write();

			stats.total_sends += 1;
			stats.total_duration += duration;
			let divisor = u32::try_from(stats.total_sends).unwrap_or(u32::MAX);
			stats.avg_send_duration = stats.total_duration / divisor;
			stats.slowest_send = stats.slowest_send.max(duration);
			stats.fastest_send = stats.fastest_send.min(duration);
		}

		*self.current_send_start.write() = None;
		Ok(())
	}

	async fn before_receiver(
		&self,
		_instance: &T,
		dispatch_uid: Option<&str>,
	) -> Result<bool, SignalError> {
		if let Some(uid) = dispatch_uid {
			self.current_receiver_start
				.write()
				.insert(uid.to_string(), Instant::now());
		}
		Ok(true)
	}

	async fn after_receiver(
		&self,
		_instance: &T,
		dispatch_uid: Option<&str>,
		result: &Result<(), SignalError>,
	) -> Result<(), SignalError> {
		if let Some(uid) = dispatch_uid
			&& let Some(start) = self.current_receiver_start.write().remove(uid)
		{
			let duration = start.elapsed();
			let success = result.is_ok();

			let mut profiles = self.receiver_profiles.write();
			let profile = profiles
				.entry(uid.to_string())
				.or_insert_with(|| ReceiverProfile::new(uid.to_string()));

			profile.record_execution(duration, success);
		}

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
	async fn test_profiler_tracks_execution_time() {
		let profiler = SignalProfiler::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(profiler.clone());

		signal.connect_with_options(
			|_| async { Ok(()) },
			None,
			Some("slow_receiver".to_string()),
			0,
		);

		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		let stats = profiler.performance_stats();
		assert_eq!(stats.total_sends, 1);
		// avg_send_duration should be recorded (non-zero for valid execution)
		assert!(stats.avg_send_duration > Duration::ZERO);
	}

	#[tokio::test]
	async fn test_profiler_tracks_receiver_performance() {
		let profiler = SignalProfiler::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(profiler.clone());

		signal.connect_with_options(
			|_| async { Ok(()) },
			None,
			Some("test_receiver".to_string()),
			0,
		);

		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		let profile = profiler.get_receiver_profile("test_receiver");
		let profile = profile.unwrap();
		assert_eq!(profile.call_count, 1);
		// avg_duration should be recorded (non-zero for valid execution)
		assert!(profile.avg_duration > Duration::ZERO);
	}

	#[tokio::test]
	async fn test_profiler_report() {
		let profiler = SignalProfiler::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(profiler.clone());

		signal.connect(|_| async { Ok(()) });
		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		let report = profiler.generate_report();
		assert!(report.contains("Performance Profile"));
		assert!(report.contains("Total sends: 1"));
	}

	#[tokio::test]
	async fn test_profiler_reset() {
		let profiler = SignalProfiler::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(profiler.clone());

		signal.connect(|_| async { Ok(()) });
		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		assert_eq!(profiler.performance_stats().total_sends, 1);

		profiler.reset();

		assert_eq!(profiler.performance_stats().total_sends, 0);
		assert_eq!(profiler.all_receiver_profiles().len(), 0);
	}

	#[tokio::test]
	async fn test_slowest_receivers() {
		let profiler = SignalProfiler::new();
		let signal = Signal::<TestData>::new(SignalName::custom("test"));
		signal.add_middleware(profiler.clone());

		signal.connect_with_options(
			|_| async {
				tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
				Ok(())
			},
			None,
			Some("slow".to_string()),
			0,
		);

		signal.connect_with_options(
			|_| async {
				tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
				Ok(())
			},
			None,
			Some("fast".to_string()),
			0,
		);

		signal
			.send(TestData {
				value: "test".to_string(),
			})
			.await
			.unwrap();

		let slowest = profiler.slowest_receivers(1);
		assert_eq!(slowest.len(), 1);
		assert_eq!(slowest[0].dispatch_uid, "slow");
	}
}
