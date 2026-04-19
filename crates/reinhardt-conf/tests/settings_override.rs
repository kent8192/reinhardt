// Integration tests for per-test settings override mechanisms (Option A + Option B).
// Covers: HighPriorityEnvSource, SettingsOverride guard, combined priority interactions.

use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::sources::{
	DefaultSource, HighPriorityEnvSource, LowPriorityEnvSource, TomlFileSource,
};
use reinhardt_conf::settings::testing::SettingsOverride;
use rstest::rstest;
use serde_json::Value;
use serial_test::serial;
use std::io::Write;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_toml(content: &str) -> (TempDir, std::path::PathBuf) {
	let dir = TempDir::new().unwrap();
	let path = dir.path().join("config.toml");
	let mut file = std::fs::File::create(&path).unwrap();
	file.write_all(content.as_bytes()).unwrap();
	(dir, path)
}

// ---------------------------------------------------------------------------
// Option A: HighPriorityEnvSource integration
// ---------------------------------------------------------------------------

#[rstest]
#[serial(settings_override)]
fn high_priority_env_overrides_toml_and_low_priority_env() {
	// Arrange
	let (_dir, toml_path) = write_toml("port = 1025\nhost = \"filehost\"");

	let prefix_low = "SOTEST_LOW_";
	let prefix_high = "SOTEST_HIGH_";

	// SAFETY: Serial test ensures exclusive env access.
	unsafe {
		std::env::set_var(format!("{prefix_low}PORT"), "4000");
		std::env::set_var(format!("{prefix_high}PORT"), "9999");
	}

	// Act
	let settings = SettingsBuilder::new()
		.add_source(LowPriorityEnvSource::new().with_prefix(prefix_low))
		.add_source(TomlFileSource::new(&toml_path))
		.add_source(HighPriorityEnvSource::new().with_prefix(prefix_high))
		.build()
		.unwrap();

	// Assert — HighPriorityEnvSource (60) wins over both TOML (50) and LowPriority (40)
	let port: i64 = settings.get("port").unwrap();
	assert_eq!(port, 9999);

	// TOML value for "host" still present (no high-priority override)
	let host: String = settings.get("host").unwrap();
	assert_eq!(host, "filehost");

	// Cleanup
	unsafe {
		std::env::remove_var(format!("{prefix_low}PORT"));
		std::env::remove_var(format!("{prefix_high}PORT"));
	}
}

// ---------------------------------------------------------------------------
// Option B: SettingsOverride integration
// ---------------------------------------------------------------------------

#[rstest]
fn settings_override_with_build_composed_style() {
	// Arrange
	let _guard = SettingsOverride::new()
		.set("app_name", "test-app")
		.set_value("port", Value::Number(3000.into()))
		.activate();

	// Act
	let settings = SettingsBuilder::new()
		.add_source(
			DefaultSource::new()
				.with_value("app_name", Value::String("default-app".to_string()))
				.with_value("port", Value::Number(8080.into())),
		)
		.build()
		.unwrap();

	// Assert — override wins
	let name: String = settings.get("app_name").unwrap();
	assert_eq!(name, "test-app");
	// set_value with Number — override replaces the default Number
	let port = settings.get_raw("port").unwrap();
	assert_eq!(port, &Value::Number(3000.into()));
}

#[rstest]
fn settings_override_merges_into_existing_nested_object() {
	// Arrange — default has a nested email object
	let _guard = SettingsOverride::new()
		.set("email.host", "test-host")
		.set_value("email.port", Value::Number(2525.into()))
		.activate();

	// Act
	let settings = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value(
			"email",
			Value::Object(serde_json::Map::from_iter([
				("host".to_string(), Value::String("localhost".to_string())),
				("port".to_string(), Value::Number(1025.into())),
				("backend".to_string(), Value::String("console".to_string())),
			])),
		))
		.build()
		.unwrap();

	// Assert — overridden fields changed, non-overridden field preserved
	let email = settings.get_raw("email").unwrap().as_object().unwrap();
	assert_eq!(
		email.get("host").unwrap(),
		&Value::String("test-host".to_string())
	);
	assert_eq!(email.get("port").unwrap(), &Value::Number(2525.into()));
	assert_eq!(
		email.get("backend").unwrap(),
		&Value::String("console".to_string())
	);
}

// ---------------------------------------------------------------------------
// Combined: SettingsOverride wins over HighPriorityEnvSource
// ---------------------------------------------------------------------------

#[rstest]
#[serial(settings_override)]
fn settings_override_wins_over_high_priority_env() {
	// Arrange
	let prefix = "SOTEST_COMBO_";
	unsafe { std::env::set_var(format!("{prefix}PORT"), "6000") };

	let _guard = SettingsOverride::new().set("port", "7777").activate();

	// Act
	let settings = SettingsBuilder::new()
		.add_source(HighPriorityEnvSource::new().with_prefix(prefix))
		.build()
		.unwrap();

	// Assert — SettingsOverride (applied after all sources) wins
	let port: String = settings.get("port").unwrap();
	assert_eq!(port, "7777");

	// Cleanup
	unsafe { std::env::remove_var(format!("{prefix}PORT")) };
}

// ---------------------------------------------------------------------------
// Combined: TOML + HighPriorityEnvSource + SettingsOverride
// ---------------------------------------------------------------------------

#[rstest]
#[serial(settings_override)]
fn full_priority_chain_toml_high_env_override() {
	// Arrange
	let (_dir, toml_path) = write_toml("port = 1025\nbackend = \"smtp\"");

	let prefix = "SOTEST_FULL_";
	unsafe { std::env::set_var(format!("{prefix}PORT"), "5000") };

	let _guard = SettingsOverride::new().set("port", "9999").activate();

	// Act
	let settings = SettingsBuilder::new()
		.add_source(TomlFileSource::new(&toml_path))
		.add_source(HighPriorityEnvSource::new().with_prefix(prefix))
		.build()
		.unwrap();

	// Assert
	// port: SettingsOverride wins (applied last)
	let port: String = settings.get("port").unwrap();
	assert_eq!(port, "9999");

	// backend: TOML value survives (no override or env for it)
	let backend: String = settings.get("backend").unwrap();
	assert_eq!(backend, "smtp");

	// Cleanup
	unsafe { std::env::remove_var(format!("{prefix}PORT")) };
}
