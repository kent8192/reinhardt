//! Authentication state stored in request extensions.
//!
//! This module provides [`AuthState`], a helper struct that stores
//! authentication information in request extensions.
//!
//! `AuthState` uses a private validation marker to prevent external construction
//! via struct literal syntax. Only the provided constructors
//! ([`AuthState::authenticated`], [`AuthState::anonymous`], [`AuthState::from_extensions`])
//! can create valid instances, preventing type collision attacks where
//! malicious code could insert a spoofed auth state into request extensions.

use crate::Extensions;

/// Private marker to validate that an `AuthState` was created through
/// official constructors, not through external struct literal construction.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AuthStateMarker;

/// Helper struct to store authentication state in request extensions.
///
/// This struct is used by authentication middleware to communicate
/// the authenticated user's information to downstream handlers.
///
/// The struct contains a private field to prevent external construction
/// via struct literal syntax. Use the provided constructors instead.
///
/// # Security Note
///
/// If this state is serialized and sent to client-side code (e.g., in
/// a WASM SPA), the permission checks (`is_authenticated()`,
/// `is_admin()`, `is_active()`) should only be used for **UI display
/// purposes** (showing/hiding elements). An attacker can modify
/// client-side state, so all authorization decisions must be enforced
/// server-side through authentication middleware and permission
/// classes (see `reinhardt-auth`).
///
/// # Example
///
/// ```rust,no_run
/// # use reinhardt_http::AuthState;
/// # struct Request { extensions: Extensions }
/// # struct Extensions;
/// # impl Extensions {
/// #     fn insert<T>(&mut self, _value: T) {}
/// #     fn get<T>(&self) -> Option<T> { None }
/// # }
/// # let mut request = Request { extensions: Extensions };
/// // In middleware (after authentication)
/// request.extensions.insert(AuthState::authenticated("123", false, true));
///
/// // In handler (via CurrentUser or directly)
/// let auth_state: Option<AuthState> = request.extensions.get();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthState {
	/// The authenticated user's ID as a string.
	///
	/// This is typically a UUID or database primary key serialized to string.
	user_id: String,

	/// Whether the user is authenticated.
	is_authenticated: bool,

	/// Whether the user has admin/superuser privileges.
	is_admin: bool,

	/// Whether the user's account is active.
	is_active: bool,

	/// Private validation marker to prevent external construction.
	_marker: AuthStateMarker,
}

impl AuthState {
	/// Creates a new authenticated state.
	///
	/// # Arguments
	///
	/// * `user_id` - The authenticated user's ID
	/// * `is_admin` - Whether the user has admin privileges
	/// * `is_active` - Whether the user's account is active
	pub fn authenticated(user_id: impl Into<String>, is_admin: bool, is_active: bool) -> Self {
		Self {
			user_id: user_id.into(),
			is_authenticated: true,
			is_admin,
			is_active,
			_marker: AuthStateMarker,
		}
	}

	/// Creates an anonymous (unauthenticated) state.
	pub fn anonymous() -> Self {
		Self {
			user_id: String::new(),
			is_authenticated: false,
			is_admin: false,
			is_active: false,
			_marker: AuthStateMarker,
		}
	}

	/// Create auth state from request extensions.
	///
	/// This method extracts authentication-related data that was stored
	/// as individual values in extensions by the authentication middleware.
	///
	/// # Returns
	///
	/// Returns `Some(AuthState)` if user_id and is_authenticated are found,
	/// `None` otherwise.
	pub fn from_extensions(extensions: &Extensions) -> Option<Self> {
		Some(Self {
			user_id: extensions.get::<String>()?,
			is_authenticated: extensions.get::<bool>()?,
			is_admin: false,
			is_active: false,
			_marker: AuthStateMarker,
		})
	}

	/// Get the authenticated user's ID.
	pub fn user_id(&self) -> &str {
		&self.user_id
	}

	/// Check if the user is authenticated.
	pub fn is_authenticated(&self) -> bool {
		self.is_authenticated
	}

	/// Check if the user has admin privileges.
	pub fn is_admin(&self) -> bool {
		self.is_admin
	}

	/// Check if the user's account is active.
	pub fn is_active(&self) -> bool {
		self.is_active
	}

	/// Check if user is anonymous (not authenticated).
	pub fn is_anonymous(&self) -> bool {
		!self.is_authenticated
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_authenticated() {
		let state = AuthState::authenticated("user-123", true, true);

		assert_eq!(state.user_id(), "user-123");
		assert!(state.is_authenticated());
		assert!(state.is_admin());
		assert!(state.is_active());
	}

	#[test]
	fn test_anonymous() {
		let state = AuthState::anonymous();

		assert!(state.user_id().is_empty());
		assert!(!state.is_authenticated());
		assert!(!state.is_admin());
		assert!(!state.is_active());
	}
}
