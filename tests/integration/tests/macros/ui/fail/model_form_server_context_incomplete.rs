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
}

fn main() {
	let mut raw = ClusterModelFormData::<AllEditableModelFields>::empty();
	raw.set_name("primary".to_owned()).unwrap();
	let cleaned = raw.clean_and_validate().unwrap();
	let _ = cleaned.into_model(ClusterModelFormServerContext::new());
}
