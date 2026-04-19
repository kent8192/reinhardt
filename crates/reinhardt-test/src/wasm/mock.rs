#![cfg(wasm)]

//! Mock infrastructure for WASM testing.
//!
//! This module provides mock utilities for server functions, storage, and other
//! browser APIs commonly needed in WASM tests.
//!
//! # Example
//!
//! ```no_run
//! use reinhardt_test::wasm::mock::{MockStorage, MockCookies};
//!
//! // Mock localStorage
//! let storage = MockStorage::new();
//! storage.set_item("user_id", "123");
//! assert_eq!(storage.get_item("user_id"), Some("123".to_string()));
//!
//! // Mock cookies
//! let cookies = MockCookies::new();
//! cookies.set("session", "abc123");
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

// Re-export server function mocking from reinhardt-pages
#[allow(deprecated)]
pub use reinhardt_pages::testing::{
	MockResponse, assert_server_fn_call_count, assert_server_fn_called,
	assert_server_fn_called_with, assert_server_fn_not_called, clear_mocks, get_call_history,
	mock_server_fn, mock_server_fn_error,
};

/// Mock implementation for Web Storage (localStorage/sessionStorage).
///
/// This allows testing code that uses browser storage without affecting
/// the actual browser storage.
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::wasm::mock::MockStorage;
///
/// let storage = MockStorage::new();
/// storage.set_item("key", "value");
/// assert_eq!(storage.get_item("key"), Some("value".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockStorage {
	data: Rc<RefCell<HashMap<String, String>>>,
}

impl MockStorage {
	/// Create a new empty mock storage.
	pub fn new() -> Self {
		Self::default()
	}

	/// Create a mock storage with initial data.
	pub fn with_data(data: HashMap<String, String>) -> Self {
		Self {
			data: Rc::new(RefCell::new(data)),
		}
	}

	/// Get the number of items in storage.
	pub fn length(&self) -> usize {
		self.data.borrow().len()
	}

	/// Get an item by key.
	pub fn get_item(&self, key: &str) -> Option<String> {
		self.data.borrow().get(key).cloned()
	}

	/// Set an item.
	pub fn set_item(&self, key: &str, value: &str) {
		self.data
			.borrow_mut()
			.insert(key.to_string(), value.to_string());
	}

	/// Remove an item by key.
	pub fn remove_item(&self, key: &str) {
		self.data.borrow_mut().remove(key);
	}

	/// Clear all items.
	pub fn clear(&self) {
		self.data.borrow_mut().clear();
	}

	/// Get a key by index.
	pub fn key(&self, index: usize) -> Option<String> {
		self.data.borrow().keys().nth(index).cloned()
	}

	/// Get all keys.
	pub fn keys(&self) -> Vec<String> {
		self.data.borrow().keys().cloned().collect()
	}

	/// Get all values.
	pub fn values(&self) -> Vec<String> {
		self.data.borrow().values().cloned().collect()
	}

	/// Get all entries as key-value pairs.
	pub fn entries(&self) -> Vec<(String, String)> {
		self.data
			.borrow()
			.iter()
			.map(|(k, v)| (k.clone(), v.clone()))
			.collect()
	}

	/// Check if a key exists.
	pub fn contains_key(&self, key: &str) -> bool {
		self.data.borrow().contains_key(key)
	}
}

/// Mock implementation for browser cookies.
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::wasm::mock::MockCookies;
///
/// let cookies = MockCookies::new();
/// cookies.set("session_id", "abc123");
/// cookies.set_with_options("auth_token", "xyz789", CookieOptions {
///     max_age: Some(3600),
///     secure: true,
///     ..Default::default()
/// });
///
/// assert_eq!(cookies.get("session_id"), Some("abc123".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockCookies {
	cookies: Rc<RefCell<HashMap<String, CookieEntry>>>,
}

/// Cookie entry with value and options.
#[derive(Debug, Clone)]
pub struct CookieEntry {
	/// The cookie value.
	pub value: String,
	/// Cookie options.
	pub options: CookieOptions,
}

/// Options for setting cookies.
#[derive(Debug, Clone, Default)]
pub struct CookieOptions {
	/// Max-Age in seconds.
	pub max_age: Option<i64>,
	/// Expiration date as Unix timestamp.
	pub expires: Option<i64>,
	/// Cookie path.
	pub path: Option<String>,
	/// Cookie domain.
	pub domain: Option<String>,
	/// Secure flag.
	pub secure: bool,
	/// HttpOnly flag.
	pub http_only: bool,
	/// SameSite attribute.
	pub same_site: Option<SameSite>,
}

