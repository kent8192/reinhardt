use async_graphql::extensions::Analyzer;
use async_graphql::{Context, EmptySubscription, ID, Object, Result as GqlResult, Schema};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Error types for GraphQL schema operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphQLError {
	/// A schema construction or configuration error.
	#[error("Schema error: {0}")]
	Schema(String),
	/// An error occurred during resolver execution.
	#[error("Resolver error: {0}")]
	Resolver(String),
	/// The requested resource was not found.
	#[error("Not found: {0}")]
	NotFound(String),
}

/// A specialized `Result` type for GraphQL schema operations.
pub type GraphQLResult<T> = Result<T, GraphQLError>;

/// Default maximum query depth limit.
///
/// Limits how deeply nested a query can be to prevent resource exhaustion
/// from deeply nested selections.
pub const DEFAULT_MAX_QUERY_DEPTH: usize = 10;

/// Default maximum query complexity limit.
///
/// Limits total complexity score for a single query to prevent
/// resource exhaustion from expensive operations.
pub const DEFAULT_MAX_QUERY_COMPLEXITY: usize = 100;

/// Default maximum query size in bytes.
///
/// Prevents excessively large query strings from consuming parsing resources.
pub const DEFAULT_MAX_QUERY_SIZE: usize = 32_768; // 32 KB

/// Default maximum number of fields in a single query.
///
/// Prevents queries that request an excessive number of fields,
/// which could lead to resource exhaustion.
pub const DEFAULT_MAX_FIELD_COUNT: usize = 200;

/// Default maximum page size for paginated queries.
///
/// Prevents unbounded result sets that could cause memory exhaustion.
pub const DEFAULT_MAX_PAGE_SIZE: usize = 100;

/// Default page size for paginated queries.
pub const DEFAULT_PAGE_SIZE: usize = 20;

/// Maximum allowed length for user name input.
const MAX_NAME_LENGTH: usize = 100;

/// Maximum allowed length for email input.
const MAX_EMAIL_LENGTH: usize = 254;

/// Check whether a string exceeds the given character limit.
///
/// Uses short-circuit counting: stops as soon as `max + 1` characters
/// have been scanned, avoiding a full O(n) traversal for large inputs.
fn exceeds_max_chars(s: &str, max: usize) -> bool {
	s.chars().nth(max).is_some()
}

/// Configuration for GraphQL query protection limits.
///
/// Controls query depth, complexity, size, and field count limits to prevent
/// denial-of-service attacks through resource exhaustion.
///
/// # Examples
///
/// ```
/// use reinhardt_graphql::schema::QueryLimits;
///
/// // Use defaults
/// let limits = QueryLimits::default();
/// assert_eq!(limits.max_depth, 10);
/// assert_eq!(limits.max_complexity, 100);
/// assert_eq!(limits.max_query_size, 32_768);
/// assert_eq!(limits.max_field_count, 200);
///
/// // Custom limits
/// let limits = QueryLimits::new(15, 200);
/// assert_eq!(limits.max_depth, 15);
/// assert_eq!(limits.max_complexity, 200);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct QueryLimits {
	/// Maximum allowed query depth
	pub max_depth: usize,
	/// Maximum allowed query complexity
	pub max_complexity: usize,
	/// Maximum allowed query string size in bytes
	pub max_query_size: usize,
	/// Maximum allowed number of fields in a query
	pub max_field_count: usize,
}

impl QueryLimits {
	/// Create a new `QueryLimits` with custom depth and complexity values.
	///
	/// Uses default values for query size and field count limits.
	pub fn new(max_depth: usize, max_complexity: usize) -> Self {
		Self {
			max_depth,
			max_complexity,
			max_query_size: DEFAULT_MAX_QUERY_SIZE,
			max_field_count: DEFAULT_MAX_FIELD_COUNT,
		}
	}

	/// Create a new `QueryLimits` with all values specified.
	pub fn full(
		max_depth: usize,
		max_complexity: usize,
		max_query_size: usize,
		max_field_count: usize,
	) -> Self {
		Self {
			max_depth,
			max_complexity,
			max_query_size,
			max_field_count,
		}
	}
}

impl Default for QueryLimits {
	fn default() -> Self {
		Self {
			max_depth: DEFAULT_MAX_QUERY_DEPTH,
			max_complexity: DEFAULT_MAX_QUERY_COMPLEXITY,
			max_query_size: DEFAULT_MAX_QUERY_SIZE,
			max_field_count: DEFAULT_MAX_FIELD_COUNT,
		}
	}
}

