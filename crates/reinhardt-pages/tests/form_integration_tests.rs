#![cfg(not(target_arch = "wasm32"))]
//! Form Component Integration Tests
//!
//! Tests for the FormComponent system's rendering, validation,
//! and value management capabilities.
//!
//! Success Criteria:
//! 1. Form creation from metadata works correctly
//! 2. Field value management (get/set) functions properly
//! 3. Validation rules are enforced correctly
//! 4. Multiple widget types are supported
//! 5. Initial values and CSRF tokens are handled correctly
//!
//! Test Categories:
//! - Category 1: Form Creation and Metadata (8 tests)
//! - Category 2: Field Value Management (8 tests)
//! - Category 3: Validation (12 tests)
//! - Category 4: Widget Types (7 tests)
//!
//! Total: 35 tests
//!
//! Note: DOM rendering tests require WASM environment with WASM test infrastructure.

use reinhardt_pages::{FieldMetadata, FormComponent, FormMetadata, Widget};
use std::collections::HashMap;

use reinhardt_core::model_form::{
	ModelFormCleanedPayload, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload,
	ModelFormPayloadError, ModelFormPolicy, ModelFormSchema, ModelFormValidatingPayload,
};
use reinhardt_core::validators::{ValidationError, ValidationErrors};
use reinhardt_pages::form::ModelFormState;
use rstest::rstest;

struct Cluster;

struct ClusterSchema;

const CLUSTER_FIELDS: [ModelFormFieldDescriptor; 2] = [
	ModelFormFieldDescriptor {
		name: "name",
		kind: ModelFormFieldKind::Text {
			min_length: Some(3),
			max_length: Some(63),
			multiline: false,
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: true,
	},
	ModelFormFieldDescriptor {
		name: "api_url",
		kind: ModelFormFieldKind::Url {
			min_length: None,
			max_length: Some(2048),
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: true,
	},
];

impl ModelFormSchema for ClusterSchema {
	type Model = Cluster;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&CLUSTER_FIELDS
	}
}

struct ClusterPolicy;

impl ModelFormPolicy for ClusterPolicy {
	fn allows(field: &str) -> bool {
		matches!(field, "name" | "api_url")
	}
}

#[derive(Debug, Default)]
struct ClusterPayload {
	name: Option<String>,
	api_url: Option<String>,
}

impl ClusterPayload {
	fn name(&self) -> Option<&String> {
		self.name.as_ref()
	}

	fn api_url(&self) -> Option<&String> {
		self.api_url.as_ref()
	}
}

impl ModelFormPayload<ClusterPolicy> for ClusterPayload {
	fn supplied_fields(&self) -> Vec<&'static str> {
		CLUSTER_FIELDS
			.iter()
			.filter(|descriptor| self.get_json(descriptor.name).is_some())
			.map(|descriptor| descriptor.name)
			.collect()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		match field {
			"name" => self.name.clone().map(serde_json::Value::String),
			"api_url" => self.api_url.clone().map(serde_json::Value::String),
			_ => None,
		}
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		let value =
			serde_json::from_value(value).map_err(|error| ModelFormPayloadError::InvalidValue {
				field: field.to_owned(),
				message: error.to_string(),
			})?;
		match field {
			"name" => self.name = Some(value),
			"api_url" => self.api_url = Some(value),
			_ => {
				return Err(ModelFormPayloadError::UnknownField {
					field: field.to_owned(),
				});
			}
		}
		Ok(())
	}
}

struct CleanedClusterPayload(ClusterPayload);

impl ModelFormCleanedPayload for CleanedClusterPayload {
	type Raw = ClusterPayload;

	fn into_raw(self) -> Self::Raw {
		self.0
	}
}

impl ModelFormValidatingPayload for ClusterPayload {
	type Cleaned = CleanedClusterPayload;

	fn clean_and_validate(self) -> Result<Self::Cleaned, ValidationErrors> {
		if self.name == self.api_url {
			let mut errors = ValidationErrors::new();
			errors.add(
				"_all",
				ValidationError::Custom("Name and API URL must differ".to_owned()),
			);
			return Err(errors);
		}
		Ok(CleanedClusterPayload(self))
	}
}

