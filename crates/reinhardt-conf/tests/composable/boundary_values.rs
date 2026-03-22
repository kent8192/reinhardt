use reinhardt_conf::settings::cache::CacheSettings;
use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;
use reinhardt_conf::settings::session::SessionSettings;
use rstest::rstest;

// ============================================================
// Test 1: allowed_hosts count boundary for production
// ============================================================

/// Verify production CoreSettings validation boundary at allowed_hosts count.
/// Empty list must fail; one or more hosts must pass.
#[rstest]
#[case::empty(0, false)]
#[case::single(1, true)]
#[case::many(10, true)]
fn allowed_hosts_count_boundary_production(#[case] count: usize, #[case] should_pass: bool) {
	// Arrange
	let allowed_hosts: Vec<String> = (0..count).map(|i| format!("host-{}.com", i)).collect();
	let production_security = SecuritySettings {
		secure_ssl_redirect: true,
		session_cookie_secure: true,
		csrf_cookie_secure: true,
		..Default::default()
	};
	let settings = CoreSettings {
		secret_key: "a-valid-secret-key-for-testing".to_string(),
		debug: false,
		allowed_hosts,
		security: production_security,
		..Default::default()
	};
	let profile = Profile::Production;

	// Act
	let result = settings.validate(&profile);

	// Assert
	if should_pass {
		assert!(
			result.is_ok(),
			"allowed_hosts count={} should pass production validation, but got: {:?}",
			count,
			result.err()
		);
	} else {
		assert!(
			result.is_err(),
			"allowed_hosts count={} should fail production validation, but it passed",
			count
		);
	}
}

// ============================================================
// Test 2: Session cookie age boundary values
// ============================================================

/// Verify that SessionSettings accepts any u64 value for cookie_age
/// and that the value survives a serde roundtrip.
#[rstest]
#[case::zero(0u64)]
#[case::one(1u64)]
#[case::two_weeks(1209600u64)]
#[case::large(u64::MAX)]
fn session_cookie_age_boundary(#[case] age: u64) {
	// Arrange
	// SessionSettings is #[non_exhaustive]; use serde to build an instance with a
	// custom cookie_age value by starting from the JSON of the default.
	let mut map: serde_json::Map<String, serde_json::Value> = {
		let default_json = serde_json::to_string(&SessionSettings::default())
			.expect("default SessionSettings should serialize");
		serde_json::from_str(&default_json).expect("should deserialize into map")
	};
	map.insert("cookie_age".to_string(), serde_json::json!(age));
	let json = serde_json::to_string(&map).expect("map should serialize");
	let settings: SessionSettings =
		serde_json::from_str(&json).expect("SessionSettings should deserialize from JSON");

	// Act
	let roundtrip_json =
		serde_json::to_string(&settings).expect("SessionSettings should serialize to JSON");
	let restored: SessionSettings = serde_json::from_str(&roundtrip_json)
		.expect("SessionSettings should deserialize from JSON");

	// Assert
	assert_eq!(
		restored.cookie_age, age,
		"cookie_age={} must survive serde roundtrip",
		age
	);
}

// ============================================================
// Test 3: Cache timeout boundary values
// ============================================================

/// Verify that CacheSettings accepts any u64 value for timeout
/// and that the value survives a serde roundtrip.
#[rstest]
#[case::zero(0u64)]
#[case::one(1u64)]
#[case::default_val(300u64)]
#[case::large(u64::MAX)]
fn cache_timeout_boundary(#[case] timeout: u64) {
	// Arrange
	// CacheSettings is #[non_exhaustive]; use serde to build an instance with a
	// custom timeout value by starting from the JSON of the default.
	let mut map: serde_json::Map<String, serde_json::Value> = {
		let default_json = serde_json::to_string(&CacheSettings::default())
			.expect("default CacheSettings should serialize");
		serde_json::from_str(&default_json).expect("should deserialize into map")
	};
	map.insert("timeout".to_string(), serde_json::json!(timeout));
	let json = serde_json::to_string(&map).expect("map should serialize");
	let settings: CacheSettings =
		serde_json::from_str(&json).expect("CacheSettings should deserialize from JSON");

	// Act
	let roundtrip_json =
		serde_json::to_string(&settings).expect("CacheSettings should serialize to JSON");
	let restored: CacheSettings =
		serde_json::from_str(&roundtrip_json).expect("CacheSettings should deserialize from JSON");

	// Assert
	assert_eq!(
		restored.timeout, timeout,
		"timeout={} must survive serde roundtrip",
		timeout
	);
}

// ============================================================
// Test 4: Secret key length boundary for CoreSettings.validate
// ============================================================

/// Verify CoreSettings.validate behaviour at secret_key length boundaries.
/// Only emptiness is checked; any non-empty key is valid.
#[rstest]
#[case::empty(0, false)]
#[case::single_char(1, true)]
#[case::short(5, true)]
#[case::long(100, true)]
fn secret_key_length_boundary_core_validate(#[case] length: usize, #[case] should_pass_dev: bool) {
	// Arrange
	let secret_key = "a".repeat(length);
	let settings = CoreSettings {
		secret_key,
		..Default::default()
	};
	let profile = Profile::Development;

	// Act
	let result = settings.validate(&profile);

	// Assert
	if should_pass_dev {
		assert!(
			result.is_ok(),
			"secret_key length={} should pass Development validation, but got: {:?}",
			length,
			result.err()
		);
	} else {
		assert!(
			result.is_err(),
			"secret_key length={} should fail Development validation, but it passed",
			length
		);
	}
}