/// Validate a GraphQL query string against size and field count limits.
///
/// Returns `Ok(())` if the query passes all checks, or an error message
/// describing which limit was exceeded.
pub fn validate_query(query: &str, limits: &QueryLimits) -> Result<(), String> {
	// Check query size
	if query.len() > limits.max_query_size {
		return Err(format!(
			"Query size {} bytes exceeds maximum of {} bytes",
			query.len(),
			limits.max_query_size
		));
	}

	// Approximate field count by counting field-like tokens
	// A more accurate count would require parsing, but this provides
	// a reasonable heuristic for DoS prevention
	let field_count = count_query_fields(query);
	if field_count > limits.max_field_count {
		return Err(format!(
			"Query field count {} exceeds maximum of {}",
			field_count, limits.max_field_count
		));
	}

	Ok(())
}

/// GraphQL keywords that should not be counted as fields.
const GRAPHQL_KEYWORDS: &[&str] = &[
	"query",
	"mutation",
	"subscription",
	"fragment",
	"on",
	"true",
	"false",
	"null",
];

/// Check whether a token is a field-like identifier.
///
/// Returns `true` when the token looks like a GraphQL field name:
/// an alphanumeric identifier that is not a keyword and does not
/// start with a fragment spread (`...`).
fn is_field_identifier(token: &str) -> bool {
	!token.is_empty()
		&& !token.starts_with("...")
		&& !GRAPHQL_KEYWORDS.contains(&token)
		&& token
			.chars()
			.next()
			.is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

/// Count approximate number of fields in a GraphQL query.
///
/// Counts field-like identifiers that appear inside selection sets
/// (brace depth > 0). Each identifier token is evaluated immediately
/// during character processing, so multiple fields on the same line
/// are counted correctly.
///
/// Handles inline fragment type conditions (`... on Type { }`) by
/// tracking the `on` keyword and skipping the subsequent type name.
/// Also handles block strings (`"""..."""`) to avoid miscounting
/// content inside them as fields.
fn count_query_fields(query: &str) -> usize {
	let mut count = 0;
	let mut in_string = false;
	let mut in_block_string = false;
	let mut depth: usize = 0;
	let mut token = String::new();
	let mut in_comment = false;
	let mut escaped = false;
	// Track whether the last flushed token was the `on` keyword,
	// so the next identifier (a type condition) is not counted as a field.
	let mut after_on_keyword = false;

	let chars: Vec<char> = query.chars().collect();
	let len = chars.len();
	let mut i = 0;

	while i < len {
		let ch = chars[i];

		if escaped {
			escaped = false;
			i += 1;
			continue;
		}

		// Handle block strings: skip everything until closing """
		if in_block_string {
			if ch == '"' && i + 2 < len && chars[i + 1] == '"' && chars[i + 2] == '"' {
				in_block_string = false;
				i += 3; // skip closing """
			} else {
				i += 1;
			}
			continue;
		}

		// Handle line comments: everything after '#' (outside strings) is ignored
		if ch == '\n' {
			in_comment = false;
			// Flush any accumulated token at end of line
			if depth > 0 && !in_string && is_field_identifier(&token) {
				if after_on_keyword {
					after_on_keyword = false;
				} else {
					count += 1;
				}
			}
			if !is_field_identifier(&token) {
				after_on_keyword = false;
			}
			token.clear();
			i += 1;
			continue;
		}

		if in_comment {
			i += 1;
			continue;
		}

		if in_string {
			match ch {
				'\\' => escaped = true,
				'"' => in_string = false,
				_ => {}
			}
			i += 1;
			continue;
		}

		match ch {
			'#' => {
				// Flush token before comment starts
				if depth > 0 && is_field_identifier(&token) {
					if after_on_keyword {
						after_on_keyword = false;
					} else {
						count += 1;
					}
				}
				token.clear();
				in_comment = true;
			}
			'"' => {
				// Check for block string opening: """
				if i + 2 < len && chars[i + 1] == '"' && chars[i + 2] == '"' {
					// Flush token before block string starts
					if depth > 0 && is_field_identifier(&token) {
						if after_on_keyword {
							after_on_keyword = false;
						} else {
							count += 1;
						}
					}
					token.clear();
					in_block_string = true;
					i += 3; // skip opening """
					continue;
				}
				// Flush token before string starts
				if depth > 0 && is_field_identifier(&token) {
					if after_on_keyword {
						after_on_keyword = false;
					} else {
						count += 1;
					}
				}
				token.clear();
				in_string = true;
			}
			'{' => {
				// Flush token — the identifier before '{' is a field with sub-selection
				if depth > 0 && is_field_identifier(&token) {
					if after_on_keyword {
						after_on_keyword = false;
					} else {
						count += 1;
					}
				}
				token.clear();
				depth += 1;
			}
			'}' => {
				// Flush token before closing brace
				if depth > 0 && is_field_identifier(&token) {
					if after_on_keyword {
						after_on_keyword = false;
					} else {
						count += 1;
					}
				}
				token.clear();
				depth = depth.saturating_sub(1);
			}
			'(' => {
				// Flush token — the identifier before '(' is a field with arguments
				if depth > 0 && is_field_identifier(&token) {
					if after_on_keyword {
						after_on_keyword = false;
					} else {
						count += 1;
					}
				}
				token.clear();
			}
			c if c.is_ascii_whitespace() || c == ',' => {
				// Token delimiter: evaluate accumulated token
				if depth > 0 && is_field_identifier(&token) {
					if after_on_keyword {
						after_on_keyword = false;
					} else {
						// Set after_on_keyword when flushing the `on` keyword itself
						if token == "on" {
							after_on_keyword = true;
						}
						count += 1;
					}
				} else if token == "on" {
					// `on` is in GRAPHQL_KEYWORDS so is_field_identifier returns false,
					// but we still need to track it for inline fragment detection
					after_on_keyword = true;
				}
				token.clear();
			}
			')' | ':' | '!' | '@' | '$' | '=' | '|' | '&' => {
				// Punctuation that terminates a token but is not a field delimiter
				token.clear();
			}
			_ => {
				token.push(ch);
			}
		}
		i += 1;
	}

	// Flush final token (query may not end with newline)
	if depth > 0 && !in_string && is_field_identifier(&token) {
		if after_on_keyword {
			// Type condition at end of query — do not count
		} else {
			count += 1;
		}
	}

	count
}

/// Validate input for creating a user.
///
/// Enforces:
/// - Name is non-empty and within length limits
/// - Name contains only valid characters
/// - Email is non-empty and within length limits
/// - Email has a basic valid format
fn validate_create_user_input(input: &CreateUserInput) -> GqlResult<()> {
	// Validate name
	let name = input.name.trim();
	if name.is_empty() {
		return Err(async_graphql::Error::new("Name cannot be empty"));
	}
	if exceeds_max_chars(name, MAX_NAME_LENGTH) {
		return Err(async_graphql::Error::new(format!(
			"Name exceeds maximum length of {} characters",
			MAX_NAME_LENGTH
		)));
	}
	if !name
		.chars()
		.all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ' || c == '.')
	{
		return Err(async_graphql::Error::new(
			"Name contains invalid characters (allowed: alphanumeric, spaces, underscores, hyphens, dots)",
		));
	}

	// Validate email
	let email = input.email.trim();
	if email.is_empty() {
		return Err(async_graphql::Error::new("Email cannot be empty"));
	}
	if exceeds_max_chars(email, MAX_EMAIL_LENGTH) {
		return Err(async_graphql::Error::new(format!(
			"Email exceeds maximum length of {} characters",
			MAX_EMAIL_LENGTH
		)));
	}
	// Basic email format validation: must contain exactly one @ with parts on both sides
	let at_count = email.chars().filter(|c| *c == '@').count();
	if at_count != 1 {
		return Err(async_graphql::Error::new("Invalid email format"));
	}
	let parts: Vec<&str> = email.splitn(2, '@').collect();
	if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() || !parts[1].contains('.') {
		return Err(async_graphql::Error::new("Invalid email format"));
	}

	Ok(())
}

/// Example: User type for GraphQL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
	/// Unique identifier for the user.
	pub id: ID,
	/// Display name of the user.
	pub name: String,
	/// Email address of the user.
	pub email: String,
	/// Whether the user account is active.
	pub active: bool,
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

	async fn active(&self) -> bool {
		self.active
	}
}

