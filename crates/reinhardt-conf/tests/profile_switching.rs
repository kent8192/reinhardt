//! Integration tests for profile switching in reinhardt-conf.
//!
//! Covers Development, Staging, and Production profile behavior including
//! default debug flags, allowed hosts configuration, and database config per profile.

use reinhardt_conf::settings::DatabaseConfig;
use reinhardt_conf::settings::Settings;
use reinhardt_conf::settings::builder::SettingsBuilder;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::DefaultSource;
use rstest::rstest;
use serde_json::Value;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Profile::parse tests
// ---------------------------------------------------------------------------

#[rstest]
fn profile_parse_development_aliases() {
	// Assert
	assert_eq!(Profile::parse("development"), Profile::Development);
	assert_eq!(Profile::parse("dev"), Profile::Development);
	assert_eq!(Profile::parse("develop"), Profile::Development);
}

#[rstest]
fn profile_parse_staging_aliases() {
	// Assert
	assert_eq!(Profile::parse("staging"), Profile::Staging);
	assert_eq!(Profile::parse("stage"), Profile::Staging);
	assert_eq!(Profile::parse("test"), Profile::Staging);
}

#[rstest]
fn profile_parse_production_aliases() {
	// Assert
	assert_eq!(Profile::parse("production"), Profile::Production);
	assert_eq!(Profile::parse("prod"), Profile::Production);
}

#[rstest]
fn profile_parse_case_insensitive() {
	// Assert
	assert_eq!(Profile::parse("DEVELOPMENT"), Profile::Development);
	assert_eq!(Profile::parse("STAGING"), Profile::Staging);
	assert_eq!(Profile::parse("PRODUCTION"), Profile::Production);
	assert_eq!(Profile::parse("Dev"), Profile::Development);
	assert_eq!(Profile::parse("Prod"), Profile::Production);
}

#[rstest]
fn profile_parse_unknown_returns_custom() {
	// Assert
	assert_eq!(Profile::parse("unknown"), Profile::Custom);
	assert_eq!(Profile::parse(""), Profile::Custom);
	assert_eq!(Profile::parse("local"), Profile::Custom);
}

// ---------------------------------------------------------------------------
// Profile identity / predicate tests
// ---------------------------------------------------------------------------

#[rstest]
fn profile_is_production() {
	// Arrange
	let prod = Profile::Production;
	let dev = Profile::Development;
	let staging = Profile::Staging;
	let custom = Profile::Custom;

	// Assert
	assert!(prod.is_production());
	assert!(!dev.is_production());
	assert!(!staging.is_production());
	assert!(!custom.is_production());
}

#[rstest]
fn profile_is_development() {
	// Arrange
	let dev = Profile::Development;
	let prod = Profile::Production;
	let staging = Profile::Staging;

	// Assert
	assert!(dev.is_development());
	assert!(!prod.is_development());
	assert!(!staging.is_development());
}

#[rstest]
fn profile_default_is_development() {
	// Act
	let profile = Profile::default();

	// Assert
	assert_eq!(profile, Profile::Development);
	assert!(profile.is_development());
}

// ---------------------------------------------------------------------------
// DEBUG flag defaults per profile
// ---------------------------------------------------------------------------

#[rstest]
fn development_profile_enables_debug_by_default() {
	// Arrange
	let profile = Profile::Development;

	// Act
	let debug = profile.default_debug();

	// Assert – development should have debug enabled
	assert!(debug);
}

#[rstest]
fn staging_profile_enables_debug_by_default() {
	// Arrange
	let profile = Profile::Staging;

	// Act
	let debug = profile.default_debug();

	// Assert – staging (non-production) has debug enabled
	assert!(debug);
}

#[rstest]
fn production_profile_disables_debug_by_default() {
	// Arrange
	let profile = Profile::Production;

	// Act
	let debug = profile.default_debug();

	// Assert – production should never have debug
	assert!(!debug);
}

#[rstest]
fn custom_profile_enables_debug_by_default() {
	// Arrange
	let profile = Profile::Custom;

	// Act
	let debug = profile.default_debug();

	// Assert – custom is not production, so debug is on
	assert!(debug);
}

