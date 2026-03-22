use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use rstest::rstest;

// ============================================================
// Test 1: Profile::parse equivalence classes
// ============================================================

/// Verify that Profile::parse maps each known input string to the expected variant.
#[rstest]
#[case::development("development", Profile::Development)]
#[case::dev("dev", Profile::Development)]
#[case::develop("develop", Profile::Development)]
#[case::staging("staging", Profile::Staging)]
#[case::stage("stage", Profile::Staging)]
#[case::test_env("test", Profile::Staging)]
#[case::production("production", Profile::Production)]
#[case::prod("prod", Profile::Production)]
#[case::custom_word("custom", Profile::Custom)]
#[case::unknown("anything_else", Profile::Custom)]
#[case::uppercase_dev("DEVELOPMENT", Profile::Development)]
#[case::uppercase_prod("PRODUCTION", Profile::Production)]
#[case::mixed_case("Production", Profile::Production)]
fn profile_parse_equivalence_classes(#[case] input: &str, #[case] expected: Profile) {
	// Arrange
	// (input and expected are provided by the test case)

	// Act
	let result = Profile::parse(input);

	// Assert
	assert_eq!(
		result, expected,
		"Profile::parse({:?}) should return {:?}",
		input, expected
	);
}

// ============================================================
// Test 2: Non-production profiles allow debug=true
// ============================================================

/// Verify that non-production profiles accept debug=true without validation errors.
#[rstest]
#[case::development(Profile::Development)]
#[case::staging(Profile::Staging)]
#[case::custom(Profile::Custom)]
fn non_production_profiles_allow_debug_true(#[case] profile: Profile) {
	// Arrange
	let settings = CoreSettings {
		secret_key: "test-key".to_string(),
		debug: true,
		..Default::default()
	};

	// Act
	let result = settings.validate(&profile);

	// Assert
	assert!(
		result.is_ok(),
		"Profile {:?} should allow debug=true, but validation failed: {:?}",
		profile,
		result.err()
	);
}

// ============================================================
// Test 3: Production rejects debug=true
// ============================================================

/// Verify that the Production profile rejects debug=true.
#[rstest]
fn production_rejects_debug_true() {
	// Arrange
	let settings = CoreSettings {
		secret_key: "test-key".to_string(),
		debug: true,
		..Default::default()
	};
	let profile = Profile::Production;

	// Act
	let result = settings.validate(&profile);

	// Assert
	assert!(
		result.is_err(),
		"Production profile must reject debug=true, but validation passed"
	);
}

// ============================================================
// Test 4: .env file name for each profile
// ============================================================

/// Verify that each profile returns the correct .env file name.
#[rstest]
#[case::dev(Profile::Development, ".env.development")]
#[case::staging(Profile::Staging, ".env.staging")]
#[case::production(Profile::Production, ".env.production")]
#[case::custom(Profile::Custom, ".env")]
fn profile_env_file_name_classes(#[case] profile: Profile, #[case] expected: &str) {
	// Arrange
	// (profile and expected are provided by the test case)

	// Act
	let result = profile.env_file_name();

	// Assert
	assert_eq!(
		result, expected,
		"Profile {:?} env_file_name should be {:?}",
		profile, expected
	);
}
