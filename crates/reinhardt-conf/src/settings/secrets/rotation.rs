//! Secret rotation functionality
//!
//! This module provides automatic secret rotation capabilities.
//!
//! ## Features
//!
//! - Automatic secret rotation on schedule
//! - Manual rotation triggers
//! - Rotation history tracking
//! - Multiple rotation strategies
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), String> {
//! let policy = RotationPolicy {
//!     interval: Duration::from_secs(86400), // Rotate daily
//!     max_age: Some(Duration::from_secs(604800)), // Max 7 days
//! };
//!
//! let rotation = SecretRotation::new(policy);
//!
//! // Check if rotation is needed
//! if rotation.should_rotate("api_key").await? {
//!     rotation.rotate("api_key").await?;
//! }
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Rotation policy for secrets
///
/// ## Example
///
/// ```rust
/// use reinhardt_conf::settings::secrets::rotation::RotationPolicy;
/// use std::time::Duration;
///
/// let policy = RotationPolicy {
///     interval: Duration::from_secs(3600),
///     max_age: Some(Duration::from_secs(7200)),
/// };
///
/// assert_eq!(policy.interval, Duration::from_secs(3600));
/// ```
#[derive(Debug, Clone)]
pub struct RotationPolicy {
	/// Rotation interval
	pub interval: Duration,
	/// Maximum age before forced rotation
	pub max_age: Option<Duration>,
}

impl Default for RotationPolicy {
	fn default() -> Self {
		Self {
			interval: Duration::from_secs(86400),       // 24 hours
			max_age: Some(Duration::from_secs(604800)), // 7 days
		}
	}
}

/// Rotation history entry
///
/// ## Example
///
/// ```rust
/// use reinhardt_conf::settings::secrets::rotation::RotationEntry;
///
/// let entry = RotationEntry::new("api_key".to_string(), "admin".to_string());
///
/// assert_eq!(entry.secret_name, "api_key");
/// assert_eq!(entry.rotated_by, "admin");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEntry {
	/// Timestamp of rotation
	pub timestamp: DateTime<Utc>,
	/// Name of the secret
	pub secret_name: String,
	/// Who triggered the rotation
	pub rotated_by: String,
	/// Optional reason
	pub reason: Option<String>,
}

impl RotationEntry {
	/// Create a new rotation entry
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::RotationEntry;
	///
	/// let entry = RotationEntry::new("secret".to_string(), "user".to_string());
	/// assert!(entry.timestamp <= chrono::Utc::now());
	/// ```
	pub fn new(secret_name: String, rotated_by: String) -> Self {
		Self {
			timestamp: Utc::now(),
			secret_name,
			rotated_by,
			reason: None,
		}
	}

	/// Create a rotation entry with a reason
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::RotationEntry;
	///
	/// let entry = RotationEntry::with_reason(
	///     "secret".to_string(),
	///     "user".to_string(),
	///     "Scheduled rotation".to_string(),
	/// );
	///
	/// assert_eq!(entry.reason, Some("Scheduled rotation".to_string()));
	/// ```
	pub fn with_reason(secret_name: String, rotated_by: String, reason: String) -> Self {
		Self {
			timestamp: Utc::now(),
			secret_name,
			rotated_by,
			reason: Some(reason),
		}
	}
}

/// Secret rotation manager
///
/// ## Example
///
/// ```rust
/// use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
/// use std::time::Duration;
///
/// let policy = RotationPolicy {
///     interval: Duration::from_secs(3600),
///     max_age: None,
/// };
///
/// let rotation = SecretRotation::new(policy);
/// ```
pub struct SecretRotation {
	policy: RotationPolicy,
	last_rotation: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
	history: Arc<RwLock<Vec<RotationEntry>>>,
}

impl SecretRotation {
	/// Create a new secret rotation manager
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
	///
	/// let rotation = SecretRotation::new(RotationPolicy::default());
	/// ```
	pub fn new(policy: RotationPolicy) -> Self {
		Self {
			policy,
			last_rotation: Arc::new(RwLock::new(HashMap::new())),
			history: Arc::new(RwLock::new(Vec::new())),
		}
	}

