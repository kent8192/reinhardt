//! # Reinhardt Test
//!
//! Testing utilities for the Reinhardt framework.
//!
//! ## Overview
//!
//! This crate provides comprehensive testing tools inspired by Django REST Framework,
//! including API clients, request factories, assertions, and TestContainers integration
//! for database testing.
//!
//! ## Features
//!
//! - **[`APIClient`]**: HTTP client for making test API requests
//! - **[`APIRequestFactory`]**: Factory for creating mock HTTP requests
//! - **[`APITestCase`]**: Base test case with common assertions
//! - **Response Assertions**: Status, header, and body assertions
//! - **[`Factory`]**: Model factory for generating test data
//! - **[`DebugToolbar`]**: Debug panel for inspecting queries and timing
//! - **[`WebSocketTestClient`]**: WebSocket connection testing
//! - **TestContainers**: Database containers (PostgreSQL, MySQL, Redis) integration
//!
//! ## Quick Start
//!
//! ### API Client
//!
//! ```rust,ignore
//! use reinhardt_test::{APIClient, assert_status};
//! use hyper::StatusCode;
//!
//! #[tokio::test]
//! async fn test_user_list() {
//!     let client = APIClient::new("http://localhost:8000");
//!
//!     let response = client.get("/api/users/").await.unwrap();
//!     assert_status(&response, StatusCode::OK);
//!
//!     let users: Vec<User> = response.json().await.unwrap();
//!     assert!(!users.is_empty());
//! }
//! ```
//!
//! ### Request Factory
//!
//! ```rust,ignore
//! use reinhardt_test::{APIRequestFactory, create_test_request};
//!
//! #[tokio::test]
//! async fn test_view_directly() {
//!     let factory = APIRequestFactory::new();
//!
//!     // Create a GET request
//!     let request = factory.get("/api/users/").build();
//!
//!     // Create a POST request with JSON body
//!     let request = factory.post("/api/users/")
//!         .json(&json!({"name": "Alice"}))
//!         .build();
//!
//!     // Pass to view handler directly
//!     let response = my_view(request).await;
//! }
//! ```
//!
//! ### Assertions
//!
//! ```rust,ignore
//! use reinhardt_test::{assert_status, assert_has_header, assert_header_equals, extract_json};
//! use hyper::StatusCode;
//!
//! // Status assertions
//! assert_status(&response, StatusCode::OK);
//! assert_status(&response, StatusCode::CREATED);
//!
//! // Header assertions
//! assert_has_header(&response, "Content-Type");
//! assert_header_equals(&response, "Content-Type", "application/json");
//!
//! // Body extraction
//! let data: MyStruct = extract_json(&response).await.unwrap();
//! ```
//!
//! ### TestContainers (Database Testing)
//!
//! Requires the `testcontainers` feature:
//!
//! ```rust,ignore
//! use reinhardt_test::{with_postgres, PostgresContainer};
//!
//! #[tokio::test]
//! async fn test_with_database() {
//!     with_postgres(|db: PostgresContainer| async move {
//!         let connection_url = db.connection_url();
//!
//!         // Run tests against the database
//!         let pool = create_pool(&connection_url).await;
//!         // ...
//!     }).await;
//! }
//! ```
//!
//! ### Model Factory
//!
//! ```rust,ignore
//! use reinhardt_test::{Factory, FactoryBuilder};
//!
//! let user = FactoryBuilder::<User>::new()
//!     .with("name", "Test User")
//!     .with("email", "test@example.com")
//!     .build();
//! ```
//!
//! ## Modules
//!
//! - [`assertions`]: Response assertion utilities
//! - [`client`]: [`APIClient`] for HTTP testing
//! - [`factory`]: [`APIRequestFactory`] for request creation
//! - [`fixtures`]: Test data generation and fixtures
//! - [`http`]: HTTP helper functions
//! - [`mock`]: Mock objects and spies
//! - [`server`]: Test server utilities
//! - [`testcase`]: [`APITestCase`] base class
//! - `containers`: TestContainers integration (requires feature)
//!
//! ## Feature Flags
//!
//! - **`testcontainers`**: Enable TestContainers for database testing
//! - **`static`**: Enable static file testing utilities
//! - **`wasm`**: Enable WASM frontend testing utilities
//! - **`wasm-full`**: Enable WASM testing with full web-sys features
//! - **`server-fn-test`**: Enable server function testing utilities

