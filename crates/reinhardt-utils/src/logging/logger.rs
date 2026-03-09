use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A trait for handling log records, dispatching them to an output destination.
#[async_trait::async_trait]
pub trait LogHandler: Send + Sync {
	/// Processes a log record (e.g., writes it to a file or sends it over the network).
	async fn handle(&self, record: &LogRecord);
	/// Returns the minimum log level this handler will process.
	fn level(&self) -> LogLevel;
	/// Sets the minimum log level this handler will process.
	fn set_level(&mut self, level: LogLevel);
}

/// Severity level for log records, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
	/// Detailed diagnostic information for debugging purposes.
	Debug,
	/// General informational messages about application progress.
	Info,
	/// Indicates a potential problem that does not prevent normal operation.
	Warning,
	/// Indicates a serious problem that may prevent some functionality.
	Error,
}

/// A single log entry containing the level, source logger name, message, and optional extra data.
#[derive(Debug, Clone)]
pub struct LogRecord {
	/// The severity level of this record.
	pub level: LogLevel,
	/// The name of the logger that created this record.
	pub logger_name: String,
	/// The log message text.
	pub message: String,
	/// Additional structured data attached to this record.
	pub extra: HashMap<String, serde_json::Value>,
}

impl LogRecord {
	/// Creates a new log record with the given level, logger name, and message.
	pub fn new(
		level: LogLevel,
		logger_name: impl Into<String>,
		message: impl Into<String>,
	) -> Self {
		Self {
			level,
			logger_name: logger_name.into(),
			message: message.into(),
			extra: HashMap::new(),
		}
	}

	/// Adds an extra field to the log record (builder style).
	///
	/// This method returns `self`, allowing method chaining.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::logging::{LogRecord, LogLevel};
	/// use serde_json::json;
	///
	/// let record = LogRecord::new(LogLevel::Info, "app", "User logged in")
	///     .with_extra("user_id", json!(123))
	///     .with_extra("ip_address", json!("192.168.1.1"));
	///
	/// assert_eq!(record.extra.get("user_id").unwrap(), &json!(123));
	/// ```
	pub fn with_extra(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
		self.extra.insert(key.into(), value);
		self
	}

	/// Adds an extra field to the log record (mutable reference style).
	///
	/// This method modifies the log record in place.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::logging::{LogRecord, LogLevel};
	/// use serde_json::json;
	///
	/// let mut record = LogRecord::new(LogLevel::Info, "app", "User action");
	/// record.add_extra("action", json!("login"));
	/// record.add_extra("timestamp", json!(1234567890));
	///
	/// assert_eq!(record.extra.len(), 2);
	/// ```
	pub fn add_extra(&mut self, key: impl Into<String>, value: serde_json::Value) {
		self.extra.insert(key.into(), value);
	}
}

/// A named logger that dispatches log records to registered handlers.
pub struct Logger {
	name: String,
	handlers: Arc<Mutex<Vec<Arc<dyn LogHandler>>>>,
	level: Arc<Mutex<LogLevel>>,
}

