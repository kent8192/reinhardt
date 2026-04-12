//! Authentication and authorization mocking for server function testing.
//!
//! This module provides utilities for simulating authenticated users,
//! sessions, and permissions in server function tests.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_testkit::server_fn::auth::{TestUser, MockSession};
//!
//! // Create an admin user
//! let admin = TestUser::admin();
//!
//! // Create a user with specific permissions
//! let user = TestUser::authenticated("alice")
//!     .with_permission("posts:read")
//!     .with_permission("posts:write")
//!     .with_role("editor");
//!
//! // Create an authenticated session
//! let session = MockSession::authenticated(user);
//! ```

#![cfg(native)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// A test user for simulating authentication in tests.
///
/// This struct represents a user that can be injected into the test context
/// to simulate authenticated requests.
///
/// # Example
///
/// ```rust,ignore
/// // Anonymous user
/// let anon = TestUser::anonymous();
///
/// // Simple authenticated user
/// let user = TestUser::authenticated("alice");
///
/// // Admin user with full permissions
/// let admin = TestUser::admin();
///
/// // Custom user with specific attributes
/// let custom = TestUser::authenticated("bob")
///     .with_email("bob@example.com")
///     .with_permission("admin:read")
///     .with_role("moderator");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestUser {
	/// Unique user identifier.
	pub id: Uuid,
	/// Username.
	pub username: String,
	/// Email address.
	pub email: String,
	/// List of permissions granted to this user.
	pub permissions: Vec<String>,
	/// List of roles assigned to this user.
	pub roles: Vec<String>,
	/// Whether the user is authenticated.
	pub is_authenticated: bool,
	/// Additional custom attributes.
	pub attributes: HashMap<String, Value>,
}

impl Default for TestUser {
	fn default() -> Self {
		Self {
			id: Uuid::now_v7(),
			username: String::new(),
			email: String::new(),
			permissions: Vec::new(),
			roles: Vec::new(),
			is_authenticated: false,
			attributes: HashMap::new(),
		}
	}
}

impl TestUser {
	/// Create an anonymous (unauthenticated) user.
	pub fn anonymous() -> Self {
		Self::default()
	}

	/// Create an authenticated user with the given username.
	pub fn authenticated(username: impl Into<String>) -> Self {
		let username = username.into();
		Self {
			id: Uuid::now_v7(),
			email: format!("{}@test.example.com", username),
			username,
			is_authenticated: true,
			..Default::default()
		}
	}

	/// Create an admin user with full permissions.
	pub fn admin() -> Self {
		Self {
			id: Uuid::now_v7(),
			username: "admin".to_string(),
			email: "admin@test.example.com".to_string(),
			permissions: vec![
				"admin".to_string(),
				"*".to_string(), // Wildcard permission
			],
			roles: vec!["admin".to_string(), "superuser".to_string()],
			is_authenticated: true,
			attributes: HashMap::new(),
		}
	}

	/// Create a user with a specific ID.
	pub fn with_id(mut self, id: Uuid) -> Self {
		self.id = id;
		self
	}

	/// Set the email address.
	pub fn with_email(mut self, email: impl Into<String>) -> Self {
		self.email = email.into();
		self
	}

	/// Add a permission.
	pub fn with_permission(mut self, permission: impl Into<String>) -> Self {
		self.permissions.push(permission.into());
		self
	}

	/// Add multiple permissions.
	pub fn with_permissions<S: Into<String>>(
		mut self,
		permissions: impl IntoIterator<Item = S>,
	) -> Self {
		for perm in permissions {
			self.permissions.push(perm.into());
		}
		self
	}

	/// Add a role.
	pub fn with_role(mut self, role: impl Into<String>) -> Self {
		self.roles.push(role.into());
		self
	}

	/// Add multiple roles.
	pub fn with_roles<S: Into<String>>(mut self, roles: impl IntoIterator<Item = S>) -> Self {
		for role in roles {
			self.roles.push(role.into());
		}
		self
	}

