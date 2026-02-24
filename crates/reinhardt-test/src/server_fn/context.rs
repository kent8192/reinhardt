//! Enhanced Server Function Test Context.
//!
//! This module provides an enhanced version of `ServerFnTestContext` with
//! additional features for authentication mocking, HTTP request/response
//! simulation, and transaction management.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::server_fn::{ServerFnTestContext, TestUser};
//! use reinhardt_di::SingletonScope;
//! use std::sync::Arc;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_protected_endpoint(singleton_scope: Arc<SingletonScope>) {
//!     let ctx = ServerFnTestContext::new(singleton_scope)
//!         .with_authenticated_user(TestUser::admin())
//!         .with_transaction_rollback()
//!         .build();
//!
//!     let result = my_server_fn::test_call(input, &ctx).await;
//!     assert!(result.is_ok());
//! }
//! ```

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use std::sync::Arc;

use http::{HeaderMap, HeaderValue, StatusCode};
use reinhardt_di::{InjectionContext, SingletonScope};
use uuid::Uuid;

use super::auth::{MockSession, TestUser};
use super::mock_request::MockHttpRequest;

/// Transaction mode for test database operations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransactionMode {
	/// Automatically rollback after each test (recommended).
	#[default]
	Rollback,
	/// Allow commits (use with caution).
	Commit,
	/// No transaction management.
	None,
}

/// Enhanced test context builder for server function testing.
///
/// This builder extends the basic `ServerFnTestContext` with:
/// - Authentication/authorization mocking
/// - HTTP request/response simulation
/// - Transaction management
/// - CSRF token handling
///
/// # Example
///
/// ```rust,ignore
/// let ctx = ServerFnTestContext::new(singleton_scope)
///     .with_authenticated_user(TestUser::authenticated("alice"))
///     .with_permissions(vec!["read", "write"])
///     .with_csrf_token("test-token")
///     .build();
/// ```
// Boxed closures for DI overrides require complex type signatures that cannot
// be simplified without losing flexibility.
#[allow(clippy::type_complexity)]
pub struct ServerFnTestContext {
	singleton_scope: Arc<SingletonScope>,
	overrides: Vec<Box<dyn FnOnce(&InjectionContext) + Send>>,
	mock_request: Option<MockHttpRequest>,
	mock_session: Option<MockSession>,
	test_user: Option<TestUser>,
	transaction_mode: TransactionMode,
	request_headers: HeaderMap,
	csrf_token: Option<String>,
}

impl ServerFnTestContext {
	/// Create a new server function test context builder.
	///
	/// # Arguments
	///
	/// * `singleton_scope` - The singleton scope for dependency resolution.
	pub fn new(singleton_scope: Arc<SingletonScope>) -> Self {
		Self {
			singleton_scope,
			overrides: Vec::new(),
			mock_request: None,
			mock_session: None,
			test_user: None,
			transaction_mode: TransactionMode::default(),
			request_headers: HeaderMap::new(),
			csrf_token: None,
		}
	}