#[rstest]
fn model_form_validated_snapshot_normalizes_without_mutating_raw_controls() {
	// Arrange
	let mut state = ModelFormState::<ClusterSchema, ClusterPolicy>::new();
	state
		.set_value("name", serde_json::json!("  cluster  "))
		.expect("raw name control should be accepted");
	state
		.set_value("api_url", serde_json::json!("  https://example.com/api  "))
		.expect("raw URL control should be accepted");

	// Act
	let payload = state
		.build_validated_payload::<ClusterPayload>()
		.expect("snapshot should validate");

	// Assert
	assert_eq!(payload.name(), Some(&"cluster".to_owned()));
	assert_eq!(
		payload.api_url(),
		Some(&"https://example.com/api".to_owned())
	);
	assert_eq!(state.value("name"), Some(&serde_json::json!("  cluster  ")));
}

#[rstest]
fn model_form_validated_snapshot_preserves_invalid_url_for_correction() {
	// Arrange
	let mut state = ModelFormState::<ClusterSchema, ClusterPolicy>::new();
	state
		.set_value("name", serde_json::json!("cluster"))
		.expect("name control should be accepted");
	state
		.set_value("api_url", serde_json::json!("not a URL"))
		.expect("raw invalid URL should remain editable");

	// Act
	let errors = state
		.build_validated_payload::<ClusterPayload>()
		.expect_err("invalid URL should reject the submission snapshot");

	// Assert
	let ordered = errors.ordered_field_errors().collect::<Vec<_>>();
	assert_eq!(
		ordered,
		vec![(
			"api_url",
			&[ValidationError::Custom("Enter a valid URL".to_owned())][..],
		)]
	);
	assert_eq!(
		state.value("api_url"),
		Some(&serde_json::json!("not a URL"))
	);
}

#[rstest]
fn model_form_validated_snapshot_runs_cross_field_validation_after_normalization() {
	// Arrange
	let mut state = ModelFormState::<ClusterSchema, ClusterPolicy>::new();
	state
		.set_value("name", serde_json::json!("  https://example.com/api  "))
		.expect("raw name control should be accepted");
	state
		.set_value("api_url", serde_json::json!("https://example.com/api"))
		.expect("URL control should be accepted");

	// Act
	let errors = state
		.build_validated_payload::<ClusterPayload>()
		.expect_err("equal normalized values should fail cross-field validation");

	// Assert
	let fields = errors
		.ordered_field_errors()
		.map(|(field, _)| field)
		.collect::<Vec<_>>();
	assert_eq!(fields, ["_all"]);
	assert_eq!(
		state.value("name"),
		Some(&serde_json::json!("  https://example.com/api  "))
	);
}

struct ModelFormQuestion;

struct ModelFormQuestionSchema;

const MODEL_FORM_QUESTION_FIELDS: [ModelFormFieldDescriptor; 2] = [
	ModelFormFieldDescriptor {
		name: "title",
		kind: ModelFormFieldKind::Text {
			min_length: Some(3),
			max_length: Some(200),
			multiline: false,
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "owner_id",
		kind: ModelFormFieldKind::Integer {
			min: Some(1),
			max: None,
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: true,
		trim: false,
	},
];

impl ModelFormSchema for ModelFormQuestionSchema {
	type Model = ModelFormQuestion;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&MODEL_FORM_QUESTION_FIELDS
	}
}

struct ModelFormTitleOnly;

impl ModelFormPolicy for ModelFormTitleOnly {
	fn allows(field: &str) -> bool {
		field == "title"
	}
}

#[derive(Default)]
struct ModelFormQuestionData {
	title: Option<String>,
}

impl ModelFormPayload<ModelFormTitleOnly> for ModelFormQuestionData {
	fn supplied_fields(&self) -> Vec<&'static str> {
		if self.title.is_some() {
			vec!["title"]
		} else {
			Vec::new()
		}
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		match field {
			"title" => self.title.clone().map(serde_json::Value::String),
			_ => None,
		}
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		if !ModelFormTitleOnly::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		match field {
			"title" => {
				self.title = serde_json::from_value(value).map_err(|error| {
					ModelFormPayloadError::InvalidValue {
						field: field.to_owned(),
						message: error.to_string(),
					}
				})?;
				Ok(())
			}
			_ => Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			}),
		}
	}
}