	/// Add a custom attribute.
	pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
		self.attributes.insert(key.into(), value.into());
		self
	}

	/// Check if the user has a specific permission.
	///
	/// Also checks for wildcard (*) permission.
	pub fn has_permission(&self, permission: &str) -> bool {
		self.permissions.iter().any(|p| p == permission || p == "*")
	}

	/// Check if the user has a specific role.
	pub fn has_role(&self, role: &str) -> bool {
		self.roles.iter().any(|r| r == role)
	}

	/// Check if the user has any of the given permissions.
	pub fn has_any_permission(&self, permissions: &[&str]) -> bool {
		permissions.iter().any(|p| self.has_permission(p))
	}

	/// Check if the user has all of the given permissions.
	pub fn has_all_permissions(&self, permissions: &[&str]) -> bool {
		permissions.iter().all(|p| self.has_permission(p))
	}

	/// Get a custom attribute.
	pub fn get_attribute(&self, key: &str) -> Option<&Value> {
		self.attributes.get(key)
	}
}

/// A mock session for testing session-based functionality.
///
/// This simulates a server-side session that can store user information
/// and arbitrary session data.
///
/// # Example
///
/// ```rust,ignore
/// // Anonymous session
/// let anon_session = MockSession::anonymous();
///
/// // Authenticated session
/// let auth_session = MockSession::authenticated(TestUser::authenticated("alice"));
///
/// // Session with custom data
/// let session = MockSession::anonymous()
///     .with_data("cart_id", serde_json::json!("abc123"))
///     .with_data("theme", serde_json::json!("dark"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MockSession {
	/// Session ID.
	pub id: String,
	/// The authenticated user, if any.
	pub user: Option<TestUser>,
	/// Session data storage.
	pub data: HashMap<String, Value>,
	/// CSRF token for the session.
	pub csrf_token: String,
	/// Session creation timestamp (Unix epoch seconds).
	pub created_at: i64,
	/// Session expiration timestamp (Unix epoch seconds).
	pub expires_at: Option<i64>,
	/// Whether the session has been invalidated.
	pub invalidated: bool,
}

impl MockSession {
	/// Create a new anonymous session.
	pub fn anonymous() -> Self {
		Self {
			id: Uuid::now_v7().to_string(),
			user: None,
			data: HashMap::new(),
			csrf_token: generate_csrf_token(),
			created_at: chrono::Utc::now().timestamp(),
			expires_at: None,
			invalidated: false,
		}
	}

	/// Create an authenticated session with the given user.
	pub fn authenticated(user: TestUser) -> Self {
		Self {
			id: Uuid::now_v7().to_string(),
			user: Some(user),
			data: HashMap::new(),
			csrf_token: generate_csrf_token(),
			created_at: chrono::Utc::now().timestamp(),
			expires_at: None,
			invalidated: false,
		}
	}

	/// Create an authenticated session from a [`SessionIdentity`](crate::auth::SessionIdentity).
	///
	/// Bridges the shared identity type to the server_fn test context.
	/// Creates a stub `TestUser` internally to maintain `is_authenticated()` compatibility.
	pub fn from_identity(identity: &crate::auth::SessionIdentity) -> Self {
		let stub_user = TestUser {
			id: uuid::Uuid::parse_str(&identity.user_id).unwrap_or(uuid::Uuid::nil()),
			username: identity.user_id.clone(),
			email: String::new(),
			permissions: Vec::new(),
			roles: Vec::new(),
			is_authenticated: true,
			attributes: HashMap::new(),
		};
		let mut session = Self::authenticated(stub_user);
		session
			.data
			.insert("user_id".into(), serde_json::json!(identity.user_id));
		session
			.data
			.insert("is_staff".into(), serde_json::json!(identity.is_staff));
		session.data.insert(
			"is_superuser".into(),
			serde_json::json!(identity.is_superuser),
		);
		session
	}

	/// Set a custom session ID.
	pub fn with_id(mut self, id: impl Into<String>) -> Self {
		self.id = id.into();
		self
	}

	/// Add session data.
	pub fn with_data(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
		self.data.insert(key.into(), value.into());
		self
	}

	/// Set the CSRF token.
	pub fn with_csrf_token(mut self, token: impl Into<String>) -> Self {
		self.csrf_token = token.into();
		self
	}