	/// Check if a secret should be rotated
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
	/// use std::time::Duration;
	///
	/// # async fn example() -> Result<(), String> {
	/// let policy = RotationPolicy {
	///     interval: Duration::from_secs(1),
	///     max_age: None,
	/// };
	///
	/// let rotation = SecretRotation::new(policy);
	///
	/// // First check should indicate rotation needed
	/// assert!(rotation.should_rotate("new_secret").await?);
	/// # Ok(())
	/// # }
	/// ```
	pub async fn should_rotate(&self, secret_name: &str) -> Result<bool, String> {
		let last_rotation = self.last_rotation.read();

		if let Some(last_time) = last_rotation.get(secret_name) {
			let elapsed = Utc::now() - *last_time;
			// Use saturating conversion to prevent negative duration wrap on clock rollback.
			// When the system clock goes backward (NTP adjustments, VM migration),
			// num_seconds() returns a negative value. Clamping to 0 prevents
			// wrapping to a very large u64 that would trigger unnecessary rotation.
			let elapsed_secs = elapsed.num_seconds().max(0) as u64;
			let elapsed_duration = Duration::from_secs(elapsed_secs);

			// Check against max age first
			if let Some(max_age) = self.policy.max_age
				&& elapsed_duration >= max_age
			{
				return Ok(true);
			}

			// Check against interval
			Ok(elapsed_duration >= self.policy.interval)
		} else {
			// No rotation history, should rotate
			Ok(true)
		}
	}

	/// Rotate a secret
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
	///
	/// # async fn example() -> Result<(), String> {
	/// let rotation = SecretRotation::new(RotationPolicy::default());
	///
	/// rotation.rotate("api_key").await?;
	///
	/// // Second rotation should fail due to interval
	/// assert!(rotation.rotate("api_key").await.is_err());
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rotate(&self, secret_name: &str) -> Result<(), String> {
		if !self.should_rotate(secret_name).await? {
			return Err(format!(
				"Secret '{}' does not need rotation yet",
				secret_name
			));
		}

		let entry = RotationEntry::new(secret_name.to_string(), "system".to_string());

		self.last_rotation
			.write()
			.insert(secret_name.to_string(), Utc::now());

		self.history.write().push(entry);

		Ok(())
	}

