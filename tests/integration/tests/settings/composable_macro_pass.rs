//! Compile-success tests for `#[settings]` proc macro.
//!
//! These tests verify that the `#[settings(fragment = true, section = "...")]`
//! and `#[settings(key: Type | !Type)]` macros compile correctly and produce
//! the expected traits, fields, and validation methods.
//!
//! Placed in the integration test crate because the macros generate code
//! referencing `reinhardt_conf`, which is not available in the macro crate's
//! dev-dependencies due to circular dependency constraints.

use reinhardt_conf::settings::core_settings::{CoreSettings, HasCoreSettings};
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_macros::settings;
use rstest::rstest;

// ============================================================================
// Fragment macro pass tests
// ============================================================================

/// Basic fragment with section — verifies SettingsFragment impl and HasXSettings trait.
#[settings(fragment = true, section = "custom_db")]
struct CustomDbSettings {
	pub host: String,
	pub port: u16,
}

/// Fragment with existing derives — macro must not add duplicates.
#[settings(fragment = true, section = "rate_limit")]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
struct RateLimitSettings {
	pub max_requests: u32,
	pub window_seconds: u64,
}

/// Fragment with no fields (unit-like struct body).
#[settings(fragment = true, section = "empty_section")]
struct EmptySettings;

#[rstest]
fn fragment_basic_section_is_correct() {
	// Arrange / Act
	let section = CustomDbSettings::section();

	// Assert
	assert_eq!(
		section, "custom_db",
		"Fragment section should match the attribute value"
	);
}

#[rstest]
fn fragment_with_existing_derives_section_is_correct() {
	// Arrange / Act
	let section = RateLimitSettings::section();

	// Assert
	assert_eq!(
		section, "rate_limit",
		"Fragment with existing derives should still implement SettingsFragment"
	);
}

#[rstest]
fn fragment_with_existing_derives_has_partial_eq() {
	// Arrange
	let a = RateLimitSettings {
		max_requests: 100,
		window_seconds: 60,
	};
	let b = RateLimitSettings {
		max_requests: 100,
		window_seconds: 60,
	};

	// Act / Assert
	assert_eq!(
		a, b,
		"PartialEq should work for fragment with custom derives"
	);
}

#[rstest]
fn fragment_empty_struct_section_is_correct() {
	// Arrange / Act
	let section = EmptySettings::section();

	// Assert
	assert_eq!(
		section, "empty_section",
		"Empty fragment should implement SettingsFragment"
	);
}

#[rstest]
fn fragment_generates_has_trait() {
	// Arrange
	// The macro generates `HasCustomDbSettings` trait with method `custom_db()`
	struct Wrapper {
		db: CustomDbSettings,
	}

	impl HasCustomDbSettings for Wrapper {
		fn custom_db(&self) -> &CustomDbSettings {
			&self.db
		}
	}

	let wrapper = Wrapper {
		db: CustomDbSettings {
			host: "localhost".to_string(),
			port: 5432,
		},
	};

	// Act
	let db = wrapper.custom_db();

	// Assert
	assert_eq!(
		db.host, "localhost",
		"HasCustomDbSettings trait should provide access"
	);
	assert_eq!(
		db.port, 5432,
		"HasCustomDbSettings trait should provide access to all fields"
	);
}

#[rstest]
fn fragment_auto_derives_clone_debug_serde() {
	// Arrange
	let original = CustomDbSettings {
		host: "db.example.com".to_string(),
		port: 3306,
	};

	// Act — Clone
	let cloned = original.clone();

	// Act — Debug
	let debug_str = format!("{:?}", cloned);

	// Act — Serde roundtrip
	let json = serde_json::to_string(&original).unwrap();
	let deserialized: CustomDbSettings = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(
		cloned.host, "db.example.com",
		"Clone should preserve fields"
	);
	assert!(
		debug_str.contains("CustomDbSettings"),
		"Debug should include type name"
	);
	assert_eq!(
		deserialized.host, "db.example.com",
		"Serde roundtrip should preserve fields"
	);
	assert_eq!(
		deserialized.port, 3306,
		"Serde roundtrip should preserve all fields"
	);
}

// ============================================================================
// Composition macro pass tests
// ============================================================================

/// Compose with CoreSettings and a single explicit fragment.
#[settings(core: CoreSettings | custom_db: CustomDbSettings)]
struct SingleFragmentSettings;

/// Compose with CoreSettings and multiple fragments.
#[settings(core: CoreSettings | custom_db: CustomDbSettings | rate_limit: RateLimitSettings)]
struct MultiFragmentSettings;

/// Compose without CoreSettings — only explicit fragments.
#[settings(custom_db: CustomDbSettings)]
struct NoCoreSettings;

/// Compose with only CoreSettings (explicit declaration required).
#[settings(core: CoreSettings)]
struct CoreOnlySettings;

