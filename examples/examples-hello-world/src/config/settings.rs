//! Settings module for examples-hello-world
//!
//! This module provides environment-specific settings configuration using TOML files.
//!
//! ## Configuration Structure
//!
//! Settings are loaded from TOML files in the `settings/` directory:
//! - `base.toml` - Common settings across all environments
//! - `local.toml` - Local development settings
//!
//! ## Priority Order
//!
//! Settings are merged with the following priority (highest to lowest):
//! 1. Environment-specific TOML file (e.g., `local.toml`)
//! 2. Base TOML file (`base.toml`)
//! 3. Environment variables with `REINHARDT_` prefix
//! 4. Default values
//!
//! ## Environment Selection
//!
//! The environment is determined by the `REINHARDT_ENV` environment variable.
//! If `REINHARDT_ENV` is not set, it defaults to `local`.

use reinhardt::core::serde::json;
use reinhardt::settings;
use reinhardt::conf::settings::builder::SettingsBuilder;
use reinhardt::conf::settings::profile::Profile;
use reinhardt::conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
use std::env;

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
/// use examples_hello_world::config::settings::get_settings;
///
/// let settings = get_settings();
/// println!("Debug mode: {}", settings.core.debug);
/// ```
///
/// # Panics
///
/// Panics if:
/// - Settings files cannot be read
/// - Settings cannot be deserialized
/// - Required settings are missing
pub fn get_settings() -> ProjectSettings {
	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| {
		if env::var("CI").is_ok() {
			"ci".to_string()
		} else {
			"local".to_string()
		}
	});
	let profile = Profile::parse(&profile_str);

	let base_dir = env::current_dir().expect("Failed to get current directory");
	let settings_dir = base_dir.join("settings");

	let merged = SettingsBuilder::new()
		.profile(profile)
		.add_source(
			DefaultSource::new()
				.with_value(
					"core.base_dir",
					json::Value::String(base_dir.to_string_lossy().to_string()),
				)
				.with_value(
					"core.secret_key",
					json::Value::String("test-secret-key-for-development-only".to_string()),
				)
				.with_value("core.debug", json::Value::Bool(true))
				.with_value("core.allowed_hosts", json::Value::Array(vec![]))
				.with_value("core.installed_apps", json::Value::Array(vec![]))
				.with_value("core.databases", json::Value::Object(json::Map::new()))
				.with_value("core.templates", json::Value::Array(vec![]))
				.with_value("core.static_url", json::Value::String("/static/".to_string()))
				.with_value("core.staticfiles_dirs", json::Value::Array(vec![]))
				.with_value("core.media_url", json::Value::String("/media/".to_string()))
				.with_value("core.language_code", json::Value::String("en-us".to_string()))
				.with_value("core.time_zone", json::Value::String("UTC".to_string()))
				.with_value("core.use_i18n", json::Value::Bool(true))
				.with_value("core.use_tz", json::Value::Bool(true))
				.with_value(
					"core.default_auto_field",
					json::Value::String("BigAutoField".to_string()),
				)
				.with_value("core.secure_ssl_redirect", json::Value::Bool(false))
				.with_value("core.secure_hsts_include_subdomains", json::Value::Bool(false))
				.with_value("core.secure_hsts_preload", json::Value::Bool(false))
				.with_value("core.session_cookie_secure", json::Value::Bool(false))
				.with_value("core.csrf_cookie_secure", json::Value::Bool(false))
				.with_value("core.append_slash", json::Value::Bool(true))
				.with_value("core.admins", json::Value::Array(vec![]))
				.with_value("core.managers", json::Value::Array(vec![]))
				// Fields for crates.io compatibility (removed in local version)
				.with_value("core.middleware", json::Value::Array(vec![]))
				.with_value("core.media_root", json::Value::Null)
				.with_value("core.root_urlconf", json::Value::String("".to_string())),
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(
			settings_dir.join(format!("{}.toml", profile_str)),
		))
		.build()
		.expect("Failed to build settings");

	merged
		.into_typed()
		.expect("Failed to convert settings to ProjectSettings")
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_get_settings() {
		let settings = get_settings();
		assert!(
			!settings.core.secret_key.is_empty(),
			"secret_key should be populated"
		);
	}
}
