//! Settings module for {{ project_name }}
//!
//! This module provides environment-specific settings configuration using TOML files.
//!
//! ## Configuration Structure
//!
//! Settings are loaded from TOML files in the `settings/` directory:
//! - `base.toml` - Common settings across all environments
//! - `local.toml` - Local development settings
//! - `staging.toml` - Staging environment settings
//! - `production.toml` - Production environment settings
//!
//! ## Priority Order
//!
//! Settings are merged with the following priority (highest to lowest):
//! 1. Environment-specific TOML file (e.g., `production.toml`)
//! 2. Base TOML file (`base.toml`)
//! 3. Environment variables with `REINHARDT_` prefix
//! 4. Default values
//!
//! ## Environment Selection
//!
//! The environment is determined by the `REINHARDT_ENV` environment variable:
//! - `local` or `development` → loads `local.toml`
//! - `staging` → loads `staging.toml`
//! - `production` → loads `production.toml`
//!
//! If `REINHARDT_ENV` is not set, it defaults to `local`.

use reinhardt::conf::settings::builder::SettingsBuilder;
use reinhardt::conf::settings::profile::Profile;
use reinhardt::conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
use reinhardt::settings;
use std::env;

// Add fragments to extend settings: e.g. `#[settings(core: CoreSettings | cache: CacheSettings)]`
#[settings(core: CoreSettings)]
pub struct ProjectSettings;

/// Get settings based on environment variable
///
/// Reads the REINHARDT_ENV environment variable to determine which settings to load.
/// Defaults to "local" if not set.
///
/// # Examples
///
/// ```no_run
/// use {{ crate_name }}::config::settings::get_settings;
///
/// let settings = get_settings();
/// ```
///
/// # Panics
///
/// Panics if:
/// - Settings files cannot be read
/// - Settings cannot be deserialized
/// - Required settings are missing
pub fn get_settings() -> ProjectSettings {
	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = Profile::parse(&profile_str);

	// Get the project root directory (parent of src/)
	let base_dir = env::current_dir().expect("Failed to get current directory");
	let settings_dir = base_dir.join("settings");

	// Build settings by merging sources in priority order
	let merged = SettingsBuilder::new()
		.profile(profile)
		// Lowest priority: Default values
		.add_source(DefaultSource::new())
		// Low priority: Environment variables (for container overrides)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		// Medium priority: Base TOML file
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		// Highest priority: Environment-specific TOML file
		.add_source(TomlFileSource::new(
			settings_dir.join(format!("{}.toml", profile_str)),
		))
		.build()
		.expect("Failed to build settings");

	// Convert MergedSettings to ProjectSettings
	merged
		.into_typed()
		.expect("Failed to convert settings to ProjectSettings struct")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_get_settings() {
		// Smoke test: ensures settings load without panic and required fields are present
		let settings = get_settings();
		assert!(
			!settings.core.secret_key.is_empty(),
			"secret_key should be populated from settings sources"
		);
	}
}
