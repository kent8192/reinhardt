//! Session key rotation for enhanced security
//!
//! This module provides functionality to rotate session keys automatically
//! to prevent session fixation attacks and improve security.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::rotation::{RotationPolicy, SessionRotator};
//! use reinhardt_auth::sessions::Session;
//! use reinhardt_auth::sessions::backends::InMemorySessionBackend;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = InMemorySessionBackend::new();
//! let mut session = Session::new(backend.clone());
//!
//! // Set some data
//! session.set("user_id", 42)?;
//!
//! // Create rotator with policy
//! let rotator = SessionRotator::new(RotationPolicy::OnLogin);
//!
//! // Rotate session key
//! rotator.rotate(&mut session).await?;
//!
//! // Data is preserved, but key has changed
//! let user_id: i32 = session.get("user_id")?.unwrap();
//! assert_eq!(user_id, 42);
//! # Ok(())
//! # }
//! ```

#![allow(clippy::field_reassign_with_default)]
use super::backends::{SessionBackend, SessionError};
use super::session::Session;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::time::Duration;

/// Session rotation policy
///
/// Defines when session keys should be rotated.
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::rotation::RotationPolicy;
/// use std::time::Duration;
///
/// // Rotate on every login
/// let on_login = RotationPolicy::OnLogin;
///
/// // Rotate every hour
/// let periodic = RotationPolicy::Periodic(Duration::from_secs(3600));
///
/// // Rotate after specific number of requests
/// let after_requests = RotationPolicy::AfterRequests(100);
/// ```
#[derive(Debug, Clone)]
pub enum RotationPolicy {
	/// Rotate session key on user login
	OnLogin,
	/// Rotate session key periodically
	Periodic(Duration),
	/// Rotate session key after N requests
	AfterRequests(usize),
	/// Rotate on privilege escalation
	OnPrivilegeEscalation,
	/// Never rotate (not recommended for production)
	Never,
}

impl Default for RotationPolicy {
	/// Create default rotation policy
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::rotation::RotationPolicy;
	///
	/// let policy = RotationPolicy::default();
	/// // Default is OnLogin
	/// ```
	fn default() -> Self {
		Self::OnLogin
	}
}

/// Session rotation metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RotationMetadata {
	/// When the session key was last rotated
	pub last_rotation: DateTime<Utc>,
	/// Number of requests since last rotation
	pub request_count: usize,
}

impl Default for RotationMetadata {
	fn default() -> Self {
		Self {
			last_rotation: Utc::now(),
			request_count: 0,
		}
	}
}

/// Session rotator
///
/// Handles session key rotation based on configured policy.
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::rotation::{SessionRotator, RotationPolicy};
/// use reinhardt_auth::sessions::Session;
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = InMemorySessionBackend::new();
/// let mut session = Session::new(backend);
///
/// session.set("user_id", 123)?;
///
/// let rotator = SessionRotator::new(RotationPolicy::OnLogin);
/// rotator.rotate(&mut session).await?;
///
/// // Session key has changed but data is preserved
/// let user_id: i32 = session.get("user_id")?.unwrap();
/// assert_eq!(user_id, 123);
/// # Ok(())
/// # }
/// ```
pub struct SessionRotator {
	policy: RotationPolicy,
}

impl SessionRotator {
	/// Create a new session rotator with the given policy
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::rotation::{SessionRotator, RotationPolicy};
	/// use std::time::Duration;
	///
	/// let rotator = SessionRotator::new(RotationPolicy::Periodic(Duration::from_secs(3600)));
	/// ```
	pub fn new(policy: RotationPolicy) -> Self {
		Self { policy }
	}

	/// Rotate session key
	///
	/// This preserves all session data while changing the session key.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::rotation::{SessionRotator, RotationPolicy};
	/// use reinhardt_auth::sessions::Session;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let backend = InMemorySessionBackend::new();
	/// let mut session = Session::new(backend);
	///
	/// session.set("data", "value")?;
	/// let old_key = session.get_or_create_key().to_string();
	///
	/// let rotator = SessionRotator::new(RotationPolicy::OnLogin);
	/// rotator.rotate(&mut session).await?;
	///
	/// // Key has changed
	/// assert_ne!(session.get_or_create_key(), old_key);
	///
	/// // Data is preserved
	/// let data: String = session.get("data")?.unwrap();
	/// assert_eq!(data, "value");
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rotate<B: SessionBackend>(
		&self,
		session: &mut Session<B>,
	) -> Result<(), SessionError> {
		// Use the existing cycle_key method which preserves data
		session.cycle_key().await?;

		// Update rotation metadata
		let metadata = RotationMetadata::default();
		session
			.set("_rotation_metadata", metadata)
			.map_err(|e| SessionError::SerializationError(e.to_string()))?;

		Ok(())
	}

