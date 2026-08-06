//! Migration settings fragment.
//!
//! Provides migration dependency resolution configuration independently of
//! [`CoreSettings`](super::core_settings::CoreSettings).

use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration used to resolve conditional migration dependencies.
///
/// These values live in the `[migrations]` section of composed project
/// settings. Keeping them in a dedicated fragment lets migration-aware command
/// entry points opt in without expanding the public [`CoreSettings`](super::core_settings::CoreSettings)
/// struct.
#[settings(fragment = true, section = "migrations")]
#[non_exhaustive]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MigrationSettings {
	/// Values used to resolve swappable migration dependencies.
	#[serde(default)]
	pub migration_swappable_settings: HashMap<String, String>,
	/// Values used to resolve setting-gated optional migration dependencies.
	#[serde(default)]
	pub migration_settings: HashMap<String, String>,
	/// Feature flags used to resolve optional migration dependencies.
	#[serde(default)]
	pub migration_features: Vec<String>,
}

#[cfg(test)]
mod tests {
	use super::MigrationSettings;
	use crate::settings::fragment::SettingsFragment;
	use crate::settings::schema::SettingsNode;
	use rstest::rstest;

	#[rstest]
	fn migration_settings_defaults_are_empty() {
		// Arrange / Act
		let settings = MigrationSettings::default();

		// Assert
		assert!(settings.migration_swappable_settings.is_empty());
		assert!(settings.migration_settings.is_empty());
		assert!(settings.migration_features.is_empty());
		assert_eq!(MigrationSettings::section(), "migrations");
	}

	#[rstest]
	fn migration_settings_serde_roundtrip() {
		// Arrange
		let toml = r#"
migration_features = ["gis"]

[migration_swappable_settings]
AUTH_USER_MODEL = "accounts.User"

[migration_settings]
ENABLE_AUDIT = "true"
"#;

		// Act
		let settings: MigrationSettings =
			toml::from_str(toml).expect("deserialize migration settings");
		let serialized = toml::to_string(&settings).expect("serialize migration settings");
		let restored: MigrationSettings =
			toml::from_str(&serialized).expect("restore migration settings");

		// Assert
		assert_eq!(restored.migration_features, ["gis"]);
		assert_eq!(
			restored
				.migration_swappable_settings
				.get("AUTH_USER_MODEL")
				.map(String::as_str),
			Some("accounts.User")
		);
		assert_eq!(
			restored
				.migration_settings
				.get("ENABLE_AUDIT")
				.map(String::as_str),
			Some("true")
		);
	}

	#[rstest]
	fn migration_settings_schema_exposes_all_fields() {
		// Arrange / Act
		let schema = MigrationSettings::node_schema();
		let field_keys: Vec<_> = schema.fields.iter().map(|field| field.key).collect();

		// Assert
		assert_eq!(
			field_keys,
			[
				"migration_swappable_settings",
				"migration_settings",
				"migration_features",
			]
		);
	}
}
