//! Compile-success tests for `#[settings]` proc macro.
//!
//! These tests verify that the `#[settings(fragment = true, section = "...")]`
//! and `#[settings(key: Type | Type | key: Type)]` macros compile correctly
//! and produce the expected traits, fields, and validation methods.
//!
//! Composition supports both explicit (`key: Type`) and implicit (`Type`)
//! syntax, where implicit entries infer the field name from the type name.
//!
//! Placed in the integration test crate because the macros generate code
//! referencing `reinhardt_conf`, which is not available in the macro crate's
//! dev-dependencies due to circular dependency constraints.

use std::collections::{BTreeMap, HashMap};

use reinhardt_auth::settings::JwtSessionSettings;
use reinhardt_conf::indexmap::IndexMap;
use reinhardt_conf::settings::builder::{BuildError, SettingsBuilder};
use reinhardt_conf::settings::cache::HasCacheSettings;
use reinhardt_conf::settings::core_settings::{CoreSettings, HasCoreSettings};
use reinhardt_conf::settings::email::EmailSettings as FragmentEmailSettings;
use reinhardt_conf::settings::fragment::SettingsFragment;
use reinhardt_conf::settings::openapi::{HasOpenApiSettings, OpenApiSettings};
use reinhardt_conf::settings::policy::FieldRequirement;
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::schema::{
	HasSettingsSchema, SecretFieldRef, SettingsNode, SettingsValueSchema,
};
use reinhardt_conf::settings::secret_types::SecretString;
use reinhardt_conf::settings::sources::DefaultSource;
use reinhardt_macros::settings;
use reinhardt_websockets::settings::RedisChannelSettings;
use rstest::rstest;
use serde_json::json;

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

/// Fragment with no fields (empty named-field struct).
#[settings(fragment = true, section = "empty_section")]
struct EmptySettings {}

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
fn fragment_implements_generic_has_settings_for_itself() {
	// Arrange
	let settings = CustomDbSettings {
		host: "localhost".to_string(),
		port: 5432,
	};

	// Act
	let db = settings.custom_db();

	// Assert
	assert_eq!(
		db.host, "localhost",
		"Fragment should implement HasSettings<Self> without a public blanket impl"
	);
	assert_eq!(db.port, 5432);
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

/// Compose the built-in email fragment as downstream projects do.
#[settings(core: CoreSettings | email: FragmentEmailSettings)]
struct MailProjectSettings;

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
fn composed_email_fragment_builds_smtp_backend() {
	// Arrange
	let settings = MailProjectSettings {
		core: CoreSettings::default(),
		email: FragmentEmailSettings::default(),
	};

	// Act
	let fragment_result = reinhardt_mail::create_smtp_backend_from_settings(&settings.email);
	let composed_result = reinhardt_mail::create_smtp_backend_from_settings(&settings);

	// Assert
	assert!(
		fragment_result.is_ok(),
		"SMTP backend helper must accept the #[settings] EmailSettings fragment: {:?}",
		fragment_result.err()
	);
	assert!(
		composed_result.is_ok(),
		"SMTP backend helper must accept composed settings via HasSettings<EmailSettings>: {:?}",
		composed_result.err()
	);
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

// ============================================================================
// Type-only syntax (implicit field name inference) pass tests
// ============================================================================

/// Type-only syntax: CoreSettings | CacheSettings (field names inferred).
#[settings(CoreSettings | CacheSettings)]
struct TypeOnlySettings;

/// Mixed syntax: implicit CoreSettings + explicit custom_db.
#[settings(CoreSettings | custom_db: CustomDbSettings)]
struct MixedSyntaxSettings;

#[rstest]
fn compose_type_only_infers_field_names() {
	// Arrange
	let settings = TypeOnlySettings {
		core: reinhardt_conf::CoreSettings::default(),
		cache: reinhardt_conf::CacheSettings::default(),
	};

	// Act
	let core = settings.core();
	let cache = settings.cache();

	// Assert
	assert!(
		core.debug,
		"Type-only CoreSettings should be accessible via inferred field name"
	);
	assert_eq!(
		cache.backend,
		reinhardt_conf::CacheSettings::default().backend,
		"Type-only CacheSettings should be accessible via inferred field name"
	);
}

#[rstest]
fn compose_mixed_syntax_combines_implicit_and_explicit() {
	// Arrange
	let settings = MixedSyntaxSettings {
		core: reinhardt_conf::CoreSettings::default(),
		custom_db: CustomDbSettings {
			host: "mixed.test".to_string(),
			port: 5432,
		},
	};

	// Act
	let core = settings.core();
	let db = settings.custom_db();

	// Assert
	assert!(
		core.debug,
		"Implicit CoreSettings should work alongside explicit fragments"
	);
	assert_eq!(
		db.host, "mixed.test",
		"Explicit fragment should be accessible in mixed syntax"
	);
}

#[rstest]
fn compose_type_only_validate_works() {
	// Arrange
	let settings = TypeOnlySettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "test-key".to_string(),
			..Default::default()
		},
		cache: reinhardt_conf::CacheSettings::default(),
	};

	// Act
	let result = settings.validate(&Profile::Development);

	// Assert
	assert!(
		result.is_ok(),
		"validate() should work with type-only syntax: {result:?}"
	);
}

