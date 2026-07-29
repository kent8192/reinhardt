// Compile-only helpers are type-checked but not run, and model macro expansion emits
// generated cfg checks for features defined by the consuming integration crate.
#![allow(dead_code, unexpected_cfgs)]
//! Fail case: a builder rejects a field accessor from another model.

use reinhardt::db::orm::CustomManager;
use reinhardt::{Model, model};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[model(app_label = "typed_upsert", table_name = "tags")]
struct Tag {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 64, unique = true)]
	slug: String,
}

#[derive(Serialize, Deserialize)]
#[model(app_label = "typed_upsert", table_name = "users")]
struct User {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 255, unique = true)]
	email: String,
}

fn main() {
	let _ = Tag::objects()
		.get_or_create()
		.lookup(User::field_email(), "a@example.com");
}
