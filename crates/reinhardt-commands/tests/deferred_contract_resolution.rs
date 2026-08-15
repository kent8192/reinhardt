//! Deferred settings and application contract resolution coverage.

#[cfg(feature = "contract")]
use reinhardt_commands::{ContractResolutionErrorKind, SafeContractTarget, resolve_contract_state};
use reinhardt_conf::indexmap::IndexMap;
use reinhardt_conf::settings::builder::{BuildError, SettingsBuilder};
use reinhardt_conf::settings::profile::Profile;
use reinhardt_conf::settings::sources::DefaultSource;
use reinhardt_conf::settings::validation::ValidationResult;
use reinhardt_conf::settings::{ComposedSettings, SettingsResolutionMetadata};
use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[settings(fragment = true, section = "service")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ServiceSettings {
	#[setting(required, secret)]
	secret: String,
}

#[settings(service: ServiceSettings | migrations: MigrationSettings)]
struct ProjectSettings;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManualSchemaSettings {
	#[serde(default)]
	value: String,
}

impl ComposedSettings for ManualSchemaSettings {
	fn validate_requirements(
		_merged: &IndexMap<String, serde_json::Value>,
	) -> Result<(), BuildError> {
		Ok(())
	}

	fn resolution_metadata(
		_merged: &IndexMap<String, serde_json::Value>,
	) -> Result<SettingsResolutionMetadata, BuildError> {
		Ok(SettingsResolutionMetadata::default())
	}

	fn validate_fragments(&self, _profile: &Profile) -> ValidationResult {
		Ok(())
	}
}

#[test]
fn pending_settings_expose_contract_inputs_before_required_validation() {
	let pending = SettingsBuilder::new()
		.with_typed_coercion(false)
		.add_source(DefaultSource::new().with_value("unrelated", json!(true)))
		.build_pending_composed::<ProjectSettings>()
		.expect("source merging must not validate required settings");

	let contract = pending.contract_state();
	assert_eq!(
		contract.merged,
		[("unrelated".to_string(), json!(true))].into()
	);
	assert_eq!(contract.root_schema.sections.len(), 2);
	assert_eq!(contract.root_schema.sections[0].canonical_key, "service");
	assert!(!contract.typed_coercion);

	assert!(matches!(
		pending.resolve(),
		Err(BuildError::MissingRequiredField {
			section: "service",
			field: "secret",
		})
	));
}

#[test]
fn section_deserialization_uses_typed_coercion() {
	let pending = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value(
			"migrations",
			json!({
				"migration_features": "[\"gis\"]",
				"migration_settings": "{\"ENABLE_AUDIT\":\"true\"}",
				"migration_swappable_settings": "{}"
			}),
		))
		.build_pending_composed::<ProjectSettings>()
		.expect("source merging should retain string containers");

	let migrations = pending
		.deserialize_section::<reinhardt_conf::MigrationSettings>("migrations")
		.expect("section deserialization should use typed coercion");

	assert_eq!(migrations.migration_features, ["gis"]);
	assert_eq!(
		migrations
			.migration_settings
			.get("ENABLE_AUDIT")
			.map(String::as_str),
		Some("true")
	);
}

#[tokio::test]
#[cfg(feature = "contract")]
async fn malformed_secret_is_discarded_at_the_resolution_boundary() {
	let sentinel = "contract-resolution-secret-sentinel-5986";
	let pending = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("migrations", json!(sentinel)))
		.build_pending_composed::<ProjectSettings>()
		.expect("source merging should retain malformed sections");

	let error = match resolve_contract_state(&pending, None).await {
		Ok(_) => panic!("malformed migration settings must fail safely"),
		Err(error) => error,
	};
	assert_eq!(error.kind, ContractResolutionErrorKind::SettingsSection);
	assert_eq!(
		error.safe_target,
		Some(SafeContractTarget::SettingsSection("migrations"))
	);

	let rendered = format!("{error:?} {error}");
	assert!(!rendered.contains(sentinel));
	let mut source = std::error::Error::source(&error);
	while let Some(error) = source {
		assert!(!error.to_string().contains(sentinel));
		source = error.source();
	}
}

#[tokio::test]
#[cfg(feature = "contract")]
async fn contract_resolution_rejects_missing_root_schema() {
	let pending = SettingsBuilder::new()
		.add_source(DefaultSource::new().with_value("migrations", json!({})))
		.build_pending_composed::<ManualSchemaSettings>()
		.expect("source merging should succeed without a generated schema");

	let error = match resolve_contract_state(&pending, None).await {
		Ok(_) => panic!("contract resolution must fail closed without a root schema"),
		Err(error) => error,
	};

	assert_eq!(error.kind, ContractResolutionErrorKind::SettingsSchema);
	assert_eq!(error.safe_target, None);
}
