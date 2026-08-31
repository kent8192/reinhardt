#![deny(unexpected_cfgs)]

use reinhardt::model;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
	use super::{
		ClusterCreateForm, ClusterCreateFormData, ClusterCreateFormField, ClusterCreateFormSchema,
	};
	use reinhardt_core::model_form::{
		ModelFormContract, ModelFormContractField, ModelFormContractSchema, ModelFormPayload,
	};
	use wasm_bindgen_test::wasm_bindgen_test;

	#[wasm_bindgen_test]
	fn generated_named_contract_executes_in_wasm_runtime() {
		assert_eq!(
			<ClusterCreateFormSchema as ModelFormContractSchema>::fields()
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
	}
}
