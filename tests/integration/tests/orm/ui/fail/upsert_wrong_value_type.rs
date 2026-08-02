// Compile-only helpers are type-checked but not run, and model macro expansion emits
// generated cfg checks for features defined by the consuming integration crate.
#![allow(dead_code, unexpected_cfgs)]
//! Fail case: a builder rejects a value incompatible with its field.

use reinhardt::db::orm::CustomManager;
use reinhardt::{Model, model};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "typed_upsert", table_name = "tags")]
struct Tag {
	#[field(primary_key = true)]
	id: Option<i64>,
	rank: i32,
}

fn main() {
	let _ = Tag::objects()
		.get_or_create()
		.lookup(Tag::field_rank(), "not-an-integer");
}
