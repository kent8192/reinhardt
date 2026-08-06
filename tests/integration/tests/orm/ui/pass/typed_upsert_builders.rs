// Compile-only helpers are type-checked but not run, and model macro expansion emits
// generated cfg checks for features defined by the consuming integration crate.
#![allow(dead_code, unexpected_cfgs)]
//! Pass case: typed upsert builders accept matching model fields and values.

use reinhardt::db::orm::{AtomicTransaction, CustomManager, OrmExecutor};
use reinhardt::{Model, model};
use serde::{Deserialize, Serialize};

#[derive(Default)]
struct TagManager;

impl CustomManager for TagManager {
	type Model = Tag;

	fn new() -> Self {
		Self
	}
}

#[derive(Serialize, Deserialize)]
#[model(
	app_label = "typed_upsert",
	table_name = "tags",
	manager = TagManager
)]
struct Tag {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 64, unique = true)]
	slug: String,
	rank: i32,
	#[field(max_length = 64)]
	created_by: String,
	#[field(max_length = 128)]
	note: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[model(app_label = "typed_upsert", table_name = "users")]
struct User {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 255, unique = true)]
	email: String,
}

async fn get_with_executor<E: OrmExecutor + ?Sized>(executor: &mut E) {
	let _ = Tag::objects()
		.get_or_create()
		.lookup(Tag::field_slug(), "rust")
		.default(Tag::field_rank(), 1_i32)
		.default(Tag::field_note(), None::<String>)
		.execute_with(executor)
		.await;
}

async fn update_with_transaction(transaction: &mut AtomicTransaction) {
	let _ = Tag::objects()
		.update_or_create()
		.lookup(Tag::field_slug(), "rust")
		.set(Tag::field_rank(), 2_i32)
		.create_default(Tag::field_created_by(), "seed")
		.create_default(Tag::field_note(), Some("stable".to_owned()))
		.execute_with(transaction)
		.await;
}

fn custom_manager_entry_points() {
	let _: TagManager = Tag::objects();
	let _ = TagManager::new()
		.get_or_create()
		.lookup(Tag::field_slug(), "custom");
	let _ = TagManager::new()
		.update_or_create()
		.lookup(Tag::field_slug(), "custom");
}

fn default_manager_entry_points() {
	let _ = User::objects()
		.get_or_create()
		.lookup(User::field_email(), "a@example.com");
	let _ = User::objects()
		.update_or_create()
		.lookup(User::field_email(), "a@example.com");
}

fn main() {}
