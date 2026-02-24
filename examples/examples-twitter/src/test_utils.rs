//! Test utilities module for examples-twitter application.
//!
//! Provides modular fixtures and factories for testing server functions,
//! components, and WebSocket handlers with TestContainers PostgreSQL.
//!
//! # Architecture
//!
//! This module wraps `reinhardt-test` fixtures with examples-twitter specific
//! data injection:
//!
//! - **Fixtures**: rstest fixtures for database, users, and test contexts
//! - **Factories**: Test data factories for creating Users, Profiles, Tweets, etc.
//!
//! # Usage
//!
//! ```rust,ignore
//! use examples_twitter::test_utils::fixtures::*;
//! use examples_twitter::test_utils::factories::*;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_with_user(
//!     #[future] twitter_test_context: TwitterTestContext,
//!     user_factory: UserFactory,
//! ) {
//!     let ctx = twitter_test_context.await;
//!     let user = user_factory.create(&ctx.db).await;
//!     // Test with user
//! }
//! ```

#[cfg(test)]
pub mod factories;

#[cfg(test)]
pub mod fixtures;

#[cfg(test)]
pub use factories::*;

#[cfg(test)]
pub use fixtures::*;
