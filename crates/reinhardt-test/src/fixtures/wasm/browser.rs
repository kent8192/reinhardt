//! In-browser WASM frontend testing fixtures.
//!
//! This module provides rstest fixtures for testing WASM frontends,
//! including DOM queries, event simulation, and mock infrastructure.
//!
//! **Note**: These fixtures are only available when targeting `wasm32`
//! and the `wasm` feature is enabled.
//!
//! # Features
//!
//! - `screen`: DOM query interface for finding elements
//! - `mock_storage`: Mock localStorage/sessionStorage
//! - `mock_cookies`: Mock document.cookie
//! - `mock_fetch`: Mock fetch API responses
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::fixtures::wasm::*;
//! use wasm_bindgen_test::*;
//!
//! wasm_bindgen_test_configure!(run_in_browser);
//!
//! #[wasm_bindgen_test]
//! async fn test_component() {
//!     let screen = screen();
//!
//!     // Find and interact with elements
//!     let button = screen.get_by_role_with_name("button", "Submit").get();
//!     // ...
//! }
//! ```

use rstest::*;

use crate::wasm::{MockCookies, MockFetch, MockStorage, Screen};

// ============================================================================
// Screen / Query Fixtures
// ============================================================================

/// Fixture providing a Screen for DOM queries.
///
/// The Screen provides Testing Library-style query methods for finding
/// elements by role, text, label, and other accessibility properties.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::screen;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// fn test_element_query(screen: Screen) {
///     let heading = screen.get_by_role("heading").get();
///     // ...
/// }
/// ```
#[fixture]
pub fn screen() -> Screen {
	Screen::new()
}

// ============================================================================
// Mock Storage Fixtures
// ============================================================================

/// Fixture providing an empty mock localStorage.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::mock_local_storage;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// fn test_storage_access(mock_local_storage: MockStorage) {
///     mock_local_storage.set("key", "value");
///     assert_eq!(mock_local_storage.get("key"), Some("value".to_string()));
/// }
/// ```
#[fixture]
pub fn mock_local_storage() -> MockStorage {
	MockStorage::new()
}

/// Fixture providing an empty mock sessionStorage.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::mock_session_storage;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// fn test_session_storage(mock_session_storage: MockStorage) {
///     mock_session_storage.set("session_key", "session_value");
///     // ...
/// }
/// ```
#[fixture]
pub fn mock_session_storage() -> MockStorage {
	MockStorage::new()
}

/// Fixture providing mock localStorage pre-populated with common test data.
///
/// Pre-populates with:
/// - `user_id`: "test-user-123"
/// - `theme`: "dark"
/// - `locale`: "en-US"
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::populated_storage;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// fn test_with_existing_data(populated_storage: MockStorage) {
///     assert_eq!(populated_storage.get("user_id"), Some("test-user-123".to_string()));
/// }
/// ```
#[fixture]
pub fn populated_storage() -> MockStorage {
	let storage = MockStorage::new();
	storage.set_item("user_id", "test-user-123");
	storage.set_item("theme", "dark");
	storage.set_item("locale", "en-US");
	storage
}

// ============================================================================
// Mock Cookies Fixtures
// ============================================================================

/// Fixture providing an empty mock cookies store.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::mock_cookies;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// fn test_cookie_handling(mock_cookies: MockCookies) {
///     mock_cookies.set("session", "abc123");
///     assert!(mock_cookies.get("session").is_some());
/// }
/// ```
#[fixture]
pub fn mock_cookies() -> MockCookies {
	MockCookies::new()
}

/// Fixture providing mock cookies with a session cookie.
///
/// Pre-populates with a `session_id` cookie.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::session_cookies;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// fn test_with_session(session_cookies: MockCookies) {
///     assert!(session_cookies.get("session_id").is_some());
/// }
/// ```
#[fixture]
pub fn session_cookies() -> MockCookies {
	let cookies = MockCookies::new();
	cookies.set("session_id", "test-session-abc123");
	cookies
}

// ============================================================================
// Mock Fetch Fixtures
// ============================================================================

/// Fixture providing an empty mock fetch handler.
///
/// Use this to set up custom fetch responses for your tests.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::mock_fetch;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// async fn test_api_call(mut mock_fetch: MockFetch) {
///     mock_fetch.respond_json("/api/users", &vec!["user1", "user2"]);
///
///     // Component makes fetch to /api/users and gets mocked response
/// }
/// ```
#[fixture]
pub fn mock_fetch() -> MockFetch {
	MockFetch::new()
}

// ============================================================================
// Combination Fixtures
// ============================================================================

/// Test environment combining screen, storage, and cookies.
///
/// Provides a complete test environment for WASM frontend tests.
pub struct WasmTestEnv {
	/// Screen for DOM queries
	pub screen: Screen,
	/// Mock localStorage
	pub local_storage: MockStorage,
	/// Mock sessionStorage
	pub session_storage: MockStorage,
	/// Mock cookies
	pub cookies: MockCookies,
	/// Mock fetch handler
	pub fetch: MockFetch,
}

impl WasmTestEnv {
	/// Create a new WASM test environment.
	pub fn new() -> Self {
		Self {
			screen: Screen::new(),
			local_storage: MockStorage::new(),
			session_storage: MockStorage::new(),
			cookies: MockCookies::new(),
			fetch: MockFetch::new(),
		}
	}
}

impl Default for WasmTestEnv {
	fn default() -> Self {
		Self::new()
	}
}

/// Fixture providing a complete WASM test environment.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::wasm_test_env;
/// use wasm_bindgen_test::*;
///
/// #[wasm_bindgen_test]
/// async fn test_full_component(mut env: WasmTestEnv) {
///     // Set up storage
///     env.local_storage.set("user", "test");
///
///     // Set up fetch mock
///     env.fetch.respond_json("/api/data", &json!({"key": "value"}));
///
///     // Query DOM
///     let element = env.screen.get_by_role("button").query();
/// }
/// ```
#[fixture]
pub fn wasm_test_env() -> WasmTestEnv {
	WasmTestEnv::new()
}

// Note: Tests for WASM fixtures must be run with wasm-bindgen-test
// Use `wasm-pack test --headless --chrome` to run these tests
