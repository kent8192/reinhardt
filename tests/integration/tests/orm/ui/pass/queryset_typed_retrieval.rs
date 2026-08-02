// The model macro emits native-only cfgs evaluated in this standalone trybuild crate.
#![allow(unexpected_cfgs)]
//! Pass case: typed retrieval metadata preserves model identity and unique keys.

use reinhardt::db::orm::{Model, OrderingField, UniqueFieldRef};
use reinhardt::model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(
	app_label = "events",
	table_name = "events",
	get_latest_by = ("created_at", "id")
)]
struct Event {
	#[field(primary_key = true)]
	id: i64,
	#[field(db_column = "created_on")]
	created_at: i64,
	#[field(max_length = 255, unique = true)]
	email: String,
}

fn accepts_event_ordering(_: &[OrderingField<Event>]) {}

fn accepts_event_unique(_: UniqueFieldRef<Event, String>) {}

fn main() {
	accepts_event_ordering(&[
		Event::ordering_created_at(),
		Event::ordering_id(),
	]);
	accepts_event_unique(Event::unique_email());
	assert_eq!(<Event as Model>::latest_by_fields(), ["created_on", "id"]);
}
