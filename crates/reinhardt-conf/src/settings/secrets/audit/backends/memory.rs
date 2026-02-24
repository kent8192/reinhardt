//! In-memory secret audit backend
//!
//! This backend stores secret access events in memory.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_conf::settings::secrets::audit::{SecretAuditBackend, SecretAccessEvent};
//! use reinhardt_conf::settings::secrets::audit::backends::MemorySecretAuditBackend;
//!
//! # async fn example() -> Result<(), String> {
//! let backend = MemorySecretAuditBackend::new();
//!
//! let event = SecretAccessEvent::new(
//!     "api_key".to_string(),
//!     "service".to_string(),
//!     true,
//!     None,
//! );
//!
//! backend.log_access(event).await?;
//!
//! let events = backend.get_accesses(None).await?;
//! assert_eq!(events.len(), 1);
//! # Ok(())
//! # }
//! ```

use crate::settings::secrets::audit::{SecretAccessEvent, SecretAccessFilter, SecretAuditBackend};
use parking_lot::RwLock;
use std::sync::Arc;

/// In-memory secret audit backend
///
/// ## Example
///
/// ```rust
/// use reinhardt_conf::settings::secrets::audit::backends::MemorySecretAuditBackend;
/// use reinhardt_conf::settings::secrets::audit::{SecretAuditBackend, SecretAccessEvent};
///
/// # async fn example() -> Result<(), String> {
/// let backend = MemorySecretAuditBackend::new();
///
/// let event = SecretAccessEvent::new("key".to_string(), "app".to_string(), true, None);
/// backend.log_access(event).await?;
/// # Ok(())
/// # }
/// ```
pub struct MemorySecretAuditBackend {
	events: Arc<RwLock<Vec<SecretAccessEvent>>>,
}

impl MemorySecretAuditBackend {
	/// Create a new memory secret audit backend
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::audit::backends::MemorySecretAuditBackend;
	///
	/// let backend = MemorySecretAuditBackend::new();
	/// ```
	pub fn new() -> Self {
		Self {
			events: Arc::new(RwLock::new(Vec::new())),
		}
	}

	/// Clear all stored events
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::audit::backends::MemorySecretAuditBackend;
	/// use reinhardt_conf::settings::secrets::audit::{SecretAuditBackend, SecretAccessEvent};
	///
	/// # async fn example() -> Result<(), String> {
	/// let backend = MemorySecretAuditBackend::new();
	///
	/// let event = SecretAccessEvent::new("key".to_string(), "app".to_string(), true, None);
	/// backend.log_access(event).await?;
	///
	/// backend.clear();
	///
	/// let events = backend.get_accesses(None).await?;
	/// assert_eq!(events.len(), 0);
	/// # Ok(())
	/// # }
	/// ```
	pub fn clear(&self) {
		self.events.write().clear();
	}

	/// Get the number of stored events
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::audit::backends::MemorySecretAuditBackend;
	///
	/// let backend = MemorySecretAuditBackend::new();
	/// assert_eq!(backend.len(), 0);
	/// ```
	pub fn len(&self) -> usize {
		self.events.read().len()
	}

	/// Check if the backend is empty
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::secrets::audit::backends::MemorySecretAuditBackend;
	///
	/// let backend = MemorySecretAuditBackend::new();
	/// assert!(backend.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.events.read().is_empty()
	}
}

impl Default for MemorySecretAuditBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl SecretAuditBackend for MemorySecretAuditBackend {
	async fn log_access(&self, event: SecretAccessEvent) -> Result<(), String> {
		self.events.write().push(event);
		Ok(())
	}

