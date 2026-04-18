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
/// This fixture composes on `graphql_schema_fixture`, which owns the shared
/// testcontainer-backed `PgPool`. The pool is returned alongside the schema
/// and seeded user so dependent fixtures (e.g. `post_model_fixture`) can
/// reuse the already-seeded user without duplicating insert logic.
#[fixture]
async fn user_model_fixture(
	#[future] graphql_schema_fixture: (Schema<Query, Mutation, EmptySubscription>, Arc<PgPool>),
) -> (
	Schema<Query, Mutation, EmptySubscription>,
	Arc<PgPool>,
	User,
) {
	let (schema, pool) = graphql_schema_fixture.await;

	// Create user using reinhardt-query (not raw SQL)
	let user = create_test_user_with_query(&pool).await;

	(schema, pool, user)
}

/// Creates a test post with author relationship.
///
/// Composes on `user_model_fixture` so user seeding is not duplicated and
/// the post is always anchored to the same user instance the test would
/// observe through `user_model_fixture`.
#[fixture]
async fn post_model_fixture(
	#[future] user_model_fixture: (
		Schema<Query, Mutation, EmptySubscription>,
		Arc<PgPool>,
		User,
	),
) -> (Schema<Query, Mutation, EmptySubscription>, User, Post) {
	let (schema, pool, user) = user_model_fixture.await;

	let post = create_test_post_with_query(&pool, user.id).await;

	(schema, user, post)
}

// ============================================================================
// Helper Functions using reinhardt-query
// ============================================================================

/// Create a test user in the database using reinhardt-query.
///
/// Returns the model populated with the actual `SERIAL` id assigned by
/// Postgres via `RETURNING id`. Using a hard-coded id collides on the
/// second insert and breaks any test that reads the row back by id.
async fn create_test_user_with_query(pool: &PgPool) -> User {
	let name = "Test User";
	let email = "test@example.com";

	// Use reinhardt-query to build SQL (not raw strings); append `RETURNING id`
	// so we can fetch the actual generated primary key.
	let mut insert_stmt = Query::insert();
	let insert_query = insert_stmt
		.into_table(Tables::Users.into_iden())
		.columns([User::name(), User::email(), User::is_active()])
		.values([name.into(), email.into(), true.into()])
		.unwrap()
		.to_string(PostgresQueryBuilder::new());
	let insert_with_returning = format!("{insert_query} RETURNING id");

	let id: i32 = sqlx::query_scalar(&insert_with_returning)
		.fetch_one(pool)
		.await
		.expect("Failed to create test user");

	User::new(id, name.to_string(), email.to_string())
}

/// Create a test post in the database using reinhardt-query.
///
/// Returns the model populated with the actual generated `SERIAL` id.
async fn create_test_post_with_query(pool: &PgPool, author_id: i32) -> Post {
	let title = "Test Post";
	let content = "Test content";

	let mut insert_stmt = Query::insert();
	let insert_query = insert_stmt
		.into_table(Tables::Posts.into_iden())
		.columns([Post::title(), Post::content(), Post::author_id()])
		.values([title.into(), content.into(), author_id.into()])
		.unwrap()
		.to_string(PostgresQueryBuilder::new());
	let insert_with_returning = format!("{insert_query} RETURNING id");

	let id: i32 = sqlx::query_scalar(&insert_with_returning)
		.fetch_one(pool)
		.await
		.expect("Failed to create test post");

	Post::new(id, title.to_string(), content.to_string(), author_id)
}

/// Creates multiple test users for batch testing.
#[fixture]
async fn multiple_users_fixture(
	#[future] graphql_schema_fixture: (Schema<Query, Mutation, EmptySubscription>, Arc<PgPool>),
) -> (Schema<Query, Mutation, EmptySubscription>, Vec<User>) {
	let (schema, pool) = graphql_schema_fixture.await;

	let users = vec![
		create_test_user_with_query(&pool).await,
		create_test_user_with_query(&pool).await,
		create_test_user_with_query(&pool).await,
	];

	(schema, users)
}