#[test]
fn model_form_builds_one_policy_safe_payload() {
	let mut state = ModelFormState::<ModelFormQuestionSchema, ModelFormTitleOnly>::new();
	state
		.set_value("title", serde_json::json!("Typed"))
		.expect("selected title should be accepted");

	let owner_error = state
		.set_value("owner_id", serde_json::json!("42"))
		.expect_err("excluded owner identifier must be forbidden");
	assert_eq!(
		owner_error,
		ModelFormPayloadError::ForbiddenField {
			field: "owner_id".to_owned(),
		}
	);

	let payload = state
		.build_payload::<ModelFormQuestionData>()
		.expect("selected control values should build one payload");
	assert_eq!(payload.supplied_fields(), ["title"]);
	assert_eq!(payload.get_json("title"), Some(serde_json::json!("Typed")));
	assert_eq!(payload.get_json("owner_id"), None);
}

#[rstest]
fn model_form_typed_setter_rejects_a_wrong_supported_primitive_immediately() {
	// Arrange
	let mut state = ModelFormState::<ModelFormQuestionSchema, ModelFormTitleOnly>::new();

	// Act
	let error = state
		.set_any_value("title", 42_i64)
		.expect_err("an integer must not be stored for a text descriptor");

	// Assert
	assert_eq!(
		error,
		ModelFormPayloadError::InvalidValue {
			field: "title".to_owned(),
			message: "expected a string".to_owned(),
		}
	);
	assert_eq!(state.value("title"), None);
}

#[rstest]
fn model_form_payload_conversion_enforces_the_descriptor_minimum_length() {
	// Arrange
	let mut state = ModelFormState::<ModelFormQuestionSchema, ModelFormTitleOnly>::new();
	state
		.set_value("title", serde_json::json!("no"))
		.expect("raw short text should remain editable");

	// Act & Assert
	assert!(matches!(
		state.build_payload::<ModelFormQuestionData>(),
		Err(ModelFormPayloadError::InvalidValue { .. })
	));
	assert_eq!(state.value("title"), Some(&serde_json::json!("no")));
	state
		.set_value("title", serde_json::json!("valid"))
		.expect("valid raw text should be accepted");
	state
		.build_payload::<ModelFormQuestionData>()
		.expect("text at the inclusive minimum length should build a payload");
	assert_eq!(state.value("title"), Some(&serde_json::json!("valid")));
	state
		.set_value("title", serde_json::json!("  valid  "))
		.expect("untrimmed raw text should be accepted");
	let payload = state
		.build_payload::<ModelFormQuestionData>()
		.expect("text without declared trimming should build a payload");
	assert_eq!(
		payload.get_json("title"),
		Some(serde_json::json!("  valid  "))
	);
}

struct ModelFormNumeric;

struct ModelFormNumericSchema;

const MODEL_FORM_NUMERIC_FIELDS: [ModelFormFieldDescriptor; 3] = [
	ModelFormFieldDescriptor {
		name: "bounded",
		kind: ModelFormFieldKind::Integer {
			min: Some(-2),
			max: Some(2),
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "unsigned",
		kind: ModelFormFieldKind::Integer {
			min: Some(0),
			max: None,
		},
		required: false,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "ratio",
		kind: ModelFormFieldKind::Float {
			min: Some(1.5),
			max: Some(2.5),
		},
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
];

impl ModelFormSchema for ModelFormNumericSchema {
	type Model = ModelFormNumeric;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&MODEL_FORM_NUMERIC_FIELDS
	}
}

struct ModelFormAllNumericFields;

impl ModelFormPolicy for ModelFormAllNumericFields {
	fn allows(field: &str) -> bool {
		matches!(field, "bounded" | "unsigned" | "ratio")
	}
}

#[derive(Default)]
struct ModelFormNumericData {
	values: HashMap<&'static str, serde_json::Value>,
}

impl ModelFormPayload<ModelFormAllNumericFields> for ModelFormNumericData {
	fn supplied_fields(&self) -> Vec<&'static str> {
		self.values.keys().copied().collect()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		self.values.get(field).cloned()
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		let field = match field {
			"bounded" => "bounded",
			"unsigned" => "unsigned",
			"ratio" => "ratio",
			_ => {
				return Err(ModelFormPayloadError::UnknownField {
					field: field.to_owned(),
				});
			}
		};
		self.values.insert(field, value);
		Ok(())
	}
}

