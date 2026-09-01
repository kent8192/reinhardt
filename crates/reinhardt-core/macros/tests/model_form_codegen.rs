// The model derive emits `cfg(wasm)` guards for target-neutral generated APIs.
// This standalone integration-test crate intentionally accepts that known cfg.
#[allow(unexpected_cfgs)]
use chrono::{DateTime, NaiveDate, Utc};
use reinhardt_macros::model;
use rstest::rstest;
use serde::{Deserialize, Serialize};

include!("ui/model/support.rs");

use model_form::{
	AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormPolicy,
	ModelFormPrimaryKeyFields, ModelFormSchema, NativeModelFormPayload,
};

#[model(app_label = "forms", form = true)]
#[derive(Clone, Deserialize, Serialize)]
#[form(validate = validate_form_document)]
struct FormDocument {
	#[field(primary_key = true)]
	id: i64,
	#[field(min_length = 3, max_length = 200)]
	#[form(trim)]
	title: String,
	#[field(max_length = 64)]
	secret: String,
	#[field(max_length = 64, blank = true)]
	nullable: Option<Option<String>>,
}

fn validate_form_document<P: ModelFormPolicy>(
	_payload: &CleanedFormDocumentModelFormData<P>,
) -> Result<(), validators::ValidationErrors> {
	Ok(())
}

#[model(app_label = "forms")]
#[derive(Clone, Deserialize, Serialize)]
struct StringKeyTarget {
	#[field(primary_key = true, max_length = 64)]
	id: String,
}

#[model(app_label = "forms", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct StringKeyChild {
	#[field(primary_key = true)]
	id: i64,
	#[rel(foreign_key, null = true)]
	target: db::associations::ForeignKeyField<StringKeyTarget>,
}

#[model(app_label = "forms", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct TemporalDocument {
	#[field(primary_key = true)]
	id: i64,
	aware_at: DateTime<Utc>,
	naive_at: chrono::NaiveDateTime,
}

#[model(app_label = "forms", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct AssignedKeyDocument {
	#[field(primary_key = true, editable = true, max_length = 64)]
	id: String,
	#[field(max_length = 200)]
	title: String,
}

#[model(app_label = "forms", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct BooleanDocument {
	#[field(primary_key = true)]
	id: i64,
	published: bool,
}

struct TitleOnly;

impl ModelFormPolicy for TitleOnly {
	fn allows(field: &str) -> bool {
		matches!(field, "title" | "nullable")
	}
}

struct PublishedOnly;

impl ModelFormPolicy for PublishedOnly {
	fn allows(field: &str) -> bool {
		field == "published"
	}
}

#[test]
fn native_form_payload_defaults_an_omitted_boolean_without_changing_json_deserialization() {
	let native = <BooleanDocumentModelFormData<PublishedOnly> as NativeModelFormPayload>::from_native_form_value(
		serde_json::json!({}),
	)
	.expect("native form payload should decode");
	assert_eq!(native.published(), Some(&false));

	let json: BooleanDocumentModelFormData<PublishedOnly> =
		serde_json::from_value(serde_json::json!({})).expect("JSON payload should decode");
	assert_eq!(json.published(), None);
}

#[test]
fn generated_payload_applies_policy_and_preserves_nullable_values() {
	let mut payload = FormDocumentModelFormData::<TitleOnly>::empty();
	payload
		.set_title("published".to_owned())
		.expect("allowed fields should use the policy-checked setter");
	assert!(matches!(
		payload.set_secret("browser-input".to_owned()),
		Err(model_form::ModelFormPayloadError::ForbiddenField { .. })
	));
	payload.set_trusted_secret("server-owned".to_owned());
	assert_eq!(
		payload.get_json("secret"),
		Some(serde_json::json!("server-owned"))
	);

	let encoded = serde_json::to_value(&payload).expect("serialize payload");
	assert_eq!(encoded, serde_json::json!({ "title": "published" }));

	let decoded: FormDocumentModelFormData<TitleOnly> = serde_json::from_value(serde_json::json!({
		"title": "decoded",
		"secret": { "ignored": true },
		"nullable": null,
	}))
	.expect("deserialize known fields");
	assert_eq!(decoded.title(), Some(&"decoded".to_owned()));
	assert_eq!(decoded.nullable(), Some(&None));
	assert_eq!(decoded.forbidden_fields(), ["secret"]);

	let error = match serde_json::from_value::<FormDocumentModelFormData<TitleOnly>>(
		serde_json::json!({ "unexpected": true }),
	) {
		Ok(_) => panic!("unknown fields must be rejected"),
		Err(error) => error,
	};
	assert!(error.to_string().contains("unexpected"));
}