impl Logger {
	/// Creates a new logger with the given name and a default level of `Debug`.
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			handlers: Arc::new(Mutex::new(Vec::new())),
			level: Arc::new(Mutex::new(LogLevel::Debug)),
		}
	}

	/// Registers a handler that will receive log records from this logger.
	pub async fn add_handler(&self, handler: Arc<dyn LogHandler>) {
		self.handlers
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.push(handler);
	}

	/// Sets the minimum log level; records below this level are discarded.
	pub async fn set_level(&self, level: LogLevel) {
		*self.level.lock().unwrap_or_else(|e| e.into_inner()) = level;
	}

	/// Dispatches a pre-built log record to all registered handlers.
	pub async fn log_record(&self, record: &LogRecord) {
		// Clone Arc references to handlers before releasing the lock
		let handlers: Vec<Arc<dyn LogHandler>> = {
			let handlers_guard = self.handlers.lock().unwrap_or_else(|e| e.into_inner());
			handlers_guard.clone()
		};

		// Now we can iterate and await without holding the lock
		for handler in handlers {
			handler.handle(record).await;
		}
	}

	async fn log(&self, level: LogLevel, message: impl Into<String>) {
		let current_level = *self.level.lock().unwrap_or_else(|e| e.into_inner());
		if level < current_level {
			return;
		}

		let record = LogRecord::new(level, &self.name, message);

		// Clone Arc references to handlers before releasing the lock
		let handlers: Vec<Arc<dyn LogHandler>> = {
			let handlers_guard = self.handlers.lock().unwrap_or_else(|e| e.into_inner());
			handlers_guard.clone()
		};

		// Now we can iterate and await without holding the lock
		for handler in handlers {
			handler.handle(&record).await;
		}
	}

	/// Logs a message at the `Debug` level.
	pub async fn debug(&self, message: impl Into<String>) {
		self.log(LogLevel::Debug, message).await;
	}

	/// Synchronous version of debug log
	///
	/// This method provides a synchronous interface for logging debug messages.
	/// Note that if handlers perform async operations, they will be blocked.
	/// Prefer using the async `debug` method when possible.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::logging::Logger;
	///
	/// let logger = Logger::new("my_logger");
	/// logger.debug_sync("This is a synchronous debug message");
	/// ```
	pub fn debug_sync(&self, message: impl Into<String>) {
		self.log_sync(LogLevel::Debug, message);
	}

	/// Logs a message at the `Info` level.
	pub async fn info(&self, message: impl Into<String>) {
		self.log(LogLevel::Info, message).await;
	}

	/// Synchronous version of info log
	///
	/// This method provides a synchronous interface for logging info messages.
	/// Note that if handlers perform async operations, they will be blocked.
	/// Prefer using the async `info` method when possible.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::logging::Logger;
	///
	/// let logger = Logger::new("my_logger");
	/// logger.info_sync("This is a synchronous info message");
	/// ```
	pub fn info_sync(&self, message: impl Into<String>) {
		self.log_sync(LogLevel::Info, message);
	}

	/// Logs a message at the `Warning` level.
	pub async fn warning(&self, message: impl Into<String>) {
		self.log(LogLevel::Warning, message).await;
	}

	/// Synchronous version of warning log
	///
	/// This method provides a synchronous interface for logging warnings.
	/// Note that if handlers perform async operations, they will be blocked.
	/// Prefer using the async `warning` method when possible.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::logging::Logger;
	///
	/// let logger = Logger::new("my_logger");
	/// logger.warning_sync("This is a synchronous warning");
	/// ```
	pub fn warning_sync(&self, message: impl Into<String>) {
		self.log_sync(LogLevel::Warning, message);
	}

	fn log_sync(&self, level: LogLevel, message: impl Into<String>) {
		let current_level = *self.level.lock().unwrap_or_else(|e| e.into_inner());
		if level < current_level {
			return;
		}

		let record = LogRecord::new(level, &self.name, message);

		// Clone Arc references to handlers before releasing the lock
		let handlers: Vec<Arc<dyn LogHandler>> = {
			let handlers_guard = self.handlers.lock().unwrap_or_else(|e| e.into_inner());
			handlers_guard.clone()
		};

		// Synchronously process handlers using tokio runtime
		let rt = tokio::runtime::Handle::try_current().ok().or_else(|| {
			// If no runtime exists, create a temporary one
			tokio::runtime::Runtime::new()
				.ok()
				.map(|rt| rt.handle().clone())
		});

		if let Some(handle) = rt {
			for handler in handlers.iter() {
				// Block on async handler using tokio runtime
				handle.block_on(handler.handle(&record));
			}
		} else {
			// Fallback: skip async operations if no runtime available
			// This should rarely happen in practice
			eprintln!("Warning: No tokio runtime available for synchronous logging");
		}
	}

	/// Logs a message at the `Error` level.
	pub async fn error(&self, message: impl Into<String>) {
		self.log(LogLevel::Error, message).await;
	}

	/// Synchronous version of error log
	///
	/// This method provides a synchronous interface for logging error messages.
	/// Note that if handlers perform async operations, they will be blocked.
	/// Prefer using the async `error` method when possible.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::logging::Logger;
	///
	/// let logger = Logger::new("my_logger");
	/// logger.error_sync("This is a synchronous error message");
	/// ```
	pub fn error_sync(&self, message: impl Into<String>) {
		self.log_sync(LogLevel::Error, message);
	}
}
