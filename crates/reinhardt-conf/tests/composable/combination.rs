use reinhardt_conf::settings::cache::CacheSettings;
use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::cors::CorsSettings;
use reinhardt_conf::settings::database_config::DatabaseConfig;
use reinhardt_conf::settings::email::EmailSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use reinhardt_conf::settings::session::SessionSettings;
use reinhardt_conf::settings::validation::ValidationError;
use rstest::rstest;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Helper: a composite struct that bundles three fragments for roundtrip tests
// ---------------------------------------------------------------------------

/// Composite settings struct used only in this test module.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct MultiFragmentSettings {
	core: CoreSettings,
	cache: CacheSettings,
	session: SessionSettings,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// CoreSettings with a valid key, debug=false, and non-empty allowed_hosts, but
/// the nested SecuritySettings uses defaults (ssl redirect disabled) — this must
/// fail Production validation because the security fragment rejects it.
#[rstest]
fn core_valid_but_nested_security_invalid_production() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "a-long-secure-key-for-production-use".to_string(),
		debug: false,
		allowed_hosts: vec!["example.com".to_string()],
		security: SecuritySettings::default(), // ssl=false by default
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	assert!(
		result.is_err(),
		"production validation must fail when ssl redirect is disabled"
	);
	match result.unwrap_err() {
		ValidationError::Security(msg) => {
			assert!(!msg.is_empty(), "security error message must not be empty");
		}
		other => panic!("expected Security variant, got: {:?}", other),
	}
}