// ---------------------------------------------------------------------------
// as_str / Display per profile
// ---------------------------------------------------------------------------

#[rstest]
fn profile_as_str_values() {
	// Assert
	assert_eq!(Profile::Development.as_str(), "development");
	assert_eq!(Profile::Staging.as_str(), "staging");
	assert_eq!(Profile::Production.as_str(), "production");
	assert_eq!(Profile::Custom.as_str(), "custom");
}

#[rstest]
fn profile_display_matches_as_str() {
	// Arrange
	let profiles = [
		Profile::Development,
		Profile::Staging,
		Profile::Production,
		Profile::Custom,
	];

	// Assert
	for profile in &profiles {
		assert_eq!(format!("{}", profile), profile.as_str());
	}
}

// ---------------------------------------------------------------------------
// env_file_name per profile
// ---------------------------------------------------------------------------

#[rstest]
fn env_file_name_development() {
	// Arrange
	let profile = Profile::Development;

	// Act
	let name = profile.env_file_name();

	// Assert
	assert_eq!(name, ".env.development");
}

#[rstest]
fn env_file_name_staging() {
	// Arrange
	let profile = Profile::Staging;

	// Act
	let name = profile.env_file_name();

	// Assert
	assert_eq!(name, ".env.staging");
}

#[rstest]
fn env_file_name_production() {
	// Arrange
	let profile = Profile::Production;

	// Act
	let name = profile.env_file_name();

	// Assert
	assert_eq!(name, ".env.production");
}

#[rstest]
fn env_file_name_custom() {
	// Arrange
	let profile = Profile::Custom;

	// Act
	let name = profile.env_file_name();

	// Assert – custom profile falls back to generic .env
	assert_eq!(name, ".env");
}

// ---------------------------------------------------------------------------
// Settings struct + profile-driven configuration
// ---------------------------------------------------------------------------

#[rstest]
fn settings_development_has_debug_true_by_default() {
	// Arrange
	let settings = Settings::default();

	// Assert – Settings::new() sets debug=true which corresponds to development defaults
	assert!(settings.debug);
}

#[rstest]
fn settings_production_profile_should_disable_debug() {
	// Arrange
	let mut settings = Settings::default();
	let profile = Profile::Production;

	// Act – apply the profile's debug default
	settings.debug = profile.default_debug();

	// Assert
	assert!(!settings.debug);
}

#[rstest]
fn settings_staging_profile_keeps_debug_enabled() {
	// Arrange
	let mut settings = Settings::default();
	let profile = Profile::Staging;

	// Act
	settings.debug = profile.default_debug();

	// Assert
	assert!(settings.debug);
}

// ---------------------------------------------------------------------------
// allowed_hosts differs by profile
// ---------------------------------------------------------------------------

#[rstest]
fn development_settings_allows_localhost() {
	// Arrange
	let mut settings = Settings::new(PathBuf::from("."), "dev-secret".to_string());

	// Act – typical development setup
	settings.allowed_hosts = vec!["localhost".to_string(), "127.0.0.1".to_string()];

	// Assert
	assert!(settings.allowed_hosts.contains(&"localhost".to_string()));
	assert!(settings.allowed_hosts.contains(&"127.0.0.1".to_string()));
}

#[rstest]
fn production_settings_disallows_debug_and_restricts_hosts() {
	// Arrange
	let mut settings = Settings::new(PathBuf::from("/app"), "prod-secret-key".to_string());

	// Act – typical production setup
	settings.debug = false;
	settings.allowed_hosts = vec!["example.com".to_string(), "www.example.com".to_string()];

	// Assert
	assert!(!settings.debug);
	assert!(!settings.allowed_hosts.contains(&"localhost".to_string()));
	assert!(settings.allowed_hosts.contains(&"example.com".to_string()));
}

#[rstest]
fn staging_settings_allows_staging_domain() {
	// Arrange
	let mut settings = Settings::new(PathBuf::from("/app"), "staging-secret".to_string());

	// Act
	settings.allowed_hosts = vec!["staging.example.com".to_string()];
	settings.debug = Profile::Staging.default_debug();

	// Assert
	assert!(settings.debug);
	assert_eq!(settings.allowed_hosts.len(), 1);
	assert_eq!(settings.allowed_hosts[0], "staging.example.com");
}

