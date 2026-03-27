// Integration tests for reinhardt-conf SettingsBuilder API.
// Covers: default values, overrides, validation, database config, middleware config,
// installed apps, profile, and error cases.
#![allow(deprecated)] // Tests exercise deprecated Settings for backward-compatibility verification

use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::DefaultSource;
use reinhardt_conf::settings::validation::{
	RequiredValidator, SecurityValidator, SettingsValidator,
};
use reinhardt_conf::settings::{DatabaseConfig, MiddlewareConfig, Settings, TemplateConfig};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serial_test::serial;
use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helper structs for into_typed tests
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct AppConfig {
	debug: bool,
	port: u16,
}

// ===========================================================================
// SettingsBuilder – empty builder
// ===========================================================================

#[rstest]
fn builder_empty_produces_no_keys() {
	// Arrange
	let builder = SettingsBuilder::new();

	// Act
	let settings = builder.build().unwrap();

	// Assert
	assert_eq!(settings.keys().count(), 0);
}

// ===========================================================================
// SettingsBuilder – single DefaultSource
// ===========================================================================

#[rstest]
fn builder_default_source_single_bool_value() {
	// Arrange
	let source = DefaultSource::new().with_value("debug", Value::Bool(true));

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();

	// Assert
	let debug: bool = settings.get("debug").unwrap();
	assert!(debug);
}

#[rstest]
fn builder_default_source_single_string_value() {
	// Arrange
	let source =
		DefaultSource::new().with_value("secret_key", Value::String("my-secret".to_string()));

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();

	// Assert
	let key: String = settings.get("secret_key").unwrap();
	assert_eq!(key, "my-secret");
}

#[rstest]
fn builder_default_source_numeric_value() {
	// Arrange
	let source = DefaultSource::new().with_value("port", Value::Number(8080.into()));

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();

	// Assert
	let port: u16 = settings.get("port").unwrap();
	assert_eq!(port, 8080);
}

#[rstest]
fn builder_default_source_multiple_values() {
	// Arrange
	let source = DefaultSource::new()
		.with_value("key1", Value::String("value1".to_string()))
		.with_value("key2", Value::String("value2".to_string()))
		.with_value("key3", Value::Number(42.into()));

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();

	// Assert
	let keys: Vec<_> = settings.keys().collect();
	assert_eq!(keys.len(), 3);
	assert!(settings.contains_key("key1"));
	assert!(settings.contains_key("key2"));
	assert!(settings.contains_key("key3"));
}

// ===========================================================================
// SettingsBuilder – contains_key
// ===========================================================================

#[rstest]
fn builder_contains_key_returns_true_for_existing() {
	// Arrange
	let settings = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("exists", Value::Bool(true)))
		.build()
		.unwrap();

	// Assert
	assert!(settings.contains_key("exists"));
}

#[rstest]
fn builder_contains_key_returns_false_for_missing() {
	// Arrange
	let settings = SettingsBuilder::new().build().unwrap();

	// Assert
	assert!(!settings.contains_key("nonexistent"));
}

// ===========================================================================
// SettingsBuilder – get_optional
// ===========================================================================

#[rstest]
fn builder_get_optional_returns_some_for_existing_key() {
	// Arrange
	let settings = SettingsBuilder::new()
		.add_source(
			DefaultSource::new().with_value("opt_key", Value::String("opt_val".to_string())),
		)
		.build()
		.unwrap();

	// Act
	let value: Option<String> = settings.get_optional("opt_key");

	// Assert
	assert_eq!(value, Some("opt_val".to_string()));
}

#[rstest]
fn builder_get_optional_returns_none_for_missing_key() {
	// Arrange
	let settings = SettingsBuilder::new().build().unwrap();

	// Act
	let value: Option<String> = settings.get_optional("missing");

	// Assert
	assert!(value.is_none());
}

// ===========================================================================
// SettingsBuilder – get_or
// ===========================================================================

#[rstest]
fn builder_get_or_returns_stored_value_when_key_exists() {
	// Arrange
	let settings = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("level", Value::String("info".to_string())))
		.build()
		.unwrap();

	// Act
	let level = settings.get_or("level", "debug".to_string());

	// Assert
	assert_eq!(level, "info");
}

