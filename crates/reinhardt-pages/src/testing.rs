//! Testing Utilities for Reinhardt Pages
//!
//! This module provides comprehensive testing utilities for reinhardt-pages
//! applications, supporting a 3-layer test architecture:
//!
//! ## Layer 1: Server Function Unit Tests
//!
//! Direct testing of server functions without HTTP layer overhead.
//! Uses [`ServerFnTestContext`] for dependency injection.
//!
//! ```rust,ignore
//! use reinhardt_pages::testing::ServerFnTestContext;
//!
//! #[tokio::test]
//! async fn test_login() {
//!     let ctx = ServerFnTestContext::new(singleton)
//!         .with_database(pool)
//!         .build();
//!     // Test server function directly
//! }
//! ```
//!
//! ## Layer 2: WASM Component Tests with Mocked HTTP
//!
//! Tests WASM components with mocked server function responses.
//! Uses the mock HTTP infrastructure for predictable testing.
//!
//! ```rust,ignore
//! use reinhardt_pages::testing::{mock_server_fn, clear_mocks, assert_server_fn_called};
//!
//! #[wasm_bindgen_test]
//! async fn test_component() {
//!     mock_server_fn("/api/server_fn/login", &user_info);
//!     // ... render component ...
//!     assert_server_fn_called("/api/server_fn/login");
//!     clear_mocks();
//! }
//! ```
//!
//! ## Layer 3: End-to-End Tests
//!
//! Full integration tests with real server and WASM frontend.
//! Uses E2E test infrastructure for complete flow testing.

// HTTP mock infrastructure (Layer 2) - available on both WASM and server
pub mod mock_fetch;
pub mod mock_http;

pub use mock_fetch::fetch_with_mock;
#[allow(deprecated)]
pub use mock_http::{
	MockCall, MockResponse, assert_server_fn_call_count, assert_server_fn_called,
	assert_server_fn_called_with, assert_server_fn_not_called, clear_mocks, get_call_history,
	get_call_history_for, mock_server_fn, mock_server_fn_custom, mock_server_fn_error,
};

// E2E test infrastructure (Layer 3)
pub mod e2e;

pub use e2e::{
	E2E_SERVER_URL_KEY, E2ETestConfig, E2ETestError, get_e2e_server_url, is_e2e_test_mode,
	set_e2e_server_url,
};

#[cfg(native)]
pub use e2e::E2ETestEnv;

#[cfg(wasm)]
pub use e2e::e2e_fetch;

// Server-side testing utilities (Layer 1)
#[cfg(native)]
pub mod server_fn_test;

#[cfg(native)]
pub use server_fn_test::*;

// WASM DOM testing utilities (Layer 2 and 3)
#[cfg(wasm)]
pub mod wasm;

#[cfg(wasm)]
pub use wasm::*;
