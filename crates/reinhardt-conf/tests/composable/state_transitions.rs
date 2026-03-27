use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use rstest::rstest;

// ===========================================================================
// Development to Production breaks validation
// ===========================================================================

#[rstest]
fn development_to_production_breaks_validation() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "a-sufficiently-long-secret-key-value".to_string(),
		debug: true,
		allowed_hosts: vec!["example.com".to_string()],
		..Default::default()
	};

	// Act
	let dev_result = settings.validate(&Profile::Development);
	let prod_result = settings.validate(&Profile::Production);

	// Assert
	assert!(
		dev_result.is_ok(),
		"development profile should accept debug=true"
	);
	assert!(
		prod_result.is_err(),
		"production profile should reject debug=true"
	);
}

// ===========================================================================
// Production to Development recovers
// ===========================================================================

#[rstest]
fn production_to_development_recovers() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "a-sufficiently-long-secret-key-value".to_string(),
		debug: true,
		allowed_hosts: vec!["example.com".to_string()],
		..Default::default()
	};

	// Act
	let prod_result = settings.validate(&Profile::Production);
	let dev_result = settings.validate(&Profile::Development);

	// Assert
	assert!(
		prod_result.is_err(),
		"production profile should reject debug=true"
	);
	assert!(
		dev_result.is_ok(),
		"development profile should accept same settings"
	);
}

// ===========================================================================
// Fix secret key then revalidate
// ===========================================================================

#[rstest]
fn fix_secret_key_then_revalidate() {
	// Arrange
	let mut settings = CoreSettings {
		secret_key: String::new(),
		debug: true,
		..Default::default()
	};

	// Act (initial state — missing key)
	let before = settings.validate(&Profile::Development);

	// Act (fix key)
	settings.secret_key = "now-i-have-a-real-key".to_string();
	let after = settings.validate(&Profile::Development);

	// Assert
	assert!(before.is_err(), "empty secret_key should fail validation");
	assert!(
		after.is_ok(),
		"non-empty secret_key should pass development validation"
	);
}

// ===========================================================================
// Fix debug flag then revalidate
// ===========================================================================

#[rstest]
fn fix_debug_flag_then_revalidate() {
	// Arrange
	let mut settings = CoreSettings {
		secret_key: "a-sufficiently-long-secret-key-value".to_string(),
		debug: true,
		allowed_hosts: vec!["example.com".to_string()],
		security: SecuritySettings {
			secure_ssl_redirect: true,
			session_cookie_secure: true,
			csrf_cookie_secure: true,
			..Default::default()
		},
		..Default::default()
	};

	// Act (initial state — debug=true fails production)
	let before = settings.validate(&Profile::Production);

	// Act (set debug=false for production)
	settings.debug = false;
	let after = settings.validate(&Profile::Production);

	// Assert
	assert!(
		before.is_err(),
		"debug=true should fail production validation"
	);
	assert!(
		after.is_ok(),
		"debug=false with valid hosts and security should pass production"
	);
}

// ===========================================================================
// Fix security flags then revalidate
// ===========================================================================

#[rstest]
fn fix_security_flags_then_revalidate() {
	// Arrange
	let mut security = SecuritySettings {
		secure_ssl_redirect: false,
		session_cookie_secure: false,
		csrf_cookie_secure: false,
		..Default::default()
	};
	let mut settings = CoreSettings {
		secret_key: "a-sufficiently-long-secret-key-value".to_string(),
		debug: false,
		allowed_hosts: vec!["example.com".to_string()],
		security: security.clone(),
		..Default::default()
	};

	// Act (initial state — ssl_redirect=false fails production)
	let before = settings.validate(&Profile::Production);

	// Act (fix all three security flags)
	security.secure_ssl_redirect = true;
	security.session_cookie_secure = true;
	security.csrf_cookie_secure = true;
	settings.security = security;
	let after = settings.validate(&Profile::Production);

	// Assert
	assert!(
		before.is_err(),
		"missing security flags should fail production validation"
	);
	assert!(
		after.is_ok(),
		"all security flags set should pass production validation"
	);
}

// ===========================================================================
// Staging matches Development leniency
// ===========================================================================

#[rstest]
fn staging_matches_development_leniency() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "staging-secret-key".to_string(),
		debug: true,
		..Default::default()
	};

	// Act
	let staging_result = settings.validate(&Profile::Staging);
	let dev_result = settings.validate(&Profile::Development);

	// Assert
	assert!(
		staging_result.is_ok(),
		"staging profile should allow debug=true like development"
	);
	assert!(
		dev_result.is_ok(),
		"development profile should allow debug=true"
	);
}

// ===========================================================================
// Custom profile matches Development leniency
// ===========================================================================

#[rstest]
fn custom_profile_matches_development_leniency() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "custom-env-secret-key".to_string(),
		debug: true,
		..Default::default()
	};

	// Act
	let custom_result = settings.validate(&Profile::Custom);
	let dev_result = settings.validate(&Profile::Development);

	// Assert
	assert!(
		custom_result.is_ok(),
		"custom profile should allow debug=true like development"
	);
	assert!(
		dev_result.is_ok(),
		"development profile should allow debug=true"
	);
}
