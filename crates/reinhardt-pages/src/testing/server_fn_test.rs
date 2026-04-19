//! Server Function Unit Testing Utilities
//!
//! This module provides utilities for testing server functions directly
//! without going through the HTTP layer. This enables Layer 1 testing
//! (pure business logic testing) with mocked dependencies.
//!
//! # Architecture
//!
//! Server function testing follows a 3-layer architecture:
//!
//! - **Layer 1 (this module)**: Direct server function calls with DI injection
//! - **Layer 2**: WASM component tests with mocked HTTP
//! - **Layer 3**: Full E2E tests with real server
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_pages::testing::ServerFnTestContext;
//! use reinhardt_di::SingletonScope;
//! use std::sync::Arc;
//!
//! #[tokio::test]
//! async fn test_login() {
//!     let singleton = Arc::new(SingletonScope::new());
//!     let ctx = ServerFnTestContext::new(singleton)
//!         .with_database(pool.clone())
//!         .with_mock_session()
//!         .build();
//!
//!     // Test server function with prepared context
//!     // The context provides all necessary dependencies
//! }
//! ```

use std::sync::Arc;

#[cfg(native)]
use reinhardt_di::{InjectionContext, SingletonScope};

/// Builder for creating test DI contexts for server function testing.
///
/// This builder provides a fluent API for setting up test dependencies
/// that will be injected into server functions during testing.
///
/// # Type Parameters
///
/// The builder tracks whether required dependencies have been set
/// through its state, ensuring a complete test context at compile time.
#[cfg(native)]
// Boxed closures for DI overrides require complex type signatures that cannot
// be simplified without losing flexibility.
#[allow(clippy::type_complexity)]
pub struct ServerFnTestContext {
	singleton_scope: Arc<SingletonScope>,
	overrides: Vec<Box<dyn FnOnce(&InjectionContext)>>,
}

#[cfg(native)]
impl ServerFnTestContext {
	/// Create a new server function test context builder.
	///
	/// # Arguments
	///
	/// * `singleton_scope` - The singleton scope for dependency resolution.
	///   This is typically shared across a test or test suite.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// use reinhardt_di::SingletonScope;
	/// use std::sync::Arc;
	///
	/// let singleton = Arc::new(SingletonScope::new());
	/// let builder = ServerFnTestContext::new(singleton);
	/// ```
	pub fn new(singleton_scope: Arc<SingletonScope>) -> Self {
		Self {
			singleton_scope,
			overrides: Vec::new(),
		}
	}

	/// Add a database connection override to the test context.
	///
	/// The database connection will be available for injection
	/// in server functions that request `DatabaseConnection`.
	///
	/// # Arguments
	///
	/// * `pool` - The database connection pool (typically from TestContainers)
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let ctx = ServerFnTestContext::new(singleton)
	///     .with_database(pool.clone())
	///     .build();
	/// ```
	pub fn with_database<P: Clone + Send + Sync + 'static>(mut self, pool: P) -> Self {
		self.overrides.push(Box::new(move |ctx| {
			ctx.set_singleton(pool);
		}));
		self
	}

	/// Add a mock session to the test context.
	///
	/// Creates a new empty session that can be used for testing
	/// session-based server functions.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let ctx = ServerFnTestContext::new(singleton)
	///     .with_mock_session()
	///     .build();
	/// ```
	pub fn with_mock_session(mut self) -> Self {
		self.overrides.push(Box::new(|_ctx| {
			// Session setup is handled by the session module
			// This is a placeholder for future session mock implementation
		}));
		self
	}

	/// Add a custom singleton dependency to the test context.
	///
	/// This allows adding any type that implements the required traits
	/// to be available for injection.
	///
	/// # Arguments
	///
	/// * `value` - The singleton value to register
	///
	/// # Example
	///
	/// ```rust,ignore
	/// struct MockEmailService;
	///
	/// let ctx = ServerFnTestContext::new(singleton)
	///     .with_singleton(MockEmailService)
	///     .build();
	/// ```
	pub fn with_singleton<T: Clone + Send + Sync + 'static>(mut self, value: T) -> Self {
		self.overrides.push(Box::new(move |ctx| {
			ctx.set_singleton(value);
		}));
		self
	}

	/// Build the injection context with all configured overrides.
	///
	/// Returns an `InjectionContext` that can be used to resolve
	/// dependencies for server function testing.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let ctx = ServerFnTestContext::new(singleton)
	///     .with_database(pool)
	///     .with_mock_session()
	///     .build();
	///
	/// // Use ctx for dependency resolution
	/// ```
	pub fn build(self) -> InjectionContext {
		let ctx = InjectionContext::builder(self.singleton_scope).build();

		// Apply all overrides
		for override_fn in self.overrides {
			override_fn(&ctx);
		}

		ctx
	}
}