#[rstest]
fn model_form_integer_conversion_preserves_signed_and_unsigned_boundaries() {
	// Arrange
	let mut state = ModelFormState::<ModelFormNumericSchema, ModelFormAllNumericFields>::new();

	// Act & Assert
	state
		.set_value("bounded", serde_json::json!(-2))
		.expect("raw signed minimum should be accepted");
	state
		.build_payload::<ModelFormNumericData>()
		.expect("inclusive signed minimum should build a payload");
	assert_eq!(state.value("bounded"), Some(&serde_json::json!(-2)));
	state
		.set_value("bounded", serde_json::json!(2))
		.expect("raw signed maximum should be accepted");
	state
		.build_payload::<ModelFormNumericData>()
		.expect("inclusive signed maximum should build a payload");
	assert_eq!(state.value("bounded"), Some(&serde_json::json!(2)));
	state
		.set_value("bounded", serde_json::json!(-3))
		.expect("raw out-of-range input should remain editable");
	assert_eq!(
		state.value("bounded"),
		Some(&serde_json::json!(-3)),
		"an invalid edit must remain available for correction"
	);
	assert!(matches!(
		state.build_payload::<ModelFormNumericData>(),
		Err(ModelFormPayloadError::InvalidValue { .. })
	));
	state
		.set_value("bounded", serde_json::json!(3))
		.expect("raw out-of-range input should remain editable");
	assert!(matches!(
		state.build_payload::<ModelFormNumericData>(),
		Err(ModelFormPayloadError::InvalidValue { .. })
	));
	state
		.set_value("bounded", serde_json::json!(u64::MAX))
		.expect("raw unsigned input should remain editable");
	assert!(matches!(
		state.build_payload::<ModelFormNumericData>(),
		Err(ModelFormPayloadError::InvalidValue { .. })
	));
	state
		.set_value("bounded", serde_json::json!(0))
		.expect("valid signed input should replace the rejected snapshot");

	let above_i64_max = i64::MAX as u64 + 1;
	state
		.set_value("unsigned", serde_json::json!(above_i64_max))
		.expect("an unsigned JSON integer above i64::MAX should be accepted");
	assert_eq!(
		state.value("unsigned"),
		Some(&serde_json::json!(above_i64_max))
	);
	state
		.set_value("unsigned", serde_json::json!(u64::MAX.to_string()))
		.expect("an unsigned integer string above i64::MAX should be accepted");
	let payload = state
		.build_payload::<ModelFormNumericData>()
		.expect("signed and unsigned integer values should build one payload");
	assert_eq!(
		payload.get_json("unsigned"),
		Some(serde_json::json!(u64::MAX))
	);
}

#[rstest]
fn model_form_float_conversion_enforces_descriptor_bounds() {
	// Arrange
	let mut state = ModelFormState::<ModelFormNumericSchema, ModelFormAllNumericFields>::new();

	// Act & Assert
	state
		.set_value("ratio", serde_json::json!(1.4))
		.expect("raw out-of-range float should remain editable");
	assert!(matches!(
		state.build_payload::<ModelFormNumericData>(),
		Err(ModelFormPayloadError::InvalidValue { .. })
	));
	state
		.set_value("ratio", serde_json::json!(2.5))
		.expect("raw float maximum should be accepted");
	state
		.build_payload::<ModelFormNumericData>()
		.expect("the inclusive float maximum should build a payload");
	state
		.set_value("ratio", serde_json::json!(2.6))
		.expect("raw out-of-range float should remain editable");
	assert!(matches!(
		state.build_payload::<ModelFormNumericData>(),
		Err(ModelFormPayloadError::InvalidValue { .. })
	));
}

