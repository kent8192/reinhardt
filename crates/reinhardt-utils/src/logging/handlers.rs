use super::logger::{LogHandler, LogLevel, LogRecord};
use std::sync::{Arc, Mutex};

pub struct ConsoleHandler;
pub struct FileHandler;
pub struct JsonHandler;

#[derive(Clone)]
pub struct MemoryHandler {
	level: LogLevel,
	records: Arc<Mutex<Vec<LogRecord>>>,
}

impl MemoryHandler {
	pub fn new(level: LogLevel) -> Self {
		Self {
			level,
			records: Arc::new(Mutex::new(Vec::new())),
		}
	}

	pub fn get_records(&self) -> Vec<LogRecord> {
		self.records
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.clone()
	}

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
