#[allow(deprecated)]
use reinhardt_conf::Settings;
use reinhardt_conf::settings::cache::{CacheSettings, HasCacheSettings};
use reinhardt_conf::settings::core_settings::{CoreSettings, HasCoreSettings};
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use rstest::rstest;

#[rstest]
#[allow(deprecated)]
fn use_case_legacy_settings_has_core_settings() {
	// Arrange
	let settings = Settings::default();

	// Act
	let core = settings.core();

	// Assert
	assert!(core.debug, "Default Settings should have debug=true");
	assert!(
		!core.secret_key.is_empty(),
		"Default Settings should have a non-empty secret_key"
	);
	assert_eq!(
		core.secret_key, "insecure-change-this-in-production",
		"Default Settings secret_key should be the placeholder value"
	);
	assert_eq!(
		CoreSettings::section(),
		"core",
		"CoreSettings section should be 'core'"
	);
}

#[rstest]
#[allow(deprecated)]
fn use_case_settings_add_app_through_core() {
	// Arrange
	let mut settings = Settings::default();
	let initial_count = settings.core.installed_apps.len();

	// Act
	settings.add_app("myapp");

	// Assert
	assert_eq!(
		settings.core.installed_apps.len(),
		initial_count + 1,
		"add_app should add one entry"
	);
	assert!(
		settings.core.installed_apps.contains(&"myapp".to_string()),
		"installed_apps should contain 'myapp'"
	);
}

#[rstest]
fn use_case_fragment_accessed_via_has_trait() {
	// Arrange
	struct MySettings {
		core: CoreSettings,
		cache: CacheSettings,
	}

	impl HasCoreSettings for MySettings {
		fn core(&self) -> &CoreSettings {
			&self.core
		}
	}

	impl HasCacheSettings for MySettings {
		fn cache(&self) -> &CacheSettings {
			&self.cache
		}
	}

	fn get_debug(s: &impl HasCoreSettings) -> bool {
		s.core().debug
	}

	fn get_cache_backend(s: &impl HasCacheSettings) -> &str {
		&s.cache().backend
	}

	let settings = MySettings {
		core: CoreSettings {
			secret_key: "test-key".to_string(),
			..Default::default()
		},
		cache: CacheSettings::default(),
	};

	// Act
	let debug = get_debug(&settings);
	let backend = get_cache_backend(&settings);

	// Assert
	assert!(debug, "Default core debug should be true");
	assert_eq!(
		backend, "memory",
		"Default cache backend should be 'memory'"
	);
}

#[rstest]
fn use_case_core_and_security_nested_validation_production() {
	// Arrange
	let valid_settings = CoreSettings {
		secret_key: "a-very-long-secure-random-key-that-is-at-least-32-chars".to_string(),
		debug: false,
		allowed_hosts: vec!["example.com".to_string()],
		security: SecuritySettings {
			secure_ssl_redirect: true,
			session_cookie_secure: true,
			csrf_cookie_secure: true,
			..Default::default()
		},
		..Default::default()
	};

	let invalid_settings = CoreSettings {
		secret_key: "a-very-long-secure-random-key-that-is-at-least-32-chars".to_string(),
		debug: false,
		allowed_hosts: vec!["example.com".to_string()],
		security: SecuritySettings::default(), // ssl_redirect = false
		..Default::default()
	};

	// Act
	let valid_result = valid_settings.validate(&Profile::Production);
	let invalid_result = invalid_settings.validate(&Profile::Production);

	// Assert
	assert!(
		valid_result.is_ok(),
		"Full production settings should validate: {valid_result:?}"
	);
	assert!(
		invalid_result.is_err(),
		"Settings with insecure SecuritySettings should fail production validation"
	);
}

#[rstest]
#[allow(deprecated)]
fn use_case_settings_serde_roundtrip() {
	// Arrange
	let original = Settings::default();

	// Act
	let json_str = serde_json::to_string(&original).unwrap();
	let deserialized: Settings = serde_json::from_str(&json_str).unwrap();

	// Assert
	assert_eq!(
		deserialized.core.secret_key, original.core.secret_key,
		"Serde roundtrip should preserve secret_key"
	);
	assert_eq!(
		deserialized.core.debug, original.core.debug,
		"Serde roundtrip should preserve debug flag"
	);
	assert_eq!(
		deserialized.language_code, original.language_code,
		"Serde roundtrip should preserve language_code"
	);
	assert_eq!(
		deserialized.time_zone, original.time_zone,
		"Serde roundtrip should preserve time_zone"
	);
}

