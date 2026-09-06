// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]

use reinhardt::model;
use serde::{Deserialize, Serialize};

#[model(app_label = "model_form_ui", form = true, info = false)]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ContextStateCollision {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	name: String,
	#[field(editable = false)]
	_state: i64,
}

fn main() {}
