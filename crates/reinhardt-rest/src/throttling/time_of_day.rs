//! Time-of-day based rate limiting implementation
//!
//! Allows different rate limits based on the time of day, enabling peak/off-peak
//! rate differentiation.

use super::backend::ThrottleBackend;
use super::{Throttle, ThrottleError, ThrottleResult};
use super::time_provider::{SystemTimeProvider, TimeProvider};
use async_trait::async_trait;
use std::sync::Arc;

/// Time range for rate limiting (in hours, 0-23)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeRange {
	/// Start hour (0-23)
	pub start_hour: u8,
	/// End hour (0-23)
	pub end_hour: u8,
}

impl TimeRange {
	/// Creates a new time range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::time_of_day::TimeRange;
	///
	/// // Peak hours: 9 AM to 5 PM
	/// let peak = TimeRange::new(9, 17);
	/// assert_eq!(peak.start_hour, 9);
	/// assert_eq!(peak.end_hour, 17);
	/// ```
	pub fn new(start_hour: u8, end_hour: u8) -> Self {
		assert!(start_hour < 24, "start_hour must be 0-23");
		assert!(end_hour < 24, "end_hour must be 0-23");

		Self {
			start_hour,
			end_hour,
		}
	}

	/// Check if the given hour is within this range
	pub fn contains(&self, hour: u8) -> bool {
		if self.start_hour <= self.end_hour {
			// Normal range (e.g., 9-17)
			hour >= self.start_hour && hour <= self.end_hour
		} else {
			// Wrapping range (e.g., 22-2)
			hour >= self.start_hour || hour <= self.end_hour
		}
	}
}

/// Configuration for time-of-day based rate limiting
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TimeOfDayConfig {
	/// Peak time range
	pub peak_hours: TimeRange,
	/// Rate limit during peak hours (requests, period in seconds)
	pub peak_rate: (usize, u64),
	/// Rate limit during off-peak hours (requests, period in seconds)
	pub off_peak_rate: (usize, u64),
}

impl TimeOfDayConfig {
	/// Creates a new time-of-day configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::time_of_day::{TimeOfDayConfig, TimeRange};
	///
	/// // Peak hours: 9 AM to 5 PM, 50 req/min
	/// // Off-peak: 100 req/min
	/// let config = TimeOfDayConfig::new(
	///     TimeRange::new(9, 17),
	///     (50, 60),
	///     (100, 60)
	/// );
	/// ```
	pub fn new(
		peak_hours: TimeRange,
		peak_rate: (usize, u64),
		off_peak_rate: (usize, u64),
	) -> Self {
		Self {
			peak_hours,
			peak_rate,
			off_peak_rate,
		}
	}

	/// Get the appropriate rate for the given hour
	pub fn get_rate(&self, hour: u8) -> (usize, u64) {
		if self.peak_hours.contains(hour) {
			self.peak_rate
		} else {
			self.off_peak_rate
		}
	}
}

/// Time-of-day based rate limiting throttle
///
/// # Examples
///
/// ```
/// use reinhardt_rest::throttling::time_of_day::{TimeOfDayThrottle, TimeOfDayConfig, TimeRange};
/// use reinhardt_rest::throttling::{MemoryBackend, Throttle};
/// use std::sync::Arc;
///
/// # tokio_test::block_on(async {
/// let backend = Arc::new(MemoryBackend::new());
/// let config = TimeOfDayConfig::new(
///     TimeRange::new(9, 17),
///     (50, 60),
///     (100, 60)
/// );
/// let throttle = TimeOfDayThrottle::new(backend, config);
/// # });
/// ```
pub struct TimeOfDayThrottle<B: ThrottleBackend, T: TimeProvider = SystemTimeProvider> {
	backend: Arc<B>,
	config: TimeOfDayConfig,
	time_provider: Arc<T>,
}

impl<B: ThrottleBackend> TimeOfDayThrottle<B, SystemTimeProvider> {
	/// Creates a new time-of-day throttle with system time provider
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::time_of_day::{TimeOfDayThrottle, TimeOfDayConfig, TimeRange};
	/// use reinhardt_rest::throttling::{MemoryBackend, Throttle};
	/// use std::sync::Arc;
	///
	/// let backend = Arc::new(MemoryBackend::new());
	/// let config = TimeOfDayConfig::new(
	///     TimeRange::new(9, 17),
	///     (50, 60),
	///     (100, 60)
	/// );
	/// let throttle = TimeOfDayThrottle::new(backend, config);
	/// ```
	pub fn new(backend: Arc<B>, config: TimeOfDayConfig) -> Self {
		Self {
			backend,
			config,
			time_provider: Arc::new(SystemTimeProvider::new()),
		}
	}
}

impl<B: ThrottleBackend, T: TimeProvider> TimeOfDayThrottle<B, T> {
	/// Creates a new time-of-day throttle with custom time provider
	pub fn with_time_provider(
		backend: Arc<B>,
		config: TimeOfDayConfig,
		time_provider: Arc<T>,
	) -> Self {
		Self {
			backend,
			config,
			time_provider,
		}
	}

