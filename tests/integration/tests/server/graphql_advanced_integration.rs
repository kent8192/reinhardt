#![cfg(feature = "graphql")]

use async_graphql::{self, EmptySubscription, ID, Object, Schema};
use reinhardt_server::GraphQLHandler;
use reinhardt_test::APIClient;
use reinhardt_test::server::{shutdown_test_server, spawn_test_server};
use rstest::{fixture, rstest};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ============================================================================
// Data Models
// ============================================================================

#[derive(Debug, Clone)]
struct User {
	id: ID,
	name: String,
	email: String,
}

#[Object]
impl User {
	async fn id(&self) -> &ID {
		&self.id
	}

	async fn name(&self) -> &str {
		&self.name
	}

	async fn email(&self) -> &str {
		&self.email
	}

	async fn posts(&self, ctx: &async_graphql::Context<'_>) -> Vec<Post> {
		let store = ctx.data_unchecked::<DataStore>();
		let posts = store.posts.lock().unwrap();
		posts
			.values()
			.filter(|p| p.author_id == self.id)
			.cloned()
			.collect()
	}
}

#[derive(Debug, Clone)]
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

	async fn author(&self, ctx: &async_graphql::Context<'_>) -> Option<User> {
		let store = ctx.data_unchecked::<DataStore>();
		let users = store.users.lock().unwrap();
		users.get(&self.author_id).cloned()
	}

	async fn comments(&self, ctx: &async_graphql::Context<'_>) -> Vec<Comment> {
		let store = ctx.data_unchecked::<DataStore>();
		let comments = store.comments.lock().unwrap();
		comments
			.values()
			.filter(|c| c.post_id == self.id)
			.cloned()
			.collect()
	}
}

#[derive(Debug, Clone)]
struct Comment {
	id: ID,
	text: String,
	author_id: ID,
	post_id: ID,
}

#[Object]
impl Comment {
	async fn id(&self) -> &ID {
		&self.id
	}

	async fn text(&self) -> &str {
		&self.text
	}

	async fn author(&self, ctx: &async_graphql::Context<'_>) -> Option<User> {
		let store = ctx.data_unchecked::<DataStore>();
		let users = store.users.lock().unwrap();
		users.get(&self.author_id).cloned()
	}

	async fn post(&self, ctx: &async_graphql::Context<'_>) -> Option<Post> {
		let store = ctx.data_unchecked::<DataStore>();
		let posts = store.posts.lock().unwrap();
		posts.get(&self.post_id).cloned()
	}
}

// ============================================================================
// Data Store
// ============================================================================

#[derive(Clone)]
struct DataStore {
	users: Arc<Mutex<HashMap<ID, User>>>,
	posts: Arc<Mutex<HashMap<ID, Post>>>,
	comments: Arc<Mutex<HashMap<ID, Comment>>>,
}

impl DataStore {
	fn new() -> Self {
		let mut users = HashMap::new();
		let mut posts = HashMap::new();
		let mut comments = HashMap::new();

		// Create sample users
		users.insert(
			ID::from("1"),
			User {
				id: ID::from("1"),
				name: "Alice".to_string(),
				email: "alice@example.com".to_string(),
			},
		);
		users.insert(
			ID::from("2"),
			User {
				id: ID::from("2"),
				name: "Bob".to_string(),
				email: "bob@example.com".to_string(),
			},
		);

		// Create sample posts
		posts.insert(
			ID::from("1"),
			Post {
				id: ID::from("1"),
				title: "First Post".to_string(),
				content: "Content of first post".to_string(),
				author_id: ID::from("1"),
			},
		);
		posts.insert(
			ID::from("2"),
			Post {
				id: ID::from("2"),
				title: "Second Post".to_string(),
				content: "Content of second post".to_string(),
				author_id: ID::from("2"),
			},
		);

		// Create sample comments
		comments.insert(
			ID::from("1"),
			Comment {
				id: ID::from("1"),
				text: "Great post!".to_string(),
				author_id: ID::from("2"),
				post_id: ID::from("1"),
			},
		);
		comments.insert(
			ID::from("2"),
			Comment {
				id: ID::from("2"),
				text: "Thanks for sharing!".to_string(),
				author_id: ID::from("1"),
				post_id: ID::from("2"),
			},
		);

		Self {
			users: Arc::new(Mutex::new(users)),
			posts: Arc::new(Mutex::new(posts)),
			comments: Arc::new(Mutex::new(comments)),
		}
	}
}

