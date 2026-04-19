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
//! - **`e2e`**: Enable E2E browser testing utilities via fantoccini/WebDriver

// Re-export modules from reinhardt-testkit for backward-compatible module paths
#[cfg(native)]
pub use reinhardt_testkit::{
	assertions, client, debug, factory, http, logging, mock, resource, response, server, testcase,
	views, websocket,
};

#[cfg(all(native, feature = "messages"))]
pub use reinhardt_testkit::messages;

#[cfg(all(native, feature = "viewsets"))]
pub use reinhardt_testkit::viewsets;

#[cfg(all(native, feature = "testcontainers"))]
pub use reinhardt_testkit::containers;

#[cfg(all(native, feature = "static"))]
pub use reinhardt_testkit::static_files;

// Re-export testcontainers crates for convenient access via reinhardt::test::testcontainers
#[cfg(all(native, feature = "testcontainers"))]
pub use reinhardt_testkit::testcontainers;

#[cfg(all(native, feature = "testcontainers"))]
pub use reinhardt_testkit::testcontainers_modules;

// Re-export reinhardt_urls for downstream crates
#[cfg(native)]
pub use reinhardt_testkit::reinhardt_urls;

// Modules that remain in reinhardt-test (depend on functional crates)
pub mod fixtures;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "msw")]
pub mod msw;

#[cfg(feature = "server-fn-test")]
pub mod server_fn;

// Re-exports for impl_test_model! macro
#[cfg(native)]
#[doc(hidden)]
pub use reinhardt_testkit::inspection;
#[cfg(native)]
#[doc(hidden)]
pub use reinhardt_testkit::paste;
#[cfg(native)]
#[doc(hidden)]
pub use reinhardt_testkit::relationship;
#[cfg(native)]
#[doc(hidden)]
pub use reinhardt_testkit::{FieldSelector, Model};

// Re-export the impl_test_model! macro from testkit
#[cfg(native)]
pub use reinhardt_testkit::impl_test_model;

// Re-export poll_until function from testkit
#[cfg(native)]
pub use reinhardt_testkit::poll_until;

// ============================================================================
// Flat re-exports for backward compatibility
// ============================================================================

#[cfg(native)]
pub use reinhardt_testkit::{
	APIClient, APIClientBuilder, APIRequestFactory, APITestCase, AsyncTeardownGuard,
	AsyncTestResource, BodyEchoHandler, CallRecord, ClientError, DebugEntry, DebugPanel,
	DebugToolbar, DelayedHandler, EchoPathHandler, ErrorKind, HttpVersion, LargeResponseHandler,
	MethodEchoHandler, MockFunction, RequestBuilder, ResponseExt, RouterHandler, SimpleHandler,
	Spy, SqlQuery, StatusCodeHandler, SuiteGuard, SuiteResource, TeardownGuard, TestResource,
	TestResponse, TimingInfo, WebSocketTestClient, acquire_suite, assert_has_header,
	assert_header_contains, assert_header_equals, assert_no_header, assert_status,
	create_api_test_objects, create_insecure_request, create_json_request,
	create_large_test_objects, create_request, create_request_with_headers,
	create_request_with_path_params, create_response_with_headers, create_response_with_status,
	create_secure_request, create_test_objects, create_test_request, create_test_response,
	extract_json, get_header, has_header, header_contains, header_equals, init_test_logging,
	shutdown_test_server, spawn_test_server,
};

#[cfg(native)]
pub use reinhardt_testkit::auth;

#[cfg(all(native, feature = "messages"))]
pub use reinhardt_testkit::{
	MessagesTestMixin, assert_message_count, assert_message_exists, assert_message_level,
	assert_message_tags, assert_messages,
};

// Re-export view types (avoid conflict with create_request)
#[cfg(native)]
pub use reinhardt_testkit::create_view_request;
#[cfg(native)]
pub use reinhardt_testkit::{ApiTestModel, ErrorTestView, SimpleTestView, TestModel};

#[cfg(all(native, feature = "viewsets"))]
pub use reinhardt_testkit::{SimpleViewSet, TestViewSet};

// Re-export commonly used types for testing
#[cfg(native)]
pub use reinhardt_testkit::ServerRouter;

// Fixture re-exports for backward compatibility
#[cfg(native)]
pub use fixtures::{
	Factory, FactoryBuilder, FixtureError, FixtureLoader, FixtureResult, api_client_from_url,
	random_test_key, test_config_value, test_server_guard,
};

#[cfg(all(native, feature = "testcontainers"))]
pub use fixtures::{postgres_container, redis_container};

#[cfg(all(native, feature = "testcontainers"))]
pub use reinhardt_testkit::containers::{
	MailpitContainer, MySqlContainer, PostgresContainer, RabbitMQContainer, RedisContainer,
	TestDatabase, with_mailpit, with_mysql, with_postgres, with_rabbitmq, with_redis,
};

#[cfg(all(native, feature = "static"))]
pub use reinhardt_testkit::static_files::*;

// E2E browser testing re-exports (native target only)
#[cfg(all(feature = "e2e", native))]
pub use fixtures::wasm::e2e::{
	BrowserClient, BrowserConfig, BrowserType, browser_client, browser_config,
};

// E2E browser testing via CDP re-exports (native target only)
#[cfg(all(feature = "e2e-cdp", not(target_arch = "wasm32")))]
pub use fixtures::wasm::e2e_cdp::{CdpBrowser, CdpConfig, CdpPage, cdp_browser, cdp_config};

/// Re-export commonly used testing types
#[cfg(native)]
pub mod prelude {
	pub use reinhardt_testkit::prelude::*;

	// Add poll_until which is in testkit's prelude
	pub use reinhardt_testkit::poll_until;
}
