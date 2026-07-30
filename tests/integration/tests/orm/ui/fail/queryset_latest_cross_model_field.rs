// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]
//! Fail case: ordering fields from distinct models cannot be combined.

use reinhardt::db::orm::OrderingField;
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "events", table_name = "events")]
struct Event {
	#[field(primary_key = true)]
	id: i64,
}

#[derive(Serialize, Deserialize)]
#[model(app_label = "projects", table_name = "projects")]
struct Project {
	#[field(primary_key = true)]
	id: i64,
}

fn accepts_event_ordering(_: &[OrderingField<Event>]) {}

fn main() {
	accepts_event_ordering(&[
		Event::field_id().ordering(),
		Project::field_id().ordering(),
	]);
}
