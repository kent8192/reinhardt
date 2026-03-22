use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use rstest::rstest;

// ============================================================
// Test 1: CoreSettings production validation decision table
// ============================================================

/// Decision table covering all 8 combinations of (debug, secret_key_empty, allowed_hosts_empty)
/// for Production profile validation.
#[rstest]
#[case::all_valid(false, false, false, true)]
#[case::debug_invalid(true, false, false, false)]
#[case::key_empty(false, true, false, false)]
#[case::hosts_empty(false, false, true, false)]
#[case::debug_and_key(true, true, false, false)]
#[case::debug_and_hosts(true, false, true, false)]
#[case::key_and_hosts(false, true, true, false)]
#[case::all_invalid(true, true, true, false)]
fn core_production_validation_decision_table(
	#[case] debug: bool,
	#[case] secret_key_empty: bool,
	#[case] allowed_hosts_empty: bool,
	#[case] expected_pass: bool,
) {
	// Arrange
	let production_security = SecuritySettings {
		secure_ssl_redirect: true,
		session_cookie_secure: true,
		csrf_cookie_secure: true,
		..Default::default()
	};
	let secret_key = if secret_key_empty {
		String::new()
	} else {
		"a-valid-secret-key-for-testing".to_string()
	};
	let allowed_hosts = if allowed_hosts_empty {
		vec![]
	} else {
		vec!["example.com".to_string()]
	};
	let settings = CoreSettings {
		secret_key,
		debug,
		allowed_hosts,
		security: production_security,
		..Default::default()
	};
	let profile = Profile::Production;

	// Act
	let result = settings.validate(&profile);

	// Assert
	if expected_pass {
		assert!(
			result.is_ok(),
			"Case (debug={}, key_empty={}, hosts_empty={}) should pass, but got: {:?}",
			debug,
			secret_key_empty,
			allowed_hosts_empty,
			result.err()
		);
	} else {
		assert!(
			result.is_err(),
			"Case (debug={}, key_empty={}, hosts_empty={}) should fail, but it passed",
			debug,
			secret_key_empty,
			allowed_hosts_empty
		);
	}
}

// ============================================================
// Test 2: SecuritySettings production validation decision table
// ============================================================

/// Decision table covering all 8 combinations of the three production security flags.
#[rstest]
#[case::all_secure(true, true, true, true)]
#[case::no_ssl(false, true, true, false)]
#[case::no_session_cookie(true, false, true, false)]
#[case::no_csrf_cookie(true, true, false, false)]
#[case::none_secure(false, false, false, false)]
#[case::only_ssl(true, false, false, false)]
#[case::only_session(false, true, false, false)]
#[case::only_csrf(false, false, true, false)]
fn security_production_validation_decision_table(
	#[case] ssl_redirect: bool,
	#[case] session_cookie_secure: bool,
	#[case] csrf_cookie_secure: bool,
	#[case] expected_pass: bool,
) {
	// Arrange
	let settings = SecuritySettings {
		secure_ssl_redirect: ssl_redirect,
		session_cookie_secure,
		csrf_cookie_secure,
		..Default::default()
	};
	let profile = Profile::Production;

	// Act
	let result = settings.validate(&profile);

	// Assert
	if expected_pass {
		assert!(
			result.is_ok(),
			"Case (ssl={}, session={}, csrf={}) should pass, but got: {:?}",
			ssl_redirect,
			session_cookie_secure,
			csrf_cookie_secure,
			result.err()
		);
	} else {
		assert!(
			result.is_err(),
			"Case (ssl={}, session={}, csrf={}) should fail, but it passed",
			ssl_redirect,
			session_cookie_secure,
			csrf_cookie_secure
		);
	}
}

// ============================================================
// Test 3: Profile vs debug validation matrix
// ============================================================

/// Matrix verifying which (profile, debug) combinations cause validation failure.
#[rstest]
#[case::dev_debug_ok(Profile::Development, true, false)]
#[case::staging_debug_ok(Profile::Staging, true, false)]
#[case::custom_debug_ok(Profile::Custom, true, false)]
#[case::prod_debug_fail(Profile::Production, true, true)]
#[case::prod_no_debug_ok(Profile::Production, false, false)]
fn profile_debug_validation_matrix(
	#[case] profile: Profile,
	#[case] debug: bool,
	#[case] should_fail: bool,
) {
	// Arrange
	let is_production = matches!(profile, Profile::Production);
	let production_security = SecuritySettings {
		secure_ssl_redirect: true,
		session_cookie_secure: true,
		csrf_cookie_secure: true,
		..Default::default()
	};
	let allowed_hosts = if is_production {
		vec!["example.com".to_string()]
	} else {
		vec![]
	};
	let security = if is_production {
		production_security
	} else {
		SecuritySettings::default()
	};
	let settings = CoreSettings {
		secret_key: "a-valid-secret-key-for-testing".to_string(),
		debug,
		allowed_hosts,
		security,
		..Default::default()
	};

	// Act
	let result = settings.validate(&profile);

	// Assert
	if should_fail {
		assert!(
			result.is_err(),
			"Profile {:?} with debug={} should fail validation, but it passed",
			profile,
			debug
		);
	} else {
		assert!(
			result.is_ok(),
			"Profile {:?} with debug={} should pass validation, but got: {:?}",
			profile,
			debug,
			result.err()
		);
	}
}
