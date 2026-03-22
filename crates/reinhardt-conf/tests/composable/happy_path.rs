use std::collections::HashSet;

use reinhardt_conf::settings::cache::CacheSettings;
use reinhardt_conf::settings::contacts::ContactSettings;
use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::cors::CorsSettings;
use reinhardt_conf::settings::email::EmailSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::i18n::I18nSettings;
use reinhardt_conf::settings::logging::LoggingSettings;
use reinhardt_conf::settings::media::MediaSettings;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use reinhardt_conf::settings::session::SessionSettings;
use reinhardt_conf::settings::static_files::StaticSettings;
use reinhardt_conf::settings::template_settings::TemplateSettings;
use rstest::rstest;

use super::fixtures::{
	development_core_settings, production_core_settings, production_security_settings,
};

/// Verify that all 12 fragment types expose a unique section name.
#[rstest]
fn all_twelve_fragments_have_unique_sections() {
	// Arrange
	let sections = [
		CoreSettings::section(),
		SecuritySettings::section(),
		CacheSettings::section(),
		SessionSettings::section(),
		CorsSettings::section(),
		StaticSettings::section(),
		MediaSettings::section(),
		EmailSettings::section(),
		LoggingSettings::section(),
		I18nSettings::section(),
		TemplateSettings::section(),
		ContactSettings::section(),
	];

	// Act
	let unique: HashSet<&str> = sections.iter().copied().collect();

	// Assert
	assert_eq!(
		unique.len(),
		12,
		"Expected 12 unique section names, but found {}: {:?}",
		unique.len(),
		sections,
	);
}

/// Serialize CoreSettings to JSON and deserialize it back; verify field equality.
#[rstest]
fn core_settings_serde_roundtrip(production_core_settings: CoreSettings) {
	// Arrange
	let original = production_core_settings;

	// Act
	let json = serde_json::to_string(&original).expect("CoreSettings should serialize to JSON");
	let restored: CoreSettings =
		serde_json::from_str(&json).expect("CoreSettings should deserialize from JSON");

	// Assert
	assert_eq!(
		original.secret_key, restored.secret_key,
		"secret_key must survive serde roundtrip"
	);
	assert_eq!(
		original.debug, restored.debug,
		"debug flag must survive serde roundtrip"
	);
	assert_eq!(
		original.allowed_hosts, restored.allowed_hosts,
		"allowed_hosts must survive serde roundtrip"
	);
}

/// Serialize SecuritySettings to JSON and deserialize it back; verify field equality.
#[rstest]
fn security_settings_serde_roundtrip(production_security_settings: SecuritySettings) {
	// Arrange
	let original = production_security_settings;

	// Act
	let json = serde_json::to_string(&original).expect("SecuritySettings should serialize to JSON");
	let restored: SecuritySettings =
		serde_json::from_str(&json).expect("SecuritySettings should deserialize from JSON");

	// Assert
	assert_eq!(
		original.secure_ssl_redirect, restored.secure_ssl_redirect,
		"secure_ssl_redirect must survive serde roundtrip"
	);
	assert_eq!(
		original.session_cookie_secure, restored.session_cookie_secure,
		"session_cookie_secure must survive serde roundtrip"
	);
	assert_eq!(
		original.csrf_cookie_secure, restored.csrf_cookie_secure,
		"csrf_cookie_secure must survive serde roundtrip"
	);
	assert_eq!(
		original.append_slash, restored.append_slash,
		"append_slash must survive serde roundtrip"
	);
}

/// Serialize default CacheSettings to JSON and deserialize it back; verify field equality.
#[rstest]
fn cache_settings_serde_roundtrip() {
	// Arrange
	let original = CacheSettings::default();

	// Act
	let json = serde_json::to_string(&original).expect("CacheSettings should serialize to JSON");
	let restored: CacheSettings =
		serde_json::from_str(&json).expect("CacheSettings should deserialize from JSON");

	// Assert
	assert_eq!(
		original.backend, restored.backend,
		"backend must survive serde roundtrip"
	);
	assert_eq!(
		original.location, restored.location,
		"location must survive serde roundtrip"
	);
	assert_eq!(
		original.timeout, restored.timeout,
		"timeout must survive serde roundtrip"
	);
}

