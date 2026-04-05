//! Template settings fragment
//!
//! Composable fragment for template engine configuration.

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Template engine configuration for a single backend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FragmentTemplateConfig {
	/// Template backend identifier.
	pub backend: String,
	/// Template search directories.
	#[serde(default)]
	pub dirs: Vec<PathBuf>,
	/// Whether to search app directories for templates.
	#[serde(default)]
	pub app_dirs: bool,
	/// Backend-specific options.
	#[serde(default)]
	pub options: HashMap<String, serde_json::Value>,
}

/// Template engine settings fragment.
///
/// Django compatibility: wraps template engine configurations.
#[settings(fragment = true, section = "templates")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TemplateSettings {
	/// Template configurations list.
	#[serde(default)]
	pub configs: Vec<FragmentTemplateConfig>,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::fragment::SettingsFragment;
	use rstest::rstest;

	#[rstest]
	fn test_template_settings_section() {
		// Arrange / Act / Assert
		assert_eq!(TemplateSettings::section(), "templates");
	}

	#[rstest]
	fn test_template_settings_default() {
		// Arrange / Act
		let settings = TemplateSettings::default();

		// Assert
		assert!(settings.configs.is_empty());
	}
}