#[rstest]
fn compose_type_only_serde_roundtrip() {
	// Arrange
	let original = TypeOnlySettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "roundtrip".to_string(),
			..Default::default()
		},
		cache: reinhardt_conf::CacheSettings::default(),
	};

	// Act
	let json = serde_json::to_string(&original).unwrap();
	let deserialized: TypeOnlySettings = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(
		deserialized.core.secret_key, "roundtrip",
		"Type-only syntax should survive serde roundtrip"
	);
}

// ============================================================================
// Field policies tests — #[setting(...)] attribute and default_policy
// ============================================================================

/// Fragment with explicit field-level setting attributes.
#[settings(fragment = true, section = "field_policy_test")]
struct FieldPolicyFragment {
	#[setting(required)]
	pub api_key: String,
	#[setting(optional)]
	pub timeout: u64,
	#[setting(default = "8080")]
	pub port: u16,
	pub unset_field: String,
}

#[rstest]
fn fragment_field_policies_returns_correct_metadata() {
	// Arrange / Act
	let policies = FieldPolicyFragment::field_policies();

	// Assert
	assert_eq!(policies.len(), 4, "Should have a policy for each field");

	// api_key: required
	assert_eq!(policies[0].name, "api_key");
	assert_eq!(policies[0].requirement, FieldRequirement::Required);
	assert!(!policies[0].has_default);

	// timeout: optional
	assert_eq!(policies[1].name, "timeout");
	assert_eq!(policies[1].requirement, FieldRequirement::Optional);
	assert!(policies[1].has_default);

	// port: default = "8080"
	assert_eq!(policies[2].name, "port");
	assert_eq!(policies[2].requirement, FieldRequirement::Optional);
	assert!(policies[2].has_default);

	// unset_field: inherits default_policy (optional by default)
	assert_eq!(policies[3].name, "unset_field");
	assert_eq!(policies[3].requirement, FieldRequirement::Optional);
	assert!(policies[3].has_default);
}

/// Fragment with `default_policy = "required"` — all unmarked fields become required.
#[settings(fragment = true, section = "strict_test", default_policy = "required")]
struct StrictFragment {
	pub host: String,
	#[setting(optional)]
	pub timeout: u64,
}

#[rstest]
fn fragment_default_policy_required_makes_unmarked_fields_required() {
	// Arrange / Act
	let policies = StrictFragment::field_policies();

	// Assert
	assert_eq!(policies.len(), 2);

	// host: inherits default_policy = "required"
	assert_eq!(policies[0].name, "host");
	assert_eq!(policies[0].requirement, FieldRequirement::Required);
	assert!(!policies[0].has_default);

	// timeout: explicitly optional
	assert_eq!(policies[1].name, "timeout");
	assert_eq!(policies[1].requirement, FieldRequirement::Optional);
	assert!(policies[1].has_default);
}

