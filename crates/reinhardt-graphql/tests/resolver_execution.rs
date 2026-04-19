//! GraphQL resolver execution integration tests
//!
//! Tests Query/Mutation resolver execution with database integration,
//! complex nested queries, error handling, and GraphQL error responses.

use async_graphql::{Context, EmptySubscription, ID, Object, Result as GqlResult, Schema};
use reinhardt_graphql::{CreateUserInput, User, UserStorage};
use rstest::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Fixtures for resolver execution tests
#[fixture]
fn user_storage() -> UserStorage {
	UserStorage::new()
}

#[fixture]
fn post_storage() -> PostStorage {
	PostStorage::new()
}

#[fixture]
fn schema_fixture(user_storage: UserStorage, post_storage: PostStorage) -> TestSchema {
	create_test_schema(user_storage, post_storage)
}

/// Extended User type with posts relationship (for nested query tests)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Post {
	id: ID,
	title: String,
	content: String,
	author_id: ID,
}

#[Object]
impl Post {
	async fn id(&self) -> &ID {
		&self.id
	}

	async fn title(&self) -> &str {
		&self.title
	}

	async fn content(&self) -> &str {
		&self.content
	}

	async fn author(&self, ctx: &Context<'_>) -> GqlResult<Option<User>> {
		let storage = ctx.data::<UserStorage>()?;
		Ok(storage.get_user(self.author_id.as_ref()).await)
	}
}

/// Post storage (in-memory)
#[derive(Clone)]
struct PostStorage {
	posts: Arc<RwLock<HashMap<String, Post>>>,
}