#[rstest]
fn model_form_clearing_optional_control_removes_previous_payload_value() {
	// Arrange
	let mut state = ModelFormState::<ModelFormNumericSchema, ModelFormAllNumericFields>::new();
	state
		.set_value("unsigned", serde_json::json!(42_u64))
		.expect("optional integer should accept a value");
	state
		.set_value("unsigned", serde_json::json!(""))
		.expect("empty optional input should unset the control");

	// Act
	let payload = state
		.build_payload::<ModelFormNumericData>()
		.expect("cleared optional input should build a payload");

	// Assert
	assert_eq!(state.value("unsigned"), Some(&serde_json::json!("")));
	assert_eq!(payload.get_json("unsigned"), None);
	assert!(payload.supplied_fields().is_empty());
}

struct ModelFormEmptyValues;

struct ModelFormEmptyValuesSchema;

const MODEL_FORM_EMPTY_VALUE_FIELDS: [ModelFormFieldDescriptor; 3] = [
	ModelFormFieldDescriptor {
		name: "nullable_note",
		kind: ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(200),
			multiline: false,
		},
		required: false,
		has_default: false,
		nullable: true,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "defaulted_label",
		kind: ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(200),
			multiline: false,
		},
		required: false,
		has_default: true,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "blank_label",
		kind: ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(200),
			multiline: false,
		},
		required: false,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
];

impl ModelFormSchema for ModelFormEmptyValuesSchema {
	type Model = ModelFormEmptyValues;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&MODEL_FORM_EMPTY_VALUE_FIELDS
	}
}

struct ModelFormAllEmptyValueFields;

impl ModelFormPolicy for ModelFormAllEmptyValueFields {
	fn allows(field: &str) -> bool {
		matches!(field, "nullable_note" | "defaulted_label" | "blank_label")
	}
}

#[derive(Default)]
struct ModelFormEmptyValueData {
	values: HashMap<&'static str, serde_json::Value>,
}

impl ModelFormPayload<ModelFormAllEmptyValueFields> for ModelFormEmptyValueData {
	fn supplied_fields(&self) -> Vec<&'static str> {
		MODEL_FORM_EMPTY_VALUE_FIELDS
			.iter()
			.filter(|descriptor| self.values.contains_key(descriptor.name))
			.map(|descriptor| descriptor.name)
			.collect()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		self.values.get(field).cloned()
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		let field = MODEL_FORM_EMPTY_VALUE_FIELDS
			.iter()
			.find(|descriptor| descriptor.name == field)
			.map(|descriptor| descriptor.name)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		self.values.insert(field, value);
		Ok(())
	}
}

#[rstest]
fn model_form_empty_nullable_control_clears_while_other_optional_inputs_are_absent() {
	// Arrange
	let mut state =
		ModelFormState::<ModelFormEmptyValuesSchema, ModelFormAllEmptyValueFields>::new();
	for field in ["nullable_note", "defaulted_label", "blank_label"] {
		state
			.set_value(field, serde_json::json!("present"))
			.expect("initial text value should be accepted");
		state
			.set_value(field, serde_json::json!(""))
			.expect("empty optional control should be accepted");
	}

	// Act
	let payload = state
		.build_payload::<ModelFormEmptyValueData>()
		.expect("empty controls should assemble a typed payload");

	// Assert
	assert_eq!(state.value("nullable_note"), Some(&serde_json::json!("")));
	assert_eq!(state.value("defaulted_label"), Some(&serde_json::json!("")));
	assert_eq!(
		state.value("blank_label"),
		Some(&serde_json::Value::String(String::new()))
	);
	assert_eq!(payload.supplied_fields(), ["nullable_note", "blank_label"]);
	assert_eq!(
		payload.get_json("nullable_note"),
		Some(serde_json::Value::Null)
	);
	assert_eq!(
		payload.get_json("blank_label"),
		Some(serde_json::Value::String(String::new()))
	);
}

struct ModelFormDateTimes;

struct ModelFormDateTimesSchema;

const MODEL_FORM_DATETIME_FIELDS: [ModelFormFieldDescriptor; 2] = [
	ModelFormFieldDescriptor {
		name: "aware_at",
		kind: ModelFormFieldKind::DateTime,
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
	ModelFormFieldDescriptor {
		name: "naive_at",
		kind: ModelFormFieldKind::NaiveDateTime,
		required: true,
		has_default: false,
		nullable: false,
		editable: true,
		generated_relation_id: false,
		trim: false,
	},
];

impl ModelFormSchema for ModelFormDateTimesSchema {
	type Model = ModelFormDateTimes;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&MODEL_FORM_DATETIME_FIELDS
	}
}