/// SameSite cookie attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
	/// Strict mode - only same-site requests.
	Strict,
	/// Lax mode - same-site and top-level navigation.
	Lax,
	/// None - cross-site requests allowed (requires Secure).
	None,
}

impl MockCookies {
	/// Create a new empty mock cookies instance.
	pub fn new() -> Self {
		Self::default()
	}

	/// Create mock cookies with initial data.
	pub fn with_cookies(cookies: HashMap<String, String>) -> Self {
		let entries: HashMap<String, CookieEntry> = cookies
			.into_iter()
			.map(|(k, v)| {
				(
					k,
					CookieEntry {
						value: v,
						options: CookieOptions::default(),
					},
				)
			})
			.collect();

		Self {
			cookies: Rc::new(RefCell::new(entries)),
		}
	}

	/// Get a cookie value by name.
	pub fn get(&self, name: &str) -> Option<String> {
		self.cookies
			.borrow()
			.get(name)
			.map(|entry| entry.value.clone())
	}

	/// Get a cookie entry with options by name.
	pub fn get_entry(&self, name: &str) -> Option<CookieEntry> {
		self.cookies.borrow().get(name).cloned()
	}

	/// Set a cookie with default options.
	pub fn set(&self, name: &str, value: &str) {
		self.set_with_options(name, value, CookieOptions::default());
	}

	/// Set a cookie with custom options.
	pub fn set_with_options(&self, name: &str, value: &str, options: CookieOptions) {
		self.cookies.borrow_mut().insert(
			name.to_string(),
			CookieEntry {
				value: value.to_string(),
				options,
			},
		);
	}

	/// Remove a cookie by name.
	pub fn remove(&self, name: &str) {
		self.cookies.borrow_mut().remove(name);
	}

	/// Clear all cookies.
	pub fn clear(&self) {
		self.cookies.borrow_mut().clear();
	}

	/// Get all cookie names.
	pub fn names(&self) -> Vec<String> {
		self.cookies.borrow().keys().cloned().collect()
	}

	/// Get all cookies as name-value pairs.
	pub fn all(&self) -> HashMap<String, String> {
		self.cookies
			.borrow()
			.iter()
			.map(|(k, v)| (k.clone(), v.value.clone()))
			.collect()
	}

	/// Check if a cookie exists.
	pub fn has(&self, name: &str) -> bool {
		self.cookies.borrow().contains_key(name)
	}

	/// Get the number of cookies.
	pub fn len(&self) -> usize {
		self.cookies.borrow().len()
	}

	/// Check if there are no cookies.
	pub fn is_empty(&self) -> bool {
		self.cookies.borrow().is_empty()
	}

	/// Generate a cookie string in the format used by document.cookie.
	pub fn to_cookie_string(&self) -> String {
		self.cookies
			.borrow()
			.iter()
			.map(|(name, entry)| format!("{}={}", name, entry.value))
			.collect::<Vec<_>>()
			.join("; ")
	}
}

/// Mock implementation for fetch API.
///
/// This allows intercepting and mocking fetch requests in tests.
///
/// # Example
///
/// ```no_run
/// use reinhardt_test::wasm::mock::{MockFetch, MockFetchResponse};
///
/// let fetch = MockFetch::new();
/// fetch.mock_response("/api/users", MockFetchResponse::json(vec![
///     serde_json::json!({"id": 1, "name": "Alice"}),
/// ]));
///
/// // Later, when code calls fetch("/api/users"), it will receive the mocked response
/// ```
#[deprecated(
	since = "0.1.0-rc.16",
	note = "Use `MockServiceWorker` from `reinhardt_test::msw` instead. See issue #3283."
)]
#[derive(Debug, Clone, Default)]
pub struct MockFetch {
	responses: Rc<RefCell<HashMap<String, MockFetchResponse>>>,
	calls: Rc<RefCell<Vec<MockFetchCall>>>,
}

/// A recorded fetch call.
#[derive(Debug, Clone)]
pub struct MockFetchCall {
	/// The URL that was fetched.
	pub url: String,
	/// The HTTP method used.
	pub method: String,
	/// Request headers.
	pub headers: HashMap<String, String>,
	/// Request body if any.
	pub body: Option<String>,
}

/// A mock response for fetch.
#[derive(Debug, Clone)]
pub struct MockFetchResponse {
	/// HTTP status code.
	pub status: u16,
	/// Status text.
	pub status_text: String,
	/// Response headers.
	pub headers: HashMap<String, String>,
	/// Response body.
	pub body: String,
	/// Whether to simulate a network error.
	pub network_error: bool,
}