#[rstest]
fn builder_get_or_returns_default_when_key_missing() {
	// Arrange
	let settings = SettingsBuilder::new().build().unwrap();

	// Act
	let level = settings.get_or("level", "warning".to_string());

	// Assert
	assert_eq!(level, "warning");
}

// ===========================================================================
// SettingsBuilder – get_raw
// ===========================================================================

#[rstest]
fn builder_get_raw_returns_json_value() {
	// Arrange
	let settings = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("raw_key", Value::Number(99.into())))
		.build()
		.unwrap();

	// Act
	let raw = settings.get_raw("raw_key");

	// Assert
	assert_eq!(raw, Some(&Value::Number(99.into())));
}

#[rstest]
fn builder_get_raw_returns_none_for_missing() {
	// Arrange
	let settings = SettingsBuilder::new().build().unwrap();

	// Assert
	assert!(settings.get_raw("missing").is_none());
}

// ===========================================================================
// SettingsBuilder – priority merging (later / higher priority wins)
// ===========================================================================

#[rstest]
fn builder_higher_priority_source_overwrites_lower() {
	// Arrange
	// DefaultSource has priority 0 (lowest). We add it first, then another
	// DefaultSource which will be sorted last because it has no higher priority
	// but is overwritten because sources with equal priority are merged in order.
	let low = DefaultSource::new().with_value("key", Value::String("low".to_string()));
	let high = DefaultSource::new().with_value("key", Value::String("high".to_string()));

	// Act
	// Two DefaultSource instances both have priority 0, but later one overwrites.
	let settings = SettingsBuilder::new()
		.add_source(low)
		.add_source(high)
		.build()
		.unwrap();

	// Assert
	let value: String = settings.get("key").unwrap();
	assert_eq!(value, "high");
}

#[rstest]
fn builder_env_source_has_higher_priority_than_default() {
	// Arrange – add DefaultSource with a value that env won't override
	let source =
		DefaultSource::new().with_value("my_config_key", Value::String("default_val".to_string()));

	// Act
	let settings = SettingsBuilder::new()
		.add_source(source)
		.with_env(None)
		.build()
		.unwrap();

	// Assert – key from default source still present
	assert!(settings.contains_key("my_config_key"));
}

// ===========================================================================
// SettingsBuilder – into_typed
// ===========================================================================

#[rstest]
fn builder_into_typed_deserializes_struct() {
	// Arrange
	let source = DefaultSource::new()
		.with_value("debug", Value::Bool(false))
		.with_value("port", Value::Number(3000.into()));

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();
	let config: AppConfig = settings.into_typed().unwrap();

	// Assert
	assert_eq!(
		config,
		AppConfig {
			debug: false,
			port: 3000
		}
	);
}

#[rstest]
fn builder_into_typed_fails_on_type_mismatch() {
	// Arrange – port is a string, not a number
	let source = DefaultSource::new()
		.with_value("debug", Value::Bool(true))
		.with_value("port", Value::String("not-a-number".to_string()));

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();
	let result: Result<AppConfig, _> = settings.into_typed();

	// Assert
	assert!(result.is_err());
}

// ===========================================================================
// SettingsBuilder – as_map
// ===========================================================================

#[rstest]
fn builder_as_map_contains_all_keys() {
	// Arrange
	let source = DefaultSource::new()
		.with_value("alpha", Value::String("a".to_string()))
		.with_value("beta", Value::String("b".to_string()));

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();
	let map = settings.as_map();

	// Assert
	assert!(map.contains_key("alpha"));
	assert!(map.contains_key("beta"));
	assert_eq!(map.len(), 2);
}

// ===========================================================================
// SettingsBuilder – profile
// ===========================================================================

#[rstest]
#[case(Profile::Development)]
#[case(Profile::Staging)]
#[case(Profile::Production)]
#[case(Profile::Custom)]
fn builder_profile_is_preserved(#[case] profile: Profile) {
	// Arrange
	let builder = SettingsBuilder::new().profile(profile);

	// Act
	let settings = builder.build().unwrap();

	// Assert
	assert_eq!(settings.profile(), Some(profile));
}