	/// Get current hour (0-23)
	fn get_current_hour(&self) -> u8 {
		// Get current time from provider
		let now = self.time_provider.now();

		// Calculate hour from elapsed time since epoch
		// This is a simplified implementation for testing
		// In production, you'd want to use chrono or time crate for proper timezone handling
		let duration_since_epoch = now.elapsed();
		let total_hours = duration_since_epoch.as_secs() / 3600;
		(total_hours % 24) as u8
	}

	/// Get the appropriate rate for current time
	async fn get_current_rate(&self) -> (usize, u64) {
		let hour = self.get_current_hour();
		self.config.get_rate(hour)
	}
}

#[async_trait]
impl<B: ThrottleBackend, T: TimeProvider> Throttle for TimeOfDayThrottle<B, T> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		let (rate, period) = self.get_current_rate().await;

		let count = self
			.backend
			.increment(key, period)
			.await
			.map_err(ThrottleError::ThrottleError)?;

		Ok(count <= rate)
	}

	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		let (rate, period) = self.get_current_rate().await;

		let count = self
			.backend
			.get_count(key)
			.await
			.map_err(ThrottleError::ThrottleError)?;

		if count > rate {
			Ok(Some(period))
		} else {
			Ok(None)
		}
	}

	fn get_rate(&self) -> (usize, u64) {
		// Return peak rate as default
		self.config.peak_rate
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::throttling::backend::MemoryBackend;
	use crate::throttling::time_provider::MockTimeProvider;
	use tokio::time::Instant;

	#[test]
	fn test_time_range_normal() {
		let range = TimeRange::new(9, 17); // 9 AM to 5 PM

		assert!(range.contains(9));
		assert!(range.contains(12));
		assert!(range.contains(17));
		assert!(!range.contains(8));
		assert!(!range.contains(18));
	}

	#[test]
	fn test_time_range_wrapping() {
		let range = TimeRange::new(22, 2); // 10 PM to 2 AM

		assert!(range.contains(22));
		assert!(range.contains(23));
		assert!(range.contains(0));
		assert!(range.contains(1));
		assert!(range.contains(2));
		assert!(!range.contains(3));
		assert!(!range.contains(21));
	}

	#[test]
	fn test_time_of_day_config_get_rate() {
		let config = TimeOfDayConfig::new(TimeRange::new(9, 17), (50, 60), (100, 60));

		// Peak hours
		assert_eq!(config.get_rate(9), (50, 60));
		assert_eq!(config.get_rate(12), (50, 60));
		assert_eq!(config.get_rate(17), (50, 60));

		// Off-peak hours
		assert_eq!(config.get_rate(8), (100, 60));
		assert_eq!(config.get_rate(18), (100, 60));
		assert_eq!(config.get_rate(0), (100, 60));
	}

	#[tokio::test]
	async fn test_time_of_day_throttle_basic() {
		let backend = Arc::new(MemoryBackend::new());
		let config = TimeOfDayConfig::new(TimeRange::new(9, 17), (5, 60), (10, 60));
		let throttle = TimeOfDayThrottle::new(backend, config);

		// Test basic throttling
		let current_rate = throttle.get_current_rate().await;
		let (limit, _) = current_rate;

		// Should allow up to limit
		for _ in 0..limit {
			assert!(throttle.allow_request("test_key").await.unwrap());
		}

		// Next request should fail
		assert!(!throttle.allow_request("test_key").await.unwrap());
	}

	#[tokio::test]
	async fn test_time_of_day_throttle_with_mock_time() {
		let time_provider = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = Arc::new(MemoryBackend::with_time_provider(time_provider.clone()));
		let config = TimeOfDayConfig::new(TimeRange::new(9, 17), (5, 60), (10, 60));
		let throttle = TimeOfDayThrottle::with_time_provider(backend, config, time_provider);

		// Test with mock time
		let (limit, _) = throttle.get_current_rate().await;

		for _ in 0..limit {
			assert!(throttle.allow_request("test_key").await.unwrap());
		}

		assert!(!throttle.allow_request("test_key").await.unwrap());
	}

	#[tokio::test]
	async fn test_time_of_day_throttle_get_rate() {
		let backend = Arc::new(MemoryBackend::new());
		let config = TimeOfDayConfig::new(TimeRange::new(9, 17), (50, 60), (100, 60));
		let throttle = TimeOfDayThrottle::new(backend, config);

		// Should return peak rate as default
		assert_eq!(throttle.get_rate(), (50, 60));
	}

	#[test]
	#[should_panic(expected = "start_hour must be 0-23")]
	fn test_time_range_invalid_start() {
		TimeRange::new(24, 10);
	}

	#[test]
	#[should_panic(expected = "end_hour must be 0-23")]
	fn test_time_range_invalid_end() {
		TimeRange::new(10, 24);
	}
}
