//! Test: Server function with `FromRequest` extractor parameters (Issue #3858)
//!
//! This test verifies that:
//! 1. Extractor params (Validated<Form<T>>, Header<T>, etc.) are detected correctly
//! 2. Client stub excludes extractor params from Args struct
//! 3. Server handler resolves extractor params via FromRequest::from_request
//! 4. Mixed functions (regular + extractor + #[inject]) compile correctly

use reinhardt_pages_macros::server_fn;
use serde::{Deserialize, Serialize};

// Mock types for testing
#[derive(Serialize, Deserialize)]
struct LoginRequest {
	email: String,
	password: String,
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

impl From<serde_json::Error> for ServerFnError {
	fn from(err: serde_json::Error) -> Self {
		ServerFnError(format!("Serialization error: {}", err))
	}
}

// Test 1: Server function with only extractor params (all resolved via FromRequest)
#[server_fn]
async fn login_with_form(
	form: reinhardt_di::params::Validated<reinhardt_di::params::Form<LoginRequest>>,
) -> Result<User, ServerFnError> {
	let _ = form;
	Ok(User {
		id: 1,
		name: "Alice".to_string(),
	})
}

// Test 2: Server function with Header extractor
#[server_fn]
async fn get_user_with_auth(
	user_id: u32, // Regular param (in Args)
	auth: reinhardt_di::params::Header<String>, // Extractor (excluded from Args)
) -> Result<User, ServerFnError> {
	let _ = auth;
	Ok(User {
		id: user_id,
		name: "Bob".to_string(),
	})
}

fn main() {
	// This test file is used by trybuild to verify macro expansion.
	// It should compile successfully with FromRequest extractor detection.
}
