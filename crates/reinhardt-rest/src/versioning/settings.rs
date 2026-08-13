//! Versioning settings fragment
//!
//! Provides composable API versioning configuration as a
//! [`SettingsFragment`](reinhardt_conf::settings::fragment::SettingsFragment).
//!
//! This fragment is read from the `[rest_versioning]` section of the project's
//! TOML settings. Convert it into a [`VersioningConfig`](super::VersioningConfig)
//! via [`VersioningConfig::from`](super::VersioningConfig::from) to obtain the
//! runtime configuration consumed by
//! [`VersioningManager`](super::VersioningManager).
//!
//! # Section naming
//!
//! The section name uses an underscore (`rest_versioning`) rather than a dotted
//! path (`rest.versioning`) because the `#[settings]` macro generates a method
//! identifier from the section string, and Rust identifiers cannot contain dots.
//! A future refactor may introduce a parent `RestSettings` fragment that hosts
//! versioning as a nested `[rest.versioning]` sub-section; until then the
//! flat `[rest_versioning]` form is the canonical location.

use super::config::VersioningStrategy;
use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_default_version() -> String {
	"1.0".to_string()
}

fn default_strict_mode() -> bool {
	true
}

fn default_strategy() -> VersioningStrategy {
	VersioningStrategy::AcceptHeader
}

/// Versioning configuration fragment.
///
/// Maps to the `[rest_versioning]` TOML section. Controls the default
/// API version, the set of allowed versions, the versioning strategy,
/// strict-mode enforcement, and strategy-specific overrides (query
/// parameter name, hostname patterns).
///
/// # Example
///
/// ```toml
/// [rest_versioning]
/// default_version = "1.0"
/// allowed_versions = ["1.0", "2.0"]
/// strict_mode = true
///
/// [rest_versioning.strategy]
/// type = "AcceptHeader"
/// ```
#[settings(fragment = true, section = "rest_versioning")]
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersioningSettings {
	/// Default version to use when no version is specified.
	#[serde(default = "default_default_version")]
	pub default_version: String,

	/// Allowed versions (empty means any version is allowed).
	#[serde(default)]
	pub allowed_versions: Vec<String>,

	/// Versioning strategy configuration.
	#[setting(leaf)]
	#[serde(default = "default_strategy")]
	pub strategy: VersioningStrategy,

	/// Whether to raise errors for invalid versions.
	#[serde(default = "default_strict_mode")]
	pub strict_mode: bool,

	/// Custom version parameter name for query parameter versioning.
	#[serde(default)]
	pub version_param: Option<String>,

	/// Custom hostname patterns for hostname versioning.
	#[serde(default)]
	pub hostname_patterns: Option<HashMap<String, String>>,
}

impl Default for VersioningSettings {
	fn default() -> Self {
		Self {
			default_version: default_default_version(),
			allowed_versions: vec![],
			strategy: default_strategy(),
			strict_mode: default_strict_mode(),
			version_param: None,
			hostname_patterns: None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_conf::settings::fragment::SettingsFragment;
	use rstest::rstest;

	#[rstest]
	fn test_versioning_section_name() {
		// Arrange / Act
		let section = VersioningSettings::section();

		// Assert
		assert_eq!(section, "rest_versioning");
	}

	#[rstest]
	fn test_versioning_default_values() {
		// Arrange / Act
		let settings = VersioningSettings::default();

		// Assert
		assert_eq!(settings.default_version, "1.0");
		assert!(settings.allowed_versions.is_empty());
		assert!(matches!(
			settings.strategy,
			VersioningStrategy::AcceptHeader
		));
		assert!(settings.strict_mode);
		assert!(settings.version_param.is_none());
		assert!(settings.hostname_patterns.is_none());
	}

	#[rstest]
	fn test_versioning_deserialize_from_json() {
		// Arrange — emulate a `[rest_versioning]` TOML section after table
		// flattening (matches what reinhardt-conf produces from TOML).
		let json = r#"{
			"default_version": "2.0",
			"allowed_versions": ["1.0", "2.0", "3.0"],
			"strict_mode": false,
			"strategy": { "type": "URLPath", "config": { "pattern": "/v{version}/" } }
		}"#;

		// Act
		let settings: VersioningSettings = serde_json::from_str(json).unwrap();

		// Assert
		assert_eq!(settings.default_version, "2.0");
		assert_eq!(settings.allowed_versions, vec!["1.0", "2.0", "3.0"]);
		assert!(!settings.strict_mode);
		assert!(matches!(
			settings.strategy,
			VersioningStrategy::URLPath { .. }
		));
	}
}
