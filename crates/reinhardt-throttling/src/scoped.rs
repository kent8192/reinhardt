use super::backend::{MemoryBackend, ThrottleBackend};
use super::key_validation::{validate_key_component, validate_scope_key};
use super::{Throttle, ThrottleResult};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct ScopedRateThrottle<B: ThrottleBackend = MemoryBackend> {
	pub scopes: HashMap<String, (usize, u64)>,
	backend: B,
}

impl ScopedRateThrottle<MemoryBackend> {
	/// Creates a new `ScopedRateThrottle` with a default memory backend.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::ScopedRateThrottle;
	///
	/// let throttle = ScopedRateThrottle::new();
	/// assert_eq!(throttle.scopes.len(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			scopes: HashMap::new(),
			backend: MemoryBackend::new(),
		}
	}
}

impl Default for ScopedRateThrottle<MemoryBackend> {
	fn default() -> Self {
		Self::new()
	}
}

impl<B: ThrottleBackend> ScopedRateThrottle<B> {
	/// Creates a new `ScopedRateThrottle` with a custom backend.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::{ScopedRateThrottle, MemoryBackend};
	///
	/// let backend = MemoryBackend::new();
	/// let throttle = ScopedRateThrottle::with_backend(backend);
	/// assert_eq!(throttle.scopes.len(), 0);
	/// ```
	pub fn with_backend(backend: B) -> Self {
		Self {
			scopes: HashMap::new(),
			backend,
		}
	}

	/// Add a scope with rate limit configuration.
	///
	/// The scope name is validated to reject empty names, names containing
	/// control characters, the `:` delimiter, or exceeding the maximum length.
	///
	/// # Errors
	///
	/// Returns [`ThrottleError::InvalidKey`] if the scope name fails validation.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_throttling::ScopedRateThrottle;
	///
	/// let throttle = ScopedRateThrottle::new()
	///     .add_scope("api", 100, 60).unwrap()
	///     .add_scope("upload", 10, 60).unwrap();
	/// assert_eq!(throttle.scopes.len(), 2);
	/// assert_eq!(throttle.scopes.get("api"), Some(&(100, 60)));
	/// assert_eq!(throttle.scopes.get("upload"), Some(&(10, 60)));
	/// ```
	pub fn add_scope(
		mut self,
		scope: impl Into<String>,
		rate: usize,
		window: u64,
	) -> ThrottleResult<Self> {
		let scope = scope.into();
		validate_key_component(&scope)?;
		self.scopes.insert(scope, (rate, window));
		Ok(self)
	}
}

#[async_trait]
impl<B: ThrottleBackend> Throttle for ScopedRateThrottle<B> {
	async fn allow_request(&self, scope_key: &str) -> ThrottleResult<bool> {
		let (scope, identifier) = validate_scope_key(scope_key)?;

		if let Some(&(rate, window)) = self.scopes.get(scope) {
			let key = format!("throttle:scope:{}:{}", scope, identifier);
			let count = self
				.backend
				.increment(&key, window)
				.await
				.map_err(super::ThrottleError::ThrottleError)?;
			Ok(count <= rate)
		} else {
			Ok(true)
		}
	}

	async fn wait_time(&self, scope_key: &str) -> ThrottleResult<Option<u64>> {
		let (scope, identifier) = validate_scope_key(scope_key)?;

		if let Some(&(rate, window)) = self.scopes.get(scope) {
			let key = format!("throttle:scope:{}:{}", scope, identifier);
			let count = self
				.backend
				.get_count(&key)
				.await
				.map_err(super::ThrottleError::ThrottleError)?;
			if count > rate {
				Ok(Some(window))
			} else {
				Ok(None)
			}
		} else {
			Ok(None)
		}
	}

