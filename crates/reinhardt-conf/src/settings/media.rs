//! Media settings fragment
//!
//! Provides composable media file serving configuration as a [`SettingsFragment`].

use super::fragment::{HasSettings, SettingsFragment};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Media files configuration fragment.
///
/// Controls the URL prefix and root directory for user-uploaded media files.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSettings {
	/// URL prefix for serving user-uploaded media files (e.g., `"/media/"`).
	#[serde(default = "default_media_url")]
	pub url: String,
	/// Root directory for user-uploaded media files.
	#[serde(default = "default_media_root")]
	pub root: PathBuf,
}

fn default_media_url() -> String {
	"/media/".to_string()
}

fn default_media_root() -> PathBuf {
	PathBuf::from("media")
}

impl Default for MediaSettings {
	fn default() -> Self {
		Self {
			url: "/media/".to_string(),
			root: PathBuf::from("media"),
		}
	}
}

impl SettingsFragment for MediaSettings {
	type Accessor = dyn HasMediaSettings;

	fn section() -> &'static str {
		"media"
	}
}

/// Trait for settings containers that include media configuration.
pub trait HasMediaSettings {
	/// Returns a reference to the media settings.
	fn media(&self) -> &MediaSettings;
}

impl<T: HasSettings<MediaSettings>> HasMediaSettings for T {
	fn media(&self) -> &MediaSettings {
		self.get_settings()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_media_section_name() {
		// Arrange / Act
		let section = MediaSettings::section();

		// Assert
		assert_eq!(section, "media");
	}

	#[rstest]
	fn test_media_default_values() {
		// Arrange / Act
		let settings = MediaSettings::default();

		// Assert
		assert_eq!(settings.url, "/media/");
		assert_eq!(settings.root, PathBuf::from("media"));
	}
}
