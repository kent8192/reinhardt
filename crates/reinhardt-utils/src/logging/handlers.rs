use super::logger::{LogHandler, LogLevel, LogRecord};
use std::sync::{Arc, Mutex};

/// A handler that writes log records to the console (stdout/stderr).
pub struct ConsoleHandler;
/// A handler that writes log records to a file.
pub struct FileHandler;
/// A handler that writes log records in JSON format.
pub struct JsonHandler;

/// A handler that stores log records in memory for later retrieval.
///
/// This is primarily useful for testing, allowing assertions on logged messages.
#[derive(Clone)]
pub struct MemoryHandler {
	level: LogLevel,
	records: Arc<Mutex<Vec<LogRecord>>>,
}

impl MemoryHandler {
	/// Creates a new `MemoryHandler` that captures records at or above the given level.
	pub fn new(level: LogLevel) -> Self {
		Self {
			level,
			records: Arc::new(Mutex::new(Vec::new())),
		}
	}

	/// Returns a clone of all captured log records.
	pub fn get_records(&self) -> Vec<LogRecord> {
		self.records
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.clone()
	}

	/// Removes all captured log records.
	pub fn clear(&self) {
		self.records
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.clear();
	}
}

#[async_trait::async_trait]
impl LogHandler for MemoryHandler {
	async fn handle(&self, record: &LogRecord) {
		if record.level >= self.level {
			self.records
				.lock()
				.unwrap_or_else(|e| e.into_inner())
				.push(record.clone());
		}
	}

	fn level(&self) -> LogLevel {
		self.level
	}

	fn set_level(&mut self, level: LogLevel) {
		self.level = level;
	}
}
