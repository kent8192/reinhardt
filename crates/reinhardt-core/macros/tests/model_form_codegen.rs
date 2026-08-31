// The model derive emits `cfg(wasm)` guards for target-neutral generated APIs.
// This standalone integration-test crate intentionally accepts that known cfg.
#[allow(unexpected_cfgs)]
use chrono::{DateTime, NaiveDate, Utc};
use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("ui/model/support.rs");

mod rust_decimal {
	pub(crate) use crate::db::orm::Decimal;
}

use model_form::{
	AllEditableModelFields, ModelFormContract, ModelFormContractField, ModelFormContractSchema,
	ModelFormFieldKind, ModelFormPayload, ModelFormPolicy, ModelFormPrimaryKeyFields,
	ModelFormSchema, NativeModelFormPayload,
};

#[model(app_label = "forms", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct FormDocument {
	#[field(primary_key = true)]
	id: i64,
	#[field(min_length = 3, max_length = 200)]
	title: String,
	#[field(max_length = 64)]
	secret: String,
	#[field(max_length = 64, blank = true)]
	nullable: Option<Option<String>>,
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

#[model(app_label = "forms")]
#[derive(Clone, Deserialize, Serialize)]
struct ClusterOrganization {
	#[field(primary_key = true)]
	id: i64,
}

#[model(
	app_label = "forms",
	form(name = ClusterCreateForm, fields(name, api_url))
)]
#[derive(Clone, Deserialize, Serialize)]
struct Cluster {
	#[field(primary_key = true)]
	id: i64,
	#[field(editable = false)]
	#[rel(foreign_key)]
	organization: db::associations::ForeignKeyField<ClusterOrganization>,
	#[field(min_length = 1, max_length = 63)]
	name: String,
	#[field(url = true, max_length = 2048, null = true)]
	api_url: Option<String>,
	#[field(max_length = 64)]
	secret: String,
}

#[model(
	app_label = "forms",
	form(name = BooleanCreateForm, fields(published))
)]
#[derive(Clone, Deserialize, Serialize)]
struct NamedBooleanDocument {
	#[field(primary_key = true)]
	id: i64,
	published: bool,
}

#[model(
	app_label = "forms",
	form(
		name = SupportedScalarCreateForm,
		fields(count, ratio, price, external_id, day, time, aware_at, naive_at, metadata)
	)
)]
#[derive(Clone, Deserialize, Serialize)]
struct SupportedScalarDocument {
	#[field(primary_key = true)]
	id: i64,
	count: i32,
	ratio: f64,
	price: rust_decimal::Decimal,
	external_id: uuid::Uuid,
	day: chrono::NaiveDate,
	time: chrono::NaiveTime,
	aware_at: chrono::DateTime<chrono::Utc>,
	naive_at: chrono::NaiveDateTime,
	metadata: serde_json::Value,
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
fn named_contract_is_strict_and_preserves_selected_field_order() {
	assert_eq!(
		<ClusterCreateFormSchema as ModelFormContractSchema>::fields()
			.iter()
			.map(|field| field.name)
			.collect::<Vec<_>>(),
		["name", "api_url"]
	);
	assert_eq!(
		<ClusterCreateForm as ModelFormContract>::fields(),
		[ClusterCreateFormField::Name, ClusterCreateFormField::ApiUrl,]
	);
	assert_eq!(ClusterCreateFormField::Name.name(), "name");
	assert_eq!(ClusterCreateForm::name().name, "name");

	let mut payload = ClusterCreateFormData::default();
	assert_eq!(payload.name(), None);
	assert_eq!(payload.api_url(), None);
	payload.set_name("cluster-a".to_owned());
	payload.set_api_url(None);
	assert_eq!(payload.name(), Some(&"cluster-a".to_owned()));
	assert_eq!(payload.api_url(), Some(&None));
	assert_eq!(
		serde_json::to_value(&payload).expect("named payload should serialize"),
		serde_json::json!({ "name": "cluster-a", "api_url": null })
	);
	assert_eq!(payload.supplied_fields(), ["name", "api_url"]);
	assert!(payload.forbidden_fields().is_empty());
	assert_eq!(payload.get_json("secret"), None);

	let omitted: ClusterCreateFormData = serde_json::from_value(serde_json::json!({}))
		.expect("selected fields may be omitted before validation");
	assert_eq!(omitted.api_url(), None);
	let explicit_null: ClusterCreateFormData =
		serde_json::from_value(serde_json::json!({ "api_url": null }))
			.expect("nullable selected fields should accept null");
	assert_eq!(explicit_null.api_url(), Some(&None));

	for value in [
		serde_json::json!({ "secret": "server-owned" }),
		serde_json::json!({ "organization_id": 42 }),
		serde_json::json!({ "name": 42 }),
	] {
		assert!(serde_json::from_value::<ClusterCreateFormData>(value).is_err());
	}
	let error = serde_json::from_str::<ClusterCreateFormData>(
		r#"{"name":"first","name":"second","api_url":"https://example.com"}"#,
	)
	.expect_err("duplicate fields must be rejected");
	assert!(error.to_string().contains("duplicate field `name`"));
}

#[test]
fn named_contract_native_payload_reuses_checkbox_normalization() {
	let native = <BooleanCreateFormData as NativeModelFormPayload>::from_native_form_value(
		serde_json::json!({}),
	)
	.expect("native form payload should decode");
	assert_eq!(native.published(), Some(&false));

	let json: BooleanCreateFormData =
		serde_json::from_value(serde_json::json!({})).expect("JSON payload should decode");
	assert_eq!(json.published(), None);
}

#[test]
fn named_contract_supports_wire_safe_scalar_types() {
	let kinds = <SupportedScalarCreateFormSchema as ModelFormContractSchema>::fields()
		.iter()
		.map(|field| field.kind)
		.collect::<Vec<_>>();
	assert_eq!(
		kinds,
		[
			ModelFormFieldKind::Integer {
				min: None,
				max: None
			},
			ModelFormFieldKind::Float {
				min: None,
				max: None
			},
			ModelFormFieldKind::Decimal {
				min: None,
				max: None
			},
			ModelFormFieldKind::Uuid,
			ModelFormFieldKind::Date,
			ModelFormFieldKind::Time,
			ModelFormFieldKind::DateTime,
			ModelFormFieldKind::NaiveDateTime,
			ModelFormFieldKind::Json,
		]
	);
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
		}
	);
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