#[rstest]
fn builder_without_profile_returns_none() {
	// Arrange
	let builder = SettingsBuilder::new();

	// Act
	let settings = builder.build().unwrap();

	// Assert
	assert_eq!(settings.profile(), None);
}

// ===========================================================================
// SettingsBuilder – strict mode (compilation / invocation test)
// ===========================================================================

#[rstest]
fn builder_strict_mode_builds_without_error_when_no_validation_added() {
	// Arrange
	let builder = SettingsBuilder::new().strict(true);

	// Act
	let result = builder.build();

	// Assert
	assert!(result.is_ok());
}

// ===========================================================================
// SettingsBuilder – with_defaults via HashMap
// ===========================================================================

#[rstest]
fn builder_default_source_with_defaults_hashmap() {
	// Arrange
	let mut defaults = HashMap::new();
	defaults.insert("h1".to_string(), Value::String("hello".to_string()));
	defaults.insert("h2".to_string(), Value::Bool(true));
	let source = DefaultSource::new().with_defaults(defaults);

	// Act
	let settings = SettingsBuilder::new().add_source(source).build().unwrap();

	// Assert
	let h1: String = settings.get("h1").unwrap();
	let h2: bool = settings.get("h2").unwrap();
	assert_eq!(h1, "hello");
	assert!(h2);
}

// ===========================================================================
// SettingsBuilder – clone of MergedSettings
// ===========================================================================

#[rstest]
fn merged_settings_clone_is_independent() {
	// Arrange
	let settings = SettingsBuilder::new()
		.add_source(
			DefaultSource::new().with_value("clone_key", Value::String("original".to_string())),
		)
		.build()
		.unwrap();

	// Act
	let cloned = settings.clone();

	// Assert
	let original_val: String = settings.get("clone_key").unwrap();
	let cloned_val: String = cloned.get("clone_key").unwrap();
	assert_eq!(original_val, cloned_val);
}

// ===========================================================================
// Settings struct – default values
// ===========================================================================

#[rstest]
fn settings_default_debug_is_true() {
	// Act
	let settings = Settings::default();

	// Assert
	assert!(settings.core.debug);
}

#[rstest]
fn settings_default_time_zone_is_utc() {
	// Act
	let settings = Settings::default();

	// Assert
	assert_eq!(settings.time_zone, "UTC");
}

#[rstest]
fn settings_default_language_code_is_en_us() {
	// Act
	let settings = Settings::default();

	// Assert
	assert_eq!(settings.language_code, "en-us");
}

#[rstest]
#[allow(deprecated)] // Test: verifies deprecated `installed_apps` field behavior
fn settings_default_installed_apps_is_empty() {
	// Act
	let settings = Settings::default();

	// Assert
	assert!(settings.core.installed_apps.is_empty());
}

#[rstest]
fn settings_default_middleware_is_empty() {
	// Act
	let settings = Settings::default();

	// Assert
	assert!(settings.core.middleware.is_empty());
}

#[rstest]
fn settings_default_static_url() {
	// Act
	let settings = Settings::default();

	// Assert
	assert_eq!(settings.static_url, "/static/");
}

#[rstest]
fn settings_default_media_url() {
	// Act
	let settings = Settings::default();

	// Assert
	assert_eq!(settings.media_url, "/media/");
}

#[rstest]
fn settings_default_databases_has_default_entry() {
	// Act
	let settings = Settings::default();

	// Assert
	assert!(settings.core.databases.contains_key("default"));
}

#[rstest]
fn settings_default_append_slash_is_true() {
	// Act
	let settings = Settings::default();

	// Assert
	assert!(settings.core.security.append_slash);
}

// ===========================================================================
// Settings struct – new() constructor
// ===========================================================================

#[rstest]
fn settings_new_sets_base_dir_and_secret_key() {
	// Arrange
	let base_dir = PathBuf::from("/app");
	let secret_key = "test-secret-key-12345".to_string();

	// Act
	let settings = Settings::new(base_dir.clone(), secret_key.clone());

	// Assert
	assert_eq!(settings.core.base_dir, base_dir);
	assert_eq!(settings.core.secret_key, secret_key);
}

