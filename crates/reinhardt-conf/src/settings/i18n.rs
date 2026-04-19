//! Internationalization settings fragment
//!
//! Django compatibility fields for i18n/l10n configuration.

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};

/// Internationalization and localization settings.
///
/// Django compatibility fields. Currently reserved for future i18n support.
#[settings(fragment = true, section = "i18n")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct I18nSettings {
	/// Language code (e.g., `"en-us"`).
	#[serde(default = "default_language_code")]
	pub language_code: String,
	/// Timezone (e.g., `"UTC"`).
	#[serde(default = "default_time_zone")]
	pub time_zone: String,
	/// Enable internationalization.
	#[serde(default = "default_true")]
	pub use_i18n: bool,
	/// Use timezone-aware datetimes.
	#[serde(default = "default_true")]
	pub use_tz: bool,
}

fn default_language_code() -> String {
	"en-us".to_string()
}

fn default_time_zone() -> String {
	"UTC".to_string()
}

fn default_true() -> bool {
	true
}

impl Default for I18nSettings {
	fn default() -> Self {
		Self {
			language_code: default_language_code(),
			time_zone: default_time_zone(),
			use_i18n: true,
			use_tz: true,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
	use rstest::rstest;

	#[rstest]
	fn test_i18n_settings_section() {
		// Arrange / Act / Assert
		assert_eq!(I18nSettings::section(), "i18n");
	}

	#[rstest]
	fn test_i18n_settings_default() {
		// Arrange / Act
		let settings = I18nSettings::default();

		// Assert
		assert_eq!(settings.language_code, "en-us");
		assert_eq!(settings.time_zone, "UTC");
		assert!(settings.use_i18n);
		assert!(settings.use_tz);
	}
}
