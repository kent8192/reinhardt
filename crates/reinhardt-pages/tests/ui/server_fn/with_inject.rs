//! Test: Server function with `#[inject]` parameters (Week 4 Day 1-2)
//!
//! This test verifies that:
//! 1. `#[inject]` parameters are detected correctly
//! 2. Client stub excludes `#[inject]` parameters from Args struct
//! 3. Server handler includes DI resolution code (placeholder)

use reinhardt_pages_macros::server_fn;
use serde::{Deserialize, Serialize};

// Mock types for testing
#[derive(Clone)]
struct Database {
	connection_string: String,
}

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

// Test: Basic server function with one #[inject] parameter
#[server_fn]
async fn get_user(
	id: u32,                 // Regular parameter (included in client Args)
	#[inject] _db: Database, // DI parameter (excluded from client Args)
) -> Result<User, ServerFnError> {
	Ok(User {
		id,
		name: format!("User {}", id),
	})
}

// Test: Server function with multiple #[inject] parameters
#[server_fn]
async fn create_user(
	name: String,             // Regular parameter
	_email: String,           // Regular parameter
	#[inject] _db: Database,  // DI parameter 1
	#[inject] _db2: Database, // DI parameter 2
) -> Result<User, ServerFnError> {
	Ok(User { id: 1, name })
}

// Test: Server function with no #[inject] parameters (no actual injections)
#[server_fn]
async fn simple_function(value: u32) -> Result<u32, ServerFnError> {
	Ok(value * 2)
}

fn main() {
	// This test file is used by trybuild to verify macro expansion
	// It should compile successfully with DI parameter detection
}
