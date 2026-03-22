//! Logging settings fragment
//!
//! Provides composable logging configuration as a [`SettingsFragment`].

use super::fragment::SettingsFragment;
use serde::{Deserialize, Serialize};

/// Logging configuration fragment.
///
/// Controls the log level and output format.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoggingSettings {
	/// Log level (e.g., `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`).
	pub level: String,
	/// Log output format (e.g., `"text"`, `"json"`).
	pub format: String,
}

impl Default for LoggingSettings {
	fn default() -> Self {
		Self {
			level: "info".to_string(),
			format: "text".to_string(),
		}
	}
}

impl SettingsFragment for LoggingSettings {
	fn section() -> &'static str {
		"logging"
	}
}

/// Trait for settings containers that include logging configuration.
pub trait HasLoggingSettings {
	/// Returns a reference to the logging settings.
	fn logging(&self) -> &LoggingSettings;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_logging_section_name() {
		// Arrange / Act
		let section = LoggingSettings::section();

		// Assert
		assert_eq!(section, "logging");
	}

	#[rstest]
	fn test_logging_default_values() {
		// Arrange / Act
		let settings = LoggingSettings::default();

		// Assert
		assert_eq!(settings.level, "info");
		assert_eq!(settings.format, "text");
	}
}
