//! Tweet model

use chrono::{DateTime, Utc};
use reinhardt::db::associations::ForeignKeyField;
use reinhardt::model;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Used by #[model] macro for type inference in ForeignKeyField<T> relationship fields.
// The macro requires this type to be in scope for generating the correct column types
// and relationship metadata, even though it appears unused to the compiler.
#[allow(unused_imports)]
use crate::apps::auth::models::User;

// Test-only dependency for sqlx::FromRow (server-side only)
#[cfg(all(test, native))]
use sqlx::FromRow;

#[model(app_label = "tweet", table_name = "tweet_tweet")]
#[derive(Serialize, Deserialize)]
#[cfg_attr(all(test, native), derive(FromRow))]
pub struct Tweet {
	#[field(primary_key = true)]
	pub id: Uuid,

	#[cfg_attr(all(test, native), sqlx(skip))]
	#[rel(foreign_key, related_name = "tweets")]
	pub user: ForeignKeyField<User>,

	#[field(max_length = 280)]
	pub content: String,

	#[field(default = 0)]
	pub like_count: i32,

	#[field(default = 0)]
	pub retweet_count: i32,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,

	#[field(auto_now = true)]
	pub updated_at: DateTime<Utc>,
}
