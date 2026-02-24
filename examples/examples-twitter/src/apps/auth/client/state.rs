//! Global state management using React-like hooks
//!
//! This module provides reactive global state for the application using the
//! Context system and hooks API.

use crate::apps::auth::shared::types::UserInfo;
#[cfg(server)]
use reinhardt::pages::{Context, Signal, get_context, provide_context, remove_context, use_state};
#[cfg(client)]
use reinhardt::pages::{Context, Signal, get_context, provide_context, remove_context, use_state};
use std::cell::RefCell;

// Authentication context (thread-local for initialization)
thread_local! {
	static AUTH_CONTEXT: Context<Signal<Option<UserInfo>>> = Context::new();
	static AUTH_INITIALIZED: RefCell<bool> = const { RefCell::new(false) };
}

/// Initialize the authentication state
///
/// This must be called once at application startup.
pub fn init_auth_state() {
	AUTH_CONTEXT.with(|ctx| {
		AUTH_INITIALIZED.with(|initialized| {
			if !*initialized.borrow() {
				// Create a reactive Signal to hold the user state
				let (user_signal, _) = use_state(None::<UserInfo>);
				// Provide it to the context
				provide_context(ctx, user_signal);
				*initialized.borrow_mut() = true;
			}
		});
	});
}

/// Hook to get the current authentication state as a reactive Signal
///
/// # Returns
///
/// A `Signal<Option<UserInfo>>` that can be read with `.get()` and will
/// trigger reactive updates when the user changes.
///
/// # Example
///
/// ```ignore
/// let user_signal = use_auth();
///
/// // Read current user
/// if let Some(user) = user_signal.get() {
///     log!("Logged in as: {}", user.username);
/// }
/// ```
pub fn use_auth() -> Signal<Option<UserInfo>> {
	AUTH_CONTEXT.with(|ctx| {
		get_context(ctx).expect("Auth state not initialized. Call init_auth_state() first.")
	})
}

/// Hook to check if a user is currently authenticated
///
/// # Returns
///
/// `true` if a user is logged in, `false` otherwise
///
/// # Example
///
/// ```ignore
/// if use_is_authenticated() {
///     // Show logged-in content
/// } else {
///     // Show login form
/// }
/// ```
pub fn use_is_authenticated() -> bool {
	use_auth().get().is_some()
}

/// Get the current authenticated user
///
/// This is a convenience function for reading the current user value.
///
/// # Returns
///
/// `Some(UserInfo)` if logged in, `None` otherwise
pub fn get_current_user() -> Option<UserInfo> {
	use_auth().get()
}

/// Set the current authenticated user
///
/// Updates the authentication state, triggering reactive updates in all
/// components that depend on the auth state.
///
/// # Arguments
///
/// * `user` - The user info to set, or `None` to log out
///
/// # Example
///
/// ```ignore
/// // Log in
/// set_current_user(Some(user_info));
///
/// // Log out
/// set_current_user(None);
/// ```
pub fn set_current_user(user: Option<UserInfo>) {
	use_auth().set(user);
}

/// Check if a user is currently authenticated
///
/// This is a convenience function that returns a boolean.
pub fn is_authenticated() -> bool {
	use_is_authenticated()
}

/// Clear all authentication state
///
/// This should be called when cleaning up or during testing.
pub fn clear_auth_state() {
	AUTH_CONTEXT.with(|ctx| {
		AUTH_INITIALIZED.with(|initialized| {
			if *initialized.borrow() {
				remove_context::<Signal<Option<UserInfo>>>(ctx);
				*initialized.borrow_mut() = false;
			}
		});
	});
}