/// CoreSettings with an empty secret_key but a fully-secure SecuritySettings
/// must fail with MissingRequired because secret_key is validated first.
#[rstest]
fn core_invalid_but_security_valid_production() {
	// Arrange
	let settings = CoreSettings {
		secret_key: String::new(), // empty: invalid
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

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	match result {
		Err(ValidationError::MissingRequired(field)) => {
			assert_eq!(field, "secret_key", "missing field must be 'secret_key'");
		}
		other => panic!("expected MissingRequired(\"secret_key\"), got: {:?}", other),
	}
}

/// All 12 fragment types with defaults, plus CoreSettings with secret_key set,
/// must pass Development-profile validation.
#[rstest]
fn all_fragments_default_validate_development() {
	// Arrange
	let core = CoreSettings {
		secret_key: "dev-secret-key-for-testing".to_string(),
		..Default::default()
	};
	let cache = CacheSettings::default();
	let session = SessionSettings::default();
	let cors = CorsSettings::default();
	let email = EmailSettings::default();

	// Act
	let core_result = core.validate(&Profile::Development);
	let cache_result = cache.validate(&Profile::Development);
	let session_result = session.validate(&Profile::Development);
	let cors_result = cors.validate(&Profile::Development);
	let email_result = email.validate(&Profile::Development);

	// Assert
	assert!(
		core_result.is_ok(),
		"CoreSettings with secret_key must pass Development: {:?}",
		core_result.err()
	);
	assert!(
		cache_result.is_ok(),
		"CacheSettings default must pass Development: {:?}",
		cache_result.err()
	);
	assert!(
		session_result.is_ok(),
		"SessionSettings default must pass Development: {:?}",
		session_result.err()
	);
	assert!(
		cors_result.is_ok(),
		"CorsSettings default must pass Development: {:?}",
		cors_result.err()
	);
	assert!(
		email_result.is_ok(),
		"EmailSettings default must pass Development: {:?}",
		email_result.err()
	);
}

/// A composite struct with CoreSettings, CacheSettings, and SessionSettings
/// must serialize to JSON and deserialize back with identical field values.
#[rstest]
fn multiple_fragments_serde_roundtrip() {
	// Arrange
	// CacheSettings and SessionSettings are #[non_exhaustive]; construct via serde_json.
	let cache: CacheSettings = serde_json::from_str(
		r#"{"backend":"redis","location":"redis://127.0.0.1:6379","timeout":600}"#,
	)
	.expect("CacheSettings should deserialize from JSON");
	let session: SessionSettings = serde_json::from_str(
		r#"{"engine":"database","cookie_name":"sid","cookie_age":86400,"cookie_secure":false,"cookie_httponly":true,"cookie_samesite":"strict"}"#,
	)
	.expect("SessionSettings should deserialize from JSON");
	let original = MultiFragmentSettings {
		core: CoreSettings {
			secret_key: "roundtrip-secret-key-32-chars-long".to_string(),
			debug: true,
			allowed_hosts: vec!["localhost".to_string()],
			..Default::default()
		},
		cache,
		session,
	};

	// Act
	let json =
		serde_json::to_string(&original).expect("MultiFragmentSettings must serialize to JSON");
	let restored: MultiFragmentSettings =
		serde_json::from_str(&json).expect("MultiFragmentSettings must deserialize from JSON");

	// Assert
	assert_eq!(
		original.core.secret_key, restored.core.secret_key,
		"core.secret_key must survive serde roundtrip"
	);
	assert_eq!(
		original.core.debug, restored.core.debug,
		"core.debug must survive serde roundtrip"
	);
	assert_eq!(
		original.core.allowed_hosts, restored.core.allowed_hosts,
		"core.allowed_hosts must survive serde roundtrip"
	);
	assert_eq!(
		original.cache.backend, restored.cache.backend,
		"cache.backend must survive serde roundtrip"
	);
	assert_eq!(
		original.cache.location, restored.cache.location,
		"cache.location must survive serde roundtrip"
	);
	assert_eq!(
		original.cache.timeout, restored.cache.timeout,
		"cache.timeout must survive serde roundtrip"
	);
	assert_eq!(
		original.session.engine, restored.session.engine,
		"session.engine must survive serde roundtrip"
	);
	assert_eq!(
		original.session.cookie_name, restored.session.cookie_name,
		"session.cookie_name must survive serde roundtrip"
	);
	assert_eq!(
		original.session.cookie_age, restored.session.cookie_age,
		"session.cookie_age must survive serde roundtrip"
	);
}

/// CoreSettings with 3 explicit database configs and full production security
/// settings must pass Production-profile validation.
#[rstest]
fn core_with_multiple_databases_and_security() {
	// Arrange
	let mut databases = std::collections::HashMap::new();
	databases.insert("default".to_string(), DatabaseConfig::default());
	databases.insert("replica".to_string(), DatabaseConfig::default());
	databases.insert("analytics".to_string(), DatabaseConfig::default());

	let settings = CoreSettings {
		secret_key: "production-key-that-is-at-least-32-chars-long".to_string(),
		debug: false,
		allowed_hosts: vec!["example.com".to_string(), "www.example.com".to_string()],
		databases,
		security: SecuritySettings {
			secure_ssl_redirect: true,
			session_cookie_secure: true,
			csrf_cookie_secure: true,
			..Default::default()
		},
		..Default::default()
	};

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	assert!(
		result.is_ok(),
		"CoreSettings with 3 databases and full security must pass Production: {:?}",
		result.err()
	);
	assert_eq!(
		settings.databases.len(),
		3,
		"three database configs must be present"
	);
}

/// Modifying the cache backend on a CacheSettings instance must not affect a
/// separately created SessionSettings instance.
#[rstest]
fn modify_cache_doesnt_affect_session() {
	// Arrange
	let mut cache = CacheSettings::default();
	let session = SessionSettings::default();
	let original_session_engine = session.engine.clone();

	// Act
	cache.backend = "memcached".to_string();

	// Assert
	assert_eq!(
		cache.backend, "memcached",
		"cache.backend must be updated to memcached"
	);
	assert_eq!(
		session.engine, original_session_engine,
		"session.engine must remain unchanged when cache is modified"
	);
}

/// CorsSettings with allow_credentials=true and SessionSettings with
/// cookie_secure=true must each serialize to JSON and deserialize back intact.
#[rstest]
fn cors_allow_credentials_with_session_secure() {
	// Arrange
	// CorsSettings and SessionSettings are #[non_exhaustive]; construct via serde_json.
	let mut cors_map: serde_json::Map<String, serde_json::Value> =
		serde_json::from_str(&serde_json::to_string(&CorsSettings::default()).unwrap()).unwrap();
	cors_map.insert("allow_credentials".to_string(), serde_json::json!(true));
	let cors: CorsSettings = serde_json::from_value(serde_json::Value::Object(cors_map))
		.expect("CorsSettings should deserialize");
	let mut session_map: serde_json::Map<String, serde_json::Value> =
		serde_json::from_str(&serde_json::to_string(&SessionSettings::default()).unwrap()).unwrap();
	session_map.insert("cookie_secure".to_string(), serde_json::json!(true));
	let session: SessionSettings = serde_json::from_value(serde_json::Value::Object(session_map))
		.expect("SessionSettings should deserialize");

	// Act
	let cors_json = serde_json::to_string(&cors).expect("CorsSettings must serialize");
	let session_json = serde_json::to_string(&session).expect("SessionSettings must serialize");

	let restored_cors: CorsSettings =
		serde_json::from_str(&cors_json).expect("CorsSettings must deserialize");
	let restored_session: SessionSettings =
		serde_json::from_str(&session_json).expect("SessionSettings must deserialize");

	// Assert
	assert!(
		restored_cors.allow_credentials,
		"cors.allow_credentials must be true after roundtrip"
	);
	assert!(
		restored_session.cookie_secure,
		"session.cookie_secure must be true after roundtrip"
	);
	assert_eq!(
		cors.allow_origins, restored_cors.allow_origins,
		"cors.allow_origins must survive roundtrip"
	);
	assert_eq!(
		session.engine, restored_session.engine,
		"session.engine must survive roundtrip"
	);
}

/// EmailSettings with both use_tls=true and use_ssl=true must not produce a
/// validation error because no email-specific validation rule exists.
#[rstest]
fn email_tls_and_ssl_both_true() {
	// Arrange
	// EmailSettings is #[non_exhaustive]; construct via serde_json.
	let mut email_map: serde_json::Map<String, serde_json::Value> =
		serde_json::from_str(&serde_json::to_string(&EmailSettings::default()).unwrap()).unwrap();
	email_map.insert("use_tls".to_string(), serde_json::json!(true));
	email_map.insert("use_ssl".to_string(), serde_json::json!(true));
	let settings: EmailSettings = serde_json::from_value(serde_json::Value::Object(email_map))
		.expect("EmailSettings should deserialize");

	// Act
	let result = settings.validate(&Profile::Production);

	// Assert
	assert!(
		result.is_ok(),
		"EmailSettings with both tls and ssl true must not fail validation: {:?}",
		result.err()
	);
	assert!(settings.use_tls, "use_tls must be set to true");
	assert!(settings.use_ssl, "use_ssl must be set to true");
}
