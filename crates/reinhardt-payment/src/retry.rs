//! Retry strategy with exponential backoff.

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

/// Retries an operation with exponential backoff and jitter.
///
/// # Arguments
///
/// * `operation` - Async operation to retry
/// * `max_retries` - Maximum number of retry attempts
///
/// # Returns
///
/// Result of the operation or the last error
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_payment::retry::retry_with_backoff;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let result = retry_with_backoff(
///     || async {
///         // Your operation here
///         Ok::<_, String>("success")
///     },
///     3,
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn retry_with_backoff<F, Fut, T, E>(mut operation: F, max_retries: u32) -> Result<T, E>
where
	F: FnMut() -> Fut,
	Fut: Future<Output = Result<T, E>>,
{
	let mut attempt = 0;

	loop {
		match operation().await {
			Ok(result) => return Ok(result),
			Err(e) if attempt >= max_retries => return Err(e),
			Err(_) => {
				// Exponential backoff: 2^attempt seconds
				let base_delay = 2_u64.pow(attempt) * 1000;

				// Random jitter: 0-1000ms
				let jitter = rand::random::<u64>() % 1000;

				let delay = Duration::from_millis(base_delay + jitter);
				sleep(delay).await;

				attempt += 1;
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicU32, Ordering};

	#[tokio::test]
	async fn test_retry_success_on_first_attempt() {
		let result = retry_with_backoff(|| async { Ok::<_, String>("success") }, 3).await;
		assert_eq!(result.unwrap(), "success");
	}

	#[tokio::test]
	async fn test_retry_success_after_failures() {
		let counter = Arc::new(AtomicU32::new(0));
		let counter_clone = Arc::clone(&counter);

		let result = retry_with_backoff(
			move || {
				let counter = Arc::clone(&counter_clone);
				async move {
					let count = counter.fetch_add(1, Ordering::SeqCst);
					if count < 2 {
						Err("temporary failure")
					} else {
						Ok("success")
					}
				}
			},
			3,
		)
		.await;

		assert_eq!(result.unwrap(), "success");
		assert_eq!(counter.load(Ordering::SeqCst), 3);
	}

	#[tokio::test]
	async fn test_retry_max_retries_exceeded() {
		let counter = Arc::new(AtomicU32::new(0));
		let counter_clone = Arc::clone(&counter);

		let result = retry_with_backoff(
			move || {
				let counter = Arc::clone(&counter_clone);
				async move {
					counter.fetch_add(1, Ordering::SeqCst);
					Err::<(), _>("always fails")
				}
			},
			2,
		)
		.await;

		assert!(result.is_err());
		// max_retries=2 means: initial attempt + 2 retries = 3 total attempts
		assert_eq!(counter.load(Ordering::SeqCst), 3);
	}
}