// ============================================================================
// GraphQL Schema
// ============================================================================

struct QueryRoot {
	store: DataStore,
}

#[Object]
impl QueryRoot {
	async fn user(&self, id: ID) -> Option<User> {
		let users = self.store.users.lock().unwrap();
		users.get(&id).cloned()
	}

	async fn users(&self) -> Vec<User> {
		let users = self.store.users.lock().unwrap();
		users.values().cloned().collect()
	}

	async fn post(&self, id: ID) -> Option<Post> {
		let posts = self.store.posts.lock().unwrap();
		posts.get(&id).cloned()
	}

	async fn posts(&self) -> Vec<Post> {
		let posts = self.store.posts.lock().unwrap();
		posts.values().cloned().collect()
	}

	/// Field that always returns an error for testing field-level errors
	async fn error_field(&self) -> Result<String, String> {
		Err("This field always fails".to_string())
	}

	/// Field that returns successfully for testing partial errors
	async fn success_field(&self) -> String {
		"Success".to_string()
	}

	/// Field that conditionally returns an error based on input
	async fn conditional_error(&self, should_fail: bool) -> Result<String, String> {
		if should_fail {
			Err("Conditional error triggered".to_string())
		} else {
			Ok("No error".to_string())
		}
	}
}

struct MutationRoot;

#[Object]
impl MutationRoot {
	async fn noop(&self) -> bool {
		true
	}
}

// ============================================================================
// Fixtures
// ============================================================================

#[fixture]
fn data_store() -> DataStore {
	DataStore::new()
}

// ============================================================================
// Test 1: Batch Queries
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_graphql_batch_queries(data_store: DataStore) {
	let query = QueryRoot {
		store: data_store.clone(),
	};
	let mutation = MutationRoot;

	let schema = Schema::build(query, mutation, EmptySubscription)
		.data(data_store)
		.finish();
	let handler = Arc::new(GraphQLHandler::new(schema));
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);

	// Batch query requesting multiple fields at once
	let batch_query = r#"{
		"query": "{ users { id name email } posts { id title author { name } } }"
	}"#;

	let response = client
		.post_raw("/", batch_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	// Verify users data
	let users = json["data"]["users"].as_array().unwrap();
	assert_eq!(users.len(), 2);

	// Verify posts data
	let posts = json["data"]["posts"].as_array().unwrap();
	assert_eq!(posts.len(), 2);

	// Verify nested author data in posts
	assert!(posts[0]["author"]["name"].is_string());

	shutdown_test_server(handle).await;
}

// ============================================================================
// Test 2: Schema Introspection
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_graphql_schema_introspection(data_store: DataStore) {
	let query = QueryRoot {
		store: data_store.clone(),
	};
	let mutation = MutationRoot;

	let schema = Schema::build(query, mutation, EmptySubscription)
		.data(data_store)
		.finish();
	let handler = Arc::new(GraphQLHandler::new(schema));
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);

	// Introspection query to get schema types
	let introspection_query = r#"{
		"query": "{ __schema { types { name kind } } }"
	}"#;

	let response = client
		.post_raw("/", introspection_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	// Verify schema introspection returns types
	let types = json["data"]["__schema"]["types"].as_array().unwrap();
	assert!(!types.is_empty());

	// Check for our custom types
	let type_names: Vec<&str> = types.iter().filter_map(|t| t["name"].as_str()).collect();
	assert!(type_names.contains(&"User"));
	assert!(type_names.contains(&"Post"));
	assert!(type_names.contains(&"Comment"));

	// Test specific type introspection
	let type_query = r#"{
		"query": "{ __type(name: \"User\") { name kind fields { name type { name kind } } } }"
	}"#;

	let response = client
		.post_raw("/", type_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	// Verify User type has expected fields
	let user_type = &json["data"]["__type"];
	assert_eq!(user_type["name"].as_str().unwrap(), "User");

	let fields = user_type["fields"].as_array().unwrap();
	let field_names: Vec<&str> = fields.iter().filter_map(|f| f["name"].as_str()).collect();
	assert!(field_names.contains(&"id"));
	assert!(field_names.contains(&"name"));
	assert!(field_names.contains(&"email"));
	assert!(field_names.contains(&"posts"));

	shutdown_test_server(handle).await;
}

