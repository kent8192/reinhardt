use reinhardt::core::serde::json;
use reinhardt::{
	DefaultSource, LowPriorityEnvSource, Profile, Settings, SettingsBuilder, TomlFileSource,
};
use std::env;

pub fn get_settings() -> Settings {
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
					"base_dir",
					json::Value::String(base_dir.to_string_lossy().to_string()),
				)
				.with_value(
					"secret_key",
					json::Value::String(
						"test-secret-key-for-development-only".to_string(),
					),
				)
				.with_value("debug", json::Value::Bool(true))
				.with_value(
					"language_code",
					json::Value::String("en-us".to_string()),
				)
				.with_value(
					"time_zone",
					json::Value::String("UTC".to_string()),
				)
				.with_value("middleware", json::Value::Array(vec![]))
				.with_value("media_root", json::Value::Null)
				.with_value(
					"root_urlconf",
					json::Value::String(String::new()),
				),
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