	/// Check if rotation is needed based on policy
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::rotation::{SessionRotator, RotationPolicy, RotationMetadata};
	/// use std::time::Duration;
	///
	/// let rotator = SessionRotator::new(RotationPolicy::AfterRequests(100));
	///
	/// let mut metadata = RotationMetadata::default();
	/// metadata.request_count = 99;
	/// assert!(!rotator.should_rotate(&metadata));
	///
	/// metadata.request_count = 100;
	/// assert!(rotator.should_rotate(&metadata));
	/// ```
	pub fn should_rotate(&self, metadata: &RotationMetadata) -> bool {
		match &self.policy {
			RotationPolicy::OnLogin => false, // Handled externally
			RotationPolicy::Periodic(duration) => {
				let elapsed = Utc::now() - metadata.last_rotation;
				elapsed > ChronoDuration::from_std(*duration).unwrap()
			}
			RotationPolicy::AfterRequests(max_requests) => metadata.request_count >= *max_requests,
			RotationPolicy::OnPrivilegeEscalation => false, // Handled externally
			RotationPolicy::Never => false,
		}
	}

	/// Increment request count in metadata
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::rotation::{SessionRotator, RotationMetadata};
	///
	/// let rotator = SessionRotator::default();
	/// let mut metadata = RotationMetadata::default();
	///
	/// assert_eq!(metadata.request_count, 0);
	/// rotator.increment_request_count(&mut metadata);
	/// assert_eq!(metadata.request_count, 1);
	/// ```
	pub fn increment_request_count(&self, metadata: &mut RotationMetadata) {
		metadata.request_count += 1;
	}
}

impl Default for SessionRotator {
	/// Create session rotator with default policy
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::rotation::SessionRotator;
	///
	/// let rotator = SessionRotator::default();
	/// ```
	fn default() -> Self {
		Self::new(RotationPolicy::default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::InMemorySessionBackend;
	use rstest::rstest;

	#[rstest]
	#[tokio::test]
	async fn test_rotation_policy_default() {
		let policy = RotationPolicy::default();
		match policy {
			RotationPolicy::OnLogin => {}
			_ => panic!("Expected OnLogin policy"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_rotation_metadata_default() {
		let metadata = RotationMetadata::default();
		assert_eq!(metadata.request_count, 0);
		assert!(metadata.last_rotation <= Utc::now());
	}

	#[rstest]
	#[tokio::test]
	async fn test_session_rotator_creation() {
		let _rotator = SessionRotator::new(RotationPolicy::OnLogin);
	}

	#[rstest]
	#[tokio::test]
	async fn test_session_rotator_default() {
		let _rotator = SessionRotator::default();
	}

	#[rstest]
	#[tokio::test]
	async fn test_rotate_session() {
		let backend = InMemorySessionBackend::new();
		let mut session = Session::new(backend);

		session.set("user_id", 123).unwrap();
		let old_key = session.get_or_create_key().to_string();

		let rotator = SessionRotator::new(RotationPolicy::OnLogin);
		rotator.rotate(&mut session).await.unwrap();

		// Key has changed
		assert_ne!(session.get_or_create_key(), old_key);

		// Data is preserved
		let user_id: i32 = session.get("user_id").unwrap().unwrap();
		assert_eq!(user_id, 123);
	}

	#[rstest]
	#[tokio::test]
	async fn test_should_rotate_periodic() {
		let rotator = SessionRotator::new(RotationPolicy::Periodic(Duration::from_secs(3600)));

		let metadata = RotationMetadata::default();
		// Just created, should not rotate
		assert!(!rotator.should_rotate(&metadata));

		// Old metadata, should rotate
		let old_metadata = RotationMetadata {
			last_rotation: Utc::now() - ChronoDuration::hours(2),
			request_count: 0,
		};
		assert!(rotator.should_rotate(&old_metadata));
	}

	#[rstest]
	#[tokio::test]
	async fn test_should_rotate_after_requests() {
		let rotator = SessionRotator::new(RotationPolicy::AfterRequests(100));

		let mut metadata = RotationMetadata::default();
		metadata.request_count = 99;
		assert!(!rotator.should_rotate(&metadata));

		metadata.request_count = 100;
		assert!(rotator.should_rotate(&metadata));

		metadata.request_count = 150;
		assert!(rotator.should_rotate(&metadata));
	}

	#[rstest]
	#[tokio::test]
	async fn test_should_rotate_never() {
		let rotator = SessionRotator::new(RotationPolicy::Never);

		let metadata = RotationMetadata::default();
		assert!(!rotator.should_rotate(&metadata));

		let old_metadata = RotationMetadata {
			last_rotation: Utc::now() - ChronoDuration::days(365),
			request_count: 1000000,
		};
		assert!(!rotator.should_rotate(&old_metadata));
	}

	#[rstest]
	#[tokio::test]
	async fn test_increment_request_count() {
		let rotator = SessionRotator::default();
		let mut metadata = RotationMetadata::default();

		assert_eq!(metadata.request_count, 0);
		rotator.increment_request_count(&mut metadata);
		assert_eq!(metadata.request_count, 1);
		rotator.increment_request_count(&mut metadata);
		assert_eq!(metadata.request_count, 2);
	}
}
