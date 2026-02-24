//! File-based secret audit backend
//!
//! This backend stores secret access events in JSON lines format.
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_conf::settings::secrets::audit::{SecretAuditBackend, SecretAccessEvent};
//! use reinhardt_conf::settings::secrets::audit::backends::FileSecretAuditBackend;
//!
//! # async fn example() -> Result<(), String> {
//! let backend = FileSecretAuditBackend::new("secret_audit.log")?;
//!
//! let event = SecretAccessEvent::new(
//!     "api_key".to_string(),
//!     "service".to_string(),
//!     true,
//!     None,
//! );
//!
//! backend.log_access(event).await?;
//! # Ok(())
//! # }
//! ```

use crate::settings::secrets::audit::{SecretAccessEvent, SecretAccessFilter, SecretAuditBackend};
use parking_lot::RwLock;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// File-based secret audit backend
///
/// ## Example
///
/// ```rust,no_run
/// use reinhardt_conf::settings::secrets::audit::backends::FileSecretAuditBackend;
/// use reinhardt_conf::settings::secrets::audit::{SecretAuditBackend, SecretAccessEvent};
///
/// # async fn example() -> Result<(), String> {
/// let backend = FileSecretAuditBackend::new("audit.log")?;
///
/// let event = SecretAccessEvent::new("key".to_string(), "app".to_string(), true, None);
/// backend.log_access(event).await?;
/// # Ok(())
/// # }
/// ```
pub struct FileSecretAuditBackend {
	path: PathBuf,
	file: Arc<RwLock<File>>,
}

impl FileSecretAuditBackend {
	/// Create a new file secret audit backend
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// use reinhardt_conf::settings::secrets::audit::backends::FileSecretAuditBackend;
	///
	/// let backend = FileSecretAuditBackend::new("secret_audit.log").unwrap();
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
	/// use reinhardt_conf::settings::secrets::audit::backends::FileSecretAuditBackend;
	/// use std::path::Path;
	///
	/// let backend = FileSecretAuditBackend::new("audit.log").unwrap();
	/// assert_eq!(backend.path(), Path::new("audit.log"));
	/// ```
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Read all events from the file
	fn read_events(&self) -> Result<Vec<SecretAccessEvent>, String> {
		let file =
			File::open(&self.path).map_err(|e| format!("Failed to open audit file: {}", e))?;

		let reader = BufReader::new(file);
		let mut events = Vec::new();

		for line in reader.lines() {
			let line = line.map_err(|e| format!("Failed to read line: {}", e))?;

			if line.trim().is_empty() {
				continue;
			}

			let event: SecretAccessEvent =
				serde_json::from_str(&line).map_err(|e| format!("Failed to parse event: {}", e))?;

			events.push(event);
		}

		Ok(events)
	}
}

#[async_trait::async_trait]
impl SecretAuditBackend for FileSecretAuditBackend {
	async fn log_access(&self, event: SecretAccessEvent) -> Result<(), String> {
		let json = serde_json::to_string(&event)
			.map_err(|e| format!("Failed to serialize event: {}", e))?;

		let mut file = self.file.write();
		writeln!(file, "{}", json).map_err(|e| format!("Failed to write event: {}", e))?;
		file.flush()
			.map_err(|e| format!("Failed to flush file: {}", e))?;

		Ok(())
	}

	async fn get_accesses(
		&self,
		filter: Option<SecretAccessFilter>,
	) -> Result<Vec<SecretAccessEvent>, String> {
		let events = self.read_events()?;

		if let Some(filter) = filter {
			let filtered: Vec<SecretAccessEvent> = events
				.into_iter()
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
	use tempfile::NamedTempFile;

	#[tokio::test]
	async fn test_file_backend_new() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileSecretAuditBackend::new(temp_file.path()).unwrap();
		assert_eq!(backend.path(), temp_file.path());
	}

	#[tokio::test]
	async fn test_file_backend_log_access() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileSecretAuditBackend::new(temp_file.path()).unwrap();

		let event =
			SecretAccessEvent::new("secret".to_string(), "accessor".to_string(), true, None);

		backend.log_access(event).await.unwrap();

		let events = backend.get_accesses(None).await.unwrap();
		assert_eq!(events.len(), 1);
	}

	#[tokio::test]
	async fn test_file_backend_multiple_events() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileSecretAuditBackend::new(temp_file.path()).unwrap();

		for i in 0..5 {
			let event =
				SecretAccessEvent::new(format!("secret_{}", i), "accessor".to_string(), true, None);
			backend.log_access(event).await.unwrap();
		}

		let events = backend.get_accesses(None).await.unwrap();
		assert_eq!(events.len(), 5);
	}

	#[tokio::test]
	async fn test_file_backend_filter_by_secret_name() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileSecretAuditBackend::new(temp_file.path()).unwrap();

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
	async fn test_file_backend_filter_by_success() {
		let temp_file = NamedTempFile::new().unwrap();
		let backend = FileSecretAuditBackend::new(temp_file.path()).unwrap();

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
	async fn test_file_backend_persistence() {
		let temp_file = NamedTempFile::new().unwrap();

		// Create first backend and log events
		{
			let backend = FileSecretAuditBackend::new(temp_file.path()).unwrap();
			let event =
				SecretAccessEvent::new("secret".to_string(), "accessor".to_string(), true, None);
			backend.log_access(event).await.unwrap();
		}

		// Create second backend and verify events persist
		{
			let backend = FileSecretAuditBackend::new(temp_file.path()).unwrap();
			let events = backend.get_accesses(None).await.unwrap();
			assert_eq!(events.len(), 1);
		}
	}
}
