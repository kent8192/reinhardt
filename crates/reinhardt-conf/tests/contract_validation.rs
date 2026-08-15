//! Integration coverage for runtime settings contract validation.

use std::collections::HashMap;

use indexmap::IndexMap;
use reinhardt_conf::settings::ComposedSettings;
use reinhardt_conf::settings::schema::{
	JsonKind, SettingsPathSegment, SettingsViolationKind, verify_settings_contract,
};
use reinhardt_core::macros::settings;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

#[settings(fragment = true)]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct NestedConfig {
	port_number: u16,
}

#[settings(fragment = true, section = "service")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct ServiceSettings {
	#[setting(required)]
	#[serde(alias = "legacyPort")]
	port_number: u16,
	#[serde(default)]
	defaulted: u16,
	nested: NestedConfig,
	ports: Vec<u16>,
	ports_by_id: HashMap<u16, Vec<u16>>,
	#[setting(secret)]
	secrets: HashMap<String, u16>,
	optional_port: Option<u16>,
	#[serde(deserialize_with = "deserialize_opaque_ports")]
	opaque_ports: Vec<u16>,
}

fn deserialize_opaque_ports<'de, D>(deserializer: D) -> Result<Vec<u16>, D::Error>
where
	D: Deserializer<'de>,
{
	let _: String = Deserialize::deserialize(deserializer)?;
	Ok(vec![443])
}

#[settings(ServiceSettings)]
struct ContractSettings;

#[settings(fragment = true, section = "service_config")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct CustomSectionSettings {
	#[setting(required)]
	value: String,
}

#[settings(CustomSectionSettings)]
struct CustomSectionComposition;

#[settings(fragment = true, section = "defaults", default_policy = "required")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DefaultsSettings {
	value: u16,
}

#[settings(defaults: DefaultsSettings)]
#[derive(Default)]
#[serde(default)]
struct StructDefaultSettings;

#[settings(fragment = true, section = "optionalSection")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct OptionalSectionSettings {
	#[serde(default)]
	value: u16,
}

#[settings(optional_section: OptionalSectionSettings)]
#[derive(Default)]
struct MissingOptionalSectionSettings;

fn merged(service: Value) -> IndexMap<String, Value> {
	IndexMap::from([("service".to_string(), service)])
}

fn verify(
	service: Value,
	typed_coercion: bool,
) -> Vec<reinhardt_conf::settings::schema::SettingsViolation> {
	verify_settings_contract(
		&ContractSettings::root_schema(),
		&merged(service),
		typed_coercion,
	)
}

#[test]
fn implicit_custom_fragment_uses_inferred_schema_key() {
	let schema = CustomSectionComposition::root_schema();

	assert_eq!(schema.sections[0].canonical_key, "custom_section");
	assert_eq!(
		schema.sections[0].accepted_keys,
		vec!["custom_section".to_owned()]
	);
	assert!(
		verify_settings_contract(
			&schema,
			&IndexMap::from([("custom_section".to_owned(), json!({"value": "ok"}),)]),
			true,
		)
		.is_empty()
	);
}

#[test]
fn missing_required_uses_canonical_renamed_path() {
	let merged = IndexMap::from([("service".to_string(), json!({}))]);
	let violations = verify_settings_contract(&ContractSettings::root_schema(), &merged, true);

	assert_eq!(violations.len(), 1);
	assert_eq!(violations[0].kind, SettingsViolationKind::MissingRequired);
	assert_eq!(violations[0].path.to_string(), "service.portNumber");
	assert_eq!(
		violations[0].path.segments(),
		&[
			SettingsPathSegment::Key("service"),
			SettingsPathSegment::Key("portNumber"),
		]
	);
}

#[test]
fn map_scalar_reports_a_value_free_shape_mismatch() {
	let violations = verify(json!({ "portNumber": 443, "portsById": "not-a-map" }), true);

	assert_eq!(violations.len(), 1);
	assert_eq!(violations[0].kind, SettingsViolationKind::TypeMismatch);
	assert_eq!(violations[0].path.to_string(), "service.portsById");
	assert_eq!(violations[0].expected, "map");
	assert_eq!(violations[0].actual, Some(JsonKind::String));
}

#[test]
fn numeric_map_keys_follow_serde_map_key_deserialization() {
	let violations = verify(
		json!({ "portNumber": 443, "portsById": { "443": [443] } }),
		false,
	);
	assert!(violations.is_empty());
}

