//! Relationship server functions
//!
//! Server functions for user follow/unfollow and follower management.

use crate::apps::auth::shared::types::UserInfo;
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use uuid::Uuid;

// Server-only imports
#[cfg(server)]
use {
	crate::apps::auth::models::User,
	reinhardt::DatabaseConnection,
	reinhardt::db::orm::{FilterOperator, FilterValue, ManyToManyAccessor, Model},
	reinhardt::middleware::session::SessionData,
};

/// Follow a user
#[server_fn(use_inject = true)]
pub async fn follow_user(
	target_user_id: Uuid,
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<(), ServerFnError> {
	let follower_id = session
		.get::<Uuid>("user_id")
		.ok_or_else(|| ServerFnError::server(401, "Not authenticated"))?;

	// Load follower user
	let follower = User::objects()
		.filter(
			User::field_id(),
			FilterOperator::Eq,
			FilterValue::String(follower_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Follower user not found"))?;

	// Load target user
	let target_user = User::objects()
		.filter(
			User::field_id(),
			FilterOperator::Eq,
			FilterValue::String(target_user_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Target user not found"))?;

	// Add follow relationship
	let accessor = ManyToManyAccessor::<User, User>::new(&follower, "following", db.clone());
	accessor
		.add(&target_user)
		.await
		.map_err(|e| ServerFnError::server(500, format!("Failed to follow user: {}", e)))?;

	Ok(())
}

/// Unfollow a user
#[server_fn(use_inject = true)]
pub async fn unfollow_user(
	target_user_id: Uuid,
	#[inject] db: DatabaseConnection,
	#[inject] session: SessionData,
) -> std::result::Result<(), ServerFnError> {
	let follower_id = session
		.get::<Uuid>("user_id")
		.ok_or_else(|| ServerFnError::server(401, "Not authenticated"))?;

	// Load follower user
	let follower = User::objects()
		.filter(
			User::field_id(),
			FilterOperator::Eq,
			FilterValue::String(follower_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Follower user not found"))?;

	// Load target user
	let target_user = User::objects()
		.filter(
			User::field_id(),
			FilterOperator::Eq,
			FilterValue::String(target_user_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Target user not found"))?;

	// Remove follow relationship
	let accessor = ManyToManyAccessor::<User, User>::new(&follower, "following", db.clone());
	accessor
		.remove(&target_user)
		.await
		.map_err(|e| ServerFnError::server(500, format!("Failed to unfollow user: {}", e)))?;

	Ok(())
}

/// Fetch followers of a user
#[server_fn(use_inject = true)]
pub async fn fetch_followers(
	user_id: Uuid,
	#[inject] db: DatabaseConnection,
) -> std::result::Result<Vec<UserInfo>, ServerFnError> {
	let user = User::objects()
		.filter(
			User::field_id(),
			FilterOperator::Eq,
			FilterValue::String(user_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "User not found"))?;

	let followers = ManyToManyAccessor::<User, User>::filter_by_target(
		&User::objects(),
		"following",
		&user,
		db,
	)
	.await
	.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	Ok(followers.into_iter().map(UserInfo::from).collect())
}

/// Fetch users that the specified user is following
#[server_fn(use_inject = true)]
pub async fn fetch_following(
	user_id: Uuid,
	#[inject] db: DatabaseConnection,
) -> std::result::Result<Vec<UserInfo>, ServerFnError> {
	// Load target user
	let user = User::objects()
		.filter(
			User::field_id(),
			FilterOperator::Eq,
			FilterValue::String(user_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "User not found"))?;

	// Get following users
	let accessor = ManyToManyAccessor::<User, User>::new(&user, "following", db.clone());
	let following = accessor
		.all()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	Ok(following.into_iter().map(UserInfo::from).collect())
}
