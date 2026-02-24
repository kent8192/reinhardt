//! Tweet server functions
//!
//! Server functions for tweet management.

use crate::apps::tweet::shared::types::TweetInfo;
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use uuid::Uuid;

// Server-only imports
#[cfg(server)]
use {
	crate::apps::auth::models::User,
	crate::apps::tweet::models::Tweet,
	crate::apps::tweet::shared::types::CreateTweetRequest,
	reinhardt::CurrentUser,
	reinhardt::DatabaseConnection,
	reinhardt::db::orm::{Filter, FilterOperator, FilterValue, Model},
	validator::Validate,
};

/// Create a new tweet
///
/// Accepts `content` as a String parameter (form! macro passes individual field values).
/// Internally constructs CreateTweetRequest for validation.
#[server_fn(use_inject = true)]
pub async fn create_tweet(
	content: String,
	#[inject] db: DatabaseConnection,
	#[inject] current_user: CurrentUser<User>,
) -> std::result::Result<TweetInfo, ServerFnError> {
	// Construct request for validation
	let request = CreateTweetRequest {
		content: content.clone(),
	};

	// Validate request
	request
		.validate()
		.map_err(|e| ServerFnError::application(format!("Validation failed: {}", e)))?;

	// Get current user (already loaded by CurrentUser<User> Injectable)
	let user = current_user
		.user()
		.map_err(|_| ServerFnError::server(401, "Not authenticated"))?;

	let user_id = current_user
		.id()
		.map_err(|_| ServerFnError::server(401, "Not authenticated"))?;

	// Create Tweet model using new() method
	let tweet = Tweet::new(
		request.content.clone(),
		0,       // like_count
		0,       // retweet_count
		user_id, // ForeignKeyField parameter (Uuid)
	);

	// Save to database
	Tweet::objects()
		.create_with_conn(&db, &tweet)
		.await
		.map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

	// Return created tweet info
	Ok(TweetInfo::new(
		tweet.id(),
		user_id,
		user.username().to_string(),
		tweet.content().to_string(),
		tweet.like_count(),
		tweet.retweet_count(),
		tweet.created_at().to_rfc3339(),
	))
}

/// List tweets
#[server_fn(use_inject = true)]
pub async fn list_tweets(
	user_id: Option<Uuid>,
	page: u32,
	#[inject] db: DatabaseConnection,
) -> std::result::Result<Vec<TweetInfo>, ServerFnError> {
	const PAGE_SIZE: u32 = 20;

	// Build query
	let mut query = Tweet::objects().all();

	// Filter by user_id if provided
	if let Some(uid) = user_id {
		query = query.filter(Filter::new(
			"user_id",
			FilterOperator::Eq,
			FilterValue::string(uid),
		));
	}

	// Fetch tweets with pagination
	let tweets = query
		.order_by(&["-created_at"]) // Django-style: "-" prefix for descending order
		.limit(PAGE_SIZE as usize)
		.offset((page * PAGE_SIZE) as usize)
		.all_with_db(&db)
		.await
		.map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

	// Collect unique user IDs for batch fetch (SELECT IN pattern)
	let user_ids: Vec<String> = tweets
		.iter()
		.map(|t| t.user_id().to_string())
		.collect::<std::collections::HashSet<_>>()
		.into_iter()
		.collect();

	// Batch fetch all users in single query
	let users = if user_ids.is_empty() {
		Vec::new()
	} else {
		User::objects()
			.filter_by(Filter::new(
				"id",
				FilterOperator::In,
				FilterValue::Array(user_ids),
			))
			.all_with_db(&db)
			.await
			.map_err(|e| ServerFnError::application(format!("Failed to fetch users: {}", e)))?
	};

	// Create HashMap for O(1) lookup
	let user_map: std::collections::HashMap<uuid::Uuid, &User> =
		users.iter().map(|u| (u.id(), u)).collect();

	// Convert to TweetInfo using HashMap lookup
	let mut tweet_infos = Vec::with_capacity(tweets.len());
	for tweet in tweets {
		let user = user_map.get(tweet.user_id()).ok_or_else(|| {
			ServerFnError::application(format!("User not found for tweet: {}", tweet.id()))
		})?;

		tweet_infos.push(TweetInfo::new(
			tweet.id(),
			*tweet.user_id(),
			user.username().to_string(),
			tweet.content().to_string(),
			tweet.like_count(),
			tweet.retweet_count(),
			tweet.created_at().to_rfc3339(),
		));
	}

	Ok(tweet_infos)
}

/// Delete a tweet
#[server_fn(use_inject = true)]
pub async fn delete_tweet(
	tweet_id: Uuid,
	#[inject] db: DatabaseConnection,
	#[inject] current_user: CurrentUser<User>,
) -> std::result::Result<(), ServerFnError> {
	// Get current user
	let user_id = current_user
		.id()
		.map_err(|_| ServerFnError::server(401, "Not authenticated"))?;

	// Fetch the tweet
	let tweet = Tweet::objects()
		.filter_by(Filter::new(
			"id",
			FilterOperator::Eq,
			FilterValue::string(tweet_id),
		))
		.first_with_db(&db)
		.await
		.map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::application("Tweet not found".to_string()))?;

	// Verify ownership
	if *tweet.user_id() != user_id {
		return Err(ServerFnError::server(
			403,
			"Permission denied: You can only delete your own tweets",
		));
	}

	// Delete the tweet
	Tweet::objects()
		.delete_with_conn(&db, tweet.id())
		.await
		.map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

	Ok(())
}
