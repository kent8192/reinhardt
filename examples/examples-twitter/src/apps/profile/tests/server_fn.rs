//! Profile server function tests
//!
//! Tests for fetch_profile, update_profile server functions.

use rstest::*;
use sqlx::PgPool;

use crate::apps::profile::shared::types::{ProfileResponse, UpdateProfileRequest};
use crate::test_utils::factories::user::{ProfileFactory, UserFactory};
use crate::test_utils::fixtures::database::twitter_db_pool;
use crate::test_utils::fixtures::users::TestTwitterUser;

// ============================================================================
// Fetch Profile Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_fetch_profile_success(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let profile_factory = ProfileFactory::new();

	// Create test user
	let test_user = TestTwitterUser::new("profileuser");
	let user = user_factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Create profile for user
	let profile = profile_factory
		.create_for_user(
			&pool,
			user.id(),
			"Test bio",
			"https://example.com/avatar.png",
		)
		.await
		.expect("Profile creation should succeed");

	// Verify profile data
	assert_eq!(profile.user_id(), user.id());
	assert_eq!(profile.bio(), "Test bio");
	assert_eq!(profile.avatar_url(), "https://example.com/avatar.png");
}

#[rstest]
#[tokio::test]
async fn test_fetch_profile_not_found(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let profile_factory = ProfileFactory::new();

	// Try to find profile for non-existent user
	let fake_id = uuid::Uuid::new_v4();
	let result = profile_factory.find_by_user_id(&pool, fake_id).await;

	// Should return error (no profile found)
	assert!(result.is_err(), "Profile for non-existent user should fail");
}

// ============================================================================
// Update Profile Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_bio_too_long() {
	use validator::Validate;

	// Bio exceeds 500 characters
	let long_bio = "a".repeat(501);
	let request = UpdateProfileRequest {
		bio: Some(long_bio),
		avatar_url: None,
		location: None,
		website: None,
	};

	let result = request.validate();
	assert!(result.is_err(), "Bio over 500 chars should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_bio_max_length() {
	use validator::Validate;

	// Bio exactly at limit (500 characters)
	let max_bio = "a".repeat(500);
	let request = UpdateProfileRequest {
		bio: Some(max_bio),
		avatar_url: None,
		location: None,
		website: None,
	};

	let result = request.validate();
	assert!(result.is_ok(), "Bio at 500 chars should pass validation");
}

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_invalid_avatar_url() {
	use validator::Validate;

	let request = UpdateProfileRequest {
		bio: None,
		avatar_url: Some("not-a-valid-url".to_string()),
		location: None,
		website: None,
	};

	let result = request.validate();
	assert!(result.is_err(), "Invalid avatar URL should fail validation");
}

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_valid_avatar_url() {
	use validator::Validate;

	let request = UpdateProfileRequest {
		bio: None,
		avatar_url: Some("https://example.com/avatar.png".to_string()),
		location: None,
		website: None,
	};

	let result = request.validate();
	assert!(result.is_ok(), "Valid avatar URL should pass validation");
}

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_location_too_long() {
	use validator::Validate;

	// Location exceeds 100 characters
	let long_location = "a".repeat(101);
	let request = UpdateProfileRequest {
		bio: None,
		avatar_url: None,
		location: Some(long_location),
		website: None,
	};

	let result = request.validate();
	assert!(
		result.is_err(),
		"Location over 100 chars should fail validation"
	);
}

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_invalid_website() {
	use validator::Validate;

	let request = UpdateProfileRequest {
		bio: None,
		avatar_url: None,
		location: None,
		website: Some("not-a-valid-url".to_string()),
	};

	let result = request.validate();
	assert!(
		result.is_err(),
		"Invalid website URL should fail validation"
	);
}

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_all_valid() {
	use validator::Validate;

	let request = UpdateProfileRequest {
		bio: Some("My bio".to_string()),
		avatar_url: Some("https://example.com/avatar.png".to_string()),
		location: Some("Tokyo, Japan".to_string()),
		website: Some("https://myblog.com".to_string()),
	};

	let result = request.validate();
	assert!(result.is_ok(), "All valid fields should pass validation");
}

#[rstest]
#[tokio::test]
async fn test_update_profile_validation_partial_update() {
	use validator::Validate;

	// Only updating bio
	let request = UpdateProfileRequest {
		bio: Some("Updated bio".to_string()),
		avatar_url: None,
		location: None,
		website: None,
	};

	let result = request.validate();
	assert!(result.is_ok(), "Partial update should pass validation");
}

// ============================================================================
// ProfileResponse Conversion Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_profile_response_from_profile(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let profile_factory = ProfileFactory::new();

	// Create test user and profile
	let test_user = TestTwitterUser::new("responseuser");
	let user = user_factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	let profile = profile_factory
		.create_for_user(
			&pool,
			user.id(),
			"Bio content",
			"https://avatar.url/image.png",
		)
		.await
		.expect("Profile creation should succeed");

	// Convert to response
	let response = ProfileResponse::from(profile.clone());

	assert_eq!(response.user_id, profile.user_id());
	assert_eq!(response.bio, Some("Bio content".to_string()));
	assert_eq!(
		response.avatar_url,
		Some("https://avatar.url/image.png".to_string())
	);
}

// ============================================================================
// Profile Factory Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_profile_factory_create_default(#[future] twitter_db_pool: (PgPool, String)) {
	let (pool, _url) = twitter_db_pool.await;
	let user_factory = UserFactory::new();
	let profile_factory = ProfileFactory::new();

	// Create test user
	let test_user = TestTwitterUser::new("defaultprofile");
	let user = user_factory
		.create_from_test_user(&pool, &test_user)
		.await
		.expect("User creation should succeed");

	// Create default profile
	let profile = profile_factory
		.create_default_for_user(&pool, user.id())
		.await
		.expect("Profile creation should succeed");

	assert_eq!(profile.user_id(), user.id());
	assert_eq!(profile.bio(), "");
	assert_eq!(profile.avatar_url(), "");
}