/// Fragment with `default_policy = "optional"` — explicit, same as default behavior.
#[settings(fragment = true, section = "lenient_test", default_policy = "optional")]
struct LenientFragment {
	pub host: String,
	#[setting(required)]
	pub secret: String,
}

#[rstest]
fn fragment_default_policy_optional_makes_unmarked_fields_optional() {
	// Arrange / Act
	let policies = LenientFragment::field_policies();

	// Assert
	assert_eq!(policies.len(), 2);

	// host: inherits default_policy = "optional"
	assert_eq!(policies[0].name, "host");
	assert_eq!(policies[0].requirement, FieldRequirement::Optional);
	assert!(policies[0].has_default);

	// secret: explicitly required
	assert_eq!(policies[1].name, "secret");
	assert_eq!(policies[1].requirement, FieldRequirement::Required);
	assert!(!policies[1].has_default);
}

/// Fragment with default expression — verify serde deserialization uses the default.
#[rstest]
fn fragment_default_expr_applies_on_deserialization() {
	// Arrange
	let json = r#"{"api_key":"test","timeout":30,"unset_field":"val"}"#;

	// Act — port is missing from JSON, should use default = 8080
	let fragment: FieldPolicyFragment = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(
		fragment.port, 8080,
		"Missing field with default expression should use the default value"
	);
}

/// Fragment without any #[setting] attrs — backward compatible, all optional by default.
#[rstest]
fn fragment_without_setting_attrs_returns_all_optional() {
	// Arrange / Act
	let policies = CustomDbSettings::field_policies();

	// Assert
	assert_eq!(policies.len(), 2);
	for policy in policies {
		assert_eq!(
			policy.requirement,
			FieldRequirement::Optional,
			"Field '{}' should be optional by default",
			policy.name,
		);
		assert!(
			policy.has_default,
			"Field '{}' should have has_default=true by default",
			policy.name,
		);
	}
}

// ============================================================================
// OpenApiSettings composition tests
// ============================================================================

/// Compose CoreSettings with OpenApiSettings.
#[settings(core: CoreSettings | openapi: OpenApiSettings)]
struct WithOpenApiSettings;

#[rstest]
fn compose_openapi_has_core_and_openapi() {
	// Arrange
	let settings = WithOpenApiSettings {
		core: reinhardt_conf::CoreSettings::default(),
		openapi: OpenApiSettings::default(),
	};

	// Act
	let core = settings.core();
	let openapi = settings.openapi();

	// Assert
	assert!(core.debug, "CoreSettings should be accessible");
	assert!(openapi.enabled, "OpenApiSettings should be accessible");
	assert_eq!(openapi.swagger_path, "/api/docs");
}

#[rstest]
fn compose_openapi_custom_values() {
	// Arrange
	// Use serde to construct OpenApiSettings with custom values
	// because the struct is #[non_exhaustive] (cannot use struct literal from outside crate)
	let openapi: OpenApiSettings = serde_json::from_str(
		r#"{"title":"My REST API","version":"2.0.0","swagger_path":"/swagger"}"#,
	)
	.unwrap();
	let settings = WithOpenApiSettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "test-key".to_string(),
			..Default::default()
		},
		openapi,
	};

	// Act
	let openapi = settings.openapi();

	// Assert
	assert_eq!(openapi.title, "My REST API");
	assert_eq!(openapi.version, "2.0.0");
	assert_eq!(openapi.swagger_path, "/swagger");
	assert_eq!(openapi.redoc_path, "/api/redoc");
}

#[rstest]
fn compose_openapi_serde_roundtrip() {
	// Arrange
	let openapi: OpenApiSettings =
		serde_json::from_str(r#"{"title":"Serde Test","description":"Roundtrip check"}"#).unwrap();
	let original = WithOpenApiSettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "roundtrip-key".to_string(),
			..Default::default()
		},
		openapi,
	};

	// Act
	let json = serde_json::to_string(&original).unwrap();
	let deserialized: WithOpenApiSettings = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.core.secret_key, "roundtrip-key");
	assert_eq!(deserialized.openapi.title, "Serde Test");
	assert_eq!(
		deserialized.openapi.description,
		Some("Roundtrip check".to_string())
	);
}

