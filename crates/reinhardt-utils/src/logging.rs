//! Logging framework for Reinhardt

/// Logging configuration and manager.
pub mod config;
/// Log record filters (debug mode, callback-based).
pub mod filters;
/// Log record formatters (standard, server, JSON).
pub mod formatters;
/// Log output handlers (console, file, JSON, in-memory).
pub mod handlers;
/// Core logger, log levels, and log records.
pub mod logger;
/// Logging parameters and configuration types.
pub mod params;
/// Security-sensitive log filtering.
pub mod security;

pub use config::{HandlerConfig, LoggerConfig, LoggingConfig, LoggingManager};
pub use filters::{CallbackFilter, Filter, RequireDebugFalse, RequireDebugTrue};
pub use formatters::{Formatter, ServerFormatter, StandardFormatter, escape_control_chars};
pub use handlers::{ConsoleHandler, FileHandler, JsonHandler, MemoryHandler};
pub use logger::{LogHandler, LogLevel, LogRecord, Logger};
pub use params::{ReprParamsConfig, repr_params, truncate_param};
pub use security::{SecurityError, SecurityLogger};

// ----------------------------------------------------------------------------
// Global logging manager and convenience APIs
// ----------------------------------------------------------------------------

use once_cell::sync::OnceCell;
use std::sync::Arc;

static GLOBAL_MANAGER: OnceCell<LoggingManager> = OnceCell::new();

/// A handle to a logger that hides the internal Arc.
///
/// This provides a user-friendly interface to the logger without exposing the Arc wrapper.
#[derive(Clone)]
pub struct LoggerHandle {
	inner: Arc<Logger>,
}

impl LoggerHandle {
	/// Create a new logger handle from a logger.
	fn new(logger: Arc<Logger>) -> Self {
		Self { inner: logger }
	}

	/// Add a handler to this logger.
	pub async fn add_handler<H: LogHandler + 'static>(&self, handler: H) {
		self.inner.add_handler(Arc::new(handler)).await;
	}

	/// Set the log level for this logger.
	pub async fn set_level(&self, level: LogLevel) {
		self.inner.set_level(level).await;
	}

	/// Log a record.
	pub async fn log_record(&self, record: &LogRecord) {
		self.inner.log_record(record).await;
	}

	/// Log a debug message.
	pub async fn debug(&self, message: impl Into<String>) {
		self.inner.debug(message.into()).await;
	}

	/// Log an info message.
	pub async fn info(&self, message: impl Into<String>) {
		self.inner.info(message.into()).await;
	}

	/// Log a warning message.
	pub async fn warning(&self, message: impl Into<String>) {
		self.inner.warning(message.into()).await;
	}

	/// Log a debug message synchronously.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::logging::get_logger;
	///
	/// let logger = get_logger("myapp");
	/// logger.debug_sync("This is a debug message");
	/// ```
	pub fn debug_sync(&self, message: impl Into<String>) {
		self.inner.debug_sync(message);
	}

	/// Log an info message synchronously.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::logging::get_logger;
	///
	/// let logger = get_logger("myapp");
	/// logger.info_sync("This is an info message");
	/// ```
	pub fn info_sync(&self, message: impl Into<String>) {
		self.inner.info_sync(message);
	}

	/// Log a warning message synchronously.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::logging::get_logger;
	///
	/// let logger = get_logger("myapp");
	/// logger.warning_sync("This is a warning message");
	/// ```
	pub fn warning_sync(&self, message: impl Into<String>) {
		self.inner.warning_sync(message);
	}

	/// Log an error message synchronously.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::logging::get_logger;
	///
	/// let logger = get_logger("myapp");
	/// logger.error_sync("This is an error message");
	/// ```
	pub fn error_sync(&self, message: impl Into<String>) {
		self.inner.error_sync(message);
	}
}

/// Initialize the global logging manager. Subsequent calls are no-ops.
pub fn init_global_logging(config: LoggingConfig) {
	let _ = GLOBAL_MANAGER.set(LoggingManager::new(config));
}

fn global_manager() -> &'static LoggingManager {
	GLOBAL_MANAGER.get_or_init(|| LoggingManager::new(LoggingConfig))
}

/// Get a logger by name from the global manager.
///
/// Returns a `LoggerHandle` that provides a user-friendly interface without exposing the internal Arc.
///
/// # Examples
///
/// ```
/// use reinhardt_utils::logging::get_logger;
///
/// let logger = get_logger("myapp");
/// logger.warning_sync("This is a warning message");
/// ```
pub fn get_logger(name: &str) -> LoggerHandle {
	LoggerHandle::new(global_manager().get_logger(name))
}

/// Emit a warning message via the named logger (global).
pub fn emit_warning(logger_name: &str, message: impl Into<String>) {
	let logger = get_logger(logger_name);
	logger.warning_sync(message.into());
}

/// Attach a `MemoryHandler` to the named logger (useful for tests).
pub async fn attach_memory_handler(logger_name: &str, level: LogLevel) -> handlers::MemoryHandler {
	let handler = handlers::MemoryHandler::new(level);
	let logger = get_logger(logger_name);
	logger.add_handler(handler.clone()).await;
	handler
}
