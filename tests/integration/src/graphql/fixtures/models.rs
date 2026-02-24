//! Database model fixtures for GraphQL testing
//!
//! This module provides model definitions and fixtures for GraphQL testing
//! using `#[model(...)]` macro from `reinhardt-orm`.

use crate::prelude::*;
use reinhardt_db::orm::Model;
use reinhardt_query::prelude::{Iden, IntoIden, PostgresQueryBuilder, Query, QueryStatementBuilder};
use sqlx::PgPool;
use std::sync::Arc;

// ============================================================================
// Model Definitions using #[model(...)] macro
// ============================================================================

/// User model for GraphQL testing.
///
/// This model uses the `#[model(...)]` macro which automatically derives
/// `Model` trait. The `Model::new()` function is used for initialization.
#[model(table_name = "users")]
pub struct User {
	#[field(primary_key = true)]
	pub id: i32,

	#[field(max_length = 255)]
	pub name: String,

	#[field(max_length = 255)]
	pub email: String,

	#[field(default = true)]
	pub is_active: bool,
}

impl User {
	/// Create a new User instance using `Model::new()` pattern.
	pub fn new(id: i32, name: String, email: String) -> Self {
		Self {
			id,
			name,
			email,
			is_active: true,
		}
	}
}

/// Post model for GraphQL testing with relationship to User.
#[model(table_name = "posts")]
pub struct Post {
	#[field(primary_key = true)]
	pub id: i32,

	#[field(max_length = 255)]
	pub title: String,

	pub content: String,

	#[field(foreign_key = "users.id")]
	pub author_id: i32,
}

impl Post {
	/// Create a new Post instance.
	pub fn new(id: i32, title: String, content: String, author_id: i32) -> Self {
		Self {
			id,
			title,
			content,
			author_id,
		}
	}
}

// ============================================================================
// Table Definitions for reinhardt-query
// ============================================================================

/// Table name identifiers for reinhardt-query
#[derive(Debug)]
pub enum Tables {
	Users,
	Posts,
}

impl Iden for Tables {
	fn unquoted(&self, s: &mut dyn std::fmt::Write) {
		write!(
			s,
			"{}",
			match self {
				Tables::Users => "users",
				Tables::Posts => "posts",
			}
		)
		.unwrap();
	}
}

// ============================================================================
// Fixtures
// ============================================================================

/// Creates a test user in the database and returns it with the schema.
///
/// This fixture demonstrates the pattern of wrapping generic database fixtures
/// with specialized test data insertion.
#[fixture]
async fn user_model_fixture(
	#[future] graphql_schema_fixture: Schema<Query, Mutation, EmptySubscription>,
) -> (Schema<Query, Mutation, EmptySubscription>, User) {
	let schema = graphql_schema_fixture.await;

	// Get database pool from schema context (simplified - in real implementation
	// we would extract it from schema data)
	let pool = get_pool_from_test_container().await;

	// Create user using reinhardt-query (not raw SQL)
	let user = create_test_user_with_query(&pool).await;

	(schema, user)
}

/// Creates a test post with author relationship.
#[fixture]
async fn post_model_fixture(
	#[future] user_model_fixture: (Schema<Query, Mutation, EmptySubscription>, User),
) -> (Schema<Query, Mutation, EmptySubscription>, User, Post) {
	let (schema, user) = user_model_fixture.await;
	let pool = get_pool_from_test_container().await;

	// Create post using reinhardt-query
	let post = create_test_post_with_query(&pool, user.id).await;

	(schema, user, post)
}

// ============================================================================
// Helper Functions using reinhardt-query
// ============================================================================

/// Get database pool from test container (simplified implementation).
///
/// In a real implementation, this would extract the pool from the schema
/// or use a shared test container fixture.
// Allow: pool extraction is a placeholder for fixture pattern (permanently excluded)
#[allow(clippy::unimplemented)]
async fn get_pool_from_test_container() -> Arc<PgPool> {
	use reinhardt_test::fixtures::postgres_container;
	use rstest::*;

	// This is a simplified implementation - in real tests we would
	// use the actual postgres_container fixture
	unimplemented!("Need to implement pool extraction from test container")
}

/// Create a test user in the database using reinhardt-query.
async fn create_test_user_with_query(pool: &PgPool) -> User {
	// Use reinhardt-query to build SQL (not raw strings)
	let mut insert_stmt = Query::insert();
	let insert_query = insert_stmt
		.into_table(Tables::Users.into_iden())
		.columns([User::name(), User::email(), User::is_active()])
		.values(["Test User".into(), "test@example.com".into(), true.into()])
		.unwrap()
		.to_string(PostgresQueryBuilder::new());

	// Execute the query
	sqlx::query(&insert_query)
		.execute(pool)
		.await
		.expect("Failed to create test user");

	// Return model instance using Model::new()
	User::new(1, "Test User".to_string(), "test@example.com".to_string())
}

/// Create a test post in the database using reinhardt-query.
async fn create_test_post_with_query(pool: &PgPool, author_id: i32) -> Post {
	let mut insert_stmt = Query::insert();
	let insert_query = insert_stmt
		.into_table(Tables::Posts.into_iden())
		.columns([Post::title(), Post::content(), Post::author_id()])
		.values(["Test Post".into(), "Test content".into(), author_id.into()])
		.unwrap()
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&insert_query)
		.execute(pool)
		.await
		.expect("Failed to create test post");

	Post::new(
		1,
		"Test Post".to_string(),
		"Test content".to_string(),
		author_id,
	)
}

/// Creates multiple test users for batch testing.
#[fixture]
async fn multiple_users_fixture(
	#[future] graphql_schema_fixture: Schema<Query, Mutation, EmptySubscription>,
) -> (Schema<Query, Mutation, EmptySubscription>, Vec<User>) {
	let schema = graphql_schema_fixture.await;
	let pool = get_pool_from_test_container().await;

	let users = vec![
		create_test_user_with_query(&pool).await,
		create_test_user_with_query(&pool).await,
		create_test_user_with_query(&pool).await,
	];

	(schema, users)
}
