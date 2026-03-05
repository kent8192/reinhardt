//! Global state management using React-like hooks
//!
//! This module provides reactive global state for the application using the
//! Context system and hooks API.

use crate::apps::auth::shared::types::UserInfo;
use reinhardt::pages::{Context, Signal, get_context, provide_context, remove_context, use_state};

thread_local! {
	static AUTH_CONTEXT: Context<Signal<Option<UserInfo>>> = Context::new();
}

/// Initialize the authentication state
///
/// This must be called once at application startup.
/// Calling it multiple times is safe (subsequent calls are no-ops).
pub fn init_auth_state() {
	AUTH_CONTEXT.with(|ctx| {
		if get_context(ctx).is_none() {
			let (user_signal, _) = use_state(None::<UserInfo>);
			provide_context(ctx, user_signal);
		}
	});
}

/// Hook to get the current authentication state as a reactive Signal
///
/// # Returns
///
/// A `Signal<Option<UserInfo>>` that can be read with `.get()` and will
/// trigger reactive updates when the user changes.
pub fn use_auth() -> Signal<Option<UserInfo>> {
	AUTH_CONTEXT.with(|ctx| {
		get_context(ctx).expect("Auth state not initialized. Call init_auth_state() first.")
	})
}

/// Hook to check if a user is currently authenticated
pub fn use_is_authenticated() -> bool {
	use_auth().get().is_some()
}

/// Get the current authenticated user
pub fn get_current_user() -> Option<UserInfo> {
	use_auth().get()
}

/// Set the current authenticated user
///
/// Updates the authentication state, triggering reactive updates in all
/// components that depend on the auth state.
pub fn set_current_user(user: Option<UserInfo>) {
	use_auth().set(user);
}

/// Check if a user is currently authenticated
pub fn is_authenticated() -> bool {
	use_is_authenticated()
}

/// Clear all authentication state
///
/// This should be called when cleaning up or during testing.
pub fn clear_auth_state() {
	AUTH_CONTEXT.with(|ctx| {
		remove_context::<Signal<Option<UserInfo>>>(ctx);
	});
}
