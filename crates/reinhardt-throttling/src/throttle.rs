use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThrottleError {
	#[error("Rate limit exceeded")]
	RateLimitExceeded,
	#[error("Throttle error: {0}")]
	ThrottleError(String),
	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),
	#[error("Invalid key: {0}")]
	InvalidKey(String),
}

pub type ThrottleResult<T> = Result<T, ThrottleError>;

#[async_trait]
pub trait Throttle: Send + Sync {
	async fn allow_request(&self, key: &str) -> ThrottleResult<bool>;

	async fn wait_time(&self, _key: &str) -> ThrottleResult<Option<u64>> {
		Ok(None)
	}

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
