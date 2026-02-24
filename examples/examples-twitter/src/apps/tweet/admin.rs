//! Admin configuration for tweet app

use crate::apps::tweet::models::Tweet;
use reinhardt::admin;

/// Admin configuration for Tweet model
///
/// This configures the admin panel display for Tweet items:
/// - List view with id, user_id, content, like_count, retweet_count, and created_at
/// - Filtering by created_at
/// - Search by content
/// - Sorted by creation date (newest first)
#[admin(model,
	for = Tweet,
	name = "Tweet",
	list_display = [id, user_id, content, like_count, retweet_count, created_at],
	list_filter = [created_at],
	search_fields = [content],
	ordering = [(created_at, desc)]
)]
pub struct TweetAdmin;
