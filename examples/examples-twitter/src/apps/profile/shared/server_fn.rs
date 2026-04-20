//! Profile server functions
//!
//! Server functions for user profile management.

use crate::apps::profile::shared::types::{ProfileResponse, UpdateProfileRequest};
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use uuid::Uuid;

// Server-only imports
#[cfg(native)]
use {
	crate::apps::auth::models::User,
	crate::apps::profile::models::Profile,
	reinhardt::AuthUser,
	reinhardt::DatabaseConnection,
	reinhardt::Validate,
	reinhardt::db::orm::{FilterOperator, FilterValue, Model},
};

/// Internal helper for profile update logic
#[cfg(native)]
async fn update_profile_internal(
	request: &UpdateProfileRequest,
	db: &DatabaseConnection,
	user: &User,
) -> std::result::Result<Profile, ServerFnError> {
	// Validate request
	request
		.validate()
		.map_err(|e| ServerFnError::server(400, format!("Validation failed: {}", e)))?;

	// Find existing profile
	let mut profile = Profile::objects()
		.filter(
			Profile::field_user_id(),
			FilterOperator::Eq,
			FilterValue::String(user.id().to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Profile not found"))?;

	// Update fields
	if let Some(ref bio) = request.bio {
		profile.set_bio(bio.clone());
	}
	if let Some(ref avatar_url) = request.avatar_url {
		profile.set_avatar_url(avatar_url.clone());
	}
	if let Some(ref location) = request.location {
		profile.set_location(Some(location.clone()));
	}
	if let Some(ref website) = request.website {
		profile.set_website(Some(website.clone()));
	}

	// Save to database
	Profile::objects()
		.update_with_conn(db, &profile)
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?;

	Ok(profile)
}

/// Fetch user profile
#[server_fn]
pub async fn fetch_profile(
	user_id: Uuid,
	#[inject] _db: DatabaseConnection,
) -> std::result::Result<ProfileResponse, ServerFnError> {
	let profile = Profile::objects()
		.filter(
			Profile::field_user_id(),
			FilterOperator::Eq,
			FilterValue::String(user_id.to_string()),
		)
		.first()
		.await
		.map_err(|e| ServerFnError::server(500, format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(404, "Profile not found"))?;

	Ok(ProfileResponse::from(profile))
}

/// Update user profile
#[server_fn]
pub async fn update_profile(
	request: UpdateProfileRequest,
	#[inject] db: DatabaseConnection,
	#[inject] AuthUser(user): AuthUser<User>,
) -> std::result::Result<ProfileResponse, ServerFnError> {
	let profile = update_profile_internal(&request, &db, &user).await?;
	Ok(ProfileResponse::from(profile))
}

/// Form-compatible wrapper for update_profile
///
/// This wrapper accepts individual field arguments from form! macro's server_fn integration
/// and converts them to UpdateProfileRequest. Returns `Result<(), ServerFnError>`
/// as expected by form! macro's submit() method.
///
/// The argument order matches form! macro's field order: avatar_url, bio, location, website,
/// followed by `_csrf_token` which the macro auto-appends for non-GET forms (#3825).
/// CSRF is enforced by middleware, so we accept and ignore it here.
#[server_fn]
pub async fn update_profile_form(
	avatar_url: String,
	bio: String,
	location: String,
	website: String,
	_csrf_token: String,
	#[inject] db: DatabaseConnection,
	#[inject] AuthUser(user): AuthUser<User>,
) -> std::result::Result<(), ServerFnError> {
	let request = UpdateProfileRequest {
		avatar_url: if avatar_url.is_empty() {
			None
		} else {
			Some(avatar_url)
		},
		bio: if bio.is_empty() { None } else { Some(bio) },
		location: if location.is_empty() {
			None
		} else {
			Some(location)
		},
		website: if website.is_empty() {
			None
		} else {
			Some(website)
		},
	};

	update_profile_internal(&request, &db, &user).await?;
	Ok(())
}
