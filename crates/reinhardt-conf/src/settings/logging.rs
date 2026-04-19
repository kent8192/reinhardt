//! Logging settings fragment
//!
//! Provides composable logging configuration as a [`SettingsFragment`](crate::settings::fragment::SettingsFragment).

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};

fn default_level() -> String {
	"info".to_string()
}

fn default_format() -> String {
	"text".to_string()
}

/// Logging configuration fragment.
///
/// Controls the log level and output format.
#[settings(fragment = true, section = "logging")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoggingSettings {
	/// Log level (e.g., `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`).
	#[serde(default = "default_level")]
	pub level: String,
	/// Log output format (e.g., `"text"`, `"json"`).
	#[serde(default = "default_format")]
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
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