struct ModelFormAllDateTimeFields;

impl ModelFormPolicy for ModelFormAllDateTimeFields {
	fn allows(field: &str) -> bool {
		matches!(field, "aware_at" | "naive_at")
	}
}

#[derive(Default)]
struct ModelFormDateTimeData {
	values: HashMap<&'static str, serde_json::Value>,
}

impl ModelFormPayload<ModelFormAllDateTimeFields> for ModelFormDateTimeData {
	fn supplied_fields(&self) -> Vec<&'static str> {
		MODEL_FORM_DATETIME_FIELDS
			.iter()
			.filter(|descriptor| self.values.contains_key(descriptor.name))
			.map(|descriptor| descriptor.name)
			.collect()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		self.values.get(field).cloned()
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		let field = MODEL_FORM_DATETIME_FIELDS
			.iter()
			.find(|descriptor| descriptor.name == field)
			.map(|descriptor| descriptor.name)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		self.values.insert(field, value);
		Ok(())
	}
}

#[rstest]
fn model_form_datetime_local_values_normalize_for_aware_and_naive_payloads() {
	// Arrange
	let mut state = ModelFormState::<ModelFormDateTimesSchema, ModelFormAllDateTimeFields>::new();
	state
		.set_value("aware_at", serde_json::json!("2026-07-25T14:30"))
		.expect("browser local datetime should map to the documented UTC convention");
	state
		.set_value("naive_at", serde_json::json!("2026-07-25T14:30"))
		.expect("browser local datetime should map to a naive ISO value");

	// Act
	let payload = state
		.build_payload::<ModelFormDateTimeData>()
		.expect("normalized datetimes should build a payload");

	// Assert
	assert_eq!(
		state.value("aware_at"),
		Some(&serde_json::json!("2026-07-25T14:30"))
	);
	assert_eq!(
		state.value("naive_at"),
		Some(&serde_json::json!("2026-07-25T14:30"))
	);
	assert_eq!(
		payload.get_json("aware_at"),
		Some(serde_json::json!("2026-07-25T14:30:00Z"))
	);
	assert_eq!(
		payload.get_json("naive_at"),
		Some(serde_json::json!("2026-07-25T14:30:00"))
	);
}

// ============================================================================
// Category 1: Form Creation and Metadata (8 tests)
// ============================================================================

/// Tests creating FormComponent from minimal metadata
#[test]
fn test_form_creation_minimal() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata.clone(), "/api/submit");
	assert_eq!(component.metadata().fields.len(), 0);
}

/// Tests creating FormComponent with single field
#[test]
fn test_form_creation_single_field() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "username".to_string(),
			label: Some("Username".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().fields.len(), 1);
	assert_eq!(component.metadata().fields[0].name, "username");
}

/// Tests creating FormComponent with multiple fields
#[test]
fn test_form_creation_multiple_fields() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "username".to_string(),
				label: Some("Username".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "email".to_string(),
				label: Some("Email".to_string()),
				required: true,
				help_text: None,
				widget: Widget::EmailInput,
				initial: None,
			},
			FieldMetadata {
				name: "age".to_string(),
				label: Some("Age".to_string()),
				required: false,
				help_text: Some("Optional field".to_string()),
				widget: Widget::NumberInput,
				initial: None,
			},
		],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().fields.len(), 3);
}

/// Tests FormComponent with field prefix
#[test]
fn test_form_creation_with_prefix() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "name".to_string(),
			label: Some("Name".to_string()),
			required: false,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: "user_form".to_string(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().prefix, "user_form");
}

/// Tests FormComponent with help text
#[test]
fn test_form_creation_with_help_text() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "password".to_string(),
			label: Some("Password".to_string()),
			required: true,
			help_text: Some("Must be at least 8 characters".to_string()),
			widget: Widget::PasswordInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(
		component.metadata().fields[0].help_text,
		Some("Must be at least 8 characters".to_string())
	);
}

/// Tests FormComponent with bound state
#[test]
fn test_form_creation_bound_state() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: true,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert!(component.metadata().is_bound);
}

