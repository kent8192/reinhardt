//! Test: Server function with `#[inject]` parameters (Week 4 Day 1-2)
//!
//! This test verifies that:
//! 1. `#[inject]` parameters are detected correctly
//! 2. Client stub excludes `#[inject]` parameters from Args struct
//! 3. Server handler includes DI resolution code (placeholder)

use reinhardt_pages_macros::server_fn;
use serde::{Deserialize, Serialize};
use std::future::Future;

// Mock types for testing
#[derive(Clone, Deserialize)]
struct Database {
	connection_string: String,
}

#[derive(Clone)]
struct Wrapper<T>(T);

#[async_trait::async_trait]
impl reinhardt_di::Injectable for Database {
	async fn inject(_ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(Self {
			connection_string: String::new(),
		})
	}
}

#[async_trait::async_trait]
impl reinhardt_di::Injectable for Wrapper<Database> {
	async fn inject(_ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(Self(Database {
			connection_string: String::new(),
		}))
	}
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

// Required for client-side error conversion in generated stubs.
impl From<reinhardt_pages::server_fn::ServerFnError> for ServerFnError {
	fn from(err: reinhardt_pages::server_fn::ServerFnError) -> Self {
		ServerFnError(format!("Client error: {}", err))
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

#[server_fn(use_inject = true)]
async fn update_database(#[inject] mut db: Database) -> Result<(), ServerFnError> {
	db.connection_string.push_str("?write=true");
	Ok(())
}

#[server_fn]
async fn preserve_inject_across_extractor_collision(
	#[inject] db: Database,
	__server_fn_inject_0: reinhardt_di::params::Json<Database>,
) -> Result<String, ServerFnError> {
	let _ = __server_fn_inject_0;
	Ok(db.connection_string)
}

#[server_fn]
async fn update_wrapped(
	#[inject] Wrapper(mut value): Wrapper<Database>,
) -> Result<(), ServerFnError> {
	value.connection_string.push_str("?write=true");
	Ok(())
}

fn assert_one<F, Fut>(_: F)
where
	F: Fn(u32) -> Fut,
	Fut: Future<Output = Result<User, ServerFnError>>,
{
}

fn assert_tuple<F, Fut>(_: F)
where
	F: Fn((String, String)) -> Fut,
	Fut: Future<Output = Result<User, ServerFnError>>,
{
}

fn assert_zero<F, Fut>(_: F)
where
	F: Fn(()) -> Fut,
	Fut: Future<Output = Result<(), ServerFnError>>,
{
}

fn main() {
	// This test file is used by trybuild to verify macro expansion
	// It should compile successfully with DI parameter detection
	assert_one(get_user::mutation());
	assert_tuple(create_user::mutation());
	assert_zero(update_database::mutation());
}
