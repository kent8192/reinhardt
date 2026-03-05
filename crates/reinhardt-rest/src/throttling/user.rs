use super::backend::{MemoryBackend, ThrottleBackend};
use super::{Throttle, ThrottleResult};
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
	/// * `rate` - Maximum number of requests allowed
	/// * `window_secs` - Time window in seconds
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::UserRateThrottle;
	///
	/// // Allow 100 requests per 60 seconds per user
	/// let throttle = UserRateThrottle::new(100, 60);
	/// assert_eq!(throttle.rate, 100);
	/// assert_eq!(throttle.window_secs, 60);
	/// ```
	pub fn new(rate: usize, window_secs: u64) -> Self {
		Self {
			rate,
			window_secs,
			backend: MemoryBackend::new(),
		}
	}
}

impl<B: ThrottleBackend> UserRateThrottle<B> {
	/// Creates a new `UserRateThrottle` with a custom backend.
	///
	/// # Arguments
	///
	/// * `rate` - Maximum number of requests allowed
	/// * `window_secs` - Time window in seconds
	/// * `backend` - Custom throttle backend
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::{UserRateThrottle, MemoryBackend};
	///
	/// let backend = MemoryBackend::new();
	/// let throttle = UserRateThrottle::with_backend(100, 60, backend);
	/// assert_eq!(throttle.rate, 100);
	/// assert_eq!(throttle.window_secs, 60);
	/// ```
	pub fn with_backend(rate: usize, window_secs: u64, backend: B) -> Self {
		Self {
			rate,
			window_secs,
			backend,
		}
	}
}

#[async_trait]
impl<B: ThrottleBackend> Throttle for UserRateThrottle<B> {
	async fn allow_request(&self, user_id: &str) -> ThrottleResult<bool> {
		let key = format!("throttle:user:{}", user_id);
		let count = self
			.backend
			.increment(&key, self.window_secs)
			.await
			.map_err(super::ThrottleError::ThrottleError)?;
		Ok(count <= self.rate)
	}
	async fn wait_time(&self, user_id: &str) -> ThrottleResult<Option<u64>> {
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

	#[tokio::test]
	async fn test_user_throttle() {
		let throttle = UserRateThrottle::new(10, 60);
		for _ in 0..10 {
			assert!(throttle.allow_request("user123").await.unwrap());
		}
		assert!(!throttle.allow_request("user123").await.unwrap());
	}

	#[tokio::test]
	async fn test_requests_are_throttled() {
		// Ensure request rate is limited
		let throttle = UserRateThrottle::new(3, 1);
		for _ in 0..3 {
			assert!(throttle.allow_request("user1").await.unwrap());
		}
		// Fourth request should be throttled
		assert!(!throttle.allow_request("user1").await.unwrap());
	}

	#[tokio::test]
	async fn test_request_throttling_expires() {
		use crate::throttling::backend::MemoryBackend;
		use crate::throttling::time_provider::MockTimeProvider;
		use std::sync::Arc;
		use tokio::time::Instant;

		let mock_time = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = MemoryBackend::with_time_provider(mock_time.clone());
		let throttle = UserRateThrottle::with_backend(3, 1, backend);

		// Fill up the limit
		for _ in 0..3 {
			assert!(throttle.allow_request("user1").await.unwrap());
		}
		// Should be throttled
		assert!(!throttle.allow_request("user1").await.unwrap());

		// Advance time by 2 seconds (past the 1-second window)
		mock_time.advance(std::time::Duration::from_secs(2));

		// Should be allowed again after window expires
		assert!(throttle.allow_request("user1").await.unwrap());
		assert!(throttle.allow_request("user1").await.unwrap());
		assert!(throttle.allow_request("user1").await.unwrap());
		// Fourth request should be throttled again
		assert!(!throttle.allow_request("user1").await.unwrap());
	}

	#[tokio::test]
	async fn test_request_throttling_is_per_user() {
		// Ensure request rate is only limited per user
		let throttle = UserRateThrottle::new(3, 1);

		for _ in 0..3 {
			assert!(throttle.allow_request("user_a").await.unwrap());
		}

		// user_b should not be affected by user_a's limit
		assert!(throttle.allow_request("user_b").await.unwrap());
	}

	#[tokio::test]
	async fn test_wait_returns_correct_waiting_time() {
		let throttle = UserRateThrottle::new(1, 60);

		assert!(throttle.allow_request("user1").await.unwrap());
		assert!(!throttle.allow_request("user1").await.unwrap());

		let wait_time = throttle.wait_time("user1").await.unwrap();
		assert_eq!(wait_time, Some(60));
	}

	#[tokio::test]
	async fn test_wait_returns_none_when_under_limit() {
		let throttle = UserRateThrottle::new(10, 60);

		let wait_time = throttle.wait_time("user1").await.unwrap();
		assert_eq!(wait_time, None);
	}

	#[tokio::test]
	async fn test_get_rate() {
		let throttle = UserRateThrottle::new(100, 3600);
		let (rate, window) = throttle.get_rate();
		assert_eq!(rate, 100);
		assert_eq!(window, 3600);
	}
}