impl MockFetchResponse {
	/// Create a successful JSON response.
	pub fn json<T: serde::Serialize>(data: T) -> Self {
		let body = serde_json::to_string(&data).unwrap_or_default();
		Self {
			status: 200,
			status_text: "OK".to_string(),
			headers: [("content-type".to_string(), "application/json".to_string())]
				.into_iter()
				.collect(),
			body,
			network_error: false,
		}
	}

	/// Create a successful text response.
	pub fn text(body: impl Into<String>) -> Self {
		Self {
			status: 200,
			status_text: "OK".to_string(),
			headers: [("content-type".to_string(), "text/plain".to_string())]
				.into_iter()
				.collect(),
			body: body.into(),
			network_error: false,
		}
	}

	/// Create an error response.
	pub fn error(status: u16, message: impl Into<String>) -> Self {
		Self {
			status,
			status_text: http_status_text(status).to_string(),
			headers: HashMap::new(),
			body: message.into(),
			network_error: false,
		}
	}

	/// Create a network error response.
	pub fn network_error() -> Self {
		Self {
			status: 0,
			status_text: String::new(),
			headers: HashMap::new(),
			body: String::new(),
			network_error: true,
		}
	}

	/// Set a response header.
	pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.headers.insert(name.into(), value.into());
		self
	}

	/// Set the response status.
	pub fn with_status(mut self, status: u16) -> Self {
		self.status = status;
		self.status_text = http_status_text(status).to_string();
		self
	}
}

impl MockFetch {
	/// Create a new mock fetch instance.
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a mock response for a URL.
	pub fn mock_response(&self, url: impl Into<String>, response: MockFetchResponse) {
		self.responses.borrow_mut().insert(url.into(), response);
	}

	/// Remove a mock response.
	pub fn remove_mock(&self, url: &str) {
		self.responses.borrow_mut().remove(url);
	}

	/// Clear all mocks.
	pub fn clear_mocks(&self) {
		self.responses.borrow_mut().clear();
	}

	/// Get all recorded calls.
	pub fn calls(&self) -> Vec<MockFetchCall> {
		self.calls.borrow().clone()
	}

	/// Get calls to a specific URL.
	pub fn calls_to(&self, url: &str) -> Vec<MockFetchCall> {
		self.calls
			.borrow()
			.iter()
			.filter(|call| call.url == url)
			.cloned()
			.collect()
	}

	/// Clear recorded calls.
	pub fn clear_calls(&self) {
		self.calls.borrow_mut().clear();
	}

	/// Check if a URL was called.
	pub fn was_called(&self, url: &str) -> bool {
		self.calls.borrow().iter().any(|call| call.url == url)
	}

	/// Get the number of times a URL was called.
	pub fn call_count(&self, url: &str) -> usize {
		self.calls
			.borrow()
			.iter()
			.filter(|call| call.url == url)
			.count()
	}

	/// Record a call (for internal use).
	pub fn record_call(&self, call: MockFetchCall) {
		self.calls.borrow_mut().push(call);
	}

	/// Get a mock response for a URL if registered.
	pub fn get_response(&self, url: &str) -> Option<MockFetchResponse> {
		self.responses.borrow().get(url).cloned()
	}
}

fn http_status_text(status: u16) -> &'static str {
	match status {
		200 => "OK",
		201 => "Created",
		204 => "No Content",
		400 => "Bad Request",
		401 => "Unauthorized",
		403 => "Forbidden",
		404 => "Not Found",
		405 => "Method Not Allowed",
		409 => "Conflict",
		422 => "Unprocessable Entity",
		429 => "Too Many Requests",
		500 => "Internal Server Error",
		502 => "Bad Gateway",
		503 => "Service Unavailable",
		_ => "Unknown",
	}
}

/// Mock timer utilities for controlling time in tests.
///
/// This allows fast-forwarding time without actually waiting.
#[derive(Debug, Default)]
pub struct MockTimers {
	callbacks: Rc<RefCell<Vec<TimerCallback>>>,
	current_time: Rc<RefCell<f64>>,
}

struct TimerCallback {
	id: u32,
	callback: Box<dyn FnOnce()>,
	scheduled_time: f64,
	is_interval: bool,
	interval_ms: Option<u32>,
}

impl std::fmt::Debug for TimerCallback {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TimerCallback")
			.field("id", &self.id)
			.field("callback", &"<FnOnce>")
			.field("scheduled_time", &self.scheduled_time)
			.field("is_interval", &self.is_interval)
			.field("interval_ms", &self.interval_ms)
			.finish()
	}
}

impl MockTimers {
	/// Create a new mock timers instance.
	pub fn new() -> Self {
		Self::default()
	}

	/// Get the current mock time.
	pub fn now(&self) -> f64 {
		*self.current_time.borrow()
	}