#[rstest]
#[allow(deprecated)]
fn use_case_all_has_traits_implemented_for_settings() {
	// Arrange
	let mut settings = Settings::default();
	settings.core.secret_key = "test-key".to_string();

	// Act
	let core: &CoreSettings = settings.core();

	// Assert
	assert_eq!(
		core.secret_key, "test-key",
		"HasCoreSettings should provide access to CoreSettings"
	);
	assert!(
		core.debug,
		"Default debug should be true through HasCoreSettings"
	);
}

#[rstest]
#[allow(deprecated)]
fn use_case_nested_security_keys_deserialize() {
	// Arrange — start from default Settings, serialize, then inject nested security keys.
	// Since PR #3176 removed #[serde(flatten)] from CoreSettings.security,
	// security fields must be provided as a nested "security" object.
	let mut default_json: serde_json::Value = serde_json::to_value(Settings::default()).unwrap();
	let obj = default_json.as_object_mut().unwrap();
	obj.insert(
		"security".to_string(),
		serde_json::json!({
			"secure_ssl_redirect": true,
			"session_cookie_secure": true,
			"csrf_cookie_secure": true,
			"append_slash": false
		}),
	);
	let json = default_json;

	// Act
	let settings: Settings = serde_json::from_value(json).unwrap();

	// Assert — security fields should be accessible via core.security
	assert!(
		settings.core.security.secure_ssl_redirect,
		"Nested secure_ssl_redirect should deserialize into core.security"
	);
	assert!(
		settings.core.security.session_cookie_secure,
		"Nested session_cookie_secure should deserialize into core.security"
	);
	assert!(
		settings.core.security.csrf_cookie_secure,
		"Nested csrf_cookie_secure should deserialize into core.security"
	);
	assert!(
		!settings.core.security.append_slash,
		"Nested append_slash should deserialize into core.security"
	);
}

#[rstest]
#[allow(deprecated)]
fn use_case_settings_builder_nested_security_into_typed() {
	// Arrange — simulate DefaultSource with nested security object.
	// Since PR #3176 removed #[serde(flatten)] from CoreSettings.security,
	// security fields must be provided as a nested "security" object.
	use reinhardt_conf::settings::builder::SettingsBuilder;
	use reinhardt_conf::settings::profile::Profile;
	use reinhardt_conf::settings::sources::DefaultSource;

	let merged = SettingsBuilder::new()
		.profile(Profile::Development)
		.add_source(
			DefaultSource::new()
				.with_value("secret_key", serde_json::json!("test-secret-key"))
				.with_value("debug", serde_json::json!(true))
				.with_value("allowed_hosts", serde_json::json!([]))
				.with_value("installed_apps", serde_json::json!([]))
				.with_value("databases", serde_json::json!({}))
				.with_value("templates", serde_json::json!([]))
				.with_value("static_url", serde_json::json!("/static/"))
				.with_value("staticfiles_dirs", serde_json::json!([]))
				.with_value("media_url", serde_json::json!("/media/"))
				.with_value("language_code", serde_json::json!("en-us"))
				.with_value("time_zone", serde_json::json!("UTC"))
				.with_value("use_i18n", serde_json::json!(false))
				.with_value("use_tz", serde_json::json!(false))
				.with_value("default_auto_field", serde_json::json!("BigAutoField"))
				// Nested security object (required since PR #3176)
				.with_value("security", serde_json::json!({
					"secure_ssl_redirect": true,
					"session_cookie_secure": true,
					"csrf_cookie_secure": true,
					"append_slash": false
				}))
				.with_value("middleware", serde_json::json!([]))
				.with_value("root_urlconf", serde_json::json!(""))
				.with_value("admins", serde_json::json!([]))
				.with_value("managers", serde_json::json!([])),
		)
		.build()
		.expect("SettingsBuilder should build with nested security keys");

	// Act — into_typed should correctly map nested security object via CoreSettings
	let settings: Settings = merged
		.into_typed()
		.expect("into_typed should succeed with nested security keys");

	// Assert — security fields should be accessible via core.security
	assert_eq!(
		settings.core.secret_key, "test-secret-key",
		"secret_key should be in core via flatten"
	);
	assert!(settings.core.debug, "debug should be in core via flatten");
	assert!(
		settings.core.security.secure_ssl_redirect,
		"Nested secure_ssl_redirect should map to core.security"
	);
	assert!(
		settings.core.security.session_cookie_secure,
		"Nested session_cookie_secure should map to core.security"
	);
	assert!(
		settings.core.security.csrf_cookie_secure,
		"Nested csrf_cookie_secure should map to core.security"
	);
	assert!(
		!settings.core.security.append_slash,
		"Nested append_slash should map to core.security"
	);
}