	async fn get_accesses(
		&self,
		filter: Option<SecretAccessFilter>,
	) -> Result<Vec<SecretAccessEvent>, String> {
		let events = self.events.read();

		if let Some(filter) = filter {
			let filtered: Vec<SecretAccessEvent> = events
				.iter()
				.filter(|event| {
					// Filter by secret name
					if let Some(ref secret_name) = filter.secret_name
						&& &event.secret_name != secret_name
					{
						return false;
					}

					// Filter by accessor
					if let Some(ref accessor) = filter.accessor
						&& &event.accessor != accessor
					{
						return false;
					}

					// Filter by success
					if let Some(success_only) = filter.success_only
						&& event.success != success_only
					{
						return false;
					}

					// Filter by start time
					if let Some(start_time) = filter.start_time
						&& event.timestamp < start_time
					{
						return false;
					}

					// Filter by end time
					if let Some(end_time) = filter.end_time
						&& event.timestamp > end_time
					{
						return false;
					}

					true
				})
				.cloned()
				.collect();

			Ok(filtered)
		} else {
			Ok(events.clone())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_memory_backend_new() {
		let backend = MemorySecretAuditBackend::new();
		assert!(backend.is_empty());
		assert_eq!(backend.len(), 0);
	}

	#[tokio::test]
	async fn test_memory_backend_log_access() {
		let backend = MemorySecretAuditBackend::new();

		let event =
			SecretAccessEvent::new("secret".to_string(), "accessor".to_string(), true, None);

		backend.log_access(event).await.unwrap();

		assert_eq!(backend.len(), 1);
		assert!(!backend.is_empty());
	}

	#[tokio::test]
	async fn test_memory_backend_get_accesses() {
		let backend = MemorySecretAuditBackend::new();

		for i in 0..5 {
			let event =
				SecretAccessEvent::new(format!("secret_{}", i), "accessor".to_string(), true, None);
			backend.log_access(event).await.unwrap();
		}

		let events = backend.get_accesses(None).await.unwrap();
		assert_eq!(events.len(), 5);
	}

	#[tokio::test]
	async fn test_memory_backend_filter_by_secret_name() {
		let backend = MemorySecretAuditBackend::new();

		let event1 =
			SecretAccessEvent::new("secret1".to_string(), "accessor".to_string(), true, None);
		let event2 =
			SecretAccessEvent::new("secret2".to_string(), "accessor".to_string(), true, None);
		let event3 =
			SecretAccessEvent::new("secret1".to_string(), "accessor".to_string(), true, None);

		backend.log_access(event1).await.unwrap();
		backend.log_access(event2).await.unwrap();
		backend.log_access(event3).await.unwrap();

		let filter = SecretAccessFilter {
			secret_name: Some("secret1".to_string()),
			..Default::default()
		};

		let events = backend.get_accesses(Some(filter)).await.unwrap();
		assert_eq!(events.len(), 2);
	}

	#[tokio::test]
	async fn test_memory_backend_filter_by_success() {
		let backend = MemorySecretAuditBackend::new();

		let event1 =
			SecretAccessEvent::new("secret".to_string(), "accessor".to_string(), true, None);
		let event2 =
			SecretAccessEvent::new("secret".to_string(), "accessor".to_string(), false, None);
		let event3 =
			SecretAccessEvent::new("secret".to_string(), "accessor".to_string(), true, None);

		backend.log_access(event1).await.unwrap();
		backend.log_access(event2).await.unwrap();
		backend.log_access(event3).await.unwrap();

		let filter = SecretAccessFilter {
			success_only: Some(true),
			..Default::default()
		};

		let events = backend.get_accesses(Some(filter)).await.unwrap();
		assert_eq!(events.len(), 2);
	}

	#[tokio::test]
	async fn test_memory_backend_clear() {
		let backend = MemorySecretAuditBackend::new();

		for _ in 0..3 {
			let event =
				SecretAccessEvent::new("secret".to_string(), "accessor".to_string(), true, None);
			backend.log_access(event).await.unwrap();
		}

		assert_eq!(backend.len(), 3);

		backend.clear();

		assert_eq!(backend.len(), 0);
		assert!(backend.is_empty());
	}
}
