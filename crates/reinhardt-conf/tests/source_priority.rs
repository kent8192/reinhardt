// Integration tests for EnvSource type inference, LowPriorityEnvSource, and cross-priority merging.
// Covers: smart parsing, prefix handling, priority chain (Default → LowPriorityEnv → Toml/Json → Env).

use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::sources::{
	ConfigSource, DefaultSource, EnvSource, JsonFileSource, LowPriorityEnvSource, TomlFileSource,
};
use rstest::rstest;
use serde_json::Value;
use serial_test::serial;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_toml_file(content: &str) -> (TempDir, PathBuf) {
	let dir = TempDir::new().unwrap();
	let path = dir.path().join("config.toml");
	let mut file = std::fs::File::create(&path).unwrap();
	file.write_all(content.as_bytes()).unwrap();
	(dir, path)
}

fn write_json_file(content: &str) -> (TempDir, PathBuf) {
	let dir = TempDir::new().unwrap();
	let path = dir.path().join("config.json");
	let mut file = std::fs::File::create(&path).unwrap();
	file.write_all(content.as_bytes()).unwrap();
	(dir, path)
}

/// Set environment variables for test. Returns the keys for cleanup.
///
/// # Safety
/// Caller must ensure exclusive access to environment variables (use `#[serial]`).
unsafe fn set_env_vars(vars: &[(&str, &str)]) {
	for (key, value) in vars {
		unsafe { std::env::set_var(key, value) };
	}
}

/// Remove environment variables after test.
///
/// # Safety
/// Caller must ensure exclusive access to environment variables (use `#[serial]`).
unsafe fn remove_env_vars(vars: &[&str]) {
	for key in vars {
		unsafe { std::env::remove_var(key) };
	}
}

// ===========================================================================
// EnvSource – type inference and smart parsing
// ===========================================================================

