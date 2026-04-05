//! Server function testing fixtures.
//!
//! This module provides rstest fixtures for testing server functions,
//! including authentication, request/response mocking, and database
//! transaction management.
//!
//! # Features
//!
//! - `test_user`: Factory for generating test users
//! - `mock_session`: Mock session with configurable authentication
//! - `mock_request`: Mock HTTP request builder
//! - Transaction management with automatic rollback
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_testkit::fixtures::server_fn::*;
//! use reinhardt_testkit::fixtures::di::singleton_scope;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_authenticated_endpoint(
//!     singleton_scope: Arc<SingletonScope>,
//!     test_admin: TestUser,
//! ) {
//!     let ctx = ServerFnTestContext::new(singleton_scope)
//!         .with_authenticated_user(test_admin)
//!         .build();
//!
//!     // Test your server function here
//! }
//! ```

#![cfg(native)]

use rstest::*;

use crate::server_fn::{MockHttpRequest, MockHttpResponse, MockSession, TestTokenClaims, TestUser};

// ============================================================================
// Test User Fixtures
// ============================================================================

/// Fixture providing an anonymous test user.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::test_anonymous;
/// use rstest::*;
///
/// #[rstest]
/// fn test_guest_access(test_anonymous: TestUser) {
///     assert!(!test_anonymous.is_authenticated);
/// }
/// ```
#[fixture]
pub fn test_anonymous() -> TestUser {
	TestUser::anonymous()
}

/// Fixture providing an authenticated test user.
///
/// The user has default username "testuser" and email "test@example.com".
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::test_user;
/// use rstest::*;
///
/// #[rstest]
/// fn test_authenticated_access(test_user: TestUser) {
///     assert!(test_user.is_authenticated);
///     assert_eq!(test_user.username, "testuser");
/// }
/// ```
#[fixture]
pub fn test_user() -> TestUser {
	TestUser::authenticated("testuser")
}

/// Fixture providing an admin test user.
///
/// The admin user has the "admin" role and common admin permissions.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::test_admin;
/// use rstest::*;
///
/// #[rstest]
/// fn test_admin_access(test_admin: TestUser) {
///     assert!(test_admin.has_role("admin"));
/// }
/// ```
#[fixture]
pub fn test_admin() -> TestUser {
	TestUser::admin()
}

// ============================================================================
// Session Fixtures
// ============================================================================

/// Fixture providing an anonymous (unauthenticated) session.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::anonymous_session;
/// use rstest::*;
///
/// #[rstest]
/// fn test_guest_session(anonymous_session: MockSession) {
///     assert!(anonymous_session.user.is_none());
/// }
/// ```
#[fixture]
pub fn anonymous_session() -> MockSession {
	MockSession::anonymous()
}

/// Fixture providing an authenticated session with a regular user.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::authenticated_session;
/// use rstest::*;
///
/// #[rstest]
/// fn test_user_session(authenticated_session: MockSession) {
///     assert!(authenticated_session.user.is_some());
/// }
/// ```
#[fixture]
pub fn authenticated_session(test_user: TestUser) -> MockSession {
	MockSession::authenticated(test_user)
}

/// Fixture providing an admin session.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::admin_session;
/// use rstest::*;
///
/// #[rstest]
/// fn test_admin_session(admin_session: MockSession) {
///     let user = admin_session.user.as_ref().unwrap();
///     assert!(user.has_role("admin"));
/// }
/// ```
#[fixture]
pub fn admin_session(test_admin: TestUser) -> MockSession {
	MockSession::authenticated(test_admin)
}

// ============================================================================
// Request/Response Fixtures
// ============================================================================

/// Fixture providing a basic GET request.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::get_request;
/// use rstest::*;
///
/// #[rstest]
/// fn test_get_endpoint(get_request: MockHttpRequest) {
///     assert_eq!(get_request.method.as_str(), "GET");
/// }
/// ```
#[fixture]
pub fn get_request() -> MockHttpRequest {
	MockHttpRequest::get("/api/test")
}

/// Fixture providing a basic POST request.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::post_request;
/// use rstest::*;
///
/// #[rstest]
/// fn test_post_endpoint(post_request: MockHttpRequest) {
///     assert_eq!(post_request.method.as_str(), "POST");
/// }
/// ```
#[fixture]
pub fn post_request() -> MockHttpRequest {
	MockHttpRequest::post("/api/test")
}

/// Fixture providing an OK (200) response.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::ok_response;
/// use rstest::*;
///
/// #[rstest]
/// fn test_success_response(ok_response: MockHttpResponse) {
///     assert_eq!(ok_response.status.as_u16(), 200);
/// }
/// ```
#[fixture]
pub fn ok_response() -> MockHttpResponse {
	MockHttpResponse::ok()
}

// ============================================================================
// Token/JWT Fixtures
// ============================================================================

/// Fixture providing basic JWT token claims.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::token_claims;
/// use rstest::*;
///
/// #[rstest]
/// fn test_jwt_validation(token_claims: TestTokenClaims) {
///     assert!(!token_claims.is_expired());
/// }
/// ```
#[fixture]
pub fn token_claims(test_user: TestUser) -> TestTokenClaims {
	TestTokenClaims::for_user(&test_user)
}

/// Fixture providing expired JWT token claims.
///
/// Useful for testing token expiration handling.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_testkit::fixtures::server_fn::expired_token_claims;
/// use rstest::*;
///
/// #[rstest]
/// fn test_expired_token(expired_token_claims: TestTokenClaims) {
///     assert!(expired_token_claims.is_expired());
/// }
/// ```
#[fixture]
pub fn expired_token_claims(test_user: TestUser) -> TestTokenClaims {
	// Use negative duration to create an already-expired token
	TestTokenClaims::for_user(&test_user).expires_in(-3600)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[rstest]
	fn test_anonymous_fixture(test_anonymous: TestUser) {
		assert!(!test_anonymous.is_authenticated);
	}

	#[rstest]
	fn test_user_fixture(test_user: TestUser) {
		assert!(test_user.is_authenticated);
		assert_eq!(test_user.username, "testuser");
	}

	#[rstest]
	fn test_admin_fixture(test_admin: TestUser) {
		assert!(test_admin.has_role("admin"));
	}

	#[rstest]
	fn test_anonymous_session_fixture(anonymous_session: MockSession) {
		assert!(anonymous_session.user.is_none());
	}

	#[rstest]
	fn test_authenticated_session_fixture(authenticated_session: MockSession) {
		assert!(authenticated_session.user.is_some());
	}

	#[rstest]
	fn test_admin_session_fixture(admin_session: MockSession) {
		let user = admin_session.user.as_ref().unwrap();
		assert!(user.has_role("admin"));
	}

	#[rstest]
	fn test_get_request_fixture(get_request: MockHttpRequest) {
		assert_eq!(get_request.method.as_str(), "GET");
	}

	#[rstest]
	fn test_post_request_fixture(post_request: MockHttpRequest) {
		assert_eq!(post_request.method.as_str(), "POST");
	}

	#[rstest]
	fn test_ok_response_fixture(ok_response: MockHttpResponse) {
		assert_eq!(ok_response.status.as_u16(), 200);
	}

	#[rstest]
	fn test_token_claims_fixture(token_claims: TestTokenClaims) {
		assert!(!token_claims.is_expired());
	}

	#[rstest]
	fn test_expired_token_claims_fixture(expired_token_claims: TestTokenClaims) {
		assert!(expired_token_claims.is_expired());
	}
}
