//! File-based audit backend
//!
//! This backend stores audit events in JSON lines format, suitable for production use.
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_conf::settings::audit::{AuditEvent, EventType, ChangeRecord};
//! use reinhardt_conf::settings::audit::backends::FileAuditBackend;
//! use reinhardt_conf::settings::audit::AuditBackend;
//! use std::collections::HashMap;
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), String> {
//! let backend = FileAuditBackend::new("audit.log")?;
//!
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
//! # Ok(())
//! # }
//! ```

use crate::settings::audit::{AuditBackend, AuditEvent, EventFilter};
use parking_lot::RwLock;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// File-based audit backend
///
/// Stores audit events in JSON lines format.
/// Each line contains one audit event as JSON.
///
/// ## Example
///
/// ```rust,no_run
/// use reinhardt_conf::settings::audit::backends::FileAuditBackend;
/// use reinhardt_conf::settings::audit::{AuditEvent, EventType, AuditBackend};
/// use std::collections::HashMap;
///
/// # async fn example() -> Result<(), String> {
/// let backend = FileAuditBackend::new("audit.log")?;
///
/// let event = AuditEvent::new(EventType::ConfigCreate, None, HashMap::new());
/// backend.log_event(event).await?;
/// # Ok(())
/// # }
/// ```
pub struct FileAuditBackend {
	path: PathBuf,
	file: Arc<RwLock<File>>,
}

impl FileAuditBackend {
	/// Create a new file audit backend
	///
	/// ## Arguments
	///
	/// * `path` - Path to the audit log file
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// use reinhardt_conf::settings::audit::backends::FileAuditBackend;
	///
	/// let backend = FileAuditBackend::new("audit.log").unwrap();
	/// ```
	pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, String> {
		let path = path.as_ref().to_path_buf();

		let file = OpenOptions::new()
			.create(true)
			.append(true)
			.read(true)
			.open(&path)
			.map_err(|e| format!("Failed to open audit file: {}", e))?;

		Ok(Self {
			path,
			file: Arc::new(RwLock::new(file)),
		})
	}

	/// Get the path to the audit log file
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// use reinhardt_conf::settings::audit::backends::FileAuditBackend;
	/// use std::path::Path;
	///
	/// let backend = FileAuditBackend::new("audit.log").unwrap();
	/// assert_eq!(backend.path(), Path::new("audit.log"));
	/// ```
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Read all events from the file
	fn read_events(&self) -> Result<Vec<AuditEvent>, String> {
		let file =
			File::open(&self.path).map_err(|e| format!("Failed to open audit file: {}", e))?;

		let reader = BufReader::new(file);
		let mut events = Vec::new();

		for line in reader.lines() {
			let line = line.map_err(|e| format!("Failed to read line: {}", e))?;

			if line.trim().is_empty() {
				continue;
			}

			let event: AuditEvent =
				serde_json::from_str(&line).map_err(|e| format!("Failed to parse event: {}", e))?;

			events.push(event);
		}

		Ok(events)
	}
}

#[async_trait::async_trait]
impl AuditBackend for FileAuditBackend {
	async fn log_event(&self, event: AuditEvent) -> Result<(), String> {
		let json = serde_json::to_string(&event)
			.map_err(|e| format!("Failed to serialize event: {}", e))?;

		let mut file = self.file.write();
		writeln!(file, "{}", json).map_err(|e| format!("Failed to write event: {}", e))?;
		file.flush()
			.map_err(|e| format!("Failed to flush file: {}", e))?;

		Ok(())
	}

	async fn get_events(&self, filter: Option<EventFilter>) -> Result<Vec<AuditEvent>, String> {
		let events = self.read_events()?;

		if let Some(filter) = filter {
			let filtered: Vec<AuditEvent> = events
				.into_iter()
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
				.collect();

			Ok(filtered)
		} else {
			Ok(events)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::audit::{ChangeRecord, EventType};
	use serde_json::json;
	use std::collections::HashMap;
	use tempfile::NamedTempFile;

	#[tokio::test]
	async fn test_file_backend_new() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileAuditBackend::new(temp_file.path()).unwrap();
		assert_eq!(backend.path(), temp_file.path());
	}

	#[tokio::test]
	async fn test_file_backend_log_event() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileAuditBackend::new(temp_file.path()).unwrap();

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

		let events = backend.get_events(None).await.unwrap();
		assert_eq!(events.len(), 1);
	}

	#[tokio::test]
	async fn test_file_backend_multiple_events() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileAuditBackend::new(temp_file.path()).unwrap();

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
	async fn test_file_backend_filter_by_event_type() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileAuditBackend::new(temp_file.path()).unwrap();

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
	async fn test_file_backend_filter_by_user() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileAuditBackend::new(temp_file.path()).unwrap();

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
	async fn test_file_backend_persistence() {
		let temp_file = NamedTempFile::new().unwrap();

		// Create first backend and log events
		{
			let backend = FileAuditBackend::new(temp_file.path()).unwrap();
			let event = AuditEvent::new(EventType::ConfigCreate, None, HashMap::new());
			backend.log_event(event).await.unwrap();
		}

		// Create second backend and verify events persist
		{
			let backend = FileAuditBackend::new(temp_file.path()).unwrap();
			let events = backend.get_events(None).await.unwrap();
			assert_eq!(events.len(), 1);
		}
	}
}