	/// Add a database connection override to the test context.
	///
	/// # Arguments
	///
	/// * `pool` - The database connection pool (typically from TestContainers)
	pub fn with_database<P: Clone + Send + Sync + 'static>(mut self, pool: P) -> Self {
		self.overrides.push(Box::new(move |ctx| {
			ctx.set_singleton(pool);
		}));
		self
	}

	/// Add a custom singleton dependency to the test context.
	///
	/// # Arguments
	///
	/// * `value` - The singleton value to register
	pub fn with_singleton<T: Clone + Send + Sync + 'static>(mut self, value: T) -> Self {
		self.overrides.push(Box::new(move |ctx| {
			ctx.set_singleton(value);
		}));
		self
	}

	/// Set the authenticated user for this test.
	///
	/// This configures the test context to simulate an authenticated user,
	/// allowing you to test protected endpoints.
	///
	/// # Arguments
	///
	/// * `user` - The test user to authenticate as
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let ctx = ServerFnTestContext::new(singleton)
	///     .with_authenticated_user(TestUser::admin())
	///     .build();
	/// ```
	pub fn with_authenticated_user(mut self, user: TestUser) -> Self {
		self.test_user = Some(user.clone());
		self.mock_session = Some(MockSession::authenticated(user));
		self
	}

	/// Set permissions for the authenticated user.
	///
	/// This is a convenience method that modifies the test user's permissions.
	///
	/// # Arguments
	///
	/// * `permissions` - List of permission strings to grant
	// Fixes #868
	pub fn with_permissions<S: Into<String>>(mut self, permissions: Vec<S>) -> Self {
		if let Some(ref mut user) = self.test_user {
			for perm in permissions {
				user.permissions.push(perm.into());
			}
			// Synchronize mock_session user with updated test_user
			if let Some(ref mut session) = self.mock_session {
				session.user = Some(user.clone());
			}
		} else {
			let mut user = TestUser::authenticated("test-user");
			for perm in permissions {
				user.permissions.push(perm.into());
			}
			self.test_user = Some(user.clone());
			self.mock_session = Some(MockSession::authenticated(user));
		}
		self
	}

	/// Set roles for the authenticated user.
	///
	/// # Arguments
	///
	/// * `roles` - List of role strings to assign
	// Fixes #868
	pub fn with_roles<S: Into<String>>(mut self, roles: Vec<S>) -> Self {
		if let Some(ref mut user) = self.test_user {
			for role in roles {
				user.roles.push(role.into());
			}
			// Synchronize mock_session user with updated test_user
			if let Some(ref mut session) = self.mock_session {
				session.user = Some(user.clone());
			}
		} else {
			let mut user = TestUser::authenticated("test-user");
			for role in roles {
				user.roles.push(role.into());
			}
			self.test_user = Some(user.clone());
			self.mock_session = Some(MockSession::authenticated(user));
		}
		self
	}

	/// Set a mock HTTP request for the context.
	///
	/// This is useful for testing server functions that access request data
	/// like headers, cookies, or body.
	///
	/// # Arguments
	///
	/// * `request` - The mock HTTP request
	pub fn with_request(mut self, request: MockHttpRequest) -> Self {
		self.mock_request = Some(request);
		self
	}

	/// Add request headers to the context.
	///
	/// # Arguments
	///
	/// * `headers` - Headers to add
	pub fn with_request_headers(mut self, headers: HeaderMap) -> Self {
		self.request_headers = headers;
		self
	}

	/// Add a single request header.
	///
	/// # Arguments
	///
	/// * `name` - Header name
	/// * `value` - Header value
	pub fn with_header(mut self, name: &str, value: &str) -> Self {
		if let Ok(header_value) = HeaderValue::from_str(value)
			&& let Ok(header_name) = http::header::HeaderName::from_bytes(name.as_bytes())
		{
			self.request_headers.insert(header_name, header_value);
		}
		self
	}

	/// Set a CSRF token for the request.
	///
	/// This automatically adds the token to both headers and session.
	///
	/// # Arguments
	///
	/// * `token` - The CSRF token string
	pub fn with_csrf_token(mut self, token: &str) -> Self {
		self.csrf_token = Some(token.to_string());

		// Add to headers
		if let Ok(header_value) = HeaderValue::from_str(token) {
			self.request_headers
				.insert("x-csrf-token", header_value.clone());
		}

		// Add to session if present
		if let Some(ref mut session) = self.mock_session {
			session.csrf_token = token.to_string();
		}

		self
	}

	/// Set the transaction mode for database operations.
	///
	/// # Arguments
	///
	/// * `mode` - The transaction mode
	pub fn with_transaction_mode(mut self, mode: TransactionMode) -> Self {
		self.transaction_mode = mode;
		self
	}

	/// Enable automatic transaction rollback after the test.
	///
	/// This is a convenience method for `with_transaction_mode(TransactionMode::Rollback)`.
	pub fn with_transaction_rollback(self) -> Self {
		self.with_transaction_mode(TransactionMode::Rollback)
	}

	/// Set a mock session directly.
	///
	/// # Arguments
	///
	/// * `session` - The mock session
	pub fn with_session(mut self, session: MockSession) -> Self {
		self.mock_session = Some(session);
		self
	}

	/// Add a mock session with default configuration.
	pub fn with_mock_session(mut self) -> Self {
		if self.mock_session.is_none() {
			self.mock_session = Some(MockSession::anonymous());
		}
		self
	}

	/// Build the test environment with all configured options.
	///
	/// Returns a `ServerFnTestEnv` containing the injection context and
	/// any additional test state.
	pub fn build(self) -> ServerFnTestEnv {
		let ctx = InjectionContext::builder(self.singleton_scope.clone()).build();

		// Apply all overrides
		for override_fn in self.overrides {
			override_fn(&ctx);
		}

		// Register mock session if present
		if let Some(session) = self.mock_session.clone() {
			ctx.set_singleton(session);
		}

		// Register test user if present
		if let Some(user) = self.test_user.clone() {
			ctx.set_singleton(user);
		}

		// Register mock request if present
		if let Some(request) = self.mock_request.clone() {
			ctx.set_singleton(request);
		}

		ServerFnTestEnv {
			injection_context: ctx,
			mock_session: self.mock_session,
			test_user: self.test_user,
			mock_request: self.mock_request,
			transaction_mode: self.transaction_mode,
			request_headers: self.request_headers,
			csrf_token: self.csrf_token,
		}
	}

	/// Build and return just the injection context.
	///
	/// This is a convenience method when you don't need the full test environment.
	pub fn build_context(self) -> InjectionContext {
		self.build().injection_context
	}
}

/// The built test environment containing all test state.
#[derive(Clone)]
pub struct ServerFnTestEnv {
	/// The injection context for dependency resolution.
	pub injection_context: InjectionContext,
	/// The mock session if configured.
	pub mock_session: Option<MockSession>,
	/// The test user if authenticated.
	pub test_user: Option<TestUser>,
	/// The mock HTTP request if configured.
	pub mock_request: Option<MockHttpRequest>,
	/// The transaction mode.
	pub transaction_mode: TransactionMode,
	/// Request headers.
	pub request_headers: HeaderMap,
	/// CSRF token if set.
	pub csrf_token: Option<String>,
}

