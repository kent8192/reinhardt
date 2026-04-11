//! Burst rate throttling

use super::backend::ThrottleBackend;
use super::{Throttle, ThrottleError, ThrottleResult};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Burst rate throttle with sustained rate
pub struct BurstRateThrottle<B: ThrottleBackend> {
	backend: Arc<Mutex<B>>,
	burst_rate: usize,
	sustained_rate: usize,
	burst_duration: std::time::Duration,
	sustained_duration: std::time::Duration,
}

impl<B: ThrottleBackend> BurstRateThrottle<B> {
	/// Creates a new burst rate throttle with separate burst and sustained rates.
	///
	/// # Arguments
	///
	/// * `backend` - The throttle backend to use for storing request counts
	/// * `burst_rate` - Maximum requests allowed in burst period
	/// * `sustained_rate` - Maximum requests allowed in sustained period
	/// * `burst_duration` - Duration for burst rate window
	/// * `sustained_duration` - Duration for sustained rate window
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::burst::BurstRateThrottle;
	/// use reinhardt_throttling::backend::MemoryBackend;
	/// use std::sync::Arc;
	/// use tokio::sync::Mutex;
	/// use std::time::Duration;
	///
	/// let backend = Arc::new(Mutex::new(MemoryBackend::new()));
	/// let throttle = BurstRateThrottle::new(
	///     backend,
	///     10,  // 10 requests burst
	///     100, // 100 requests sustained
	///     Duration::from_secs(1),   // 1 second burst window
	///     Duration::from_secs(60),  // 60 second sustained window
	/// );
	/// ```
	pub fn new(
		backend: Arc<Mutex<B>>,
		burst_rate: usize,
		sustained_rate: usize,
		burst_duration: std::time::Duration,
		sustained_duration: std::time::Duration,
	) -> Self {
		Self {
			backend,
			burst_rate,
			sustained_rate,
			burst_duration,
			sustained_duration,
		}
	}
}

#[async_trait::async_trait]
impl<B: ThrottleBackend> Throttle for BurstRateThrottle<B> {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool> {
		let backend = self.backend.lock().await;

		let burst_key = format!("burst:{}", key);
		let sustained_key = format!("sustained:{}", key);

		// Check burst rate
		let burst_count = backend
			.get_count(&burst_key)
			.await
			.map_err(ThrottleError::ThrottleError)?;
		if burst_count >= self.burst_rate {
			return Ok(false);
		}

		// Check sustained rate
		let sustained_count = backend
			.get_count(&sustained_key)
			.await
			.map_err(ThrottleError::ThrottleError)?;
		if sustained_count >= self.sustained_rate {
			return Ok(false);
		}

		// Increment both counters
		backend
			.increment_duration(&burst_key, self.burst_duration)
			.await?;
		backend
			.increment_duration(&sustained_key, self.sustained_duration)
			.await?;

		Ok(true)
	}

	async fn wait_time(&self, key: &str) -> ThrottleResult<Option<u64>> {
		let backend = self.backend.lock().await;

		let burst_key = format!("burst:{}", key);
		let sustained_key = format!("sustained:{}", key);

		let burst_wait = backend.get_wait_time(&burst_key).await?;
		let sustained_wait = backend.get_wait_time(&sustained_key).await?;

		// Return the longer wait time of the two windows
		match (burst_wait, sustained_wait) {
			(Some(b), Some(s)) => Ok(Some(b.max(s).as_secs())),
			(Some(w), None) | (None, Some(w)) => Ok(Some(w.as_secs())),
			(None, None) => Ok(None),
		}
	}

	fn get_rate(&self) -> (usize, u64) {
		(self.sustained_rate, self.sustained_duration.as_secs())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::backend::MemoryBackend;
	use rstest::rstest;
	use std::time::Duration;

	#[rstest]
	#[tokio::test]
	async fn test_burst_and_sustained_use_distinct_keys() {
		// Arrange
		let backend = Arc::new(Mutex::new(MemoryBackend::new()));
		let throttle = BurstRateThrottle::new(
			backend.clone(),
			2,   // 2 requests burst
			100, // 100 requests sustained
			Duration::from_secs(1),
			Duration::from_secs(60),
		);

		// Act - send requests up to burst limit
		let first = throttle.allow_request("user1").await.unwrap();
		let second = throttle.allow_request("user1").await.unwrap();
		let third = throttle.allow_request("user1").await.unwrap();

		// Assert - burst limit blocks the third request
		assert!(first);
		assert!(second);
		assert!(!third);

		// Assert - verify backend stores with prefixed keys, not raw key
		let b = backend.lock().await;
		let raw_count = b.get_count("user1").await.unwrap();
		let burst_count = b.get_count("burst:user1").await.unwrap();
		let sustained_count = b.get_count("sustained:user1").await.unwrap();
		assert_eq!(raw_count, 0, "raw key should have no entries");
		assert_eq!(burst_count, 2, "burst key should track burst requests");
		assert_eq!(
			sustained_count, 2,
			"sustained key should track sustained requests"
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_wait_time_uses_prefixed_keys() {
		// Arrange
		let backend = Arc::new(Mutex::new(MemoryBackend::new()));
		let throttle = BurstRateThrottle::new(
			backend,
			5,
			100,
			Duration::from_secs(1),
			Duration::from_secs(60),
		);

		// Act - wait_time should query prefixed keys, not raw key
		let wait = throttle.wait_time("user1").await.unwrap();

		// Assert - default MemoryBackend returns None for wait_time
		assert_eq!(wait, None);
	}
}