	/// Force rotate a secret, bypassing policy checks
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
	///
	/// # async fn example() -> Result<(), String> {
	/// let rotation = SecretRotation::new(RotationPolicy::default());
	///
	/// rotation.force_rotate("api_key", "admin", Some("Security breach".to_string())).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn force_rotate(
		&self,
		secret_name: &str,
		rotated_by: &str,
		reason: Option<String>,
	) -> Result<(), String> {
		let entry = if let Some(reason) = reason {
			RotationEntry::with_reason(secret_name.to_string(), rotated_by.to_string(), reason)
		} else {
			RotationEntry::new(secret_name.to_string(), rotated_by.to_string())
		};

		self.last_rotation
			.write()
			.insert(secret_name.to_string(), Utc::now());

		self.history.write().push(entry);

		Ok(())
	}

	/// Get rotation history
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
	///
	/// # async fn example() -> Result<(), String> {
	/// let rotation = SecretRotation::new(RotationPolicy::default());
	///
	/// rotation.force_rotate("key1", "admin", None).await?;
	/// rotation.force_rotate("key2", "admin", None).await?;
	///
	/// let history = rotation.get_history();
	/// assert_eq!(history.len(), 2);
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_history(&self) -> Vec<RotationEntry> {
		self.history.read().clone()
	}

	/// Get rotation history for a specific secret
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::rotation::{SecretRotation, RotationPolicy};
	///
	/// # async fn example() -> Result<(), String> {
	/// let rotation = SecretRotation::new(RotationPolicy::default());
	///
	/// rotation.force_rotate("key1", "admin", None).await?;
	/// rotation.force_rotate("key2", "admin", None).await?;
	/// rotation.force_rotate("key1", "admin", None).await?;
	///
	/// let key1_history = rotation.get_history_for_secret("key1");
	/// assert_eq!(key1_history.len(), 2);
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_history_for_secret(&self, secret_name: &str) -> Vec<RotationEntry> {
		self.history
			.read()
			.iter()
			.filter(|entry| entry.secret_name == secret_name)
			.cloned()
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_rotation_policy_default() {
		let policy = RotationPolicy::default();
		assert_eq!(policy.interval, Duration::from_secs(86400));
		assert_eq!(policy.max_age, Some(Duration::from_secs(604800)));
	}

	#[test]
	fn test_rotation_entry_new() {
		let entry = RotationEntry::new("secret".to_string(), "user".to_string());
		assert_eq!(entry.secret_name, "secret");
		assert_eq!(entry.rotated_by, "user");
		assert!(entry.reason.is_none());
		assert!(entry.timestamp <= Utc::now());
	}

	#[test]
	fn test_rotation_entry_with_reason() {
		let entry = RotationEntry::with_reason(
			"secret".to_string(),
			"user".to_string(),
			"reason".to_string(),
		);
		assert_eq!(entry.reason, Some("reason".to_string()));
	}

	#[tokio::test]
	async fn test_secret_rotation_new() {
		let rotation = SecretRotation::new(RotationPolicy::default());
		assert!(rotation.get_history().is_empty());
	}

	#[tokio::test]
	async fn test_should_rotate_new_secret() {
		let rotation = SecretRotation::new(RotationPolicy::default());
		assert!(rotation.should_rotate("new_secret").await.unwrap());
	}

	#[tokio::test]
	async fn test_rotate_secret() {
		let policy = RotationPolicy {
			interval: Duration::from_secs(1),
			max_age: None,
		};
		let rotation = SecretRotation::new(policy);

		rotation.rotate("secret").await.unwrap();

		let history = rotation.get_history();
		assert_eq!(history.len(), 1);
		assert_eq!(history[0].secret_name, "secret");
	}

	#[tokio::test]
	async fn test_rotate_too_soon() {
		let policy = RotationPolicy {
			interval: Duration::from_secs(3600),
			max_age: None,
		};
		let rotation = SecretRotation::new(policy);

		rotation.rotate("secret").await.unwrap();

		// Try to rotate again immediately
		let result = rotation.rotate("secret").await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_force_rotate() {
		let rotation = SecretRotation::new(RotationPolicy::default());

		rotation
			.force_rotate("secret", "admin", Some("Emergency".to_string()))
			.await
			.unwrap();

		let history = rotation.get_history();
		assert_eq!(history.len(), 1);
		assert_eq!(history[0].rotated_by, "admin");
		assert_eq!(history[0].reason, Some("Emergency".to_string()));
	}

	#[tokio::test]
	async fn test_get_history_for_secret() {
		let rotation = SecretRotation::new(RotationPolicy::default());

		rotation.force_rotate("key1", "admin", None).await.unwrap();
		rotation.force_rotate("key2", "admin", None).await.unwrap();
		rotation.force_rotate("key1", "admin", None).await.unwrap();

		let key1_history = rotation.get_history_for_secret("key1");
		assert_eq!(key1_history.len(), 2);

		let key2_history = rotation.get_history_for_secret("key2");
		assert_eq!(key2_history.len(), 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_should_rotate_negative_elapsed_does_not_trigger_rotation() {
		// Arrange: Create rotation with a long interval and simulate a clock rollback
		// by inserting a future timestamp as the last rotation time
		let policy = RotationPolicy {
			interval: Duration::from_secs(3600),
			max_age: None,
		};
		let rotation = SecretRotation::new(policy);

		// Simulate clock rollback: set last_rotation to a future timestamp
		let future_time = Utc::now() + chrono::Duration::seconds(600);
		rotation
			.last_rotation
			.write()
			.insert("clock_test".to_string(), future_time);

		// Act: Check if rotation is needed (elapsed will be negative)
		let should_rotate = rotation.should_rotate("clock_test").await.unwrap();

		// Assert: Negative elapsed time should NOT trigger rotation
		assert_eq!(should_rotate, false);
	}

	#[rstest]
	#[tokio::test]
	async fn test_should_rotate_negative_elapsed_with_max_age_does_not_trigger() {
		// Arrange: Create rotation with max_age and simulate a clock rollback
		let policy = RotationPolicy {
			interval: Duration::from_secs(3600),
			max_age: Some(Duration::from_secs(7200)),
		};
		let rotation = SecretRotation::new(policy);

		// Simulate clock rollback: set last_rotation to a future timestamp
		let future_time = Utc::now() + chrono::Duration::seconds(600);
		rotation
			.last_rotation
			.write()
			.insert("clock_test".to_string(), future_time);

		// Act: Check if rotation is needed (elapsed will be negative)
		let should_rotate = rotation.should_rotate("clock_test").await.unwrap();

		// Assert: Negative elapsed time should NOT trigger rotation even with max_age
		assert_eq!(should_rotate, false);
	}
}