/// User storage (in-memory for example)
#[derive(Clone)]
pub struct UserStorage {
	users: Arc<RwLock<HashMap<String, User>>>,
}

impl UserStorage {
	/// Create a new user storage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_graphql::schema::UserStorage;
	///
	/// let storage = UserStorage::new();
	/// // Creates a new storage instance with defaults
	/// ```
	pub fn new() -> Self {
		Self {
			users: Arc::new(RwLock::new(HashMap::new())),
		}
	}
	/// Add a user to storage
	///
	pub async fn add_user(&self, user: User) {
		self.users.write().await.insert(user.id.to_string(), user);
	}
	/// Get a user by ID
	///
	/// # Examples
	///
	/// ```no_run
	/// # fn main() {
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// use reinhardt_graphql::schema::UserStorage;
	/// let storage = UserStorage::new();
	/// // Retrieve user
	/// let user = storage.get_user("user-1").await;
	/// # });
	/// # }
	/// ```
	pub async fn get_user(&self, id: &str) -> Option<User> {
		self.users.read().await.get(id).cloned()
	}
	/// List all users
	///
	/// # Examples
	///
	/// ```no_run
	/// # fn main() {
	/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
	/// use reinhardt_graphql::schema::UserStorage;
	/// let storage = UserStorage::new();
	/// // List all users
	/// let users = storage.list_users().await;
	/// # });
	/// # }
	/// ```
	pub async fn list_users(&self) -> Vec<User> {
		self.users.read().await.values().cloned().collect()
	}
}

