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
#[cfg(all(test, server))]
use sqlx::FromRow;

#[model(app_label = "tweet", table_name = "tweet_tweet")]
#[derive(Serialize, Deserialize)]
#[cfg_attr(all(test, server), derive(FromRow))]
pub struct Tweet {
	#[field(primary_key = true)]
	id: Uuid,

	#[cfg_attr(all(test, server), sqlx(skip))]
	#[rel(foreign_key, related_name = "tweets")]
	user: ForeignKeyField<User>,

	#[field(max_length = 280)]
	content: String,

	#[field(default = 0)]
	like_count: i32,

	#[field(default = 0)]
	retweet_count: i32,

	#[field(auto_now_add = true)]
	created_at: DateTime<Utc>,

	#[field(auto_now = true)]
	updated_at: DateTime<Utc>,
}
