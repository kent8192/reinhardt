#![deny(unexpected_cfgs)]

use reinhardt::model;
use serde::{Deserialize, Serialize};
use decimal as rust_decimal;
use identifier as uuid;
use json as serde_json;
use time as chrono;

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use reinhardt::db::associations::ForeignKeyField;

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
#[model(
	app_label = "organizations",
	table_name = "organizations",
	info = false
)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Organization {
	#[field(primary_key = true)]
	pub id: i64,
}

#[model(
	app_label = "clusters",
	table_name = "clusters",
	form(name = ClusterCreateForm, fields(name, api_url)),
	info = false
)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Cluster {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(editable = false)]
	#[rel(foreign_key, related_name = "clusters")]
	pub organization: ForeignKeyField<Organization>,

	#[field(min_length = 1, max_length = 63)]
	pub name: String,

	#[field(url = true, max_length = 2048, null = true)]
	pub api_url: Option<String>,
}

#[model(
	app_label = "keywords",
	table_name = "keyword_documents",
	form(name = RawIdentifierCreateForm, fields(r#type)),
	info = false
)]
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct RawIdentifierDocument {
	#[field(primary_key = true)]
	pub id: i64,
	#[field(max_length = 32)]
	pub r#type: String,
}

#[model(
	app_label = "scalar_documents",
	table_name = "scalar_documents",
	form(name = ScalarCreateForm, fields(price, external_id, day, metadata)),
	info = false
)]
#[derive(Clone, Serialize, Deserialize)]
pub struct ScalarDocument {
	#[field(primary_key = true)]
	pub id: i64,
	pub price: rust_decimal::Decimal,
	pub external_id: uuid::Uuid,
	pub day: chrono::NaiveDate,
	pub metadata: serde_json::Value,
}

#[cfg(test)]
mod tests {
	use super::{
		ClusterCreateForm, ClusterCreateFormData, ClusterCreateFormField, ClusterCreateFormSchema,
		RawIdentifierCreateForm, RawIdentifierCreateFormData, RawIdentifierCreateFormField,
		RawIdentifierCreateFormSchema, ScalarCreateFormSchema,
	};
	use reinhardt_core::model_form::{
		ModelFormContract, ModelFormContractField, ModelFormContractSchema, ModelFormFieldKind,
		ModelFormPayload,
	};
	use json as serde_json;
	use wasm_bindgen_test::wasm_bindgen_test;

	#[wasm_bindgen_test]
	fn generated_named_contract_executes_in_wasm_runtime() {
		assert_eq!(
			<ClusterCreateFormSchema as ModelFormContractSchema>::contract_fields()
				.iter()
				.map(|field| field.name)
				.collect::<Vec<_>>(),
			["name", "api_url"],
		);
		assert_eq!(
			<ClusterCreateForm as ModelFormContract>::fields(),
			[ClusterCreateFormField::Name, ClusterCreateFormField::ApiUrl],
		);
		assert_eq!(ClusterCreateFormField::Name.name(), "name");

		let mut payload = ClusterCreateFormData::default();
		payload.set_name("cluster-a".to_owned());
		payload.set_api_url(None);
		assert_eq!(payload.supplied_fields(), ["name", "api_url"]);
		assert_eq!(
			serde_json::to_value(&payload).expect("named contract should serialize in WASM"),
			serde_json::json!({ "name": "cluster-a", "api_url": null }),
		);
		assert_eq!(
			<RawIdentifierCreateFormSchema as ModelFormContractSchema>::contract_fields()
				.iter()
				.map(|field| field.name)
				.collect::<Vec<_>>(),
			["type"],
		);
		assert_eq!(RawIdentifierCreateFormField::Type.name(), "type");
		assert_eq!(RawIdentifierCreateForm::r#type().name, "type");
		let mut raw_payload = RawIdentifierCreateFormData::default();
		raw_payload.set_type("json".to_owned());
		assert_eq!(raw_payload.supplied_fields(), ["type"]);
		assert_eq!(
			serde_json::to_value(raw_payload).expect("raw identifier payload should serialize"),
			serde_json::json!({ "type": "json" }),
		);
		assert_eq!(
			<ScalarCreateFormSchema as ModelFormContractSchema>::contract_fields()
				.iter()
				.map(|field| field.kind)
				.collect::<Vec<_>>(),
			[
				ModelFormFieldKind::Decimal {
					min: None,
					max: None,
				},
				ModelFormFieldKind::Uuid,
				ModelFormFieldKind::Date,
				ModelFormFieldKind::Json,
			],
		);
	}
}