impl Default for UserStorage {
	fn default() -> Self {
		Self::new()
	}
}

/// GraphQL Query root
pub struct Query;

#[Object]
impl Query {
	async fn user(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<User>> {
		let storage = ctx.data::<UserStorage>()?;
		Ok(storage.get_user(id.as_ref()).await)
	}

	/// List users with pagination support.
	///
	/// # Arguments
	///
	/// * `first` - Maximum number of users to return (default: 20, max: 100)
	/// * `offset` - Number of users to skip (default: 0)
	async fn users(
		&self,
		ctx: &Context<'_>,
		first: Option<usize>,
		offset: Option<usize>,
	) -> GqlResult<Vec<User>> {
		let storage = ctx.data::<UserStorage>()?;
		let limit = first
			.unwrap_or(DEFAULT_PAGE_SIZE)
			.min(DEFAULT_MAX_PAGE_SIZE);
		let skip = offset.unwrap_or(0);
		let all_users = storage.list_users().await;
		Ok(all_users.into_iter().skip(skip).take(limit).collect())
	}

	async fn hello(&self, name: Option<String>) -> String {
		format!("Hello, {}!", name.unwrap_or_else(|| "World".to_string()))
	}
}

/// Input type for creating users
#[derive(async_graphql::InputObject)]
pub struct CreateUserInput {
	/// Name of the user to create.
	pub name: String,
	/// Email address of the user to create.
	pub email: String,
}

/// GraphQL Mutation root
pub struct Mutation;

#[Object]
impl Mutation {
	async fn create_user(&self, ctx: &Context<'_>, input: CreateUserInput) -> GqlResult<User> {
		// Validate input before processing
		validate_create_user_input(&input)?;

		let storage = ctx.data::<UserStorage>()?;

		let user = User {
			id: ID::from(uuid::Uuid::now_v7().to_string()),
			name: input.name.trim().to_string(),
			email: input.email.trim().to_string(),
			active: true,
		};

		storage.add_user(user.clone()).await;
		Ok(user)
	}

