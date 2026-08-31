// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]

use reinhardt::model;
use reinhardt_core::model_form::{AllEditableModelFields, ModelFormValidatingPayload};
use serde::{Deserialize, Serialize};

#[model(app_label = "model_form_ui", form = true, info = false)]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Cluster {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	name: String,
	#[field(editable = false)]
	organization_id: i64,
	#[field(max_length = 200, editable = false)]
	note: Option<String>,
	#[field(default = "system", editable = false, max_length = 200)]
	audit_token: String,
}

fn main() {
	let mut raw = ClusterModelFormData::<AllEditableModelFields>::empty();
	raw.set_name("primary".to_owned())
		.expect("editable name should be accepted");
	let cleaned = raw
		.clean_and_validate()
		.expect("valid payload should clean");
	let context = ClusterModelFormServerContext::new().organization_id(42);
	let cluster = cleaned
		.into_model(context)
		.expect("complete context should construct the model");

	assert_eq!(cluster.organization_id, 42);
}