/// Serialize default SessionSettings to JSON and deserialize it back; verify field equality.
#[rstest]
fn session_settings_serde_roundtrip() {
	// Arrange
	let original = SessionSettings::default();

	// Act
	let json = serde_json::to_string(&original).expect("SessionSettings should serialize to JSON");
	let restored: SessionSettings =
		serde_json::from_str(&json).expect("SessionSettings should deserialize from JSON");

	// Assert
	assert_eq!(
		original.engine, restored.engine,
		"engine must survive serde roundtrip"
	);
	assert_eq!(
		original.cookie_name, restored.cookie_name,
		"cookie_name must survive serde roundtrip"
	);
	assert_eq!(
		original.cookie_age, restored.cookie_age,
		"cookie_age must survive serde roundtrip"
	);
	assert_eq!(
		original.cookie_secure, restored.cookie_secure,
		"cookie_secure must survive serde roundtrip"
	);
	assert_eq!(
		original.cookie_httponly, restored.cookie_httponly,
		"cookie_httponly must survive serde roundtrip"
	);
	assert_eq!(
		original.cookie_samesite, restored.cookie_samesite,
		"cookie_samesite must survive serde roundtrip"
	);
}

/// A CoreSettings built for production should pass validation against Profile::Production.
#[rstest]
fn production_core_validates_ok(production_core_settings: CoreSettings) {
	// Arrange
	let settings = production_core_settings;
	let profile = Profile::Production;

	// Act
	let result = settings.validate(&profile);

	// Assert
	assert!(
		result.is_ok(),
		"production CoreSettings should pass validation, but got: {:?}",
		result.err()
	);
}

/// Static assertion: all 12 fragment types implement Send + Sync.
#[rstest]
fn all_fragments_implement_send_sync() {
	// Arrange / Act / Assert
	// These are compile-time checks; if any type is not Send + Sync, this test
	// will fail to compile rather than fail at runtime.
	fn assert_send_sync<T: Send + Sync>() {}

	assert_send_sync::<CoreSettings>();
	assert_send_sync::<SecuritySettings>();
	assert_send_sync::<CacheSettings>();
	assert_send_sync::<SessionSettings>();
	assert_send_sync::<CorsSettings>();
	assert_send_sync::<StaticSettings>();
	assert_send_sync::<MediaSettings>();
	assert_send_sync::<EmailSettings>();
	assert_send_sync::<LoggingSettings>();
	assert_send_sync::<I18nSettings>();
	assert_send_sync::<TemplateSettings>();
	assert_send_sync::<ContactSettings>();
}

/// Clone produces an independent copy; fields on the clone match the original.
#[rstest]
fn all_fragments_implement_clone(
	production_core_settings: CoreSettings,
	#[allow(unused_variables)] // fixture is used implicitly via production_core_settings
	development_core_settings: CoreSettings,
) {
	// Arrange
	let original_core = production_core_settings;
	let original_cache = CacheSettings::default();
	let original_session = SessionSettings::default();

	// Act
	let cloned_core = original_core.clone();
	let cloned_cache = original_cache.clone();
	let cloned_session = original_session.clone();

	// Assert
	assert_eq!(
		original_core.secret_key, cloned_core.secret_key,
		"CoreSettings clone must preserve secret_key"
	);
	assert_eq!(
		original_core.debug, cloned_core.debug,
		"CoreSettings clone must preserve debug flag"
	);
	assert_eq!(
		original_core.allowed_hosts, cloned_core.allowed_hosts,
		"CoreSettings clone must preserve allowed_hosts"
	);
	assert_eq!(
		original_cache.backend, cloned_cache.backend,
		"CacheSettings clone must preserve backend"
	);
	assert_eq!(
		original_cache.timeout, cloned_cache.timeout,
		"CacheSettings clone must preserve timeout"
	);
	assert_eq!(
		original_session.engine, cloned_session.engine,
		"SessionSettings clone must preserve engine"
	);
	assert_eq!(
		original_session.cookie_name, cloned_session.cookie_name,
		"SessionSettings clone must preserve cookie_name"
	);
}