#[rstest]
fn settings_new_debug_defaults_to_true() {
	// Act
	let settings = Settings::new(PathBuf::from("/app"), "some-key".to_string());

	// Assert
	assert!(settings.core.debug);
}

// ===========================================================================
// Settings struct – add_app / installed_apps
// ===========================================================================

#[rstest]
#[allow(deprecated)] // Test: verifies deprecated `add_app` method behavior
fn settings_add_app_increases_count() {
	// Arrange
	let mut settings = Settings::default();

	// Act
	settings.add_app("myapp");

	// Assert
	assert_eq!(settings.core.installed_apps.len(), 1);
	assert!(settings.core.installed_apps.contains(&"myapp".to_string()));
}

#[rstest]
#[allow(deprecated)] // Test: verifies deprecated `add_app` method behavior
fn settings_add_multiple_apps() {
	// Arrange
	let mut settings = Settings::default();

	// Act
	settings.add_app("app_a");
	settings.add_app("app_b");
	settings.add_app("app_c");

	// Assert
	assert_eq!(settings.core.installed_apps.len(), 3);
	assert!(settings.core.installed_apps.contains(&"app_a".to_string()));
	assert!(settings.core.installed_apps.contains(&"app_b".to_string()));
	assert!(settings.core.installed_apps.contains(&"app_c".to_string()));
}

#[rstest]
#[allow(deprecated)] // Test: verifies deprecated `with_validated_apps` method behavior
fn settings_with_validated_apps_replaces_apps_list() {
	// Arrange
	let mut settings = Settings::default();
	settings.add_app("old_app");

	// Act
	let settings =
		settings.with_validated_apps(|| vec!["new_app_one".to_string(), "new_app_two".to_string()]);

	// Assert
	assert_eq!(settings.core.installed_apps.len(), 2);
	assert!(
		!settings
			.core
			.installed_apps
			.contains(&"old_app".to_string())
	);
	assert!(
		settings
			.core
			.installed_apps
			.contains(&"new_app_one".to_string())
	);
}

// ===========================================================================
// Settings struct – middleware
// ===========================================================================

#[rstest]
fn settings_middleware_field_can_be_set_directly() {
	// Arrange
	let mut settings = Settings::default();

	// Act
	settings.core.middleware = vec![
		"reinhardt.middleware.SecurityMiddleware".to_string(),
		"reinhardt.middleware.SessionMiddleware".to_string(),
	];

	// Assert
	assert_eq!(settings.core.middleware.len(), 2);
	assert_eq!(
		settings.core.middleware[0],
		"reinhardt.middleware.SecurityMiddleware"
	);
}

// ===========================================================================
// Settings struct – admin / manager contacts
// ===========================================================================

#[rstest]
fn settings_add_admin_and_manager() {
	// Arrange
	let mut settings = Settings::default();

	// Act
	settings.add_admin("Alice", "alice@example.com");
	settings.add_manager("Bob", "bob@example.com");

	// Assert
	assert_eq!(settings.admins.len(), 1);
	assert_eq!(settings.admins[0].name, "Alice");
	assert_eq!(settings.managers.len(), 1);
	assert_eq!(settings.managers[0].name, "Bob");
}

#[rstest]
fn settings_managers_from_admins_copies_all() {
	// Arrange
	let mut settings = Settings::default();
	settings.add_admin("Admin One", "one@example.com");
	settings.add_admin("Admin Two", "two@example.com");

	// Act
	settings.managers_from_admins();

	// Assert
	assert_eq!(settings.managers.len(), 2);
	assert_eq!(settings.managers, settings.admins);
}

// ===========================================================================
// DatabaseConfig – factory methods
// ===========================================================================

#[rstest]
fn database_config_sqlite_factory() {
	// Act
	let db = DatabaseConfig::sqlite("myapp.db");

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
	assert_eq!(db.name, "myapp.db");
	assert!(db.user.is_none());
	assert!(db.password.is_none());
	assert!(db.host.is_none());
	assert!(db.port.is_none());
}