#[rstest]
fn generated_payload_is_cloneable_and_exposes_an_opaque_cleaned_type() {
	fn assert_clone<T: Clone>() {}

	assert_clone::<FormDocumentModelFormData<TitleOnly>>();
	let _: Option<CleanedFormDocumentModelFormData<TitleOnly>> = None;

	let mut payload = FormDocumentModelFormData::<TitleOnly>::empty();
	payload
		.set_title("snapshot".to_owned())
		.expect("allowed field should be set");
	let snapshot = payload.clone();
	assert_eq!(snapshot.title(), Some(&"snapshot".to_owned()));
}

#[test]
fn generated_schema_exposes_descriptors_and_target_primary_key_kinds() {
	assert_eq!(
		FormDocumentFormSchema::title(),
		&model_form::ModelFormFieldDescriptor {
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
			trim: true,
		}
	);
	assert!(!FormDocumentFormSchema::secret().trim);
	assert!(FormDocumentFormSchema::nullable().nullable);
	assert_eq!(
		StringKeyChildFormSchema::target_id().kind,
		ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(64),
			multiline: false,
		}
	);
	assert!(StringKeyChildFormSchema::target_id().nullable);
	assert!(
		<StringKeyChildFormSchema as ModelFormSchema>::relation_target_matches::<StringKeyTarget>(
			"target_id"
		)
	);
	assert!(
		!<StringKeyChildFormSchema as ModelFormSchema>::relation_target_matches::<FormDocument>(
			"target_id"
		)
	);
	assert_eq!(FormDocument::primary_key_fields(), ["id"]);

	let mut child_payload = StringKeyChildModelFormData::<AllEditableModelFields>::empty();
	child_payload
		.set_json("target_id", serde_json::json!(null))
		.expect("nullable relationship identifiers should accept an explicit clear");
	assert_eq!(child_payload.target_id(), Some(&None));

	let mut payload = FormDocumentModelFormData::<AllEditableModelFields>::empty();
	payload
		.set_json("title", serde_json::json!("updated"))
		.expect("known editable field");
	assert_eq!(payload.supplied_fields(), ["title"]);
}

#[test]
fn generated_datetime_schema_and_payload_distinguish_aware_from_naive_values() {
	assert_eq!(
		TemporalDocumentFormSchema::aware_at().kind,
		ModelFormFieldKind::DateTime
	);
	assert_eq!(
		TemporalDocumentFormSchema::naive_at().kind,
		ModelFormFieldKind::NaiveDateTime
	);

	let mut payload = TemporalDocumentModelFormData::<AllEditableModelFields>::empty();
	payload
		.set_json("aware_at", serde_json::json!("2026-07-25T14:30:00Z"))
		.expect("UTC datetime should deserialize into generated payload");
	payload
		.set_json("naive_at", serde_json::json!("2026-07-25T14:30:00"))
		.expect("naive datetime should deserialize into generated payload");

	assert_eq!(
		payload.aware_at(),
		Some(&DateTime::from_naive_utc_and_offset(
			NaiveDate::from_ymd_opt(2026, 7, 25)
				.expect("valid date")
				.and_hms_opt(14, 30, 0)
				.expect("valid time"),
			Utc,
		))
	);
	assert_eq!(
		payload.naive_at(),
		Some(
			&NaiveDate::from_ymd_opt(2026, 7, 25)
				.expect("valid date")
				.and_hms_opt(14, 30, 0)
				.expect("valid time")
		)
	);
}

#[test]
fn generated_model_forms_include_editable_assigned_primary_keys() {
	assert_eq!(
		AssignedKeyDocumentFormSchema::id().kind,
		ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(64),
			multiline: false,
		}
	);
	assert!(AssignedKeyDocumentFormSchema::id().required);

	let mut payload = AssignedKeyDocumentModelFormData::<AllEditableModelFields>::empty();
	payload
		.set_id("external-key".to_owned())
		.expect("editable assigned keys should use the policy-checked setter");
	assert_eq!(payload.id(), Some(&"external-key".to_owned()));
	assert_eq!(payload.supplied_fields(), ["id"]);
}
