//! Shared types for tweet application
//!
//! These types are serializable and can be sent between the WASM client
//! and the Rust server via server functions.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// OpenAPI schema generation (server-side only)
#[cfg(server)]
use reinhardt::rest::ToSchema;
#[cfg(server)]
use reinhardt::rest::openapi::Schema;

/// Tweet information
#[cfg_attr(server, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TweetInfo {
	pub id: Uuid,
	pub user_id: Uuid,
	pub username: String,
	pub content: String,
	pub like_count: i32,
	pub retweet_count: i32,
	pub created_at: String,
}

impl TweetInfo {
	/// Create a new TweetInfo instance
	pub fn new(
		id: Uuid,
		user_id: Uuid,
		username: String,
		content: String,
		like_count: i32,
		retweet_count: i32,
		created_at: String,
	) -> Self {
		Self {
			id,
			user_id,
			username,
			content,
			like_count,
			retweet_count,
			created_at,
		}
	}
}

/// Conversion from server-side Tweet model to shared TweetInfo
#[cfg(server)]
impl From<crate::apps::tweet::models::Tweet> for TweetInfo {
	fn from(tweet: crate::apps::tweet::models::Tweet) -> Self {
		TweetInfo {
			id: tweet.id(),
			user_id: *tweet.user_id(),
			username: String::new(), // Will be set by server_fn
			content: tweet.content().to_string(),
			like_count: tweet.like_count(),
			retweet_count: tweet.retweet_count(),
			created_at: tweet.created_at().to_rfc3339(),
		}
	}
}

/// Create tweet request
#[cfg_attr(server, derive(Schema))]
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateTweetRequest {
	#[validate(length(
		min = 1,
		max = 280,
		message = "Tweet must be between 1 and 280 characters"
	))]
	pub content: String,
}