#[rstest]
fn database_config_postgresql_factory() {
	// Act
	let db = DatabaseConfig::postgresql("appdb", "admin", "secret", "db.local", 5432);

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
	assert_eq!(db.name, "appdb");
	assert_eq!(db.user, Some("admin".to_string()));
	assert_eq!(db.host, Some("db.local".to_string()));
	assert_eq!(db.port, Some(5432));
}

#[rstest]
fn database_config_mysql_factory() {
	// Act
	let db = DatabaseConfig::mysql("appdb", "root", "rootpass", "mysql.local", 3306);

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.mysql");
	assert_eq!(db.name, "appdb");
	assert_eq!(db.user, Some("root".to_string()));
	assert_eq!(db.port, Some(3306));
}

#[rstest]
fn database_config_to_url_sqlite_relative() {
	// Arrange
	let db = DatabaseConfig::sqlite("data.db");

	// Act
	let url = db.to_url();

	// Assert
	assert_eq!(url, "sqlite:data.db");
}

#[rstest]
fn database_config_to_url_postgresql() {
	// Arrange
	let db = DatabaseConfig::postgresql("mydb", "user", "pass", "localhost", 5432);

	// Act
	let url = db.to_url();

	// Assert
	assert_eq!(url, "postgresql://user:pass@localhost:5432/mydb");
}

#[rstest]
fn database_config_builder_methods() {
	// Act
	let db = DatabaseConfig::new("reinhardt.db.backends.postgresql", "testdb")
		.with_user("dbuser")
		.with_password("dbpass")
		.with_host("postgres.local")
		.with_port(5433);

	// Assert
	assert_eq!(db.user, Some("dbuser".to_string()));
	assert_eq!(db.host, Some("postgres.local".to_string()));
	assert_eq!(db.port, Some(5433));
}

#[rstest]
fn database_config_default_is_sqlite() {
	// Act
	let db = DatabaseConfig::default();

	// Assert
	assert!(db.engine.contains("sqlite"));
}

// ===========================================================================
// MiddlewareConfig
// ===========================================================================

#[rstest]
fn middleware_config_new_has_empty_options() {
	// Act
	let mw = MiddlewareConfig::new("myapp.middleware.Auth");

	// Assert
	assert_eq!(mw.path, "myapp.middleware.Auth");
	assert!(mw.options.is_empty());
}

#[rstest]
fn middleware_config_with_option_stores_value() {
	// Act
	let mw = MiddlewareConfig::new("myapp.middleware.Timeout")
		.with_option("timeout_secs", serde_json::json!(60));

	// Assert
	assert_eq!(mw.options.get("timeout_secs"), Some(&serde_json::json!(60)));
}

#[rstest]
fn middleware_config_multiple_options() {
	// Act
	let mw = MiddlewareConfig::new("myapp.middleware.Cors")
		.with_option("allow_all", serde_json::json!(true))
		.with_option("max_age", serde_json::json!(3600));

	// Assert
	assert_eq!(mw.options.len(), 2);
	assert_eq!(mw.options.get("allow_all"), Some(&serde_json::json!(true)));
}

// ===========================================================================
// TemplateConfig
// ===========================================================================

#[rstest]
fn template_config_default_backend() {
	// Act
	let cfg = TemplateConfig::default();

	// Assert
	assert_eq!(cfg.backend, "reinhardt.template.backends.jinja2.Jinja2");
	assert!(cfg.app_dirs);
}

#[rstest]
fn template_config_add_dir_appends_path() {
	// Arrange
	let cfg =
		TemplateConfig::new("reinhardt.template.backends.jinja2.Jinja2").add_dir("/app/templates");

	// Assert
	assert_eq!(cfg.dirs.len(), 1);
	assert_eq!(cfg.dirs[0], PathBuf::from("/app/templates"));
}

// ===========================================================================
// Settings struct – static / media config
// ===========================================================================

