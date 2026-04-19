//! Test: Server function compiles without msw feature (Fixes #3666)
//!
//! This test verifies that:
//! 1. Server functions compile successfully when the `msw` feature is NOT enabled
//! 2. No `unexpected cfg condition value: "msw"` errors from check-cfg
//! 3. The macro does not emit `#[cfg(feature = "msw")]` into the destination crate

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

impl From<serde_json::Error> for ServerFnError {
	fn from(err: serde_json::Error) -> Self {
		ServerFnError(format!("Serialization error: {}", err))
	}
}

// Test: Basic server_fn without msw feature should not trigger check-cfg errors
#[server_fn]
async fn get_user(id: u32) -> Result<User, ServerFnError> {
	Ok(User {
		id,
		name: format!("User {}", id),
	})
}

// Test: server_fn with inject without msw feature
#[server_fn(use_inject = true)]
async fn get_user_with_inject(id: u32) -> Result<User, ServerFnError> {
	Ok(User {
		id,
		name: format!("User {}", id),
	})
}

fn main() {
	// This test file is used by trybuild to verify that #[server_fn] compiles
	// without the `msw` feature enabled, ensuring no check-cfg errors (Fixes #3666).
}