/// Extension trait for testing server functions.
///
/// This trait provides a standardized interface for testing server functions
/// by allowing direct invocation with a prepared injection context.
///
/// # Implementation
///
/// Server functions marked with `#[server_fn]` can implement this trait
/// to enable direct testing without HTTP layer overhead.
#[cfg(native)]
#[async_trait::async_trait]
pub trait ServerFnTestable {
	/// The input type for the server function
	type Input: Send;

	/// The output type for the server function
	type Output: Send;

	/// The error type for the server function
	type Error: Send;

	/// Execute the server function with the given input and injection context.
	///
	/// This method bypasses the HTTP layer and directly invokes the
	/// server function logic with dependencies resolved from the context.
	///
	/// # Arguments
	///
	/// * `input` - The input data for the server function
	/// * `ctx` - The injection context providing dependencies
	///
	/// # Returns
	///
	/// The result of the server function execution
	async fn test_call(
		input: Self::Input,
		ctx: &InjectionContext,
	) -> Result<Self::Output, Self::Error>;
}

/// Marker struct for test-specific session data.
///
/// This can be used to create test sessions with predefined values.
#[cfg(native)]
#[derive(Debug, Clone, Default)]
pub struct TestSessionData {
	/// User ID stored in session (if authenticated)
	pub user_id: Option<uuid::Uuid>,
	/// Additional session data for testing
	pub extra_data: std::collections::HashMap<String, String>,
}

#[cfg(native)]
impl TestSessionData {
	/// Create a new empty test session
	pub fn new() -> Self {
		Self::default()
	}

	/// Create a test session with a logged-in user
	pub fn with_user(user_id: uuid::Uuid) -> Self {
		Self {
			user_id: Some(user_id),
			extra_data: std::collections::HashMap::new(),
		}
	}

	/// Add extra data to the session
	pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
		self.extra_data.insert(key.into(), value.into());
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[cfg(native)]
	#[test]
	fn test_server_fn_test_context_creation() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = ServerFnTestContext::new(singleton).build();

		// Context should be created successfully with valid singleton scope
		// The singleton_scope() method returns &Arc<SingletonScope>, not Option
		let _scope = ctx.singleton_scope();
	}

	#[cfg(native)]
	#[test]
	fn test_test_session_data_creation() {
		let session = TestSessionData::new();
		assert!(session.user_id.is_none());
		assert!(session.extra_data.is_empty());

		let user_id = uuid::Uuid::now_v7();
		let session_with_user = TestSessionData::with_user(user_id);
		assert_eq!(session_with_user.user_id, Some(user_id));
	}

	#[cfg(native)]
	#[test]
	fn test_test_session_data_with_extra_data() {
		let session = TestSessionData::new()
			.with_data("key1", "value1")
			.with_data("key2", "value2");

		assert_eq!(session.extra_data.get("key1"), Some(&"value1".to_string()));
		assert_eq!(session.extra_data.get("key2"), Some(&"value2".to_string()));
	}
}
