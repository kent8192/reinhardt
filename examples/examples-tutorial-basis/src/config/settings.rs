use reinhardt::conf::settings::builder::SettingsBuilder;
use reinhardt::conf::settings::profile::Profile;
use reinhardt::conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
use reinhardt::core::serde::json;
use reinhardt::settings;
use std::env;
use std::path::PathBuf;

#[settings(core: CoreSettings)]
pub struct ProjectSettings;

fn profile_name() -> String {
	env::var("REINHARDT_ENV").unwrap_or_else(|_| {
		if env::var("CI").is_ok() {
			"ci".to_string()
		} else {
			"local".to_string()
		}
	})
}

fn resolve_settings_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("settings")
}

pub fn get_settings() -> ProjectSettings {
	let profile_str = profile_name();
	let settings_dir = resolve_settings_dir();
	let base_dir = env::current_dir().expect("Failed to get current directory");

	SettingsBuilder::new()
		.profile(Profile::parse(&profile_str))
		.add_source(
			DefaultSource::new().with_value(
				"core.base_dir",
				json::Value::String(base_dir.to_string_lossy().to_string()),
			),
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(
			settings_dir.join(format!("{}.toml", profile_str)),
		))
		.build_composed()
		.expect("Failed to build settings")
}