	fn get_rate(&self) -> (usize, u64) {
		(0, 0)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_scoped_throttle() {
		// Arrange
		let throttle = ScopedRateThrottle::new()
			.add_scope("api", 100, 60)
			.unwrap()
			.add_scope("upload", 10, 60)
			.unwrap();

		// Act & Assert
		for _ in 0..10 {
			assert!(throttle.allow_request("upload:user1").await.unwrap());
		}
		assert!(!throttle.allow_request("upload:user1").await.unwrap());
		assert!(throttle.allow_request("api:user1").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_scoped_rate_throttle() {
		// Arrange
		let throttle = ScopedRateThrottle::new()
			.add_scope("x", 3, 60)
			.unwrap()
			.add_scope("y", 1, 60)
			.unwrap();

		// Act & Assert - should be able to hit x scope 3 times
		assert!(throttle.allow_request("x:req1").await.unwrap());
		assert!(throttle.allow_request("x:req1").await.unwrap());
		assert!(throttle.allow_request("x:req1").await.unwrap());

		// Fourth request should be throttled
		assert!(!throttle.allow_request("x:req1").await.unwrap());

		// Should be able to hit y scope 1 time
		assert!(throttle.allow_request("y:req1").await.unwrap());
		assert!(!throttle.allow_request("y:req1").await.unwrap());

		// Different identifier should have separate limit
		assert!(throttle.allow_request("x:req2").await.unwrap());
		assert!(throttle.allow_request("y:req2").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_scoped_rate_throttle_with_time() {
		// Arrange
		use crate::backend::MemoryBackend;
		use crate::time_provider::MockTimeProvider;
		use std::sync::Arc;
		use tokio::time::Instant;

		let mock_time = Arc::new(MockTimeProvider::new(Instant::now()));
		let backend = MemoryBackend::with_time_provider(mock_time.clone());

		let throttle = ScopedRateThrottle::with_backend(backend)
			.add_scope("api", 2, 5)
			.unwrap();

		// Act & Assert - fill up the limit
		assert!(throttle.allow_request("api:user1").await.unwrap());
		assert!(throttle.allow_request("api:user1").await.unwrap());

		// Should be throttled
		assert!(!throttle.allow_request("api:user1").await.unwrap());

		// Advance time past the window
		mock_time.advance(std::time::Duration::from_secs(6));

		// Should be allowed again after window expires
		assert!(throttle.allow_request("api:user1").await.unwrap());
		assert!(throttle.allow_request("api:user1").await.unwrap());

		// Third request should be throttled
		assert!(!throttle.allow_request("api:user1").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_unscoped_view_not_throttled() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("x", 3, 60).unwrap();

		// Act & Assert - requests with unknown scope should not be throttled
		for _ in 0..10 {
			assert!(throttle.allow_request("unknown:req1").await.unwrap());
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_invalid_format_returns_error() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("x", 3, 60).unwrap();

		// Act
		let result = throttle.allow_request("invalid_format").await;

		// Assert - invalid format should return an error
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_get_cache_key_returns_correct_key() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("user", 10, 60).unwrap();

		// Act & Assert - different users get different keys
		assert!(throttle.allow_request("user:123").await.unwrap());
		assert!(throttle.allow_request("user:456").await.unwrap());

		// Verify they are tracked separately
		for _ in 0..9 {
			assert!(throttle.allow_request("user:123").await.unwrap());
		}
		// user:123 should now be at limit
		assert!(!throttle.allow_request("user:123").await.unwrap());

		// user:456 should still have capacity
		assert!(throttle.allow_request("user:456").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_key_with_null_byte() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();

		// Act
		let result = throttle.allow_request("api:user\0id").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_key_with_control_characters() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();

		// Act
		let result = throttle.allow_request("api:user\nid").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_crafted_collision_key() {
		// Arrange - attacker tries to use colons in the identifier to collide
		// with another scope's internal cache key format
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();

		// Act - "api:admin:secret" would split on first `:` giving identifier "admin:secret"
		// which contains `:` and should be rejected
		let result = throttle.allow_request("api:admin:secret").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_empty_scope_in_key() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();

		// Act
		let result = throttle.allow_request(":user123").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_empty_identifier_in_key() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();

		// Act
		let result = throttle.allow_request("api:").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	fn test_add_scope_rejects_invalid_scope_name() {
		// Arrange & Act
		let result = ScopedRateThrottle::new().add_scope("scope:name", 10, 60);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.err()
				.unwrap()
				.to_string()
				.contains("key component must not contain ':' delimiter")
		);
	}

	#[rstest]
	fn test_add_scope_rejects_empty_scope_name() {
		// Arrange & Act
		let result = ScopedRateThrottle::new().add_scope("", 10, 60);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.err()
				.unwrap()
				.to_string()
				.contains("key component must not be empty")
		);
	}

	#[rstest]
	fn test_add_scope_rejects_scope_with_control_chars() {
		// Arrange & Act
		let result = ScopedRateThrottle::new().add_scope("api\0scope", 10, 60);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.err()
				.unwrap()
				.to_string()
				.contains("key component must not contain control characters")
		);
	}

	#[rstest]
	#[tokio::test]
	async fn test_reject_excessively_long_identifier() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();
		let long_id = "a".repeat(257);
		let key = format!("api:{}", long_id);

		// Act
		let result = throttle.allow_request(&key).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_wait_time_validates_key() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();

		// Act - invalid format
		let result = throttle.wait_time("invalid_format").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}

	#[rstest]
	#[tokio::test]
	async fn test_wait_time_validates_control_chars() {
		// Arrange
		let throttle = ScopedRateThrottle::new().add_scope("api", 10, 60).unwrap();

		// Act - key with control chars
		let result = throttle.wait_time("api:user\0id").await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(
			result.unwrap_err(),
			super::super::ThrottleError::InvalidKey(_)
		));
	}
}