impl PostStorage {
	fn new() -> Self {
		Self {
			posts: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	async fn add_post(&self, post: Post) {
		self.posts.write().await.insert(post.id.to_string(), post);
	}

	async fn get_post(&self, id: &str) -> Option<Post> {
		self.posts.read().await.get(id).cloned()
	}

	async fn get_posts_by_author(&self, author_id: &str) -> Vec<Post> {
		self.posts
			.read()
			.await
			.values()
			.filter(|p| p.author_id.as_ref() == author_id)
			.cloned()
			.collect()
	}
}

/// Extended Query root with posts
struct ExtendedQuery;

#[Object]
impl ExtendedQuery {
	async fn user(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<User>> {
		let storage = ctx.data::<UserStorage>()?;
		Ok(storage.get_user(id.as_ref()).await)
	}

	async fn users(&self, ctx: &Context<'_>) -> GqlResult<Vec<User>> {
		let storage = ctx.data::<UserStorage>()?;
		Ok(storage.list_users().await)
	}

	async fn post(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<Post>> {
		let storage = ctx.data::<PostStorage>()?;
		Ok(storage.get_post(id.as_ref()).await)
	}

	async fn posts_by_author(&self, ctx: &Context<'_>, author_id: ID) -> GqlResult<Vec<Post>> {
		let storage = ctx.data::<PostStorage>()?;
		Ok(storage.get_posts_by_author(author_id.as_ref()).await)
	}
}

/// Extended Mutation root
struct ExtendedMutation;

#[Object]
impl ExtendedMutation {
	async fn create_user(&self, ctx: &Context<'_>, input: CreateUserInput) -> GqlResult<User> {
		let storage = ctx.data::<UserStorage>()?;

		let user = User {
			id: ID::from(uuid::Uuid::now_v7().to_string()),
			name: input.name,
			email: input.email,
			active: true,
		};

		storage.add_user(user.clone()).await;
		Ok(user)
	}

	async fn create_post(
		&self,
		ctx: &Context<'_>,
		title: String,
		content: String,
		author_id: ID,
	) -> GqlResult<Post> {
		let post_storage = ctx.data::<PostStorage>()?;
		let user_storage = ctx.data::<UserStorage>()?;

		// Validate author exists
		if user_storage.get_user(author_id.as_ref()).await.is_none() {
			return Err("Author not found".into());
		}

		let post = Post {
			id: ID::from(uuid::Uuid::now_v7().to_string()),
			title,
			content,
			author_id,
		};

		post_storage.add_post(post.clone()).await;
		Ok(post)
	}
}

type TestSchema = Schema<ExtendedQuery, ExtendedMutation, EmptySubscription>;

fn create_test_schema(user_storage: UserStorage, post_storage: PostStorage) -> TestSchema {
	Schema::build(ExtendedQuery, ExtendedMutation, EmptySubscription)
		.data(user_storage)
		.data(post_storage)
		.finish()
}

/// Test: Basic Query resolver execution
#[rstest]
#[tokio::test]
async fn test_query_resolver_basic(user_storage: UserStorage, post_storage: PostStorage) {
	let user = User {
		id: ID::from("user-1"),
		name: "Alice".to_string(),
		email: "alice@example.com".to_string(),
		active: true,
	};
	user_storage.add_user(user).await;

	let schema = create_test_schema(user_storage, post_storage);

	let query = r#"
        {
            user(id: "user-1") {
                id
                name
                email
                active
            }
        }
    "#;

	let result = schema.execute(query).await;
	assert!(result.errors.is_empty(), "Should not have errors");

	let data = result.data.into_json().unwrap();
	assert_eq!(data["user"]["id"], "user-1");
	assert_eq!(data["user"]["name"], "Alice");
	assert_eq!(data["user"]["email"], "alice@example.com");
	assert!(data["user"]["active"].as_bool().unwrap());
}

/// Test: Mutation resolver execution
#[rstest]
#[tokio::test]
async fn test_mutation_resolver_execution(user_storage: UserStorage, post_storage: PostStorage) {
	let schema = create_test_schema(user_storage.clone(), post_storage);

	let mutation = r#"
        mutation {
            createUser(input: { name: "Bob", email: "bob@example.com" }) {
                id
                name
                email
                active
            }
        }
    "#;

	let result = schema.execute(mutation).await;
	assert!(result.errors.is_empty(), "Should not have errors");

	let data = result.data.into_json().unwrap();
	assert_eq!(data["createUser"]["name"], "Bob");
	assert_eq!(data["createUser"]["email"], "bob@example.com");
	assert!(data["createUser"]["active"].as_bool().unwrap());

	// Verify data persisted to storage
	let user_id = data["createUser"]["id"].as_str().unwrap();
	let stored_user = user_storage.get_user(user_id).await;
	assert!(stored_user.is_some());
	assert_eq!(stored_user.unwrap().name, "Bob");
}

/// Test: Complex nested query
#[rstest]
#[tokio::test]
async fn test_complex_nested_query(user_storage: UserStorage, post_storage: PostStorage) {
	// Add user
	let user = User {
		id: ID::from("author-1"),
		name: "Charlie".to_string(),
		email: "charlie@example.com".to_string(),
		active: true,
	};
	user_storage.add_user(user).await;

	// Add posts
	let post1 = Post {
		id: ID::from("post-1"),
		title: "First Post".to_string(),
		content: "Content 1".to_string(),
		author_id: ID::from("author-1"),
	};
	let post2 = Post {
		id: ID::from("post-2"),
		title: "Second Post".to_string(),
		content: "Content 2".to_string(),
		author_id: ID::from("author-1"),
	};
	post_storage.add_post(post1).await;
	post_storage.add_post(post2).await;

	let schema = create_test_schema(user_storage, post_storage);

	// Nested query: Post → Author
	let query = r#"
        {
            post(id: "post-1") {
                id
                title
                author {
                    id
                    name
                    email
                }
            }
        }
    "#;

	let result = schema.execute(query).await;
	assert!(result.errors.is_empty(), "Should not have errors");

	let data = result.data.into_json().unwrap();
	assert_eq!(data["post"]["id"], "post-1");
	assert_eq!(data["post"]["title"], "First Post");
	assert_eq!(data["post"]["author"]["id"], "author-1");
	assert_eq!(data["post"]["author"]["name"], "Charlie");
}

/// Test: Query with multiple relationships
#[rstest]
#[tokio::test]
async fn test_query_multiple_relationships(user_storage: UserStorage, post_storage: PostStorage) {
	let user = User {
		id: ID::from("author-2"),
		name: "David".to_string(),
		email: "david@example.com".to_string(),
		active: true,
	};
	user_storage.add_user(user).await;

	// Add 3 posts for same author
	for i in 1..=3 {
		post_storage
			.add_post(Post {
				id: ID::from(format!("post-{}", i)),
				title: format!("Post {}", i),
				content: format!("Content {}", i),
				author_id: ID::from("author-2"),
			})
			.await;
	}

	let schema = create_test_schema(user_storage, post_storage);

	let query = r#"
        {
            postsByAuthor(authorId: "author-2") {
                id
                title
            }
        }
    "#;

	let result = schema.execute(query).await;
	assert!(result.errors.is_empty(), "Should not have errors");

	let data = result.data.into_json().unwrap();
	let posts = data["postsByAuthor"].as_array().unwrap();
	assert_eq!(posts.len(), 3);
}

/// Test: Error handling - GraphQL error response
#[rstest]
#[tokio::test]
async fn test_error_handling_graphql_error(user_storage: UserStorage, post_storage: PostStorage) {
	let schema = create_test_schema(user_storage, post_storage);

	// Try to create post with non-existent author
	let mutation = r#"
        mutation {
            createPost(title: "Invalid Post", content: "Content", authorId: "nonexistent") {
                id
                title
            }
        }
    "#;

	let result = schema.execute(mutation).await;
	assert!(!result.errors.is_empty(), "Should have errors");
	assert!(
		result.errors[0].message.contains("Author not found"),
		"Error message should mention author not found"
	);
}

/// Test: Field-level error handling
#[rstest]
#[tokio::test]
async fn test_field_level_error(user_storage: UserStorage, post_storage: PostStorage) {
	// Add post with non-existent author (simulating orphaned data)
	let orphan_post = Post {
		id: ID::from("orphan-post"),
		title: "Orphan Post".to_string(),
		content: "No author".to_string(),
		author_id: ID::from("deleted-author"),
	};
	post_storage.add_post(orphan_post).await;

	let schema = create_test_schema(user_storage, post_storage);

	// Query should succeed, but author field should be null
	let query = r#"
        {
            post(id: "orphan-post") {
                id
                title
                author {
                    id
                    name
                }
            }
        }
    "#;

	let result = schema.execute(query).await;
	assert!(result.errors.is_empty(), "Should not have errors");

	let data = result.data.into_json().unwrap();
	assert_eq!(data["post"]["id"], "orphan-post");
	assert!(data["post"]["author"].is_null(), "Author should be null");
}

/// Test: Multiple mutations in sequence
#[rstest]
#[tokio::test]
async fn test_multiple_mutations_sequence(user_storage: UserStorage, post_storage: PostStorage) {
	let schema = create_test_schema(user_storage.clone(), post_storage.clone());

	// First mutation: Create user
	let mutation1 = r#"
        mutation {
            createUser(input: { name: "Eve", email: "eve@example.com" }) {
                id
                name
            }
        }
    "#;

	let result1 = schema.execute(mutation1).await;
	assert!(result1.errors.is_empty());
	let user_id = result1.data.into_json().unwrap()["createUser"]["id"]
		.as_str()
		.unwrap()
		.to_string();

	// Second mutation: Create post for that user
	let mutation2 = format!(
		r#"
        mutation {{
            createPost(title: "Eve's Post", content: "Hello", authorId: "{}") {{
                id
                title
            }}
        }}
    "#,
		user_id
	);

	let result2 = schema.execute(&mutation2).await;
	assert!(result2.errors.is_empty());
	let post_id = result2.data.into_json().unwrap()["createPost"]["id"]
		.as_str()
		.unwrap()
		.to_string();

	// Verify both exist in storage
	assert!(user_storage.get_user(&user_id).await.is_some());
	assert!(post_storage.get_post(&post_id).await.is_some());
}

/// Test: Query batching (multiple fields in one query)
#[rstest]
#[tokio::test]
async fn test_query_batching(user_storage: UserStorage, post_storage: PostStorage) {
	// Add test data
	user_storage
		.add_user(User {
			id: ID::from("batch-user-1"),
			name: "User1".to_string(),
			email: "user1@example.com".to_string(),
			active: true,
		})
		.await;
	user_storage
		.add_user(User {
			id: ID::from("batch-user-2"),
			name: "User2".to_string(),
			email: "user2@example.com".to_string(),
			active: false,
		})
		.await;

	let schema = create_test_schema(user_storage, post_storage);

	// Query multiple fields in one request
	let query = r#"
        {
            user1: user(id: "batch-user-1") {
                id
                name
            }
            user2: user(id: "batch-user-2") {
                id
                name
            }
            allUsers: users {
                id
            }
        }
    "#;

	let result = schema.execute(query).await;
	assert!(result.errors.is_empty());

	let data = result.data.into_json().unwrap();
	assert_eq!(data["user1"]["id"], "batch-user-1");
	assert_eq!(data["user2"]["id"], "batch-user-2");
	assert_eq!(data["allUsers"].as_array().unwrap().len(), 2);
}

/// Test: Resolver execution with context data access
#[rstest]
#[tokio::test]
async fn test_resolver_context_data_access(user_storage: UserStorage, post_storage: PostStorage) {
	user_storage
		.add_user(User {
			id: ID::from("context-test"),
			name: "ContextUser".to_string(),
			email: "context@example.com".to_string(),
			active: true,
		})
		.await;

	let schema = create_test_schema(user_storage, post_storage);

	// Execute query - resolver should access UserStorage via Context
	let query = r#"
        {
            user(id: "context-test") {
                name
            }
        }
    "#;

	let result = schema.execute(query).await;
	assert!(result.errors.is_empty());
	assert_eq!(
		result.data.into_json().unwrap()["user"]["name"],
		"ContextUser"
	);
}