/// Tests FormComponent with server-side errors
#[test]
fn test_form_creation_with_errors() {
	let mut errors = HashMap::new();
	errors.insert(
		"username".to_string(),
		vec!["This username is already taken.".to_string()],
	);

	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "username".to_string(),
			label: Some("Username".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: true,
		errors,
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert!(!component.metadata().errors.is_empty());
	assert!(component.metadata().errors.contains_key("username"));
}

// ============================================================================
// Category 2: Field Value Management (8 tests)
// ============================================================================

/// Tests getting default empty value
#[test]
fn test_get_value_empty_default() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "name".to_string(),
			label: Some("Name".to_string()),
			required: false,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.get_value("name"), "");
}

/// Tests setting and getting field value
#[test]
fn test_set_and_get_value() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "email".to_string(),
			label: Some("Email".to_string()),
			required: false,
			help_text: None,
			widget: Widget::EmailInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	component.set_value("email", "test@example.com");
	assert_eq!(component.get_value("email"), "test@example.com");
}

/// Tests updating field value multiple times
#[test]
fn test_update_value_multiple_times() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "status".to_string(),
			label: Some("Status".to_string()),
			required: false,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	component.set_value("status", "draft");
	assert_eq!(component.get_value("status"), "draft");

	component.set_value("status", "published");
	assert_eq!(component.get_value("status"), "published");

	component.set_value("status", "archived");
	assert_eq!(component.get_value("status"), "archived");
}

/// Tests getting value from non-existent field
#[test]
fn test_get_value_nonexistent_field() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.get_value("nonexistent"), "");
}

/// Tests setting value on non-existent field (should be no-op)
#[test]
fn test_set_value_nonexistent_field() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	component.set_value("nonexistent", "value");
	// Should not panic, just silently ignore
}

/// Tests initial values from metadata
#[test]
fn test_initial_values_from_metadata() {
	let mut initial = HashMap::new();
	initial.insert("username".to_string(), serde_json::json!("john_doe"));
	initial.insert("email".to_string(), serde_json::json!("john@example.com"));

	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "username".to_string(),
				label: Some("Username".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "email".to_string(),
				label: Some("Email".to_string()),
				required: false,
				help_text: None,
				widget: Widget::EmailInput,
				initial: None,
			},
		],
		initial,
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.get_value("username"), "john_doe");
	assert_eq!(component.get_value("email"), "john@example.com");
}

/// Tests field-level initial value
#[test]
fn test_field_level_initial_value() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "country".to_string(),
			label: Some("Country".to_string()),
			required: false,
			help_text: None,
			widget: Widget::TextInput,
			initial: Some(serde_json::json!("USA")),
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.get_value("country"), "USA");
}

/// Tests managing values for multiple fields
#[test]
fn test_multiple_field_values() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "first_name".to_string(),
				label: Some("First Name".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "last_name".to_string(),
				label: Some("Last Name".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "age".to_string(),
				label: Some("Age".to_string()),
				required: false,
				help_text: None,
				widget: Widget::NumberInput,
				initial: None,
			},
		],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	component.set_value("first_name", "John");
	component.set_value("last_name", "Doe");
	component.set_value("age", "30");

	assert_eq!(component.get_value("first_name"), "John");
	assert_eq!(component.get_value("last_name"), "Doe");
	assert_eq!(component.get_value("age"), "30");
}

// ============================================================================
// Category 3: Validation (12 tests)
// ============================================================================

/// Tests validation passes for valid required field
#[test]
fn test_validation_required_field_valid() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "username".to_string(),
			label: Some("Username".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	component.set_value("username", "john_doe");

	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation fails for empty required field
#[test]
fn test_validation_required_field_empty() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "email".to_string(),
			label: Some("Email".to_string()),
			required: true,
			help_text: None,
			widget: Widget::EmailInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	assert!(!component.validate());

	let errors = component.errors();
	assert!(errors.contains_key("email"));
	assert_eq!(errors.get("email").unwrap()[0], "This field is required.");
}

/// Tests validation passes for optional empty field
#[test]
fn test_validation_optional_field_empty() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "bio".to_string(),
			label: Some("Bio".to_string()),
			required: false,
			help_text: None,
			widget: Widget::TextArea,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation with multiple required fields
