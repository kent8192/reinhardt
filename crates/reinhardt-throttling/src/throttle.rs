use async_trait::async_trait;
use thiserror::Error;

/// Error types for throttle operations.
#[derive(Debug, Error)]
pub enum ThrottleError {
	/// The rate limit has been exceeded.
	#[error("Rate limit exceeded")]
	RateLimitExceeded,
	/// A backend or internal throttle error occurred.
	#[error("Throttle error: {0}")]
	ThrottleError(String),
	/// The throttle configuration is invalid.
	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),
	/// The provided throttle key is invalid.
	#[error("Invalid key: {0}")]
	InvalidKey(String),
}

/// A specialized `Result` type for throttle operations.
pub type ThrottleResult<T> = Result<T, ThrottleError>;

/// Core trait for rate-limiting strategies.
#[async_trait]
pub trait Throttle: Send + Sync {
	/// Check whether a request identified by `key` should be allowed.
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool>;

	/// Return the number of seconds the caller should wait before retrying.
	///
	/// Returns `None` if the caller is not rate-limited.
	async fn wait_time(&self, _key: &str) -> ThrottleResult<Option<u64>> {
		Ok(None)
	}

	/// Return the rate limit configuration as (max_requests, window_seconds).
	fn get_rate(&self) -> (usize, u64);
}

#[cfg(test)]
mod tests {
	use super::*;

	struct MockThrottle;

	#[async_trait]
	impl Throttle for MockThrottle {
		async fn allow_request(&self, _key: &str) -> ThrottleResult<bool> {
			Err(ThrottleError::RateLimitExceeded)
		}

		async fn wait_time(&self, _key: &str) -> ThrottleResult<Option<u64>> {
			Ok(Some(60))
		}

		fn get_rate(&self) -> (usize, u64) {
			(10, 60)
		}
	}

	#[tokio::test]
	async fn test_allow_request_raises_not_implemented_error() {
		// Test that base throttle behavior can return error
		let throttle = MockThrottle;
		let result = throttle.allow_request("test_key").await;
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			ThrottleError::RateLimitExceeded
		));
	}
}
