//! In-memory audit backend
//!
//! This backend stores audit events in memory, useful for testing and temporary storage.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_conf::settings::audit::{AuditEvent, EventType, EventFilter, ChangeRecord};
//! use reinhardt_conf::settings::audit::backends::MemoryAuditBackend;
//! use reinhardt_conf::settings::audit::AuditBackend;
//! use std::collections::HashMap;
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), String> {
//! let backend = MemoryAuditBackend::new();
//!
//! // Log an event
//! let mut changes = HashMap::new();
//! changes.insert(
//!     "setting".to_string(),
//!     ChangeRecord {
//!         old_value: Some(json!(false)),
//!         new_value: Some(json!(true)),
//!     },
//! );
//!
//! let event = AuditEvent::new(EventType::ConfigUpdate, Some("admin".to_string()), changes);
//! backend.log_event(event).await?;
//!
//! // Retrieve events
//! let events = backend.get_events(None).await?;
//! assert_eq!(events.len(), 1);
//! # Ok(())
//! # }
//! ```

use crate::settings::audit::{AuditBackend, AuditEvent, EventFilter};
use parking_lot::RwLock;
use std::sync::Arc;

/// In-memory audit backend
///
/// Stores audit events in memory using a thread-safe Vec.
/// Events are lost when the application restarts.
///
/// ## Example
///
/// ```rust
/// use reinhardt_conf::settings::audit::backends::MemoryAuditBackend;
/// use reinhardt_conf::settings::audit::{AuditEvent, EventType};
/// use reinhardt_conf::settings::audit::AuditBackend;
/// use std::collections::HashMap;
///
/// # async fn example() -> Result<(), String> {
/// let backend = MemoryAuditBackend::new();
///
/// let event = AuditEvent::new(EventType::ConfigCreate, None, HashMap::new());
/// backend.log_event(event).await?;
///
/// let events = backend.get_events(None).await?;
/// assert_eq!(events.len(), 1);
/// # Ok(())
/// # }
/// ```
pub struct MemoryAuditBackend {
	events: Arc<RwLock<Vec<AuditEvent>>>,
}

impl MemoryAuditBackend {
	/// Create a new memory audit backend
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::audit::backends::MemoryAuditBackend;
	///
	/// let backend = MemoryAuditBackend::new();
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
	/// use reinhardt_conf::settings::audit::backends::MemoryAuditBackend;
	/// use reinhardt_conf::settings::audit::{AuditEvent, EventType, AuditBackend};
	/// use std::collections::HashMap;
	///
	/// # async fn example() -> Result<(), String> {
	/// let backend = MemoryAuditBackend::new();
	///
	/// let event = AuditEvent::new(EventType::ConfigCreate, None, HashMap::new());
	/// backend.log_event(event).await?;
	///
	/// backend.clear();
	///
	/// let events = backend.get_events(None).await?;
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
	/// use reinhardt_conf::settings::audit::backends::MemoryAuditBackend;
	/// use reinhardt_conf::settings::audit::{AuditEvent, EventType, AuditBackend};
	/// use std::collections::HashMap;
	///
	/// # async fn example() -> Result<(), String> {
	/// let backend = MemoryAuditBackend::new();
	///
	/// assert_eq!(backend.len(), 0);
	///
	/// let event = AuditEvent::new(EventType::ConfigCreate, None, HashMap::new());
	/// backend.log_event(event).await?;
	///
	/// assert_eq!(backend.len(), 1);
	/// # Ok(())
	/// # }
	/// ```
	pub fn len(&self) -> usize {
		self.events.read().len()
	}

	/// Check if the backend is empty
	///
	/// ## Example
	///
	/// ```rust
	/// use reinhardt_conf::settings::audit::backends::MemoryAuditBackend;
	///
	/// let backend = MemoryAuditBackend::new();
	/// assert!(backend.is_empty());
	/// ```
	pub fn is_empty(&self) -> bool {
		self.events.read().is_empty()
	}
}

impl Default for MemoryAuditBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl AuditBackend for MemoryAuditBackend {
	async fn log_event(&self, event: AuditEvent) -> Result<(), String> {
		self.events.write().push(event);
		Ok(())
	}

	async fn get_events(&self, filter: Option<EventFilter>) -> Result<Vec<AuditEvent>, String> {
		let events = self.events.read();

		if let Some(filter) = filter {
			let filtered: Vec<AuditEvent> = events
				.iter()
				.filter(|event| {
					// Filter by event type
					if let Some(ref event_type) = filter.event_type
						&& &event.event_type != event_type
					{
						return false;
					}

					// Filter by user
					if let Some(ref user) = filter.user
						&& event.user.as_ref() != Some(user)
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
	use crate::settings::audit::{ChangeRecord, EventType};
	use serde_json::json;
	use std::collections::HashMap;

	#[tokio::test]
	async fn test_memory_backend_new() {
		let backend = MemoryAuditBackend::new();
		assert!(backend.is_empty());
		assert_eq!(backend.len(), 0);
	}

	#[tokio::test]
	async fn test_memory_backend_log_event() {
		let backend = MemoryAuditBackend::new();

		let mut changes = HashMap::new();
		changes.insert(
			"test".to_string(),
			ChangeRecord {
				old_value: Some(json!(1)),
				new_value: Some(json!(2)),
			},
		);

		let event = AuditEvent::new(EventType::ConfigUpdate, Some("user".to_string()), changes);

		backend.log_event(event).await.unwrap();

		assert_eq!(backend.len(), 1);
		assert!(!backend.is_empty());
	}

	#[tokio::test]
	async fn test_memory_backend_get_events() {
		let backend = MemoryAuditBackend::new();

		// Log multiple events
		for i in 0..5 {
			let mut changes = HashMap::new();
			changes.insert(
				format!("key_{}", i),
				ChangeRecord {
					old_value: None,
					new_value: Some(json!(i)),
				},
			);

			let event = AuditEvent::new(EventType::ConfigCreate, Some("user".to_string()), changes);
			backend.log_event(event).await.unwrap();
		}

		let events = backend.get_events(None).await.unwrap();
		assert_eq!(events.len(), 5);
	}

	#[tokio::test]
	async fn test_memory_backend_filter_by_event_type() {
		let backend = MemoryAuditBackend::new();

		// Log different event types
		let event1 = AuditEvent::new(EventType::ConfigCreate, None, HashMap::new());
		let event2 = AuditEvent::new(EventType::ConfigUpdate, None, HashMap::new());
		let event3 = AuditEvent::new(EventType::ConfigDelete, None, HashMap::new());

		backend.log_event(event1).await.unwrap();
		backend.log_event(event2).await.unwrap();
		backend.log_event(event3).await.unwrap();

		let filter = EventFilter {
			event_type: Some(EventType::ConfigUpdate),
			..Default::default()
		};

		let events = backend.get_events(Some(filter)).await.unwrap();
		assert_eq!(events.len(), 1);
		assert_eq!(events[0].event_type, EventType::ConfigUpdate);
	}

	#[tokio::test]
	async fn test_memory_backend_filter_by_user() {
		let backend = MemoryAuditBackend::new();

		// Log events with different users
		let event1 = AuditEvent::new(
			EventType::ConfigCreate,
			Some("alice".to_string()),
			HashMap::new(),
		);
		let event2 = AuditEvent::new(
			EventType::ConfigCreate,
			Some("bob".to_string()),
			HashMap::new(),
		);
		let event3 = AuditEvent::new(
			EventType::ConfigCreate,
			Some("alice".to_string()),
			HashMap::new(),
		);

		backend.log_event(event1).await.unwrap();
		backend.log_event(event2).await.unwrap();
		backend.log_event(event3).await.unwrap();

		let filter = EventFilter {
			user: Some("alice".to_string()),
			..Default::default()
		};

		let events = backend.get_events(Some(filter)).await.unwrap();
		assert_eq!(events.len(), 2);
	}

	#[tokio::test]
	async fn test_memory_backend_clear() {
		let backend = MemoryAuditBackend::new();

		// Log some events
		for _ in 0..3 {
			let event = AuditEvent::new(EventType::ConfigCreate, None, HashMap::new());
			backend.log_event(event).await.unwrap();
		}

		assert_eq!(backend.len(), 3);

		backend.clear();

		assert_eq!(backend.len(), 0);
		assert!(backend.is_empty());
	}
}