#[test]
fn test_validation_multiple_required_fields() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "username".to_string(),
				label: Some("Username".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "password".to_string(),
				label: Some("Password".to_string()),
				required: true,
				help_text: None,
				widget: Widget::PasswordInput,
				initial: None,
			},
			FieldMetadata {
				name: "email".to_string(),
				label: Some("Email".to_string()),
				required: true,
				help_text: None,
				widget: Widget::EmailInput,
				initial: None,
			},
		],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	// All fields empty - should fail
	assert!(!component.validate());
	let errors = component.errors();
	assert_eq!(errors.len(), 3);

	// Set one field
	component.set_value("username", "john");
	assert!(!component.validate());

	// Set all fields
	component.set_value("password", "secret123");
	component.set_value("email", "john@example.com");
	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation error message format
#[test]
fn test_validation_error_message() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "title".to_string(),
			label: Some("Title".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	component.validate();

	let errors = component.errors();
	assert_eq!(errors.get("title").unwrap()[0], "This field is required.");
}

/// Tests validation clears previous errors
#[test]
fn test_validation_clears_previous_errors() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "name".to_string(),
			label: Some("Name".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	// First validation - fails
	assert!(!component.validate());
	assert!(!component.errors().is_empty());

	// Set value and validate again - should clear errors
	component.set_value("name", "John");
	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation with whitespace-only value
#[test]
fn test_validation_whitespace_only() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "description".to_string(),
			label: Some("Description".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextArea,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	// Whitespace-only should fail validation
	component.set_value("description", "   ");
	assert!(!component.validate());
}

/// Tests validation is callable multiple times
#[test]
fn test_validation_multiple_calls() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "field".to_string(),
			label: Some("Field".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	assert!(!component.validate());
	assert!(!component.validate());

	component.set_value("field", "value");
	assert!(component.validate());
	assert!(component.validate());
}

/// Tests mixed required and optional fields validation
#[test]
fn test_validation_mixed_required_optional() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "required_field".to_string(),
				label: Some("Required".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "optional_field".to_string(),
				label: Some("Optional".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
		],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	// Only optional filled - should fail
	component.set_value("optional_field", "value");
	assert!(!component.validate());

	// Required filled - should pass
	component.set_value("required_field", "required_value");
	assert!(component.validate());
}

/// Tests validation with no fields
#[test]
fn test_validation_empty_form() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert!(component.validate());
}

/// Tests validation state persists until re-validated
#[test]
fn test_validation_state_persistence() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "name".to_string(),
			label: Some("Name".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextInput,
			initial: None,
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");

	// Validate - fails
	component.validate();
	assert!(!component.errors().is_empty());

	// Set value but don't validate yet
	component.set_value("name", "John");
	// Errors should still be present
	assert!(!component.errors().is_empty());

	// Re-validate
	component.validate();
	// Now errors should be cleared
	assert!(component.errors().is_empty());
}

// ============================================================================
// Category 4: Widget Types (7 tests)
// ============================================================================

/// Tests form with multiple widget types
#[test]
fn test_multiple_widget_types() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "username".to_string(),
				label: Some("Username".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "email".to_string(),
				label: Some("Email".to_string()),
				required: true,
				help_text: None,
				widget: Widget::EmailInput,
				initial: None,
			},
			FieldMetadata {
				name: "password".to_string(),
				label: Some("Password".to_string()),
				required: true,
				help_text: None,
				widget: Widget::PasswordInput,
				initial: None,
			},
			FieldMetadata {
				name: "age".to_string(),
				label: Some("Age".to_string()),
				required: false,
				help_text: None,
				widget: Widget::NumberInput,
				initial: None,
			},
			FieldMetadata {
				name: "bio".to_string(),
				label: Some("Bio".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextArea,
				initial: None,
			},
		],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().fields.len(), 5);

	// Verify field names
	assert_eq!(component.metadata().fields[0].name, "username");
	assert_eq!(component.metadata().fields[1].name, "email");
	assert_eq!(component.metadata().fields[2].name, "password");
	assert_eq!(component.metadata().fields[3].name, "age");
	assert_eq!(component.metadata().fields[4].name, "bio");
}
