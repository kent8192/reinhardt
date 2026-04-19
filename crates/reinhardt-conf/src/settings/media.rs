//! Media settings fragment
//!
//! Provides composable media file serving configuration as a [`SettingsFragment`](crate::settings::fragment::SettingsFragment).

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Media files configuration fragment.
///
/// Controls the URL prefix and root directory for user-uploaded media files.
#[settings(fragment = true, section = "media")]
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
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
