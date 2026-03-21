use super::backend::{MemoryBackend, ThrottleBackend};
use super::{Throttle, ThrottleResult};
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
	/// * `rate` - Maximum number of requests allowed
	/// * `window_secs` - Time window in seconds
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::throttling::AnonRateThrottle;
	///
	/// // Allow 60 requests per hour for anonymous users
	/// let throttle = AnonRateThrottle::new(60, 3600);
	/// assert_eq!(throttle.rate, 60);
	/// assert_eq!(throttle.window_secs, 3600);
	/// ```
	pub fn new(rate: usize, window_secs: u64) -> Self {
		Self {
			rate,
			window_secs,
			backend: MemoryBackend::new(),
		}
	}
}

impl<B: ThrottleBackend> AnonRateThrottle<B> {
	/// Creates a new `AnonRateThrottle` with a custom backend.
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
	/// use reinhardt_rest::throttling::{AnonRateThrottle, MemoryBackend};
	///
	/// let backend = MemoryBackend::new();
	/// let throttle = AnonRateThrottle::with_backend(60, 3600, backend);
	/// assert_eq!(throttle.rate, 60);
	/// assert_eq!(throttle.window_secs, 3600);
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
impl<B: ThrottleBackend> Throttle for AnonRateThrottle<B> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		let throttle_key = format!("throttle:anon:{}", key);
		let count = self
			.backend
			.increment(&throttle_key, self.window_secs)
			.await
			.map_err(super::ThrottleError::ThrottleError)?;
		Ok(count <= self.rate)
	}
	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
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

	#[tokio::test]
	async fn test_anon_throttle() {
		let throttle = AnonRateThrottle::new(5, 60);
		for _ in 0..5 {
			assert!(throttle.allow_request("192.168.1.1").await.unwrap());
		}
		assert!(!throttle.allow_request("192.168.1.1").await.unwrap());
	}

	#[tokio::test]
	async fn test_authenticated_user_not_affected() {
		// In the context of anon throttle, we test that different IPs are tracked separately
		let throttle = AnonRateThrottle::new(3, 60);

		// Fill up the limit for one IP
		for _ in 0..3 {
			assert!(throttle.allow_request("192.168.1.1").await.unwrap());
		}
		assert!(!throttle.allow_request("192.168.1.1").await.unwrap());

		// Different IP should not be affected
		assert!(throttle.allow_request("192.168.1.2").await.unwrap());
	}

	#[tokio::test]
	async fn test_get_cache_key_returns_correct_value() {
		let throttle = AnonRateThrottle::new(10, 60);

		// Test that requests are tracked properly by IP
		let ip1 = "10.0.0.1";
		let ip2 = "10.0.0.2";

		// Make requests from ip1
		for _ in 0..10 {
			assert!(throttle.allow_request(ip1).await.unwrap());
		}
		assert!(!throttle.allow_request(ip1).await.unwrap());

		// ip2 should still work
		assert!(throttle.allow_request(ip2).await.unwrap());
	}

	#[tokio::test]
	async fn test_accepts_request_under_limit() {
		let throttle = AnonRateThrottle::new(1, 86400); // 1 per day
		assert!(throttle.allow_request("3.3.3.3").await.unwrap());
	}

	#[tokio::test]
	async fn test_denies_request_over_limit() {
		let throttle = AnonRateThrottle::new(1, 86400); // 1 per day
		assert!(throttle.allow_request("3.3.3.3").await.unwrap());
		assert!(!throttle.allow_request("3.3.3.3").await.unwrap());
	}
}
