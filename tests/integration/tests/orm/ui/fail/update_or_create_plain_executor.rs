// Compile-only fixtures intentionally leave model fields and helper functions uncalled.
#![allow(dead_code, unexpected_cfgs)]
//! Fail case: update-or-create requires an atomic transaction executor.

use reinhardt::db::orm::{CustomManager, OrmExecutor};
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

async fn update_with_plain_executor<E: OrmExecutor + ?Sized>(executor: &mut E) {
	let _ = Tag::objects()
		.update_or_create()
		.lookup(Tag::field_slug(), "rust")
		.execute_with(executor)
		.await;
}

fn main() {}
