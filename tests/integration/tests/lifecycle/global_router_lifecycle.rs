//! Global Router Lifecycle Tests
//!
//! Verifies the specification invariants of the global router registration:
//! `register_router()` → `get_router()` / `is_router_registered()`
//!
//! These tests ensure the router global state behaves according to its contract.

use reinhardt_urls::routers::{
	ServerRouter, clear_router, get_router, is_router_registered, register_router,
};
use rstest::rstest;
use serial_test::serial;

/// Specification: `get_router()` returns `None` when no router is registered.
#[rstest]
#[serial(global_router)]
fn get_router_before_register_returns_none() {
	// Arrange
	clear_router();

	// Act
	let result = get_router();

	// Assert
	assert!(
		result.is_none(),
		"get_router must return None before registration"
	);
}

/// Specification: `is_router_registered()` returns `false` when no router is registered.
#[rstest]
#[serial(global_router)]
fn is_router_registered_before_register_returns_false() {
	// Arrange
	clear_router();

	// Act
	let result = is_router_registered();

	// Assert
	assert!(
		!result,
		"is_router_registered must return false before registration"
	);
}

/// Specification: `get_router()` returns `Some` after registration.
#[rstest]
#[serial(global_router)]
fn get_router_after_register_returns_some() {
	// Arrange
	clear_router();
	let router = ServerRouter::new();
	register_router(router);

	// Act
	let result = get_router();

	// Assert
	assert!(
		result.is_some(),
		"get_router must return Some after registration"
	);
}

/// Specification: `is_router_registered()` returns `true` after registration.
#[rstest]
#[serial(global_router)]
fn is_router_registered_after_register_returns_true() {
	// Arrange
	clear_router();
	let router = ServerRouter::new();
	register_router(router);

	// Act
	let result = is_router_registered();

	// Assert
	assert!(
		result,
		"is_router_registered must return true after registration"
	);
}

/// Specification: `is_router_registered()` must be consistent with `get_router().is_some()`.
/// Both APIs must agree at every point in the lifecycle.
#[rstest]
#[serial(global_router)]
fn is_router_registered_consistent_with_get_router() {
	// Arrange
	clear_router();

	// Act & Assert — before registration
	assert_eq!(
		is_router_registered(),
		get_router().is_some(),
		"APIs must agree before registration"
	);

	// Act — register
	let router = ServerRouter::new();
	register_router(router);

	// Assert — after registration
	assert_eq!(
		is_router_registered(),
		get_router().is_some(),
		"APIs must agree after registration"
	);
}

/// Specification: `clear_router()` resets the global state to unregistered.
#[rstest]
#[serial(global_router)]
fn clear_router_resets_state() {
	// Arrange
	let router = ServerRouter::new();
	register_router(router);

	// Act
	clear_router();

	// Assert
	assert!(
		get_router().is_none(),
		"get_router must return None after clear"
	);
	assert!(
		!is_router_registered(),
		"is_router_registered must return false after clear"
	);
}

/// Specification: After clearing, re-registration must succeed normally.
#[rstest]
#[serial(global_router)]
fn re_register_after_clear_succeeds() {
	// Arrange
	clear_router();
	let first_router = ServerRouter::new();
	register_router(first_router);
	clear_router();

	// Act
	let second_router = ServerRouter::new();
	register_router(second_router);

	// Assert
	assert!(
		get_router().is_some(),
		"get_router must return Some after re-registration"
	);
	assert!(
		is_router_registered(),
		"is_router_registered must return true after re-registration"
	);
}
