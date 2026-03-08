/// Configuration for a single log handler.
pub struct HandlerConfig;
/// Configuration for a single logger instance.
pub struct LoggerConfig;

/// Global logging configuration that holds settings for the entire logging system.
#[derive(Clone)]
pub struct LoggingConfig;

impl Default for LoggingConfig {
	fn default() -> Self {
		Self
	}
}

/// Manages the logging system and provides access to named loggers.
pub struct LoggingManager;

impl LoggingManager {
	/// Creates a new `LoggingManager` from the given configuration.
	pub fn new(_config: LoggingConfig) -> Self {
		Self
	}

	/// Returns a shared reference to the logger with the given name.
	pub fn get_logger(&self, name: &str) -> std::sync::Arc<crate::logging::logger::Logger> {
		std::sync::Arc::new(crate::logging::logger::Logger::new(name.to_string()))
	}
}