	/// Advance time by the specified milliseconds and run due callbacks.
	pub fn advance_by(&self, ms: u32) {
		let new_time = *self.current_time.borrow() + ms as f64;
		*self.current_time.borrow_mut() = new_time;
		self.run_due_callbacks();
	}

	/// Run all pending timers immediately.
	pub fn run_all(&self) {
		let max_time = self
			.callbacks
			.borrow()
			.iter()
			.map(|cb| cb.scheduled_time)
			.max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
			.unwrap_or(0.0);

		if max_time > *self.current_time.borrow() {
			*self.current_time.borrow_mut() = max_time;
			self.run_due_callbacks();
		}
	}

	/// Get the number of pending timers.
	pub fn pending_count(&self) -> usize {
		self.callbacks.borrow().len()
	}

	/// Clear all pending timers.
	pub fn clear_all(&self) {
		self.callbacks.borrow_mut().clear();
	}

	// Fixes #879
	fn run_due_callbacks(&self) {
		let current = *self.current_time.borrow();

		// Drain all callbacks from the list, then partition into due and remaining
		let all_callbacks: Vec<TimerCallback> = self.callbacks.borrow_mut().drain(..).collect();

		let mut remaining = Vec::new();
		let mut due = Vec::new();

		for cb in all_callbacks {
			if cb.scheduled_time <= current {
				due.push(cb);
			} else {
				remaining.push(cb);
			}
		}

		// Restore non-due callbacks
		*self.callbacks.borrow_mut() = remaining;

		// Execute due callbacks in scheduled order
		due.sort_by(|a, b| {
			a.scheduled_time
				.partial_cmp(&b.scheduled_time)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		for cb in due {
			(cb.callback)();
		}
	}
}

/// Test helper for tracking element mutations.
///
/// This uses MutationObserver to track DOM changes.
#[derive(Debug)]
pub struct MutationTracker {
	mutations: Rc<RefCell<Vec<MutationRecord>>>,
}

/// A recorded DOM mutation.
#[derive(Debug, Clone)]
pub struct MutationRecord {
	/// Type of mutation (childList, attributes, characterData).
	pub mutation_type: String,
	/// Target element tag name.
	pub target: String,
	/// Attribute name for attribute mutations.
	pub attribute_name: Option<String>,
	/// Old value if available.
	pub old_value: Option<String>,
	/// Number of added nodes.
	pub added_nodes_count: usize,
	/// Number of removed nodes.
	pub removed_nodes_count: usize,
}

impl MutationTracker {
	/// Create a new mutation tracker for the given element.
	///
	/// # Panics
	///
	/// Always panics because `MutationObserver` requires a live WASM runtime
	/// with access to the browser DOM API. This type cannot be used outside
	/// of an actual browser environment.
	// Fixes #879
	pub fn new(_element: &web_sys::Element) -> Self {
		unimplemented!(
			"MutationTracker requires a WASM runtime with browser DOM access. \
			 MutationObserver cannot be set up outside of an actual browser environment."
		)
	}

	/// Get all recorded mutations.
	pub fn mutations(&self) -> Vec<MutationRecord> {
		self.mutations.borrow().clone()
	}

	/// Clear recorded mutations.
	pub fn clear(&self) {
		self.mutations.borrow_mut().clear();
	}

	/// Check if any mutations occurred.
	pub fn has_mutations(&self) -> bool {
		!self.mutations.borrow().is_empty()
	}

	/// Get the number of mutations.
	pub fn mutation_count(&self) -> usize {
		self.mutations.borrow().len()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ==================== MockStorage tests ====================

	#[rstest]
	fn test_mock_storage_operations() {
		// Arrange
		let storage = MockStorage::new();

		// Act & Assert
		assert_eq!(storage.length(), 0);

		storage.set_item("key1", "value1");
		storage.set_item("key2", "value2");

		assert_eq!(storage.length(), 2);
		assert_eq!(storage.get_item("key1"), Some("value1".to_string()));
		assert_eq!(storage.get_item("key2"), Some("value2".to_string()));
		assert_eq!(storage.get_item("key3"), None);

		storage.remove_item("key1");
		assert_eq!(storage.length(), 1);
		assert_eq!(storage.get_item("key1"), None);

		storage.clear();
		assert_eq!(storage.length(), 0);
	}

	#[rstest]
	fn test_mock_storage_with_data() {
		// Arrange
		let mut data = HashMap::new();
		data.insert("a".to_string(), "1".to_string());
		data.insert("b".to_string(), "2".to_string());

		// Act
		let storage = MockStorage::with_data(data);

		// Assert
		assert_eq!(storage.length(), 2);
		assert_eq!(storage.get_item("a"), Some("1".to_string()));
		assert_eq!(storage.get_item("b"), Some("2".to_string()));
	}

	#[rstest]
	fn test_mock_storage_key_by_index() {
		// Arrange
		let storage = MockStorage::new();
		storage.set_item("alpha", "val");

		// Act
		let key_0 = storage.key(0);
		let key_out_of_bounds = storage.key(99);

		// Assert
		assert_eq!(key_0, Some("alpha".to_string()));
		assert_eq!(key_out_of_bounds, None);
	}

	#[rstest]
	fn test_mock_storage_keys_values_entries() {
		// Arrange
		let storage = MockStorage::new();
		storage.set_item("x", "10");
		storage.set_item("y", "20");

		// Act
		let keys = storage.keys();
		let values = storage.values();
		let entries = storage.entries();

		// Assert
		assert_eq!(keys.len(), 2);
		assert!(keys.contains(&"x".to_string()));
		assert!(keys.contains(&"y".to_string()));
		assert_eq!(values.len(), 2);
		assert!(values.contains(&"10".to_string()));
		assert!(values.contains(&"20".to_string()));
		assert_eq!(entries.len(), 2);
	}

	#[rstest]
	fn test_mock_storage_contains_key() {
		// Arrange
		let storage = MockStorage::new();
		storage.set_item("present", "yes");

		// Act & Assert
		assert!(storage.contains_key("present"));
		assert!(!storage.contains_key("absent"));
	}

	#[rstest]
	fn test_mock_storage_length_after_ops() {
		// Arrange
		let storage = MockStorage::new();

		// Act & Assert
		assert_eq!(storage.length(), 0);
		storage.set_item("a", "1");
		assert_eq!(storage.length(), 1);
		storage.set_item("b", "2");
		assert_eq!(storage.length(), 2);
		storage.remove_item("a");
		assert_eq!(storage.length(), 1);
		storage.clear();
		assert_eq!(storage.length(), 0);
	}

	#[rstest]
	fn test_mock_storage_overwrite() {
		// Arrange
		let storage = MockStorage::new();
		storage.set_item("key", "old_value");

		// Act
		storage.set_item("key", "new_value");

		// Assert
		assert_eq!(storage.length(), 1);
		assert_eq!(storage.get_item("key"), Some("new_value".to_string()));
	}

	#[rstest]
	fn test_mock_storage_remove_nonexistent() {
		// Arrange
		let storage = MockStorage::new();
		storage.set_item("key", "value");

		// Act
		storage.remove_item("nonexistent");

		// Assert
		assert_eq!(storage.length(), 1);
		assert_eq!(storage.get_item("key"), Some("value".to_string()));
	}

	#[rstest]
	fn test_mock_storage_empty_key_and_value() {
		// Arrange
		let storage = MockStorage::new();

		// Act
		storage.set_item("", "empty_key");
		storage.set_item("empty_value", "");

		// Assert
		assert_eq!(storage.get_item(""), Some("empty_key".to_string()));
		assert_eq!(storage.get_item("empty_value"), Some(String::new()));
		assert_eq!(storage.length(), 2);
	}

	#[rstest]
	fn test_mock_storage_clone_independence() {
		// Arrange
		let storage = MockStorage::new();
		storage.set_item("shared", "data");
		let cloned = storage.clone();

		// Act
		cloned.set_item("extra", "value");

		// Assert: Rc<RefCell> means clones share the same data
		assert_eq!(storage.length(), 2);
		assert_eq!(storage.get_item("extra"), Some("value".to_string()));
	}

	// ==================== MockCookies tests ====================

	#[rstest]
	fn test_mock_cookies_operations() {
		// Arrange
		let cookies = MockCookies::new();

		// Act & Assert
		assert!(cookies.is_empty());

		cookies.set("session", "abc123");
		cookies.set_with_options(
			"auth",
			"xyz",
			CookieOptions {
				secure: true,
				http_only: true,
				..Default::default()
			},
		);

		assert_eq!(cookies.len(), 2);
		assert_eq!(cookies.get("session"), Some("abc123".to_string()));
		assert_eq!(cookies.get("auth"), Some("xyz".to_string()));

		let entry = cookies.get_entry("auth").unwrap();
		assert!(entry.options.secure);
		assert!(entry.options.http_only);

		cookies.remove("session");
		assert!(!cookies.has("session"));
		assert!(cookies.has("auth"));
	}

	#[rstest]
	fn test_mock_cookies_with_cookies_constructor() {
		// Arrange
		let mut initial = HashMap::new();
		initial.insert("a".to_string(), "1".to_string());
		initial.insert("b".to_string(), "2".to_string());

		// Act
		let cookies = MockCookies::with_cookies(initial);

		// Assert
		assert_eq!(cookies.len(), 2);
		assert_eq!(cookies.get("a"), Some("1".to_string()));
		assert_eq!(cookies.get("b"), Some("2".to_string()));
	}

	#[rstest]
	fn test_mock_cookies_names_and_all() {
		// Arrange
		let cookies = MockCookies::new();
		cookies.set("x", "10");
		cookies.set("y", "20");

		// Act
		let names = cookies.names();
		let all = cookies.all();

		// Assert
		assert_eq!(names.len(), 2);
		assert!(names.contains(&"x".to_string()));
		assert!(names.contains(&"y".to_string()));
		assert_eq!(all.len(), 2);
		assert_eq!(all.get("x"), Some(&"10".to_string()));
		assert_eq!(all.get("y"), Some(&"20".to_string()));
	}

	#[rstest]
	fn test_mock_cookies_is_empty_transitions() {
		// Arrange
		let cookies = MockCookies::new();

		// Assert: initially empty
		assert!(cookies.is_empty());

		// Act: add a cookie
		cookies.set("k", "v");

		// Assert: no longer empty
		assert!(!cookies.is_empty());

		// Act: remove the cookie
		cookies.remove("k");

		// Assert: empty again
		assert!(cookies.is_empty());
	}

	#[rstest]
	fn test_mock_cookies_get_entry_with_options() {
		// Arrange
		let cookies = MockCookies::new();
		cookies.set_with_options(
			"tracking",
			"abc",
			CookieOptions {
				max_age: Some(3600),
				path: Some("/app".to_string()),
				domain: Some("example.com".to_string()),
				secure: true,
				http_only: false,
				same_site: Some(SameSite::Lax),
				expires: None,
			},
		);

		// Act
		let entry = cookies.get_entry("tracking").unwrap();

		// Assert
		assert_eq!(entry.value, "abc");
		assert_eq!(entry.options.max_age, Some(3600));
		assert_eq!(entry.options.path, Some("/app".to_string()));
		assert_eq!(entry.options.domain, Some("example.com".to_string()));
		assert!(entry.options.secure);
		assert!(!entry.options.http_only);
		assert_eq!(entry.options.same_site, Some(SameSite::Lax));
	}

	#[rstest]
	fn test_mock_cookies_get_entry_nonexistent() {
		// Arrange
		let cookies = MockCookies::new();

		// Act
		let entry = cookies.get_entry("missing");

		// Assert
		assert!(entry.is_none());
	}

	#[rstest]
	fn test_mock_cookies_to_cookie_string_format() {
		// Arrange
		let mut initial = HashMap::new();
		initial.insert("a".to_string(), "1".to_string());
		let cookies = MockCookies::with_cookies(initial);

		// Act
		let cookie_str = cookies.to_cookie_string();

		// Assert
		assert_eq!(cookie_str, "a=1");
	}

	#[rstest]
	fn test_mock_cookies_to_cookie_string_empty() {
		// Arrange
		let cookies = MockCookies::new();

		// Act
		let cookie_str = cookies.to_cookie_string();

		// Assert
		assert_eq!(cookie_str, "");
	}

	#[rstest]
	fn test_mock_cookies_to_cookie_string_multiple() {
		// Arrange
		let cookies = MockCookies::new();
		cookies.set("a", "1");
		cookies.set("b", "2");

		// Act
		let cookie_str = cookies.to_cookie_string();

		// Assert: order is not guaranteed, but format should be "key=value; key=value"
		assert!(cookie_str.contains("a=1"));
		assert!(cookie_str.contains("b=2"));
		assert!(cookie_str.contains("; "));
	}

	#[rstest]
	fn test_cookie_options_default() {
		// Arrange & Act
		let opts = CookieOptions::default();

		// Assert
		assert_eq!(opts.max_age, None);
		assert_eq!(opts.expires, None);
		assert_eq!(opts.path, None);
		assert_eq!(opts.domain, None);
		assert!(!opts.secure);
		assert!(!opts.http_only);
		assert!(opts.same_site.is_none());
	}

	#[rstest]
	fn test_same_site_variants() {
		// Arrange & Act & Assert
		assert_eq!(SameSite::Strict, SameSite::Strict);
		assert_eq!(SameSite::Lax, SameSite::Lax);
		assert_eq!(SameSite::None, SameSite::None);
		assert_ne!(SameSite::Strict, SameSite::Lax);
		assert_ne!(SameSite::Lax, SameSite::None);
		assert_ne!(SameSite::Strict, SameSite::None);
	}

	#[rstest]
	fn test_mock_cookies_clear() {
		// Arrange
		let cookies = MockCookies::new();
		cookies.set("a", "1");
		cookies.set("b", "2");
		assert_eq!(cookies.len(), 2);

		// Act
		cookies.clear();

		// Assert
		assert_eq!(cookies.len(), 0);
		assert!(cookies.is_empty());
	}

	// ==================== MockFetch tests ====================

	#[rstest]
	fn test_mock_fetch_response_builders() {
		// Arrange & Act
		let json_response = MockFetchResponse::json(serde_json::json!({"status": "ok"}));

		// Assert
		assert_eq!(json_response.status, 200);
		assert!(json_response.body.contains("status"));

		// Arrange & Act
		let text_response = MockFetchResponse::text("Hello, World!");

		// Assert
		assert_eq!(text_response.body, "Hello, World!");

		// Arrange & Act
		let error_response = MockFetchResponse::error(404, "Not found");

		// Assert
		assert_eq!(error_response.status, 404);
		assert_eq!(error_response.status_text, "Not Found");

		// Arrange & Act
		let network_error = MockFetchResponse::network_error();

		// Assert
		assert!(network_error.network_error);
	}

	#[rstest]
	fn test_mock_fetch_recording() {
		// Arrange
		let fetch = MockFetch::new();

		// Act
		fetch.record_call(MockFetchCall {
			url: "/api/users".to_string(),
			method: "GET".to_string(),
			headers: HashMap::new(),
			body: None,
		});

		fetch.record_call(MockFetchCall {
			url: "/api/posts".to_string(),
			method: "POST".to_string(),
			headers: HashMap::new(),
			body: Some(r#"{"title":"Test"}"#.to_string()),
		});

		// Assert
		assert!(fetch.was_called("/api/users"));
		assert!(fetch.was_called("/api/posts"));
		assert!(!fetch.was_called("/api/comments"));

		assert_eq!(fetch.call_count("/api/users"), 1);
		assert_eq!(fetch.calls().len(), 2);
	}

	#[rstest]
	fn test_mock_fetch_get_response_matching() {
		// Arrange
		let fetch = MockFetch::new();
		let response = MockFetchResponse::json(serde_json::json!({"id": 1}));
		fetch.mock_response("/api/data", response);

		// Act
		let result = fetch.get_response("/api/data");

		// Assert
		assert!(result.is_some());
		let resp = result.unwrap();
		assert_eq!(resp.status, 200);
		assert!(resp.body.contains("id"));
	}

	#[rstest]
	fn test_mock_fetch_get_response_unregistered_returns_none() {
		// Arrange
		let fetch = MockFetch::new();

		// Act
		let result = fetch.get_response("/unknown");

		// Assert
		assert!(result.is_none());
	}

	#[rstest]
	fn test_mock_fetch_remove_mock() {
		// Arrange
		let fetch = MockFetch::new();
		fetch.mock_response("/api/x", MockFetchResponse::text("hello"));
		assert!(fetch.get_response("/api/x").is_some());

		// Act
		fetch.remove_mock("/api/x");

		// Assert
		assert!(fetch.get_response("/api/x").is_none());
	}

	#[rstest]
	fn test_mock_fetch_clear_mocks() {
		// Arrange
		let fetch = MockFetch::new();
		fetch.mock_response("/a", MockFetchResponse::text("1"));
		fetch.mock_response("/b", MockFetchResponse::text("2"));

		// Act
		fetch.clear_mocks();

		// Assert
		assert!(fetch.get_response("/a").is_none());
		assert!(fetch.get_response("/b").is_none());
	}

	#[rstest]
	fn test_mock_fetch_calls_to_filtering() {
		// Arrange
		let fetch = MockFetch::new();
		fetch.record_call(MockFetchCall {
			url: "/api/a".to_string(),
			method: "GET".to_string(),
			headers: HashMap::new(),
			body: None,
		});
		fetch.record_call(MockFetchCall {
			url: "/api/b".to_string(),
			method: "GET".to_string(),
			headers: HashMap::new(),
			body: None,
		});
		fetch.record_call(MockFetchCall {
			url: "/api/a".to_string(),
			method: "POST".to_string(),
			headers: HashMap::new(),
			body: Some("data".to_string()),
		});

		// Act
		let calls_a = fetch.calls_to("/api/a");
		let calls_b = fetch.calls_to("/api/b");
		let calls_c = fetch.calls_to("/api/c");

		// Assert
		assert_eq!(calls_a.len(), 2);
		assert_eq!(calls_b.len(), 1);
		assert_eq!(calls_c.len(), 0);
	}

	#[rstest]
	fn test_mock_fetch_clear_calls() {
		// Arrange
		let fetch = MockFetch::new();
		fetch.record_call(MockFetchCall {
			url: "/api/x".to_string(),
			method: "GET".to_string(),
			headers: HashMap::new(),
			body: None,
		});
		assert_eq!(fetch.calls().len(), 1);

		// Act
		fetch.clear_calls();

		// Assert
		assert_eq!(fetch.calls().len(), 0);
		assert!(!fetch.was_called("/api/x"));
	}

	#[rstest]
	fn test_mock_fetch_call_count_multiple() {
		// Arrange
		let fetch = MockFetch::new();
		for _ in 0..5 {
			fetch.record_call(MockFetchCall {
				url: "/api/repeat".to_string(),
				method: "GET".to_string(),
				headers: HashMap::new(),
				body: None,
			});
		}
		fetch.record_call(MockFetchCall {
			url: "/api/other".to_string(),
			method: "GET".to_string(),
			headers: HashMap::new(),
			body: None,
		});

		// Act & Assert
		assert_eq!(fetch.call_count("/api/repeat"), 5);
		assert_eq!(fetch.call_count("/api/other"), 1);
		assert_eq!(fetch.call_count("/api/none"), 0);
	}

	// ==================== MockFetchResponse tests ====================

	#[rstest]
	fn test_mock_fetch_response_json_content_type() {
		// Arrange & Act
		let resp = MockFetchResponse::json(serde_json::json!([1, 2, 3]));

		// Assert
		assert_eq!(
			resp.headers.get("content-type"),
			Some(&"application/json".to_string())
		);
		assert_eq!(resp.status, 200);
		assert_eq!(resp.status_text, "OK");
		assert!(!resp.network_error);
	}

	#[rstest]
	fn test_mock_fetch_response_text_content_type() {
		// Arrange & Act
		let resp = MockFetchResponse::text("plain text");

		// Assert
		assert_eq!(
			resp.headers.get("content-type"),
			Some(&"text/plain".to_string())
		);
		assert_eq!(resp.body, "plain text");
		assert_eq!(resp.status, 200);
		assert!(!resp.network_error);
	}

	#[rstest]
	fn test_mock_fetch_response_with_status() {
		// Arrange
		let resp = MockFetchResponse::text("created");

		// Act
		let resp = resp.with_status(201);

		// Assert
		assert_eq!(resp.status, 201);
		assert_eq!(resp.status_text, "Created");
	}

	#[rstest]
	fn test_mock_fetch_response_with_header() {
		// Arrange
		let resp = MockFetchResponse::text("body");

		// Act
		let resp = resp
			.with_header("X-Custom", "value1")
			.with_header("X-Another", "value2");

		// Assert
		assert_eq!(resp.headers.get("X-Custom"), Some(&"value1".to_string()));
		assert_eq!(resp.headers.get("X-Another"), Some(&"value2".to_string()));
		// Original content-type header should still be present
		assert_eq!(
			resp.headers.get("content-type"),
			Some(&"text/plain".to_string())
		);
	}

	#[rstest]
	fn test_mock_fetch_response_error_various_statuses() {
		// Arrange & Act & Assert
		let resp_400 = MockFetchResponse::error(400, "bad");
		assert_eq!(resp_400.status, 400);
		assert_eq!(resp_400.status_text, "Bad Request");

		let resp_500 = MockFetchResponse::error(500, "fail");
		assert_eq!(resp_500.status, 500);
		assert_eq!(resp_500.status_text, "Internal Server Error");

		let resp_unknown = MockFetchResponse::error(999, "mystery");
		assert_eq!(resp_unknown.status, 999);
		assert_eq!(resp_unknown.status_text, "Unknown");
	}

	#[rstest]
	fn test_mock_fetch_response_network_error_fields() {
		// Arrange & Act
		let resp = MockFetchResponse::network_error();

		// Assert
		assert!(resp.network_error);
		assert_eq!(resp.status, 0);
		assert_eq!(resp.status_text, "");
		assert!(resp.headers.is_empty());
		assert_eq!(resp.body, "");
	}

	// ==================== MockTimers tests ====================

	#[rstest]
	fn test_mock_timers_initial_state() {
		// Arrange & Act
		let timers = MockTimers::new();

		// Assert
		assert_eq!(timers.now(), 0.0);
		assert_eq!(timers.pending_count(), 0);
	}

	#[rstest]
	fn test_mock_timers_advance_by() {
		// Arrange
		let timers = MockTimers::new();

		// Act
		timers.advance_by(100);

		// Assert
		assert_eq!(timers.now(), 100.0);

		// Act
		timers.advance_by(50);

		// Assert
		assert_eq!(timers.now(), 150.0);
	}

	#[rstest]
	fn test_mock_timers_clear_all() {
		// Arrange
		let timers = MockTimers::new();

		// Act
		timers.clear_all();

		// Assert
		assert_eq!(timers.pending_count(), 0);
	}
}
