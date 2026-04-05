//! Test: Server function with URL codec (Week 4 Day 3-4)
//!
//! This test verifies that:
//! 1. Server functions can use URL encoding codec for GET requests
//! 2. URL codec is suitable for simple query parameters
//! 3. URL-encoded data compiles successfully

use reinhardt_pages_macros::server_fn;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct SearchResult {
	title: String,
	url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerFnError(String);

impl std::fmt::Display for ServerFnError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl std::error::Error for ServerFnError {}

// Required for client-side error conversion (WASM only)
#[cfg(wasm)]
impl From<reqwest::Error> for ServerFnError {
	fn from(err: reqwest::Error) -> Self {
		ServerFnError(format!("Network error: {:?}", err))
	}
}

impl From<serde_json::Error> for ServerFnError {
	fn from(err: serde_json::Error) -> Self {
		ServerFnError(format!("Serialization error: {}", err))
	}
}

impl From<serde_urlencoded::ser::Error> for ServerFnError {
	fn from(err: serde_urlencoded::ser::Error) -> Self {
		ServerFnError(format!("URL encoding error: {}", err))
	}
}

impl From<serde_urlencoded::de::Error> for ServerFnError {
	fn from(err: serde_urlencoded::de::Error) -> Self {
		ServerFnError(format!("URL decoding error: {}", err))
	}
}

// Test: URL codec for simple GET request parameters
#[server_fn(codec = "url")]
async fn search(query: String, page: u32) -> Result<Vec<SearchResult>, ServerFnError> {
	Ok(vec![SearchResult {
		title: format!("Result for: {}", query),
		url: format!("https://example.com/search?q={}&page={}", query, page),
	}])
}

// Test: URL codec with multiple simple parameters
#[server_fn(codec = "url")]
async fn filter_items(
	category: String,
	_min_price: u32,
	_max_price: u32,
	_sort_by: String,
) -> Result<Vec<String>, ServerFnError> {
	Ok(vec![
		format!("Item 1 in {}", category),
		format!("Item 2 in {}", category),
	])
}

// Test: URL codec with boolean and string parameters
#[server_fn(codec = "url")]
async fn get_settings(user_id: u32, include_private: bool) -> Result<String, ServerFnError> {
	Ok(format!(
		"Settings for user {} (private: {})",
		user_id, include_private
	))
}

fn main() {
	// This test file is used by trybuild to verify macro expansion
	// It should compile successfully with URL codec specification
}
