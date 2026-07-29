// Compile-only fixtures intentionally leave model fields unconstructed.
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