pub mod assertions;
pub mod client;
pub mod debug;
pub mod factory;
pub mod fixtures;
pub mod http;
pub mod logging;
pub mod messages;
pub mod mock;
pub mod resource;
pub mod response;
pub mod server;
pub mod testcase;
pub mod views;
pub mod viewsets;

#[cfg(feature = "testcontainers")]
pub mod containers;

pub mod websocket;

// Re-export testcontainers crates for convenient access via reinhardt::test::testcontainers
#[cfg(feature = "testcontainers")]
pub use testcontainers;

#[cfg(feature = "testcontainers")]
pub use testcontainers_modules;

#[cfg(feature = "static")]
pub mod static_files;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "server-fn-test")]
pub mod server_fn;

// Re-exports for impl_test_model! macro
#[doc(hidden)]
pub use paste;
#[doc(hidden)]
pub use reinhardt_db::orm::inspection;
#[doc(hidden)]
pub use reinhardt_db::orm::relationship;
#[doc(hidden)]
pub use reinhardt_db::orm::{FieldSelector, Model};

pub use assertions::*;
pub use client::{APIClient, APIClientBuilder, ClientError, HttpVersion};
pub use debug::{DebugEntry, DebugPanel, DebugToolbar, SqlQuery, TimingInfo};
pub use factory::{APIRequestFactory, RequestBuilder};
pub use fixtures::{
	Factory, FactoryBuilder, FixtureError, FixtureLoader, FixtureResult, api_client_from_url,
	random_test_key, test_config_value, test_server_guard,
};

// Re-export commonly used types for testing
pub use reinhardt_urls::routers::ServerRouter;

#[cfg(feature = "testcontainers")]
pub use fixtures::{postgres_container, redis_container};
pub use http::{
	assert_has_header, assert_header_contains, assert_header_equals, assert_no_header,
	assert_status, create_insecure_request, create_request, create_response_with_headers,
	create_response_with_status, create_secure_request, create_test_request, create_test_response,
	extract_json, get_header, has_header, header_contains, header_equals,
};
pub use logging::init_test_logging;
pub use messages::{
	MessagesTestMixin, assert_message_count, assert_message_exists, assert_message_level,
	assert_message_tags, assert_messages,
};
pub use mock::{CallRecord, MockFunction, SimpleHandler, Spy};
pub use resource::{
	AsyncTeardownGuard, AsyncTestResource, SuiteGuard, SuiteResource, TeardownGuard, TestResource,
	acquire_suite,
};
pub use response::{ResponseExt, TestResponse};
pub use server::{
	BodyEchoHandler, DelayedHandler, EchoPathHandler, LargeResponseHandler, MethodEchoHandler,
	RouterHandler, StatusCodeHandler, shutdown_test_server, spawn_test_server,
};
pub use testcase::APITestCase;
pub use views::{
	ApiTestModel, ErrorKind, ErrorTestView, SimpleTestView, TestModel, create_api_test_objects,
	create_json_request, create_large_test_objects, create_request as create_view_request,
	create_request_with_headers, create_request_with_path_params, create_test_objects,
};
pub use viewsets::{SimpleViewSet, TestViewSet};

#[cfg(feature = "testcontainers")]
pub use containers::{
	MailpitContainer, MySqlContainer, PostgresContainer, RabbitMQContainer, RedisContainer,
	TestDatabase, with_mailpit, with_mysql, with_postgres, with_rabbitmq, with_redis,
};

#[cfg(feature = "static")]
pub use static_files::*;

pub use websocket::WebSocketTestClient;

/// Re-export commonly used testing types
pub mod prelude {
	pub use super::assertions::*;
	pub use super::client::APIClient;
	pub use super::debug::DebugToolbar;
	pub use super::factory::APIRequestFactory;
	pub use super::fixtures::{
		Factory, FactoryBuilder, FixtureLoader, api_client_from_url, random_test_key,
		test_config_value,
	};