	/// Set the session expiration.
	pub fn with_expiration(mut self, expires_at: i64) -> Self {
		self.expires_at = Some(expires_at);
		self
	}

	/// Set session to expire after the given duration in seconds.
	pub fn expires_in(mut self, seconds: i64) -> Self {
		self.expires_at = Some(chrono::Utc::now().timestamp() + seconds);
		self
	}

	/// Check if the session is authenticated.
	pub fn is_authenticated(&self) -> bool {
		self.user.is_some() && !self.invalidated
	}

	/// Check if the session has expired.
	pub fn is_expired(&self) -> bool {
		if let Some(expires_at) = self.expires_at {
			chrono::Utc::now().timestamp() > expires_at
		} else {
			false
		}
	}

	/// Check if the session is valid (not invalidated and not expired).
	pub fn is_valid(&self) -> bool {
		!self.invalidated && !self.is_expired()
	}

	/// Get session data by key.
	pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
		self.data
			.get(key)
			.and_then(|v| serde_json::from_value(v.clone()).ok())
	}

	/// Get raw session data by key.
	pub fn get_raw(&self, key: &str) -> Option<&Value> {
		self.data.get(key)
	}

	/// Set session data.
	pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
		self.data.insert(key.into(), value.into());
	}

	/// Remove session data by key.
	pub fn remove(&mut self, key: &str) -> Option<Value> {
		self.data.remove(key)
	}

	/// Clear all session data (but keep user).
	pub fn clear_data(&mut self) {
		self.data.clear();
	}

	/// Invalidate the session.
	pub fn invalidate(&mut self) {
		self.invalidated = true;
	}

	/// Get the user ID if authenticated.
	pub fn user_id(&self) -> Option<Uuid> {
		self.user.as_ref().map(|u| u.id)
	}

	/// Regenerate the session ID (for security after login).
	pub fn regenerate_id(&mut self) {
		self.id = Uuid::now_v7().to_string();
	}

	/// Regenerate the CSRF token.
	pub fn regenerate_csrf(&mut self) {
		self.csrf_token = generate_csrf_token();
	}

	/// Verify a CSRF token.
	pub fn verify_csrf(&self, token: &str) -> bool {
		!token.is_empty() && self.csrf_token == token
	}
}

/// Token claims for JWT testing.
///
/// This represents the claims typically found in a JWT token,
/// useful for testing JWT-based authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTokenClaims {
	/// Subject (user ID).
	pub sub: String,
	/// Issued at timestamp.
	pub iat: i64,
	/// Expiration timestamp.
	pub exp: i64,
	/// Issuer.
	pub iss: Option<String>,
	/// Audience.
	pub aud: Option<String>,
	/// Custom claims.
	#[serde(flatten)]
	pub custom: HashMap<String, Value>,
}

impl TestTokenClaims {
	/// Create new token claims for a user.
	pub fn for_user(user: &TestUser) -> Self {
		let now = chrono::Utc::now().timestamp();
		Self {
			sub: user.id.to_string(),
			iat: now,
			exp: now + 3600, // 1 hour default
			iss: None,
			aud: None,
			custom: HashMap::new(),
		}
	}

	/// Set expiration duration in seconds from now.
	pub fn expires_in(mut self, seconds: i64) -> Self {
		self.exp = chrono::Utc::now().timestamp() + seconds;
		self
	}

	/// Set the issuer.
	pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
		self.iss = Some(issuer.into());
		self
	}

	/// Set the audience.
	pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
		self.aud = Some(audience.into());
		self
	}

	/// Add a custom claim.
	pub fn with_claim(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
		self.custom.insert(key.into(), value.into());
		self
	}

	/// Check if the token has expired.
	pub fn is_expired(&self) -> bool {
		chrono::Utc::now().timestamp() > self.exp
	}
}

/// Generate a random CSRF token.
fn generate_csrf_token() -> String {
	// Use UUID for simplicity in tests
	Uuid::now_v7().to_string().replace('-', "")
}

/// Test helper for permission assertions.
pub mod assert_permissions {
	use super::*;