#[rstest]
fn compose_openapi_validate_delegates() {
	// Arrange
	let settings = WithOpenApiSettings {
		core: reinhardt_conf::CoreSettings {
			secret_key: "valid-key".to_string(),
			..Default::default()
		},
		openapi: OpenApiSettings::default(),
	};

	// Act
	let result = settings.validate(&Profile::Development);

	// Assert
	assert!(result.is_ok(), "validate() should pass: {result:?}");
}

#[rstest]
fn compose_openapi_has_trait_as_generic_bound() {
	// Arrange
	fn get_swagger_path(s: &impl HasOpenApiSettings) -> &str {
		&s.openapi().swagger_path
	}

	// Use serde to construct with custom swagger_path (#[non_exhaustive])
	let openapi: OpenApiSettings =
		serde_json::from_str(r#"{"swagger_path":"/custom/docs"}"#).unwrap();
	let settings = WithOpenApiSettings {
		core: reinhardt_conf::CoreSettings::default(),
		openapi,
	};

	// Act
	let path = get_swagger_path(&settings);

	// Assert
	assert_eq!(path, "/custom/docs");
}

/// Type-only syntax with OpenApiSettings.
/// The macro infers the field name as `open_api` from `OpenApiSettings`.
#[settings(CoreSettings | OpenApiSettings)]
struct TypeOnlyWithOpenApi;

#[rstest]
fn compose_openapi_type_only_syntax() {
	// Arrange
	// Type-only syntax infers field name `open_api` from `OpenApiSettings`
	let openapi: OpenApiSettings = serde_json::from_str(r#"{"title":"Type-Only Test"}"#).unwrap();
	let settings = TypeOnlyWithOpenApi {
		core: reinhardt_conf::CoreSettings::default(),
		open_api: openapi,
	};

	// Act — trait method .openapi() works via HasSettings<OpenApiSettings> blanket impl
	let openapi = settings.openapi();

	// Assert
	assert_eq!(openapi.title, "Type-Only Test");
}

// ============================================================================
// Embedded settings node schema tests
// ============================================================================

#[settings(fragment = true, default_policy = "required")]
struct SchemaDatabaseConfig {
	pub engine: String,
	pub host: String,
	#[serde(rename = "db-password")]
	pub password: SecretString,
}

impl Default for SchemaDatabaseConfig {
	fn default() -> Self {
		Self {
			engine: String::new(),
			host: String::new(),
			password: SecretString::new(""),
		}
	}
}

#[settings(fragment = true, section = "schema_leaf_config")]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
struct SchemaLeafConfig {
	pub label: String,
}

#[settings(fragment = true, section = "database")]
struct SchemaDatabaseSettings {
	#[setting(required)]
	pub default: SchemaDatabaseConfig,
	pub replica: Option<SchemaDatabaseConfig>,
	pub pools: HashMap<String, SchemaDatabaseConfig>,
	pub ordered: BTreeMap<String, SchemaDatabaseConfig>,
	pub indexed: IndexMap<String, SchemaDatabaseConfig>,
	pub shards: Vec<SchemaDatabaseConfig>,
	pub boxed: Box<SchemaDatabaseConfig>,
	pub tokens: Vec<SecretString>,
	pub optional_token: Option<SecretString>,
	#[setting(leaf)]
	pub leaf: SchemaLeafConfig,
}

#[settings(database: SchemaDatabaseSettings)]
struct SchemaProjectSettings;

#[settings(SchemaDatabaseSettings)]
struct TypeOnlySchemaProjectSettings;

#[settings(fragment = true)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
struct MetadataNode {
	value: String,
}

#[settings(fragment = true, section = "metadata")]
struct MetadataFragment {
	leaf: String,
	#[setting(node)]
	node: MetadataNode,
	items: Vec<String>,
}

#[settings(metadata: MetadataFragment { leaf: required, node: required, items: required })]
struct MetadataProjectSettings;

fn schema_database_config(host: &str) -> SchemaDatabaseConfig {
	SchemaDatabaseConfig {
		engine: "postgres".to_string(),
		host: host.to_string(),
		password: SecretString::new(format!("{host}-password")),
	}
}

fn schema_database_settings() -> SchemaDatabaseSettings {
	let mut pools = HashMap::new();
	pools.insert("main".to_string(), schema_database_config("pool-main"));

	let mut ordered = BTreeMap::new();
	ordered.insert("east".to_string(), schema_database_config("ordered-east"));

	let mut indexed = IndexMap::new();
	indexed.insert("west".to_string(), schema_database_config("indexed-west"));

	SchemaDatabaseSettings {
		default: schema_database_config("primary.host"),
		replica: Some(schema_database_config("replica.host")),
		pools,
		ordered,
		indexed,
		shards: vec![schema_database_config("shard.host")],
		boxed: Box::new(schema_database_config("boxed.host")),
		tokens: vec![SecretString::new("token")],
		optional_token: Some(SecretString::new("optional-token")),
		leaf: SchemaLeafConfig {
			label: "leaf".to_string(),
		},
	}
}

#[rstest]
fn schema_fluent_refs_render_nested_paths() {
	// Arrange / Act
	let schema = SchemaProjectSettings::schema();

	// Assert
	assert_eq!(
		schema.database.default.password.path().to_string(),
		"database.default.db-password"
	);
	assert_eq!(
		schema.database.replica.some().password.path().to_string(),
		"database.replica.db-password"
	);
	assert_eq!(
		schema
			.database
			.pools
			.entry("main")
			.password
			.path()
			.to_string(),
		"database.pools.main.db-password"
	);
	assert_eq!(
		schema.database.pools.any().password.path().to_string(),
		"database.pools.*.db-password"
	);
	assert_eq!(
		schema.database.ordered.any().password.path().to_string(),
		"database.ordered.*.db-password"
	);
	assert_eq!(
		schema.database.indexed.any().password.path().to_string(),
		"database.indexed.*.db-password"
	);
	assert_eq!(
		schema.database.shards.any().password.path().to_string(),
		"database.shards.*.db-password"
	);
	assert_eq!(
		schema.database.boxed.password.path().to_string(),
		"database.boxed.db-password"
	);
	assert_eq!(
		schema.database.tokens.any().path().to_string(),
		"database.tokens.*"
	);
	assert_eq!(
		schema.database.optional_token.some().path().to_string(),
		"database.optional_token"
	);
	assert_eq!(schema.database.leaf.path().to_string(), "database.leaf");

	let database_schema = <SchemaDatabaseSettings as SettingsNode>::node_schema();
	let leaf_schema = database_schema
		.fields
		.iter()
		.find(|field| field.rust_name == "leaf")
		.expect("leaf field should be present in schema metadata");
	assert!(
		matches!(leaf_schema.value, SettingsValueSchema::Leaf { .. }),
		"`#[setting(leaf)]` should prevent a *Config field from becoming a settings node"
	);
}

#[rstest]
fn schema_secret_refs_are_typed() {
	// Arrange / Act
	let schema = SchemaProjectSettings::schema();
	let _: SecretFieldRef<SchemaProjectSettings, SecretString> = schema.database.default.password;
}

#[rstest]
fn built_in_secret_markers_classify_only_the_named_leaves() {
	fn terminal_secret(value: &SettingsValueSchema) -> Option<bool> {
		match value {
			SettingsValueSchema::Leaf { secret, .. } => Some(*secret),
			SettingsValueSchema::Optional { inner }
			| SettingsValueSchema::Sequence { inner }
			| SettingsValueSchema::Map { inner } => terminal_secret(inner),
			SettingsValueSchema::Node { .. } => None,
		}
	}

	fn assert_pair<T: SettingsNode>(secret_name: &str, plain_name: &str) {
		let schema = T::node_schema();
		let secret = schema
			.fields
			.iter()
			.find(|field| field.rust_name == secret_name)
			.expect("named secret field")
			.value
			.clone();
		let plain = schema
			.fields
			.iter()
			.find(|field| field.rust_name == plain_name)
			.expect("named non-secret field")
			.value
			.clone();

		assert_eq!(terminal_secret(&secret), Some(true));
		assert_eq!(terminal_secret(&plain), Some(false));
	}

	assert_pair::<CoreSettings>("secret_key", "debug");
	assert_pair::<FragmentEmailSettings>("password", "host");
	assert_pair::<JwtSessionSettings>("secret", "algorithm");
	assert_pair::<RedisChannelSettings>("password", "channel_prefix");
}

#[rstest]
fn resolved_composed_settings_export_leaf_policy_without_secret_values() {
	let sentinel = "not-a-secret-contract-sentinel-5985";
	let resolved = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value(
			"metadata",
			json!({ "leaf": sentinel, "node": { "value": "nested" }, "items": ["item"] }),
		))
		.build_resolved_composed::<MetadataProjectSettings>()
		.expect("resolved metadata project settings");
	let fields = resolved.metadata().fields();
	let leaf = fields
		.iter()
		.find(|field| field.path.to_string() == "metadata.leaf")
		.expect("leaf metadata");
	let nested = fields
		.iter()
		.find(|field| field.path.to_string() == "metadata.node.value")
		.expect("nested metadata");
	let item = fields
		.iter()
		.find(|field| field.path.to_string() == "metadata.items.*")
		.expect("container metadata");

	assert_eq!(leaf.policy.requirement, FieldRequirement::Required);
	assert_eq!(nested.policy.requirement, FieldRequirement::Optional);
	assert_eq!(item.policy.requirement, FieldRequirement::Optional);
	assert_eq!(resolved.settings().metadata.leaf, sentinel);
	assert!(!format!("{:?}", resolved.metadata()).contains(sentinel));
}

#[rstest]
fn schema_type_only_root_uses_section_hint() {
	// Arrange / Act
	let schema = TypeOnlySchemaProjectSettings::schema();
	let settings = TypeOnlySchemaProjectSettings {
		schema_database: schema_database_settings(),
	};

	// Assert
	assert_eq!(SchemaDatabaseSettings::section(), "database");
	assert_eq!(
		schema.schema_database.default.host.path().to_string(),
		"database.default.host"
	);
	assert_eq!(settings.schema_database.default.host, "primary.host");
}

#[rstest]
fn build_composed_reports_missing_nested_required_path() {
	// Arrange / Act
	let result = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value(
			"database",
			json!({
				"default": {
					"engine": "postgres",
					"host": "db.local"
				}
			}),
		))
		.build_composed::<SchemaProjectSettings>();

	// Assert
	match result {
		Err(BuildError::MissingRequiredPath { path }) => {
			assert_eq!(path.to_string(), "database.default.db-password");
		}
		other => panic!("expected MissingRequiredPath for nested database password, got {other:?}"),
	}
}

