//! Settings module for example-rest-api
//!
//! This module provides environment-specific settings configuration using TOML files.

use reinhardt::core::serde::json;
use reinhardt::{
	DefaultSource, LowPriorityEnvSource, Profile, Settings, SettingsBuilder, TomlFileSource,
};
use std::env;

/// Get settings based on environment variable
///
/// Reads the REINHARDT_ENV environment variable to determine which settings to load.
/// Defaults to "local" if not set.
///
/// Priority order (highest to lowest):
/// 1. Environment-specific TOML file (e.g., `production.toml`)
/// 2. Base TOML file (`base.toml`)
/// 3. Environment variables with `REINHARDT_` prefix
/// 4. Default values
pub fn get_settings() -> Settings {
	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| {
		if env::var("CI").is_ok() {
			"ci".to_string()
		} else {
			"local".to_string()
		}
	});
	let profile = Profile::parse(&profile_str);

	// Get the project root directory
	let base_dir = env::current_dir().expect("Failed to get current directory");
	let settings_dir = base_dir.join("settings");

	let merged = SettingsBuilder::new()
		.profile(profile)
		.add_source(
			DefaultSource::new()
				.with_value(
					"base_dir",
					json::Value::String(base_dir.to_string_lossy().to_string()),
				)
				.with_value(
					"secret_key",
					json::Value::String("test-secret-key-for-development-only".to_string()),
				)
				.with_value("debug", json::Value::Bool(true))
				.with_value("allowed_hosts", json::Value::Array(vec![]))
				.with_value("installed_apps", json::Value::Array(vec![]))
				.with_value("databases", json::Value::Object(json::Map::new()))
				.with_value("templates", json::Value::Array(vec![]))
				.with_value("static_url", json::Value::String("/static/".to_string()))
				.with_value("staticfiles_dirs", json::Value::Array(vec![]))
				.with_value("media_url", json::Value::String("/media/".to_string()))
				.with_value("language_code", json::Value::String("en-us".to_string()))
				.with_value("time_zone", json::Value::String("UTC".to_string()))
				.with_value("use_i18n", json::Value::Bool(true))
				.with_value("use_tz", json::Value::Bool(true))
				.with_value(
					"default_auto_field",
					json::Value::String("BigAutoField".to_string()),
				)
				.with_value("secure_ssl_redirect", json::Value::Bool(false))
				.with_value("secure_hsts_include_subdomains", json::Value::Bool(false))
				.with_value("secure_hsts_preload", json::Value::Bool(false))
				.with_value("session_cookie_secure", json::Value::Bool(false))
				.with_value("csrf_cookie_secure", json::Value::Bool(false))
				.with_value("append_slash", json::Value::Bool(true))
				.with_value("admins", json::Value::Array(vec![]))
				.with_value("managers", json::Value::Array(vec![]))
				// Fields for crates.io compatibility (removed in local version)
				.with_value("middleware", json::Value::Array(vec![]))
				.with_value("media_root", json::Value::Null)
				.with_value("root_urlconf", json::Value::String("".to_string())),
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
		.expect("Failed to convert settings to Settings struct")
}
