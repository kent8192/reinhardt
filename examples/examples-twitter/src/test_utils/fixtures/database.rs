//! Database fixtures for examples-twitter tests.
//!
//! Provides TestContainers PostgreSQL with automatic migration application.

use reinhardt::DatabaseConnection;
use reinhardt::db::migrations::executor::DatabaseMigrationExecutor;
use reinhardt::db::migrations::{DatabaseConnection as MigrationsConnection, MigrationProvider};
use reinhardt::test::fixtures::shared_postgres::get_test_pool_with_orm;
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;

use crate::migrations::TwitterMigrations;

/// Applies all Twitter migrations to the given database using DatabaseMigrationExecutor.
///
/// Executes migrations in dependency order:
/// 1. auth (User model)
/// 2. tweet (Tweet model)
/// 3. profile (Profile model)
/// 4. dm (DM Room/Message models)
pub async fn apply_twitter_migrations(url: &str) {
	let connection = MigrationsConnection::connect_postgres(url)
		.await
		.expect("Failed to connect to PostgreSQL for migrations");
	let migrations = TwitterMigrations::migrations();
	let mut executor = DatabaseMigrationExecutor::new(connection);
	executor
		.apply_migrations(&migrations)
		.await
		.expect("Failed to apply Twitter migrations");
}

/// rstest fixture providing an isolated PostgreSQL database with migrations applied.
///
/// Each test gets its own database cloned from template with all Twitter
/// migrations applied. Returns both the PgPool and the database URL.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::twitter_db_pool;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_pool(#[future] twitter_db_pool: (PgPool, String)) {
///     let (pool, _url) = twitter_db_pool.await;
///     // Use pool for raw SQL operations
/// }
/// ```
#[fixture]
pub async fn twitter_db_pool() -> (PgPool, String) {
	let (pool, url) = get_test_pool_with_orm().await;
	apply_twitter_migrations(&url).await;
	(pool, url)
}

/// rstest fixture providing a DatabaseConnection for ORM operations.
///
/// Wraps the PostgreSQL pool in a `DatabaseConnection` for use with
/// Reinhardt ORM methods.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::twitter_db_connection;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_with_orm(#[future] twitter_db_connection: Arc<DatabaseConnection>) {
///     let db = twitter_db_connection.await;
///     // Use db with ORM methods
/// }
/// ```
#[fixture]
pub async fn twitter_db_connection(
	#[future] twitter_db_pool: (PgPool, String),
) -> Arc<DatabaseConnection> {
	let (_pool, url) = twitter_db_pool.await;
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("Failed to connect to PostgreSQL");
	Arc::new(connection)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_twitter_db_pool_applies_migrations(#[future] twitter_db_pool: (PgPool, String)) {
		let (pool, _url) = twitter_db_pool.await;

		// Verify auth_user table exists
		let result = sqlx::query("SELECT COUNT(*) FROM auth_user")
			.fetch_one(&pool)
			.await;
		assert!(result.is_ok(), "auth_user table should exist");

		// Verify tweet_tweet table exists
		let result = sqlx::query("SELECT COUNT(*) FROM tweet_tweet")
			.fetch_one(&pool)
			.await;
		assert!(result.is_ok(), "tweet_tweet table should exist");

		// Verify profile_profile table exists
		let result = sqlx::query("SELECT COUNT(*) FROM profile_profile")
			.fetch_one(&pool)
			.await;
		assert!(result.is_ok(), "profile_profile table should exist");

		// Verify dm_room table exists
		let result = sqlx::query("SELECT COUNT(*) FROM dm_room")
			.fetch_one(&pool)
			.await;
		assert!(result.is_ok(), "dm_room table should exist");

		// Verify dm_message table exists
		let result = sqlx::query("SELECT COUNT(*) FROM dm_message")
			.fetch_one(&pool)
			.await;
		assert!(result.is_ok(), "dm_message table should exist");
	}
}
