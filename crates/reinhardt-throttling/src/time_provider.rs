use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::time::Instant;

/// Trait for providing time information to throttle backends.
/// This allows for time mocking in tests.
#[async_trait]
pub trait TimeProvider: Send + Sync {
	fn now(&self) -> Instant;

	/// Returns the current hour of the day (0-23) from wall clock time.
	///
	/// Monotonic clocks (`Instant`) have an arbitrary epoch and cannot be used
	/// for wall clock calculations. This method uses `SystemTime` by default
	/// to derive the actual hour of the day (UTC).
	fn wall_clock_hour(&self) -> u8 {
		use std::time::{SystemTime, UNIX_EPOCH};
		let secs = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("system clock is before UNIX epoch")
			.as_secs();
		((secs % 86400) / 3600) as u8
	}
}

/// System time provider that uses the actual system clock.
#[derive(Clone, Default)]
pub struct SystemTimeProvider;

impl SystemTimeProvider {
	pub fn new() -> Self {
		Self
	}
}

#[async_trait]
impl TimeProvider for SystemTimeProvider {
	fn now(&self) -> Instant {
		Instant::now()
	}
}

/// Mock time provider for testing that allows manual time control.
#[derive(Clone)]
pub struct MockTimeProvider {
	current_time: Arc<RwLock<Instant>>,
	mock_hour: Arc<RwLock<Option<u8>>>,
}

impl MockTimeProvider {
	pub fn new(start_time: Instant) -> Self {
		Self {
			current_time: Arc::new(RwLock::new(start_time)),
			mock_hour: Arc::new(RwLock::new(None)),
		}
	}

	pub fn advance(&self, duration: std::time::Duration) {
		let mut time = self.current_time.write();
		*time += duration;
	}

	pub fn set_time(&self, time: Instant) {
		let mut current = self.current_time.write();
		*current = time;
	}

	/// Set a fixed wall clock hour for testing time-of-day throttling
	pub fn set_wall_clock_hour(&self, hour: u8) {
		assert!(hour < 24, "hour must be 0-23");
		let mut mock_hour = self.mock_hour.write();
		*mock_hour = Some(hour);
	}
}

impl Default for MockTimeProvider {
	fn default() -> Self {
		Self::new(Instant::now())
	}
}

#[async_trait]
impl TimeProvider for MockTimeProvider {
	fn now(&self) -> Instant {
		*self.current_time.read()
	}

	fn wall_clock_hour(&self) -> u8 {
		self.mock_hour.read().unwrap_or_else(|| {
			// Fall back to actual wall clock if no mock hour is set
			use std::time::{SystemTime, UNIX_EPOCH};
			let secs = SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.expect("system clock is before UNIX epoch")
				.as_secs();
			((secs % 86400) / 3600) as u8
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::time::Duration;

	#[rstest]
	fn test_system_time_provider_returns_current_time() {
		// Arrange
		let provider = SystemTimeProvider::new();

		// Act
		let time1 = provider.now();
		std::thread::sleep(Duration::from_millis(10));
		let time2 = provider.now();

		// Assert
		assert!(time2 > time1);
	}

	#[rstest]
	fn test_mock_time_provider_allows_time_control() {
		// Arrange
		let start = Instant::now();
		let provider = MockTimeProvider::new(start);

		// Act & Assert
		let time1 = provider.now();
		assert_eq!(time1, start);

		// Act
		provider.advance(Duration::from_secs(60));
		let time2 = provider.now();

		// Assert
		assert_eq!(time2, start + Duration::from_secs(60));
	}

	#[rstest]
	fn test_mock_time_provider_set_time() {
		// Arrange
		let provider = MockTimeProvider::new(Instant::now());
		let new_time = Instant::now() + Duration::from_secs(100);

		// Act
		provider.set_time(new_time);

		// Assert
		assert_eq!(provider.now(), new_time);
	}

	#[rstest]
	fn test_system_time_provider_wall_clock_hour_returns_valid_range() {
		// Arrange
		let provider = SystemTimeProvider::new();

		// Act
		let hour = provider.wall_clock_hour();

		// Assert
		assert!(hour < 24);
	}

	#[rstest]
	#[case::midnight(0)]
	#[case::noon(12)]
	#[case::evening(23)]
	fn test_mock_wall_clock_hour(#[case] expected_hour: u8) {
		// Arrange
		let provider = MockTimeProvider::new(Instant::now());
		provider.set_wall_clock_hour(expected_hour);

		// Act
		let hour = provider.wall_clock_hour();

		// Assert
		assert_eq!(hour, expected_hour);
	}

	#[rstest]
	#[should_panic(expected = "hour must be 0-23")]
	fn test_mock_wall_clock_hour_rejects_invalid() {
		// Arrange
		let provider = MockTimeProvider::new(Instant::now());

		// Act - should panic
		provider.set_wall_clock_hour(24);
	}
}