	#[cfg(feature = "testcontainers")]
	pub use super::fixtures::{postgres_container, redis_container};
	pub use super::http::{
		assert_has_header, assert_header_contains, assert_header_equals, assert_no_header,
		assert_status, create_insecure_request, create_request, create_response_with_headers,
		create_response_with_status, create_secure_request, create_test_request,
		create_test_response, extract_json, get_header, has_header, header_contains, header_equals,
	};
	pub use super::logging::init_test_logging;
	pub use super::messages::{
		MessagesTestMixin, assert_message_count, assert_message_exists, assert_messages,
	};
	pub use super::mock::{MockFunction, SimpleHandler, Spy};
	pub use super::poll_until;
	pub use super::resource::{
		AsyncTeardownGuard, AsyncTestResource, SuiteGuard, SuiteResource, TeardownGuard,
		TestResource, acquire_suite,
	};
	pub use super::response::TestResponse;
	pub use super::server::{
		BodyEchoHandler, DelayedHandler, EchoPathHandler, LargeResponseHandler, MethodEchoHandler,
		RouterHandler, StatusCodeHandler, shutdown_test_server, spawn_test_server,
	};
	#[cfg(feature = "testcontainers")]
	pub use super::testcase::TransactionHandle;
	pub use super::testcase::{APITestCase, TeardownError};
	pub use super::views::{
		ApiTestModel, ErrorTestView, SimpleTestView, TestModel, create_api_test_objects,
		create_test_objects,
	};
	pub use super::viewsets::{SimpleViewSet, TestViewSet};

	#[cfg(feature = "testcontainers")]
	pub use super::containers::{
		MySqlContainer, PostgresContainer, RedisContainer, TestDatabase, with_mysql, with_postgres,
		with_redis,
	};

	#[cfg(feature = "static")]
	pub use super::static_files::*;
}

/// Poll a condition until it becomes true or timeout is reached.
///
/// This is useful for testing asynchronous operations that may take some time to complete,
/// such as cache expiration, rate limit window resets, or background task completion.
///
/// # Arguments
///
/// * `timeout` - Maximum duration to wait for the condition to become true
/// * `interval` - Duration to wait between each poll attempt
/// * `condition` - Async closure that returns `true` when the desired state is reached
///
/// # Returns
///
/// * `Ok(())` if the condition becomes true within the timeout
/// * `Err(String)` if the timeout is reached before the condition becomes true
///
/// # Examples
///
/// ```no_run
/// use reinhardt_test::poll_until;
/// use std::time::Duration;
///
/// # async fn example() {
/// // Poll until a cache entry expires
/// poll_until(
///     Duration::from_millis(200),
///     Duration::from_millis(10),
///     || async {
///         // Check if cache entry has expired
///         // cache.get("key").await.is_none()
///         true
///     }
/// ).await.expect("Condition should be met");
/// # }
/// ```
pub async fn poll_until<F, Fut>(
	timeout: std::time::Duration,
	interval: std::time::Duration,
	mut condition: F,
) -> Result<(), String>
where
	F: FnMut() -> Fut,
	Fut: std::future::Future<Output = bool>,
{
	let start = std::time::Instant::now();
	while start.elapsed() < timeout {
		if condition().await {
			return Ok(());
		}
		tokio::time::sleep(interval).await;
	}
	Err(format!("Timeout after {:?} waiting for condition", timeout))
}