#[rstest]
fn settings_get_static_config_fails_when_static_root_not_set() {
	// Arrange
	let settings = Settings::default();

	// Act
	let result = settings.get_static_config();

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn settings_get_static_config_succeeds_when_static_root_is_set() {
	// Arrange
	let mut settings = Settings::default();
	settings.static_root = Some(PathBuf::from("/app/static"));

	// Act
	let result = settings.get_static_config();

	// Assert
	assert!(result.is_ok());
	let config = result.unwrap();
	assert_eq!(config.static_url, "/static/");
}

// ===========================================================================
// Validation – RequiredValidator
// ===========================================================================

#[rstest]
fn required_validator_passes_when_all_fields_present() {
	// Arrange
	let validator = RequiredValidator::new(vec!["secret_key".to_string(), "debug".to_string()]);
	let mut settings = HashMap::new();
	settings.insert(
		"secret_key".to_string(),
		Value::String("some-key".to_string()),
	);
	settings.insert("debug".to_string(), Value::Bool(false));

	// Act
	let result = validator.validate_settings(&settings);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn required_validator_fails_when_field_missing() {
	// Arrange
	let validator =
		RequiredValidator::new(vec!["secret_key".to_string(), "database_url".to_string()]);
	let mut settings = HashMap::new();
	settings.insert(
		"secret_key".to_string(),
		Value::String("some-key".to_string()),
	);
	// database_url is intentionally absent

	// Act
	let result = validator.validate_settings(&settings);

	// Assert
	assert!(result.is_err());
}

// ===========================================================================
// Validation – SecurityValidator (non-production skips checks)
// ===========================================================================

#[rstest]
fn security_validator_passes_for_development_regardless_of_debug() {
	// Arrange
	let validator = SecurityValidator::new(Profile::Development);
	let mut settings = HashMap::new();
	settings.insert("debug".to_string(), Value::Bool(true));

	// Act
	let result = validator.validate_settings(&settings);

	// Assert
	assert!(result.is_ok());
}

#[rstest]
fn security_validator_fails_for_production_with_debug_true() {
	// Arrange
	let validator = SecurityValidator::new(Profile::Production);
	let mut settings = HashMap::new();
	settings.insert("debug".to_string(), Value::Bool(true));
	settings.insert("secret_key".to_string(), Value::String("a".repeat(40)));
	settings.insert(
		"allowed_hosts".to_string(),
		Value::Array(vec![Value::String("example.com".to_string())]),
	);
	settings.insert("secure_ssl_redirect".to_string(), Value::Bool(true));

	// Act
	let result = validator.validate_settings(&settings);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn security_validator_fails_for_production_with_empty_allowed_hosts() {
	// Arrange
	let validator = SecurityValidator::new(Profile::Production);
	let mut settings = HashMap::new();
	settings.insert("debug".to_string(), Value::Bool(false));
	settings.insert("secret_key".to_string(), Value::String("a".repeat(40)));
	settings.insert("allowed_hosts".to_string(), Value::Array(vec![]));
	settings.insert("secure_ssl_redirect".to_string(), Value::Bool(true));

	// Act
	let result = validator.validate_settings(&settings);

	// Assert
	assert!(result.is_err());
}

// ===========================================================================
// SettingsBuilder – with_env prefix filtering
// ===========================================================================

#[rstest]
#[serial(env)]
fn builder_with_env_no_prefix_loads_env_vars() {
	// Arrange
	let builder = SettingsBuilder::new().with_env(None);

	// Act
	let settings = builder.build().unwrap();

	// Assert – PATH is always present in a shell environment
	assert!(settings.contains_key("path") || settings.keys().count() > 0);
}

#[rstest]
#[serial(env)]
fn builder_with_env_prefix_loads_only_matching_vars() {
	// Arrange – set a test-specific env var
	// SAFETY: manipulating env vars is inherently racy in multi-threaded tests,
	// but this is an integration test running in a dedicated process.
	unsafe {
		std::env::set_var("RTEST_MY_SETTING", "hello");
	}

	// Act
	let settings = SettingsBuilder::new()
		.with_env(Some("RTEST_"))
		.build()
		.unwrap();

	// Assert
	assert!(settings.contains_key("my_setting"));
	let val: String = settings.get("my_setting").unwrap();
	assert_eq!(val, "hello");

	// Cleanup
	unsafe {
		std::env::remove_var("RTEST_MY_SETTING");
	}
}
