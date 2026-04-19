//! Integration tests for GraphQL dependency injection
//!
//! These tests verify that the `#[graphql_handler]` macro correctly integrates
//! with the DI system.

#![cfg(feature = "di")]

use async_graphql::{Context, EmptyMutation, EmptySubscription, ID, Object, Result, Schema};
use reinhardt_di::{
	DependencyScope, DiError, Injectable, InjectionContext, SingletonScope, global_registry,
};
use reinhardt_graphql::{SchemaBuilderExt, graphql_handler};
use rstest::*;
use std::sync::{Arc, Mutex};

/// Mock database connection for testing
#[derive(Clone)]
struct MockDatabase {
	calls: Arc<Mutex<Vec<String>>>,
}

impl MockDatabase {
	fn new() -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
		}
	}

	async fn fetch_user(&self, user_id: &str) -> Result<User> {
		self.calls
			.lock()
			.unwrap()
			.push(format!("fetch_user({})", user_id));
		Ok(User {
			id: user_id.to_string(),
			name: format!("User {}", user_id),
			email: format!("user{}@example.com", user_id),
		})
	}

	async fn fetch_users(&self, limit: i32) -> Result<Vec<User>> {
		self.calls
			.lock()
			.unwrap()
			.push(format!("fetch_users({})", limit));
		Ok((1..=limit)
			.map(|i| User {
				id: i.to_string(),
				name: format!("User {}", i),
				email: format!("user{}@example.com", i),
			})
			.collect())
	}
}

#[async_trait::async_trait]
impl Injectable for MockDatabase {
	async fn inject(_ctx: &InjectionContext) -> Result<Self, DiError> {
		Ok(MockDatabase::new())
	}
}

/// Mock cache for testing
#[derive(Clone)]
struct MockCache {
	calls: Arc<Mutex<Vec<String>>>,
}

impl MockCache {
	fn new() -> Self {
		Self {
			calls: Arc::new(Mutex::new(Vec::new())),
		}
	}

	async fn get(&self, key: &str) -> Option<User> {
		self.calls.lock().unwrap().push(format!("get({})", key));
		None
	}
}

#[async_trait::async_trait]
impl Injectable for MockCache {
	async fn inject(_ctx: &InjectionContext) -> Result<Self, DiError> {
		Ok(MockCache::new())
	}
}

/// GraphQL User type
#[derive(Clone)]
struct User {
	id: String,
	name: String,
	email: String,
}

#[Object]
impl User {
	async fn id(&self) -> &str {
		&self.id
	}

	async fn name(&self) -> &str {
		&self.name
	}

	async fn email(&self) -> &str {
		&self.email
	}
}

/// Register mock types in the global registry for DI resolution.
fn register_mock_types() {
	let registry = global_registry();
	if !registry.is_registered::<MockDatabase>() {
		registry.register_async::<MockDatabase, _, _>(DependencyScope::Request, |_ctx| async {
			Ok(MockDatabase::new())
		});
	}
	if !registry.is_registered::<MockCache>() {
		registry.register_async::<MockCache, _, _>(DependencyScope::Request, |_ctx| async {
			Ok(MockCache::new())
		});
	}
}

/// Fixture: Injection context with database and cache
#[fixture]
fn injection_context_with_database() -> Arc<InjectionContext> {
	register_mock_types();
	let singleton_scope = SingletonScope::new();
	Arc::new(InjectionContext::builder(singleton_scope).build())
}

/// GraphQL Query root
pub struct Query;

#[Object]
impl Query {
	async fn user(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
		user_handler(ctx, id).await
	}

	async fn users(&self, ctx: &Context<'_>, limit: Option<i32>) -> Result<Vec<User>> {
		users_handler(ctx, limit).await
	}

	async fn user_with_cache(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
		user_with_cache_handler(ctx, id).await
	}

	async fn user_uncached(&self, ctx: &Context<'_>, id: ID) -> Result<User> {
		user_uncached_handler(ctx, id).await
	}
}

#[graphql_handler]
async fn user_handler(_ctx: &Context<'_>, id: ID, #[inject] db: MockDatabase) -> Result<User> {
	let user = db.fetch_user(&id).await?;
	Ok(user)
}