#[test]
fn struct_level_serde_default_makes_the_root_section_optional() {
	let violations = verify_settings_contract(
		&StructDefaultSettings::root_schema(),
		&IndexMap::new(),
		true,
	);

	assert!(violations.is_empty());
}

#[test]
fn missing_non_default_root_section_is_required_even_when_children_are_optional() {
	let violations = verify_settings_contract(
		&MissingOptionalSectionSettings::root_schema(),
		&IndexMap::new(),
		true,
	);

	assert_eq!(violations.len(), 1);
	assert_eq!(violations[0].kind, SettingsViolationKind::MissingRequired);
	assert_eq!(violations[0].path.to_string(), "optional_section");
	assert_eq!(violations[0].expected, "section");
	assert_eq!(violations[0].actual, None);
}

#[test]
fn aliases_are_accepted_but_duplicates_do_not_select_a_value() {
	let violations = verify(json!({ "portNumber": 443, "legacyPort": 8443 }), true);

	assert_eq!(violations.len(), 1);
	assert_eq!(violations[0].kind, SettingsViolationKind::DuplicateInput);
	assert_eq!(violations[0].path.to_string(), "service.portNumber");
}

#[test]
fn recursively_reports_node_sequence_map_key_and_leaf_mismatches() {
	let violations = verify(
		json!({
			"portNumber": "invalid",
			"nested": "not-a-map",
			"ports": "not-a-sequence",
			"portsById": { "not-an-id": [443] },
			"secrets": { "customer-private": "not-a-port" },
		}),
		true,
	);

	assert_eq!(violations.len(), 5);
	assert_eq!(violations[0].kind, SettingsViolationKind::TypeMismatch);
	assert_eq!(violations[0].path.to_string(), "service.portNumber");
	assert_eq!(violations[0].actual, Some(JsonKind::String));
	assert_eq!(violations[1].path.to_string(), "service.nested");
	assert_eq!(violations[1].expected, "map");
	assert_eq!(violations[2].path.to_string(), "service.ports");
	assert_eq!(violations[2].expected, "sequence");
	assert_eq!(
		violations[3].kind,
		SettingsViolationKind::MapKeyTypeMismatch
	);
	assert_eq!(violations[3].path.to_string(), "service.portsById.*");
	assert_eq!(violations[4].path.to_string(), "service.secrets.*");
}

#[test]
fn optional_absence_and_null_are_valid() {
	assert!(verify(json!({ "portNumber": 443, "optionalPort": null }), true).is_empty());
	assert!(verify(json!({ "portNumber": 443 }), true).is_empty());
}

#[test]
fn typed_coercion_matches_the_builder_for_json_containers() {
	let input = json!({
		"portNumber": "443",
		"ports": "[\"80\", \"443\"]",
		"portsById": "{\"443\": [\"80\"]}",
	});

	let typed_violations = verify(input.clone(), true);
	assert!(typed_violations.is_empty());
	let violations = verify(input, false);
	assert_eq!(violations.len(), 3);
	assert_eq!(violations[0].path.to_string(), "service.portNumber");
	assert_eq!(violations[1].path.to_string(), "service.ports");
	assert_eq!(violations[2].path.to_string(), "service.portsById");
}

#[test]
fn custom_whole_field_deserializer_stops_generic_traversal() {
	let input = json!({ "portNumber": 443, "opaquePorts": "not-a-json-array" });
	let violations = verify(input.clone(), true);
	let settings: ServiceSettings = serde_json::from_value(input).expect("custom field input");
	let opaque_ports = settings.opaque_ports;

	assert!(violations.is_empty());
	assert_eq!(opaque_ports, vec![443]);
}

#[test]
fn finding_rendering_never_contains_dynamic_keys_or_values() {
	let sentinel = "postgresql://operator:secret@db.example/private";
	let violations = verify(
		json!({
			"portNumber": 443,
			"secrets": { "production-primary": sentinel },
		}),
		true,
	);

	assert_eq!(violations.len(), 1);
	assert_eq!(violations[0].path.to_string(), "service.secrets.*");
	for violation in violations {
		assert!(!format!("{violation:?}").contains(sentinel));
		assert!(!violation.to_string().contains(sentinel));
		assert!(!format!("{violation:?}").contains("production-primary"));
		assert!(!violation.to_string().contains("production-primary"));
	}
}