// ============================================================================
// Test 3: Field-Level Errors
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_graphql_field_level_errors(data_store: DataStore) {
	let query = QueryRoot {
		store: data_store.clone(),
	};
	let mutation = MutationRoot;

	let schema = Schema::build(query, mutation, EmptySubscription)
		.data(data_store)
		.finish();
	let handler = Arc::new(GraphQLHandler::new(schema));
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);

	// Query with both error and success fields
	// Note: async-graphql converts Rust snake_case to camelCase
	let mixed_query = r#"{
		"query": "{ successField errorField }"
	}"#;

	let response = client
		.post_raw("/", mixed_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body)
		.unwrap_or_else(|e| panic!("Failed to parse response as JSON: {}\nBody: {}", e, body));

	// In async-graphql, Result<T, E> fields are nullable. When a nullable field
	// returns Err, only that field becomes null while other fields return normally.
	// This is the expected GraphQL behavior for partial errors.
	let data = json.get("data").expect("data should exist in response");
	assert!(
		!data.is_null(),
		"data should not be null for nullable field errors: {}",
		json
	);
	assert_eq!(
		data.get("successField").and_then(|v| v.as_str()),
		Some("Success"),
		"successful fields should still return values"
	);
	// errorField should be null since it returned Err
	assert!(
		data.get("errorField").map_or(true, |v| v.is_null()),
		"errorField should be null when it returns Err: {}",
		json
	);

	// Verify errors array exists and contains error for errorField
	let errors = json
		.get("errors")
		.and_then(|e| e.as_array())
		.unwrap_or_else(|| panic!("Expected 'errors' to be an array: {}", json));
	assert_eq!(errors.len(), 1);
	assert_eq!(
		errors[0]["message"].as_str().unwrap(),
		"This field always fails"
	);

	// Verify error path points to the correct field
	let error_path = errors[0]["path"].as_array().unwrap();
	assert_eq!(error_path[0].as_str().unwrap(), "errorField");

	// Test conditional error
	let conditional_query = r#"{
		"query": "{ success: conditionalError(shouldFail: false) error: conditionalError(shouldFail: true) }"
	}"#;

	let response = client
		.post_raw("/", conditional_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body)
		.unwrap_or_else(|e| panic!("Failed to parse response as JSON: {}\nBody: {}", e, body));

	// Conditional error: conditionalError returns Result<T, E> which is nullable.
	// When one alias fails, only that field becomes null while the successful alias returns normally.
	let data = json.get("data").expect("data should exist in response");
	assert!(
		!data.is_null(),
		"data should not be null for nullable field errors: {}",
		json
	);
	assert_eq!(
		data.get("success").and_then(|v| v.as_str()),
		Some("No error"),
		"successful alias should still return value"
	);
	// error alias should be null since it returned Err
	assert!(
		data.get("error").map_or(true, |v| v.is_null()),
		"error alias should be null when conditionalError returns Err: {}",
		json
	);

	// Verify error exists for the failed field
	let errors = json
		.get("errors")
		.and_then(|e| e.as_array())
		.unwrap_or_else(|| panic!("Expected 'errors' to be an array: {}", json));
	assert_eq!(errors.len(), 1);
	assert_eq!(
		errors[0]["message"].as_str().unwrap(),
		"Conditional error triggered"
	);

	shutdown_test_server(handle).await;
}