#[graphql_handler]
async fn users_handler(
	_ctx: &Context<'_>,
	limit: Option<i32>,
	#[inject] db: MockDatabase,
) -> Result<Vec<User>> {
	let limit = limit.unwrap_or(10);
	let users = db.fetch_users(limit).await?;
	Ok(users)
}

#[graphql_handler]
async fn user_with_cache_handler(
	_ctx: &Context<'_>,
	id: ID,
	#[inject] db: MockDatabase,
	#[inject] cache: MockCache,
) -> Result<User> {
	// Check cache first
	if let Some(user) = cache.get(&id).await {
		return Ok(user);
	}

	// Fetch from database
	let user = db.fetch_user(&id).await?;
	Ok(user)
}

#[graphql_handler]
async fn user_uncached_handler(
	_ctx: &Context<'_>,
	id: ID,
	#[inject(cache = false)] db: MockDatabase,
) -> Result<User> {
	let user = db.fetch_user(&id).await?;
	Ok(user)
}

#[rstest]
#[tokio::test]
async fn test_graphql_handler_basic_di(injection_context_with_database: Arc<InjectionContext>) {
	// Setup DI context from fixture
	let injection_ctx = injection_context_with_database;

	// Build schema with DI context
	let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
		.with_di_context(injection_ctx)
		.finish();

	// Execute query
	let query = r#"{ user(id: "123") { id name email } }"#;
	let result = schema.execute(query).await;

	// Verify
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert_eq!(data["user"]["id"], "123");
	assert_eq!(data["user"]["name"], "User 123");
	assert_eq!(data["user"]["email"], "user123@example.com");
}

#[rstest]
#[tokio::test]
async fn test_graphql_handler_multiple_dependencies(
	injection_context_with_database: Arc<InjectionContext>,
) {
	// Setup DI context from fixture
	let injection_ctx = injection_context_with_database;

	// Build schema with DI context
	let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
		.with_di_context(injection_ctx)
		.finish();

	// Execute query
	let query = r#"{ userWithCache(id: "456") { id name email } }"#;
	let result = schema.execute(query).await;

	// Verify
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert_eq!(data["userWithCache"]["id"], "456");
	assert_eq!(data["userWithCache"]["name"], "User 456");
}

#[rstest]
#[tokio::test]
async fn test_graphql_handler_list_query(injection_context_with_database: Arc<InjectionContext>) {
	// Setup DI context from fixture
	let injection_ctx = injection_context_with_database;

	// Build schema with DI context
	let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
		.with_di_context(injection_ctx)
		.finish();

	// Execute query
	let query = r#"{ users(limit: 3) { id name } }"#;
	let result = schema.execute(query).await;

	// Verify
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	let users = data["users"].as_array().unwrap();
	assert_eq!(users.len(), 3);
	assert_eq!(users[0]["id"], "1");
	assert_eq!(users[1]["id"], "2");
	assert_eq!(users[2]["id"], "3");
}

#[rstest]
#[tokio::test]
async fn test_graphql_handler_missing_di_context() {
	// Build schema WITHOUT DI context
	let schema = Schema::build(Query, EmptyMutation, EmptySubscription).finish();

	// Execute query - should fail
	let query = r#"{ user(id: "789") { id name email } }"#;
	let result = schema.execute(query).await;

	// Verify
	assert!(!result.errors.is_empty());
	let error = &result.errors[0];
	assert!(error.message.contains("DI context not set"));
}

#[rstest]
#[tokio::test]
async fn test_graphql_handler_cache_control(
	injection_context_with_database: Arc<InjectionContext>,
) {
	// Setup DI context from fixture
	let injection_ctx = injection_context_with_database;

	// Build schema with DI context
	let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
		.with_di_context(injection_ctx)
		.finish();

	// Execute query with uncached dependency
	let query = r#"{ userUncached(id: "999") { id name } }"#;
	let result = schema.execute(query).await;

	// Verify
	assert!(result.errors.is_empty());
	let data = result.data.into_json().unwrap();
	assert_eq!(data["userUncached"]["id"], "999");
}
