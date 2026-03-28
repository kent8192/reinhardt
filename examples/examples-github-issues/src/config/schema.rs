//! GraphQL schema configuration with singleton pattern
//!
//! This module provides a singleton GraphQL schema that is created once
//! and reused for all requests, improving performance and consistency.

use std::env;
use std::sync::{Arc, LazyLock};

use crate::apps::auth::views::UserStorage;
use crate::apps::issues::views::{IssueEventBroadcaster, IssueStorage};
use crate::apps::projects::views::{ProjectMemberStorage, ProjectStorage};

use super::urls::{AppSchema, Mutation, Query, Subscription};

/// Create the unified GraphQL schema with all context data
fn create_schema_internal() -> AppSchema {
	let user_storage = UserStorage::new();
	let issue_storage = IssueStorage::new();
	let project_storage = ProjectStorage::new();
	let member_storage = ProjectMemberStorage::new();
	let broadcaster = IssueEventBroadcaster::new();

	// Load JWT secret from environment variable with fallback default
	let jwt_secret = env::var("JWT_SECRET")
		.unwrap_or_else(|_| "your-secret-key-change-in-production".to_string())
		.into_bytes();
	let jwt_auth = reinhardt::JwtAuth::new(&jwt_secret);

	reinhardt::graphql::Schema::build(
		Query::default(),
		Mutation::default(),
		Subscription::default(),
	)
	.data(user_storage)
	.data(issue_storage)
	.data(project_storage)
	.data(member_storage)
	.data(broadcaster)
	.data(jwt_auth)
	.finish()
}

/// Singleton GraphQL schema instance
///
/// The schema is created lazily on first access and reused for all subsequent requests.
/// This improves performance by avoiding schema construction overhead per request.
static SCHEMA: LazyLock<Arc<AppSchema>> = LazyLock::new(|| Arc::new(create_schema_internal()));

/// Get a reference to the singleton schema
pub fn get_schema() -> Arc<AppSchema> {
	SCHEMA.clone()
}