	/// Assert that the user has the given permission.
	pub fn has_permission(user: &TestUser, permission: &str) {
		assert!(
			user.has_permission(permission),
			"Expected user '{}' to have permission '{}', but they don't.\nActual permissions: {:?}",
			user.username,
			permission,
			user.permissions
		);
	}

	/// Assert that the user does not have the given permission.
	pub fn lacks_permission(user: &TestUser, permission: &str) {
		assert!(
			!user.has_permission(permission),
			"Expected user '{}' to NOT have permission '{}', but they do.\nActual permissions: {:?}",
			user.username,
			permission,
			user.permissions
		);
	}

	/// Assert that the user has the given role.
	pub fn has_role(user: &TestUser, role: &str) {
		assert!(
			user.has_role(role),
			"Expected user '{}' to have role '{}', but they don't.\nActual roles: {:?}",
			user.username,
			role,
			user.roles
		);
	}

	/// Assert that the user does not have the given role.
	pub fn lacks_role(user: &TestUser, role: &str) {
		assert!(
			!user.has_role(role),
			"Expected user '{}' to NOT have role '{}', but they do.\nActual roles: {:?}",
			user.username,
			role,
			user.roles
		);
	}

	/// Assert that the session is authenticated.
	pub fn is_authenticated(session: &MockSession) {
		assert!(
			session.is_authenticated(),
			"Expected session to be authenticated, but it's not."
		);
	}

	/// Assert that the session is anonymous.
	pub fn is_anonymous(session: &MockSession) {
		assert!(
			!session.is_authenticated(),
			"Expected session to be anonymous, but it's authenticated."
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_user_anonymous() {
		let user = TestUser::anonymous();
		assert!(!user.is_authenticated);
		assert!(user.permissions.is_empty());
		assert!(user.roles.is_empty());
	}

	#[test]
	fn test_user_authenticated() {
		let user = TestUser::authenticated("alice");
		assert!(user.is_authenticated);
		assert_eq!(user.username, "alice");
		assert!(user.email.contains("alice"));
	}

	#[test]
	fn test_user_admin() {
		let admin = TestUser::admin();
		assert!(admin.is_authenticated);
		assert!(admin.has_permission("admin"));
		assert!(admin.has_permission("anything")); // Wildcard
		assert!(admin.has_role("admin"));
	}

	#[test]
	fn test_user_permissions() {
		let user = TestUser::authenticated("bob")
			.with_permission("read")
			.with_permission("write");

		assert!(user.has_permission("read"));
		assert!(user.has_permission("write"));
		assert!(!user.has_permission("admin"));
	}

	#[test]
	fn test_session_anonymous() {
		let session = MockSession::anonymous();
		assert!(!session.is_authenticated());
		assert!(session.is_valid());
	}

	#[test]
	fn test_session_authenticated() {
		let user = TestUser::authenticated("alice");
		let session = MockSession::authenticated(user);

		assert!(session.is_authenticated());
		assert!(session.user_id().is_some());
	}

	#[test]
	fn test_session_data() {
		let mut session = MockSession::anonymous();
		session.set("key", serde_json::json!("value"));

		let value: Option<String> = session.get("key");
		assert_eq!(value, Some("value".to_string()));
	}

	#[test]
	fn test_session_csrf() {
		let session = MockSession::anonymous().with_csrf_token("test-token");

		assert!(session.verify_csrf("test-token"));
		assert!(!session.verify_csrf("wrong-token"));
		assert!(!session.verify_csrf(""));
	}

	#[test]
	fn test_session_expiration() {
		let expired = MockSession::anonymous().expires_in(-100);
		assert!(expired.is_expired());
		assert!(!expired.is_valid());

		let valid = MockSession::anonymous().expires_in(3600);
		assert!(!valid.is_expired());
		assert!(valid.is_valid());
	}

	#[test]
	fn test_token_claims() {
		let user = TestUser::authenticated("alice");
		let claims = TestTokenClaims::for_user(&user)
			.expires_in(3600)
			.with_issuer("test-issuer")
			.with_claim("role", "user");

		assert_eq!(claims.sub, user.id.to_string());
		assert!(!claims.is_expired());
		assert_eq!(claims.iss, Some("test-issuer".to_string()));
	}
}
