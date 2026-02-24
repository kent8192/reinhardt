use super::backend::{MemoryBackend, ThrottleBackend};
use super::key_validation::validate_key_component;
use super::{Throttle, ThrottleError, ThrottleResult};
use async_trait::async_trait;

pub struct UserRateThrottle<B: ThrottleBackend = MemoryBackend> {
	pub rate: usize,
	pub window_secs: u64,
	backend: B,
}

impl UserRateThrottle<MemoryBackend> {
	/// Creates a new `UserRateThrottle` with a memory backend.
	///
	/// # Arguments
	///
	/// * `rate` - Maximum number of requests allowed (must be non-zero)
	/// * `window_secs` - Time window in seconds (must be non-zero)
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidConfig`] if `rate` or `window_secs` is zero.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::UserRateThrottle;
	///
	/// // Allow 100 requests per 60 seconds per user
	/// let throttle = UserRateThrottle::new(100, 60).unwrap();
	/// assert_eq!(throttle.rate, 100);
	/// assert_eq!(throttle.window_secs, 60);
	/// ```
	pub fn new(rate: usize, window_secs: u64) -> ThrottleResult<Self> {
		if rate == 0 {
			return Err(ThrottleError::InvalidConfig(
				"rate must be non-zero".to_string(),
			));
		}
		if window_secs == 0 {
			return Err(ThrottleError::InvalidConfig(
				"window_secs must be non-zero".to_string(),
			));
		}
		Ok(Self {
			rate,
			window_secs,
			backend: MemoryBackend::new(),
		})
	}
}

impl<B: ThrottleBackend> UserRateThrottle<B> {
	/// Creates a new `UserRateThrottle` with a custom backend.
	///
	/// # Arguments
	///
	/// * `rate` - Maximum number of requests allowed (must be non-zero)
	/// * `window_secs` - Time window in seconds (must be non-zero)
	/// * `backend` - Custom throttle backend
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidConfig`] if `rate` or `window_secs` is zero.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::{UserRateThrottle, MemoryBackend};
	///
	/// let backend = MemoryBackend::new();
	/// let throttle = UserRateThrottle::with_backend(100, 60, backend).unwrap();
	/// assert_eq!(throttle.rate, 100);
	/// assert_eq!(throttle.window_secs, 60);
	/// ```
	pub fn with_backend(rate: usize, window_secs: u64, backend: B) -> ThrottleResult<Self> {
		if rate == 0 {
			return Err(ThrottleError::InvalidConfig(
				"rate must be non-zero".to_string(),
			));
		}
		if window_secs == 0 {
			return Err(ThrottleError::InvalidConfig(
				"window_secs must be non-zero".to_string(),
			));
		}
		Ok(Self {
			rate,
			window_secs,
			backend,
		})
	}
}

#[async_trait]
impl<B: ThrottleBackend> Throttle for UserRateThrottle<B> {
	async fn allow_request(&self, user_id: &str) -> ThrottleResult<bool> {
		validate_key_component(user_id)?;
		let key = format!("throttle:user:{}", user_id);
		let count = self
			.backend
			.increment(&key, self.window_secs)
			.await
			.map_err(super::ThrottleError::ThrottleError)?;
		Ok(count <= self.rate)
	}
	async fn wait_time(&self, user_id: &str) -> ThrottleResult<Option<u64>> {
		validate_key_component(user_id)?;
		let key = format!("throttle:user:{}", user_id);
		let count = self
			.backend
			.get_count(&key)
			.await
			.map_err(super::ThrottleError::ThrottleError)?;
		if count > self.rate {
			Ok(Some(self.window_secs))
		} else {
			Ok(None)
		}
	}
	fn get_rate(&self) -> (usize, u64) {
		(self.rate, self.window_secs)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_user_throttle() {
		// Arrange
		let throttle = UserRateThrottle::new(10, 60).unwrap();

		// Act & Assert
		for _ in 0..10 {
			assert!(throttle.allow_request("user123").await.unwrap());
		}
		assert!(!throttle.allow_request("user123").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_requests_are_throttled() {
		// Arrange
		let throttle = UserRateThrottle::new(3, 1).unwrap();

		// Act & Assert
		for _ in 0..3 {
			assert!(throttle.allow_request("user1").await.unwrap());
		}

		// Assert - fourth request should be throttled
		assert!(!throttle.allow_request("user1").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_request_throttling_expires() {
		// Arrange
		use crate::backend::MemoryBackend;
		use crate::time_provider::MockTimeProvider;
		use std::sync::Arc;
		use tokio::time::Instant;

		let mock_time = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = MemoryBackend::with_time_provider(mock_time.clone());
		let throttle = UserRateThrottle::with_backend(3, 1, backend).unwrap();

		// Act - fill up the limit
		for _ in 0..3 {
			assert!(throttle.allow_request("user1").await.unwrap());
		}

		// Assert - should be throttled
		assert!(!throttle.allow_request("user1").await.unwrap());

		// Act - advance time by 2 seconds (past the 1-second window)
		mock_time.advance(std::time::Duration::from_secs(2));

		// Assert - should be allowed again after window expires
		assert!(throttle.allow_request("user1").await.unwrap());
		assert!(throttle.allow_request("user1").await.unwrap());
		assert!(throttle.allow_request("user1").await.unwrap());
		assert!(!throttle.allow_request("user1").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_request_throttling_is_per_user() {
		// Arrange
		let throttle = UserRateThrottle::new(3, 1).unwrap();

		// Act
		for _ in 0..3 {
			assert!(throttle.allow_request("user_a").await.unwrap());
		}

		// Assert - user_b should not be affected by user_a's limit
		assert!(throttle.allow_request("user_b").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_wait_returns_correct_waiting_time() {
		// Arrange
		let throttle = UserRateThrottle::new(1, 60).unwrap();

		// Act
		assert!(throttle.allow_request("user1").await.unwrap());
		assert!(!throttle.allow_request("user1").await.unwrap());

		// Assert
		let wait_time = throttle.wait_time("user1").await.unwrap();
		assert_eq!(wait_time, Some(60));
	}

	#[rstest]
	#[tokio::test]
	async fn test_wait_returns_none_when_under_limit() {
		// Arrange
		let throttle = UserRateThrottle::new(10, 60).unwrap();

		// Act & Assert
		let wait_time = throttle.wait_time("user1").await.unwrap();
		assert_eq!(wait_time, None);
	}

	#[rstest]
	#[tokio::test]
	async fn test_get_rate() {
		// Arrange
		let throttle = UserRateThrottle::new(100, 3600).unwrap();

		// Act
		let (rate, window) = throttle.get_rate();

		// Assert
		assert_eq!(rate, 100);
		assert_eq!(window, 3600);
	}

	#[rstest]
	fn test_new_rejects_zero_rate() {
		// Arrange & Act
		let result = UserRateThrottle::new(0, 60);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.err().unwrap(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_new_rejects_zero_window() {
		// Arrange & Act
		let result = UserRateThrottle::new(10, 0);

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.err().unwrap(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_with_backend_rejects_zero_rate() {
		// Arrange & Act
		let result = UserRateThrottle::with_backend(0, 60, MemoryBackend::new());

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.err().unwrap(),
			ThrottleError::InvalidConfig(_)
		));
	}

	#[rstest]
	fn test_with_backend_rejects_zero_window() {
		// Arrange & Act
		let result = UserRateThrottle::with_backend(10, 0, MemoryBackend::new());

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.err().unwrap(),
			ThrottleError::InvalidConfig(_)
		));
	}
}
