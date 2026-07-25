#![allow(unexpected_cfgs)]

use reinhardt_macros::model;
use serde::{Deserialize, Serialize};

include!("ui/model/support.rs");

use model_form::{AllEditableModelFields, ModelFormFieldKind, ModelFormPayload, ModelFormPolicy};

#[model(app_label = "forms", form = true)]
#[derive(Clone, Deserialize, Serialize)]
struct FormDocument {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 200)]
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
	#[rel(foreign_key)]
	target: db::associations::ForeignKeyField<StringKeyTarget>,
}

struct TitleOnly;

impl ModelFormPolicy for TitleOnly {
	fn allows(field: &str) -> bool {
		matches!(field, "title" | "nullable")
	}
}

#[test]
fn generated_payload_applies_policy_and_preserves_nullable_values() {
	let mut payload = FormDocumentModelFormData::<TitleOnly>::empty();
	payload.set_title("published".to_owned());
	payload.set_secret("do-not-serialize".to_owned());

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
				max_length: Some(200),
				multiline: false,
			},
			required: true,
			has_default: false,
			editable: true,
			generated_relation_id: false,
		}
	);
	assert_eq!(
		StringKeyChildFormSchema::target_id().kind,
		ModelFormFieldKind::Text {
			max_length: Some(64),
			multiline: false,
		}
	);

	let mut payload = FormDocumentModelFormData::<AllEditableModelFields>::empty();
	payload
		.set_json("title", serde_json::json!("updated"))
		.expect("known editable field");
	assert_eq!(payload.supplied_fields(), ["title"]);
}