#[rstest]
#[serial(env)]
fn env_source_infers_integer_type() {
	// Arrange
	unsafe { set_env_vars(&[("SRCTEST_PORT", "8080")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	assert_eq!(config.get("port").unwrap(), &Value::Number(8080.into()));

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_PORT"]) };
}

#[rstest]
#[serial(env)]
fn env_source_infers_bool_type() {
	// Arrange
	unsafe { set_env_vars(&[("SRCTEST_ENABLED", "true")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	assert_eq!(config.get("enabled").unwrap(), &Value::Bool(true));

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_ENABLED"]) };
}

#[rstest]
#[serial(env)]
fn env_source_debug_key_smart_parsing_numeric() {
	// Arrange – "1" should be parsed as Bool(true) for the debug key
	unsafe { set_env_vars(&[("SRCTEST_DEBUG", "1")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	assert_eq!(config.get("debug").unwrap(), &Value::Bool(true));

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_DEBUG"]) };
}

#[rstest]
#[serial(env)]
fn env_source_debug_key_parses_yes() {
	// Arrange
	unsafe { set_env_vars(&[("SRCTEST_DEBUG", "yes")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	assert_eq!(config.get("debug").unwrap(), &Value::Bool(true));

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_DEBUG"]) };
}

#[rstest]
#[serial(env)]
fn env_source_debug_key_parses_off() {
	// Arrange
	unsafe { set_env_vars(&[("SRCTEST_DEBUG", "off")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	assert_eq!(config.get("debug").unwrap(), &Value::Bool(false));

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_DEBUG"]) };
}

#[rstest]
#[serial(env)]
fn env_source_allowed_hosts_parsed_as_array() {
	// Arrange
	unsafe { set_env_vars(&[("SRCTEST_ALLOWED_HOSTS", "a.com,b.com")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	let hosts = config.get("allowed_hosts").unwrap();
	assert!(hosts.is_array());
	let arr = hosts.as_array().unwrap();
	assert_eq!(arr.len(), 2);
	assert_eq!(arr[0], Value::String("a.com".to_string()));
	assert_eq!(arr[1], Value::String("b.com".to_string()));

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_ALLOWED_HOSTS"]) };
}

#[rstest]
#[serial(env)]
fn env_source_string_value_remains_string() {
	// Arrange
	unsafe { set_env_vars(&[("SRCTEST_NAME", "myapp")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	assert_eq!(
		config.get("name").unwrap(),
		&Value::String("myapp".to_string())
	);

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_NAME"]) };
}

#[rstest]
#[serial(env)]
fn env_source_strips_prefix_and_lowercases() {
	// Arrange
	unsafe { set_env_vars(&[("SRCTEST_MY_KEY", "val")]) };

	// Act
	let source = EnvSource::new().with_prefix("SRCTEST_");
	let config = source.load().unwrap();

	// Assert
	assert!(config.contains_key("my_key"));
	assert_eq!(
		config.get("my_key").unwrap(),
		&Value::String("val".to_string())
	);

	// Cleanup
	unsafe { remove_env_vars(&["SRCTEST_MY_KEY"]) };
}

// ===========================================================================
// LowPriorityEnvSource
// ===========================================================================

#[rstest]
fn low_priority_env_source_priority_is_40() {
	// Assert
	assert_eq!(LowPriorityEnvSource::new().priority(), 40);
}

#[rstest]
#[serial(env)]
fn low_priority_env_source_loads_env_vars() {
	// Arrange
	unsafe { set_env_vars(&[("LPTEST_PORT", "9090")]) };

	// Act
	let source = LowPriorityEnvSource::new().with_prefix("LPTEST_");
	let config = source.load().unwrap();

	// Assert
	assert_eq!(config.get("port").unwrap(), &Value::Number(9090.into()));

	// Cleanup
	unsafe { remove_env_vars(&["LPTEST_PORT"]) };
}

#[rstest]
#[serial(env)]
fn low_priority_env_source_with_prefix_filters() {
	// Arrange
	unsafe { set_env_vars(&[("LPTEST_INSIDE", "yes_val"), ("OTHER_OUTSIDE", "no_val")]) };

	// Act
	let source = LowPriorityEnvSource::new().with_prefix("LPTEST_");
	let config = source.load().unwrap();

	// Assert
	assert!(config.contains_key("inside"));
	assert!(!config.contains_key("outside"));
	assert!(!config.contains_key("other_outside"));

	// Cleanup
	unsafe { remove_env_vars(&["LPTEST_INSIDE", "OTHER_OUTSIDE"]) };
}

#[rstest]
fn low_priority_env_source_description_contains_low_priority() {
	// Act
	let desc = LowPriorityEnvSource::new().description();

	// Assert
	assert!(
		desc.to_lowercase().contains("low"),
		"description should contain 'low', got: {desc}"
	);
}

// ===========================================================================
// Cross-priority merging
// ===========================================================================

#[rstest]
#[serial(env)]
fn env_overrides_toml() {
	// Arrange
	let (_dir, toml_path) = write_toml_file("port = 3000\n");
	unsafe { set_env_vars(&[("MPTEST_PORT", "9000")]) };

	// Act
	let settings = SettingsBuilder::new()
		.add_source(TomlFileSource::new(&toml_path))
		.add_source(EnvSource::new().with_prefix("MPTEST_"))
		.build()
		.unwrap();

	// Assert – Env(100) wins over Toml(50)
	let port: u16 = settings.get("port").unwrap();
	assert_eq!(port, 9000);

	// Cleanup
	unsafe { remove_env_vars(&["MPTEST_PORT"]) };
}

#[rstest]
fn toml_overrides_default() {
	// Arrange
	let (_dir, toml_path) = write_toml_file("debug = false\n");
	let default = DefaultSource::new().with_value("debug", Value::Bool(true));

	// Act
	let settings = SettingsBuilder::new()
		.add_source(default)
		.add_source(TomlFileSource::new(&toml_path))
		.build()
		.unwrap();

	// Assert – Toml(50) wins over Default(0)
	let debug: bool = settings.get("debug").unwrap();
	assert!(!debug);
}

#[rstest]
#[serial(env)]
fn toml_overrides_low_priority_env() {
	// Arrange
	let (_dir, toml_path) = write_toml_file("port = 5000\n");
	unsafe { set_env_vars(&[("LPMPTEST_PORT", "4000")]) };

	// Act
	let settings = SettingsBuilder::new()
		.add_source(LowPriorityEnvSource::new().with_prefix("LPMPTEST_"))
		.add_source(TomlFileSource::new(&toml_path))
		.build()
		.unwrap();

	// Assert – Toml(50) wins over LowPriorityEnv(40)
	let port: u16 = settings.get("port").unwrap();
	assert_eq!(port, 5000);

	// Cleanup
	unsafe { remove_env_vars(&["LPMPTEST_PORT"]) };
}

#[rstest]
#[serial(env)]
fn low_priority_env_overrides_default() {
	// Arrange
	let default = DefaultSource::new().with_value("timeout", Value::Number(30.into()));
	unsafe { set_env_vars(&[("LPMPTEST2_TIMEOUT", "60")]) };

	// Act
	let settings = SettingsBuilder::new()
		.add_source(default)
		.add_source(LowPriorityEnvSource::new().with_prefix("LPMPTEST2_"))
		.build()
		.unwrap();

	// Assert – LowPriorityEnv(40) wins over Default(0)
	let timeout: u64 = settings.get("timeout").unwrap();
	assert_eq!(timeout, 60);

	// Cleanup
	unsafe { remove_env_vars(&["LPMPTEST2_TIMEOUT"]) };
}

#[rstest]
fn json_overrides_default() {
	// Arrange
	let (_dir, json_path) = write_json_file(r#"{"name": "from_json"}"#);
	let default = DefaultSource::new().with_value("name", Value::String("default".to_string()));

	// Act
	let settings = SettingsBuilder::new()
		.add_source(default)
		.add_source(JsonFileSource::new(&json_path))
		.build()
		.unwrap();

	// Assert – Json(50) wins over Default(0)
	let name: String = settings.get("name").unwrap();
	assert_eq!(name, "from_json");
}

#[rstest]
#[serial(env)]
fn env_overrides_json() {
	// Arrange
	let (_dir, json_path) = write_json_file(r#"{"port": 2000}"#);
	unsafe { set_env_vars(&[("MPTEST2_PORT", "7000")]) };

	// Act
	let settings = SettingsBuilder::new()
		.add_source(JsonFileSource::new(&json_path))
		.add_source(EnvSource::new().with_prefix("MPTEST2_"))
		.build()
		.unwrap();

	// Assert – Env(100) wins over Json(50)
	let port: u16 = settings.get("port").unwrap();
	assert_eq!(port, 7000);

	// Cleanup
	unsafe { remove_env_vars(&["MPTEST2_PORT"]) };
}

#[rstest]
#[serial(env)]
fn full_chain_same_key_highest_priority_wins() {
	// Arrange – all sources provide "port" with different values
	let default = DefaultSource::new().with_value("port", Value::Number(1000.into()));
	let (_dir, toml_path) = write_toml_file("port = 3000\n");
	unsafe { set_env_vars(&[("FCTEST_PORT", "9999")]) };

	// LowPriorityEnv will also pick up FCTEST_PORT
	let low_env = LowPriorityEnvSource::new().with_prefix("FCTEST_");
	let high_env = EnvSource::new().with_prefix("FCTEST_");

	// Act
	let settings = SettingsBuilder::new()
		.add_source(default) // priority 0
		.add_source(low_env) // priority 40
		.add_source(TomlFileSource::new(&toml_path)) // priority 50
		.add_source(high_env) // priority 100
		.build()
		.unwrap();

	// Assert – Env(100) wins
	let port: u16 = settings.get("port").unwrap();
	assert_eq!(port, 9999);

	// Cleanup
	unsafe { remove_env_vars(&["FCTEST_PORT"]) };
}

#[rstest]
#[serial(env)]
fn full_chain_disjoint_keys_all_present() {
	// Arrange – each source provides a unique key
	let default = DefaultSource::new().with_value("default_key", Value::String("dv".to_string()));
	let (_dir, toml_path) = write_toml_file("toml_key = \"tv\"\n");
	unsafe { set_env_vars(&[("FCTEST2_ENV_KEY", "ev")]) };

	// Act
	let settings = SettingsBuilder::new()
		.add_source(default)
		.add_source(TomlFileSource::new(&toml_path))
		.add_source(EnvSource::new().with_prefix("FCTEST2_"))
		.build()
		.unwrap();

	// Assert – all disjoint keys present
	assert!(settings.contains_key("default_key"));
	assert!(settings.contains_key("toml_key"));
	assert!(settings.contains_key("env_key"));

	let dv: String = settings.get("default_key").unwrap();
	assert_eq!(dv, "dv");
	let tv: String = settings.get("toml_key").unwrap();
	assert_eq!(tv, "tv");
	let ev: String = settings.get("env_key").unwrap();
	assert_eq!(ev, "ev");

	// Cleanup
	unsafe { remove_env_vars(&["FCTEST2_ENV_KEY"]) };
}

#[rstest]
#[serial(env)]
fn full_chain_partial_overlap() {
	// Arrange – "port" overlaps between default and toml; "name" is unique to default;
	// "host" is unique to env
	let default = DefaultSource::new()
		.with_value("port", Value::Number(1111.into()))
		.with_value("name", Value::String("default_app".to_string()));
	let (_dir, toml_path) = write_toml_file("port = 2222\n");
	unsafe { set_env_vars(&[("FCTEST3_HOST", "env.local")]) };

	// Act
	let settings = SettingsBuilder::new()
		.add_source(default) // priority 0
		.add_source(TomlFileSource::new(&toml_path)) // priority 50
		.add_source(EnvSource::new().with_prefix("FCTEST3_")) // priority 100
		.build()
		.unwrap();

	// Assert – overlapping "port" won by Toml(50) over Default(0)
	let port: u16 = settings.get("port").unwrap();
	assert_eq!(port, 2222);

	// Assert – unique keys all present
	let name: String = settings.get("name").unwrap();
	assert_eq!(name, "default_app");
	let host: String = settings.get("host").unwrap();
	assert_eq!(host, "env.local");

	// Cleanup
	unsafe { remove_env_vars(&["FCTEST3_HOST"]) };
}
