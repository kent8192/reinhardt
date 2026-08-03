// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]
//! Fail case: in-bulk retrieval accepts only metadata-proven unique fields.

use reinhardt::db::orm::UniqueFieldRef;
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "events", table_name = "events")]
struct Event {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
}

fn accepts_unique(_: UniqueFieldRef<Event, String>) {}

fn main() {
	accepts_unique(Event::field_name());
}
