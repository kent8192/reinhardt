//! Migration provider for examples-twitter application.
//!
//! This module provides all migrations for the Twitter example application,
//! enabling TestContainers-based tests to apply migrations automatically.

use reinhardt::db::migrations::{Migration, MigrationProvider};

// Migration modules referenced via #[path] attribute to external migrations/ directory
#[path = "../migrations/auth/0001_initial.rs"]
mod auth_0001_initial;

#[path = "../migrations/tweet/0001_initial.rs"]
mod tweet_0001_initial;

#[path = "../migrations/profile/0001_initial.rs"]
mod profile_0001_initial;

#[path = "../migrations/dm/0001_initial.rs"]
mod dm_0001_initial;

/// Migration provider for the Twitter example application.
///
/// Provides migrations for all apps in dependency order:
/// 1. auth - User model (base)
/// 2. tweet - Tweet model (depends on auth)
/// 3. profile - Profile model (depends on auth)
/// 4. dm - DM Room and Message models (depends on auth)
pub struct TwitterMigrations;

impl MigrationProvider for TwitterMigrations {
	fn migrations() -> Vec<Migration> {
		vec![
			// Auth migrations first (no dependencies)
			auth_0001_initial::migration(),
			// Tweet migrations (depends on auth_user)
			tweet_0001_initial::migration(),
			// Profile migrations (depends on auth_user)
			profile_0001_initial::migration(),
			// DM migrations (depends on auth_user)
			dm_0001_initial::migration(),
		]
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_migrations_count() {
		let migrations = TwitterMigrations::migrations();
		assert_eq!(migrations.len(), 4);
	}

	#[rstest]
	fn test_migrations_order() {
		let migrations = TwitterMigrations::migrations();
		assert_eq!(migrations[0].app_label, "auth");
		assert_eq!(migrations[1].app_label, "tweet");
		assert_eq!(migrations[2].app_label, "profile");
		assert_eq!(migrations[3].app_label, "dm");
	}
}
