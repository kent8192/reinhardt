// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]

use reinhardt_macros::model;

include!("../support.rs");

#[model(app_label = "forms", form = true)]
struct Cluster {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	name: String,
}

fn main() {
	let raw = ClusterModelFormData::<model_form::AllEditableModelFields>::empty();
	let _ = raw.into_model();
}
