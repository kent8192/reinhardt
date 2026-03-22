use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::database_config::DatabaseConfig;
use reinhardt_conf::settings::security::SecuritySettings;
use reinhardt_conf::settings::{CacheSettings, EmailSettings, SessionSettings};
use rstest::rstest;
use std::collections::HashMap;

// ===========================================================================
// Clone independence
// ===========================================================================

#[rstest]
fn fragment_clone_independence() {
	// Arrange
	let original = CoreSettings {
		secret_key: "original-key".to_string(),
		debug: false,
		..Default::default()
	};

	// Act
	let mut cloned = original.clone();
	cloned.debug = true;

	// Assert
	assert!(!original.debug);
	assert!(cloned.debug);
	assert_eq!(original.secret_key, "original-key");
	assert_eq!(cloned.secret_key, "original-key");
}

// ===========================================================================
// CoreSettings with empty databases
// ===========================================================================

#[rstest]
fn core_settings_with_empty_databases() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "test-key".to_string(),
		databases: HashMap::new(),
		..Default::default()
	};

	// Act
	let json = serde_json::to_string(&settings).expect("serialization should succeed");
	let restored: CoreSettings =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	assert!(restored.databases.is_empty());
	assert_eq!(restored.secret_key, "test-key");
}

// ===========================================================================
// CoreSettings with multiple databases
// ===========================================================================

#[rstest]
fn core_settings_with_multiple_databases() {
	// Arrange
	let mut databases = HashMap::new();
	databases.insert(
		"default".to_string(),
		DatabaseConfig::new("reinhardt.db.backends.sqlite3", "default.db"),
	);
	databases.insert(
		"analytics".to_string(),
		DatabaseConfig::new("reinhardt.db.backends.postgresql", "analytics"),
	);
	databases.insert(
		"cache_db".to_string(),
		DatabaseConfig::new("reinhardt.db.backends.mysql", "cache"),
	);

	let settings = CoreSettings {
		secret_key: "multi-db-key".to_string(),
		databases: databases.clone(),
		..Default::default()
	};

	// Act
	let cloned = settings.clone();

	// Assert
	assert_eq!(cloned.databases.len(), 3);
	assert!(cloned.databases.contains_key("default"));
	assert!(cloned.databases.contains_key("analytics"));
	assert!(cloned.databases.contains_key("cache_db"));
	assert_eq!(
		cloned.databases["analytics"].engine,
		"reinhardt.db.backends.postgresql"
	);
	assert_eq!(
		cloned.databases["cache_db"].engine,
		"reinhardt.db.backends.mysql"
	);
}

// ===========================================================================
// SecuritySettings with all optional fields None
// ===========================================================================

#[rstest]
fn security_all_optional_fields_none() {
	// Arrange
	let security = SecuritySettings {
		secure_proxy_ssl_header: None,
		secure_hsts_seconds: None,
		..Default::default()
	};

	// Act
	let json = serde_json::to_string(&security).expect("serialization should succeed");
	let restored: SecuritySettings =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	assert!(restored.secure_proxy_ssl_header.is_none());
	assert!(restored.secure_hsts_seconds.is_none());
	assert!(!restored.secure_ssl_redirect);
	assert!(!restored.secure_hsts_include_subdomains);
}

// ===========================================================================
// SecuritySettings proxy SSL header roundtrip
// ===========================================================================

#[rstest]
fn security_proxy_ssl_header_roundtrip() {
	// Arrange
	let security = SecuritySettings {
		secure_proxy_ssl_header: Some(("X-Forwarded-Proto".to_string(), "https".to_string())),
		..Default::default()
	};

	// Act
	let json = serde_json::to_string(&security).expect("serialization should succeed");
	let restored: SecuritySettings =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	let header = restored
		.secure_proxy_ssl_header
		.expect("header should be present after roundtrip");
	assert_eq!(header.0, "X-Forwarded-Proto");
	assert_eq!(header.1, "https");
}

// ===========================================================================
// CacheSettings with timeout zero
// ===========================================================================

#[rstest]
fn cache_timeout_zero_is_valid() {
	// Arrange
	// CacheSettings is #[non_exhaustive]; use serde_json to construct it with custom timeout.
	let json_input = r#"{"backend":"memory","timeout":0}"#;
	let cache: CacheSettings =
		serde_json::from_str(json_input).expect("construction from JSON should succeed");

	// Act
	let json = serde_json::to_string(&cache).expect("serialization should succeed");
	let restored: CacheSettings =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	assert_eq!(restored.timeout, 0);
	assert_eq!(restored.backend, "memory");
}

// ===========================================================================
// SessionSettings with empty cookie name
// ===========================================================================

#[rstest]
fn session_empty_cookie_name_is_valid() {
	// Arrange
	// SessionSettings is #[non_exhaustive]; use serde_json to construct with empty cookie_name.
	let json_input = r#"{"engine":"cookie","cookie_name":"","cookie_age":1209600,"cookie_secure":false,"cookie_httponly":true,"cookie_samesite":"lax"}"#;
	let session: SessionSettings =
		serde_json::from_str(json_input).expect("construction from JSON should succeed");

	// Act
	let json = serde_json::to_string(&session).expect("serialization should succeed");
	let restored: SessionSettings =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	assert_eq!(restored.cookie_name, "");
	assert_eq!(restored.engine, "cookie");
}

// ===========================================================================
// EmailSettings default serde roundtrip
// ===========================================================================

#[rstest]
fn email_settings_defaults_serde_roundtrip() {
	// Arrange
	let email = EmailSettings::default();
	let expected_backend = email.backend.clone();
	let expected_host = email.host.clone();

	// Act
	let json = serde_json::to_string(&email).expect("serialization should succeed");
	let restored: EmailSettings =
		serde_json::from_str(&json).expect("deserialization should succeed");

	// Assert
	assert_eq!(restored.backend, expected_backend);
	assert_eq!(restored.host, expected_host);
	assert_eq!(restored.port, 25);
	assert_eq!(restored.from_email, "noreply@example.com");
	assert!(!restored.use_tls);
	assert!(!restored.use_ssl);
}