	async fn update_user_status(
		&self,
		ctx: &Context<'_>,
		id: ID,
		active: bool,
	) -> GqlResult<Option<User>> {
		let storage = ctx.data::<UserStorage>()?;

		if let Some(mut user) = storage.get_user(id.as_ref()).await {
			user.active = active;
			storage.add_user(user.clone()).await;
			Ok(Some(user))
		} else {
			Ok(None)
		}
	}
}

/// Create GraphQL schema
pub type AppSchema = Schema<Query, Mutation, EmptySubscription>;

/// Create a GraphQL schema with default query protection limits.
///
/// Applies default depth and complexity limits to prevent
/// resource exhaustion from malicious queries.
pub fn create_schema(storage: UserStorage) -> AppSchema {
	create_schema_with_limits(storage, QueryLimits::default())
}

/// Create a GraphQL schema with custom query protection limits.
///
/// Configures depth limit, complexity limit, and the `Analyzer` extension
/// for query cost analysis.
///
/// # Arguments
///
/// * `storage` - User data storage
/// * `limits` - Query protection limits configuration
pub fn create_schema_with_limits(storage: UserStorage, limits: QueryLimits) -> AppSchema {
	Schema::build(Query, Mutation, EmptySubscription)
		.data(storage)
		.limit_depth(limits.max_depth)
		.limit_complexity(limits.max_complexity)
		.extension(Analyzer)
		.finish()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_query_hello() {
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		let query = r#"
            {
                hello(name: "GraphQL")
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert_eq!(data["hello"], "Hello, GraphQL!");
	}

	#[tokio::test]
	async fn test_mutation_create_user() {
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		let query = r#"
            mutation {
                createUser(input: { name: "Alice", email: "alice@example.com" }) {
                    name
                    email
                    active
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert_eq!(data["createUser"]["name"], "Alice");
		assert!(data["createUser"]["active"].as_bool().unwrap());
	}

	#[tokio::test]
	async fn test_query_user() {
		let storage = UserStorage::new();
		let user = User {
			id: ID::from("test-id-123"),
			name: "Bob".to_string(),
			email: "bob@example.com".to_string(),
			active: true,
		};
		storage.add_user(user).await;

		let schema = create_schema(storage);

		let query = r#"
            {
                user(id: "test-id-123") {
                    id
                    name
                    email
                    active
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert_eq!(data["user"]["id"], "test-id-123");
		assert_eq!(data["user"]["name"], "Bob");
		assert_eq!(data["user"]["email"], "bob@example.com");
		assert!(data["user"]["active"].as_bool().unwrap());
	}

	#[tokio::test]
	async fn test_query_user_not_found() {
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		let query = r#"
            {
                user(id: "nonexistent-id") {
                    id
                    name
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert!(data["user"].is_null());
	}

	#[tokio::test]
	async fn test_query_users_empty() {
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		let query = r#"
            {
                users {
                    id
                    name
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert!(data["users"].is_array());
		assert_eq!(data["users"].as_array().unwrap().len(), 0);
	}

	#[tokio::test]
	async fn test_query_users_multiple() {
		let storage = UserStorage::new();

		let user1 = User {
			id: ID::from("1"),
			name: "Alice".to_string(),
			email: "alice@example.com".to_string(),
			active: true,
		};
		let user2 = User {
			id: ID::from("2"),
			name: "Bob".to_string(),
			email: "bob@example.com".to_string(),
			active: false,
		};
		let user3 = User {
			id: ID::from("3"),
			name: "Charlie".to_string(),
			email: "charlie@example.com".to_string(),
			active: true,
		};

		storage.add_user(user1).await;
		storage.add_user(user2).await;
		storage.add_user(user3).await;

		let schema = create_schema(storage);

		let query = r#"
            {
                users {
                    id
                    name
                    email
                    active
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		let users = data["users"].as_array().unwrap();
		assert_eq!(users.len(), 3);

		// Verify that all users are present
		let names: Vec<&str> = users.iter().map(|u| u["name"].as_str().unwrap()).collect();
		assert!(names.contains(&"Alice"));
		assert!(names.contains(&"Bob"));
		assert!(names.contains(&"Charlie"));
	}

	#[tokio::test]
	async fn test_query_users_pagination_with_first() {
		// Arrange
		let storage = UserStorage::new();
		for i in 0..10 {
			storage
				.add_user(User {
					id: ID::from(format!("user-{}", i)),
					name: format!("User{}", i),
					email: format!("user{}@example.com", i),
					active: true,
				})
				.await;
		}
		let schema = create_schema(storage);

		// Act: request only 3 users
		let query = r#"{ users(first: 3) { id } }"#;
		let result = schema.execute(query).await;

		// Assert
		assert!(result.errors.is_empty());
		let data = result.data.into_json().unwrap();
		let users = data["users"].as_array().unwrap();
		assert_eq!(users.len(), 3);
	}

	#[tokio::test]
	async fn test_query_users_pagination_with_offset() {
		// Arrange
		let storage = UserStorage::new();
		for i in 0..5 {
			storage
				.add_user(User {
					id: ID::from(format!("user-{}", i)),
					name: format!("User{}", i),
					email: format!("user{}@example.com", i),
					active: true,
				})
				.await;
		}
		let schema = create_schema(storage);

		// Act: skip 3, take 10 -> should get 2
		let query = r#"{ users(first: 10, offset: 3) { id } }"#;
		let result = schema.execute(query).await;

		// Assert
		assert!(result.errors.is_empty());
		let data = result.data.into_json().unwrap();
		let users = data["users"].as_array().unwrap();
		assert_eq!(users.len(), 2);
	}

	#[tokio::test]
	async fn test_query_users_enforces_max_page_size() {
		// Arrange
		let storage = UserStorage::new();
		for i in 0..150 {
			storage
				.add_user(User {
					id: ID::from(format!("user-{}", i)),
					name: format!("User{}", i),
					email: format!("user{}@example.com", i),
					active: true,
				})
				.await;
		}
		let schema = create_schema(storage);

		// Act: request 500 users but max is 100
		let query = r#"{ users(first: 500) { id } }"#;
		let result = schema.execute(query).await;

		// Assert: clamped to max page size
		assert!(result.errors.is_empty());
		let data = result.data.into_json().unwrap();
		let users = data["users"].as_array().unwrap();
		assert_eq!(users.len(), DEFAULT_MAX_PAGE_SIZE);
	}

	#[tokio::test]
	async fn test_create_user_validates_empty_name() {
		// Arrange
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		// Act
		let query = r#"
			mutation {
				createUser(input: { name: "   ", email: "test@example.com" }) {
					id
				}
			}
		"#;
		let result = schema.execute(query).await;

		// Assert
		assert!(
			!result.errors.is_empty(),
			"expected validation error for empty name"
		);
	}

	#[tokio::test]
	async fn test_create_user_validates_invalid_email() {
		// Arrange
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		// Act
		let query = r#"
			mutation {
				createUser(input: { name: "Alice", email: "not-an-email" }) {
					id
				}
			}
		"#;
		let result = schema.execute(query).await;

		// Assert
		assert!(
			!result.errors.is_empty(),
			"expected validation error for invalid email"
		);
	}

	#[tokio::test]
	async fn test_validate_query_rejects_oversized_query() {
		// Arrange
		let limits = QueryLimits::full(10, 100, 100, 200); // 100 byte limit

		// Act
		let long_query = "{ ".to_string() + &"a ".repeat(100) + "}";
		let result = validate_query(&long_query, &limits);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().contains("exceeds maximum"));
	}

	#[tokio::test]
	async fn test_validate_query_accepts_normal_query() {
		// Arrange
		let limits = QueryLimits::default();

		// Act
		let result = validate_query("{ users { id name } }", &limits);

		// Assert
		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_mutation_update_user_status() {
		let storage = UserStorage::new();
		let user = User {
			id: ID::from("update-test-id"),
			name: "David".to_string(),
			email: "david@example.com".to_string(),
			active: true,
		};
		storage.add_user(user).await;

		let schema = create_schema(storage);

		let query = r#"
            mutation {
                updateUserStatus(id: "update-test-id", active: false) {
                    id
                    name
                    active
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert_eq!(data["updateUserStatus"]["id"], "update-test-id");
		assert!(!data["updateUserStatus"]["active"].as_bool().unwrap());
	}

	#[tokio::test]
	async fn test_mutation_update_nonexistent_user() {
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		let query = r#"
            mutation {
                updateUserStatus(id: "does-not-exist", active: false) {
                    id
                    name
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert!(data["updateUserStatus"].is_null());
	}

	#[tokio::test]
	async fn test_user_object_fields() {
		let user = User {
			id: ID::from("field-test-id"),
			name: "Eve".to_string(),
			email: "eve@example.com".to_string(),
			active: false,
		};

		// Test direct field access
		assert_eq!(user.id.to_string(), "field-test-id");
		assert_eq!(user.name, "Eve");
		assert_eq!(user.email, "eve@example.com");
		assert!(!user.active);
	}

	#[tokio::test]
	async fn test_user_storage_add_get() {
		let storage = UserStorage::new();

		let user = User {
			id: ID::from("storage-test-1"),
			name: "Frank".to_string(),
			email: "frank@example.com".to_string(),
			active: true,
		};

		storage.add_user(user.clone()).await;

		let retrieved = storage.get_user("storage-test-1").await;
		let retrieved = retrieved.unwrap();
		assert_eq!(retrieved.id.to_string(), "storage-test-1");
		assert_eq!(retrieved.name, "Frank");
		assert_eq!(retrieved.email, "frank@example.com");
		assert!(retrieved.active);
	}

	#[tokio::test]
	async fn test_user_storage_list() {
		let storage = UserStorage::new();

		// Initially empty
		let users = storage.list_users().await;
		assert_eq!(users.len(), 0);

		// Add users
		storage
			.add_user(User {
				id: ID::from("list-1"),
				name: "User1".to_string(),
				email: "user1@example.com".to_string(),
				active: true,
			})
			.await;

		storage
			.add_user(User {
				id: ID::from("list-2"),
				name: "User2".to_string(),
				email: "user2@example.com".to_string(),
				active: false,
			})
			.await;

		let users = storage.list_users().await;
		assert_eq!(users.len(), 2);
	}

	#[tokio::test]
	async fn test_create_schema_with_data() {
		let storage = UserStorage::new();
		storage
			.add_user(User {
				id: ID::from("pre-existing"),
				name: "PreExisting".to_string(),
				email: "preexisting@example.com".to_string(),
				active: true,
			})
			.await;

		let schema = create_schema(storage);

		// Verify schema can query pre-existing data
		let query = r#"
            {
                user(id: "pre-existing") {
                    name
                }
            }
        "#;

		let result = schema.execute(query).await;
		let data = result.data.into_json().unwrap();
		assert_eq!(data["user"]["name"], "PreExisting");
	}

	#[tokio::test]
	async fn test_graphql_error_types() {
		let err1 = GraphQLError::Schema("test schema error".to_string());
		assert!(err1.to_string().contains("Schema error"));

		let err2 = GraphQLError::Resolver("test resolver error".to_string());
		assert!(err2.to_string().contains("Resolver error"));

		let err3 = GraphQLError::NotFound("test item".to_string());
		assert!(err3.to_string().contains("Not found"));
	}

	#[tokio::test]
	async fn test_query_depth_limit_rejects_deep_query() {
		// Arrange: depth limit of 1 only allows top-level fields
		let storage = UserStorage::new();
		let limits = QueryLimits::new(1, 1000);
		let schema = create_schema_with_limits(storage, limits);

		// Act: query with nested selection exceeds depth limit of 1
		let query = r#"
			{
				users {
					name
				}
			}
		"#;
		let result = schema.execute(query).await;

		// Assert: should produce a depth-limit error
		assert!(
			!result.errors.is_empty(),
			"expected depth limit error but query succeeded"
		);
		let error_message = &result.errors[0].message;
		assert!(
			error_message.to_lowercase().contains("too deep"),
			"expected depth-limit message, got: {error_message}"
		);
	}

	#[tokio::test]
	async fn test_query_depth_limit_allows_shallow_query() {
		// Arrange
		let storage = UserStorage::new();
		let limits = QueryLimits::new(10, 1000);
		let schema = create_schema_with_limits(storage, limits);

		// Act
		let query = r#"{ hello(name: "Test") }"#;
		let result = schema.execute(query).await;

		// Assert
		assert!(
			result.errors.is_empty(),
			"expected no errors for shallow query"
		);
		let data = result.data.into_json().unwrap();
		assert_eq!(data["hello"], "Hello, Test!");
	}

	#[tokio::test]
	async fn test_query_complexity_limit_rejects_complex_query() {
		// Arrange: very low complexity limit
		let storage = UserStorage::new();
		let limits = QueryLimits::new(100, 1);
		let schema = create_schema_with_limits(storage, limits);

		// Act: query with multiple fields exceeds complexity of 1
		let query = r#"
			{
				users {
					id
					name
					email
					active
				}
			}
		"#;
		let result = schema.execute(query).await;

		// Assert: should produce a complexity-limit error
		assert!(
			!result.errors.is_empty(),
			"expected complexity limit error but query succeeded"
		);
		let error_message = &result.errors[0].message;
		assert!(
			error_message.to_lowercase().contains("complex"),
			"expected complexity-limit message, got: {error_message}"
		);
	}

	#[tokio::test]
	async fn test_query_limits_default_values() {
		// Arrange / Act
		let limits = QueryLimits::default();

		// Assert
		assert_eq!(limits.max_depth, DEFAULT_MAX_QUERY_DEPTH);
		assert_eq!(limits.max_complexity, DEFAULT_MAX_QUERY_COMPLEXITY);
	}

	#[tokio::test]
	async fn test_create_schema_with_custom_limits() {
		// Arrange
		let storage = UserStorage::new();
		let limits = QueryLimits::new(20, 500);
		let schema = create_schema_with_limits(storage, limits);

		// Act: simple query within limits
		let query = r#"{ hello }"#;
		let result = schema.execute(query).await;

		// Assert
		assert!(result.errors.is_empty());
		let data = result.data.into_json().unwrap();
		assert_eq!(data["hello"], "Hello, World!");
	}

	#[tokio::test]
	async fn test_analyzer_extension_present() {
		// Arrange
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		// Act: execute query and check for complexity/depth in extensions
		let query = r#"{ hello(name: "Analyzer") }"#;
		let result = schema.execute(query).await;

		// Assert: Analyzer extension adds complexity/depth to response extensions
		assert!(result.errors.is_empty());
		assert!(
			!result.extensions.is_empty(),
			"expected Analyzer extension data in response"
		);
	}

	#[rstest::rstest]
	#[case(
		"{\n  user(name: \"hello \\\"world\\\"\") {\n    id\n  }\n}",
		2,
		"escaped quotes inside string should not affect field count"
	)]
	#[case(
		"{\n  user(name: \"hello \\\\\\\"end\") {\n    id\n    name\n  }\n}",
		3,
		"escaped backslash before quote should correctly toggle string state"
	)]
	#[case(
		"{\n  user(name: \"no escapes\") {\n    id\n  }\n}",
		2,
		"string without escapes should count fields normally"
	)]
	#[case(
		"{\n  user(name: \"a\\\"b\\\"c\") {\n    id\n    name\n    email\n  }\n}",
		4,
		"multiple escaped quotes in a single string literal"
	)]
	fn test_count_query_fields_with_escaped_strings(
		#[case] query: &str,
		#[case] expected: usize,
		#[case] description: &str,
	) {
		// Arrange — query and expected count provided by rstest parametrization

		// Act
		let count = count_query_fields(query);

		// Assert
		assert_eq!(count, expected, "{}", description);
	}

	#[rstest::rstest]
	#[case(
		"{ users { id name email } }",
		4,
		"parent field plus multiple fields on same line within sub-selection"
	)]
	#[case(
		"{ users { id } }",
		2,
		"parent field plus single field on same line within sub-selection"
	)]
	#[case(
		"{ users { id name } posts { title body } }",
		6,
		"two parent fields plus their sub-selection fields on same line"
	)]
	fn test_count_query_fields_same_line(
		#[case] query: &str,
		#[case] expected: usize,
		#[case] description: &str,
	) {
		// Arrange — query and expected count provided by rstest parametrization

		// Act
		let count = count_query_fields(query);

		// Assert
		assert_eq!(count, expected, "{}", description);
	}

	#[rstest::rstest]
	#[case(
		"{ ... on User { id name } }",
		2,
		"inline fragment type condition should not be counted as a field"
	)]
	#[case(
		"{ users { ... on Admin { role } ... on Member { level } } }",
		3,
		"multiple inline fragments: users + role + level, type names excluded"
	)]
	fn test_count_query_fields_inline_fragments(
		#[case] query: &str,
		#[case] expected: usize,
		#[case] description: &str,
	) {
		// Arrange — query and expected count provided by rstest parametrization

		// Act
		let count = count_query_fields(query);

		// Assert
		assert_eq!(count, expected, "{}", description);
	}

	#[rstest::rstest]
	#[case(
		"{ user(bio: \"\"\"multi\nline\"\"\") { id } }",
		2,
		"block string argument content should not be counted as fields"
	)]
	#[case(
		"{ user(desc: \"\"\"has identifier inside\"\"\") { id name } }",
		3,
		"block string with identifier-like content should not affect field count"
	)]
	fn test_count_query_fields_block_strings(
		#[case] query: &str,
		#[case] expected: usize,
		#[case] description: &str,
	) {
		// Arrange — query and expected count provided by rstest parametrization

		// Act
		let count = count_query_fields(query);

		// Assert
		assert_eq!(count, expected, "{}", description);
	}

	#[tokio::test]
	async fn test_exceeds_max_chars_short_circuits() {
		// Arrange / Act / Assert
		assert!(!exceeds_max_chars("hello", 5)); // exactly at limit
		assert!(exceeds_max_chars("hello!", 5)); // one over
		assert!(!exceeds_max_chars("", 0)); // empty at zero limit
		assert!(exceeds_max_chars("a", 0)); // single char over zero limit
	}

	#[tokio::test]
	async fn test_create_user_accepts_multibyte_name_within_limit() {
		// Arrange: CJK characters are multi-byte in UTF-8 but each is 1 char
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		// 4 CJK characters = 4 chars (well under MAX_NAME_LENGTH of 100)
		let query = r#"
			mutation {
				createUser(input: { name: "田中太郎", email: "tanaka@example.com" }) {
					name
				}
			}
		"#;

		// Act
		let result = schema.execute(query).await;

		// Assert: should succeed because character count is within limit
		assert!(
			result.errors.is_empty(),
			"expected success for multi-byte name within limit, got: {:?}",
			result.errors
		);
		let data = result.data.into_json().unwrap();
		assert_eq!(data["createUser"]["name"], "田中太郎");
	}

	#[tokio::test]
	async fn test_create_user_rejects_multibyte_name_over_limit() {
		// Arrange: build a name with exactly MAX_NAME_LENGTH + 1 CJK characters
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		let long_name: String = "あ".repeat(MAX_NAME_LENGTH + 1);
		let query = format!(
			r#"mutation {{ createUser(input: {{ name: "{}", email: "test@example.com" }}) {{ id }} }}"#,
			long_name
		);

		// Act
		let result = schema.execute(&query).await;

		// Assert: should reject because character count exceeds limit
		assert!(
			!result.errors.is_empty(),
			"expected validation error for name exceeding {} characters",
			MAX_NAME_LENGTH
		);
	}

	#[tokio::test]
	async fn test_create_user_accepts_emoji_name_at_limit() {
		// Arrange: emoji are multi-byte in UTF-8 but each is 1 char count
		let storage = UserStorage::new();
		let schema = create_schema(storage);

		// Exactly MAX_NAME_LENGTH emoji characters
		// Note: name validation only allows alphanumeric, spaces, underscores,
		// hyphens, and dots, so emoji will be rejected by the character check,
		// not the length check. We test length via CJK instead.
		// Here we verify that a name at exactly the limit passes length validation.
		let name_at_limit: String = "a".repeat(MAX_NAME_LENGTH);
		let query = format!(
			r#"mutation {{ createUser(input: {{ name: "{}", email: "test@example.com" }}) {{ id }} }}"#,
			name_at_limit
		);

		// Act
		let result = schema.execute(&query).await;

		// Assert: should succeed (exactly at limit)
		assert!(
			result.errors.is_empty(),
			"expected success for name at exactly the limit, got: {:?}",
			result.errors
		);
	}
}
