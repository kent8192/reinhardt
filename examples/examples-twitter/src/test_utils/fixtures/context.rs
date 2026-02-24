//! Test context fixtures for examples-twitter.
//!
//! Provides integrated test contexts that combine database, user, and session
//! for testing server functions and handlers.

use reinhardt::DatabaseConnection;
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;

use super::database::twitter_db_pool;
use super::users::{TestTwitterUser, twitter_user};
use crate::apps::auth::shared::types::SessionData;
use crate::test_utils::factories::UserFactory;

/// Test context for server function tests.
///
/// Combines database connection, optional authenticated user, and session data
/// for comprehensive server function testing.
pub struct TwitterTestContext {
	/// Database pool for direct SQL operations
	pub pool: PgPool,
	/// Database URL for connection
	pub url: String,
	/// Database connection for ORM operations
	pub db: Arc<DatabaseConnection>,
	/// Optional authenticated user (None for anonymous tests)
	pub user: Option<TestTwitterUser>,
	/// Session data (None for unauthenticated tests)
	pub session: Option<SessionData>,
}

impl TwitterTestContext {
	/// Create a new unauthenticated test context.
	pub async fn new(pool: PgPool, url: String) -> Self {
		let connection = DatabaseConnection::connect_postgres(&url)
			.await
			.expect("Failed to connect to PostgreSQL");
		let db = Arc::new(connection);
		Self {
			pool,
			url,
			db,
			user: None,
			session: None,
		}
	}

	/// Create a test context with an authenticated user.
	///
	/// The user is inserted into the database and a session is created.
	pub async fn authenticated(pool: PgPool, url: String, user: TestTwitterUser) -> Self {
		let connection = DatabaseConnection::connect_postgres(&url)
			.await
			.expect("Failed to connect to PostgreSQL");
		let db = Arc::new(connection);

		// Insert user into database
		let factory = UserFactory::default();
		factory
			.create_from_test_user(&pool, &user)
			.await
			.expect("Failed to create test user in database");

		let session = user.to_session_data();

		Self {
			pool,
			url,
			db,
			user: Some(user),
			session: Some(session),
		}
	}

	/// Get the session data, panicking if not authenticated.
	pub fn session(&self) -> &SessionData {
		self.session
			.as_ref()
			.expect("Test context is not authenticated")
	}

	/// Get the user, panicking if not authenticated.
	pub fn user(&self) -> &TestTwitterUser {
		self.user
			.as_ref()
			.expect("Test context is not authenticated")
	}

	/// Check if the context has an authenticated user.
	pub fn is_authenticated(&self) -> bool {
		self.session.is_some()
	}
}

/// rstest fixture providing an unauthenticated test context.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::twitter_test_context;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_anonymous(#[future] twitter_test_context: TwitterTestContext) {
///     let ctx = twitter_test_context.await;
///     assert!(!ctx.is_authenticated());
/// }
/// ```
#[fixture]
pub async fn twitter_test_context(
	#[future] twitter_db_pool: (PgPool, String),
) -> TwitterTestContext {
	let (pool, url) = twitter_db_pool.await;
	TwitterTestContext::new(pool, url).await
}

/// rstest fixture providing an authenticated test context.
///
/// Creates a test user in the database and provides session data.
///
/// # Example
///
/// ```rust,ignore
/// use examples_twitter::test_utils::fixtures::authenticated_twitter_context;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_authenticated(#[future] authenticated_twitter_context: TwitterTestContext) {
///     let ctx = authenticated_twitter_context.await;
///     assert!(ctx.is_authenticated());
///     assert_eq!(ctx.user().username, "testuser");
/// }
/// ```
#[fixture]
pub async fn authenticated_twitter_context(
	#[future] twitter_db_pool: (PgPool, String),
	twitter_user: TestTwitterUser,
) -> TwitterTestContext {
	let (pool, url) = twitter_db_pool.await;
	TwitterTestContext::authenticated(pool, url, twitter_user).await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[rstest]
	#[tokio::test]
	async fn test_twitter_test_context_is_unauthenticated(
		#[future] twitter_test_context: TwitterTestContext,
	) {
		let ctx = twitter_test_context.await;
		assert!(!ctx.is_authenticated());
	}

	#[rstest]
	#[tokio::test]
	async fn test_authenticated_twitter_context_has_user(
		#[future] authenticated_twitter_context: TwitterTestContext,
	) {
		let ctx = authenticated_twitter_context.await;
		assert!(ctx.is_authenticated());
		assert_eq!(ctx.user().username, "testuser");
		assert_eq!(ctx.session().username, "testuser");
	}

	#[rstest]
	#[tokio::test]
	async fn test_context_db_connection_works(
		#[future] authenticated_twitter_context: TwitterTestContext,
	) {
		let ctx = authenticated_twitter_context.await;

		// Verify user exists in database
		let result =
			sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM auth_user WHERE username = $1")
				.bind(&ctx.user().username)
				.fetch_one(&ctx.pool)
				.await
				.expect("Query should succeed");

		assert_eq!(result, 1, "User should exist in database");
	}
}
