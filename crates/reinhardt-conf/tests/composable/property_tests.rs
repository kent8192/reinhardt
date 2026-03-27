use proptest::prelude::*;
use reinhardt_conf::settings::cache::CacheSettings;
use reinhardt_conf::settings::core_settings::CoreSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::security::SecuritySettings;

proptest! {
	/// Property: cloning a CoreSettings instance preserves the secret_key field exactly.
	#[test]
	fn prop_core_clone_preserves_secret_key(key in "\\PC{1,100}") {
		// Arrange
		let original = CoreSettings {
			secret_key: key.clone(),
			..Default::default()
		};

		// Act
		let cloned = original.clone();

		// Assert
		prop_assert_eq!(
			original.secret_key,
			cloned.secret_key,
			"clone must preserve secret_key"
		);
	}

	/// Property: serialize then deserialize a CoreSettings value preserves both
	/// secret_key and debug across the JSON roundtrip.
	#[test]
	fn prop_core_serde_identity(
		key in "\\PC{1,100}",
		debug in any::<bool>(),
	) {
		// Arrange
		let original = CoreSettings {
			secret_key: key.clone(),
			debug,
			..Default::default()
		};

		// Act
		let json = serde_json::to_string(&original)
			.expect("CoreSettings must serialize to JSON");
		let restored: CoreSettings = serde_json::from_str(&json)
			.expect("CoreSettings must deserialize from JSON");

		// Assert
		prop_assert_eq!(
			&restored.secret_key,
			&key,
			"secret_key must survive serde identity"
		);
		prop_assert_eq!(
			restored.debug,
			debug,
			"debug flag must survive serde identity"
		);
	}

	/// Property: any non-empty lowercase alphabetic key always passes validation
	/// under Profile::Development (no strict production rules apply).
	#[test]
	fn prop_development_always_validates_with_key(key in "[a-z]{1,50}") {
		// Arrange
		let settings = CoreSettings {
			secret_key: key.clone(),
			// Use Development-safe defaults: debug=true, hosts may be empty
			..Default::default()
		};

		// Act
		let result = settings.validate(&Profile::Development);

		// Assert: a non-empty key is always sufficient for Development
		prop_assert!(
			result.is_ok(),
			"Development validation must pass for key={:?}, got: {:?}",
			key,
			result.err(),
		);
	}

	/// Property: if a CoreSettings instance passes Production validation, it must also
	/// pass Development validation (Production is strictly more restrictive).
	#[test]
	fn prop_valid_production_implies_valid_development(
		key in "[a-z]{32,64}",
		extra_host in "[a-z]{3,20}\\.[a-z]{2,6}",
	) {
		// Arrange — build settings that are valid for Production
		let settings = CoreSettings {
			secret_key: key,
			debug: false,
			allowed_hosts: vec![extra_host],
			security: SecuritySettings {
				secure_ssl_redirect: true,
				session_cookie_secure: true,
				csrf_cookie_secure: true,
				..Default::default()
			},
			..Default::default()
		};

		// Act
		let production_result = settings.validate(&Profile::Production);
		let development_result = settings.validate(&Profile::Development);

		// Assert: valid Production implies valid Development
		if production_result.is_ok() {
			prop_assert!(
				development_result.is_ok(),
				"valid-for-Production must also be valid-for-Development, \
				but Development validation failed: {:?}",
				development_result.err(),
			);
		}
	}

	/// Property: CacheSettings::section() always returns the same static string,
	/// regardless of instance data (backend, location, timeout).
	/// Uses Default instances because CacheSettings is #[non_exhaustive].
	#[test]
	fn prop_section_is_static(
		_backend in "[a-z]{1,20}",
		_timeout in any::<u64>(),
	) {
		// Arrange — CacheSettings is #[non_exhaustive], so use Default for construction
		let instance_a = CacheSettings::default();
		let instance_b = CacheSettings::default();

		// Act
		let section_a = CacheSettings::section();
		let section_b = CacheSettings::section();

		// Assert: both calls return the same value, regardless of instance contents
		prop_assert_eq!(section_a, section_b, "section() must be a stable static value");
		prop_assert_eq!(
			section_a,
			"cache",
			"CacheSettings section must always be 'cache', got: {:?}",
			section_a,
		);

		// Suppress unused-variable warnings for the constructed instances
		let _ = instance_a;
		let _ = instance_b;
	}
}