#[rstest]
fn build_composed_preserves_direct_missing_required_field() {
	// Arrange / Act
	let result = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("database", json!({})))
		.build_composed::<SchemaProjectSettings>();

	// Assert
	match result {
		Err(BuildError::MissingRequiredField { section, field }) => {
			assert_eq!(section, "database");
			assert_eq!(field, "default");
		}
		other => {
			panic!("expected MissingRequiredField for direct database.default miss, got {other:?}")
		}
	}
}

#[rstest]
fn schema_secret_fields_include_supported_wrappers() {
	// Arrange
	let schema = SchemaProjectSettings::schema();
	let mut paths = schema
		.database
		.secret_fields()
		.into_iter()
		.map(|field| field.path().to_string())
		.collect::<Vec<_>>();
	let mut expected = vec![
		"database.default.db-password".to_string(),
		"database.replica.db-password".to_string(),
		"database.pools.*.db-password".to_string(),
		"database.ordered.*.db-password".to_string(),
		"database.indexed.*.db-password".to_string(),
		"database.shards.*.db-password".to_string(),
		"database.boxed.db-password".to_string(),
		"database.tokens.*".to_string(),
		"database.optional_token".to_string(),
	];

	// Act
	paths.sort();
	expected.sort();

	// Assert
	assert_eq!(paths, expected);
}