// ============================================================================
// Test 4: Nesting Limits
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_graphql_nesting_limits(data_store: DataStore) {
	let query = QueryRoot {
		store: data_store.clone(),
	};
	let mutation = MutationRoot;

	// Build schema with depth limit of 5
	let schema = Schema::build(query, mutation, EmptySubscription)
		.data(data_store)
		.limit_depth(5)
		.finish();
	let handler = Arc::new(GraphQLHandler::new(schema));
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);

	// Query within depth limit (depth = 4)
	let shallow_query = r#"{
		"query": "{ posts { id author { name posts { id title } } } }"
	}"#;

	let response = client
		.post_raw("/", shallow_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	// Should succeed within limit
	assert!(json["data"].is_object());
	assert!(json.get("errors").is_none() || json["errors"].as_array().unwrap().is_empty());

	// Query exceeding depth limit (depth > 5)
	let deep_query = r#"{
		"query": "{ posts { id author { posts { author { posts { author { name } } } } } } }"
	}"#;

	let response = client
		.post_raw("/", deep_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	// Should return error for exceeding depth limit
	let errors = json["errors"].as_array().unwrap();
	assert!(!errors.is_empty());

	// Error message should mention depth, complexity, or nesting
	let error_msg = errors[0]["message"].as_str().unwrap().to_lowercase();
	assert!(
		error_msg.contains("depth")
			|| error_msg.contains("complex")
			|| error_msg.contains("nested"),
		"Expected depth/complexity/nesting error, got: {}",
		error_msg
	);

	shutdown_test_server(handle).await;
}

// ============================================================================
// Test 5: Complex Query Patterns
// ============================================================================

#[rstest]
#[tokio::test]
async fn test_graphql_complex_query_patterns(data_store: DataStore) {
	let query = QueryRoot {
		store: data_store.clone(),
	};
	let mutation = MutationRoot;

	let schema = Schema::build(query, mutation, EmptySubscription)
		.data(data_store)
		.finish();
	let handler = Arc::new(GraphQLHandler::new(schema));
	let (url, handle) = spawn_test_server(handler).await;

	let client = APIClient::with_base_url(&url);

	// Test 1: Fragments
	let fragment_query = r#"{
		"query": "fragment UserFields on User { id name email } query { users { ...UserFields } }"
	}"#;

	let response = client
		.post_raw("/", fragment_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	let users = json["data"]["users"].as_array().unwrap();
	assert!(!users.is_empty());
	assert!(users[0]["id"].is_string());
	assert!(users[0]["name"].is_string());
	assert!(users[0]["email"].is_string());

	// Test 2: Variables
	let variable_query = r#"{
		"query": "query GetUser($userId: ID!) { user(id: $userId) { id name } }",
		"variables": { "userId": "1" }
	}"#;

	let response = client
		.post_raw("/", variable_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	assert_eq!(json["data"]["user"]["id"].as_str().unwrap(), "1");
	assert_eq!(json["data"]["user"]["name"].as_str().unwrap(), "Alice");

	// Test 3: Aliases
	let alias_query = r#"{
		"query": "{ alice: user(id: \"1\") { name } bob: user(id: \"2\") { name } }"
	}"#;

	let response = client
		.post_raw("/", alias_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	assert_eq!(json["data"]["alice"]["name"].as_str().unwrap(), "Alice");
	assert_eq!(json["data"]["bob"]["name"].as_str().unwrap(), "Bob");

	// Test 4: Directives (@skip, @include)
	let directive_query = r#"{
		"query": "query GetUser($skipEmail: Boolean!) { users { name email @skip(if: $skipEmail) } }",
		"variables": { "skipEmail": true }
	}"#;

	let response = client
		.post_raw("/", directive_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	let users = json["data"]["users"].as_array().unwrap();
	assert!(!users.is_empty());
	assert!(users[0]["name"].is_string());
	assert!(users[0].get("email").is_none()); // Email should be skipped

	// Test 5: Combined complex query with fragments, variables, and aliases
	let complex_query = r#"{
		"query": "fragment PostInfo on Post { id title } query GetUserData($userId: ID!, $includeEmail: Boolean!) { currentUser: user(id: $userId) { name email @include(if: $includeEmail) posts { ...PostInfo } } }",
		"variables": { "userId": "1", "includeEmail": false }
	}"#;

	let response = client
		.post_raw("/", complex_query.as_bytes(), "application/json")
		.await
		.unwrap();

	assert_eq!(response.status_code(), 200);
	let body = response.text();
	let json: Value = serde_json::from_str(&body).unwrap();

	assert_eq!(
		json["data"]["currentUser"]["name"].as_str().unwrap(),
		"Alice"
	);
	assert!(json["data"]["currentUser"].get("email").is_none()); // Email should not be included
	assert!(json["data"]["currentUser"]["posts"].is_array());

	shutdown_test_server(handle).await;
}