/// Helper macro for implementing Model trait with empty Fields for test models
///
/// This macro generates the boilerplate code needed for test models that don't use
/// the full `#[model(...)]` macro. It creates an empty field selector struct and
/// implements the required Model trait methods.
///
/// # Usage
///
/// ```ignore
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct TestUser {
///     id: Option<i64>,
///     name: String,
/// }
///
/// impl_test_model!(TestUser, i64, "test_users");
/// ```
///
/// This expands to:
/// - A `TestUserFields` struct that implements `FieldSelector`
/// - A complete `Model` trait implementation for `TestUser`
///
/// # Parameters
///
/// - `$model`: The model struct name
/// - `$pk`: The primary key type
/// - `$table`: The table name as a string literal
/// - `$app`: The application label as a string literal (optional, defaults to "default")
/// - `relationships`: Optional relationship definitions (see examples below)
///
/// # Constraints
/// - Model must have an `id: Option<PrimaryKey>` field
/// - Primary key field name is fixed to `"id"`
///
/// # Examples
///
/// ## Basic usage
/// ```ignore
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct User {
///     id: Option<i64>,
///     name: String,
/// }
///
/// // With app_label
/// reinhardt_test::impl_test_model!(User, i64, "users", "auth");
///
/// // Without app_label (defaults to "default")
/// reinhardt_test::impl_test_model!(Product, i32, "products");
/// ```
///
/// ## With relationships
/// ```ignore
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Author {
///     id: Option<i32>,
///     name: String,
/// }
///
/// // OneToMany relationship
/// reinhardt_test::impl_test_model!(
///     Author, i32, "authors", "test",
///     relationships: [
///         (OneToMany, "books", "Book", "author_id", "author")
///     ]
/// );
///
/// // Multiple relationships
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Book {
///     id: Option<i32>,
///     title: String,
///     author_id: i32,
///     publisher_id: i32,
/// }
///
/// reinhardt_test::impl_test_model!(
///     Book, i32, "books", "test",
///     relationships: [
///         (ManyToOne, "author", "Author", "author_id", "books"),
///         (ManyToOne, "publisher", "Publisher", "publisher_id", "books"),
///         (OneToMany, "reviews", "Review", "book_id", "book")
///     ]
/// );
/// ```
#[macro_export]
macro_rules! impl_test_model {
	// Composite version (OneToMany/ManyToOne + ManyToMany) - HIGHEST PRIORITY MATCHING
	(
		$model:ident,
		$pk:ty,
		$table:expr,
		$app:expr,
		relationships: [
			$(($rel_type:ident, $rel_name:expr, $related:expr, $fk:expr, $back_pop:expr)),* $(,)?
		],
		many_to_many: [
			$(($m2m_name:expr, $m2m_related:expr, $m2m_through:expr, $m2m_source:expr, $m2m_target:expr)),* $(,)?
		]
	) => {
		$crate::paste::paste! {
			#[derive(Debug, Clone)]
			pub struct [<$model Fields>];

			impl $crate::FieldSelector for [<$model Fields>] {
				fn with_alias(self, _alias: &str) -> Self {
					self
				}
			}

			impl $crate::Model for $model {
				type PrimaryKey = $pk;
				type Fields = [<$model Fields>];

				fn table_name() -> &'static str {
					$table
				}

				fn app_label() -> &'static str {
					$app
				}

				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					self.id
				}

				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					self.id = Some(value);
				}

				fn primary_key_field() -> &'static str {
					"id"
				}

				fn new_fields() -> Self::Fields {
					[<$model Fields>]
				}

				fn relationship_metadata() -> Vec<$crate::inspection::RelationInfo> {
					// OneToMany/ManyToOne/OneToOne relationships
					let mut relations = vec![
						$(
							$crate::inspection::RelationInfo {
								name: $rel_name.to_string(),
								relationship_type: $crate::relationship::RelationshipType::$rel_type,
								related_model: $related.to_string(),
								foreign_key: Some($fk.to_string()),
								back_populates: Some($back_pop.to_string()),
								through_table: None,
								source_field: None,
								target_field: None,
							}
						),*
					];

					// ManyToMany relationships
					relations.extend(vec![
						$(
							$crate::inspection::RelationInfo {
								name: $m2m_name.to_string(),
								relationship_type: $crate::relationship::RelationshipType::ManyToMany,
								related_model: $m2m_related.to_string(),
								foreign_key: None,
								back_populates: None,
								through_table: Some($m2m_through.to_string()),
								source_field: Some($m2m_source.to_string()),
								target_field: Some($m2m_target.to_string()),
							}
						),*
					]);

					relations
				}
			}
		}
	};

	// Version with relationships (OneToMany/ManyToOne/OneToOne)
	(
		$model:ident,
		$pk:ty,
		$table:expr,
		$app:expr,
		relationships: [
			$(($rel_type:ident, $rel_name:expr, $related:expr, $fk:expr, $back_pop:expr)),* $(,)?
		]
	) => {
		$crate::paste::paste! {
			#[derive(Debug, Clone)]
			pub struct [<$model Fields>];

			impl $crate::FieldSelector for [<$model Fields>] {
				fn with_alias(self, _alias: &str) -> Self {
					self
				}
			}

			impl $crate::Model for $model {
				type PrimaryKey = $pk;
				type Fields = [<$model Fields>];

				fn table_name() -> &'static str {
					$table
				}

				fn app_label() -> &'static str {
					$app
				}

				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					self.id
				}

				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					self.id = Some(value);
				}

				fn primary_key_field() -> &'static str {
					"id"
				}

				fn new_fields() -> Self::Fields {
					[<$model Fields>]
				}

				fn relationship_metadata() -> Vec<$crate::inspection::RelationInfo> {
					vec![
						$(
							$crate::inspection::RelationInfo {
								name: $rel_name.to_string(),
								relationship_type: $crate::relationship::RelationshipType::$rel_type,
								related_model: $related.to_string(),
								foreign_key: Some($fk.to_string()),
								back_populates: Some($back_pop.to_string()),
								through_table: None,
								source_field: None,
								target_field: None,
							}
						),*
					]
				}
			}
		}
	};

	// ManyToMany only version
	(
		$model:ident,
		$pk:ty,
		$table:expr,
		$app:expr,
		many_to_many: [
			$(($rel_name:expr, $related:expr, $through:expr, $source:expr, $target:expr)),* $(,)?
		]
	) => {
		$crate::paste::paste! {
			#[derive(Debug, Clone)]
			pub struct [<$model Fields>];

			impl $crate::FieldSelector for [<$model Fields>] {
				fn with_alias(self, _alias: &str) -> Self {
					self
				}
			}

			impl $crate::Model for $model {
				type PrimaryKey = $pk;
				type Fields = [<$model Fields>];

				fn table_name() -> &'static str {
					$table
				}

				fn app_label() -> &'static str {
					$app
				}

				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					self.id
				}

				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					self.id = Some(value);
				}

				fn primary_key_field() -> &'static str {
					"id"
				}

				fn new_fields() -> Self::Fields {
					[<$model Fields>]
				}

				fn relationship_metadata() -> Vec<$crate::inspection::RelationInfo> {
					vec![
						$(
							$crate::inspection::RelationInfo {
								name: $rel_name.to_string(),
								relationship_type: $crate::relationship::RelationshipType::ManyToMany,
								related_model: $related.to_string(),
								foreign_key: None,
								back_populates: None,
								through_table: Some($through.to_string()),
								source_field: Some($source.to_string()),
								target_field: Some($target.to_string()),
							}
						),*
					]
				}
			}
		}
	};

	// Version with app_label (no relationships)
	($model:ident, $pk:ty, $table:expr, $app:expr) => {
		$crate::paste::paste! {
			#[derive(Debug, Clone)]
			pub struct [<$model Fields>];

			impl $crate::FieldSelector for [<$model Fields>] {
				fn with_alias(self, _alias: &str) -> Self {
					self
				}
			}

			impl $crate::Model for $model {
				type PrimaryKey = $pk;
				type Fields = [<$model Fields>];

				fn table_name() -> &'static str {
					$table
				}

				fn app_label() -> &'static str {
					$app
				}

				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					self.id
				}

				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					self.id = Some(value);
				}

				fn primary_key_field() -> &'static str {
					"id"
				}

				fn new_fields() -> Self::Fields {
					[<$model Fields>]
				}
			}
		}
	};

	// Backward compatibility: default app_label
	($model:ident, $pk:ty, $table:expr) => {
		$crate::impl_test_model!($model, $pk, $table, "default");
	};

	// Non-option primary key version with app_label
	// Use this when the primary key field is NOT wrapped in Option<T>
	// Example: id: Uuid instead of id: Option<Uuid>
	($model:ident, $pk:ty, $table:expr, $app:expr, non_option_pk) => {
		$crate::paste::paste! {
			#[derive(Debug, Clone)]
			pub struct [<$model Fields>];

			impl $crate::FieldSelector for [<$model Fields>] {
				fn with_alias(self, _alias: &str) -> Self {
					self
				}
			}

			impl $crate::Model for $model {
				type PrimaryKey = $pk;
				type Fields = [<$model Fields>];

				fn table_name() -> &'static str {
					$table
				}

				fn app_label() -> &'static str {
					$app
				}

				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					Some(self.id)
				}

				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					self.id = value;
				}

				fn primary_key_field() -> &'static str {
					"id"
				}

				fn new_fields() -> Self::Fields {
					[<$model Fields>]
				}
			}
		}
	};

	// Non-option primary key version with default app_label
	($model:ident, $pk:ty, $table:expr, non_option_pk) => {
		$crate::impl_test_model!($model, $pk, $table, "default", non_option_pk);
	};
}
