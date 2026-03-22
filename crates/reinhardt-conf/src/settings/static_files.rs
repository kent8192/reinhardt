//! Static files settings fragment
//!
//! Provides composable static file serving configuration as a [`SettingsFragment`].

use super::fragment::SettingsFragment;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Static files configuration fragment.
///
/// Controls the URL prefix and root directory for serving static files.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticSettings {
	/// URL prefix for serving static files (e.g., `"/static/"`).
	pub url: String,
	/// Root directory for collected static files.
	pub root: PathBuf,
}

impl Default for StaticSettings {
	fn default() -> Self {
		Self {
			url: "/static/".to_string(),
			root: PathBuf::from("static"),
		}
	}
}

impl SettingsFragment for StaticSettings {
	fn section() -> &'static str {
		"static_files"
	}
}

/// Trait for settings containers that include static files configuration.
pub trait HasStaticSettings {
	/// Returns a reference to the static files settings.
	fn static_files(&self) -> &StaticSettings;
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_static_files_section_name() {
		// Arrange / Act
		let section = StaticSettings::section();

		// Assert
		assert_eq!(section, "static_files");
	}

	#[rstest]
	fn test_static_files_default_values() {
		// Arrange / Act
		let settings = StaticSettings::default();

		// Assert
		assert_eq!(settings.url, "/static/");
		assert_eq!(settings.root, PathBuf::from("static"));
	}
}
