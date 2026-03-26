// The `User` trait is deprecated in favour of the new `#[model]`-based user macro system.
// Re-exported here for downstream compatibility during the transition period.
#![allow(deprecated)]

//! REST API authentication
//!
//! Re-exports authentication types from reinhardt-core::auth.

// Re-export core authentication types from reinhardt-auth
pub use reinhardt_auth::{
	AllowAny, AnonymousUser, AuthBackend, IsAdminUser, IsAuthenticated, IsAuthenticatedOrReadOnly,
	Permission, SimpleUser, User,
};

// Re-export JWT types conditionally from reinhardt-auth
#[cfg(feature = "jwt")]
pub use reinhardt_auth::{Claims, JwtAuth};

/// Authentication result (REST-specific utility)
#[derive(Debug, Clone)]
pub enum AuthResult<U> {
	/// Successfully authenticated user
	Authenticated(U),

	/// Anonymous (unauthenticated) user
	Anonymous,

	/// Authentication failed
	Failed(String),
}

impl<U> AuthResult<U> {
	/// Check if authenticated
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::authentication::AuthResult;
	///
	/// let result = AuthResult::<String>::Authenticated("user123".to_string());
	/// assert!(result.is_authenticated());
	///
	/// let anonymous = AuthResult::<String>::Anonymous;
	/// assert!(!anonymous.is_authenticated());
	/// ```
	pub fn is_authenticated(&self) -> bool {
		matches!(self, AuthResult::Authenticated(_))
	}
	/// Get user if authenticated
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::authentication::AuthResult;
	///
	/// let result = AuthResult::Authenticated("user123".to_string());
	/// assert_eq!(result.user(), Some("user123".to_string()));
	///
	/// let anonymous = AuthResult::<String>::Anonymous;
	/// assert_eq!(anonymous.user(), None);
	/// ```
	pub fn user(self) -> Option<U> {
		match self {
			AuthResult::Authenticated(user) => Some(user),
			_ => None,
		}
	}
	/// Get error message if failed
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::authentication::AuthResult;
	///
	/// let result = AuthResult::<String>::Failed("Invalid credentials".to_string());
	/// assert_eq!(result.error(), Some("Invalid credentials"));
	///
	/// let auth = AuthResult::Authenticated("user".to_string());
	/// assert_eq!(auth.error(), None);
	/// ```
	pub fn error(&self) -> Option<&str> {
		match self {
			AuthResult::Failed(msg) => Some(msg),
			_ => None,
		}
	}
}
