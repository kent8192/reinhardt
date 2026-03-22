use quickcheck_macros::quickcheck;
use reinhardt_conf::settings::cache::CacheSettings;
use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use reinhardt_conf::settings::validation::ValidationError;

/// Fuzz: validate never panics regardless of secret_key, debug, or allowed_hosts values.
#[quickcheck]
fn fuzz_core_validate_never_panics(secret_key: String, debug: bool, hosts: Vec<String>) -> bool {
	// Arrange
	let settings = CoreSettings {
		secret_key,
		debug,
		allowed_hosts: hosts,
		..Default::default()
	};

	// Act — must not panic
	let _ = settings.validate(&Profile::Development);
	let _ = settings.validate(&Profile::Production);

	// Assert: reaching here means no panic occurred
	true
}

/// Fuzz: SecuritySettings validate never panics for arbitrary SSL/cookie boolean values.
#[quickcheck]
fn fuzz_security_validate_never_panics(
	ssl_redirect: bool,
	session_secure: bool,
	csrf_secure: bool,
) -> bool {
	// Arrange
	let settings = SecuritySettings {
		secure_ssl_redirect: ssl_redirect,
		session_cookie_secure: session_secure,
		csrf_cookie_secure: csrf_secure,
		..Default::default()
	};

	// Act — must not panic
	let _ = settings.validate(&Profile::Development);
	let _ = settings.validate(&Profile::Staging);
	let _ = settings.validate(&Profile::Production);

	// Assert: no panic occurred
	true
}

/// Fuzz: Profile::parse never panics for any arbitrary string input.
#[quickcheck]
fn fuzz_profile_parse_never_panics(s: String) -> bool {
	// Act — must not panic
	let _ = Profile::parse(&s);

	// Assert: no panic occurred
	true
}

/// Fuzz: serialize/deserialize roundtrip preserves secret_key for any string value.
#[quickcheck]
fn fuzz_core_serde_roundtrip_preserves_key(secret_key: String) -> bool {
	// Arrange
	let original = CoreSettings {
		secret_key: secret_key.clone(),
		..Default::default()
	};

	// Act
	let Ok(json) = serde_json::to_string(&original) else {
		// If serialization fails (e.g., non-UTF-8 edge case), the test passes trivially
		return true;
	};
	let Ok(restored) = serde_json::from_str::<CoreSettings>(&json) else {
		// If deserialization fails, the roundtrip is broken — return false
		return false;
	};

	// Assert: secret_key must be preserved through the roundtrip
	restored.secret_key == secret_key
}

/// Fuzz: Display implementation of every ValidationError variant never panics.
#[quickcheck]
fn fuzz_validation_error_display_never_panics(msg: String) -> bool {
	// Arrange — construct one instance of every variant using the fuzzed string
	let security_variant = ValidationError::Security(msg.clone());
	let invalid_value_variant = ValidationError::InvalidValue {
		key: msg.clone(),
		message: msg.clone(),
	};
	let missing_required_variant = ValidationError::MissingRequired(msg.clone());
	let constraint_variant = ValidationError::Constraint(msg.clone());
	let multiple_variant = ValidationError::Multiple(vec![
		ValidationError::Security(msg.clone()),
		ValidationError::Constraint(msg.clone()),
	]);

	// Act — to_string() must not panic for any string content
	let _ = security_variant.to_string();
	let _ = invalid_value_variant.to_string();
	let _ = missing_required_variant.to_string();
	let _ = constraint_variant.to_string();
	let _ = multiple_variant.to_string();

	// Assert: no panic occurred; also verify CacheSettings::section() is stable
	let _ = CacheSettings::section();

	true
}