#[rstest]
fn compose_single_fragment_has_core_and_custom() {
	// Arrange
	let settings = SingleFragmentSettings {
		core: reinhardt_conf::CoreSettings::default(),
		custom_db: CustomDbSettings {
			host: "localhost".to_string(),
			port: 5432,
		},
	};

	// Act
	let core = settings.core();
	let db = settings.custom_db();

	// Assert
	assert!(core.debug, "Explicit CoreSettings should be included");
	assert_eq!(
		db.host, "localhost",
		"Explicit fragment should be accessible via Has trait"
	);
}

#[rstest]
fn compose_multi_fragment_has_all_three() {
	// Arrange
	let settings = MultiFragmentSettings {
		core: reinhardt_conf::CoreSettings::default(),
		custom_db: CustomDbSettings {
			host: "db.local".to_string(),
			port: 5432,
		},
		rate_limit: RateLimitSettings {
			max_requests: 1000,
			window_seconds: 3600,
		},
	};

	// Act
	let core = settings.core();
	let db = settings.custom_db();
	let rl = settings.rate_limit();

	// Assert
	assert!(core.debug, "CoreSettings should be included");
	assert_eq!(db.port, 5432, "CustomDbSettings should be accessible");
	assert_eq!(
		rl.max_requests, 1000,
		"RateLimitSettings should be accessible"
	);
}

#[rstest]
fn compose_exclude_core_only_has_explicit() {
	// Arrange
	let settings = NoCoreSettings {
		custom_db: CustomDbSettings {
			host: "remote.db".to_string(),
			port: 3306,
		},
	};

	// Act
	let db = settings.custom_db();

	// Assert
	assert_eq!(
		db.host, "remote.db",
		"Only explicit fragment should be present when CoreSettings is omitted"
	);
	assert_eq!(db.port, 3306, "Only explicit fragment should exist");
}

#[rstest]
fn compose_core_only_has_core() {
	// Arrange
	let settings = CoreOnlySettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "test-key".to_string(),
			..Default::default()
		},
	};

	// Act
	let core = settings.core();

	// Assert
	assert_eq!(
		core.secret_key, "test-key",
		"Explicit CoreSettings should be the only fragment"
	);
	assert!(core.debug, "CoreSettings default debug should be true");
}

#[rstest]
fn compose_generates_validate_method() {
	// Arrange
	let settings = SingleFragmentSettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "dev-key".to_string(),
			..Default::default()
		},
		custom_db: CustomDbSettings {
			host: "localhost".to_string(),
			port: 5432,
		},
	};

	// Act
	let result = settings.validate(&Profile::Development);

	// Assert
	assert!(
		result.is_ok(),
		"validate() should be generated and call all fragment validations: {result:?}"
	);
}

#[rstest]
fn compose_validate_delegates_to_fragments() {
	// Arrange — CoreSettings with empty secret_key should fail even in Development
	let settings = SingleFragmentSettings {
		core: reinhardt_conf::CoreSettings::default(), // secret_key is empty
		custom_db: CustomDbSettings {
			host: "localhost".to_string(),
			port: 5432,
		},
	};

	// Act
	let result = settings.validate(&Profile::Development);

	// Assert
	assert!(
		result.is_err(),
		"validate() should delegate to CoreSettings.validate() which fails on empty secret_key"
	);
}

#[rstest]
fn compose_serde_roundtrip() {
	// Arrange
	let original = MultiFragmentSettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "roundtrip-key".to_string(),
			..Default::default()
		},
		custom_db: CustomDbSettings {
			host: "serde.test".to_string(),
			port: 9999,
		},
		rate_limit: RateLimitSettings {
			max_requests: 500,
			window_seconds: 1800,
		},
	};

	// Act
	let json = serde_json::to_string(&original).unwrap();
	let deserialized: MultiFragmentSettings = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(
		deserialized.core.secret_key, "roundtrip-key",
		"CoreSettings should survive serde roundtrip"
	);
	assert_eq!(
		deserialized.custom_db.host, "serde.test",
		"CustomDbSettings should survive serde roundtrip"
	);
	assert_eq!(
		deserialized.rate_limit.max_requests, 500,
		"RateLimitSettings should survive serde roundtrip"
	);
}

#[rstest]
fn compose_has_trait_as_generic_bound() {
	// Arrange
	fn get_db_host(s: &impl HasCustomDbSettings) -> &str {
		&s.custom_db().host
	}

	let settings = SingleFragmentSettings {
		core: reinhardt_conf::CoreSettings::default(),
		custom_db: CustomDbSettings {
			host: "generic-bound.test".to_string(),
			port: 5432,
		},
	};

	// Act
	let host = get_db_host(&settings);

	// Assert
	assert_eq!(
		host, "generic-bound.test",
		"Has* trait should work as generic bound with composed settings"
	);
}
