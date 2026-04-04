//! Test: Server function with JSON codec (Week 4 Day 3-4)
//!
//! This test verifies that:
//! 1. Server functions can explicitly specify JSON codec
//! 2. JSON codec is the default when no codec is specified
//! 3. Both syntaxes compile successfully

use reinhardt_pages_macros::server_fn;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct User {
	id: u32,
	name: String,
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

// Test: Explicit JSON codec specification
#[server_fn(codec = "json")]
async fn get_user_explicit_json(id: u32) -> Result<User, ServerFnError> {
	Ok(User {
		id,
		name: format!("User {}", id),
	})
}

// Test: Default codec (should be JSON)
#[server_fn]
async fn get_user_default_codec(id: u32) -> Result<User, ServerFnError> {
	Ok(User {
		id,
		name: format!("User {}", id),
	})
}

// Test: JSON codec with complex data structures
#[server_fn(codec = "json")]
async fn create_user_json(
	name: String,
	_email: String,
	_settings: Vec<String>,
) -> Result<User, ServerFnError> {
	Ok(User { id: 1, name })
}

fn main() {
	// This test file is used by trybuild to verify macro expansion
	// It should compile successfully with JSON codec specification
}
