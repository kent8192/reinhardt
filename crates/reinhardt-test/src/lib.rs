#![warn(missing_docs)]
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
//! Most functionality is provided by `reinhardt-testkit`. This crate adds testing
//! utilities that depend on functional crates (reinhardt-auth, reinhardt-admin,
//! reinhardt-pages, reinhardt-tasks).
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
//! ## Feature Flags
//!
//! - **`testcontainers`**: Enable TestContainers for database testing
//! - **`static`**: Enable static file testing utilities
//! - **`wasm`**: Enable WASM frontend testing utilities
//! - **`wasm-full`**: Enable WASM testing with full web-sys features
//! - **`server-fn-test`**: Enable server function testing utilities
//! - **`tasks`**: Enable task queue testing utilities
//! - **`admin`**: Enable admin panel testing utilities

// Re-export modules from reinhardt-testkit for backward-compatible module paths
pub use reinhardt_testkit::{
	assertions, client, debug, factory, http, logging, messages, mock, resource, response, server,
	testcase, views, viewsets, websocket,
};

#[cfg(feature = "testcontainers")]
pub use reinhardt_testkit::containers;

#[cfg(feature = "static")]
pub use reinhardt_testkit::static_files;

// Re-export testcontainers crates for convenient access via reinhardt::test::testcontainers
#[cfg(feature = "testcontainers")]
pub use reinhardt_testkit::testcontainers;

#[cfg(feature = "testcontainers")]
pub use reinhardt_testkit::testcontainers_modules;

// Re-export reinhardt_urls for downstream crates
pub use reinhardt_testkit::reinhardt_urls;

// Modules that remain in reinhardt-test (depend on functional crates)
pub mod fixtures;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "server-fn-test")]
pub mod server_fn;

// Re-exports for impl_test_model! macro
#[doc(hidden)]
pub use reinhardt_testkit::inspection;
#[doc(hidden)]
pub use reinhardt_testkit::paste;
#[doc(hidden)]
pub use reinhardt_testkit::relationship;
#[doc(hidden)]
pub use reinhardt_testkit::{FieldSelector, Model};

// Re-export the impl_test_model! macro from testkit
pub use reinhardt_testkit::impl_test_model;

// Re-export poll_until function from testkit
pub use reinhardt_testkit::poll_until;

// ============================================================================
// Flat re-exports for backward compatibility
// ============================================================================

pub use reinhardt_testkit::{
	APIClient, APIClientBuilder, APIRequestFactory, APITestCase, AsyncTeardownGuard,
	AsyncTestResource, BodyEchoHandler, CallRecord, ClientError, DebugEntry, DebugPanel,
	DebugToolbar, DelayedHandler, EchoPathHandler, ErrorKind, HttpVersion, LargeResponseHandler,
	MessagesTestMixin, MethodEchoHandler, MockFunction, RequestBuilder, ResponseExt, RouterHandler,
	SimpleHandler, Spy, SqlQuery, StatusCodeHandler, SuiteGuard, SuiteResource, TeardownGuard,
	TestResource, TestResponse, TimingInfo, WebSocketTestClient, acquire_suite, assert_has_header,
	assert_header_contains, assert_header_equals, assert_message_count, assert_message_exists,
	assert_message_level, assert_message_tags, assert_messages, assert_no_header, assert_status,
	create_api_test_objects, create_insecure_request, create_json_request,
	create_large_test_objects, create_request, create_request_with_headers,
	create_request_with_path_params, create_response_with_headers, create_response_with_status,
	create_secure_request, create_test_objects, create_test_request, create_test_response,
	extract_json, get_header, has_header, header_contains, header_equals, init_test_logging,
	shutdown_test_server, spawn_test_server,
};

// Re-export view types (avoid conflict with create_request)
pub use reinhardt_testkit::create_view_request;
pub use reinhardt_testkit::{
	ApiTestModel, ErrorTestView, SimpleTestView, SimpleViewSet, TestModel, TestViewSet,
};

// Re-export commonly used types for testing
pub use reinhardt_testkit::ServerRouter;

// Fixture re-exports for backward compatibility
pub use fixtures::{
	Factory, FactoryBuilder, FixtureError, FixtureLoader, FixtureResult, api_client_from_url,
	random_test_key, test_config_value, test_server_guard,
};

#[cfg(feature = "testcontainers")]
pub use fixtures::{postgres_container, redis_container};

#[cfg(feature = "testcontainers")]
pub use reinhardt_testkit::containers::{
	MailpitContainer, MySqlContainer, PostgresContainer, RabbitMQContainer, RedisContainer,
	TestDatabase, with_mailpit, with_mysql, with_postgres, with_rabbitmq, with_redis,
};

#[cfg(feature = "static")]
pub use reinhardt_testkit::static_files::*;

/// Re-export commonly used testing types
pub mod prelude {
	pub use reinhardt_testkit::prelude::*;

	// Add poll_until which is in testkit's prelude
	pub use reinhardt_testkit::poll_until;
}