impl ServerFnTestEnv {
	/// Get a reference to the injection context.
	pub fn context(&self) -> &InjectionContext {
		&self.injection_context
	}

	/// Check if a user is authenticated.
	pub fn is_authenticated(&self) -> bool {
		self.test_user.is_some() && self.mock_session.as_ref().is_some_and(|s| s.user.is_some())
	}

	/// Get the current user ID if authenticated.
	pub fn user_id(&self) -> Option<Uuid> {
		self.test_user.as_ref().map(|u| u.id)
	}

	/// Check if the user has a specific permission.
	// Fixes #864
	pub fn has_permission(&self, permission: &str) -> bool {
		self.test_user
			.as_ref()
			.is_some_and(|u| u.has_permission(permission))
	}

	/// Check if the user has a specific role.
	pub fn has_role(&self, role: &str) -> bool {
		self.test_user
			.as_ref()
			.is_some_and(|u| u.roles.iter().any(|r| r == role))
	}

	/// Get a request header value.
	pub fn get_header(&self, name: &str) -> Option<&str> {
		self.request_headers.get(name).and_then(|v| v.to_str().ok())
	}
}

impl std::ops::Deref for ServerFnTestEnv {
	type Target = InjectionContext;

	fn deref(&self) -> &Self::Target {
		&self.injection_context
	}
}

/// Result builder for testing server function responses.
///
/// This allows building expected results for assertion comparisons.
#[derive(Debug, Clone)]
pub struct ExpectedResult<T> {
	/// The expected value.
	pub value: Option<T>,
	/// The expected status code.
	pub status: Option<StatusCode>,
	/// Expected headers.
	pub headers: HashMap<String, String>,
}

impl<T> Default for ExpectedResult<T> {
	fn default() -> Self {
		Self {
			value: None,
			status: None,
			headers: HashMap::new(),
		}
	}
}

impl<T> ExpectedResult<T> {
	/// Create a new expected result builder.
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the expected value.
	pub fn with_value(mut self, value: T) -> Self {
		self.value = Some(value);
		self
	}

	/// Set the expected status code.
	pub fn with_status(mut self, status: StatusCode) -> Self {
		self.status = Some(status);
		self
	}

	/// Add an expected header.
	pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.headers.insert(name.into(), value.into());
		self
	}

	/// Expect a successful (200 OK) response.
	pub fn success(self) -> Self {
		self.with_status(StatusCode::OK)
	}

	/// Expect a created (201 Created) response.
	pub fn created(self) -> Self {
		self.with_status(StatusCode::CREATED)
	}

	/// Expect a bad request (400) response.
	pub fn bad_request(self) -> Self {
		self.with_status(StatusCode::BAD_REQUEST)
	}

	/// Expect an unauthorized (401) response.
	pub fn unauthorized(self) -> Self {
		self.with_status(StatusCode::UNAUTHORIZED)
	}

	/// Expect a forbidden (403) response.
	pub fn forbidden(self) -> Self {
		self.with_status(StatusCode::FORBIDDEN)
	}

	/// Expect a not found (404) response.
	pub fn not_found(self) -> Self {
		self.with_status(StatusCode::NOT_FOUND)
	}

	/// Expect a conflict (409) response.
	pub fn conflict(self) -> Self {
		self.with_status(StatusCode::CONFLICT)
	}

	/// Expect an internal server error (500) response.
	pub fn internal_error(self) -> Self {
		self.with_status(StatusCode::INTERNAL_SERVER_ERROR)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_context_builder() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = ServerFnTestContext::new(singleton)
			.with_mock_session()
			.build();

		assert!(ctx.mock_session.is_some());
	}

	#[test]
	fn test_authenticated_user() {
		let singleton = Arc::new(SingletonScope::new());
		let user = TestUser::admin();
		let ctx = ServerFnTestContext::new(singleton)
			.with_authenticated_user(user)
			.build();

		assert!(ctx.is_authenticated());
		assert!(ctx.test_user.is_some());
	}

	#[test]
	fn test_permissions() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = ServerFnTestContext::new(singleton)
			.with_authenticated_user(TestUser::authenticated("alice"))
			.with_permissions(vec!["read", "write"])
			.build();

		assert!(ctx.has_permission("read"));
		assert!(ctx.has_permission("write"));
		assert!(!ctx.has_permission("admin"));
	}

	#[test]
	fn test_csrf_token() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = ServerFnTestContext::new(singleton)
			.with_mock_session()
			.with_csrf_token("test-token")
			.build();

		assert_eq!(ctx.csrf_token.as_deref(), Some("test-token"));
		assert_eq!(ctx.get_header("x-csrf-token"), Some("test-token"));
	}

	#[test]
	fn test_transaction_mode() {
		let singleton = Arc::new(SingletonScope::new());
		let ctx = ServerFnTestContext::new(singleton)
			.with_transaction_rollback()
			.build();

		assert_eq!(ctx.transaction_mode, TransactionMode::Rollback);
	}
}