// ---------------------------------------------------------------------------
// Database configuration differs by profile
// ---------------------------------------------------------------------------

#[rstest]
fn development_uses_sqlite_database() {
	// Arrange
	let db = DatabaseConfig::sqlite("dev.sqlite3");

	// Act
	let url = db.to_url();

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.sqlite3");
	assert!(url.starts_with("sqlite:"));
	assert!(db.user.is_none());
	assert!(db.password.is_none());
}

#[rstest]
fn production_uses_postgresql_database() {
	// Arrange
	let db = DatabaseConfig::postgresql(
		"proddb",
		"produser",
		"prodpass",
		"db.prod.example.com",
		5432,
	);

	// Act
	let url = db.to_url();

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
	assert_eq!(db.name, "proddb");
	assert_eq!(db.host, Some("db.prod.example.com".to_string()));
	assert_eq!(db.port, Some(5432));
	assert!(url.starts_with("postgresql://"));
}

#[rstest]
fn staging_uses_postgresql_database_with_staging_host() {
	// Arrange
	let db = DatabaseConfig::postgresql(
		"stagingdb",
		"staginguser",
		"stagingpass",
		"db.staging.example.com",
		5432,
	);

	// Act
	let url = db.to_url();

	// Assert
	assert_eq!(db.engine, "reinhardt.db.backends.postgresql");
	assert!(url.contains("db.staging.example.com"));
	assert!(url.contains("stagingdb"));
}

// ---------------------------------------------------------------------------
// SettingsBuilder profile integration
// ---------------------------------------------------------------------------

#[rstest]
fn builder_with_development_profile() {
	// Act
	let merged = SettingsBuilder::new()
		.profile(Profile::Development)
		.add_source(
			DefaultSource::new()
				.with_value("debug", Value::Bool(true))
				.with_value("allowed_hosts", Value::Array(vec![])),
		)
		.build()
		.unwrap();

	// Assert
	assert_eq!(merged.profile(), Some(Profile::Development));
	let debug: bool = merged.get("debug").unwrap();
	assert!(debug);
}

#[rstest]
fn builder_with_production_profile() {
	// Act
	let merged = SettingsBuilder::new()
		.profile(Profile::Production)
		.add_source(
			DefaultSource::new()
				.with_value("debug", Value::Bool(false))
				.with_value(
					"allowed_hosts",
					Value::Array(vec![Value::String("example.com".to_string())]),
				),
		)
		.build()
		.unwrap();

	// Assert
	assert_eq!(merged.profile(), Some(Profile::Production));
	let debug: bool = merged.get("debug").unwrap();
	assert!(!debug);
}

#[rstest]
fn builder_with_staging_profile() {
	// Act
	let merged = SettingsBuilder::new()
		.profile(Profile::Staging)
		.add_source(
			DefaultSource::new()
				.with_value("debug", Value::Bool(true))
				.with_value(
					"allowed_hosts",
					Value::Array(vec![Value::String("staging.example.com".to_string())]),
				),
		)
		.build()
		.unwrap();

	// Assert
	assert_eq!(merged.profile(), Some(Profile::Staging));
	assert!(!merged.profile().unwrap().is_production());
}

#[rstest]
fn builder_without_profile_has_no_profile() {
	// Act
	let merged = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("key", Value::String("value".to_string())))
		.build()
		.unwrap();

	// Assert
	assert_eq!(merged.profile(), None);
}

#[rstest]
fn profile_switching_production_forbids_debug() {
	// Arrange
	let profile = Profile::Production;

	// Act
	let should_debug = profile.default_debug();

	// Assert
	assert!(
		!should_debug,
		"Production profile must not enable debug mode"
	);
}

#[rstest]
fn all_non_production_profiles_allow_debug() {
	// Arrange
	let non_production = [Profile::Development, Profile::Staging, Profile::Custom];

	// Assert
	for profile in &non_production {
		assert!(
			profile.default_debug(),
			"{} profile should allow debug by default",
			profile.as_str()
		);
	}
}
