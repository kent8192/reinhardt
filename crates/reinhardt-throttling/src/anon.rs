use super::backend::{MemoryBackend, ThrottleBackend};
use super::key_validation::validate_key_component;
use super::{Throttle, ThrottleError, ThrottleResult};
use async_trait::async_trait;

pub struct AnonRateThrottle<B: ThrottleBackend = MemoryBackend> {
	pub rate: usize,
	pub window_secs: u64,
	backend: B,
}

impl AnonRateThrottle<MemoryBackend> {
	/// Creates a new `AnonRateThrottle` with a memory backend.
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
	/// use reinhardt_throttling::AnonRateThrottle;
	///
	/// // Allow 60 requests per hour for anonymous users
	/// let throttle = AnonRateThrottle::new(60, 3600).unwrap();
	/// assert_eq!(throttle.rate, 60);
	/// assert_eq!(throttle.window_secs, 3600);
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

impl<B: ThrottleBackend> AnonRateThrottle<B> {
	/// Creates a new `AnonRateThrottle` with a custom backend.
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
	/// use reinhardt_throttling::{AnonRateThrottle, MemoryBackend};
	///
	/// let backend = MemoryBackend::new();
	/// let throttle = AnonRateThrottle::with_backend(60, 3600, backend).unwrap();
	/// assert_eq!(throttle.rate, 60);
	/// assert_eq!(throttle.window_secs, 3600);
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
impl<B: ThrottleBackend> Throttle for AnonRateThrottle<B> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		validate_key_component(key)?;
		let throttle_key = format!("throttle:anon:{}", key);
		let count = self
			.backend
			.increment(&throttle_key, self.window_secs)
			.await
			.map_err(super::ThrottleError::ThrottleError)?;
		Ok(count <= self.rate)
	}
	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		validate_key_component(key)?;
		let throttle_key = format!("throttle:anon:{}", key);
		let count = self
			.backend
			.get_count(&throttle_key)
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
	async fn test_anon_throttle() {
		// Arrange
		let throttle = AnonRateThrottle::new(5, 60).unwrap();

		// Act & Assert
		for _ in 0..5 {
			assert!(throttle.allow_request("192.168.1.1").await.unwrap());
		}
		assert!(!throttle.allow_request("192.168.1.1").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticated_user_not_affected() {
		// Arrange
		let throttle = AnonRateThrottle::new(3, 60).unwrap();

		// Act - fill up the limit for one IP
		for _ in 0..3 {
			assert!(throttle.allow_request("192.168.1.1").await.unwrap());
		}
		assert!(!throttle.allow_request("192.168.1.1").await.unwrap());

		// Assert - different IP should not be affected
		assert!(throttle.allow_request("192.168.1.2").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_get_cache_key_returns_correct_value() {
		// Arrange
		let throttle = AnonRateThrottle::new(10, 60).unwrap();
		let ip1 = "10.0.0.1";
		let ip2 = "10.0.0.2";

		// Act - make requests from ip1
		for _ in 0..10 {
			assert!(throttle.allow_request(ip1).await.unwrap());
		}

		// Assert
		assert!(!throttle.allow_request(ip1).await.unwrap());
		assert!(throttle.allow_request(ip2).await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_accepts_request_under_limit() {
		// Arrange
		let throttle = AnonRateThrottle::new(1, 86400).unwrap();

		// Act & Assert
		assert!(throttle.allow_request("3.3.3.3").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_denies_request_over_limit() {
		// Arrange
		let throttle = AnonRateThrottle::new(1, 86400).unwrap();

		// Act & Assert
		assert!(throttle.allow_request("3.3.3.3").await.unwrap());
		assert!(!throttle.allow_request("3.3.3.3").await.unwrap());
	}

	#[rstest]
	fn test_new_rejects_zero_rate() {
		// Arrange & Act
		let result = AnonRateThrottle::new(0, 60);

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
		let result = AnonRateThrottle::new(10, 0);

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
		let result = AnonRateThrottle::with_backend(0, 60, MemoryBackend::new());

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
		let result = AnonRateThrottle::with_backend(10, 0, MemoryBackend::new());

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.err().unwrap(),
			ThrottleError::InvalidConfig(_)
		));
	}
}
