//! Authentication state management
//!
//! Uses the framework's `AuthState` for reactive auth state management.
//! Session cookies handle server-side authentication; this module manages
//! client-side awareness of the auth state.

pub use reinhardt::pages::auth::{AuthData, AuthState, auth_state};

use crate::apps::auth::shared::types::UserInfo;

/// Update auth state from a `UserInfo` returned by server functions
pub fn set_current_user(user: Option<UserInfo>) {
	if let Some(user) = user {
		auth_state().update(AuthData {
			is_authenticated: true,
			user_id: None,
			username: Some(user.username.clone()),
			email: Some(user.email.clone()),
			..Default::default()
		});
	} else {
		auth_state().logout();
	}
}

/// Check if a user is currently authenticated
pub fn is_authenticated() -> bool {
	auth_state().is_authenticated()
}

/// Get the current authenticated username
pub fn get_current_username() -> Option<String> {
	auth_state().username()
}

/// Clear all authentication state
pub fn clear_auth_state() {
	auth_state().logout();
}
