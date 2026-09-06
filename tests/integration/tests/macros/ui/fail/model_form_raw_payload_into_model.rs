// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]

use reinhardt::model;
use reinhardt_core::model_form::AllEditableModelFields;
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
	let raw = ClusterModelFormData::<AllEditableModelFields>::empty();
	let context = ClusterModelFormServerContext::new().organization_id(42);
	let _ = raw.into_model(context);
}
